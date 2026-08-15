//! The session state machine.
//!
//! [`SessionState`] used to be advisory: `Session` held it as a plain field
//! next to a separate `eof: bool`, exposed a public `set_state`, and updated
//! the two independently. A session whose child had closed its output carried
//! `eof == true` and `state == Running`, so it reported itself usable and
//! accepted writes that could never arrive.
//!
//! This type is the single writer. Every transition goes through one of its
//! methods, so the legal-transition table in [`SessionState`]'s documentation
//! is enforced in one place rather than restated at each call site.

use crate::types::{ProcessExitStatus, SessionState};

/// Owns a session's state and enforces the legal transitions between states.
///
/// Terminal states — `Exited`, `Closed`, `Failed` — absorb: once reached, no
/// later observation moves the session out of them. This matters because EOF
/// and reaping race in practice; a late `reached_eof` must not pull an already
/// reaped session back out of `Exited`.
///
/// [`SessionState::Interacting`] has no transition here yet. `interact()`
/// hands out a builder holding only the transport, so the session is not
/// reachable when the interact loop ends and a session that entered
/// `Interacting` could never leave it. The transition lands with the work that
/// funnels `interact()` back through the session.
#[derive(Debug, Clone)]
pub(crate) struct SessionLifecycle {
    state: SessionState,
    /// Whether a read has observed end of output.
    ///
    /// Kept separately from `state` rather than derived from it, because it
    /// must stay true after the session moves on to `Exited`: `wait` and the
    /// expect helpers loop on "have we reached EOF", not "are we at EOF right
    /// now".
    eof: bool,
}

impl SessionLifecycle {
    /// A session that has been constructed but has not yet run.
    pub(crate) const fn new() -> Self {
        Self {
            state: SessionState::Starting,
            eof: false,
        }
    }

    /// The current state.
    pub(crate) const fn state(&self) -> SessionState {
        self.state
    }

    /// Whether a read has observed end of output at any point.
    pub(crate) const fn is_eof(&self) -> bool {
        self.eof
    }

    /// Whether the session has reached a state it can never leave.
    const fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            SessionState::Exited(_) | SessionState::Closed | SessionState::Failed(_)
        )
    }

    /// Whether a write to the child is legal right now.
    ///
    /// False from `Eof` onwards: the child has closed its input, so the write
    /// cannot be delivered. Rejecting here rather than at the syscall is what
    /// makes the error the same on every transport instead of depending on
    /// whether that transport's writes happen to fail after EOF.
    pub(crate) const fn can_send(&self) -> bool {
        matches!(
            self.state,
            SessionState::Starting | SessionState::Running | SessionState::Interacting
        )
    }

    /// The child is spawned and the session is live.
    pub(crate) const fn started(&mut self) {
        if matches!(self.state, SessionState::Starting) {
            self.state = SessionState::Running;
        }
    }

    /// A read observed end of output.
    pub(crate) const fn reached_eof(&mut self) {
        self.eof = true;
        if !self.is_terminal() {
            self.state = SessionState::Eof;
        }
    }

    /// A read failed with an error the session cannot continue past.
    pub(crate) const fn failed(&mut self, kind: std::io::ErrorKind) {
        if !self.is_terminal() {
            self.state = SessionState::Failed(kind);
        }
    }

    /// The child was reaped and its exit status collected.
    pub(crate) const fn exited(&mut self, status: ProcessExitStatus) {
        self.state = SessionState::Exited(status);
    }

    /// The transport reported that the peer is gone during a write.
    pub(crate) const fn closed(&mut self) {
        if !self.is_terminal() {
            self.state = SessionState::Closed;
        }
    }
}

impl Default for SessionLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::SessionLifecycle;
    use crate::types::{ProcessExitStatus, SessionState};

    #[test]
    fn a_new_session_is_starting_and_writable() {
        let lifecycle = SessionLifecycle::new();
        assert_eq!(lifecycle.state(), SessionState::Starting);
        assert!(lifecycle.can_send());
        assert!(!lifecycle.is_eof());
    }

    #[test]
    fn eof_closes_the_write_side() {
        let mut lifecycle = SessionLifecycle::new();
        lifecycle.started();
        assert!(lifecycle.can_send());

        lifecycle.reached_eof();

        assert_eq!(lifecycle.state(), SessionState::Eof);
        assert!(lifecycle.is_eof());
        assert!(!lifecycle.can_send(), "a child at EOF cannot be written to");
    }

    #[test]
    fn eof_survives_the_move_to_exited() {
        let mut lifecycle = SessionLifecycle::new();
        lifecycle.reached_eof();
        lifecycle.exited(ProcessExitStatus::Exited(0));

        assert_eq!(
            lifecycle.state(),
            SessionState::Exited(ProcessExitStatus::Exited(0))
        );
        assert!(
            lifecycle.is_eof(),
            "callers loop on 'has EOF been seen', which stays true after reaping"
        );
    }

    /// EOF and reaping race: `wait` reaps as soon as a read returns 0, and a
    /// second reader can report EOF just after. That must not un-exit the
    /// session.
    #[test]
    fn a_late_eof_does_not_reopen_an_exited_session() {
        let mut lifecycle = SessionLifecycle::new();
        lifecycle.exited(ProcessExitStatus::Exited(3));

        lifecycle.reached_eof();

        assert_eq!(
            lifecycle.state(),
            SessionState::Exited(ProcessExitStatus::Exited(3))
        );
    }

    #[test]
    fn failure_is_terminal() {
        let mut lifecycle = SessionLifecycle::new();
        lifecycle.failed(std::io::ErrorKind::BrokenPipe);
        assert_eq!(
            lifecycle.state(),
            SessionState::Failed(std::io::ErrorKind::BrokenPipe)
        );

        lifecycle.failed(std::io::ErrorKind::TimedOut);

        assert_eq!(
            lifecycle.state(),
            SessionState::Failed(std::io::ErrorKind::BrokenPipe),
            "the first failure is the one that ended the session"
        );
        assert!(!lifecycle.can_send());
    }
}
