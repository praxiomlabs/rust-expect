//! Human-like typing simulation.
//!
//! This module provides functionality for simulating human typing patterns,
//! including variable delays, typos, and corrections.

use std::time::Duration;

use rand::Rng;

use crate::config::HumanTypingConfig;
use crate::error::Result;

/// A human-like typing simulator.
pub struct HumanTyper {
    /// Configuration for typing behavior.
    config: HumanTypingConfig,
    /// Random number generator.
    rng: rand::rngs::ThreadRng,
}

impl HumanTyper {
    /// Create a new human typer with default configuration.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: HumanTypingConfig::default(),
            rng: rand::rng(),
        }
    }

    /// Create a new human typer with custom configuration.
    #[must_use]
    pub fn with_config(config: HumanTypingConfig) -> Self {
        Self {
            config,
            rng: rand::rng(),
        }
    }

    /// Get the configuration.
    #[must_use]
    pub const fn config(&self) -> &HumanTypingConfig {
        &self.config
    }

    /// Set the configuration.
    pub fn set_config(&mut self, config: HumanTypingConfig) {
        self.config = config;
    }

    /// Generate a random delay between key presses.
    pub fn next_delay(&mut self) -> Duration {
        let base = self.config.base_delay.as_millis() as f64;
        let variance = self.config.variance.as_millis() as f64;

        // Normal-ish distribution around base delay
        let offset = self.rng.random_range(-1.0..1.0) * variance;
        let delay_ms = (base + offset).max(10.0);

        Duration::from_millis(delay_ms as u64)
    }

    /// Check if a typo should be made based on configuration.
    pub fn should_make_typo(&mut self) -> bool {
        self.config.typo_chance > 0.0 && self.rng.random::<f32>() < self.config.typo_chance
    }

    /// Generate a typo for a character.
    ///
    /// Returns the typo character and whether a correction should follow.
    pub fn make_typo(&mut self, c: char) -> (char, bool) {
        // Get nearby keys on QWERTY layout
        let nearby = get_nearby_keys(c);

        if nearby.is_empty() {
            return (c, false);
        }

        let idx = self.rng.random_range(0..nearby.len());
        let typo = nearby[idx];
        let should_correct = self.config.correction_chance > 0.0
            && self.rng.random::<f32>() < self.config.correction_chance;

        (typo, should_correct)
    }

    /// Generate pause duration for thinking.
    pub fn thinking_pause(&mut self) -> Duration {
        let base_ms: f64 = 500.0;
        let variance_ms: f64 = 300.0;
        let offset: f64 = self.rng.random_range(-1.0..1.0) * variance_ms;
        Duration::from_millis((base_ms + offset).max(100.0) as u64)
    }

    /// Plan the keystrokes for a string, including delays and potential typos.
    pub fn plan_typing(&mut self, text: &str) -> Vec<TypeEvent> {
        let mut events = Vec::new();

        for c in text.chars() {
            // Possibly make a typo
            if self.should_make_typo() && c.is_alphabetic() {
                let (typo, should_correct) = self.make_typo(c);

                events.push(TypeEvent::Char(typo));
                events.push(TypeEvent::Delay(self.next_delay()));

                if should_correct {
                    // Pause to "notice" the mistake
                    events.push(TypeEvent::Delay(self.thinking_pause()));
                    // Delete the typo
                    events.push(TypeEvent::Backspace);
                    events.push(TypeEvent::Delay(self.next_delay()));
                    // Type the correct character
                    events.push(TypeEvent::Char(c));
                    events.push(TypeEvent::Delay(self.next_delay()));
                }
            } else {
                events.push(TypeEvent::Char(c));
                events.push(TypeEvent::Delay(self.next_delay()));
            }

            // Add longer pause at word boundaries
            if c == ' ' || c == '.' || c == ',' || c == '\n' {
                events.push(TypeEvent::Delay(Duration::from_millis(
                    self.rng.random_range(50..150),
                )));
            }
        }

        events
    }
}

impl Default for HumanTyper {
    fn default() -> Self {
        Self::new()
    }
}

/// An event in the typing sequence.
#[derive(Debug, Clone)]
pub enum TypeEvent {
    /// Type a character.
    Char(char),
    /// Wait for a duration.
    Delay(Duration),
    /// Press backspace.
    Backspace,
    /// Send a control character.
    Control(u8),
}

impl TypeEvent {
    /// Get the bytes to send for this event.
    #[must_use]
    pub fn as_bytes(&self) -> Option<Vec<u8>> {
        match self {
            Self::Char(c) => {
                let mut buf = [0u8; 4];
                let s = c.encode_utf8(&mut buf);
                Some(s.as_bytes().to_vec())
            }
            Self::Backspace => Some(vec![0x7f]),
            Self::Control(c) => Some(vec![*c]),
            Self::Delay(_) => None,
        }
    }
}

/// Get nearby keys on a QWERTY keyboard layout.
fn get_nearby_keys(c: char) -> Vec<char> {
    let c_lower = c.to_ascii_lowercase();

    let nearby = match c_lower {
        'q' => vec!['w', 'a', 's'],
        'w' => vec!['q', 'e', 'a', 's', 'd'],
        'e' => vec!['w', 'r', 's', 'd', 'f'],
        'r' => vec!['e', 't', 'd', 'f', 'g'],
        't' => vec!['r', 'y', 'f', 'g', 'h'],
        'y' => vec!['t', 'u', 'g', 'h', 'j'],
        'u' => vec!['y', 'i', 'h', 'j', 'k'],
        'i' => vec!['u', 'o', 'j', 'k', 'l'],
        'o' => vec!['i', 'p', 'k', 'l'],
        'p' => vec!['o', 'l'],
        'a' => vec!['q', 'w', 's', 'z'],
        's' => vec!['q', 'w', 'e', 'a', 'd', 'z', 'x'],
        'd' => vec!['w', 'e', 'r', 's', 'f', 'x', 'c'],
        'f' => vec!['e', 'r', 't', 'd', 'g', 'c', 'v'],
        'g' => vec!['r', 't', 'y', 'f', 'h', 'v', 'b'],
        'h' => vec!['t', 'y', 'u', 'g', 'j', 'b', 'n'],
        'j' => vec!['y', 'u', 'i', 'h', 'k', 'n', 'm'],
        'k' => vec!['u', 'i', 'o', 'j', 'l', 'm'],
        'l' => vec!['i', 'o', 'p', 'k'],
        'z' => vec!['a', 's', 'x'],
        'x' => vec!['s', 'd', 'z', 'c'],
        'c' => vec!['d', 'f', 'x', 'v'],
        'v' => vec!['f', 'g', 'c', 'b'],
        'b' => vec!['g', 'h', 'v', 'n'],
        'n' => vec!['h', 'j', 'b', 'm'],
        'm' => vec!['j', 'k', 'n'],
        _ => vec![],
    };

    // Preserve case
    if c.is_uppercase() {
        nearby.into_iter().map(|c| c.to_ascii_uppercase()).collect()
    } else {
        nearby
    }
}

/// Typing speed presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypingSpeed {
    /// Very slow typing (hunt and peck).
    VerySlow,
    /// Slow typing (beginner).
    Slow,
    /// Normal typing speed.
    Normal,
    /// Fast typing speed.
    Fast,
    /// Very fast typing (professional).
    VeryFast,
}

impl TypingSpeed {
    /// Get the configuration for this typing speed.
    #[must_use]
    pub fn config(self) -> HumanTypingConfig {
        match self {
            Self::VerySlow => HumanTypingConfig {
                base_delay: Duration::from_millis(300),
                variance: Duration::from_millis(150),
                typo_chance: 0.03,
                correction_chance: 0.95,
            },
            Self::Slow => HumanTypingConfig {
                base_delay: Duration::from_millis(180),
                variance: Duration::from_millis(80),
                typo_chance: 0.02,
                correction_chance: 0.9,
            },
            Self::Normal => HumanTypingConfig::default(),
            Self::Fast => HumanTypingConfig {
                base_delay: Duration::from_millis(60),
                variance: Duration::from_millis(30),
                typo_chance: 0.02,
                correction_chance: 0.8,
            },
            Self::VeryFast => HumanTypingConfig {
                base_delay: Duration::from_millis(30),
                variance: Duration::from_millis(15),
                typo_chance: 0.03,
                correction_chance: 0.7,
            },
        }
    }
}

/// Extension trait for human-like typing.
pub trait HumanSend {
    /// Send text with human-like typing patterns.
    fn send_human(
        &mut self,
        text: &str,
        config: HumanTypingConfig,
    ) -> impl std::future::Future<Output = Result<()>> + Send;

    /// Send text with a preset typing speed.
    fn send_human_speed(
        &mut self,
        text: &str,
        speed: TypingSpeed,
    ) -> impl std::future::Future<Output = Result<()>> + Send {
        self.send_human(text, speed.config())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_typer_delay() {
        let mut typer = HumanTyper::new();

        let delay = typer.next_delay();
        assert!(delay.as_millis() >= 10);
    }

    #[test]
    fn human_typer_plan() {
        let mut typer = HumanTyper::with_config(HumanTypingConfig {
            typo_chance: 0.0, // Disable typos for predictable testing
            ..Default::default()
        });

        let events = typer.plan_typing("hi");

        // Should have: Char('h'), Delay, Char('i'), Delay
        assert!(events.len() >= 4);
        assert!(matches!(events[0], TypeEvent::Char('h')));
        assert!(matches!(events[2], TypeEvent::Char('i')));
    }

    #[test]
    fn nearby_keys() {
        let nearby = get_nearby_keys('f');
        assert!(nearby.contains(&'d'));
        assert!(nearby.contains(&'g'));
        assert!(!nearby.contains(&'z'));

        let nearby_upper = get_nearby_keys('F');
        assert!(nearby_upper.contains(&'D'));
        assert!(nearby_upper.contains(&'G'));
    }

    #[test]
    fn typing_speed_config() {
        let slow = TypingSpeed::Slow.config();
        let fast = TypingSpeed::Fast.config();

        assert!(slow.base_delay > fast.base_delay);
    }
}
