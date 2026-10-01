//! The pre-game pack picker, porting `src/pack_select.c`.
//!
//! This runs BEFORE any pack is open, so it cannot use the bitmap font, the
//! chrome or any sprite -- all of those come from the pack. It draws with
//! macroquad's built-in `draw_text`, exactly as C falls back to raylib's
//! `DrawText` for the same reason.
//!
//! The decision logic below is deliberately separate from the draw loop, as
//! it is in C: one call is one frame, so it is exercised without a window.
//!
//! That makes this the second module, after `backend.rs`, allowed to name
//! macroquad, and the exception stays narrow: text and rectangles only, and
//! the loop is over before the real shell starts.

use macroquad::prelude::{
    clear_background, draw_rectangle, draw_text, is_key_pressed, measure_text, next_frame,
    screen_height, screen_width, Color as MqColor, KeyCode,
};

use crate::pack::PackEntry;

/// Run the picker until a row is confirmed or the user cancels, porting
/// `pack_select_flow`. `None` means "load nothing", which the caller must
/// honour by exiting.
///
/// C reads its window-close signal into `close`. macroquad cannot do that
/// without `prevent_quit`, which cannot be switched back off for the game
/// that follows, so `close` stays false here and a closed window ends the
/// process instead -- which, like C's cancel, loads nothing.
pub async fn run(entries: &[PackEntry]) -> Option<usize> {
    const DIGITS: [KeyCode; 9] = [
        KeyCode::Key1,
        KeyCode::Key2,
        KeyCode::Key3,
        KeyCode::Key4,
        KeyCode::Key5,
        KeyCode::Key6,
        KeyCode::Key7,
        KeyCode::Key8,
        KeyCode::Key9,
    ];
    let mut state = SelectState::default();
    while !state.done {
        // C: `for (i = 0; i < n && i < 9; i++)`, the last pressed row winning.
        let mut digit = None;
        for (index, key) in DIGITS.into_iter().enumerate().take(entries.len()) {
            if is_key_pressed(key) {
                digit = Some(index);
            }
        }
        let input = SelectInput {
            close: false,
            cancel: is_key_pressed(KeyCode::Escape),
            up: is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Kp8),
            down: is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Kp2),
            digit,
            confirm: is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::KpEnter)
                || is_key_pressed(KeyCode::Space),
        };
        select_step(&mut state, &input, entries.len());

        draw(entries, state.cursor);
        next_frame().await;
    }
    (!state.quit).then_some(state.cursor)
}

/// raylib's `DrawText` places a string by its TOP edge; macroquad's places
/// it by its baseline. Shift by a tall reference string's ascent so every
/// call below keeps C's coordinates.
fn draw_text_top(text: &str, x: f32, top: f32, size: u16, color: MqColor) {
    let ascent = measure_text("Mg", None, size, 1.0).offset_y;
    draw_text(text, x, top + ascent, f32::from(size), color);
}

/// C's colours and metrics, centred on the live window rather than C's fixed
/// 640x400, because a high-DPI window reports a larger drawable area.
fn draw(entries: &[PackEntry], cursor: usize) {
    let w = screen_width();
    let h = screen_height();
    let bg = MqColor::from_rgba(16, 16, 32, 255);
    let sel_bg = MqColor::from_rgba(40, 40, 70, 255);
    let white = MqColor::from_rgba(245, 245, 245, 255);
    let yellow = MqColor::from_rgba(253, 249, 0, 255);
    let grey = MqColor::from_rgba(130, 130, 130, 255);

    clear_background(bg);

    let title = "Select Game Pack";
    let tw = measure_text(title, None, 24, 1.0).width;
    draw_text_top(title, (w - tw) / 2.0, 32.0, 24, white);

    let row_h = 24.0;
    let top = (h - entries.len() as f32 * row_h) / 2.0 - 8.0;
    for (index, entry) in entries.iter().enumerate() {
        let x = w / 2.0 - 200.0;
        let y = top + index as f32 * row_h;
        if index == cursor {
            draw_rectangle(x - 6.0, y - 2.0, 412.0, row_h, sel_bg);
        }
        let line = format!(" {}. {}", index + 1, entry.name);
        let fg = if index == cursor { yellow } else { white };
        draw_text_top(&line, x, y, 20, fg);
    }

    let hint = "UP/DN select   ENTER load   ESC quit";
    let hw = measure_text(hint, None, 16, 1.0).width;
    draw_text_top(hint, (w - hw) / 2.0, h - 40.0, 16, grey);
}

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
