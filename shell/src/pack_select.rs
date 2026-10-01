//! The pre-game pack picker, porting `src/pack_select.c`.
//!
//! This runs BEFORE any pack is open, so it cannot use the bitmap font, the
//! chrome or any sprite -- all of those come from the pack. It draws with
//! macroquad's built-in `draw_text`, exactly as C falls back to raylib's
//! `DrawText` for the same reason.
//!
//! The decision logic below is deliberately separate from the draw loop, as
//! it is in C: one call is one frame, so it is exercised without a window.

/// One frame of input, C's `PackSelectInput`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectInput {
    /// Window close signal.
    pub close: bool,
    /// ESC.
    pub cancel: bool,
    pub up: bool,
    pub down: bool,
    /// Zero-based row from a 1..9 keypress. C's `-1` for none is `None`.
    pub digit: Option<usize>,
    /// ENTER / SPACE.
    pub confirm: bool,
}

/// C's `PackSelectState`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectState {
    pub cursor: usize,
    /// The loop should stop.
    pub done: bool,
    /// Stopped WITHOUT a selection; the caller loads nothing.
    pub quit: bool,
}

/// Advance one frame, porting `pack_select_step`.
///
/// Closing the window and pressing ESC both end the flow as a cancel. C's
/// comment on why: without it "the loop fell out with quit unset and the
/// caller booted whatever row the cursor happened to rest on."
pub fn select_step(state: &mut SelectState, input: &SelectInput, n: usize) {
    if n == 0 {
        return;
    }
    if input.close || input.cancel {
        state.quit = true;
        state.done = true;
        return;
    }
    if input.up {
        state.cursor = (state.cursor + n - 1) % n;
    }
    if input.down {
        state.cursor = (state.cursor + 1) % n;
    }
    if let Some(digit) = input.digit {
        if digit < n {
            state.cursor = digit;
        }
    }
    if input.confirm {
        state.done = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SelectInput {
        SelectInput::default()
    }

    #[test]
    fn the_cursor_wraps_in_both_directions() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                down: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 1);
        select_step(
            &mut state,
            &SelectInput {
                up: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 0);
        select_step(
            &mut state,
            &SelectInput {
                up: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 2, "wraps backwards past zero");
        select_step(
            &mut state,
            &SelectInput {
                down: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 0, "wraps forwards past the end");
    }

    #[test]
    fn a_digit_jumps_to_that_row_and_out_of_range_is_ignored() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                digit: Some(2),
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 2);
        select_step(
            &mut state,
            &SelectInput {
                digit: Some(7),
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 2, "out of range leaves the cursor alone");
    }

    #[test]
    fn confirm_finishes_without_quitting() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                confirm: true,
                ..input()
            },
            3,
        );
        assert!(state.done);
        assert!(!state.quit);
    }

    /// C's comment: "A dismissed window is a cancel, not a selection. Without
    /// this the loop fell out with quit unset and the caller booted whatever
    /// row the cursor happened to rest on."
    #[test]
    fn cancel_and_window_close_both_finish_without_selecting() {
        for dismissal in [
            SelectInput {
                cancel: true,
                ..input()
            },
            SelectInput {
                close: true,
                ..input()
            },
        ] {
            let mut state = SelectState {
                cursor: 2,
                ..SelectState::default()
            };
            select_step(&mut state, &dismissal, 3);
            assert!(state.done);
            assert!(state.quit, "a dismissed picker must select nothing");
        }
    }

    /// C guards `n <= 0` and returns; an empty list cannot move a cursor.
    #[test]
    fn an_empty_list_is_inert() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                down: true,
                ..input()
            },
            0,
        );
        assert_eq!(state.cursor, 0);
        assert!(!state.done);
    }

    /// Cancel is checked BEFORE movement, so a frame carrying both cancels.
    #[test]
    fn cancel_beats_movement_in_the_same_frame() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                cancel: true,
                down: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 0, "cancel returns before the cursor moves");
        assert!(state.quit);
    }

    /// C applies up, then down, then the digit, in that order, so a digit in
    /// the same frame as an arrow wins.
    #[test]
    fn a_digit_overrides_an_arrow_in_the_same_frame() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput {
                down: true,
                digit: Some(2),
                confirm: true,
                ..input()
            },
            3,
        );
        assert_eq!(state.cursor, 2);
        assert!(state.done && !state.quit);
    }
}
