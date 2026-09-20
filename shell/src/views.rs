//! Full-screen view overlays, porting `views.c`'s stack and the read-only
//! views of `views_render.c`.
//!
//! A view covers the map area (and, for the character view, the sidebar too)
//! with its own panel while the overworld keeps running underneath. Views are
//! stacked so a sub-screen can ESC back to its parent; the top of the stack is
//! the active view and an empty stack is the overworld.
//!
//! Nothing here touches a graphics library: each view emits
//! [`crate::draw::DrawList`] commands, so the layout is testable headlessly
//! the way the engine's combat digests are.

use openbounty_engine::fog::Fog;
use openbounty_engine::game::{
    CastleOwnerKind, Game, GateDestination, Mount, GAME_ARMY_SLOTS, GAME_CASTLES, GAME_CONTINENTS,
    GAME_GATE_DESTS_MAX,
};
use openbounty_engine::map::Map;
use openbounty_engine::resources::{ResColors, Resources, RES_MAX_CONTROLS};
use openbounty_engine::tables::puzzle_grid_entity;
use openbounty_engine::tile::{Interact, Terrain};

use crate::banner;
use crate::draw::{Color, DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::input::{Key, KeySource};
use crate::layout::{Layout, FRAME_BOTTOM_H, FRAME_RIGHT_W, MAP_X, MAP_Y, PANEL_H, PANEL_PAD_X};
use crate::palette::{Pal, Palette};
use crate::present::Present;
use crate::sprites::SpriteCatalog;

/// The character portrait's legacy size. C defaults to 96x102 and the shipped
/// pack's class portraits are exactly that, so the belt below it is placed a
/// deterministic 102px from the view top.
const PORTRAIT_W: i32 = 96;
const PORTRAIT_H: i32 = 102;

/// The wanted-poster face and the no-contract silhouette are both 48x34.
const FACE_W: i32 = 48;
const FACE_H: i32 = 34;

/// Inset around the worldmap's grid inside the map panel, C's `VIEW_PAD`.
const VIEW_PAD: i32 = 4;

/// Controls-panel geometry, C's `draw_controls` literals: a `CTL_COLS`-glyph
/// panel with `CTL_PAD` pixels of padding, flush against the left edge of the
/// map area.
const CTL_PAD: i32 = 3;
const CTL_COLS: i32 = 18;

/// Controls-panel label text C compiles in, overridden under `strings.controls`.
const DEFAULT_CTL_TITLE: &str = " Controls ";
const DEFAULT_CTL_ON: &str = "On";
const DEFAULT_CTL_OFF: &str = "Off";

/// Spells-view strings, overridden under `strings.spells_view`.
const DEFAULT_SV_TITLE: &str = "Spells";
const DEFAULT_SV_COMBAT: &str = "Combat";
const DEFAULT_SV_ADVENTURE: &str = "Adventuring";

/// The A-G keys the spells view reads, in order. C indexes `KEY_A + i`, which
/// only works because raylib's letters are contiguous; this enum is sparse, so
/// the seven keys are spelled out.
const SPELL_KEYS: [Key; 7] = [Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G];

/// The full A-Z key row, for the gate picker's letter shortcuts. Ports C's
/// `KEY_A + letter` indexing.
const LETTER_KEYS: [Key; 26] = [
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::F,
    Key::G,
    Key::H,
    Key::I,
    Key::J,
    Key::K,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::Q,
    Key::R,
    Key::S,
    Key::T,
    Key::U,
    Key::V,
    Key::W,
    Key::X,
    Key::Y,
    Key::Z,
];

/// How many characters of a gate destination name the picker shows, C's
/// `GATE_NAME_COL`.
const GATE_NAME_COL: usize = 14;

/// Contract-view labels C compiles in; the pack overrides them under
/// `strings.contract_view`.
const DEFAULT_CV_TITLE_NO_CONTRACT: &str = "You have no Contract!";
const DEFAULT_CV_LABEL_NAME: &str = "Name: %VALUE%";
const DEFAULT_CV_LABEL_ALIAS: &str = "Alias: %VALUE%";
const DEFAULT_CV_LABEL_REWARD: &str = "Reward: %VALUE% gold";
const DEFAULT_CV_LABEL_LAST_SEEN: &str = "Last Seen: %VALUE%";
const DEFAULT_CV_LABEL_CASTLE: &str = "Castle: %VALUE%";
const DEFAULT_CV_ALIAS_NONE: &str = "None";
const DEFAULT_CV_CASTLE_UNKNOWN: &str = "Unknown";
const DEFAULT_CV_FEATURES_HEADER: &str = "Distinguishing Features:";
const DEFAULT_CV_CRIMES_HEADER: &str = "Crimes:";

/// Which overlay is on top of the stack.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ViewKind {
    Character,
    Army,
    Contract,
    Puzzle,
    Worldmap,
    Controls,
    Options,
    Spells,
    Gate,
    Town,
    HomeCastle,
    OwnCastle,
    RecruitSoldiers,
    Dwelling,
    WinCartoon,
    Win,
    Lose,
}

/// What one frame of spells-view input did, porting `views_spells_update`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellsInput {
    /// A spell was chosen; the view has already dismissed itself.
    Cast(i32),
    /// A navigation key (Left/Right/ESC) was handled; the view stays open.
    Handled,
    /// Nothing was handled; the caller may dismiss on any other key.
    None,
}

/// The town menu's action rows, porting `TownRow`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TownRow {
    Contract,
    Boat,
    Info,
    Spell,
    Siege,
}

impl TownRow {
    fn from_index(index: usize) -> TownRow {
        match index {
            0 => TownRow::Contract,
            1 => TownRow::Boat,
            2 => TownRow::Info,
            3 => TownRow::Spell,
            _ => TownRow::Siege,
        }
    }
}

/// What one frame of town-view input did, porting `views_town_update`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TownInput {
    /// No key was handled.
    None,
    /// A navigation or info-dismiss key was handled; the view stays open.
    Handled,
    /// ESC closed the view.
    Dismissed,
    /// The player executed a row; the caller runs the transaction.
    RunRow(TownRow),
}

/// The town menu's five rows.
const TOWN_ROW_COUNT: i32 = 5;

/// The A-E keys that jump straight to a town row.
const TOWN_KEYS: [Key; 5] = [Key::A, Key::B, Key::C, Key::D, Key::E];

/// The recruit-soldiers list holds at most five castle-dwelling troops.
const RECRUIT_SLOTS: usize = 5;

/// Gold held back from a max-out recruit, C's `RECRUIT_GOLD_RESERVE`, so the
/// hero keeps several weeks of boat upkeep.
const RECRUIT_GOLD_RESERVE: i32 = 1000;

/// End-screen geometry, C's `screen_end_game_draw` constants: the native
/// ending art is 144x170, inset from the text by `MARGIN_L`/`MARGIN_T`.
const END_IMG_W: i32 = 144;
const END_IMG_H: i32 = 170;
const END_MARGIN_L: i32 = 6;
const END_MARGIN_T: i32 = 2;

/// Seconds per cartoon tick, C's `tick_interval`.
const CARTOON_TICK: f64 = 0.08;

/// The view stack. `Views::default()` is the empty (overworld) stack.
#[derive(Clone, Debug, Default)]
pub struct Views {
    stack: Vec<ViewKind>,
    /// When the puzzle view became active, for its reveal animation.
    puzzle_open_time: f64,
    /// Whether the puzzle view was active the previous frame.
    puzzle_was_active: bool,
    /// The scepter's continent, lazy-loaded for the puzzle's terrain backdrop.
    puzzle_map: Option<Map>,
    puzzle_zone: String,
    /// Whether the worldmap is showing the whole continent or just the fog the
    /// hero has revealed. Ports C's `s_worldmap_whole_map` static.
    worldmap_whole_map: bool,
    /// Which visible row the controls panel highlights. Ports C's
    /// `g_controls_cursor` static.
    controls_cursor: i32,
    /// Whether the spells view is in cast mode (opened with U) rather than the
    /// read-only view the game menu opens. Ports `spell_state.active`.
    spells_cast_mode: bool,
    /// Which spells column the A-G keys address: 0 combat, 1 adventure.
    /// Ports `spell_state.column`.
    spells_column: i32,
    /// The gate picker's snapshot, porting the `gate_view` static.
    gate_list: [GateDestination; GAME_GATE_DESTS_MAX],
    gate_count: usize,
    gate_is_town: bool,
    gate_cursor: usize,
    gate_chosen: i32,
    /// The town menu's context, porting the `town` static.
    town_display_name: String,
    town_record_key: String,
    town_boat_x: i32,
    town_boat_y: i32,
    town_cursor: i32,
    town_info_body: String,
    town_info_active: bool,
    /// The own-castle screen's context, porting `s_castle_id` and
    /// `s_garrison_mode`.
    own_castle_id: String,
    own_castle_garrison_mode: bool,
    /// The recruit-soldiers screen's context, porting the `s_*` statics.
    recruit_pool: [i32; RECRUIT_SLOTS],
    recruit_pool_count: usize,
    recruit_anim_troop: i32,
    recruit_whom: i32,
    recruit_max: i32,
    recruit_input: String,
    recruit_error: String,
    recruit_error_just_set: bool,
    /// The dwelling screen's context, porting the `s_*` statics.
    dwelling_kind: Interact,
    dwelling_troop_index: i32,
    dwelling_troop_name: String,
    dwelling_pop: i32,
    dwelling_cost: i32,
    dwelling_gold: i32,
    dwelling_cap: i32,
    /// The end-screen body and verdict, porting `screen_end_game.c`'s cached
    /// `s_body` / `s_won`.
    end_won: bool,
    end_body: String,
    /// When the win cartoon became active, for its frame clock, and whether it
    /// was active the previous frame.
    cartoon_started: f64,
    cartoon_was_active: bool,
}

impl Views {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active(&self) -> Option<ViewKind> {
        self.stack.last().copied()
    }

    pub fn is_active(&self) -> bool {
        !self.stack.is_empty()
    }

    /// Replace the stack with a single view.
    pub fn set(&mut self, view: ViewKind) {
        self.stack.clear();
        self.stack.push(view);
    }

    /// Pop the top view. Popping an empty stack is a no-op.
    pub fn dismiss(&mut self) {
        self.stack.pop();
    }

    /// Open the win or lose end screen, porting `screen_end_game_open`: the
    /// verdict and the already-substituted body are cached for the renderer.
    pub fn open_end_game(&mut self, won: bool, body: &str) {
        self.end_won = won;
        self.end_body = body.to_owned();
        self.set(if won { ViewKind::Win } else { ViewKind::Lose });
    }

    pub fn end_won(&self) -> bool {
        self.end_won
    }

    pub fn end_body(&self) -> &str {
        &self.end_body
    }

    /// Open the win ending, starting with the cartoon cutscene when the pack
    /// ships its tiles, porting `run_end_cartoon` + `show_win_game`.
    pub fn win_cartoon_open(&mut self, resources: &Resources, body: &str) {
        self.end_won = true;
        self.end_body = body.to_owned();
        let ending = &resources.ending;
        if ending.grass_tile.is_empty()
            || ending.carpet_tile.is_empty()
            || ending.hero_tile.is_empty()
        {
            self.set(ViewKind::Win);
        } else {
            self.cartoon_started = 0.0;
            self.cartoon_was_active = false;
            self.set(ViewKind::WinCartoon);
        }
    }

    /// Leave the cartoon for the win screen.
    pub fn finish_win_cartoon(&mut self) {
        self.set(ViewKind::Win);
    }

    /// True once the cartoon has played past its last frame.
    pub fn win_cartoon_done(&self, resources: &Resources, time: f64) -> bool {
        let (_, frame) = cartoon_clock(resources, time - self.cartoon_started);
        let max_frames = if resources.ending.frame_count > 0 {
            resources.ending.frame_count
        } else {
            10
        };
        frame > max_frames
    }

    /// Ports `screen_end_game_draw`: the ending art on the right of the map
    /// area over a DBlue field, with the pre-wrapped body text on the left.
    ///
    /// DIVERGENCE, deliberate: C crops the image's left `MARGIN_L` pixels to
    /// clear the text. The DrawList blit has no source rectangle, so this
    /// squeezes the whole image into the same narrowed box instead.
    fn draw_end_game(&self, list: &mut DrawList, layout: &Layout, palette: &Palette, won: bool) {
        let left = MAP_X;
        let top = MAP_Y;
        let w = layout.screen_w - MAP_X - FRAME_RIGHT_W;
        let h = layout.screen_h - MAP_Y - FRAME_BOTTOM_H;
        list.rect(Rect::new(left, top, w, h), palette.clr(Pal::DBlue));

        let draw_w = (END_IMG_W - END_MARGIN_L).max(1);
        let img_x = left + w - draw_w;
        let sprite = if won {
            Sprite::EndingWin
        } else {
            Sprite::EndingLose
        };
        list.blit(sprite, Dst::rect(img_x, top, draw_w, END_IMG_H));

        let max_chars = ((w - END_IMG_W) / font::GLYPH_W).max(1) as usize;
        let tx = left + END_MARGIN_L;
        let mut ty = top + END_MARGIN_T;
        let floor_y = top + h;

        for logical in self.end_body.split('\n') {
            let mut cursor = logical.to_owned();
            while cursor.chars().count() > max_chars && ty + font::GLYPH_H <= floor_y {
                // Break at the last space that fits, hard-breaking if none.
                let chars: Vec<char> = cursor.chars().collect();
                let mut cut = max_chars;
                while cut > 0 && chars[cut] != ' ' {
                    cut -= 1;
                }
                if cut == 0 {
                    cut = max_chars;
                }
                let line: String = chars[..cut].iter().collect();
                list.text(&line, tx, ty, palette.clr(Pal::White));
                ty += font::GLYPH_H;
                cursor = chars[cut..]
                    .iter()
                    .collect::<String>()
                    .trim_start_matches(' ')
                    .to_owned();
            }
            if ty + font::GLYPH_H <= floor_y {
                list.text(&cursor, tx, ty, palette.clr(Pal::White));
                ty += font::GLYPH_H;
            }
        }
    }

    /// Ports `run_end_cartoon`'s `draw_cartoon_frame`: grass across the grid, a
    /// carpet column growing from the bottom, the hero advancing along it, and
    /// the troop border filling the other cells.
    fn draw_win_cartoon(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        time: f64,
    ) {
        let ending = &resources.ending;
        let gw = if ending.grid_width > 0 {
            ending.grid_width
        } else {
            6
        };
        let gh = if ending.grid_height > 0 {
            ending.grid_height
        } else {
            5
        };
        let carpet_col = ending.carpet_column;
        let carpet_max = ending.carpet_length;
        let tile_w = layout.tile_w;
        let tile_h = layout.tile_h;
        let total_w = gw * tile_w;
        let origin_x = ((layout.screen_w - total_w) / 2).max(0);
        let origin_y = MAP_Y;

        let (tick, frame) = cartoon_clock(resources, time - self.cartoon_started);
        let bridge_len = frame.min(carpet_max);
        let hero_prog = (frame - 5).min(gh - 1);

        // C clears the whole screen to black before the grid, so the cartoon
        // owns the frame (no chrome showing through).
        list.rect(
            Rect::new(0, 0, layout.screen_w, layout.screen_h),
            palette.clr(Pal::Black),
        );

        let cell = |x: i32, y: i32| {
            Dst::rect(origin_x + x * tile_w, origin_y + y * tile_h, tile_w, tile_h)
        };

        for y in 0..gh {
            for x in 0..gw {
                list.blit(Sprite::EndGrass, cell(x, y));
            }
        }
        for i in 0..bridge_len {
            list.blit(Sprite::EndCarpet, cell(carpet_col, gh - 1 - i));
        }
        if hero_prog >= 0 {
            list.blit(Sprite::EndHero, cell(carpet_col, gh - 1 - hero_prog));
        }
        if ending.troop_border {
            let nt = resources.troops_count().min(25);
            let mut x = 0;
            let mut y = 0;
            let mut index = 0;
            while index < nt && y < gh {
                let flip = x == gw - 1;
                list.blit_flipped(troop_cartoon_sprite(catalog, index, tick), cell(x, y), flip);
                x += 1;
                if x == carpet_col {
                    x = carpet_col + 1;
                }
                if x >= gw {
                    x = 0;
                    y += 1;
                }
                index += 1;
            }
        }
    }

    /// True when the active view swaps the status bar to "Press 'ESC' to
    /// exit". The character, army and puzzle views always do; the contract
    /// view only when a contract is actually active. The worldmap is handled
    /// separately by [`StatusMode::Worldmap`](crate::chrome::StatusMode).
    pub fn wants_exit_hint(&self, game: &Game) -> bool {
        match self.active() {
            Some(
                ViewKind::Character
                | ViewKind::Army
                | ViewKind::Puzzle
                | ViewKind::Controls
                | ViewKind::Options
                | ViewKind::Spells
                | ViewKind::Gate
                | ViewKind::Town
                | ViewKind::HomeCastle
                | ViewKind::OwnCastle
                | ViewKind::RecruitSoldiers
                | ViewKind::Dwelling
                | ViewKind::WinCartoon
                | ViewKind::Win
                | ViewKind::Lose,
            ) => true,
            Some(ViewKind::Contract) => !game.contract.active_id.is_empty(),
            Some(ViewKind::Worldmap) | None => false,
        }
    }

    /// Whether the worldmap is revealing the whole continent (true) or only
    /// the fog the hero has uncovered (false).
    pub fn whole_map(&self) -> bool {
        self.worldmap_whole_map
    }

    /// Flip the worldmap between fog-limited and whole-continent. Ports
    /// `views_render_worldmap_toggle_hero_only`.
    pub fn toggle_whole_map(&mut self) {
        self.worldmap_whole_map = !self.worldmap_whole_map;
    }

    /// The controls panel's highlighted visible row. Ports
    /// `views_controls_cursor`.
    pub fn controls_cursor(&self) -> i32 {
        self.controls_cursor
    }

    /// Set the controls cursor, clamping a negative row to zero exactly as C
    /// does (`views_controls_set_cursor`).
    pub fn controls_set_cursor(&mut self, row: i32) {
        self.controls_cursor = row.max(0);
    }

    /// Cycle one control's value in `stats.options`, porting
    /// `views_controls_advance`. A disabled row is a no-op, and the range is
    /// floored at 2 so a value always has somewhere to go.
    pub fn controls_advance(&self, game: &mut Game, resources: &Resources, row: usize) {
        let Some(control) = resources.control_by_index(row) else {
            return;
        };
        if controls_row_disabled(resources, row) {
            return;
        }
        if row >= game.stats.options.len() {
            return;
        }
        let range = control.range.max(2);
        game.stats.options[row] = (game.stats.options[row] + 1) % range;
    }

    /// Arm (or disarm) the spells view's cast mode, porting
    /// `views_spells_set_mode`. Cast mode always starts on the adventure
    /// column, as C does for an overworld cast.
    pub fn spells_set_cast_mode(&mut self, cast_mode: bool) {
        self.spells_cast_mode = cast_mode;
        self.spells_column = 1;
    }

    /// Read one frame of spells-view input, porting `views_spells_update`.
    ///
    /// DIVERGENCE, deliberate: C's `views_spells_update` returns false after
    /// a Left/Right and `main.c` then falls through to `ui_any_key_pressed`,
    /// so the very column switch is defeated by an immediate dismiss. This
    /// port treats Left/Right as handled so the column actually switches.
    pub fn spells_update(&mut self, keys: &dyn KeySource) -> SpellsInput {
        if !self.spells_cast_mode {
            return SpellsInput::None;
        }
        let mut handled = false;
        if keys.pressed(Key::Left) {
            self.spells_column = 0;
            handled = true;
        }
        if keys.pressed(Key::Right) {
            self.spells_column = 1;
            handled = true;
        }
        if keys.pressed(Key::Escape) {
            self.spells_cast_mode = false;
            self.dismiss();
            handled = true;
        }
        for (i, key) in SPELL_KEYS.iter().enumerate() {
            if keys.pressed(*key) {
                let chosen = self.spells_column * 7 + i as i32;
                self.dismiss();
                return SpellsInput::Cast(chosen);
            }
        }
        if handled {
            SpellsInput::Handled
        } else {
            SpellsInput::None
        }
    }

    /// Open the gate destination picker with a snapshot of the eligible list,
    /// porting `views_gate_open`.
    pub fn gate_open(&mut self, destinations: &[GateDestination], is_town: bool) {
        let count = destinations.len().min(GAME_GATE_DESTS_MAX);
        self.gate_list[..count].copy_from_slice(&destinations[..count]);
        self.gate_count = count;
        self.gate_is_town = is_town;
        self.gate_cursor = 0;
        self.gate_chosen = -1;
        self.set(ViewKind::Gate);
    }

    /// The snapshot size, porting `views_gate_count`.
    pub fn gate_count(&self) -> usize {
        self.gate_count
    }

    /// Whether this picker lists towns (true) or castles (false), porting
    /// `views_gate_is_town`.
    pub fn gate_is_town(&self) -> bool {
        self.gate_is_town
    }

    /// The highlighted row, porting `views_gate_cursor`.
    pub fn gate_cursor(&self) -> usize {
        self.gate_cursor
    }

    /// A destination by snapshot index, porting `views_gate_dest`.
    pub fn gate_dest(&self, index: usize) -> Option<&GateDestination> {
        if index >= self.gate_count {
            return None;
        }
        Some(&self.gate_list[index])
    }

    /// Consume the confirmed destination index, porting `views_gate_chosen`.
    pub fn gate_take_chosen(&mut self) -> Option<usize> {
        if self.gate_chosen < 0 {
            return None;
        }
        let chosen = self.gate_chosen as usize;
        self.gate_chosen = -1;
        Some(chosen)
    }

    /// Read one frame of gate-picker input, porting `views_gate_update`.
    /// Returns true when a destination was confirmed (letter, Enter or Space);
    /// the caller reads it with [`Views::gate_take_chosen`].
    pub fn gate_update(&mut self, keys: &dyn KeySource) -> bool {
        let n = self.gate_count;
        if n == 0 {
            return false;
        }
        let left = (n + 1) / 2;

        if keys.pressed(Key::Escape) {
            self.dismiss();
            return false;
        }
        // Letter shortcut: each destination is keyed by the first letter of
        // its name, not its row. Ports C's `toupper(name[0]) - 'A'`.
        for index in 0..n {
            let Some(destination) = self.gate_dest(index) else {
                continue;
            };
            let Some(first) = destination.name.as_str().chars().next() else {
                continue;
            };
            let letter = first.to_ascii_uppercase() as i32 - 'A' as i32;
            if !(0..26).contains(&letter) {
                continue;
            }
            if keys.pressed(LETTER_KEYS[letter as usize]) {
                self.gate_chosen = index as i32;
                self.dismiss();
                return true;
            }
        }
        if keys.pressed(Key::Down) {
            self.gate_cursor = (self.gate_cursor + 1) % n;
        }
        if keys.pressed(Key::Up) {
            self.gate_cursor = if self.gate_cursor == 0 {
                n - 1
            } else {
                self.gate_cursor - 1
            };
        }
        // Left/Right jump columns, preserving the row offset.
        if keys.pressed(Key::Right) && self.gate_cursor < left {
            let target = self.gate_cursor + left;
            if target < n {
                self.gate_cursor = target;
            }
        }
        if keys.pressed(Key::Left) && self.gate_cursor >= left {
            self.gate_cursor -= left;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::Space) {
            self.gate_chosen = self.gate_cursor as i32;
            self.dismiss();
            return true;
        }
        false
    }

    /// Open the town menu with the visiting town's context, porting
    /// `views_open_town`. The cursor starts on the contract row.
    pub fn town_open(&mut self, display_name: &str, record_key: &str, boat_x: i32, boat_y: i32) {
        self.town_display_name = display_name.to_owned();
        self.town_record_key = record_key.to_owned();
        self.town_boat_x = boat_x;
        self.town_boat_y = boat_y;
        self.town_cursor = 0;
        self.town_info_body.clear();
        self.town_info_active = false;
        self.set(ViewKind::Town);
    }

    /// The visiting town's display name, porting `views_town_display_name`.
    pub fn town_display_name(&self) -> &str {
        &self.town_display_name
    }

    /// The visiting town's record key, porting `views_town_record_key`.
    pub fn town_record_key(&self) -> &str {
        &self.town_record_key
    }

    /// The town's boat dock, used by the rent-boat transaction.
    pub fn town_boat(&self) -> (i32, i32) {
        (self.town_boat_x, self.town_boat_y)
    }

    /// Show the info overlay, porting `town_show_info`.
    pub fn town_show_info(&mut self, body: &str) {
        self.town_info_body = body.to_owned();
        self.town_info_active = true;
    }

    /// Read one frame of town-menu input, porting `views_town_update`. When a
    /// row is executed the caller runs the transaction and shows the result
    /// with [`Views::town_show_info`].
    pub fn town_update(&mut self, keys: &dyn KeySource) -> TownInput {
        if self.town_info_active {
            // The info overlay owns the keys until ESC/Enter/Space or A-E.
            if keys.pressed(Key::Escape)
                || keys.pressed(Key::Enter)
                || keys.pressed(Key::KpEnter)
                || keys.pressed(Key::Space)
            {
                self.town_info_active = false;
                return TownInput::Handled;
            }
            for key in TOWN_KEYS {
                if keys.pressed(key) {
                    self.town_info_active = false;
                    return TownInput::Handled;
                }
            }
            return TownInput::Handled;
        }

        if keys.pressed(Key::Escape) {
            self.dismiss();
            return TownInput::Dismissed;
        }
        if keys.pressed(Key::Up) || keys.pressed(Key::W) || keys.pressed(Key::Kp8) {
            self.town_cursor = (self.town_cursor - 1 + TOWN_ROW_COUNT) % TOWN_ROW_COUNT;
            return TownInput::Handled;
        }
        if keys.pressed(Key::Down) || keys.pressed(Key::S) || keys.pressed(Key::Kp2) {
            self.town_cursor = (self.town_cursor + 1) % TOWN_ROW_COUNT;
            return TownInput::Handled;
        }
        if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) || keys.pressed(Key::Space) {
            return TownInput::RunRow(TownRow::from_index(self.town_cursor as usize));
        }
        for (index, key) in TOWN_KEYS.iter().enumerate() {
            if keys.pressed(*key) {
                self.town_cursor = index as i32;
                return TownInput::RunRow(TownRow::from_index(index));
            }
        }
        TownInput::None
    }

    /// Open the home castle (throne room / barracks), porting
    /// `screen_home_castle_open`.
    pub fn home_castle_open(&mut self) {
        self.set(ViewKind::HomeCastle);
    }

    /// Open the own-castle screen, porting `screen_own_castle_open`. The mode
    /// starts on REMOVE, as C does.
    pub fn own_castle_open(&mut self, castle_id: &str) {
        self.own_castle_id = castle_id.to_owned();
        self.own_castle_garrison_mode = false;
        self.set(ViewKind::OwnCastle);
    }

    /// Whether the own-castle screen is in garrison (true) or remove (false)
    /// mode, porting `screen_own_castle_is_garrison_mode`.
    pub fn own_castle_garrison_mode(&self) -> bool {
        self.own_castle_garrison_mode
    }

    /// Flip the own-castle mode, porting `screen_own_castle_toggle_mode`.
    pub fn own_castle_toggle_mode(&mut self) {
        self.own_castle_garrison_mode = !self.own_castle_garrison_mode;
    }

    /// The castle the own-castle screen is bound to, porting
    /// `screen_own_castle_castle_id`.
    pub fn own_castle_castle_id(&self) -> &str {
        &self.own_castle_id
    }

    /// Open the recruit-soldiers screen, porting `screen_recruit_soldiers_open`:
    /// build the castle-dwelling pool in recruit-cost order and pick a
    /// deterministic animated troop.
    pub fn recruit_soldiers_open(&mut self, resources: &Resources, game: &Game) {
        let mut pool = [0i32; RECRUIT_SLOTS];
        let mut count = 0;
        for index in 0..resources.troops_count() {
            if count >= RECRUIT_SLOTS {
                break;
            }
            if let Some(troop) = resources.troop_by_index(index as i32) {
                if troop.dwelling.as_str() == "castle" {
                    pool[count] = index as i32;
                    count += 1;
                }
            }
        }
        // Insertion sort by recruit cost ascending (n <= 5).
        for i in 1..count {
            let key = pool[i];
            let key_cost = resources
                .troop_by_index(key)
                .map(|t| t.recruit_cost)
                .unwrap_or(0);
            let mut j = i;
            while j > 0 {
                let prev_cost = resources
                    .troop_by_index(pool[j - 1])
                    .map(|t| t.recruit_cost)
                    .unwrap_or(0);
                if prev_cost <= key_cost {
                    break;
                }
                pool[j] = pool[j - 1];
                j -= 1;
            }
            pool[j] = key;
        }

        self.recruit_pool = pool;
        self.recruit_pool_count = count;
        self.recruit_anim_troop = if count > 0 {
            let mut h = game.seed ^ 0xC451_E11A;
            h ^= game.stats.days_left as u64;
            h = h.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
            pool[(h % count as u64) as usize]
        } else {
            -1
        };
        self.recruit_whom = 0;
        self.recruit_max = 9999;
        self.recruit_input.clear();
        self.recruit_error.clear();
        self.recruit_error_just_set = false;
        self.set(ViewKind::RecruitSoldiers);
    }

    /// The gold+leadership cap for a selected troop, porting `recompute_max`.
    fn recruit_recompute_max(&self, resources: &Resources, game: &Game, slot: usize) -> i32 {
        let Some(troop) = self
            .recruit_pool
            .get(slot)
            .and_then(|index| resources.troop_by_index(*index))
        else {
            return 0;
        };
        let mut max = game.max_recruitable(resources, troop.id.as_str()).max(0);
        if troop.recruit_cost > 0 {
            let budget = (game.stats.gold - RECRUIT_GOLD_RESERVE).max(0);
            let affordable = budget / troop.recruit_cost;
            max = max.min(affordable);
        }
        max
    }

    /// Read one frame of recruit-soldiers input, porting
    /// `screen_recruit_soldiers_update`. Returns true when the screen should be
    /// dismissed (ESC while idle).
    pub fn recruit_soldiers_update(
        &mut self,
        game: &mut Game,
        resources: &Resources,
        keys: &dyn KeySource,
    ) -> bool {
        // An error popup swallows input until the next key clears it.
        if !self.recruit_error.is_empty() {
            if self.recruit_error_just_set {
                self.recruit_error_just_set = false;
            } else if keys.any_pressed() {
                self.recruit_error.clear();
            }
            return false;
        }

        if self.recruit_whom == 0 {
            if keys.pressed(Key::Escape) {
                return true;
            }
            for i in 0..RECRUIT_SLOTS {
                let key = TOWN_KEYS[i]; // A..E
                if keys.pressed(key) {
                    if i < self.recruit_pool_count {
                        let troop = resources.troop_by_index(self.recruit_pool[i]);
                        let unreachable = troop.map_or(true, |t| {
                            t.hit_points <= 0 || game.stats.leadership_current < t.hit_points * 6
                        });
                        if !unreachable {
                            self.recruit_whom = (i + 1) as i32;
                            self.recruit_max = self.recruit_recompute_max(resources, game, i);
                            self.recruit_input.clear();
                        }
                    }
                    break;
                }
            }
        } else {
            if keys.pressed(Key::Escape) {
                self.recruit_whom = 0;
                self.recruit_input.clear();
            } else if keys.pressed(Key::Enter) || keys.pressed(Key::KpEnter) {
                let number: i32 = self.recruit_input.parse().unwrap_or(0);
                let pool_slot = (self.recruit_whom - 1) as usize;
                let troop_id = self
                    .recruit_pool
                    .get(pool_slot)
                    .and_then(|index| resources.troop_by_index(*index))
                    .map(|t| t.id.as_str().to_owned());
                if let Some(troop_id) = troop_id {
                    let mut count = number;
                    let max = self.recruit_recompute_max(resources, game, pool_slot);
                    if count > max {
                        count = max;
                    }
                    if count > 0 {
                        let outcome = game.buy_troop(resources, &troop_id, count);
                        if let Some(message) = crate::interact::recruit_refusal(resources, outcome)
                        {
                            self.recruit_error = message;
                            self.recruit_error_just_set = true;
                        }
                    }
                    self.recruit_max = self.recruit_recompute_max(resources, game, pool_slot);
                }
                self.recruit_whom = 0;
                self.recruit_input.clear();
            } else if keys.pressed(Key::Backspace) {
                self.recruit_input.pop();
            } else {
                for digit in 0..10u8 {
                    if keys.pressed(crate::input::DIGITS[digit as usize])
                        || keys.pressed(crate::input::NUMPAD_DIGITS[digit as usize])
                    {
                        if self.recruit_input.len() < 6 {
                            self.recruit_input.push(char::from(b'0' + digit));
                        }
                    }
                }
            }
        }
        false
    }

    /// Open the dwelling screen, porting `screen_dwelling_open`.
    pub fn dwelling_open(
        &mut self,
        kind: Interact,
        troop_index: i32,
        troop_name: &str,
        pop: i32,
        cost: i32,
        gold: i32,
        cap: i32,
    ) {
        self.dwelling_kind = kind;
        self.dwelling_troop_index = troop_index;
        self.dwelling_troop_name = troop_name.to_owned();
        self.dwelling_pop = pop;
        self.dwelling_cost = cost;
        self.dwelling_gold = gold;
        self.dwelling_cap = cap;
        self.set(ViewKind::Dwelling);
    }

    /// Draw the active view over the map area.
    pub fn draw(
        &mut self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        map: &Map,
        fog: &Fog,
        game: &Game,
        present: &Present,
        time: f64,
    ) {
        // The puzzle reveal animation resets its clock each time the view
        // becomes active.
        let puzzle_active = self.active() == Some(ViewKind::Puzzle);
        if puzzle_active && !self.puzzle_was_active {
            self.puzzle_open_time = time;
        }
        self.puzzle_was_active = puzzle_active;

        // The win cartoon's frame clock resets when it becomes active.
        let cartoon_active = self.active() == Some(ViewKind::WinCartoon);
        if cartoon_active && !self.cartoon_was_active {
            self.cartoon_started = time;
        }
        self.cartoon_was_active = cartoon_active;

        match self.active() {
            Some(ViewKind::Character) => draw_character(list, layout, palette, resources, game),
            Some(ViewKind::Army) => {
                draw_army(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::Contract) => {
                draw_contract(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::Puzzle) => {
                self.draw_puzzle(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::Worldmap) => {
                self.draw_worldmap(list, layout, palette, resources, map, fog, game, time)
            }
            Some(ViewKind::Controls) => {
                self.draw_controls(list, layout, palette, resources, game, present)
            }
            Some(ViewKind::Options) => draw_options(list, layout, palette, resources, game),
            Some(ViewKind::Spells) => self.draw_spells(list, layout, palette, resources, game),
            Some(ViewKind::Gate) => self.draw_gate(list, layout, palette),
            Some(ViewKind::Town) => {
                self.draw_town(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::HomeCastle) => {
                self.draw_home_castle(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::OwnCastle) => {
                self.draw_own_castle(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::RecruitSoldiers) => {
                self.draw_recruit_soldiers(list, layout, palette, resources, catalog, game, time)
            }
            Some(ViewKind::Dwelling) => {
                self.draw_dwelling(list, layout, palette, resources, catalog)
            }
            Some(ViewKind::WinCartoon) => {
                self.draw_win_cartoon(list, layout, palette, resources, catalog, time)
            }
            Some(ViewKind::Win) => self.draw_end_game(list, layout, palette, true),
            Some(ViewKind::Lose) => self.draw_end_game(list, layout, palette, false),
            None => {}
        }
    }

    /// Ports `draw_puzzle`: a 5x5 grid of villain faces and artifact icons
    /// over the scepter's terrain, each cell flipping from its cover to the
    /// revealed tile as the corresponding entity is caught or found.
    fn draw_puzzle(
        &mut self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
        // The puzzle covers only the map area, not the sidebar.
        list.rect(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DGrey),
        );
        list.rect_lines(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DRed),
        );

        let cell_w = layout.map_w / 5;
        let cell_h = layout.map_h / 5;
        let grid_x = MAP_X;
        let grid_y = MAP_Y;

        // Load the scepter's continent once, and again if it changes.
        let scepter_zone = game.scepter.zone.as_str();
        if scepter_zone.is_empty() {
            self.puzzle_map = None;
            self.puzzle_zone.clear();
        } else if self.puzzle_zone != scepter_zone {
            match Map::load_zone(resources, scepter_zone) {
                Ok(map) => {
                    self.puzzle_map = Some(map);
                    self.puzzle_zone = scepter_zone.to_owned();
                }
                Err(_) => {
                    self.puzzle_map = None;
                    self.puzzle_zone.clear();
                }
            }
        }

        // Centre a 5x5 window on the scepter, clamped to the map bounds.
        let mut cam_x = game.scepter.x - 2;
        let mut cam_y = game.scepter.y - 2;
        if let Some(map) = &self.puzzle_map {
            cam_x = cam_x.clamp(0, map.width as i32 - 5);
            cam_y = cam_y.clamp(0, map.height as i32 - 5);
        }

        // Cells reveal one every 150ms: artifacts first, then villains, in
        // row-major order within each pass.
        let reveal_step = ((time - self.puzzle_open_time) / 0.150) as i32;
        let anim_tick = (time * 2.0) as i32;

        let mut seq = [[0i32; 5]; 5];
        {
            let mut n = 0;
            for row in 0..5 {
                for col in 0..5 {
                    if puzzle_grid_entity(row, col) < 0 {
                        seq[row as usize][col as usize] = n;
                        n += 1;
                    }
                }
            }
            for row in 0..5 {
                for col in 0..5 {
                    if puzzle_grid_entity(row, col) >= 0 {
                        seq[row as usize][col as usize] = n;
                        n += 1;
                    }
                }
            }
        }

        for row in 0..5 {
            for col in 0..5 {
                let id = puzzle_grid_entity(row, col);
                let x = grid_x + col * cell_w;
                let y = grid_y + row * cell_h;

                let (mut caught, face) = if id < 0 {
                    let artifact = (-id - 1) as usize;
                    (
                        game.artifacts.found[artifact],
                        Some(Sprite::ViewIcon(artifact)),
                    )
                } else {
                    let villain = id as usize;
                    (
                        game.contract.villains_caught[villain],
                        villain_sprite(catalog, id, anim_tick),
                    )
                };

                // The reveal animation holds the cover until the cell's turn.
                if reveal_step < seq[row as usize][col as usize] {
                    caught = false;
                }

                if caught {
                    // Revealed: the scepter terrain underneath, or black.
                    list.rect(Rect::new(x, y, cell_w, cell_h), palette.clr(Pal::Black));
                    if let Some(map) = &self.puzzle_map {
                        if let Some(tile) = map.tile_i32(cam_x + col, cam_y + row) {
                            if !tile.art.is_empty() {
                                list.blit(Sprite::Tile(tile.art), Dst::rect(x, y, cell_w, cell_h));
                            }
                        }
                    }
                } else if let Some(face) = face {
                    // Cover: the entity's face, or the grey panel when the
                    // pack names no art for it (the backend skips the blit).
                    list.blit(face, Dst::rect(x, y, cell_w, cell_h));
                } else {
                    list.rect(Rect::new(x, y, cell_w, cell_h), palette.clr(Pal::DGrey));
                }
            }
        }
    }

    /// Ports `draw_worldmap`: the whole continent scaled to fit the map panel,
    /// fog-gated (or fully revealed with the orb), town/castle markers and the
    /// blinking hero. The exit hint is the status bar's job, drawn by
    /// [`Chrome`](crate::chrome::Chrome).
    fn draw_worldmap(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        map: &Map,
        fog: &Fog,
        game: &Game,
        time: f64,
    ) {
        // draw_view_panel: the map area only, like the puzzle view.
        list.rect(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DGrey),
        );
        list.rect_lines(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DRed),
        );

        if map.width == 0 || map.height == 0 {
            return;
        }

        let reveal_all = worldmap_has_orb(game, resources) && self.worldmap_whole_map;

        // Largest integer tile size that fits the padded panel.
        let avail_w = layout.map_w - 2 * VIEW_PAD;
        let avail_h = layout.map_h - 2 * VIEW_PAD;
        let mut pix = (avail_w / map.width as i32).min(avail_h / map.height as i32);
        if pix < 1 {
            pix = 1;
        }
        let grid_w = pix * map.width as i32;
        let grid_h = pix * map.height as i32;
        let gx = MAP_X + (layout.map_w - grid_w) / 2;
        let gy = MAP_Y + (layout.map_h - grid_h) / 2;

        list.rect(Rect::new(gx, gy, grid_w, grid_h), palette.clr(Pal::Black));

        for y in 0..map.height as i32 {
            for x in 0..map.width as i32 {
                if !reveal_all && !fog.seen(x, y) {
                    continue;
                }
                let Some(tile) = map.tile_i32(x, y) else {
                    continue;
                };
                list.rect(
                    Rect::new(gx + x * pix, gy + y * pix, pix, pix),
                    terrain_minimap_color(&resources.colors, tile.terrain),
                );
            }
        }

        // Towns are green markers, castles red, for the current zone only and
        // fog-gated unless the orb has revealed the whole continent.
        for i in 0..resources.town_count() {
            let Some(town) = resources.town_by_index(i) else {
                continue;
            };
            if town.zone.as_str() != game.position.zone.as_str() {
                continue;
            }
            if town.x < 0 || town.y < 0 {
                continue;
            }
            if !reveal_all && !fog.seen(town.x, town.y) {
                continue;
            }
            list.rect(
                Rect::new(gx + town.x * pix, gy + town.y * pix, pix, pix),
                palette.clr(Pal::Green),
            );
        }
        for i in 0..resources.castle_count() {
            let Some(castle) = resources.castle_by_index(i) else {
                continue;
            };
            if castle.zone.as_str() != game.position.zone.as_str() {
                continue;
            }
            if castle.x < 0 || castle.y < 0 {
                continue;
            }
            if !reveal_all && !fog.seen(castle.x, castle.y) {
                continue;
            }
            list.rect(
                Rect::new(gx + castle.x * pix, gy + castle.y * pix, pix, pix),
                palette.clr(Pal::Red),
            );
        }

        // The hero blinks yellow/red at ~3 Hz.
        let blink = if ((time * 3.0) as i32) & 1 == 1 {
            palette.clr(Pal::Yellow)
        } else {
            palette.clr(Pal::Red)
        };
        list.rect(
            Rect::new(
                gx + game.position.x * pix,
                gy + game.position.y * pix,
                pix,
                pix,
            ),
            blink,
        );

        let label = format!("X={} Y={}", game.position.x, game.position.y);
        list.text(&label, gx, gy + grid_h + 2, palette.clr(Pal::White));
    }

    /// Ports `draw_controls`: the settings panel flush against the map's left
    /// edge so the live game stays visible to its right. The highlighted row
    /// is the cursor, the three audio rows gray out because the shell has no
    /// audio, and the modern-only Scale row is appended after the pack's.
    ///
    /// DIVERGENCE, deliberate: C's `draw_controls` compares the cursor against
    /// the CONTROL index (`vis_idx[k] == cursor`) while `main.c` stores a
    /// VISIBLE-row cursor, so a hidden control desyncs the highlight. This
    /// port treats the cursor as the visible row (`k == cursor`), which is what
    /// both the input loop and the row labels assume.
    fn draw_controls(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        game: &Game,
        present: &Present,
    ) {
        let vis = controls_visible(resources);
        if vis.is_empty() {
            return;
        }
        let mut cursor = self.controls_cursor;
        if cursor >= vis.len() as i32 {
            cursor = (vis.len() - 1) as i32;
        }
        if cursor < 0 {
            cursor = 0;
        }

        let rows = vis.len() + 1 + usize::from(layout.is_modern);
        let w = CTL_COLS * font::GLYPH_W + 2 * CTL_PAD;
        let h = rows as i32 * (font::GLYPH_H + 2) + 2 * CTL_PAD;
        let x = MAP_X;
        let y = MAP_Y;

        list.rounded_panel(
            Rect::new(x, y, w, h),
            palette.clr(Pal::DBlue),
            palette.clr(Pal::Yellow),
        );

        let tx = x + CTL_PAD;
        let mut ty = y + CTL_PAD;
        list.text(
            banner::text(resources, "controls", "title", DEFAULT_CTL_TITLE),
            tx,
            ty,
            palette.clr(Pal::Yellow),
        );
        ty += font::GLYPH_H + 2;

        let on = banner::text(resources, "controls", "on", DEFAULT_CTL_ON);
        let off = banner::text(resources, "controls", "off", DEFAULT_CTL_OFF);

        for (k, &index) in vis.iter().enumerate() {
            let Some(control) = resources.control_by_index(index) else {
                continue;
            };
            if index >= game.stats.options.len() {
                continue;
            }
            let is_selected = k as i32 == cursor;
            let disabled = controls_row_disabled(resources, index);
            let fg = if disabled {
                palette.clr(Pal::DGrey)
            } else if is_selected {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };

            let label = format!("{} {}", char::from(b'1' + k as u8), control.label.as_str());
            list.text(&label, tx, ty, fg);

            let value = game.stats.options[index];
            if control.kind.as_str() == "bool" {
                let text = if value == 1 { on } else { off };
                let tw = font::measure_width(text);
                list.text(text, x + w - CTL_PAD - tw, ty, fg);
            } else {
                let mut range = control.range;
                if range > 10 {
                    range = 10;
                }
                let sx = x + w - CTL_PAD - range * font::GLYPH_W;
                for n in 0..range {
                    let nc = if disabled {
                        palette.clr(Pal::DGrey)
                    } else if n == value {
                        palette.clr(Pal::Yellow)
                    } else {
                        palette.clr(Pal::White)
                    };
                    list.text(
                        &char::from(b'0' + n as u8).to_string(),
                        sx + n * font::GLYPH_W,
                        ty,
                        nc,
                    );
                }
            }
            ty += font::GLYPH_H + 2;
        }

        if layout.is_modern {
            let is_selected = vis.len() as i32 == cursor;
            let fg = if is_selected {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            let label = format!("{} Scale", char::from(b'1' + vis.len() as u8));
            list.text(&label, tx, ty, fg);

            let sc = present.scale_override();
            let value = if sc <= 0 {
                "Auto".to_owned()
            } else {
                format!("{sc}x")
            };
            let vw = font::measure_width(&value);
            list.text(&value, x + w - CTL_PAD - vw, ty, fg);
        }
    }

    /// Ports `draw_spells`: the two-column spellbook. Combat spells (0..6) on
    /// the left, adventuring spells (7..13) on the right, each row dimming
    /// when the hero holds no charge.
    fn draw_spells(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        game: &Game,
    ) {
        list.rect(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DGrey),
        );
        list.rect_lines(
            Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h),
            palette.clr(Pal::DRed),
        );

        let title = banner::text(resources, "spells_view", "title", DEFAULT_SV_TITLE);
        list.text_centered(
            title,
            MAP_X + layout.map_w / 2,
            MAP_Y + VIEW_PAD,
            palette.clr(Pal::Yellow),
        );

        let head_y = MAP_Y + VIEW_PAD + font::GLYPH_H + 4;
        let col_l = MAP_X + VIEW_PAD + 4;
        let col_r = MAP_X + layout.map_w / 2 + 4;
        list.text(
            banner::text(resources, "spells_view", "combat_col", DEFAULT_SV_COMBAT),
            col_l,
            head_y,
            palette.clr(Pal::Yellow),
        );
        list.text(
            banner::text(
                resources,
                "spells_view",
                "adventure_col",
                DEFAULT_SV_ADVENTURE,
            ),
            col_r,
            head_y,
            palette.clr(Pal::Yellow),
        );

        let row_y = head_y + font::GLYPH_H + 4;
        let row_h = font::GLYPH_H + 1;
        for i in 0..7 {
            let combat = resources.spell_by_index(i);
            let adventure = resources.spell_by_index(i + 7);
            let cc = game.spells.counts[i as usize];
            let ca = game.spells.counts[(i + 7) as usize];
            let lc = if cc > 0 {
                palette.clr(Pal::White)
            } else {
                palette.clr(Pal::DGrey)
            };
            let rc = if ca > 0 {
                palette.clr(Pal::White)
            } else {
                palette.clr(Pal::DGrey)
            };

            if let Some(spell) = combat {
                let label = format!(
                    "{:2} {} {}",
                    cc,
                    char::from(b'A' + i as u8),
                    spell.name.as_str()
                );
                list.text(&label, col_l, row_y + i * row_h, lc);
            }
            if let Some(spell) = adventure {
                let label = format!(
                    "{} {:<12} {:2}",
                    char::from(b'A' + i as u8),
                    spell.name.as_str(),
                    ca
                );
                list.text(&label, col_r, row_y + i * row_h, rc);
            }
        }
    }

    /// Ports `draw_gate`: a cursored, two-column list of gate destinations.
    /// The panel covers the map and the sidebar so long names fit.
    fn draw_gate(&self, list: &mut DrawList, layout: &Layout, palette: &Palette) {
        let vx = MAP_X;
        let vw = layout.map_w + layout.sidebar_w;
        let vy = MAP_Y;
        let vh = layout.map_h;
        list.rect(Rect::new(vx, vy, vw, vh), palette.clr(Pal::DGrey));
        list.rect_lines(Rect::new(vx, vy, vw, vh), palette.clr(Pal::DRed));

        let title = if self.gate_is_town {
            "Town Gate"
        } else {
            "Castle Gate"
        };
        list.text_centered(title, vx + vw / 2, vy + VIEW_PAD, palette.clr(Pal::Yellow));

        let n = self.gate_count;
        let cursor = self.gate_cursor;
        let left = (n + 1) / 2;

        let row_y = vy + VIEW_PAD + font::GLYPH_H + 6;
        let row_h = font::GLYPH_H + 2;
        let col_l = vx + VIEW_PAD + 2;
        let col_r = vx + vw / 2 + 2;

        for r in 0..left {
            for side in 0..2 {
                let index = if side == 0 { r } else { r + left };
                if index >= n {
                    continue;
                }
                let Some(destination) = self.gate_dest(index) else {
                    continue;
                };
                let x = if side == 0 { col_l } else { col_r };
                let y = row_y + r as i32 * row_h;
                let selected = index == cursor;
                let fg = if selected {
                    palette.clr(Pal::Yellow)
                } else {
                    palette.clr(Pal::White)
                };
                let key = destination
                    .name
                    .as_str()
                    .chars()
                    .next()
                    .unwrap_or(' ')
                    .to_ascii_uppercase();
                let short: String = destination
                    .name
                    .as_str()
                    .chars()
                    .take(GATE_NAME_COL)
                    .collect();
                let label = format!("{}{}) {}", if selected { ">" } else { " " }, key, short);
                list.text(&label, x, y, fg);
            }
        }

        list.text(
            "Arrows/letter select  Enter go  ESC cancel",
            vx + VIEW_PAD + 2,
            vy + vh - font::GLYPH_H - 3,
            palette.clr(Pal::Grey),
        );
    }

    /// Ports `draw_town`: the town backdrop with an animated troop, then the
    /// bottom action menu (header + rows), and the info overlay when one is
    /// showing.
    fn draw_town(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
        let name = &self.town_display_name;

        let troop_index = town_backdrop_troop(game, resources, name);
        draw_location_backdrop(
            list,
            palette,
            catalog,
            crate::draw::Backdrop::Town,
            troop_index,
            time,
            layout.map_h,
        );

        // Menu panel at the bottom of the map area.
        let row_h = font::GLYPH_H + 1;
        let pad = 4;
        let lines = 2 + TOWN_ROW_COUNT;
        let h = lines * row_h + 2 * pad + 4;
        let w = layout.map_w + layout.sidebar_w;
        let x = MAP_X;
        let y = MAP_Y + layout.map_h - h;

        list.rounded_panel(
            Rect::new(x, y, w, h),
            palette.clr(Pal::DBlue),
            palette.clr(Pal::Yellow),
        );

        let tx = x + pad;
        let mut ty = y + pad - row_h / 4 - row_h / 8;
        if ty < y + 1 {
            ty = y + 1;
        }

        let header = crate::interact::town_header_text(resources, name);
        list.text(&header, tx, ty, palette.clr(Pal::Yellow));
        ty += row_h;

        let gold = crate::interact::town_gold_text(resources, game.stats.gold);
        let gold_w = font::measure_width(&gold);
        list.text(&gold, x + w - gold_w - pad, ty, palette.clr(Pal::Yellow));
        ty += row_h + 2;

        for row in 0..TOWN_ROW_COUNT {
            let label = crate::interact::town_row_text(resources, game, &self.town_record_key, row);
            let selected = row == self.town_cursor;
            let fg = if selected {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            list.text(&label, tx, ty, fg);
            ty += row_h;
        }

        if self.town_info_active {
            // The info overlay covers the menu with the same panel shape. The
            // body is already line-broken by the banner templates, so it is
            // split on newlines and capped at the menu's line budget.
            list.rounded_panel(
                Rect::new(x, y, w, h),
                palette.clr(Pal::DBlue),
                palette.clr(Pal::Yellow),
            );
            let mut ty = y + pad;
            for line in self.town_info_body.split('\n').take(lines as usize) {
                list.text(line, tx, ty, palette.clr(Pal::White));
                ty += row_h;
            }
        }
    }

    /// Ports `screen_home_castle_draw`: the castle backdrop and the two-choice
    /// menu (Recruit Soldiers / Audience with the King).
    fn draw_home_castle(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
        let troop_index = home_castle_troop(game, resources);
        draw_location_backdrop(
            list,
            palette,
            catalog,
            crate::draw::Backdrop::Castle,
            troop_index,
            time,
            layout.map_h,
        );

        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let body = crate::interact::home_castle_body(resources);
        let row_h = font::GLYPH_H + 1;
        let mut ty = y + PANEL_PAD_X;
        for line in body.split('\n') {
            if ty + row_h > y + h - PANEL_PAD_X {
                break;
            }
            list.text(line, x + PANEL_PAD_X, ty, palette.clr(Pal::White));
            ty += row_h;
        }
    }

    /// Ports `screen_own_castle_draw`: the castle backdrop and the garrison /
    /// remove list with the mode subtitle.
    fn draw_own_castle(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
        let troop_index = own_castle_troop(game, resources, &self.own_castle_id);
        draw_location_backdrop(
            list,
            palette,
            catalog,
            crate::draw::Backdrop::Castle,
            troop_index,
            time,
            layout.map_h,
        );

        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let row_h = font::GLYPH_H + 1;
        let tx = x + PANEL_PAD_X;
        let mut ty = y + PANEL_PAD_X;

        let castle_name = resources
            .castle_by_id(self.own_castle_id.as_str())
            .map(|castle| {
                if castle.name.is_empty() {
                    castle.id.as_str()
                } else {
                    castle.name.as_str()
                }
            })
            .unwrap_or(self.own_castle_id.as_str());
        list.text(
            &format!("Castle {castle_name}"),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        ty += row_h;

        let mode = crate::interact::own_castle_mode_label(resources, self.own_castle_garrison_mode);
        list.text(&mode, tx, ty, palette.clr(Pal::White));
        ty += row_h + 1;

        for slot in 0..GAME_ARMY_SLOTS {
            let line = crate::interact::own_castle_row(
                resources,
                game,
                &self.own_castle_id,
                self.own_castle_garrison_mode,
                slot,
            );
            list.text(&line, tx, ty, palette.clr(Pal::White));
            ty += row_h;
        }
    }

    /// Ports `screen_recruit_soldiers_draw`: the castle backdrop and the troop
    /// list with the inline count entry on the right.
    fn draw_recruit_soldiers(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
        draw_location_backdrop(
            list,
            palette,
            catalog,
            crate::draw::Backdrop::Castle,
            self.recruit_anim_troop,
            time,
            layout.map_h,
        );

        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let row_h = font::GLYPH_H + 1;
        let tx = x + PANEL_PAD_X;
        let ty = y + PANEL_PAD_X;

        // An error popup replaces the panel content (3 leading blank rows for
        // the gold banner's own padding).
        if !self.recruit_error.is_empty() {
            list.text_centered(
                &self.recruit_error,
                x + w / 2,
                ty + 3 * row_h,
                palette.clr(Pal::White),
            );
            return;
        }

        let title = crate::interact::recruit_soldiers_title(resources);
        list.text(&title, tx, ty, palette.clr(Pal::White));

        let troop_ty = ty + row_h + 1;
        let total_lead = game.stats.leadership_current;
        for i in 0..self.recruit_pool_count {
            let Some(troop) = resources.troop_by_index(self.recruit_pool[i]) else {
                continue;
            };
            let unreachable = troop.hit_points <= 0 || total_lead < troop.hit_points * 6;
            let label = if unreachable {
                format!(
                    "{}) {:<11}n/a",
                    char::from(b'A' + i as u8),
                    troop.name.as_str()
                )
            } else {
                format!(
                    "{}) {:<11}{}",
                    char::from(b'A' + i as u8),
                    troop.name.as_str(),
                    troop.recruit_cost
                )
            };
            list.text(
                &label,
                tx,
                troop_ty + i as i32 * row_h,
                palette.clr(Pal::White),
            );
        }

        // Right column.
        let rx = tx + 20 * font::GLYPH_W;
        let gp = format!("GP={}K", game.stats.gold / 1000);
        list.text(&gp, rx, ty, palette.clr(Pal::White));

        let rby = ty + row_h + 1 + row_h;
        list.text("(A-C) ", rx, rby, palette.clr(Pal::White));
        let after_hint_x = rx + 6 * font::GLYPH_W;

        if self.recruit_whom == 0 {
            // The spinner twirl (the `|` glyph as a stable stand-in; the real
            // four-frame twirl is a cosmetic tick the tests ignore).
            list.text("|", after_hint_x, rby, palette.clr(Pal::White));
        } else {
            let letter = char::from(b'A' + (self.recruit_whom - 1) as u8);
            list.text(
                &letter.to_string(),
                after_hint_x,
                rby,
                palette.clr(Pal::White),
            );
            list.text(
                &format!("Max={}", self.recruit_max),
                rx,
                rby + row_h,
                palette.clr(Pal::White),
            );
            list.text(
                &crate::interact::recruit_soldiers_how_many(resources),
                rx,
                rby + 2 * row_h,
                palette.clr(Pal::White),
            );
            list.text(
                &format!("{}_", self.recruit_input),
                rx,
                rby + 3 * row_h,
                palette.clr(Pal::White),
            );
        }
    }

    /// Ports `screen_dwelling_draw`: the kind-specific backdrop and the info
    /// panel. The "how many" entry itself is the modal prompt's.
    fn draw_dwelling(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
    ) {
        // Dwellings never animate their troop (C hardcodes frame 0).
        draw_location_backdrop(
            list,
            palette,
            catalog,
            dwelling_backdrop(self.dwelling_kind),
            self.dwelling_troop_index,
            0.0,
            layout.map_h,
        );

        let x = layout.panel_x();
        let y = layout.panel_y();
        let w = layout.panel_w();
        let h = PANEL_H;
        list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
        list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));

        let row_h = font::GLYPH_H + 1;
        let tx = x + PANEL_PAD_X;
        let mut ty = y + PANEL_PAD_X;

        let title = crate::interact::dwelling_kind_name(resources, self.dwelling_kind);
        list.text_centered(&title, x + w / 2, ty, palette.clr(Pal::White));
        ty += row_h;
        let ruler = "-".repeat(title.len().min(28));
        list.text_centered(&ruler, x + w / 2, ty, palette.clr(Pal::White));
        ty += row_h;

        let name = if self.dwelling_troop_name.is_empty() {
            "Troops"
        } else {
            &self.dwelling_troop_name
        };
        list.text(
            &format!("{} {} are available", self.dwelling_pop, name),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        ty += row_h;

        list.text(
            &format!("Cost={} each.", self.dwelling_cost),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        list.text(
            &format!("GP={}K", self.dwelling_gold / 1000),
            tx + 20 * font::GLYPH_W,
            ty,
            palette.clr(Pal::White),
        );
        ty += row_h;

        list.text(
            &format!("You may recruit up to {}", self.dwelling_cap),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        ty += row_h;

        list.text(
            &crate::interact::dwelling_how_many(resources),
            tx,
            ty,
            palette.clr(Pal::White),
        );
    }
}

/// Whether the hero owns the crystal orb for the current continent. Ports
/// `worldmap_has_orb`, including its `zi < GAME_CONTINENTS` guard.
pub fn worldmap_has_orb(game: &Game, resources: &Resources) -> bool {
    let Some(zone_index) = resources.zone_index_by_id(game.position.zone.as_str()) else {
        return false;
    };
    if zone_index >= GAME_CONTINENTS {
        return false;
    }
    game.world.orbs_found[zone_index]
}

/// A terrain's minimap colour from the pack's packed-ARGB `colors` block,
/// porting `terrain_minimap_color`. The no-colour fallback branch is dead in
/// Rust: `Resources.colors` is a value, not an optional pointer.
fn terrain_minimap_color(colors: &ResColors, terrain: Terrain) -> Color {
    let packed = match terrain {
        Terrain::Grass => colors.minimap_grass,
        Terrain::Forest => colors.minimap_forest,
        Terrain::Mountain => colors.minimap_mountain,
        Terrain::Water => colors.minimap_water,
        Terrain::Desert => colors.minimap_desert,
    };
    Color::from_argb(packed)
}

/// The control indices shown by the settings panel, in presentation order:
/// every non-hidden setting, capped at eight. Ports the `vis_idx` build in
/// `draw_controls` and `main.c`.
pub fn controls_visible(resources: &Resources) -> Vec<usize> {
    let mut visible = Vec::new();
    for index in 0..resources.controls_count() {
        if visible.len() >= RES_MAX_CONTROLS {
            break;
        }
        let Some(control) = resources.control_by_index(index) else {
            continue;
        };
        if control.hidden {
            continue;
        }
        visible.push(index);
    }
    visible
}

/// The `O` options screen: a fixed movement reference followed by the pack's
/// keybind list, porting `draw_options`. Read-only -- any key dismisses it via
/// the caller. The list is capped to the rows that fit, and the three
/// mount-conditional entries are hidden when they do not apply.
/// The cartoon's `(display tick, frame)` at `elapsed` seconds, porting
/// `run_end_cartoon`: a tick every `CARTOON_TICK`, a frame every
/// `ticks_per_step` ticks, the display tick wrapping at 4.
fn cartoon_clock(resources: &Resources, elapsed: f64) -> (i32, i32) {
    let ticks_per_step = if resources.ending.ticks_per_step > 0 {
        resources.ending.ticks_per_step
    } else {
        2
    };
    let total = (elapsed.max(0.0) / CARTOON_TICK) as i32;
    (total % 4, total / ticks_per_step)
}

/// The sprite for one cartoon troop cell, animating when the pack ships a
/// cycle and falling back to the still otherwise.
fn troop_cartoon_sprite(catalog: &SpriteCatalog, troop: usize, tick: i32) -> Sprite {
    let frames = catalog.troop_anim_len(troop);
    if frames > 0 {
        Sprite::TroopAnim {
            troop,
            frame: crate::sprites::frame_index(tick, frames) as i32,
        }
    } else {
        Sprite::TroopStill(troop)
    }
}

fn draw_options(
    list: &mut DrawList,
    _layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    game: &Game,
) {
    const MOVEMENT: [(&str, &str); 8] = [
        ("\x18 or 2", "Move Down"),
        ("\x1B or 4", "Move Left"),
        ("\x1A or 6", "Move Right"),
        ("\x19 or 8", "Move Up"),
        ("END or 1", "Down Left"),
        ("PGDN or 3", "Down Right"),
        ("HOME or 7", "Up Left"),
        ("PGUP or 9", "Up Right"),
    ];

    let pad = 3;
    let cols = 28;
    let rows = 19;
    let w = cols * font::GLYPH_W + 2 * pad;
    let h = rows * font::GLYPH_H + 2 * pad;
    let x = MAP_X;
    let y = MAP_Y;
    list.rounded_panel(
        Rect::new(x, y, w, h),
        palette.clr(Pal::DBlue),
        palette.clr(Pal::Yellow),
    );

    let tx = x + pad;
    let mut ty = y + pad;

    for (keys, label) in MOVEMENT {
        list.text(
            &format!("{keys:<10} {label}"),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        ty += font::GLYPH_H;
    }

    let max_row = ((y + h - pad - ty) / font::GLYPH_H).max(0) as usize;
    let count = resources.keybind_count.min(max_row);
    for index in 0..count {
        let keybind = &resources.keybinds[index];
        let key = keybind.key.as_str();
        if key == "F" && game.character.mount == Mount::Fly {
            continue;
        }
        if key == "L" && game.character.mount != Mount::Fly {
            continue;
        }
        if key == "N" && game.character.mount != Mount::Sail {
            continue;
        }
        list.text(
            &format!("{key:<4} {}", keybind.label.as_str()),
            tx,
            ty,
            palette.clr(Pal::White),
        );
        ty += font::GLYPH_H;
    }
}

/// The shell ships no audio, so the three audio-dependent rows are always
/// grayed out, exactly as C grays them when its audio init fails.
fn audio_available() -> bool {
    false
}

/// True for the rows labelled Sounds / Music / Volume, C's
/// `controls_row_is_audio`.
fn controls_row_is_audio(resources: &Resources, row: usize) -> bool {
    let Some(control) = resources.control_by_index(row) else {
        return false;
    };
    matches!(control.label.as_str(), "Sounds" | "Music" | "Volume")
}

/// True when a row cannot be changed, C's `views_controls_row_disabled`.
fn controls_row_disabled(resources: &Resources, row: usize) -> bool {
    controls_row_is_audio(resources, row) && !audio_available()
}

fn stat_label<'a>(resources: &'a Resources, key: &str, default: &'a str) -> &'a str {
    banner::text(resources, "stats", key, default)
}

fn stat_row(
    list: &mut DrawList,
    label: &str,
    count: i32,
    label_x: i32,
    val_right: i32,
    y: &mut i32,
    white: Color,
) {
    list.text(label, label_x, *y, white);
    // A zero value draws no number, matching C.
    if count != 0 {
        let value = count.to_string();
        list.text(&value, val_right - font::measure_width(&value), *y, white);
    }
    *y += font::GLYPH_H;
}

/// Ports `draw_character`: portrait on the left, stat rows on the right, and
/// the artifact/map belt below.
fn draw_character(
    list: &mut DrawList,
    layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    game: &Game,
) {
    let vx = MAP_X;
    let vw = layout.map_w + layout.sidebar_w;
    let view_y = MAP_Y;
    let view_h = layout.map_h;

    // Full content width (covers the sidebar), solid grey background.
    list.rect(Rect::new(vx, view_y, vw, view_h), palette.clr(Pal::DGrey));

    if let Some(cls) = resources.class_by_id(game.character.cls.id.as_str()) {
        list.blit(
            Sprite::ClassPortrait(cls.index as usize),
            Dst::rect(vx, view_y, PORTRAIT_W, PORTRAIT_H),
        );
    }

    let sx = vx + PORTRAIT_W;
    let label_x = sx + 1;
    let val_right = vx + vw - 2;
    let white = palette.clr(Pal::White);
    let mut y = view_y + 1;

    // Name row: "Sir Rust the General".
    let name_row = format!(
        "{} the {}",
        game.character.name.as_str(),
        game.character.cls.rank_title.as_str()
    );
    list.text(&name_row, label_x, y, white);
    y += font::GLYPH_H;

    stat_row(
        list,
        stat_label(resources, "leadership", "Leadership"),
        game.stats.leadership_current,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "commission", "Commission/Week"),
        game.stats.commission_weekly,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "gold", "Gold"),
        game.stats.gold,
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "spell_power", "Spell power"),
        game.stats.spell_power,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "max_spells", "Max # of spells"),
        game.stats.max_spells,
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "villains_caught", "Villains caught"),
        game.villains_caught_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "artifacts_found", "Artifacts found"),
        game.artifacts_found_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    y += 4;
    stat_row(
        list,
        stat_label(resources, "castles_garrisoned", "Castles garrisoned"),
        game.castles_owned_count(),
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "followers_killed", "Followers killed"),
        game.stats.followers_killed,
        label_x,
        val_right,
        &mut y,
        white,
    );
    stat_row(
        list,
        stat_label(resources, "current_score", "Current score"),
        game.compute_score(resources),
        label_x,
        val_right,
        &mut y,
        white,
    );

    // Inventory belt: 6 columns (4 artifacts + 2 zone maps) by 2 rows.
    let inv_x = vx;
    let inv_y = view_y + PORTRAIT_H;
    let item_w = vw / 6;
    let item_h = layout.tile_h;
    let belt_h = item_h * 2;

    list.rect(Rect::new(inv_x, inv_y, vw, belt_h), palette.clr(Pal::DRed));
    list.rect_lines(Rect::new(inv_x, inv_y, vw, belt_h), palette.clr(Pal::Grey));
    for col in 1..6 {
        list.rect(
            Rect::new(inv_x + col * item_w, inv_y, 1, belt_h),
            palette.clr(Pal::Grey),
        );
    }
    list.rect(
        Rect::new(inv_x, inv_y + item_h, vw, 1),
        palette.clr(Pal::Grey),
    );

    for index in 0..8 {
        if !game.artifacts.found[index] {
            continue;
        }
        let ix = inv_x + (index % 4) as i32 * item_w;
        let iy = inv_y + (index / 4) as i32 * item_h;
        list.blit(Sprite::ViewIcon(index), Dst::rect(ix, iy, item_w, item_h));
    }

    // Zone maps sit in columns 4 and 5, only when the zone is discovered.
    let map_x = inv_x + 4 * item_w;
    for index in 0..4 {
        if index >= game.world.zones_discovered.len() || !game.world.zones_discovered[index] {
            continue;
        }
        let ix = map_x + (index % 2) as i32 * item_w;
        let iy = inv_y + (index / 2) as i32 * item_h;
        list.blit(
            Sprite::ViewIcon(8 + index),
            Dst::rect(ix, iy, item_w, item_h),
        );
    }
}

/// A troop's animated frame, falling back to its still sprite. Mirrors the
/// helper `map_render` uses, since the army view animates the same way.
pub(crate) fn troop_sprite(catalog: &SpriteCatalog, index: i32, tick: i32) -> Option<Sprite> {
    let index = usize::try_from(index).ok()?;
    let frames = catalog.troop_anim_len(index);
    if frames > 0 {
        return Some(Sprite::TroopAnim {
            troop: index,
            frame: crate::sprites::frame_index(tick, frames) as i32,
        });
    }
    catalog
        .troop_frame(index, tick)
        .map(|_| Sprite::TroopStill(index))
}

/// Deterministically pick which castle-dwelling troop idles on a town backdrop,
/// porting `town_backdrop_troop`: the pool is limited to troops dwelling at
/// castles, hashed by `(seed, town key)`.
fn town_backdrop_troop(game: &Game, resources: &Resources, key: &str) -> i32 {
    let troop_count = resources.troops_count();
    let mut pool = [0i32; 32];
    let mut npool = 0;
    for index in 0..troop_count.min(32) {
        if let Some(troop) = resources.troop_by_index(index as i32) {
            if troop.dwelling.as_str() == "castle" {
                pool[npool] = index as i32;
                npool += 1;
            }
        }
    }

    let hash = |key: &str| -> u64 {
        let mut h = game.seed ^ 0xA1B2C3;
        for byte in key.bytes() {
            h = h.wrapping_mul(131).wrapping_add(byte as u64);
        }
        h
    };

    if npool < 1 {
        if troop_count < 1 {
            return 0;
        }
        return (hash(key) % troop_count as u64) as i32;
    }
    pool[(hash(key) % npool as u64) as usize]
}

/// Draw a 240x102 location backdrop with an animated troop and the black fill
/// below it, porting `draw_location_backdrop`.
fn draw_location_backdrop(
    list: &mut DrawList,
    palette: &Palette,
    catalog: &SpriteCatalog,
    backdrop: crate::draw::Backdrop,
    troop_index: i32,
    time: f64,
    map_h: i32,
) {
    list.rect(Rect::new(MAP_X, MAP_Y, 240, 102), palette.clr(Pal::Black));
    list.blit(
        Sprite::Backdrop(backdrop),
        Dst::rect(MAP_X, MAP_Y, 240, 102),
    );

    let tick = (time * 6.66) as i32;
    if let Some(sprite) = troop_sprite(catalog, troop_index, tick) {
        list.blit(sprite, Dst::rect(MAP_X + 48, MAP_Y + 102 - 34 - 4, 48, 34));
    }

    let below_y = MAP_Y + 102;
    if below_y < MAP_Y + map_h {
        list.rect(
            Rect::new(MAP_X, below_y, 240, MAP_Y + map_h - below_y),
            palette.clr(Pal::Black),
        );
    }
}

/// The castle-class troop that idles on the home-castle backdrop, porting
/// `pick_castle_troop` in `home_castle.c`.
fn home_castle_troop(game: &Game, resources: &Resources) -> i32 {
    let troop_count = resources.troops_count();
    let mut pool = [0i32; 8];
    let mut npool = 0;
    for index in 0..troop_count.min(8) {
        if let Some(troop) = resources.troop_by_index(index as i32) {
            if troop.dwelling.as_str() == "castle" {
                pool[npool] = index as i32;
                npool += 1;
            }
        }
    }
    if npool < 1 {
        return -1;
    }
    let mut h = game.seed ^ 0xC451_E11A;
    h ^= game.stats.days_left as u64;
    h = h.wrapping_mul(2_654_435_761).wrapping_add(0x9E37_79B9);
    pool[(h % npool as u64) as usize]
}

/// The castle-class troop that idles on an own-castle backdrop, porting
/// `pick_castle_troop` in `own_castle.c`.
fn own_castle_troop(game: &Game, resources: &Resources, castle_id: &str) -> i32 {
    let troop_count = resources.troops_count();
    let mut pool = [0i32; 8];
    let mut npool = 0;
    for index in 0..troop_count.min(8) {
        if let Some(troop) = resources.troop_by_index(index as i32) {
            if troop.dwelling.as_str() == "castle" {
                pool[npool] = index as i32;
                npool += 1;
            }
        }
    }
    if npool < 1 {
        return -1;
    }
    let mut h = game.seed ^ 0x0CA5_71E5;
    for byte in castle_id.bytes() {
        h = h.wrapping_mul(131).wrapping_add(byte as u64);
    }
    pool[(h % npool as u64) as usize]
}

/// The backdrop a dwelling kind draws, porting `loc_kind_for`.
fn dwelling_backdrop(kind: Interact) -> crate::draw::Backdrop {
    match kind {
        Interact::DwellingForest => crate::draw::Backdrop::Forest,
        Interact::DwellingHills => crate::draw::Backdrop::HillCave,
        Interact::DwellingDungeon => crate::draw::Backdrop::Dungeon,
        _ => crate::draw::Backdrop::Plains,
    }
}

/// Morale of one army slot: any 'L' against another stack reads Low, all 'H'
/// reads High, a lone stack reads High, else Normal. Ports `army_slot_morale`.
fn army_slot_morale<'a>(
    resources: &'a Resources,
    game: &Game,
    slot: usize,
    normal: &'a str,
    low: &'a str,
    high: &'a str,
) -> &'a str {
    let Some(me) = resources.troop_by_id(game.army[slot].id.as_str()) else {
        return normal;
    };
    let mut others = 0;
    let mut low_count = 0;
    let mut high_count = 0;
    for other in 0..GAME_ARMY_SLOTS {
        if other == slot {
            continue;
        }
        if game.army[other].id.is_empty() || game.army[other].count == 0 {
            continue;
        }
        let Some(other_troop) = resources.troop_by_id(game.army[other].id.as_str()) else {
            continue;
        };
        others += 1;
        match resources.morale_result(me.morale_group, other_troop.morale_group) {
            b'L' => low_count += 1,
            b'H' => high_count += 1,
            _ => {}
        }
    }
    if others == 0 {
        high
    } else if low_count > 0 {
        low
    } else if high_count == others {
        high
    } else {
        normal
    }
}

/// Ports `draw_army`: five rows, one per troop stack, with a sprite, the
/// roster text, and two columns of stats.
fn draw_army(
    list: &mut DrawList,
    layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    catalog: &SpriteCatalog,
    game: &Game,
    time: f64,
) {
    // Troops tick-animate at ~8 Hz, matching C's `GetTime() * 8.0`.
    let tick = (time * 8.0) as i32;
    let vx = MAP_X;
    let vw = layout.map_w + layout.sidebar_w;
    let view_y = MAP_Y;
    let view_h = layout.map_h;

    list.rect(Rect::new(vx, view_y, vw, view_h), palette.clr(Pal::DGrey));
    list.rect_lines(Rect::new(vx, view_y, vw, view_h), palette.clr(Pal::DRed));

    let row_h = 34;
    let pad = 2;
    let sprite_w = 48;
    let sprite_h = 34;
    let white = palette.clr(Pal::White);

    let skill = banner::text(resources, "army_view", "skill", "SL:");
    let move_lbl = banner::text(resources, "army_view", "move", "MV:");
    let morale_lbl = banner::text(resources, "army_view", "morale", "Morale:");
    let hp_lbl = banner::text(resources, "army_view", "hit_points", "HitPts:");
    let damage_lbl = banner::text(resources, "army_view", "damage", "Damage:");
    let cost_lbl = banner::text(resources, "army_view", "g_cost", "G-Cost:");
    let out_of_control = banner::text(resources, "ui", "out_of_control", "OUT OF CONTROL");
    let morale_normal = banner::text(resources, "morale", "normal", "Normal");
    let morale_low = banner::text(resources, "morale", "low", "Low");
    let morale_high = banner::text(resources, "morale", "high", "High");

    for slot in 0..GAME_ARMY_SLOTS {
        let ry = view_y + pad + slot as i32 * row_h;

        // The green sprite cell is drawn even for an empty slot.
        list.rect(
            Rect::new(vx + pad, ry, sprite_w, sprite_h),
            palette.clr(Pal::DGreen),
        );

        if game.army[slot].id.is_empty() || game.army[slot].count == 0 {
            continue;
        }
        let Some(troop) = resources.troop_by_id(game.army[slot].id.as_str()) else {
            continue;
        };

        if let Some(sprite) = troop_sprite(catalog, troop.index, tick) {
            list.blit(sprite, Dst::rect(vx + pad, ry, sprite_w, sprite_h));
        }

        // Left column: count+name, skill+move, morale.
        let tx = vx + pad + sprite_w + 4;
        let ty = ry + 2;
        list.text(
            &format!("{:3} {}", game.army[slot].count, troop.name.as_str()),
            tx,
            ty,
            white,
        );
        list.text(
            &format!(
                "{}{:2} {}{:2}",
                skill, troop.skill_level, move_lbl, troop.move_rate
            ),
            tx,
            ty + font::GLYPH_H,
            white,
        );

        // A stack whose hit points exceed leadership is out of control.
        let free_lead = game.stats.leadership_current - troop.hit_points * game.army[slot].count;
        let (morale_text, morale_color) = if free_lead < 0 {
            (out_of_control.to_owned(), palette.clr(Pal::Red))
        } else {
            let label = army_slot_morale(
                resources,
                game,
                slot,
                morale_normal,
                morale_low,
                morale_high,
            );
            (format!("{}{}", morale_lbl, label), white)
        };
        list.text(&morale_text, tx, ty + font::GLYPH_H * 2, morale_color);

        // Right column: hit points, damage range, gold cost.
        let rx = tx + 16 * font::GLYPH_W;
        list.text(
            &format!("{}{}", hp_lbl, troop.hit_points * game.army[slot].count),
            rx,
            ty,
            white,
        );
        list.text(
            &format!(
                "{}{}-{}",
                damage_lbl,
                troop.melee_min * game.army[slot].count,
                troop.melee_max * game.army[slot].count
            ),
            rx,
            ty + font::GLYPH_H,
            white,
        );
        list.text(
            &format!(
                "{}{}",
                cost_lbl,
                (troop.recruit_cost / 10) * game.army[slot].count
            ),
            rx,
            ty + font::GLYPH_H * 2,
            white,
        );

        // A thin rule between occupied rows.
        if slot < GAME_ARMY_SLOTS - 1 {
            list.rect(
                Rect::new(vx + pad, ry + row_h - 1, vw - 2 * pad, 1),
                palette.clr(Pal::DRed),
            );
        }
    }
}

/// A villain's wanted-poster face: the animated frame when the pack declares
/// one, else the still portrait.
fn villain_sprite(catalog: &SpriteCatalog, index: i32, tick: i32) -> Option<Sprite> {
    let index = usize::try_from(index).ok()?;
    let frames = catalog.villain_anim_len(index);
    if frames > 0 {
        return Some(Sprite::VillainAnim {
            villain: index,
            frame: crate::sprites::frame_index(tick, frames) as i32,
        });
    }
    if catalog.path(Sprite::VillainPortrait(index)).is_some() {
        Some(Sprite::VillainPortrait(index))
    } else {
        None
    }
}

/// Draw a multi-line block, respecting embedded newlines, returning the y just
/// past the last line. Ports `draw_text_block`.
fn draw_text_block(list: &mut DrawList, text: &str, x: i32, y: i32, color: Color) -> i32 {
    let mut ly = y;
    for line in text.split('\n') {
        list.text(line, x, ly, color);
        ly += font::GLYPH_H;
    }
    ly
}

/// Ports `draw_contract`: a rounded blue panel with the villain's face and a
/// wanted-poster description, or a "no contract" placeholder.
fn draw_contract(
    list: &mut DrawList,
    layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    catalog: &SpriteCatalog,
    game: &Game,
    time: f64,
) {
    let panel_x = MAP_X + 2;
    let panel_y = MAP_Y + layout.tile_h;
    let panel_w = layout.map_w + layout.sidebar_w - 4;
    let pad = 4;
    let white = palette.clr(Pal::White);
    let yellow = palette.clr(Pal::Yellow);

    let cv_title_no_contract = banner::text(
        resources,
        "contract_view",
        "title_no_contract",
        DEFAULT_CV_TITLE_NO_CONTRACT,
    );

    if game.contract.active_id.is_empty() {
        let panel_h = layout.tile_h * 3;
        list.rounded_panel(
            Rect::new(panel_x, panel_y, panel_w, panel_h),
            palette.clr(Pal::DBlue),
            yellow,
        );
        let tx = panel_x + pad;
        let ty = panel_y + pad;
        // Silhouette box with a yellow outline.
        list.rect_lines(Rect::new(tx - 1, ty - 1, FACE_W + 2, FACE_H + 2), yellow);
        list.blit(
            Sprite::HudContractSilhouette,
            Dst::rect(tx, ty, FACE_W, FACE_H),
        );
        list.text_centered(
            cv_title_no_contract,
            panel_x + panel_w / 2,
            panel_y + panel_h / 2 - font::GLYPH_H / 2,
            white,
        );
        return;
    }

    let Some(villain) = resources.villain_by_id(game.contract.active_id.as_str()) else {
        return;
    };

    let panel_h = layout.tile_h * 4;
    list.rounded_panel(
        Rect::new(panel_x, panel_y, panel_w, panel_h),
        palette.clr(Pal::DBlue),
        yellow,
    );

    let tx = panel_x + pad;
    let ty = panel_y + pad;

    // Villain face, animated at ~2 Hz, with a yellow outline.
    let tick = (time * 2.0) as i32;
    if let Some(sprite) = villain_sprite(catalog, villain.index, tick) {
        list.blit(sprite, Dst::rect(tx, ty, FACE_W, FACE_H));
    }
    list.rect_lines(Rect::new(tx - 1, ty - 1, FACE_W + 2, FACE_H + 2), yellow);

    let desc = resources.villain_desc(villain.id.as_str());
    let alias_none = banner::text(
        resources,
        "contract_view",
        "alias_none",
        DEFAULT_CV_ALIAS_NONE,
    );
    let castle_unknown = banner::text(
        resources,
        "contract_view",
        "castle_unknown",
        DEFAULT_CV_CASTLE_UNKNOWN,
    );

    let mut sy = ty;
    let sx = tx + FACE_W + font::GLYPH_W;

    // Header band, all in yellow.
    let name_line = banner::formatted(
        resources,
        "contract_view",
        "label_name",
        DEFAULT_CV_LABEL_NAME,
        &[("VALUE", villain.name.as_str())],
    );
    list.text(&name_line, sx, sy, yellow);
    sy += font::GLYPH_H;

    let alias = desc
        .filter(|desc| !desc.alias.is_empty())
        .map(|desc| desc.alias.as_str())
        .unwrap_or(alias_none);
    let alias_line = banner::formatted(
        resources,
        "contract_view",
        "label_alias",
        DEFAULT_CV_LABEL_ALIAS,
        &[("VALUE", alias)],
    );
    list.text(&alias_line, sx, sy, yellow);
    sy += font::GLYPH_H;

    let reward_line = banner::formatted(
        resources,
        "contract_view",
        "label_reward",
        DEFAULT_CV_LABEL_REWARD,
        &[("VALUE", &villain.reward.to_string())],
    );
    list.text(&reward_line, sx, sy, yellow);
    sy += font::GLYPH_H;

    // Last Seen is the villain's home zone name, falling back to its id.
    let zone_label = resources
        .zone_by_id(villain.zone.as_str())
        .filter(|zone| !zone.name.is_empty())
        .map(|zone| zone.name.as_str())
        .unwrap_or(villain.zone.as_str());
    let last_seen_line = banner::formatted(
        resources,
        "contract_view",
        "label_last_seen",
        DEFAULT_CV_LABEL_LAST_SEEN,
        &[("VALUE", zone_label)],
    );
    list.text(&last_seen_line, sx, sy, yellow);
    sy += font::GLYPH_H;

    // Castle: the known villain-owned castle with a matching villain id.
    let mut castle_name = castle_unknown;
    for castle in &game.castles[..GAME_CASTLES] {
        if castle.id.is_empty() {
            continue;
        }
        if castle.owner_kind != CastleOwnerKind::Villain {
            continue;
        }
        if castle.villain_id.as_str() != villain.id.as_str() {
            continue;
        }
        if castle.known {
            castle_name = castle.id.as_str();
        }
        break;
    }
    let castle_line = banner::formatted(
        resources,
        "contract_view",
        "label_castle",
        DEFAULT_CV_LABEL_CASTLE,
        &[("VALUE", castle_name)],
    );
    list.text(&castle_line, sx, sy, yellow);
    sy += font::GLYPH_H + 2;

    // Features block.
    if let Some(desc) = desc.filter(|desc| !desc.features.is_empty()) {
        let header = banner::text(
            resources,
            "contract_view",
            "features_header",
            DEFAULT_CV_FEATURES_HEADER,
        );
        list.text(header, panel_x + pad, sy, yellow);
        sy += font::GLYPH_H;
        sy = draw_text_block(list, desc.features.as_str(), panel_x + pad, sy, white);
        sy += 2;
    }

    // Crimes block.
    if let Some(desc) = desc.filter(|desc| !desc.crimes.is_empty()) {
        let header = banner::text(
            resources,
            "contract_view",
            "crimes_header",
            DEFAULT_CV_CRIMES_HEADER,
        );
        list.text(header, panel_x + pad, sy, yellow);
        sy += font::GLYPH_H;
        draw_text_block(list, desc.crimes.as_str(), panel_x + pad, sy, white);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
    use crate::input::Keys;
    use openbounty_engine::map::Tile;

    /// A trivial map and fog for views that ignore them; only the worldmap
    /// reads these, and its tests build a real one.
    fn map_and_fog() -> (Map, Fog) {
        let map = Map::from_tiles(1, 1, vec![Tile::plain_grass()]).expect("1x1 map");
        (map, Fog::new())
    }

    fn fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "classes": [
                  {"index":0,"id":"knight","name":"Knight","portrait":"knight.png",
                   "ranks":[{"id":"general","name":"General"}]}
                ],
                "artifacts": [
                  {"index":0,"id":"sword","name":"Sword","icon":"sword.png","power":"increased_damage","effect":"e"}
                ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    fn knight_game() -> Game {
        let mut game = Game::default();
        game.character.cls.id.copy_from_str("knight");
        game.character.cls.rank_title.copy_from_str("General");
        game.character.name.copy_from_str("Sir Rust");
        game.stats.leadership_current = 100;
        game.stats.commission_weekly = 20;
        game.stats.gold = 7500;
        game
    }

    #[test]
    fn the_stack_replaces_and_pops() {
        let mut views = Views::new();
        assert!(!views.is_active());
        assert_eq!(views.active(), None);

        views.set(ViewKind::Character);
        assert!(views.is_active());
        assert_eq!(views.active(), Some(ViewKind::Character));

        views.dismiss();
        assert!(!views.is_active());
        // Popping an empty stack is a no-op.
        views.dismiss();
        assert!(!views.is_active());
    }

    #[test]
    fn the_character_view_requests_the_exit_hint() {
        let game = knight_game();
        let mut views = Views::new();
        assert!(!views.wants_exit_hint(&game));
        views.set(ViewKind::Character);
        assert!(views.wants_exit_hint(&game));
    }

    #[test]
    fn the_character_view_draws_the_name_and_stats() {
        let (resources, layout, catalog) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        let labels: Vec<&str> = list.texts().iter().map(|(_, _, t)| *t).collect();
        assert!(labels[0].starts_with("Sir Rust the General"));
        assert!(labels.contains(&"Leadership"));
        assert!(labels.contains(&"Gold"));
        assert!(labels.contains(&"7500"));
        assert!(labels.contains(&"Current score"));
        // Zero-valued stats still show their label but no number.
        assert!(labels.contains(&"Villains caught"));
        assert!(labels.contains(&"Followers killed"));
        assert!(!labels.contains(&"0"));
    }

    #[test]
    fn the_inventory_belt_draws_an_artifact_icon_when_found() {
        let (resources, layout, catalog) = fixtures();
        let mut game = knight_game();
        game.artifacts.found[0] = true;
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        assert!(!list.blits_of(Sprite::ViewIcon(0)).is_empty());
        assert!(list.blits_of(Sprite::ViewIcon(1)).is_empty());
        // The class portrait is drawn from the class index.
        assert!(!list.blits_of(Sprite::ClassPortrait(0)).is_empty());
    }

    #[test]
    fn the_pack_may_reword_a_stat_label() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "strings": {"stats": {"gold": "GP"}}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        let game = knight_game();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );
        assert!(list.has_text_containing("GP"));
        assert!(!list.has_text_containing("Gold"));
    }

    #[test]
    fn an_inactive_view_draws_nothing() {
        let (resources, layout, catalog) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );
        assert!(list.is_empty());
    }

    #[test]
    fn a_zero_value_draws_no_number() {
        let (resources, layout, catalog) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        // "Followers killed" is 0, so its number must not appear.
        let texts = list.texts();
        assert!(texts.iter().any(|(_, _, t)| *t == "Followers killed"));
        assert!(!texts.iter().any(|(_, _, t)| *t == "0"));
    }

    fn army_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","anim":["a0.png","a1.png"]},
                {"index":1,"id":"ogres","name":"Ogres","skill_level":3,"hit_points":20,"move_rate":2,"melee":[5,10],"ranged":[0,0,0],"recruit_cost":100,"spoils_factor":1,"abilities":"","morale_group":"D"}
            ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    #[test]
    fn a_lone_stack_reads_high_morale() {
        let (resources, _, _) = army_fixtures();
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;

        assert_eq!(
            army_slot_morale(&resources, &game, 0, "Normal", "Low", "High"),
            "High"
        );
    }

    #[test]
    fn a_low_morale_relationship_reads_low() {
        let (resources, _, _) = army_fixtures();
        let mut game = Game::default();
        game.army[0].id.copy_from_str("ogres");
        game.army[0].count = 5;
        game.army[1].id.copy_from_str("peasants");
        game.army[1].count = 5;

        // Ogres (D) against peasants (A) reads 'L', so the ogre row is Low.
        assert_eq!(
            army_slot_morale(&resources, &game, 0, "Normal", "Low", "High"),
            "Low"
        );
    }

    #[test]
    fn the_army_view_draws_the_roster_and_out_of_control() {
        let (resources, layout, catalog) = army_fixtures();
        let mut game = Game::default();
        game.stats.leadership_current = 100;
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;
        game.army[1].id.copy_from_str("ogres");
        game.army[1].count = 6; // 6 * 20 hp = 120 > 100 leadership.

        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Army);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        assert!(list.has_text_containing("Peasants"));
        assert!(list.has_text_containing("Ogres"));
        assert!(list.has_text_containing("SL: 1 MV: 1"));
        assert!(list.has_text_containing("OUT OF CONTROL"));
        // The 6-strong ogre stack is out of control; the peasants (A vs D)
        // stay in control and report Normal morale.
        assert!(list.has_text_containing("Morale:Normal"));
        // A troop with declared art draws an animated frame.
        assert!(!list
            .blits_of(Sprite::TroopAnim { troop: 0, frame: 0 })
            .is_empty());
    }

    fn contract_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "zones": [{"id":"home","name":"Continentia"}],
                "villains": [
                  {"index":0,"id":"murray","name":"Murray","portrait":"m.png","zone":"home","reward":1000}
                ],
                "villain_descriptions": {
                  "murray": {"alias":"None","features":"Bald.","crimes":"Treason."}
                }}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    #[test]
    fn the_contract_view_requests_the_exit_hint_only_with_a_contract() {
        let mut views = Views::new();
        views.set(ViewKind::Contract);
        let mut game = Game::default();
        assert!(!views.wants_exit_hint(&game));
        game.contract.active_id.copy_from_str("murray");
        assert!(views.wants_exit_hint(&game));
    }

    #[test]
    fn the_contract_view_shows_a_placeholder_without_a_contract() {
        let (resources, layout, catalog) = contract_fixtures();
        let game = Game::default();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Contract);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );
        assert!(list.has_text_containing("You have no Contract!"));
    }

    #[test]
    fn the_contract_view_shows_the_wanted_poster() {
        let (resources, layout, catalog) = contract_fixtures();
        let mut game = Game::default();
        game.contract.active_id.copy_from_str("murray");
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let mut views = Views::new();
        views.set(ViewKind::Contract);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        assert!(list.has_text_containing("Name: Murray"));
        assert!(list.has_text_containing("Alias: None"));
        assert!(list.has_text_containing("Reward: 1000 gold"));
        assert!(list.has_text_containing("Last Seen: Continentia"));
        assert!(list.has_text_containing("Castle: Unknown"));
        assert!(list.has_text_containing("Distinguishing Features:"));
        assert!(list.has_text_containing("Bald."));
        assert!(list.has_text_containing("Crimes:"));
        assert!(list.has_text_containing("Treason."));
    }

    fn puzzle_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "villains": [
                  {"index":0,"id":"v0","name":"V0","portrait":"v.png","zone":"home","reward":100}
                ],
                "artifacts": [
                  {"index":0,"id":"a0","name":"A0","icon":"a.png","power":"increased_damage","effect":"e"}
                ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    /// Draw the puzzle twice: once to latch the open time, once at a late time
    /// so the reveal animation has played out and every cell shows its true
    /// caught/found state.
    fn draw_puzzle_fully(
        views: &mut Views,
        resources: &Resources,
        layout: &Layout,
        game: &Game,
    ) -> DrawList {
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let catalog = SpriteCatalog::from_resources(resources);
        let mut list = DrawList::new();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, &present, 0.0,
        );
        list.clear();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, &present, 100.0,
        );
        list
    }

    fn black_rects(list: &DrawList) -> usize {
        list.as_slice()
            .iter()
            .filter(|cmd| matches!(cmd, DrawCmd::Rect { color, .. } if *color == Color::BLACK))
            .count()
    }

    #[test]
    fn a_fresh_puzzle_shows_covers_and_no_reveals() {
        let (resources, layout, _) = puzzle_fixtures();
        let game = Game::default();
        // No scepter zone: no terrain, and nothing is caught or found.
        let mut views = Views::new();
        views.set(ViewKind::Puzzle);

        let list = draw_puzzle_fully(&mut views, &resources, &layout, &game);

        assert_eq!(black_rects(&list), 0, "nothing is revealed yet");
        // Villain 0 (cell 4,3) and artifact 0 (cell 0,0) still show covers.
        assert!(!list.blits_of(Sprite::ViewIcon(0)).is_empty());
        assert!(!list
            .blits_of(Sprite::VillainAnim {
                villain: 0,
                frame: 0
            })
            .is_empty());
    }

    #[test]
    fn caught_and_found_entities_reveal_their_cells() {
        let (resources, layout, _) = puzzle_fixtures();
        let mut game = Game::default();
        // Artifact 0 sits at cell (0,0); villain 0 at cell (4,3).
        game.artifacts.found[0] = true;
        game.contract.villains_caught[0] = true;
        let mut views = Views::new();
        views.set(ViewKind::Puzzle);

        let list = draw_puzzle_fully(&mut views, &resources, &layout, &game);

        // The found artifact and caught villain no longer draw their covers.
        assert!(list.blits_of(Sprite::ViewIcon(0)).is_empty());
        assert!(list
            .blits_of(Sprite::VillainAnim {
                villain: 0,
                frame: 0
            })
            .is_empty());
        // Both revealed cells draw black (no scepter terrain is loaded).
        assert_eq!(black_rects(&list), 2);
    }

    fn worldmap_parts() -> (Resources, Layout, SpriteCatalog, Palette, Map, Fog, Game) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "zones": [{"id":"home"}],
                "towns": [{"id":"t0","zone":"home","x":1,"y":1}],
                "castles": [{"id":"c0","zone":"home","x":2,"y":2}]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        let palette = Palette::fallback();
        let map = Map::from_tiles(5, 5, vec![Tile::plain_grass(); 25]).expect("5x5 map");
        let fog = Fog::new();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 0;
        game.position.y = 0;
        (resources, layout, catalog, palette, map, fog, game)
    }

    fn rects_of_color(list: &DrawList, color: Color) -> usize {
        list.as_slice()
            .iter()
            .filter(|cmd| matches!(cmd, DrawCmd::Rect { color: c, .. } if *c == color))
            .count()
    }

    fn draw_worldmap_view(
        views: &mut Views,
        resources: &Resources,
        layout: &Layout,
        catalog: &SpriteCatalog,
        palette: &Palette,
        map: &Map,
        fog: &Fog,
        game: &Game,
        time: f64,
    ) -> DrawList {
        let mut list = DrawList::new();
        let present = Present::new();
        views.draw(
            &mut list, layout, palette, resources, catalog, map, fog, game, &present, time,
        );
        list
    }

    #[test]
    fn worldmap_has_orb_reads_the_current_zone() {
        let (resources, _, _, _, _, _, mut game) = worldmap_parts();
        assert!(!worldmap_has_orb(&game, &resources));
        game.world.orbs_found[0] = true;
        assert!(worldmap_has_orb(&game, &resources));
    }

    #[test]
    fn the_worldmap_toggle_flips_and_the_view_never_uses_the_exit_hint() {
        let mut views = Views::new();
        assert!(!views.whole_map());
        views.toggle_whole_map();
        assert!(views.whole_map());
        views.toggle_whole_map();
        assert!(!views.whole_map());

        // The worldmap's hint is its own status mode, not the plain exit hint.
        let game = Game::default();
        views.set(ViewKind::Worldmap);
        assert!(!views.wants_exit_hint(&game));
    }

    #[test]
    fn the_worldmap_hides_fogged_tiles_without_the_orb() {
        let (resources, layout, catalog, palette, map, mut fog, game) = worldmap_parts();
        fog.seen[0][0] = true;
        let mut views = Views::new();
        views.set(ViewKind::Worldmap);

        let list = draw_worldmap_view(
            &mut views, &resources, &layout, &catalog, &palette, &map, &fog, &game, 0.0,
        );

        // Only the one revealed grass tile is painted (plus the black backing
        // and the hero's own blink rect).
        assert_eq!(rects_of_color(&list, Color::rgb(0x00, 0xAA, 0x00)), 1);
        assert!(list.has_text_containing("X=0 Y=0"));
    }

    #[test]
    fn the_worldmap_reveals_everything_with_the_orb() {
        let (resources, layout, catalog, palette, map, mut fog, mut game) = worldmap_parts();
        fog.seen[0][0] = true;
        game.world.orbs_found[0] = true;
        let mut views = Views::new();
        views.set(ViewKind::Worldmap);
        views.toggle_whole_map();

        let list = draw_worldmap_view(
            &mut views, &resources, &layout, &catalog, &palette, &map, &fog, &game, 0.0,
        );

        assert_eq!(rects_of_color(&list, Color::rgb(0x00, 0xAA, 0x00)), 25);
    }

    #[test]
    fn the_worldmap_marks_towns_and_castles() {
        let (resources, layout, catalog, palette, map, mut fog, game) = worldmap_parts();
        fog.seen[1][1] = true;
        fog.seen[2][2] = true;
        let mut views = Views::new();
        views.set(ViewKind::Worldmap);

        // Time 1.0 puts the hero's blink on yellow, so it cannot be mistaken
        // for the castle's red marker.
        let list = draw_worldmap_view(
            &mut views, &resources, &layout, &catalog, &palette, &map, &fog, &game, 1.0,
        );

        assert_eq!(rects_of_color(&list, palette.clr(Pal::Green)), 1);
        assert_eq!(rects_of_color(&list, palette.clr(Pal::Red)), 1);
        assert_eq!(rects_of_color(&list, palette.clr(Pal::Yellow)), 1);
    }

    fn controls_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "controls": {"settings": [
                  {"id":"delay","label":"Delay","type":"numeric","range":10,"default":5},
                  {"id":"sounds","label":"Sounds","type":"bool"},
                  {"id":"animation","label":"Animation","type":"bool"},
                  {"id":"cga","label":"CGA","type":"numeric","range":8,"hidden":true}
                ]},
                "strings": {"controls": {"title":" CTL ","on":"Yes","off":"No"}}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    fn draw_controls_view(
        views: &mut Views,
        resources: &Resources,
        layout: &Layout,
        game: &Game,
        present: &Present,
    ) -> DrawList {
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let catalog = SpriteCatalog::from_resources(resources);
        let mut list = DrawList::new();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, present, 0.0,
        );
        list
    }

    #[test]
    fn controls_visible_skips_hidden_rows() {
        let (resources, _, _) = controls_fixtures();
        assert_eq!(controls_visible(&resources), vec![0, 1, 2]);
    }

    #[test]
    fn audio_rows_are_disabled_without_audio() {
        let (resources, _, _) = controls_fixtures();
        // The shell ships no audio, so Sounds/Music/Volume gray out, as C does
        // when its audio init fails.
        assert!(controls_row_disabled(&resources, 1));
        assert!(!controls_row_disabled(&resources, 0));
        assert!(!controls_row_disabled(&resources, 2));
    }

    #[test]
    fn options_view_lists_movement_and_hides_inapplicable_keybinds() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "strings": {"keybinds": [
                  {"key":"F","label":"Take Flight"},
                  {"key":"L","label":"Make Landing"},
                  {"key":"N","label":"Sail Away"},
                  {"key":"A","label":"View Army"}
                ]}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let mut game = Game::default();
        game.character.mount = Mount::Ride;
        let mut views = Views::new();
        views.set(ViewKind::Options);
        let present = Present::for_layout(&layout);
        let list = draw_controls_view(&mut views, &resources, &layout, &game, &present);

        let texts = list.texts();
        // The movement reference is always drawn.
        assert!(texts.iter().any(|(_, _, t)| t.contains("Move Down")));
        // Riding hides Land (needs flight) and Sail Away (needs sailing), but
        // keeps Take Flight and the unconditional rows.
        assert!(texts.iter().any(|(_, _, t)| t.contains("Take Flight")));
        assert!(!texts.iter().any(|(_, _, t)| t.contains("Make Landing")));
        assert!(!texts.iter().any(|(_, _, t)| t.contains("Sail Away")));
        assert!(texts.iter().any(|(_, _, t)| t.contains("View Army")));
    }

    #[test]
    fn the_win_screen_draws_the_ending_art_and_body() {
        let (resources, layout, _) = controls_fixtures();
        let mut views = Views::new();
        views.open_end_game(true, "Good job, soldier!\nFinal Score: 42");
        assert!(views.end_won());
        let present = Present::for_layout(&layout);
        let list = draw_controls_view(&mut views, &resources, &layout, &Game::default(), &present);

        let texts = list.texts();
        assert!(texts
            .iter()
            .any(|(_, _, t)| t.contains("Good job, soldier!")));
        assert!(texts.iter().any(|(_, _, t)| t.contains("Final Score: 42")));
        assert_eq!(list.blits_of(Sprite::EndingWin).len(), 1);
        assert_eq!(list.blits_of(Sprite::EndingLose).len(), 0);
    }

    fn cartoon_fixture() -> (Resources, Layout) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                  {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,
                   "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,
                   "spoils_factor":1,"abilities":"","morale_group":"A"}
                ],
                "ending": {"grass_tile":"g.png","carpet_tile":"c.png","hero_tile":"h.png",
                           "grid_width":6,"grid_height":5,"carpet_column":4,
                           "carpet_length":5,"frame_count":10,"ticks_per_step":2,
                           "troop_border":true}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        (resources, layout)
    }

    #[test]
    fn cartoon_clock_advances_a_frame_per_step_and_wraps_the_tick() {
        let (resources, _) = cartoon_fixture();
        assert_eq!(cartoon_clock(&resources, 0.0), (0, 0));
        assert_eq!(cartoon_clock(&resources, 0.08), (1, 0));
        assert_eq!(cartoon_clock(&resources, 0.16), (2, 1));
        assert_eq!(cartoon_clock(&resources, 0.32), (0, 2));
    }

    #[test]
    fn the_win_cartoon_draws_the_grid_and_completes() {
        let (resources, layout) = cartoon_fixture();
        let mut views = Views::new();
        views.win_cartoon_open(&resources, "You win!");
        assert_eq!(views.active(), Some(ViewKind::WinCartoon));
        let game = Game::default();
        let present = Present::for_layout(&layout);
        let (map, fog) = map_and_fog();
        let catalog = SpriteCatalog::from_resources(&resources);
        let palette = Palette::fallback();

        // Frame 0: grass across the grid, no carpet or hero yet.
        let list = draw_controls_view(&mut views, &resources, &layout, &game, &present);
        assert_eq!(list.blits_of(Sprite::EndGrass).len(), 30);
        assert_eq!(list.blits_of(Sprite::EndCarpet).len(), 0);
        assert_eq!(list.blits_of(Sprite::EndHero).len(), 0);
        // The fixture ships no troop animation, so the border falls back to
        // the still sprite.
        assert_eq!(list.blits_of(Sprite::TroopStill(0)).len(), 1);
        assert!(!views.win_cartoon_done(&resources, 0.0));

        // Frame 10: the carpet is at full length and the hero stands on it.
        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 1.6,
        );
        assert_eq!(list.blits_of(Sprite::EndGrass).len(), 30);
        assert_eq!(list.blits_of(Sprite::EndCarpet).len(), 5);
        assert_eq!(list.blits_of(Sprite::EndHero).len(), 1);
        // Past the last frame, the cutscene is done.
        assert!(views.win_cartoon_done(&resources, 2.0));
    }

    #[test]
    fn controls_advance_cycles_and_skips_disabled_rows() {
        let (resources, _, _) = controls_fixtures();
        let mut game = Game::default();
        let views = Views::new();

        views.controls_advance(&mut game, &resources, 0);
        assert_eq!(game.stats.options[0], 2, "Delay 1 -> 2 over range 10");

        views.controls_advance(&mut game, &resources, 1);
        assert_eq!(game.stats.options[1], 1, "Sounds is disabled, a no-op");

        views.controls_advance(&mut game, &resources, 2);
        assert_eq!(game.stats.options[2], 0, "Animation bool 1 -> 0");
    }

    #[test]
    fn the_controls_cursor_clamps_a_negative_row_to_zero() {
        let mut views = Views::new();
        assert_eq!(views.controls_cursor(), 0);
        views.controls_set_cursor(3);
        assert_eq!(views.controls_cursor(), 3);
        views.controls_set_cursor(-1);
        assert_eq!(views.controls_cursor(), 0);
    }

    #[test]
    fn the_controls_view_draws_title_rows_and_values() {
        let (resources, layout, _) = controls_fixtures();
        let mut game = Game::default();
        game.stats.options[0] = 4; // Delay
        game.stats.options[1] = 1; // Sounds (on)
        game.stats.options[2] = 0; // Animation (off)
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        views.controls_set_cursor(1);
        let present = Present::new();

        let list = draw_controls_view(&mut views, &resources, &layout, &game, &present);

        // Title and the three visible rows (CGA is hidden).
        assert!(list.has_text_containing("CTL"));
        assert!(list.has_text_containing("1 Delay"));
        assert!(list.has_text_containing("2 Sounds"));
        assert!(list.has_text_containing("3 Animation"));
        assert!(
            !list.has_text_containing("CGA"),
            "the hidden row is not shown"
        );
        // The bool rows draw their pack-rewritten on/off labels.
        assert!(list.has_text_containing("Yes"));
        assert!(list.has_text_containing("No"));
        // The numeric Delay row draws its 0..9 scale.
        assert!(list.texts().iter().any(|(_, _, t)| *t == "4"));
    }

    #[test]
    fn a_modern_pack_appends_the_scale_row() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "modern"}, "troops": [],
                "controls": {"settings": [{"id":"delay","label":"Delay","type":"numeric","range":10}]}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let game = Game::default();
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        let present = Present::new();

        let list = draw_controls_view(&mut views, &resources, &layout, &game, &present);

        assert!(list.has_text_containing("2 Scale"));
        assert!(list.has_text_containing("Auto"));
    }

    fn spells_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "spells": [
                  {"id":"fireball","name":"Fireball","kind":"combat","cost":50},
                  {},{},{},{},{},{},
                  {"id":"time_stop","name":"Time Stop","kind":"adventure"}
                ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    #[test]
    fn the_spells_view_draws_two_columns_with_counts() {
        let (resources, layout, _) = spells_fixtures();
        let mut game = Game::default();
        game.spells.counts[0] = 2;
        game.spells.counts[7] = 1;
        let palette = Palette::fallback();
        let mut views = Views::new();
        views.set(ViewKind::Spells);

        let mut list = DrawList::new();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let catalog = SpriteCatalog::from_resources(&resources);
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, &present, 0.0,
        );

        assert!(list.has_text_containing("Combat"));
        assert!(list.has_text_containing("Adventuring"));
        assert!(list.has_text_containing("Fireball"));
        assert!(list.has_text_containing("Time Stop"));
        // The combat row carries its count on the left, the adventure row on
        // the right.
        assert!(list.has_text_containing("2 A Fireball"));
        assert!(list.has_text_containing("A Time Stop"));
    }

    #[test]
    fn the_spells_view_is_read_only_outside_cast_mode() {
        let mut views = Views::new();
        views.set(ViewKind::Spells);
        assert_eq!(
            views.spells_update(&Keys::pressed(&[Key::A])),
            SpellsInput::None
        );
    }

    #[test]
    fn cast_mode_defaults_to_the_adventure_column() {
        let mut views = Views::new();
        views.set(ViewKind::Spells);
        views.spells_set_cast_mode(true);

        // A in the default (adventure) column casts slot 7 + 0.
        assert_eq!(
            views.spells_update(&Keys::pressed(&[Key::A])),
            SpellsInput::Cast(7)
        );
    }

    #[test]
    fn left_and_right_switch_columns_without_dismissing() {
        let mut views = Views::new();
        views.set(ViewKind::Spells);
        views.spells_set_cast_mode(true);

        assert_eq!(
            views.spells_update(&Keys::pressed(&[Key::Left])),
            SpellsInput::Handled
        );
        assert_eq!(
            views.spells_update(&Keys::pressed(&[Key::C])),
            SpellsInput::Cast(2)
        );
    }

    #[test]
    fn escape_cancels_cast_mode_and_dismisses() {
        let mut views = Views::new();
        views.set(ViewKind::Spells);
        views.spells_set_cast_mode(true);

        assert_eq!(
            views.spells_update(&Keys::pressed(&[Key::Escape])),
            SpellsInput::Handled
        );
        assert!(!views.is_active());
        assert_eq!(views.active(), None);
    }

    fn gate_dest(name: &str) -> GateDestination {
        let mut destination = GateDestination::default();
        destination.name.copy_from_str(name);
        destination
    }

    #[test]
    fn the_gate_picker_snapshots_the_destination_list() {
        let mut views = Views::new();
        views.gate_open(
            &[gate_dest("Avalon"), gate_dest("Bari"), gate_dest("Cortona")],
            true,
        );
        assert_eq!(views.active(), Some(ViewKind::Gate));
        assert_eq!(views.gate_count(), 3);
        assert!(views.gate_is_town());
        assert_eq!(views.gate_cursor(), 0);
        assert_eq!(views.gate_dest(1).map(|d| d.name.as_str()), Some("Bari"));
        assert!(views.gate_dest(3).is_none());
    }

    #[test]
    fn a_letter_shortcut_selects_by_first_name_letter() {
        let mut views = Views::new();
        views.gate_open(
            &[
                gate_dest("Avalon"),
                gate_dest("Elan's Landing"),
                gate_dest("Zion"),
            ],
            false,
        );
        // 'E' matches Elan's Landing (index 1), not its row.
        assert!(views.gate_update(&Keys::pressed(&[Key::E])));
        assert_eq!(views.gate_take_chosen(), Some(1));
        assert!(!views.is_active());
    }

    #[test]
    fn arrows_wrap_and_enter_confirms_the_cursor() {
        let mut views = Views::new();
        views.gate_open(
            &[gate_dest("Avalon"), gate_dest("Bari"), gate_dest("Cortona")],
            false,
        );

        views.gate_update(&Keys::pressed(&[Key::Down]));
        views.gate_update(&Keys::pressed(&[Key::Down]));
        assert_eq!(views.gate_cursor(), 2);
        views.gate_update(&Keys::pressed(&[Key::Down]));
        assert_eq!(views.gate_cursor(), 0);

        views.gate_update(&Keys::pressed(&[Key::Up]));
        assert_eq!(views.gate_cursor(), 2);

        assert!(views.gate_update(&Keys::pressed(&[Key::Enter])));
        assert_eq!(views.gate_take_chosen(), Some(2));
    }

    #[test]
    fn escape_cancels_the_gate_picker() {
        let mut views = Views::new();
        views.gate_open(&[gate_dest("Avalon")], false);
        assert!(!views.gate_update(&Keys::pressed(&[Key::Escape])));
        assert!(!views.is_active());
    }

    #[test]
    fn the_gate_picker_draws_two_columns_and_the_footer() {
        let (resources, layout, _) = spells_fixtures();
        let mut views = Views::new();
        views.gate_open(
            &[gate_dest("Avalon"), gate_dest("Bari"), gate_dest("Cortona")],
            false,
        );
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let catalog = SpriteCatalog::from_resources(&resources);
        views.draw(
            &mut list,
            &layout,
            &palette,
            &resources,
            &catalog,
            &map,
            &fog,
            &Game::default(),
            &present,
            0.0,
        );

        assert!(list.has_text_containing("Castle Gate"));
        assert!(list.has_text_containing("Avalon"));
        assert!(list.has_text_containing("Bari"));
        assert!(list.has_text_containing("Cortona"));
        assert!(list.has_text_containing("Enter go"));
    }

    #[test]
    fn town_open_sets_context_and_cursor() {
        let mut views = Views::new();
        views.town_open("Riverton", "town0", 3, 4);
        assert_eq!(views.active(), Some(ViewKind::Town));
        assert_eq!(views.town_display_name(), "Riverton");
        assert_eq!(views.town_record_key(), "town0");
        assert_eq!(views.town_boat(), (3, 4));
    }

    #[test]
    fn town_arrows_navigate_and_letters_execute_rows() {
        let mut views = Views::new();
        views.town_open("Riverton", "town0", 3, 4);

        assert_eq!(
            views.town_update(&Keys::pressed(&[Key::Down])),
            TownInput::Handled
        );
        assert_eq!(
            views.town_update(&Keys::pressed(&[Key::B])),
            TownInput::RunRow(TownRow::Boat)
        );
        assert_eq!(
            views.town_update(&Keys::pressed(&[Key::Escape])),
            TownInput::Dismissed
        );
        assert!(!views.is_active());
    }

    #[test]
    fn the_town_info_overlay_is_sticky_until_acknowledged() {
        let mut views = Views::new();
        views.town_open("Riverton", "town0", 3, 4);
        views.town_show_info("Hello");

        // An unrelated key is consumed but keeps the overlay open.
        assert_eq!(
            views.town_update(&Keys::pressed(&[Key::X])),
            TownInput::Handled
        );
        // ESC dismisses only the overlay, not the view.
        assert_eq!(
            views.town_update(&Keys::pressed(&[Key::Escape])),
            TownInput::Handled
        );
        assert!(views.is_active());
    }

    #[test]
    fn the_town_view_draws_header_and_rows() {
        let (resources, layout, _) = spells_fixtures();
        let mut views = Views::new();
        views.town_open("Riverton", "town0", 3, 4);
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let catalog = SpriteCatalog::from_resources(&resources);
        views.draw(
            &mut list,
            &layout,
            &palette,
            &resources,
            &catalog,
            &map,
            &fog,
            &Game::default(),
            &present,
            0.0,
        );

        assert!(list.has_text_containing("Town of Riverton"));
        assert!(list.has_text_containing("Get New Contract"));
        assert!(list.has_text_containing("Gather information"));
        assert!(!list
            .blits_of(Sprite::Backdrop(crate::draw::Backdrop::Town))
            .is_empty());
    }

    fn castle_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,
                  "spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}
               ],
               "castles": [{"id":"castle0","name":"Darkstone","zone":"home","x":2,"y":2}]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    fn draw_castle_view(
        views: &mut Views,
        resources: &Resources,
        layout: &Layout,
        game: &Game,
    ) -> DrawList {
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let present = Present::new();
        let catalog = SpriteCatalog::from_resources(resources);
        let mut list = DrawList::new();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, &present, 0.0,
        );
        list
    }

    #[test]
    fn own_castle_opens_in_remove_mode_and_toggles() {
        let mut views = Views::new();
        views.own_castle_open("castle0");
        assert_eq!(views.active(), Some(ViewKind::OwnCastle));
        assert_eq!(views.own_castle_castle_id(), "castle0");
        assert!(!views.own_castle_garrison_mode());
        views.own_castle_toggle_mode();
        assert!(views.own_castle_garrison_mode());
    }

    #[test]
    fn the_home_castle_view_draws_the_menu() {
        let (resources, layout, _) = castle_fixtures();
        let mut views = Views::new();
        views.home_castle_open();
        let list = draw_castle_view(&mut views, &resources, &layout, &Game::default());

        assert!(list.has_text_containing("Recruit Soldiers"));
        assert!(list.has_text_containing("Audience with the King"));
        assert!(!list
            .blits_of(Sprite::Backdrop(crate::draw::Backdrop::Castle))
            .is_empty());
    }

    #[test]
    fn the_own_castle_view_draws_title_and_rows() {
        let (resources, layout, _) = castle_fixtures();
        let mut game = Game::default();
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 5;
        let mut views = Views::new();
        views.own_castle_open("castle0");
        let list = draw_castle_view(&mut views, &resources, &layout, &game);

        assert!(list.has_text_containing("Castle Darkstone"));
        assert!(list.has_text_containing("Remove troops"));
        assert!(list.has_text_containing("Peasants"));
    }

    fn recruit_fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"archers","name":"Archers","skill_level":1,"hit_points":3,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":250,
                  "spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"},
                 {"index":1,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":50,
                  "spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}
               ]}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    #[test]
    fn recruit_soldiers_opens_a_cost_sorted_pool() {
        let (resources, _, _) = recruit_fixtures();
        let mut game = Game::default();
        game.stats.leadership_current = 100;
        let mut views = Views::new();
        views.recruit_soldiers_open(&resources, &game);

        assert_eq!(views.active(), Some(ViewKind::RecruitSoldiers));
        assert_eq!(views.recruit_pool_count, 2);
        // Militia (cost 50) sorts before Archers (cost 250) despite catalog order.
        assert_eq!(views.recruit_pool[0], 1);
        assert_eq!(views.recruit_pool[1], 0);
    }

    #[test]
    fn recruit_soldiers_types_a_count_and_commits() {
        let (resources, _, _) = recruit_fixtures();
        let mut game = Game::default();
        game.stats.leadership_current = 1000;
        game.stats.gold = 5000;
        game.position.home_castle.copy_from_str("castle0");
        let mut views = Views::new();
        views.recruit_soldiers_open(&resources, &game);

        // A selects militia (pool slot 0).
        assert!(!views.recruit_soldiers_update(&mut game, &resources, &Keys::pressed(&[Key::A])));
        assert_eq!(views.recruit_whom, 1);

        views.recruit_soldiers_update(&mut game, &resources, &Keys::pressed(&[Key::Digit3]));
        assert_eq!(views.recruit_input, "3");

        assert!(!views.recruit_soldiers_update(
            &mut game,
            &resources,
            &Keys::pressed(&[Key::Enter])
        ));
        assert_eq!(views.recruit_whom, 0);
        assert!(views.recruit_input.is_empty());
        assert_eq!(game.army[0].id.as_str(), "militia");
        assert_eq!(game.army[0].count, 3);
    }

    #[test]
    fn recruit_soldiers_esc_at_idle_requests_dismiss() {
        let (resources, _, _) = recruit_fixtures();
        let mut game = Game::default();
        game.stats.leadership_current = 100;
        let mut views = Views::new();
        views.recruit_soldiers_open(&resources, &game);

        assert!(views.recruit_soldiers_update(
            &mut game,
            &resources,
            &Keys::pressed(&[Key::Escape])
        ));
    }

    #[test]
    fn the_recruit_soldiers_view_draws_rows_and_cost() {
        let (resources, layout, _) = recruit_fixtures();
        let mut game = Game::default();
        game.stats.leadership_current = 100;
        let mut views = Views::new();
        views.recruit_soldiers_open(&resources, &game);
        let list = draw_castle_view(&mut views, &resources, &layout, &game);

        assert!(list.has_text_containing("Recruit Soldiers"));
        assert!(list.has_text_containing("Militia"));
        assert!(list.has_text_containing("Archers"));
        assert!(list.has_text_containing("50"));
        assert!(list.has_text_containing("250"));
    }

    #[test]
    fn the_dwelling_view_draws_title_and_info() {
        let (resources, layout, _) = castle_fixtures();
        let mut views = Views::new();
        views.dwelling_open(Interact::DwellingPlains, 0, "Peasants", 5, 10, 5000, 5);
        let list = draw_castle_view(&mut views, &resources, &layout, &Game::default());

        assert!(list.has_text_containing("Plains"));
        assert!(list.has_text_containing("5 Peasants are available"));
        assert!(list.has_text_containing("Cost=10 each."));
        assert!(list.has_text_containing("You may recruit up to 5"));
        assert!(list.has_text_containing("Recruit how many"));
        assert!(!list
            .blits_of(Sprite::Backdrop(crate::draw::Backdrop::Plains))
            .is_empty());
    }
}
