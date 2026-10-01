//! The in-game Game Menu.
//!
//! NOT a port. C's `VIEW_MENU` exists but is unreachable -- nothing calls
//! `views_set(VIEW_MENU)` -- so there is no C behaviour to match. See
//! `specs/ingame-menu-design.md` for what was taken from C's shape and what
//! was deliberately dropped: the Views submenu duplicates existing single
//! keys, and C's "New Game" keeps gold, army and days while only teleporting
//! the hero home, which its label does not admit.

use openbounty_engine::resources::Resources;
use openbounty_engine::savegame::SaveError;

use crate::banner;
use crate::draw::{DrawList, Rect};
use crate::font::{GLYPH_H, GLYPH_W};
use crate::input::{Key, KeySource, DIGITS};
use crate::layout::Layout;
use crate::palette::{Pal, Palette};
use crate::slot_picker::LoadError;

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

// The menu's messages. Each returns `(header, body)` for a dialog or prompt,
// as `crate::interact` does, with the slot shown 1-based like the picker.

fn slot_label(slot: usize) -> String {
    (slot + 1).to_string()
}

/// The picker title for each job.
pub fn picker_title(resources: &Resources, purpose: PickerPurpose) -> &str {
    match purpose {
        PickerPurpose::Save => {
            banner::text(resources, "menu", "save_title", " Save to which slot?")
        }
        PickerPurpose::Load => banner::text(resources, "menu", "load_title", " Load which game?"),
    }
}

/// The picker's key hint in-game, where ESC steps back to the menu.
pub fn picker_hint(resources: &Resources) -> &str {
    banner::text(
        resources,
        "menu",
        "picker_hint",
        "UP/DN move  ENTER pick  ESC back",
    )
}

/// Confirm overwriting an occupied slot; `name` is the hero saved there.
pub fn overwrite_prompt(resources: &Resources, slot: usize, name: &str) -> (String, String) {
    let header = banner::text(resources, "menu", "overwrite_title", "Overwrite Save");
    let body = banner::formatted(
        resources,
        "menu",
        "overwrite_body",
        "Save slot %SLOT% holds\n%NAME%'s game.\nOverwrite it?",
        &[("SLOT", &slot_label(slot)), ("NAME", name)],
    );
    (header.to_owned(), body)
}

pub fn saved_message(resources: &Resources, slot: usize) -> (String, String) {
    let header = banner::text(resources, "menu", "saved_title", "Game Saved");
    let body = banner::formatted(
        resources,
        "menu",
        "saved_body",
        "Your game was saved\nto slot %SLOT%.",
        &[("SLOT", &slot_label(slot))],
    );
    (header.to_owned(), body)
}

/// `no_dir` is true when there was no usable save directory at all.
pub fn save_failed_message(resources: &Resources, no_dir: bool) -> (String, String) {
    let header = banner::text(resources, "menu", "save_failed_title", "Save Failed");
    let body = if no_dir {
        banner::text(
            resources,
            "menu",
            "no_save_dir",
            "There is no usable save folder\non this computer.",
        )
    } else {
        banner::text(
            resources,
            "menu",
            "save_io_error",
            "The save could not be written.",
        )
    };
    (header.to_owned(), body.to_owned())
}

/// Choosing an empty slot to load is a no-op with a message, not an error.
pub fn empty_slot_message(resources: &Resources) -> (String, String) {
    let header = banner::text(resources, "menu", "load_title_short", "Load Game");
    let body = banner::text(resources, "menu", "empty_slot", "That slot is empty.");
    (header.to_owned(), body.to_owned())
}

pub fn load_failed_message(resources: &Resources, err: &LoadError) -> (String, String) {
    let header = banner::text(resources, "menu", "load_failed_title", "Load Failed");
    let body = match err {
        LoadError::NoPath => banner::text(
            resources,
            "menu",
            "no_save_dir",
            "There is no usable save folder\non this computer.",
        )
        .to_owned(),
        LoadError::Read(SaveError::Io) => banner::text(
            resources,
            "menu",
            "load_io_error",
            "The save could not be read.",
        )
        .to_owned(),
        LoadError::Read(SaveError::Parse | SaveError::Mismatch) => {
            banner::text(resources, "menu", "load_damaged", "The save is damaged.").to_owned()
        }
        LoadError::Read(SaveError::Version) => banner::text(
            resources,
            "menu",
            "load_wrong_version",
            "The save was written by a\ndifferent version.",
        )
        .to_owned(),
        LoadError::Read(SaveError::Pack) => banner::text(
            resources,
            "menu",
            "load_wrong_pack",
            "The save belongs to a\ndifferent game pack.",
        )
        .to_owned(),
        LoadError::Zone(detail) => banner::formatted(
            resources,
            "menu",
            "load_bad_zone",
            "The save names a map this\npack cannot load:\n%ZONE%",
            &[("ZONE", detail)],
        ),
    };
    (header.to_owned(), body)
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

    fn resources() -> Resources {
        Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses")
    }

    /// Slots are 0-based inside the shell and 1-based on screen, matching the
    /// picker's own numbering.
    #[test]
    fn slot_messages_number_slots_from_one() {
        let resources = resources();
        let (_, body) = overwrite_prompt(&resources, 0, "Murray");
        assert!(body.contains("slot 1") && body.contains("Murray"), "{body}");
        let (_, body) = saved_message(&resources, 9);
        assert!(body.contains("slot 10"), "{body}");
    }

    /// Every load failure gets a sentence, not a Debug dump; the foreign-pack
    /// refusal in particular must say why, since the picker lists that save.
    #[test]
    fn each_load_failure_reads_as_a_sentence() {
        let resources = resources();
        let cases = [
            (LoadError::NoPath, "save folder"),
            (LoadError::Read(SaveError::Io), "could not be read"),
            (LoadError::Read(SaveError::Parse), "damaged"),
            (LoadError::Read(SaveError::Mismatch), "damaged"),
            (LoadError::Read(SaveError::Version), "different version"),
            (LoadError::Read(SaveError::Pack), "different game pack"),
            (LoadError::Zone("atlantis: missing".to_owned()), "atlantis"),
        ];
        for (err, needle) in cases {
            let (header, body) = load_failed_message(&resources, &err);
            assert_eq!(header, "Load Failed");
            assert!(body.contains(needle), "{err:?}: {body}");
            assert!(!body.contains("Read("), "no Debug text: {body}");
        }
    }

    #[test]
    fn a_save_failure_says_whether_there_was_anywhere_to_write() {
        let resources = resources();
        let (_, no_dir) = save_failed_message(&resources, true);
        let (_, io) = save_failed_message(&resources, false);
        assert!(no_dir.contains("save folder"), "{no_dir}");
        assert!(io.contains("could not be written"), "{io}");
    }
}
