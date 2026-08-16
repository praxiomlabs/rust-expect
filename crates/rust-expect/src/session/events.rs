//! The session event stream.
//!
//! A session used to expose exactly one observation point: an output tap, fired
//! with every chunk read from the transport. Everything else a caller might
//! want to observe had no home. Input was the clearest gap — `send()` had no
//! hook at all, so `Recorder::record_input` was unreachable from a session even
//! though the recorder implements it. Resize, state transitions and I/O errors
//! were equally unobservable.
//!
//! This module generalises the tap into [`SessionEvent`], and the tap into one
//! kind of subscriber. Output taps keep working unchanged: they are registered
//! in the same list as full subscribers and fire in registration order with
//! them, so an existing tap and a new subscriber observe output in the order
//! they were added.
//!
//! # Delivery
//!
//! Subscribers are invoked **synchronously, in-line, in registration order**,
//! at the point the event occurs. A slow subscriber slows the session. That was
//! already true of output taps and is the honest behaviour for a library whose
//! job is ordering: a queue would decouple a subscriber's view of "when" from
//! the session's.
//!
//! Because delivery is synchronous, events borrow their payload rather than
//! owning it — a subscriber that wants to keep bytes copies them. This keeps
//! the read path allocation-free and makes "do not stash this" a property of
//! the type rather than a line of documentation.
//!
//! A panicking subscriber is caught and logged, and the remaining subscribers
//! still run. Subscribers are observers, not error sources.

use std::sync::Arc;

use crate::error::ExpectError;
use crate::types::SessionState;

/// Callback invoked for every chunk of bytes read from the transport.
///
/// Taps observe the raw byte stream as it arrives, after it is appended to the
/// matcher buffer but before any pattern matching is performed. They are the
/// foundation for screen emulation, transcript recording, and other features
/// that need to see output as it happens.
///
/// Equivalent to a subscriber that ignores every event except
/// [`SessionEvent::Output`].
pub type OutputTap = Arc<dyn Fn(&[u8]) + Send + Sync>;

/// Callback invoked for every [`SessionEvent`] a session emits.
pub type EventSubscriber = Arc<dyn Fn(&SessionEvent<'_>) + Send + Sync>;

/// Something a session observed.
///
/// Payloads are borrowed for the duration of the call: delivery is synchronous,
/// so a subscriber that needs to retain bytes must copy them.
///
/// This enum is `#[non_exhaustive]`. Match with a `_` arm; new variants are
/// added as more of the session becomes observable.
#[non_exhaustive]
#[derive(Debug)]
pub enum SessionEvent<'a> {
    /// Bytes read from the child, exactly as it produced them — raw, including
    /// any ANSI escape sequences, and before any decoding.
    Output(&'a [u8]),

    /// Bytes written to the child, exactly as they were sent.
    Input(&'a [u8]),

    /// The terminal was resized.
    Resize {
        /// New width in columns.
        cols: u16,
        /// New height in rows.
        rows: u16,
    },

    /// The session moved between states. Emitted only when the state actually
    /// changed.
    StateChanged {
        /// The state being left.
        from: SessionState,
        /// The state being entered.
        to: SessionState,
    },

    /// A pattern matched and was consumed from the buffer.
    ///
    /// `pattern_index` is the index within the pattern set the match came
    /// from — the same number [`Match::pattern_index`] reports — and is `0` for
    /// a single-pattern expect. It identifies the match within its own call,
    /// not across calls: the crate has no stable pattern identity, and a
    /// counter does not need one.
    ///
    /// [`Match::pattern_index`]: crate::types::Match::pattern_index
    Matched {
        /// Index of the pattern that matched, within the set it was matched
        /// against.
        pattern_index: usize,
    },

    /// An I/O error ended a read.
    Error(&'a ExpectError),
}

/// Opaque handle identifying a registered output tap or event subscriber.
///
/// Returned by [`Session::add_output_tap`](crate::Session::add_output_tap) and
/// accepted by [`Session::remove_output_tap`](crate::Session::remove_output_tap).
///
/// Backed by `u64`. The id space is large enough that wraparound is not
/// reachable in practice; the implementation uses a non-wrapping `+= 1`
/// so a hypothetical exhaustion would surface as a loud panic instead of
/// silently colliding with a still-registered tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TapId(u64);

impl std::fmt::Display for TapId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tap#{}", self.0)
    }
}

/// One registered observer.
enum Sink {
    /// An output tap: sees `Output` payloads only.
    Tap(OutputTap),
    /// A full subscriber: sees every event.
    Subscriber(EventSubscriber),
}

/// The session's registered observers, in registration order.
#[derive(Default)]
pub(crate) struct Subscribers {
    sinks: Vec<(TapId, Sink)>,
    /// Monotonic counter for assigning new ids.
    next_id: u64,
}

impl Subscribers {
    pub(crate) const fn new() -> Self {
        Self {
            sinks: Vec::new(),
            next_id: 0,
        }
    }

    /// Allocate the next id.
    ///
    /// Plain addition (not `wrapping_add`): on the astronomically unlikely
    /// event of `u64` exhaustion on a single session, we would rather panic
    /// loudly than silently issue a colliding id.
    const fn next(&mut self) -> TapId {
        let id = TapId(self.next_id);
        self.next_id += 1;
        id
    }

    pub(crate) fn add_tap(&mut self, tap: OutputTap) -> TapId {
        let id = self.next();
        self.sinks.push((id, Sink::Tap(tap)));
        id
    }

    pub(crate) fn add_subscriber(&mut self, subscriber: EventSubscriber) -> TapId {
        let id = self.next();
        self.sinks.push((id, Sink::Subscriber(subscriber)));
        id
    }

    /// Remove a tap or subscriber. Returns whether one was registered.
    pub(crate) fn remove(&mut self, id: TapId) -> bool {
        let before = self.sinks.len();
        self.sinks.retain(|(existing, _)| *existing != id);
        self.sinks.len() != before
    }

    /// How many observers are registered, of either kind.
    pub(crate) const fn len(&self) -> usize {
        self.sinks.len()
    }

    /// The callbacks of every registered output tap, in registration order.
    pub(crate) fn taps(&self) -> impl Iterator<Item = &OutputTap> {
        self.sinks.iter().filter_map(|(_, sink)| match sink {
            Sink::Tap(tap) => Some(tap),
            Sink::Subscriber(_) => None,
        })
    }

    /// Deliver an event to every observer that wants it.
    pub(crate) fn emit(&self, event: &SessionEvent<'_>) {
        for (id, sink) in &self.sinks {
            // Run in catch_unwind so a panicking user callback cannot unwind
            // across an await boundary or stop the remaining subscribers.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match sink {
                Sink::Tap(tap) => {
                    if let SessionEvent::Output(chunk) = event {
                        tap(chunk);
                    }
                }
                Sink::Subscriber(subscriber) => subscriber(event),
            }));
            if result.is_err() {
                tracing::warn!(
                    %id,
                    "session subscriber panicked; the panic was caught and other subscribers continue"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{SessionEvent, Subscribers};
    use crate::types::SessionState;

    #[test]
    fn taps_see_output_and_nothing_else() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let mut subscribers = Subscribers::new();
        subscribers.add_tap(Arc::new(move |chunk: &[u8]| {
            sink.lock().unwrap().push(chunk.to_vec());
        }));

        subscribers.emit(&SessionEvent::Output(b"out"));
        subscribers.emit(&SessionEvent::Input(b"in"));
        subscribers.emit(&SessionEvent::Resize { cols: 80, rows: 24 });

        assert_eq!(*seen.lock().unwrap(), vec![b"out".to_vec()]);
    }

    #[test]
    fn subscribers_see_input_which_taps_never_could() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let mut subscribers = Subscribers::new();
        subscribers.add_subscriber(Arc::new(move |event: &SessionEvent<'_>| {
            if let SessionEvent::Input(bytes) = event {
                sink.lock().unwrap().push(bytes.to_vec());
            }
        }));

        subscribers.emit(&SessionEvent::Input(b"secret"));

        assert_eq!(*seen.lock().unwrap(), vec![b"secret".to_vec()]);
    }

    /// Taps and subscribers share one list so that an old tap and a new
    /// subscriber observe output in the order they were registered.
    #[test]
    fn taps_and_subscribers_fire_in_one_registration_order() {
        let order = Arc::new(Mutex::new(Vec::new()));

        let first = Arc::clone(&order);
        let second = Arc::clone(&order);
        let third = Arc::clone(&order);

        let mut subscribers = Subscribers::new();
        subscribers.add_tap(Arc::new(move |_: &[u8]| {
            first.lock().unwrap().push("tap-1");
        }));
        subscribers.add_subscriber(Arc::new(move |_: &SessionEvent<'_>| {
            second.lock().unwrap().push("subscriber");
        }));
        subscribers.add_tap(Arc::new(move |_: &[u8]| {
            third.lock().unwrap().push("tap-2");
        }));

        subscribers.emit(&SessionEvent::Output(b"x"));

        assert_eq!(*order.lock().unwrap(), vec!["tap-1", "subscriber", "tap-2"]);
    }

    #[test]
    fn a_panicking_subscriber_does_not_stop_the_others() {
        let reached = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&reached);

        let mut subscribers = Subscribers::new();
        subscribers.add_subscriber(Arc::new(|_: &SessionEvent<'_>| {
            panic!("subscriber blew up");
        }));
        subscribers.add_subscriber(Arc::new(move |_: &SessionEvent<'_>| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        subscribers.emit(&SessionEvent::StateChanged {
            from: SessionState::Running,
            to: SessionState::Eof,
        });

        assert_eq!(
            reached.load(Ordering::SeqCst),
            1,
            "the subscriber after the panicking one must still run"
        );
    }

    #[test]
    fn removal_takes_a_tap_out_of_the_stream() {
        let count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&count);
        let mut subscribers = Subscribers::new();
        let id = subscribers.add_tap(Arc::new(move |_: &[u8]| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        subscribers.emit(&SessionEvent::Output(b"x"));
        assert!(subscribers.remove(id));
        subscribers.emit(&SessionEvent::Output(b"x"));

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert!(!subscribers.remove(id), "removing twice reports nothing");
    }
}
