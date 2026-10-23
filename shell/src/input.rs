//! Overworld key bindings, porting `src/input.c`.
//!
//! One step per keypress, no auto-repeat: the original is a turn-based game
//! where holding a key must not run the hero across the map.
//!
//! Nothing here touches a graphics library. The backend supplies a
//! [`KeySource`]; a test supplies a set of keys instead, so the whole binding
//! table is exercised without a window.
//!
//! NOT ported: gamepad support. C's `poll_gamepad` is additive to the
//! keyboard and no-ops when no pad is connected, so leaving it out changes
//! nothing for a keyboard player.

use std::collections::BTreeSet;

/// The keys the shell reads. The overworld bindings use a subset of these;
/// the bottom-frame prompt (`crate::prompt`) reads the rest -- digits, Enter,
/// Backspace -- which is why they share this one vocabulary rather than a
/// second enum.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Kp0,
    Kp1,
    Kp2,
    Kp3,
    Kp4,
    Kp5,
    Kp6,
    Kp7,
    Kp8,
    Kp9,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Enter,
    KpEnter,
    Backspace,
    Space,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    /// Answers the fast-quit prompt. Not an overworld binding, so [`poll`]
    /// ignores it; the prompt is modal and reads it directly. `N` answers the
    /// same prompt and doubles as the New Continent binding, which is safe
    /// because the prompt is checked first.
    Y,
    Z,
    LeftControl,
    RightControl,
    /// Opens and closes the debug cheat menu (`crate::cheats`). Not an
    /// overworld binding, so [`poll`] ignores it.
    F10,
}

/// The top-row digit keys, 0..9, in order.
impl Key {
    /// The key a playtest script names, spelled as the variant (`Up`, `Kp7`,
    /// `Digit3`, `A`, `F10`, ...). Bare digits and lowercase letters also work.
    pub fn from_name(name: &str) -> Option<Key> {
        use Key::*;
        let letters = [
            A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
        ];
        if name.len() == 1 {
            let ch = name.chars().next()?;
            if ch.is_ascii_alphabetic() {
                return Some(letters[(ch.to_ascii_uppercase() as u8 - b'A') as usize]);
            }
            if let Some(digit) = ch.to_digit(10) {
                return Some(DIGITS[digit as usize]);
            }
        }
        if let Some(rest) = name.strip_prefix("Digit") {
            return rest
                .parse::<usize>()
                .ok()
                .and_then(|d| DIGITS.get(d).copied());
        }
        if let Some(rest) = name.strip_prefix("Kp") {
            if let Ok(d) = rest.parse::<usize>() {
                return NUMPAD_DIGITS.get(d).copied();
            }
        }
        Some(match name {
            "Up" => Up,
            "Down" => Down,
            "Left" => Left,
            "Right" => Right,
            "Home" => Home,
            "End" => End,
            "PageUp" => PageUp,
            "PageDown" => PageDown,
            "Escape" | "Esc" => Escape,
            "Enter" => Enter,
            "KpEnter" => KpEnter,
            "Backspace" => Backspace,
            "Space" => Space,
            "LeftControl" | "Ctrl" => LeftControl,
            "RightControl" => RightControl,
            "F10" => F10,
            _ => return None,
        })
    }

    /// The character typing this key produces, for text entry.
    pub fn typed_char(self) -> Option<char> {
        let name = format!("{self:?}");
        match self {
            Key::Space => Some(' '),
            _ if name.len() == 1 => name.chars().next().map(|c| c.to_ascii_lowercase()),
            _ => DIGITS
                .iter()
                .position(|d| *d == self)
                .and_then(|d| char::from_digit(d as u32, 10)),
        }
    }
}

pub const DIGITS: [Key; 10] = [
    Key::Digit0,
    Key::Digit1,
    Key::Digit2,
    Key::Digit3,
    Key::Digit4,
    Key::Digit5,
    Key::Digit6,
    Key::Digit7,
    Key::Digit8,
    Key::Digit9,
];

/// The numpad digit keys, 0..9, in order.
pub const NUMPAD_DIGITS: [Key; 10] = [
    Key::Kp0,
    Key::Kp1,
    Key::Kp2,
    Key::Kp3,
    Key::Kp4,
    Key::Kp5,
    Key::Kp6,
    Key::Kp7,
    Key::Kp8,
    Key::Kp9,
];

/// Where key state comes from. `pressed` is a rising edge -- true for the one
/// frame the key went down -- and `down` is the held state, which only the
/// control modifier needs.
pub trait KeySource {
    fn pressed(&self, key: Key) -> bool;
    fn down(&self, key: Key) -> bool;
    /// True when any non-modifier key went down this frame. The message
    /// dialog advances on "any key", porting C's `ui_any_key_pressed`, which
    /// skips the shift/control/alt/super modifiers and the lock keys.
    fn any_pressed(&self) -> bool;
    /// The printable characters typed this frame, draining them like C's
    /// `input_get_char_pressed`. Text entry (the new-game name) is the only
    /// consumer.
    fn chars_pressed(&self) -> Vec<char>;
}

/// What a keypress asks for, porting `InputAction`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Action {
    #[default]
    None,
    ViewArmy,
    ViewCharacter,
    ViewContract,
    ViewPuzzle,
    ViewMap,
    ViewControls,
    CastSpell,
    Search,
    EndWeek,
    Fly,
    Land,
    DismissArmy,
    NewContinent,
    OptionsMenu,
    SaveQuit,
    FastQuit,
    /// Numpad 5: rest one day in place.
    Rest,
    /// Bare ESC: open the in-game Game Menu. Not a C binding; see
    /// `specs/ingame-menu-design.md`.
    GameMenu,
}

/// One frame of input.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputState {
    /// -1, 0 or 1 per axis.
    pub dx: i32,
    pub dy: i32,
    pub action: Action,
}

impl InputState {
    /// True when this frame asked for a step and nothing else. C only moves
    /// the hero when no action fired, so a letter key never also walks.
    pub fn is_move(&self) -> bool {
        self.action == Action::None && (self.dx != 0 || self.dy != 0)
    }
}

/// Cardinal steps, checked before the diagonals overwrite them.
const VERTICAL: [(&[Key], i32); 2] = [(&[Key::Up, Key::Kp8], -1), (&[Key::Down, Key::Kp2], 1)];
const HORIZONTAL: [(&[Key], i32); 2] = [(&[Key::Left, Key::Kp4], -1), (&[Key::Right, Key::Kp6], 1)];

/// Diagonals, which set BOTH axes and so override whatever the cardinal pass
/// decided. The VGA keypad equivalents sit beside the numpad ones.
const DIAGONAL: [(&[Key], i32, i32); 4] = [
    (&[Key::Kp7, Key::Home], -1, -1),
    (&[Key::Kp9, Key::PageUp], 1, -1),
    (&[Key::Kp1, Key::End], -1, 1),
    (&[Key::Kp3, Key::PageDown], 1, 1),
];

/// Letter bindings, in C's exact order. The order is load-bearing: C walks an
/// else-if chain, so two keys pressed in the same frame resolve to whichever
/// appears first here.
const BINDINGS: [(Key, Action); 15] = [
    (Key::A, Action::ViewArmy),
    (Key::C, Action::ViewControls),
    (Key::F, Action::Fly),
    (Key::L, Action::Land),
    (Key::I, Action::ViewContract),
    (Key::M, Action::ViewMap),
    (Key::P, Action::ViewPuzzle),
    (Key::S, Action::Search),
    (Key::U, Action::CastSpell),
    (Key::V, Action::ViewCharacter),
    (Key::W, Action::EndWeek),
    (Key::D, Action::DismissArmy),
    (Key::O, Action::OptionsMenu),
    (Key::N, Action::NewContinent),
    (Key::Kp5, Action::Rest),
];

fn any_pressed(keys: &dyn KeySource, group: &[Key]) -> bool {
    group.iter().any(|key| keys.pressed(*key))
}

/// The first held direction on one axis, or 0.
fn held_axis(keys: &dyn KeySource, axis: &[(&[Key], i32); 2]) -> i32 {
    axis.iter()
        .find(|(group, _)| group.iter().any(|key| keys.down(*key)))
        .map_or(0, |(_, step)| *step)
}

/// Read one frame, porting `input_poll`.
pub fn poll(keys: &dyn KeySource) -> InputState {
    let mut state = InputState::default();

    for (group, dy) in VERTICAL {
        if any_pressed(keys, group) {
            state.dy = dy;
            break;
        }
    }
    for (group, dx) in HORIZONTAL {
        if any_pressed(keys, group) {
            state.dx = dx;
            break;
        }
    }
    // DIVERGENCE: C takes two arrows as a diagonal only when both go down in
    // the same frame, which a player almost never manages. Here an arrow
    // pressed while the other axis is HELD makes the diagonal too. It is still
    // one step per press; a held key alone never moves.
    if state.dy != 0 && state.dx == 0 {
        state.dx = held_axis(keys, &HORIZONTAL);
    } else if state.dx != 0 && state.dy == 0 {
        state.dy = held_axis(keys, &VERTICAL);
    }
    for (group, dx, dy) in DIAGONAL {
        if any_pressed(keys, group) {
            state.dx = dx;
            state.dy = dy;
            break;
        }
    }

    for (key, action) in BINDINGS {
        if keys.pressed(key) {
            state.action = action;
            return state;
        }
    }
    // Quit last, and the modifier first: Ctrl+Q leaves without saving, plain
    // Q saves and then asks.
    if keys.pressed(Key::Q) {
        let ctrl = keys.down(Key::LeftControl) || keys.down(Key::RightControl);
        state.action = if ctrl {
            Action::FastQuit
        } else {
            Action::SaveQuit
        };
    }
    // ESC last of all. It is NOT a C binding -- C's ESC only ever dismisses --
    // so it stays out of the parity-ordered BINDINGS table above, and any
    // letter pressed in the same frame wins. main.rs only reaches this when
    // no view, prompt, dialog or quit confirm is up.
    if state.action == Action::None && keys.pressed(Key::Escape) {
        state.action = Action::GameMenu;
    }
    state
}

/// A fixed set of keys, for tests and for replaying a recorded frame.
#[derive(Clone, Debug, Default)]
pub struct Keys {
    pressed: BTreeSet<Key>,
    down: BTreeSet<Key>,
    chars: Vec<char>,
}

impl Keys {
    pub fn pressed(keys: &[Key]) -> Self {
        Self {
            pressed: keys.iter().copied().collect(),
            down: BTreeSet::new(),
            chars: Vec::new(),
        }
    }

    pub fn with_held(mut self, keys: &[Key]) -> Self {
        self.down = keys.iter().copied().collect();
        self
    }

    /// The printable characters this frame typed, as text entry would see them.
    pub fn with_chars(mut self, chars: &[char]) -> Self {
        self.chars = chars.to_vec();
        self
    }
}

impl KeySource for Keys {
    fn pressed(&self, key: Key) -> bool {
        self.pressed.contains(&key)
    }

    fn down(&self, key: Key) -> bool {
        self.down.contains(&key)
    }

    fn any_pressed(&self) -> bool {
        // The control modifiers are the only modifier keys in this
        // vocabulary, so they are all `any_pressed` skips.
        self.pressed
            .iter()
            .any(|k| !matches!(k, Key::LeftControl | Key::RightControl))
    }

    fn chars_pressed(&self) -> Vec<char> {
        self.chars.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poll_keys(keys: &[Key]) -> InputState {
        poll(&Keys::pressed(keys))
    }

    #[test]
    fn the_arrows_and_the_numpad_walk_the_same_way() {
        assert_eq!(
            (poll_keys(&[Key::Up]).dx, poll_keys(&[Key::Up]).dy),
            (0, -1)
        );
        assert_eq!(poll_keys(&[Key::Kp8]).dy, -1);
        assert_eq!(poll_keys(&[Key::Down]).dy, 1);
        assert_eq!(poll_keys(&[Key::Left]).dx, -1);
        assert_eq!(poll_keys(&[Key::Right]).dx, 1);
        assert_eq!(poll_keys(&[Key::Kp6]).dx, 1);
    }

    #[test]
    fn two_cardinals_at_once_make_a_diagonal() {
        let state = poll_keys(&[Key::Up, Key::Right]);
        assert_eq!((state.dx, state.dy), (1, -1));
    }

    #[test]
    fn an_arrow_pressed_while_another_is_held_makes_a_diagonal() {
        let held = Keys::pressed(&[Key::Right]).with_held(&[Key::Up]);
        let state = poll(&held);
        assert_eq!((state.dx, state.dy), (1, -1));
        let held = Keys::pressed(&[Key::Down]).with_held(&[Key::Left]);
        let state = poll(&held);
        assert_eq!((state.dx, state.dy), (-1, 1));
        // A held key on its own still never steps.
        let state = poll(&Keys::pressed(&[]).with_held(&[Key::Up, Key::Right]));
        assert_eq!((state.dx, state.dy), (0, 0));
    }

    #[test]
    fn a_diagonal_key_overrides_whatever_the_cardinals_said() {
        // C runs the cardinal pass first and then lets a diagonal key
        // overwrite BOTH axes, so a diagonal always wins outright.
        let state = poll_keys(&[Key::Up, Key::Kp1]);
        assert_eq!((state.dx, state.dy), (-1, 1), "the numpad 1 wins");

        // The VGA keypad equivalents bind to the same four diagonals.
        assert_eq!(
            (poll_keys(&[Key::Home]).dx, poll_keys(&[Key::Home]).dy),
            (-1, -1)
        );
        let page_up = poll_keys(&[Key::PageUp]);
        assert_eq!((page_up.dx, page_up.dy), (1, -1));
        let end = poll_keys(&[Key::End]);
        assert_eq!((end.dx, end.dy), (-1, 1));
        let page_down = poll_keys(&[Key::PageDown]);
        assert_eq!((page_down.dx, page_down.dy), (1, 1));
    }

    #[test]
    fn opposing_keys_resolve_to_the_first_one_checked() {
        // C's cardinal pass is an else-if, so up beats down and left beats
        // right rather than cancelling out.
        assert_eq!(poll_keys(&[Key::Up, Key::Down]).dy, -1);
        assert_eq!(poll_keys(&[Key::Left, Key::Right]).dx, -1);
    }

    #[test]
    fn every_letter_binds_to_its_action() {
        for (key, expected) in BINDINGS {
            assert_eq!(poll_keys(&[key]).action, expected, "key {key:?}");
        }
    }

    #[test]
    fn the_binding_order_decides_when_two_keys_land_together() {
        // A comes before W in C's chain, so a frame with both is a view, not
        // a week. This is the sort of thing that only matters at 60fps, and
        // it is exactly what C does.
        assert_eq!(poll_keys(&[Key::W, Key::A]).action, Action::ViewArmy);
        assert_eq!(poll_keys(&[Key::N, Key::F]).action, Action::Fly);
    }

    #[test]
    fn control_turns_quit_into_a_quit_without_saving() {
        assert_eq!(poll_keys(&[Key::Q]).action, Action::SaveQuit);
        let held = Keys::pressed(&[Key::Q]).with_held(&[Key::LeftControl]);
        assert_eq!(poll(&held).action, Action::FastQuit);
        let held = Keys::pressed(&[Key::Q]).with_held(&[Key::RightControl]);
        assert_eq!(poll(&held).action, Action::FastQuit);
    }

    #[test]
    fn quit_is_checked_after_every_letter() {
        // Q sits at the end of C's chain, so any other bound letter in the
        // same frame wins and the game is not saved.
        assert_eq!(poll_keys(&[Key::Q, Key::S]).action, Action::Search);
    }

    #[test]
    fn an_action_suppresses_the_step_it_was_pressed_with() {
        // C only steps when the action is NONE, so a letter pressed while an
        // arrow is held must not also walk.
        let state = poll_keys(&[Key::Up, Key::S]);
        assert_eq!(state.action, Action::Search);
        assert_eq!((state.dx, state.dy), (0, -1), "the direction is still read");
        assert!(!state.is_move(), "but it is not a step");

        assert!(poll_keys(&[Key::Up]).is_move());
        assert!(!poll_keys(&[]).is_move());
    }

    #[test]
    fn an_unbound_key_does_nothing() {
        let state = poll_keys(&[Key::G]);
        assert_eq!(state, InputState::default());
    }

    #[test]
    fn escape_asks_for_the_game_menu() {
        assert_eq!(poll_keys(&[Key::Escape]).action, Action::GameMenu);
    }

    /// ESC is not a C binding, so it must not disturb the parity-ordered
    /// letter table: a letter pressed in the same frame still wins, and so
    /// does Q, which C checks last of its own keys.
    #[test]
    fn a_letter_still_beats_escape_in_the_same_frame() {
        assert_eq!(poll_keys(&[Key::Escape, Key::A]).action, Action::ViewArmy);
        assert_eq!(poll_keys(&[Key::Escape, Key::Q]).action, Action::SaveQuit);
    }

    /// Opening the menu is an action, so the arrow held with it is not a step.
    #[test]
    fn escape_suppresses_a_step_pressed_with_it() {
        assert!(!poll_keys(&[Key::Escape, Key::Up]).is_move());
    }

    #[test]
    fn f10_is_not_an_overworld_binding() {
        // The cheat menu reads F10 before the overworld chain runs; `poll`
        // must not turn it into an action or a step.
        assert_eq!(poll_keys(&[Key::F10]), InputState::default());
    }

    #[test]
    fn any_pressed_skips_the_control_modifiers() {
        assert!(Keys::pressed(&[Key::A]).any_pressed());
        assert!(Keys::pressed(&[Key::Enter]).any_pressed());
        // A modifier alone is not "any key", the way C's ui_any_key_pressed
        // skips them.
        assert!(!Keys::pressed(&[Key::LeftControl]).any_pressed());
        assert!(!Keys::pressed(&[Key::RightControl]).any_pressed());
        assert!(!Keys::default().any_pressed());
    }
}
