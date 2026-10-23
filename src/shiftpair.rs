//! Two Shift presses in a row. The key itself is never swallowed: a held
//! Shift still makes a capital letter. Only a second press, soon enough and
//! with no other key between, asks the engine to convert.

use std::time::{Duration, Instant};

/// How soon the second Shift must follow the first.
pub const DOUBLE_SHIFT_GAP: Duration = Duration::from_millis(400);

#[derive(Debug)]
pub struct ShiftPair {
    armed: Option<Instant>,
}

impl ShiftPair {
    pub const fn new() -> Self {
        Self { armed: None }
    }

    pub fn clear(&mut self) {
        self.armed = None;
    }

    /// A Shift that was not already held went down. `true` when this press
    /// completes the pair.
    pub fn press(&mut self, now: Instant) -> bool {
        match self.armed {
            Some(at) if now.saturating_duration_since(at) <= DOUBLE_SHIFT_GAP => {
                self.armed = None;
                true
            }
            _ => {
                self.armed = Some(now);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_press_inside_the_gap_fires() {
        let mut pair = ShiftPair::new();
        let start = Instant::now();
        assert!(!pair.press(start));
        assert!(pair.press(start + Duration::from_millis(400)));
        assert!(!pair.press(start + Duration::from_millis(400)));
    }

    #[test]
    fn a_late_press_starts_over() {
        let mut pair = ShiftPair::new();
        let start = Instant::now();
        assert!(!pair.press(start));
        assert!(!pair.press(start + Duration::from_millis(401)));
        assert!(pair.press(start + Duration::from_millis(700)));
    }

    #[test]
    fn another_key_cancels_the_first_press() {
        let mut pair = ShiftPair::new();
        let start = Instant::now();
        assert!(!pair.press(start));
        pair.clear();
        assert!(!pair.press(start + Duration::from_millis(10)));
    }
}
