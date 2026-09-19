//! The pre-game startup flow, porting `src/startup.c`.
//!
//! These screens run before the overworld exists: class select, the
//! name/difficulty wizard, and the new-game intro. Each frame
//! [`Startup::update`] consumes one frame of input and, when the player
//! commits, yields a [`StartupChoice`]; [`Startup::draw`] emits the matching
//! [`DrawList`]. Nothing here touches a graphics library, so the wizard is
//! testable headlessly the way the views are.
//!
//! NOT ported yet (later startup slices): the two splash screens, the credits
//! screen, the save picker, and the LOAD path. `L` on class select is inert
//! until the picker lands.

use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font::{GLYPH_H, GLYPH_W};
use crate::input::{Key, KeySource};
use crate::layout::Layout;
use crate::palette::{Pal, Palette};
use crate::savepath::{scan_slots, SavePaths, SlotSet, SAVE_SLOT_COUNT};

/// Text C compiles in for the startup screens; the pack overrides it under
/// `strings.startup` and `strings.difficulty`.
const DEFAULT_CLASS_SELECT_HINT: &str = "Select Char A-D or L-Load saved game";
const DEFAULT_SAVE_PICKER_TITLE: &str = " Select game:";
const DEFAULT_SAVE_PICKER_EMPTY: &str = "(empty)";
const DEFAULT_SAVE_PICKER_NEW_GAME: &str = "New game";
const DEFAULT_CONTROLS_HINT: &str = "UP/DN move  ENTER pick  ESC quit";
const DEFAULT_NEW_GAME_TABLE_HEADER: &str = "   Difficulty   Days  Score";
const DEFAULT_NEW_GAME_SELECT_HINT: &str = "^v to select   Ent to Accept";
const DEFAULT_DIFFICULTY_LABELS: [&str; 4] = ["Easy", "Normal", "Hard", "Impossible?"];
const DEFAULT_DIFFICULTY_SCORES: [&str; 4] = ["x.5", " x1", " x2", " x4"];
const DEFAULT_NEW_GAME_INTRO: &str = "%NAME% the %CLASS%,\n\
    A new game is being created.\n\
    Please wait while I perform\n\
    godlike actions to make this\n\
    game playable.";

/// The class-picker bitmap's placement size. The shell cannot read a texture's
/// intrinsic size, so this uses the shipped art (and C's missing-asset
/// fallback), which are both 288x184.
const CLASS_PICKER_W: i32 = 288;
const CLASS_PICKER_H: i32 = 184;

/// Longest accepted hero name, C's `char name_buf[11]`.
const NAME_MAX: usize = 10;
/// How long the new-game intro holds before auto-advancing, C's `timeout`.
const INTRO_TIMEOUT: f64 = 4.0;
/// Corner padding of the intro panel, C's `pad`.
const INTRO_PAD: i32 = 8;

/// The `Difficulty` enum order, also the pack's `strings.difficulty` key order.
const DIFFICULTY_KEYS: [&str; 4] = ["easy", "normal", "hard", "impossible"];

/// What the wizard decided.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartupAction {
    /// Start a new game with the chosen class, name and difficulty.
    New,
    /// Load an existing save from the chosen slot.
    Load,
    /// The player pressed ESC at the root (class select).
    Quit,
}

/// The result of [`Startup::update`]: what to do once the wizard finishes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupChoice {
    pub action: StartupAction,
    /// Which save slot the choice applies to.
    pub slot: usize,
    pub class_id: String,
    pub name: String,
    /// 0..3, in `Difficulty` order.
    pub difficulty: i32,
}

/// Which wizard screen is up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Screen {
    ClassSelect,
    SavePicker,
    CreateGame,
    Intro,
}

/// The pre-game wizard's state. A default one starts on class select.
#[derive(Clone, Debug, PartialEq)]
pub struct Startup {
    screen: Screen,
    class_id: String,
    name: String,
    /// False while typing the name; true once Enter confirms and the
    /// difficulty selector takes over.
    has_name: bool,
    difficulty: i32,
    intro_started: f64,
    /// Chosen save slot: 0 for a plain new game, the picked slot otherwise.
    slot: usize,
    /// The picker's scanned slots.
    slots: SlotSet,
    /// The picker's highlighted row, 0..=SAVE_SLOT_COUNT.
    cursor: usize,
}

impl Default for Startup {
    fn default() -> Self {
        Self {
            screen: Screen::ClassSelect,
            class_id: String::new(),
            name: String::new(),
            has_name: false,
            // C's initial selection is Normal.
            difficulty: 1,
            intro_started: 0.0,
            slot: 0,
            slots: SlotSet::default(),
            cursor: SAVE_SLOT_COUNT,
        }
    }
}

impl Startup {
    pub fn new() -> Self {
        Self::default()
    }

    /// Consume one frame. Returns a choice only when the player commits or
    /// quits; call again next frame otherwise.
    pub fn update(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        paths: &SavePaths,
        time: f64,
    ) -> Option<StartupChoice> {
        match self.screen {
            Screen::ClassSelect => self.update_class_select(keys, resources, paths),
            Screen::SavePicker => self.update_save_picker(keys),
            Screen::CreateGame => self.update_create_game(keys, resources, time),
            Screen::Intro => self.update_intro(keys, time),
        }
    }

    fn update_class_select(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        paths: &SavePaths,
    ) -> Option<StartupChoice> {
        if keys.pressed(Key::Escape) {
            return Some(self.choice(StartupAction::Quit));
        }
        // L opens the save picker; the cursor lands on the first existing slot
        // (or "New game" when there are none).
        if keys.pressed(Key::L) {
            self.slots = scan_slots(paths, resources.pack_id.as_str());
            self.cursor = SAVE_SLOT_COUNT;
            for (slot, header) in self.slots.headers.iter().enumerate() {
                if header.is_some() {
                    self.cursor = slot;
                    break;
                }
            }
            self.screen = Screen::SavePicker;
            let _ = keys.chars_pressed();
            return None;
        }
        // A-D pick a class directly.
        let count = resources.classes_count().min(4);
        for index in 0..count {
            let key = [Key::A, Key::B, Key::C, Key::D][index];
            if keys.pressed(key) {
                self.class_id = resources
                    .class_by_index(index as i32)
                    .map(|class| class.id.as_str().to_owned())
                    .unwrap_or_else(|| "knight".to_owned());
                self.screen = Screen::CreateGame;
                // Do not leak the committing letter into name entry.
                let _ = keys.chars_pressed();
                return None;
            }
        }
        None
    }

    fn update_save_picker(&mut self, keys: &dyn KeySource) -> Option<StartupChoice> {
        let row_count = SAVE_SLOT_COUNT + 1;
        if keys.pressed(Key::Escape) {
            self.screen = Screen::ClassSelect;
            return None;
        }
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            self.cursor = (self.cursor + row_count - 1) % row_count;
        }
        if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            self.cursor = (self.cursor + 1) % row_count;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) || keys.pressed(Key::Space) {
            if self.cursor == SAVE_SLOT_COUNT {
                // The "New game" row targets the first empty slot.
                let mut target = 0;
                for (slot, header) in self.slots.headers.iter().enumerate() {
                    if header.is_none() {
                        target = slot;
                        break;
                    }
                }
                self.slot = target;
                self.screen = Screen::CreateGame;
                return None;
            }
            self.slot = self.cursor;
            if self.slots.headers[self.cursor].is_some() {
                return Some(self.choice(StartupAction::Load));
            }
            // An empty slot chosen directly starts a new game into it.
            self.screen = Screen::CreateGame;
        }
        None
    }

    fn update_create_game(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        time: f64,
    ) -> Option<StartupChoice> {
        if !self.has_name {
            return self.update_name(keys, resources);
        }
        if keys.pressed(Key::Escape) {
            self.screen = Screen::ClassSelect;
            return None;
        }
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            self.difficulty = (self.difficulty - 1).max(0);
        }
        if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            self.difficulty = (self.difficulty + 1).min(3);
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
            self.screen = Screen::Intro;
            self.intro_started = time;
            let _ = keys.chars_pressed();
        }
        None
    }

    fn update_name(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
    ) -> Option<StartupChoice> {
        if keys.pressed(Key::Escape) {
            self.screen = Screen::ClassSelect;
            return None;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
            if self.name.is_empty() {
                let default = resources.world.default_name.as_str();
                self.name = if default.is_empty() {
                    "Hero".to_owned()
                } else {
                    default.to_owned()
                };
            }
            // C uppercases the first character only.
            let mut chars: Vec<char> = self.name.chars().collect();
            if let Some(first) = chars.first_mut() {
                *first = first.to_ascii_uppercase();
            }
            self.name = chars.into_iter().collect();
            self.has_name = true;
            return None;
        }
        if keys.pressed(Key::Backspace) {
            self.name.pop();
            return None;
        }
        for ch in keys.chars_pressed() {
            if self.name.chars().count() >= NAME_MAX {
                break;
            }
            if ch.is_ascii_alphanumeric() || ch == ' ' {
                self.name.push(ch);
            }
        }
        None
    }

    fn update_intro(&mut self, keys: &dyn KeySource, time: f64) -> Option<StartupChoice> {
        if keys.any_pressed() || (time - self.intro_started) >= INTRO_TIMEOUT {
            Some(self.choice(StartupAction::New))
        } else {
            None
        }
    }

    fn choice(&self, action: StartupAction) -> StartupChoice {
        StartupChoice {
            action,
            slot: self.slot,
            class_id: self.class_id.clone(),
            name: self.name.clone(),
            difficulty: self.difficulty,
        }
    }

    /// Emit one frame of the current screen.
    pub fn draw(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        time: f64,
    ) {
        list.rect(
            Rect::new(0, 0, layout.screen_w, layout.screen_h),
            palette.clr(Pal::Black),
        );
        self.draw_class_picker(list, layout);
        self.draw_status_hint(list, layout, palette, resources);
        match self.screen {
            Screen::ClassSelect => {}
            Screen::SavePicker => self.draw_save_picker(list, layout, palette, resources),
            Screen::CreateGame => self.draw_create_game(list, layout, palette, resources, time),
            Screen::Intro => self.draw_intro(list, layout, palette, resources),
        }
    }

    fn draw_class_picker(&self, list: &mut DrawList, layout: &Layout) {
        let px = (layout.screen_w - CLASS_PICKER_W) / 2;
        let py = (layout.screen_h - CLASS_PICKER_H) / 2;
        list.blit(
            Sprite::ClassPicker,
            Dst::rect(px, py, CLASS_PICKER_W, CLASS_PICKER_H),
        );
    }

    fn draw_status_hint(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        let hint = banner::text(
            resources,
            "startup",
            "class_select_hint",
            DEFAULT_CLASS_SELECT_HINT,
        );
        list.rect(
            Rect::new(0, 0, layout.screen_w, GLYPH_H + 2),
            palette.clr(Pal::DRed),
        );
        let x = (layout.screen_w - hint.chars().count() as i32 * GLYPH_W) / 2;
        list.text(hint, x.max(0), 1, palette.clr(Pal::White));
    }

    fn draw_save_picker(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        let row_count = SAVE_SLOT_COUNT + 1;
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

        let title = banner::text(
            resources,
            "startup",
            "save_picker_title",
            DEFAULT_SAVE_PICKER_TITLE,
        );
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

        let new_fg = if self.cursor == SAVE_SLOT_COUNT {
            palette.clr(Pal::Yellow)
        } else {
            palette.clr(Pal::White)
        };
        let new_label = banner::text(
            resources,
            "startup",
            "save_picker_new_game",
            DEFAULT_SAVE_PICKER_NEW_GAME,
        );
        list.text(&format!("    {new_label}"), x + pad, ty, new_fg);

        let hint = banner::text(resources, "startup", "controls_hint", DEFAULT_CONTROLS_HINT);
        list.text(hint, x + pad, y + h - pad - instr_h, palette.clr(Pal::Grey));
    }

    fn draw_create_game(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        time: f64,
    ) {
        let w = 30 * GLYPH_W;
        let h = 12 * GLYPH_H;
        let x = (layout.screen_w - w) / 2;
        let y = (layout.screen_h - h) / 2;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let class_title = self.class_title(resources);
        let prefix = format!(" {class_title:<9} Name: ");
        list.text(&prefix, x + GLYPH_W, y + GLYPH_H, palette.clr(Pal::White));

        let name_x = x + GLYPH_W * 18;
        let name_y = y + GLYPH_H;
        list.text(&self.name, name_x, name_y, palette.clr(Pal::White));
        let show_caret = ((time * 2.0) as i64) & 1 == 0;
        if !self.has_name && show_caret && self.name.chars().count() < NAME_MAX {
            let caret_x = name_x + self.name.chars().count() as i32 * GLYPH_W;
            list.rect(
                Rect::new(caret_x, name_y, 1, GLYPH_H),
                palette.clr(Pal::Yellow),
            );
        }

        let header = banner::text(
            resources,
            "startup",
            "new_game_table_header",
            DEFAULT_NEW_GAME_TABLE_HEADER,
        );
        list.text(
            header,
            x + GLYPH_W,
            y + GLYPH_H * 3,
            palette.clr(Pal::White),
        );

        for (i, key) in DIFFICULTY_KEYS.iter().enumerate() {
            let label_key = format!("{key}.label");
            let label = banner::text(
                resources,
                "difficulty",
                &label_key,
                DEFAULT_DIFFICULTY_LABELS[i],
            );
            let score_key = format!("{key}.score_mult");
            let score = banner::text(
                resources,
                "difficulty",
                &score_key,
                DEFAULT_DIFFICULTY_SCORES[i],
            );
            let days = resources.time.days_per_difficulty[i];
            let line = format!("   {label:<11} {days:>3}    {score}");
            list.text(
                &line,
                x + GLYPH_W,
                y + GLYPH_H * (5 + i as i32),
                palette.clr(Pal::White),
            );
        }

        if self.has_name {
            list.text(
                ">",
                x + GLYPH_W,
                y + GLYPH_H * (5 + self.difficulty),
                palette.clr(Pal::White),
            );
            let hint = banner::text(
                resources,
                "startup",
                "new_game_select_hint",
                DEFAULT_NEW_GAME_SELECT_HINT,
            );
            list.text(hint, x + GLYPH_W, y + GLYPH_H * 10, palette.clr(Pal::White));
        }
    }

    fn draw_intro(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        let class_title = self.class_title(resources);
        let body = banner::formatted(
            resources,
            "banners",
            "new_game_intro",
            DEFAULT_NEW_GAME_INTRO,
            &[("NAME", &self.name), ("CLASS", &class_title)],
        );
        let lines: Vec<&str> = body.split('\n').collect();
        let max_chars = lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0);
        let panel_w = max_chars as i32 * GLYPH_W + INTRO_PAD * 2;
        let panel_h = lines.len() as i32 * GLYPH_H + INTRO_PAD * 2;
        let px = (layout.screen_w - panel_w) / 2;
        let py = (layout.screen_h - panel_h) / 2;
        list.rect(Rect::new(px, py, panel_w, panel_h), palette.clr(Pal::DBlue));
        list.rect_lines(
            Rect::new(px, py, panel_w, panel_h),
            palette.clr(Pal::Yellow),
        );
        let mut y = py + INTRO_PAD;
        for line in lines {
            list.text(line, px + INTRO_PAD, y, palette.clr(Pal::White));
            y += GLYPH_H;
        }
    }

    fn class_title(&self, resources: &Resources) -> String {
        resources
            .class_by_id(self.class_id.as_str())
            .map(|class| class.name.as_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("Hero")
            .to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Keys;
    use openbounty_engine::state_serialize::SAVE_VERSION;

    fn resources() -> Resources {
        Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [], "pack_id": "openbounty",
                "world": {"default_name": "Hero"},
                "classes": [
                  {"index":0,"id":"knight","name":"Knight","portrait":"k.png","starting_gold":1000},
                  {"index":1,"id":"paladin","name":"Paladin","portrait":"p.png","starting_gold":1000}
                ],
                "time": {"days_per_difficulty": {"easy":900,"normal":600,"hard":400,"impossible":200}},
                "strings": {"difficulty": {"easy":{"label":"Easy","score_mult":"x.5"}}}}"#,
        )
        .expect("fixture parses")
    }

    fn layout() -> Layout {
        Layout::from_resources(&resources()).expect("lays out")
    }

    /// A flat save dir unique to one test, so parallel tests never collide.
    fn paths(tag: &str) -> SavePaths {
        let dir = std::env::temp_dir().join(format!("openbounty-startup-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        SavePaths::with_override(dir)
    }

    /// A minimal valid save the header reader accepts.
    fn write_save(path: &std::path::Path, name: &str, days: i32) {
        let body = format!(
            r#"{{"version": {SAVE_VERSION},
                 "character": {{"name": "{name}", "rank_title": "General"}},
                 "stats": {{"days_left": {days}}},
                 "position": {{"zone": "home"}}}}"#
        );
        std::fs::write(path, body).expect("writes");
    }

    /// Confirm the (possibly typed) name, accept the difficulty and dismiss
    /// the intro, from the create-game screen.
    fn finish_create_game(
        startup: &mut Startup,
        resources: &Resources,
        paths: &SavePaths,
    ) -> StartupChoice {
        assert_eq!(
            startup.update(&Keys::pressed(&[Key::Enter]), resources, paths, 0.0),
            None
        );
        assert_eq!(
            startup.update(&Keys::pressed(&[Key::Enter]), resources, paths, 0.0),
            None
        );
        startup
            .update(&Keys::pressed(&[Key::Space]), resources, paths, 0.0)
            .expect("intro dismisses")
    }

    /// Walk class A -> type "hero" -> name -> difficulty -> dismiss the intro.
    fn run_wizard(
        startup: &mut Startup,
        resources: &Resources,
        paths: &SavePaths,
    ) -> StartupChoice {
        assert_eq!(
            startup.update(
                &Keys::pressed(&[Key::A]).with_chars(&['A']),
                resources,
                paths,
                0.0
            ),
            None
        );
        assert_eq!(
            startup.update(
                &Keys::pressed(&[]).with_chars(&['h', 'e', 'r', 'o']),
                resources,
                paths,
                0.0
            ),
            None
        );
        finish_create_game(startup, resources, paths)
    }

    #[test]
    fn the_wizard_yields_the_picked_class_and_typed_name() {
        let resources = resources();
        let paths = paths("wizard");
        let mut startup = Startup::new();
        let choice = run_wizard(&mut startup, &resources, &paths);
        assert_eq!(choice.action, StartupAction::New);
        assert_eq!(choice.slot, 0);
        assert_eq!(choice.class_id, "knight");
        assert_eq!(choice.name, "Hero");
        assert_eq!(choice.difficulty, 1);
    }

    #[test]
    fn escape_at_class_select_quits() {
        let resources = resources();
        let paths = paths("quit");
        let mut startup = Startup::new();
        let choice = startup
            .update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0)
            .expect("quits");
        assert_eq!(choice.action, StartupAction::Quit);
    }

    #[test]
    fn an_empty_name_falls_back_to_the_world_default() {
        let resources = resources();
        let paths = paths("default-name");
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0);
        // The second Enter confirms the (defaulted) name and opens difficulty.
        assert_eq!(
            startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0),
            None
        );
        // The intro is now showing; dismiss it to read the name back.
        let choice = startup
            .update(&Keys::pressed(&[Key::Space]), &resources, &paths, 0.0)
            .expect("intro dismisses");
        assert_eq!(choice.name, "Hero");
    }

    #[test]
    fn the_name_caps_at_ten_characters() {
        let resources = resources();
        let paths = paths("name-cap");
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        startup.update(
            &Keys::pressed(&[])
                .with_chars(&['a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l']),
            &resources,
            &paths,
            0.0,
        );
        startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0);
        let choice = startup
            .update(&Keys::pressed(&[Key::Space]), &resources, &paths, 0.0)
            .expect("intro dismisses");
        assert_eq!(choice.name, "Abcdefghij");
    }

    #[test]
    fn the_intro_auto_advances_on_the_timeout() {
        let resources = resources();
        let paths = paths("intro-timeout");
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 10.0);
        // No key, but past the 4-second hold.
        let choice = startup.update(&Keys::pressed(&[]), &resources, &paths, 15.0);
        assert_eq!(choice.expect("times out").action, StartupAction::New);
    }

    #[test]
    fn class_select_draws_the_hint_bar_and_picker() {
        let resources = resources();
        let startup = Startup::new();
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(&mut list, &layout, &palette, &resources, 0.0);

        assert!(list
            .texts()
            .iter()
            .any(|(_, _, text)| text.contains("Select Char A-D")));
        assert!(list.blits_of(Sprite::ClassPicker).len() == 1);
    }

    #[test]
    fn the_create_game_screen_shows_the_difficulty_table() {
        let resources = resources();
        let paths = paths("difficulty-draw");
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(&mut list, &layout, &palette, &resources, 0.0);

        let texts = list.texts();
        assert!(texts.iter().any(|(_, _, text)| text.contains("Knight")));
        assert!(texts.iter().any(|(_, _, text)| text.contains("Difficulty")));
        // The pack's own label and the default days value both show.
        assert!(texts
            .iter()
            .any(|(_, _, text)| text.contains("Easy") && text.contains("900")));
    }

    #[test]
    fn the_picker_loads_an_existing_slot() {
        let resources = resources();
        let paths = paths("picker-load");
        write_save(&paths.slot("openbounty", 4).expect("slot"), "Murray", 123);
        let mut startup = Startup::new();

        // L opens the picker with the cursor on the first existing slot.
        assert_eq!(
            startup.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0),
            None
        );
        let choice = startup
            .update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0)
            .expect("loads");
        assert_eq!(choice.action, StartupAction::Load);
        assert_eq!(choice.slot, 4);
    }

    #[test]
    fn the_new_row_targets_the_first_empty_slot() {
        let resources = resources();
        let paths = paths("picker-new");
        write_save(&paths.slot("openbounty", 0).expect("slot"), "Murray", 1);
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0);
        // The cursor starts on slot 0; walk to the "New game" row.
        for _ in 0..SAVE_SLOT_COUNT {
            startup.update(&Keys::pressed(&[Key::Down]), &resources, &paths, 0.0);
        }
        // Choosing New starts the wizard; slot 1 is the first empty.
        assert_eq!(
            startup.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 0.0),
            None
        );
        let choice = finish_create_game(&mut startup, &resources, &paths);
        assert_eq!(choice.action, StartupAction::New);
        assert_eq!(choice.slot, 1);
    }

    #[test]
    fn escape_from_the_picker_returns_to_class_select() {
        let resources = resources();
        let paths = paths("picker-esc");
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0);
        // Back at the root: ESC now quits rather than stepping back.
        let choice = startup
            .update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0)
            .expect("quits");
        assert_eq!(choice.action, StartupAction::Quit);
    }

    #[test]
    fn the_picker_lists_a_save_with_its_summary() {
        let resources = resources();
        let paths = paths("picker-draw");
        write_save(&paths.slot("openbounty", 2).expect("slot"), "Murray", 123);
        let mut startup = Startup::new();
        startup.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(&mut list, &layout, &palette, &resources, 0.0);

        let texts = list.texts();
        assert!(texts
            .iter()
            .any(|(_, _, text)| text.contains("Select game:")));
        assert!(texts
            .iter()
            .any(|(_, _, text)| text.contains("Murray") && text.contains("123d")));
        assert!(texts.iter().any(|(_, _, text)| text.contains("New game")));
    }
}
