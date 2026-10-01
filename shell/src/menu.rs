//! The in-game Game Menu.
//!
//! NOT a port. C's `VIEW_MENU` exists but is unreachable -- nothing calls
//! `views_set(VIEW_MENU)` -- so there is no C behaviour to match. See
//! `specs/ingame-menu-design.md` for what was taken from C's shape and what
//! was deliberately dropped: the Views submenu duplicates existing single
//! keys, and C's "New Game" keeps gold, army and days while only teleporting
//! the hero home, which its label does not admit.

use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{DrawList, Rect};
use crate::font::{GLYPH_H, GLYPH_W};
use crate::input::{Key, KeySource, DIGITS};
use crate::layout::Layout;
use crate::palette::{Pal, Palette};

pub const ENTRY_COUNT: usize = 3;

/// Text the shell compiles in; the pack may override it under `strings.menu`.
const DEFAULT_TITLE: &str = "Game Menu";
const DEFAULT_ENTRIES: [(&str, &str); ENTRY_COUNT] = [
    ("save", "Save game"),
    ("load", "Load game"),
    ("quit", "Quit"),
];
const DEFAULT_HINT: &str = "ENTER pick  ESC close";

/// What one frame of menu input asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    Save,
    Load,
    Quit,
    /// ESC: close without acting.
    Close,
}

/// The entries, in drawn order.
const ACTIONS: [MenuAction; ENTRY_COUNT] = [MenuAction::Save, MenuAction::Load, MenuAction::Quit];

/// Which save-slot job the menu's picker is doing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickerPurpose {
    Save,
    Load,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Menu {
    pub cursor: usize,
}

impl Menu {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume one frame: Up/Down move with wrap, a digit picks and confirms
    /// its row, Enter/Space confirm, ESC closes.
    pub fn update(&mut self, keys: &dyn KeySource) -> Option<MenuAction> {
        if keys.pressed(Key::Escape) {
            return Some(MenuAction::Close);
        }
        for (index, action) in ACTIONS.iter().enumerate() {
            if keys.pressed(DIGITS[1 + index]) {
                self.cursor = index;
                return Some(*action);
            }
        }
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            self.cursor = (self.cursor + ENTRY_COUNT - 1) % ENTRY_COUNT;
        }
        if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            self.cursor = (self.cursor + 1) % ENTRY_COUNT;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) || keys.pressed(Key::Space) {
            return Some(ACTIONS[self.cursor]);
        }
        None
    }

    /// A rounded panel with a title, the numbered entries (the cursor row in
    /// yellow) and its own key hint, centred on the screen.
    pub fn draw(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        let pad = 6;
        let row_h = GLYPH_H + 2;
        let title = banner::text(resources, "menu", "title", DEFAULT_TITLE);
        let hint = banner::text(resources, "menu", "hint", DEFAULT_HINT);
        let labels: Vec<String> = DEFAULT_ENTRIES
            .iter()
            .enumerate()
            .map(|(index, (key, default))| {
                format!(
                    "{}. {}",
                    index + 1,
                    banner::text(resources, "menu", key, default)
                )
            })
            .collect();

        let widest = labels
            .iter()
            .map(|label| label.chars().count())
            .chain([title.chars().count(), hint.chars().count()])
            .max()
            .unwrap_or(0) as i32;
        let w = widest * GLYPH_W + 2 * pad;
        let h = pad + (GLYPH_H + 4) + ENTRY_COUNT as i32 * row_h + 4 + GLYPH_H + pad;
        let x = (layout.screen_w - w) / 2;
        let y = (layout.screen_h - h) / 2;
        list.rounded_panel(
            Rect::new(x, y, w, h),
            palette.clr(Pal::DBlue),
            palette.clr(Pal::Yellow),
        );

        list.text(title, x + pad, y + pad, palette.clr(Pal::Yellow));
        let mut ty = y + pad + GLYPH_H + 4;
        for (index, label) in labels.iter().enumerate() {
            let fg = if index == self.cursor {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            list.text(label, x + pad, ty, fg);
            ty += row_h;
        }
        list.text(hint, x + pad, y + h - pad - GLYPH_H, palette.clr(Pal::Grey));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
    use crate::input::{Key, Keys};

    #[test]
    fn the_cursor_wraps_in_both_directions() {
        let mut menu = Menu::new();
        assert_eq!(menu.cursor, 0);
        menu.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(menu.cursor, ENTRY_COUNT - 1);
        menu.update(&Keys::pressed(&[Key::Down]));
        assert_eq!(menu.cursor, 0);
    }

    #[test]
    fn each_entry_confirms_to_its_own_action() {
        for (index, expected) in [
            (0, MenuAction::Save),
            (1, MenuAction::Load),
            (2, MenuAction::Quit),
        ] {
            let mut menu = Menu::new();
            menu.cursor = index;
            assert_eq!(menu.update(&Keys::pressed(&[Key::Enter])), Some(expected));
        }
    }

    #[test]
    fn escape_closes_without_acting() {
        let mut menu = Menu::new();
        menu.cursor = 2;
        assert_eq!(
            menu.update(&Keys::pressed(&[Key::Escape])),
            Some(MenuAction::Close)
        );
    }

    /// The digit shortcuts match the drawn numbering, 1-based.
    #[test]
    fn digits_select_and_confirm_their_row() {
        let mut menu = Menu::new();
        assert_eq!(
            menu.update(&Keys::pressed(&[Key::Digit2])),
            Some(MenuAction::Load)
        );
        assert_eq!(menu.cursor, 1, "the digit also moves the cursor");
    }

    #[test]
    fn nothing_pressed_reports_nothing() {
        let mut menu = Menu::new();
        assert_eq!(menu.update(&Keys::pressed(&[])), None);
    }

    #[test]
    fn the_menu_draws_its_title_entries_and_hint_with_the_cursor_in_yellow() {
        let resources = Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let palette = Palette::fallback();
        let mut menu = Menu::new();
        menu.cursor = 1;
        let mut list = DrawList::new();
        menu.draw(&mut list, &layout, &palette, &resources);

        let texts = list.texts();
        for expected in [
            "Game Menu",
            "1. Save game",
            "2. Load game",
            "3. Quit",
            "ESC close",
        ] {
            assert!(
                texts.iter().any(|(_, _, text)| text.contains(expected)),
                "missing {expected:?}"
            );
        }
        let colour_of = |label: &str| {
            list.as_slice().iter().find_map(|cmd| match cmd {
                DrawCmd::Text { text, color, .. } if text == label => Some(*color),
                _ => None,
            })
        };
        assert_eq!(colour_of("2. Load game"), Some(palette.clr(Pal::Yellow)));
        assert_eq!(colour_of("1. Save game"), Some(palette.clr(Pal::White)));
    }
}
