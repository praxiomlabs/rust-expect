//! Interaction hooks for customizing behavior.

use std::sync::Arc;

/// Hook type for input processing.
pub type InputHook = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// Hook type for output processing.
pub type OutputHook = Arc<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// Hook type for events.
pub type EventHook = Arc<dyn Fn(InteractionEvent) + Send + Sync>;

/// Interaction events.
#[derive(Debug, Clone)]
pub enum InteractionEvent {
    /// Session started.
    Started,
    /// Session ended.
    Ended,
    /// Input received from user.
    Input(Vec<u8>),
    /// Output received from session.
    Output(Vec<u8>),
    /// Exit character pressed.
    ExitRequested,
    /// Escape sequence detected.
    EscapeSequence(Vec<u8>),
    /// Window resized.
    Resize {
        /// New column count.
        cols: u16,
        /// New row count.
        rows: u16,
    },
}

/// Hook manager for interaction sessions.
#[derive(Default)]
pub struct HookManager {
    /// Input processing hooks.
    input_hooks: Vec<InputHook>,
    /// Output processing hooks.
    output_hooks: Vec<OutputHook>,
    /// Event notification hooks.
    event_hooks: Vec<EventHook>,
}

impl HookManager {
    /// Create a new hook manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an input processing hook.
    pub fn add_input_hook<F>(&mut self, hook: F)
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.input_hooks.push(Arc::new(hook));
    }

    /// Add an output processing hook.
    pub fn add_output_hook<F>(&mut self, hook: F)
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        self.output_hooks.push(Arc::new(hook));
    }

    /// Add an event notification hook.
    pub fn add_event_hook<F>(&mut self, hook: F)
    where
        F: Fn(InteractionEvent) + Send + Sync + 'static,
    {
        self.event_hooks.push(Arc::new(hook));
    }

    /// Process input through all hooks.
    #[must_use]
    pub fn process_input(&self, mut data: Vec<u8>) -> Vec<u8> {
        for hook in &self.input_hooks {
            data = hook(&data);
        }
        data
    }

    /// Process output through all hooks.
    #[must_use]
    pub fn process_output(&self, mut data: Vec<u8>) -> Vec<u8> {
        for hook in &self.output_hooks {
            data = hook(&data);
        }
        data
    }

    /// Notify all event hooks.
    pub fn notify(&self, event: InteractionEvent) {
        for hook in &self.event_hooks {
            hook(event.clone());
        }
    }

    /// Clear all hooks.
    pub fn clear(&mut self) {
        self.input_hooks.clear();
        self.output_hooks.clear();
        self.event_hooks.clear();
    }
}

impl std::fmt::Debug for HookManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookManager")
            .field("input_hooks", &self.input_hooks.len())
            .field("output_hooks", &self.output_hooks.len())
            .field("event_hooks", &self.event_hooks.len())
            .finish()
    }
}

/// Builder for creating hook chains.
#[derive(Default)]
pub struct HookBuilder {
    manager: HookManager,
}

impl HookBuilder {
    /// Create a new builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add CRLF translation hook.
    #[must_use]
    pub fn with_crlf(mut self) -> Self {
        self.manager.add_input_hook(|data| {
            let mut result = Vec::with_capacity(data.len() * 2);
            for &b in data {
                if b == b'\n' {
                    result.push(b'\r');
                    result.push(b'\n');
                } else {
                    result.push(b);
                }
            }
            result
        });
        self
    }

    /// Add local echo hook.
    #[must_use]
    pub fn with_echo(mut self) -> Self {
        self.manager.add_input_hook(|data| {
            // Echo to stdout
            let _ = std::io::Write::write_all(&mut std::io::stdout(), data);
            let _ = std::io::Write::flush(&mut std::io::stdout());
            data.to_vec()
        });
        self
    }

    /// Add logging hook.
    #[must_use]
    pub fn with_logging(mut self) -> Self {
        self.manager.add_event_hook(|event| match event {
            InteractionEvent::Started => eprintln!("[interact] Session started"),
            InteractionEvent::Ended => eprintln!("[interact] Session ended"),
            InteractionEvent::ExitRequested => eprintln!("[interact] Exit requested"),
            _ => {}
        });
        self
    }

    /// Build the hook manager.
    #[must_use]
    pub fn build(self) -> HookManager {
        self.manager
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_manager_process_input() {
        let mut manager = HookManager::new();
        manager.add_input_hook(|data| data.iter().map(u8::to_ascii_uppercase).collect());

        let result = manager.process_input(b"hello".to_vec());
        assert_eq!(result, b"HELLO");
    }

    #[test]
    fn hook_chain() {
        let mut manager = HookManager::new();
        manager.add_input_hook(|data| {
            let mut v = data.to_vec();
            v.push(b'1');
            v
        });
        manager.add_input_hook(|data| {
            let mut v = data.to_vec();
            v.push(b'2');
            v
        });

        let result = manager.process_input(b"x".to_vec());
        assert_eq!(result, b"x12");
    }

    #[test]
    fn hook_builder() {
        let manager = HookBuilder::new().with_crlf().build();

        let result = manager.process_input(b"a\nb".to_vec());
        assert_eq!(result, b"a\r\nb");
    }
}
