//! The save slot picker, shared by the startup wizard and the in-game menu.
//!
//! Extracted from `startup.rs` so both callers show the same widget. It is
//! deliberately dumb: it owns the cursor and the scanned headers and reports
//! which row was confirmed. What a row MEANS is the caller's business --
//! startup treats an empty slot as "start a new game here", the menu treats it
//! as an overwrite target or as nothing to load.
//!
//! [`load_slot`] is the matching shared load recipe, for the same reason: two
//! callers, one function, or they drift.

use openbounty_engine::fog::Fog;
use openbounty_engine::game::Game;
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::savegame::{save_read, SaveError};

use crate::banner;
use crate::draw::{DrawList, Rect};
use crate::font::GLYPH_H;
use crate::input::{Key, KeySource};
use crate::layout::Layout;
use crate::palette::{Pal, Palette};
use crate::savepath::{SavePaths, SlotSet, SAVE_SLOT_COUNT};

/// Text C compiles in for the picker; the pack overrides it under
/// `strings.startup`.
const DEFAULT_SAVE_PICKER_EMPTY: &str = "(empty)";

/// Which row the player confirmed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotChoice {
    /// A slot row was confirmed. It may be empty; the caller decides.
    Slot(usize),
    /// The trailing extra row was confirmed (startup's "New game").
    Extra,
    /// ESC.
    Cancel,
}

/// The picker's state: the highlighted row and the headers it lists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotPicker {
    /// The highlighted row, 0..row_count. With an extra row, the extra row is
    /// `SAVE_SLOT_COUNT`.
    pub cursor: usize,
    slots: SlotSet,
    extra_row: bool,
}

impl SlotPicker {
    pub fn new(slots: SlotSet, extra_row: bool) -> Self {
        Self {
            cursor: 0,
            slots,
            extra_row,
        }
    }

    pub fn slots(&self) -> &SlotSet {
        &self.slots
    }

    fn row_count(&self) -> usize {
        SAVE_SLOT_COUNT + usize::from(self.extra_row)
    }

    /// Consume one frame: Up/Down move with wrap, Enter/Space confirm, ESC
    /// cancels. The navigation half of startup's former `update_save_picker`.
    pub fn update(&mut self, keys: &dyn KeySource) -> Option<SlotChoice> {
        let row_count = self.row_count();
        if keys.pressed(Key::Escape) {
            return Some(SlotChoice::Cancel);
        }
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            self.cursor = (self.cursor + row_count - 1) % row_count;
        }
        if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            self.cursor = (self.cursor + 1) % row_count;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) || keys.pressed(Key::Space) {
            if self.cursor == SAVE_SLOT_COUNT {
                return Some(SlotChoice::Extra);
            }
            return Some(SlotChoice::Slot(self.cursor));
        }
        None
    }

    /// Emit the picker panel. `extra_label` is drawn as the trailing row only
    /// when the picker was built with one. `hint` is the key line at the
    /// bottom, which differs because ESC quits at startup but steps back
    /// in-game.
    pub fn draw(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        title: &str,
        extra_label: &str,
        hint: &str,
    ) {
        let row_count = self.row_count();
        let pad = 6;
        let row_h = GLYPH_H + 2;
        let header_h = GLYPH_H + 4;
        let gap_h = 4;
        let instr_h = GLYPH_H;
        let body_h = row_count as i32 * row_h;
        let w = 280;
        let h = pad + header_h + body_h + gap_h + instr_h + pad;
        let x = (layout.screen_w - w) / 2;
        let y = (layout.screen_h - h) / 2;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        list.text(title, x + pad, y + pad, palette.clr(Pal::Yellow));

        let empty = banner::text(
            resources,
            "startup",
            "save_picker_empty",
            DEFAULT_SAVE_PICKER_EMPTY,
        );
        let mut ty = y + pad + header_h;
        for slot in 0..SAVE_SLOT_COUNT {
            let fg = if slot == self.cursor {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            let line = match &self.slots.headers[slot] {
                Some(header) => format!(
                    "{:>2}. {:<10}  {:<9}  {:>3}d",
                    slot + 1,
                    header.name.as_str(),
                    header.rank_title.as_str(),
                    header.days_left
                ),
                None => format!("{:>2}. {}", slot + 1, empty),
            };
            list.text(&line, x + pad, ty, fg);
            ty += row_h;
        }

        if self.extra_row {
            let extra_fg = if self.cursor == SAVE_SLOT_COUNT {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            list.text(&format!("    {extra_label}"), x + pad, ty, extra_fg);
        }

        list.text(hint, x + pad, y + h - pad - instr_h, palette.clr(Pal::Grey));
    }
}

/// Why a load could not complete. Every variant leaves the caller's live
/// session untouched, because nothing is returned to install.
#[derive(Debug)]
pub enum LoadError {
    /// No usable save directory for this slot.
    NoPath,
    Read(SaveError),
    /// The save named a zone the pack could not load.
    Zone(String),
}

/// Load a slot into fresh state: `save_read` against a placeholder map, then
/// the zone the save names, with its consumed tiles re-applied.
///
/// Returns everything or nothing. The in-game menu relies on that: a failed
/// load must not disturb a live session, so there is deliberately no variant
/// that hands back a partially built game. `save_read` resets the game it is
/// given before reading, so starting from `Game::default()` loads exactly
/// what a pre-seeded game would.
pub fn load_slot(
    paths: &SavePaths,
    resources: &Resources,
    pack_id: &str,
    slot: usize,
) -> Result<(Game, Map, Fog), LoadError> {
    let path = paths.slot(pack_id, slot).ok_or(LoadError::NoPath)?;
    let mut game = Game::default();
    let mut fog = Fog::new();
    save_read(&path, &mut game, resources, &Map::new(1, 1), &mut fog).map_err(LoadError::Read)?;

    let zone = game.position.zone.as_str().to_owned();
    let mut map = Map::load_zone_with_placements(resources, &zone, &game)
        .map_err(|err| LoadError::Zone(format!("{zone}: {err}")))?;
    map.apply_consumed(&game, &zone);
    Ok((game, map, fog))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Key, Keys};
    use openbounty_engine::savegame::SaveHeader;
    use openbounty_engine::state_serialize::SAVE_VERSION;
    use openbounty_engine::tables::FixedStr;

    fn slots_with(filled: &[usize]) -> SlotSet {
        let mut set = SlotSet::default();
        for &slot in filled {
            set.headers[slot] = Some(SaveHeader {
                exists: true,
                name: FixedStr::from_str("Sir Rust"),
                rank_title: FixedStr::from_str("General"),
                days_left: 400,
                ..SaveHeader::default()
            });
            set.existing += 1;
        }
        set
    }

    fn resources() -> Resources {
        Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses")
    }

    /// A flat save dir unique to one test, so parallel tests never collide.
    fn paths(tag: &str) -> SavePaths {
        let dir = std::env::temp_dir().join(format!(
            "openbounty-slot-picker-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        SavePaths::with_override(dir)
    }

    /// The transactional guarantee: a load that fails returns Err and produces
    /// no half-built state for a caller to accidentally install.
    #[test]
    fn loading_a_missing_slot_fails_without_producing_state() {
        let paths = paths("missing");
        assert!(matches!(
            load_slot(&paths, &resources(), "kb", 0),
            Err(LoadError::Read(SaveError::Io))
        ));
    }

    /// A slot number past the picker's range has no path at all.
    #[test]
    fn a_slot_out_of_range_has_no_path() {
        let paths = paths("out-of-range");
        assert!(matches!(
            load_slot(&paths, &resources(), "kb", SAVE_SLOT_COUNT),
            Err(LoadError::NoPath)
        ));
    }

    /// A corrupt save is a Read error, not a panic.
    #[test]
    fn loading_a_corrupt_slot_reports_a_read_error() {
        let paths = paths("corrupt");
        let path = paths.slot("kb", 0).expect("slot path");
        std::fs::write(&path, b"{ not valid json").expect("write");
        assert!(matches!(
            load_slot(&paths, &resources(), "kb", 0),
            Err(LoadError::Read(SaveError::Parse))
        ));
    }

    /// The late failure: `save_read` succeeds, then the zone it names will not
    /// load. The half-read game must still not come back to the caller.
    #[test]
    fn a_save_naming_an_unknown_zone_reports_a_zone_error() {
        let paths = paths("zone");
        let path = paths.slot("kb", 0).expect("slot path");
        let body = format!(
            r#"{{"version": {SAVE_VERSION},
                 "character": {{"name": "Murray"}},
                 "position": {{"zone": "atlantis"}}}}"#
        );
        std::fs::write(&path, body).expect("write");
        match load_slot(&paths, &resources(), "kb", 0) {
            Err(LoadError::Zone(message)) => assert!(message.contains("atlantis")),
            other => panic!("expected a zone error, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn the_cursor_wraps_over_the_slots_alone_without_an_extra_row() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(picker.cursor, 0);
        picker.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(picker.cursor, SAVE_SLOT_COUNT - 1, "wraps to the last slot");
        picker.update(&Keys::pressed(&[Key::Down]));
        assert_eq!(picker.cursor, 0);
    }

    /// Startup adds a trailing "New game" row, so its cursor runs one further.
    #[test]
    fn an_extra_row_adds_one_position_past_the_slots() {
        let mut picker = SlotPicker::new(slots_with(&[]), true);
        picker.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(picker.cursor, SAVE_SLOT_COUNT, "the extra row is last");
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Enter])),
            Some(SlotChoice::Extra)
        );
    }

    #[test]
    fn confirming_a_slot_reports_that_slot_whether_or_not_it_is_filled() {
        let mut picker = SlotPicker::new(slots_with(&[3]), false);
        picker.cursor = 3;
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Enter])),
            Some(SlotChoice::Slot(3))
        );
        // An empty slot reports too; the CALLER decides what that means.
        let mut picker = SlotPicker::new(slots_with(&[3]), false);
        picker.cursor = 5;
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Space])),
            Some(SlotChoice::Slot(5))
        );
    }

    #[test]
    fn escape_cancels() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Escape])),
            Some(SlotChoice::Cancel)
        );
    }

    #[test]
    fn nothing_pressed_reports_nothing() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(picker.update(&Keys::pressed(&[])), None);
    }

    /// The one drawn difference between the two callers: the extra row's label
    /// appears only when the picker has one.
    #[test]
    fn the_extra_label_is_drawn_only_with_an_extra_row() {
        let resources = resources();
        let layout = Layout::from_resources(&resources).expect("lays out");
        let palette = Palette::fallback();
        for (extra_row, expected) in [(true, true), (false, false)] {
            let picker = SlotPicker::new(slots_with(&[1]), extra_row);
            let mut list = DrawList::new();
            picker.draw(
                &mut list, &layout, &palette, &resources, "Title", "New game", "Hint",
            );
            let texts = list.texts();
            assert_eq!(
                texts.iter().any(|(_, _, text)| text.contains("New game")),
                expected,
                "extra_row = {extra_row}"
            );
            assert!(texts.iter().any(|(_, _, text)| *text == "Title"));
            assert!(texts.iter().any(|(_, _, text)| *text == "Hint"));
            assert!(texts
                .iter()
                .any(|(_, _, text)| text.contains("Sir Rust") && text.contains("400d")));
        }
    }
}
