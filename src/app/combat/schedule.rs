//! `_DAT_0081D5F0` / `_DAT_0081D568`: the per-frame AI action budget and the
//! round-robin cursor consulted by `FUN_004116CC`.
//!
//! `FUN_00421B50` opens every frame with
//!
//! ```text
//! _DAT_0081D5F0 = 0;                 // charged actions this frame
//! _DAT_0081D570 = _DAT_0081D568;     // cursor inherited from the previous frame
//! ```
//!
//! and closes it with
//!
//! ```text
//! if (_DAT_0081D568 == _DAT_0081D570) _DAT_0081D568 = 0;
//! ```
//!
//! `FUN_004116CC(entity)` is `_DAT_0081D5F0 < 3 && _DAT_0081D568 < entity`: a unit
//! may take its turn only while fewer than three actions have been charged and
//! only if it sorts *after* the cursor. Passing the gate sets the cursor to that
//! entity (`_DAT_0081D568 = entity`) but charges nothing — the three charges come
//! from the equipment refresh (`FUN_004105A0`), the armed pick (`FUN_00410E84`),
//! each line-of-fire probe (`FUN_0041130C`) and the citizen AI (`FUN_004168A4`).
//!
//! Order keys are one-based (`entity.index + 1`): the original compares entity
//! *addresses*, which are never zero, while zero is the "nothing served" cursor.

/// `FUN_004116CC`: at most three charged AI actions per logic frame.
pub const AI_ACTION_BUDGET: u32 = 3;

/// The `FUN_004116CC` budget + cursor pair.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AiSchedule {
    /// `_DAT_0081D5F0`.
    actions: u32,
    /// `_DAT_0081D568`.
    cursor: usize,
    /// `_DAT_0081D570`.
    previous: usize,
}

impl AiSchedule {
    /// `FUN_004116CC(entity)`: budget left and the entity sorts after the cursor.
    pub fn allows(&self, order: usize) -> bool {
        self.actions < AI_ACTION_BUDGET && self.cursor < order
    }

    /// `_DAT_0081D568 = entity`: the unit took its turn this frame.
    pub fn serve(&mut self, order: usize) {
        self.cursor = order;
    }

    /// Charge one work unit (`_DAT_0081D5F0++`).
    pub fn charge(&mut self) {
        self.actions = self.actions.saturating_add(1);
    }

    /// `FUN_00421B50` entry.
    pub fn begin_frame(&mut self) {
        self.actions = 0;
        self.previous = self.cursor;
    }

    /// `FUN_00421B50` exit: reset the cursor when nobody was served.
    pub fn end_frame(&mut self) {
        if self.cursor == self.previous {
            self.cursor = 0;
        }
    }

    /// Charged actions this frame (`_DAT_0081D5F0`).
    pub fn actions(&self) -> u32 {
        self.actions
    }

    /// Current cursor (`_DAT_0081D568`).
    pub fn cursor(&self) -> usize {
        self.cursor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The gate needs budget left *and* a strictly larger order key.
    #[test]
    fn gate_requires_budget_and_an_advancing_order() {
        let mut s = AiSchedule::default();
        assert!(s.allows(1), "empty frame accepts the first unit");
        s.serve(1);
        assert!(!s.allows(1), "the same unit cannot take two turns");
        assert!(!s.allows(0), "orders below the cursor are skipped");
        assert!(s.allows(2), "a later unit passes");
        for _ in 0..AI_ACTION_BUDGET {
            s.charge();
        }
        assert!(!s.allows(3), "the budget stops every later unit");
    }

    /// `begin_frame` clears the charges and remembers the inherited cursor.
    #[test]
    fn begin_frame_clears_the_budget_and_keeps_the_cursor() {
        let mut s = AiSchedule::default();
        s.serve(5);
        s.charge();
        s.begin_frame();
        assert_eq!(s.actions(), 0);
        assert_eq!(s.cursor(), 5);
        assert!(!s.allows(4), "the inherited cursor still filters");
        assert!(s.allows(6));
    }

    /// `end_frame` resets the cursor only when the frame served nobody.
    #[test]
    fn end_frame_resets_only_an_unused_cursor() {
        let mut s = AiSchedule::default();
        s.serve(4);
        s.begin_frame();
        s.serve(9);
        s.end_frame();
        assert_eq!(s.cursor(), 9, "an advancing cursor survives the frame");

        s.begin_frame();
        s.end_frame();
        assert_eq!(s.cursor(), 0, "an idle frame clears the cursor");

        // A frame that serves only entities below the cursor is also idle.
        let mut s = AiSchedule::default();
        s.serve(9);
        s.begin_frame();
        assert!(!s.allows(4));
        s.end_frame();
        assert_eq!(s.cursor(), 0);
    }
}
