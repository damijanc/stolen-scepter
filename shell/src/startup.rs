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
use crate::pointer::{HitList, HitTarget};
use crate::savepath::{scan_slots, SavePaths, SlotSet, SAVE_SLOT_COUNT};
use crate::slot_picker::{SlotChoice, SlotPicker};

/// Text C compiles in for the startup screens; the pack overrides it under
/// `strings.startup` and `strings.difficulty`.
const DEFAULT_CLASS_SELECT_HINT: &str = "Select Char A-D or L-Load saved game";
const DEFAULT_SAVE_PICKER_TITLE: &str = " Select game:";
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

/// Where each class's figure stands on the stock class-select art, relative
/// to the picker's top-left in its 288x184 placement, keyed by class index
/// (the letter A-D printed on the art).
///
/// DIVERGENCE (mouse): C has no pointer, so nothing in C or the pack locates
/// the figures. This geometry is authored by eye from the shipped art; the HD
/// pack's picker is the same composition at 4x, and the shell always places
/// it at 288x184 logical. Pushed in this order and a later region wins: the
/// barbarian (D), in front, covers the paladin's legs, and his region is split
/// so the sorceress keeps her own "C" label.
const CLASS_FIGURES: [(usize, Rect); 5] = [
    (0, Rect::new(0, 55, 112, 129)),
    (1, Rect::new(112, 8, 113, 176)),
    (2, Rect::new(225, 18, 63, 166)),
    (3, Rect::new(165, 80, 60, 104)),
    (3, Rect::new(225, 110, 37, 74)),
];

/// Longest accepted hero name, C's `char name_buf[11]`.
const NAME_MAX: usize = 10;
/// How long a splash holds before auto-advancing, C's `timeout`.
const SPLASH_TIMEOUT: f64 = 2.5;
/// How long the credits hold before auto-advancing, C's `timeout`.
const CREDITS_TIMEOUT: f64 = 2.5;
/// Shipped splash-art sizes, which C reads from the textures; the shell cannot.
const SPLASH_LOGO_W: i32 = 320;
const SPLASH_LOGO_H: i32 = 84;
const SPLASH_TITLE_W: i32 = 320;
const SPLASH_TITLE_H: i32 = 200;
/// Credits layout constants, C's locals.
const CREDIT_LINE_H: i32 = GLYPH_H + 1;
const CREDIT_PAD: i32 = 6;
const CREDIT_IMAGE_GAP: i32 = 8;
/// The class-select highlight sprite, C's `class_select_highlight`.
const CREDIT_INSET_W: i32 = 42;
const CREDIT_INSET_H: i32 = 44;
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
    SplashLogo,
    SplashTitle,
    Credits,
    ClassSelect,
    SavePicker,
    CreateGame,
    Intro,
}

/// The pre-game wizard's state. A default one starts on class select.
#[derive(Clone, Debug, PartialEq)]
pub struct Startup {
    screen: Screen,
    /// When the current timed screen (a splash, the credits, or the intro)
    /// was entered, so its hold can be measured against the wall clock.
    screen_started: f64,
    class_id: String,
    name: String,
    /// False while typing the name; true once Enter confirms and the
    /// difficulty selector takes over.
    has_name: bool,
    difficulty: i32,
    intro_started: f64,
    /// Chosen save slot: 0 for a plain new game, the picked slot otherwise.
    slot: usize,
    /// The save picker, with startup's trailing "New game" row.
    picker: SlotPicker,
    /// What the pointer was over last frame (see [`Startup::set_hover`]).
    hover: Option<HitTarget>,
}

impl Default for Startup {
    fn default() -> Self {
        Self {
            screen: Screen::ClassSelect,
            screen_started: 0.0,
            class_id: String::new(),
            name: String::new(),
            has_name: false,
            // C's initial selection is Normal.
            difficulty: 1,
            intro_started: 0.0,
            slot: 0,
            picker: SlotPicker::new(SlotSet::default(), true),
            hover: None,
        }
    }
}

impl Startup {
    /// Start the flow. When the pack declares a splash or credits, the flow
    /// opens there, exactly as C's `startup_flow` does; otherwise it opens on
    /// class select.
    pub fn new(resources: &Resources) -> Self {
        let has_splash_or_credits = !resources.sprites.splash_logo.is_empty()
            || !resources.sprites.splash_title.is_empty()
            || resources.credits.group_count > 0
            || resources.credits.copyright_count > 0;
        Self {
            screen: if has_splash_or_credits {
                Screen::SplashLogo
            } else {
                Screen::ClassSelect
            },
            ..Self::default()
        }
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
            Screen::SplashLogo => self.step_splash(
                keys,
                resources.sprites.splash_logo.is_empty(),
                Screen::SplashTitle,
                time,
            ),
            Screen::SplashTitle => self.step_splash(
                keys,
                resources.sprites.splash_title.is_empty(),
                Screen::Credits,
                time,
            ),
            Screen::Credits => self.step_credits(keys, resources, time),
            Screen::ClassSelect => self.update_class_select(keys, resources, paths),
            Screen::SavePicker => self.update_save_picker(keys),
            Screen::CreateGame => self.update_create_game(keys, resources, time),
            Screen::Intro => self.update_intro(keys, time),
        }
    }

    /// The pointer moved. On class select it outlines the figure under it.
    /// Entering a difficulty row (once the name is confirmed) or a picker row
    /// moves the selection there, as the Game Menu does: one highlight, and it
    /// is what Enter or a click picks.
    pub fn set_hover(&mut self, target: Option<HitTarget>) {
        if target != self.hover {
            match (self.screen, target) {
                (Screen::CreateGame, Some(HitTarget::Difficulty(row)))
                    if self.has_name && row < DIFFICULTY_KEYS.len() =>
                {
                    self.difficulty = row as i32;
                }
                _ => {}
            }
        }
        if self.screen == Screen::SavePicker {
            self.picker.set_hover(target);
        }
        self.hover = target;
    }

    /// A click, resolved to what the matching key does on the current screen.
    /// Name entry has no click affordance; it stays keyboard.
    pub fn click(
        &mut self,
        target: HitTarget,
        resources: &Resources,
        paths: &SavePaths,
        time: f64,
    ) -> Option<StartupChoice> {
        match (self.screen, target) {
            // Any key skips a splash or the credits.
            (Screen::SplashLogo, HitTarget::StartupSkip) => {
                self.enter(Screen::SplashTitle, time);
                None
            }
            (Screen::SplashTitle, HitTarget::StartupSkip) => {
                self.enter(Screen::Credits, time);
                None
            }
            (Screen::Credits, HitTarget::StartupSkip) => {
                self.enter(Screen::ClassSelect, time);
                None
            }
            // A figure is its letter; the hint bar is L.
            (Screen::ClassSelect, HitTarget::StartupClass(index)) => {
                if index < resources.classes_count().min(4) {
                    self.pick_class(index, resources);
                }
                None
            }
            (Screen::ClassSelect, HitTarget::StartupLoad) => {
                self.open_save_picker(resources, paths);
                None
            }
            (Screen::SavePicker, target) => {
                let choice = self.picker.click(target)?;
                self.apply_slot_choice(choice)
            }
            // A difficulty row is "select it, then Enter".
            (Screen::CreateGame, HitTarget::Difficulty(row))
                if self.has_name && row < DIFFICULTY_KEYS.len() =>
            {
                self.difficulty = row as i32;
                self.screen = Screen::Intro;
                self.intro_started = time;
                None
            }
            // Any key dismisses the intro.
            (Screen::Intro, HitTarget::StartupSkip) => Some(self.choice(StartupAction::New)),
            _ => None,
        }
    }

    fn enter(&mut self, screen: Screen, time: f64) {
        self.screen = screen;
        self.screen_started = time;
    }

    /// A timed, full-screen splash: advance on any key, or after the hold. An
    /// absent texture skips straight through, as C does.
    fn step_splash(
        &mut self,
        keys: &dyn KeySource,
        absent: bool,
        next: Screen,
        time: f64,
    ) -> Option<StartupChoice> {
        if absent || keys.any_pressed() || (time - self.screen_started) >= SPLASH_TIMEOUT {
            self.enter(next, time);
        }
        None
    }

    /// The credits screen: advance on any key, or after the hold. An empty
    /// credits block is skipped.
    fn step_credits(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        time: f64,
    ) -> Option<StartupChoice> {
        let absent = resources.credits.group_count == 0 && resources.credits.copyright_count == 0;
        if absent || keys.any_pressed() || (time - self.screen_started) >= CREDITS_TIMEOUT {
            self.enter(Screen::ClassSelect, time);
        }
        None
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
            self.open_save_picker(resources, paths);
            let _ = keys.chars_pressed();
            return None;
        }
        // A-D pick a class directly.
        let count = resources.classes_count().min(4);
        for index in 0..count {
            let key = [Key::A, Key::B, Key::C, Key::D][index];
            if keys.pressed(key) {
                self.pick_class(index, resources);
                // Do not leak the committing letter into name entry.
                let _ = keys.chars_pressed();
                return None;
            }
        }
        None
    }

    /// `L`, or a click on the hint bar.
    fn open_save_picker(&mut self, resources: &Resources, paths: &SavePaths) {
        self.picker = SlotPicker::new(scan_slots(paths, resources.pack_id.as_str()), true);
        self.picker.cursor = SAVE_SLOT_COUNT;
        for (slot, header) in self.picker.slots().headers.iter().enumerate() {
            if header.is_some() {
                self.picker.cursor = slot;
                break;
            }
        }
        self.screen = Screen::SavePicker;
    }

    /// A class letter, or a click on that figure.
    fn pick_class(&mut self, index: usize, resources: &Resources) {
        self.class_id = resources
            .class_by_index(index as i32)
            .map(|class| class.id.as_str().to_owned())
            .unwrap_or_else(|| "knight".to_owned());
        self.screen = Screen::CreateGame;
    }

    /// The shared picker reports a row; what each row means here is
    /// startup's own rule.
    fn update_save_picker(&mut self, keys: &dyn KeySource) -> Option<StartupChoice> {
        let choice = self.picker.update(keys)?;
        self.apply_slot_choice(choice)
    }

    /// What a picker row means at startup, from a key or a click alike.
    fn apply_slot_choice(&mut self, choice: SlotChoice) -> Option<StartupChoice> {
        match choice {
            SlotChoice::Cancel => {
                self.screen = Screen::ClassSelect;
                None
            }
            SlotChoice::Extra => {
                // The "New game" row targets the first empty slot.
                let mut target = 0;
                for (slot, header) in self.picker.slots().headers.iter().enumerate() {
                    if header.is_none() {
                        target = slot;
                        break;
                    }
                }
                self.slot = target;
                self.screen = Screen::CreateGame;
                None
            }
            SlotChoice::Slot(slot) => {
                self.slot = slot;
                if self.picker.slots().headers[slot].is_some() {
                    return Some(self.choice(StartupAction::Load));
                }
                // An empty slot chosen directly starts a new game into it.
                self.screen = Screen::CreateGame;
                None
            }
        }
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
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        time: f64,
    ) {
        let whole = Rect::new(0, 0, layout.screen_w, layout.screen_h);
        // A click anywhere is the "any key" that skips these.
        if matches!(
            self.screen,
            Screen::SplashLogo | Screen::SplashTitle | Screen::Credits | Screen::Intro
        ) {
            hits.push(whole, HitTarget::StartupSkip);
        }
        match self.screen {
            // The splashes and credits own the whole screen.
            Screen::SplashLogo => self.draw_splash(
                list,
                layout,
                palette,
                Sprite::SplashLogo,
                SPLASH_LOGO_W,
                SPLASH_LOGO_H,
            ),
            Screen::SplashTitle => self.draw_splash(
                list,
                layout,
                palette,
                Sprite::SplashTitle,
                SPLASH_TITLE_W,
                SPLASH_TITLE_H,
            ),
            Screen::Credits => self.draw_credits(list, layout, palette, resources),
            // Every wizard screen sits over the class-picker cartoon.
            Screen::ClassSelect | Screen::SavePicker | Screen::CreateGame | Screen::Intro => {
                list.rect(
                    Rect::new(0, 0, layout.screen_w, layout.screen_h),
                    palette.clr(Pal::Black),
                );
                self.draw_class_picker(list, layout);
                self.draw_status_hint(list, layout, palette, resources);
                match self.screen {
                    Screen::ClassSelect => {
                        self.publish_class_select(list, hits, layout, palette, resources)
                    }
                    Screen::SavePicker => {
                        let title = banner::text(
                            resources,
                            "startup",
                            "save_picker_title",
                            DEFAULT_SAVE_PICKER_TITLE,
                        );
                        let new_label = banner::text(
                            resources,
                            "startup",
                            "save_picker_new_game",
                            DEFAULT_SAVE_PICKER_NEW_GAME,
                        );
                        let hint = banner::text(
                            resources,
                            "startup",
                            "controls_hint",
                            DEFAULT_CONTROLS_HINT,
                        );
                        self.picker.draw(
                            list, hits, layout, palette, resources, title, new_label, hint,
                        )
                    }
                    Screen::CreateGame => {
                        self.draw_create_game(list, hits, layout, palette, resources, time)
                    }
                    Screen::Intro => self.draw_intro(list, layout, palette, resources),
                    _ => {}
                }
            }
        }
    }

    fn draw_splash(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        sprite: Sprite,
        w: i32,
        h: i32,
    ) {
        list.rect(
            Rect::new(0, 0, layout.screen_w, layout.screen_h),
            palette.clr(Pal::Black),
        );
        let x = (layout.screen_w - w) / 2;
        let y = (layout.screen_h - h) / 2;
        list.blit(sprite, Dst::rect(x, y, w, h));
    }

    fn draw_credits(
        &self,
        list: &mut DrawList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        // The credits drape over the character-pick cartoon, as C's do.
        list.rect(
            Rect::new(0, 0, layout.screen_w, layout.screen_h),
            palette.clr(Pal::Black),
        );
        self.draw_class_picker(list, layout);
        self.draw_status_hint(list, layout, palette, resources);

        let credits = &resources.credits;
        let line_h = CREDIT_LINE_H;
        let pad = CREDIT_PAD;

        let mut max_text_chars = 0;
        for group in &credits.groups[..credits.group_count] {
            max_text_chars = max_text_chars.max(group.label.as_str().chars().count());
            for name in &group.names[..group.name_count] {
                max_text_chars = max_text_chars.max(name.as_str().chars().count() + 2);
            }
        }
        for line in &credits.copyright[..credits.copyright_count] {
            max_text_chars = max_text_chars.max(line.as_str().chars().count());
        }

        let has_image = !credits.image.is_empty();
        let (image_w, image_h) = if has_image {
            (CREDIT_INSET_W, CREDIT_INSET_H)
        } else {
            (0, 0)
        };
        let text_w = max_text_chars as i32 * GLYPH_W;
        let gap = if has_image { CREDIT_IMAGE_GAP } else { 0 };
        let mut panel_w = pad + text_w + gap + image_w + pad;
        if panel_w > layout.screen_w - 8 {
            panel_w = layout.screen_w - 8;
        }

        let mut rows = 0;
        for (index, group) in credits.groups[..credits.group_count].iter().enumerate() {
            rows += 1 + group.name_count as i32;
            if index + 1 < credits.group_count {
                rows += 1;
            }
        }
        let copyright_rows = credits.copyright_count as i32;
        let spacer = if credits.group_count > 0 && credits.copyright_count > 0 {
            1
        } else {
            0
        };
        let total_rows = rows + spacer + copyright_rows;
        let mut panel_h = pad * 2 + total_rows * line_h;
        let min_panel_h = pad * 2 + image_h;
        if panel_h < min_panel_h {
            panel_h = min_panel_h;
        }

        let px = (layout.screen_w - panel_w) / 2;
        let py = (layout.screen_h - panel_h) / 2;
        list.rect(Rect::new(px, py, panel_w, panel_h), palette.clr(Pal::DBlue));
        list.rect_lines(
            Rect::new(px, py, panel_w, panel_h),
            palette.clr(Pal::Yellow),
        );

        let tx = px + pad;
        let mut ty = py + pad;
        for (index, group) in credits.groups[..credits.group_count].iter().enumerate() {
            list.text(group.label.as_str(), tx, ty, palette.clr(Pal::White));
            ty += line_h;
            for name in &group.names[..group.name_count] {
                list.text(name.as_str(), tx + 2 * GLYPH_W, ty, palette.clr(Pal::White));
                ty += line_h;
            }
            if index + 1 < credits.group_count {
                ty += line_h;
            }
        }
        if spacer > 0 {
            ty += line_h;
        }
        for line in &credits.copyright[..credits.copyright_count] {
            let line = expand_version(line.as_str(), resources.version);
            let tw = line.as_str().chars().count() as i32 * GLYPH_W;
            let cx = px + (panel_w - tw) / 2;
            list.text(&line, cx, ty, palette.clr(Pal::White));
            ty += line_h;
        }

        if has_image {
            let ix = px + panel_w - pad - image_w;
            let mut iy = py + pad + line_h;
            if iy + image_h > py + panel_h - pad {
                iy = py + panel_h - pad - image_h;
            }
            list.blit(Sprite::ClassHighlight, Dst::rect(ix, iy, image_w, image_h));
            list.rect_lines(
                Rect::new(ix - 1, iy - 1, image_w + 2, image_h + 2),
                palette.clr(Pal::DGreen),
            );
        }
    }

    /// The picker's top-left, shared by its blit and its click regions.
    fn class_picker_origin(layout: &Layout) -> (i32, i32) {
        (
            (layout.screen_w - CLASS_PICKER_W) / 2,
            (layout.screen_h - CLASS_PICKER_H) / 2,
        )
    }

    /// Class select's click regions: the hint bar (it names `L`), then each
    /// figure the pack has a class for. The hovered figure gets a yellow
    /// outline around its whole region.
    fn publish_class_select(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
    ) {
        hits.push(
            Rect::new(0, 0, layout.screen_w, GLYPH_H + 2),
            HitTarget::StartupLoad,
        );
        let (px, py) = Self::class_picker_origin(layout);
        let count = resources.classes_count().min(4);
        for (index, rect) in CLASS_FIGURES {
            if index < count {
                hits.push(
                    Rect::new(px + rect.x, py + rect.y, rect.w, rect.h),
                    HitTarget::StartupClass(index),
                );
            }
        }
        if let Some(HitTarget::StartupClass(index)) = self.hover {
            let parts = CLASS_FIGURES.iter().filter(|(i, _)| *i == index);
            let x0 = parts.clone().map(|(_, r)| r.x).min();
            let y0 = parts.clone().map(|(_, r)| r.y).min();
            let x1 = parts.clone().map(|(_, r)| r.x + r.w).max();
            let y1 = parts.map(|(_, r)| r.y + r.h).max();
            if let (Some(x0), Some(y0), Some(x1), Some(y1)) = (x0, y0, x1, y1) {
                list.rect_lines(
                    Rect::new(px + x0, py + y0, x1 - x0, y1 - y0),
                    palette.clr(Pal::Yellow),
                );
            }
        }
    }

    fn draw_class_picker(&self, list: &mut DrawList, layout: &Layout) {
        let (px, py) = Self::class_picker_origin(layout);
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

    fn draw_create_game(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
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
            let row_y = y + GLYPH_H * (5 + i as i32);
            list.text(&line, x + GLYPH_W, row_y, palette.clr(Pal::White));
            // Rows are clickable only once the name is confirmed, as the
            // arrows and Enter are.
            if self.has_name {
                hits.push(Rect::new(x, row_y, w, GLYPH_H), HitTarget::Difficulty(i));
            }
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

/// Substitute `%VERSION%` in a credits line, porting `expand_version`.
fn expand_version(src: &str, version: i32) -> String {
    src.replace("%VERSION%", &version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
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
        let mut startup = Startup::new(&resources);
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
        let mut startup = Startup::new(&resources);
        let choice = startup
            .update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0)
            .expect("quits");
        assert_eq!(choice.action, StartupAction::Quit);
    }

    #[test]
    fn an_empty_name_falls_back_to_the_world_default() {
        let resources = resources();
        let paths = paths("default-name");
        let mut startup = Startup::new(&resources);
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
        let mut startup = Startup::new(&resources);
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
        let mut startup = Startup::new(&resources);
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
        let startup = Startup::new(&resources);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(
            &mut list,
            &mut HitList::new(),
            &layout,
            &palette,
            &resources,
            0.0,
        );

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
        let mut startup = Startup::new(&resources);
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(
            &mut list,
            &mut HitList::new(),
            &layout,
            &palette,
            &resources,
            0.0,
        );

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
        let mut startup = Startup::new(&resources);

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
        let mut startup = Startup::new(&resources);
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
        let mut startup = Startup::new(&resources);
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
        let mut startup = Startup::new(&resources);
        startup.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(
            &mut list,
            &mut HitList::new(),
            &layout,
            &palette,
            &resources,
            0.0,
        );

        let texts = list.texts();
        assert!(texts
            .iter()
            .any(|(_, _, text)| text.contains("Select game:")));
        assert!(texts
            .iter()
            .any(|(_, _, text)| text.contains("Murray") && text.contains("123d")));
        assert!(texts.iter().any(|(_, _, text)| text.contains("New game")));
    }

    fn splash_resources() -> Resources {
        Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [], "pack_id": "openbounty", "version": 7,
                "world": {"default_name": "Hero"},
                "classes": [{"index":0,"id":"knight","name":"Knight","portrait":"k.png","starting_gold":1000}],
                "sprites": {"ui": {"splash_logo": "logo.png", "splash_title": "title.png"}},
                "credits": {
                  "image": "highlight.png",
                  "groups": [{"label": "By:", "names": ["A"]}],
                  "copyright": ["v%VERSION%"]
                }}"#,
        )
        .expect("fixture parses")
    }

    #[test]
    fn version_tokens_are_substituted() {
        assert_eq!(expand_version("v%VERSION%", 7), "v7");
        assert_eq!(expand_version("no token", 3), "no token");
    }

    #[test]
    fn the_splash_and_credits_precede_class_select() {
        let resources = splash_resources();
        let paths = paths("splash");
        let mut startup = Startup::new(&resources);
        // Logo splash -> title splash -> credits -> class select, one key each.
        for _ in 0..3 {
            assert_eq!(
                startup.update(&Keys::pressed(&[Key::Space]), &resources, &paths, 0.0),
                None
            );
        }
        // Now at class select: ESC quits rather than stepping back.
        let choice = startup
            .update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0)
            .expect("quits");
        assert_eq!(choice.action, StartupAction::Quit);
    }

    #[test]
    fn a_pack_without_splash_or_credits_opens_on_class_select() {
        let resources = resources();
        let paths = paths("no-splash");
        let mut startup = Startup::new(&resources);
        let choice = startup
            .update(&Keys::pressed(&[Key::Escape]), &resources, &paths, 0.0)
            .expect("quits");
        assert_eq!(choice.action, StartupAction::Quit);
    }

    #[test]
    fn the_splash_draws_the_logo_sprite() {
        let resources = splash_resources();
        let startup = Startup::new(&resources);
        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(
            &mut list,
            &mut HitList::new(),
            &layout,
            &palette,
            &resources,
            0.0,
        );
        assert_eq!(list.blits_of(Sprite::SplashLogo).len(), 1);
    }

    #[test]
    fn the_credits_draw_the_groups_and_versioned_copyright() {
        let resources = splash_resources();
        let paths = paths("credits-draw");
        let mut startup = Startup::new(&resources);
        // Advance past the two splashes to the credits screen.
        startup.update(&Keys::pressed(&[Key::Space]), &resources, &paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Space]), &resources, &paths, 0.0);

        let layout = layout();
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        startup.draw(
            &mut list,
            &mut HitList::new(),
            &layout,
            &palette,
            &resources,
            0.0,
        );

        let texts = list.texts();
        assert!(texts.iter().any(|(_, _, text)| *text == "By:"));
        assert!(texts.iter().any(|(_, _, text)| *text == "A"));
        assert!(texts.iter().any(|(_, _, text)| *text == "v7"));
        assert_eq!(list.blits_of(Sprite::ClassHighlight).len(), 1);
    }

    // ---- Mouse (a deliberate DIVERGENCE: C has no pointer) ----

    fn four_class_resources() -> Resources {
        Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [], "pack_id": "openbounty",
                "classes": [
                  {"index":0,"id":"knight","name":"Knight","portrait":"k.png"},
                  {"index":1,"id":"paladin","name":"Paladin","portrait":"p.png"},
                  {"index":2,"id":"sorceress","name":"Sorceress","portrait":"s.png"},
                  {"index":3,"id":"barbarian","name":"Barbarian","portrait":"b.png"}
                ]}"#,
        )
        .expect("fixture parses")
    }

    fn drawn_hits(startup: &Startup, resources: &Resources) -> (DrawList, HitList) {
        let layout = Layout::from_resources(resources).expect("lays out");
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        startup.draw(
            &mut list,
            &mut hits,
            &layout,
            &Palette::fallback(),
            resources,
            0.0,
        );
        (list, hits)
    }

    /// Each letter printed on the stock art lands on its own class. The
    /// letter positions are read off `class_select_picker.png`.
    #[test]
    fn each_printed_letter_on_the_art_hits_its_own_class() {
        let resources = four_class_resources();
        let layout = Layout::from_resources(&resources).expect("lays out");
        let (px, py) = Startup::class_picker_origin(&layout);
        let (_, hits) = drawn_hits(&Startup::default(), &resources);
        for (index, (lx, ly)) in [(37, 77), (182, 22), (233, 78), (258, 172)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                hits.hit(px + lx, py + ly),
                Some(HitTarget::StartupClass(index)),
                "letter {}",
                ['A', 'B', 'C', 'D'][index]
            );
        }
    }

    /// The sky above the knight is no figure, and class select publishes no
    /// backdrop: a stray click there does nothing -- it certainly never quits.
    #[test]
    fn a_click_off_the_figures_on_class_select_does_nothing() {
        let resources = four_class_resources();
        let layout = Layout::from_resources(&resources).expect("lays out");
        let (px, py) = Startup::class_picker_origin(&layout);
        let (_, hits) = drawn_hits(&Startup::default(), &resources);
        assert_eq!(hits.hit(px + 20, py + 30), None);
    }

    #[test]
    fn clicking_a_figure_matches_its_letter() {
        let resources = four_class_resources();
        let paths = paths("click-class");
        for (index, key) in [Key::A, Key::B, Key::C, Key::D].into_iter().enumerate() {
            let mut by_key = Startup::default();
            by_key.update(&Keys::pressed(&[key]), &resources, &paths, 0.0);
            let mut by_click = Startup::default();
            assert_eq!(
                by_click.click(HitTarget::StartupClass(index), &resources, &paths, 0.0),
                None
            );
            assert_eq!(by_click, by_key, "class {index}");
            assert_eq!(by_click.screen, Screen::CreateGame);
        }
    }

    /// A two-class pack has no C or D: no region, and a stray target is inert.
    #[test]
    fn only_the_classes_a_pack_has_are_clickable() {
        let resources = resources();
        let paths = paths("click-class-count");
        let (_, hits) = drawn_hits(&Startup::default(), &resources);
        assert!(hits.regions().iter().all(|r| !matches!(
            r.target,
            HitTarget::StartupClass(index) if index >= 2
        )));
        let mut startup = Startup::default();
        startup.click(HitTarget::StartupClass(3), &resources, &paths, 0.0);
        assert_eq!(startup.screen, Screen::ClassSelect);
    }

    #[test]
    fn hovering_a_figure_outlines_it_and_picks_nothing() {
        let resources = four_class_resources();
        let mut startup = Startup::default();
        startup.set_hover(Some(HitTarget::StartupClass(1)));
        let (list, _) = drawn_hits(&startup, &resources);
        let yellow = Palette::fallback().clr(Pal::Yellow);
        assert!(list
            .as_slice()
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::RectLines { color, .. } if *color == yellow)));
        assert_eq!(startup.screen, Screen::ClassSelect);
    }

    /// The hint bar names `L`; a click on it is `L`, cursor placement and all.
    #[test]
    fn clicking_the_hint_bar_opens_the_picker_like_l() {
        let resources = resources();
        let paths = paths("click-load-bar");
        write_save(&paths.slot("openbounty", 3).expect("slot"), "Murray", 9);
        let (_, hits) = drawn_hits(&Startup::default(), &resources);
        assert_eq!(hits.hit(1, 1), Some(HitTarget::StartupLoad));
        let mut by_key = Startup::default();
        by_key.update(&Keys::pressed(&[Key::L]), &resources, &paths, 0.0);
        let mut by_click = Startup::default();
        by_click.click(HitTarget::StartupLoad, &resources, &paths, 0.0);
        assert_eq!(by_click, by_key);
        assert_eq!(by_click.picker.cursor, 3);
    }

    #[test]
    fn clicking_a_saved_slot_loads_it_and_off_the_panel_steps_back() {
        let resources = resources();
        let paths = paths("click-slot");
        write_save(&paths.slot("openbounty", 2).expect("slot"), "Murray", 9);
        let mut startup = Startup::default();
        startup.click(HitTarget::StartupLoad, &resources, &paths, 0.0);
        let (_, hits) = drawn_hits(&startup, &resources);
        let row = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::SlotRow(2))
            .expect("slot 2 is clickable at startup");
        let target = hits.hit(row.rect.x + 1, row.rect.y + 1).expect("hit");
        let choice = startup
            .click(target, &resources, &paths, 0.0)
            .expect("loads");
        assert_eq!(choice.action, StartupAction::Load);
        assert_eq!(choice.slot, 2);

        // Off the panel is ESC: back to class select, never Quit.
        let mut startup = Startup::default();
        startup.click(HitTarget::StartupLoad, &resources, &paths, 0.0);
        assert_eq!(
            startup.click(HitTarget::SlotCancel, &resources, &paths, 0.0),
            None
        );
        assert_eq!(startup.screen, Screen::ClassSelect);
    }

    /// A click anywhere skips each splash and the credits, as any key does.
    #[test]
    fn a_click_skips_the_splashes_and_credits() {
        let resources = splash_resources();
        let paths = paths("click-splash");
        let mut startup = Startup::new(&resources);
        for expected in [Screen::SplashTitle, Screen::Credits, Screen::ClassSelect] {
            let (_, hits) = drawn_hits(&startup, &resources);
            assert_eq!(hits.hit(0, 0), Some(HitTarget::StartupSkip));
            startup.click(HitTarget::StartupSkip, &resources, &paths, 1.0);
            assert_eq!(startup.screen, expected);
            assert_eq!(startup.screen_started, 1.0, "the next hold starts now");
        }
        // Class select itself is not skippable.
        startup.click(HitTarget::StartupSkip, &resources, &paths, 1.0);
        assert_eq!(startup.screen, Screen::ClassSelect);
    }

    /// From the create-game screen with the name confirmed.
    fn at_difficulty(resources: &Resources, paths: &SavePaths) -> Startup {
        let mut startup = Startup::default();
        startup.update(&Keys::pressed(&[Key::A]), resources, paths, 0.0);
        startup.update(&Keys::pressed(&[Key::Enter]), resources, paths, 0.0);
        assert!(startup.has_name);
        startup
    }

    /// The anti-drift test for the difficulty table: each row's region holds
    /// the line drawn for it and hits that row alone.
    #[test]
    fn the_difficulty_hit_regions_line_up_with_the_drawn_rows() {
        let resources = resources();
        let paths = paths("click-difficulty-drift");
        let startup = at_difficulty(&resources, &paths);
        let (list, hits) = drawn_hits(&startup, &resources);
        let texts = list.texts();
        for (row, days) in ["900", "600", "400", "200"].iter().enumerate() {
            let region = hits
                .regions()
                .iter()
                .find(|r| r.target == HitTarget::Difficulty(row))
                .unwrap_or_else(|| panic!("row {row} published no region"));
            let (tx, ty, _) = texts
                .iter()
                .find(|(_, _, text)| text.contains(days))
                .copied()
                .expect("row drawn");
            let r = region.rect;
            assert!(tx >= r.x && tx < r.x + r.w && ty >= r.y && ty < r.y + r.h);
            assert_eq!(
                hits.hit(r.x + r.w / 2, r.y + r.h / 2),
                Some(HitTarget::Difficulty(row))
            );
        }
    }

    /// While the name is being typed the table is not live, by key or click.
    #[test]
    fn the_difficulty_rows_are_inert_during_name_entry() {
        let resources = resources();
        let paths = paths("click-difficulty-name");
        let mut startup = Startup::default();
        startup.update(&Keys::pressed(&[Key::A]), &resources, &paths, 0.0);
        let (_, hits) = drawn_hits(&startup, &resources);
        assert!(hits
            .regions()
            .iter()
            .all(|r| !matches!(r.target, HitTarget::Difficulty(_))));
        startup.set_hover(Some(HitTarget::Difficulty(3)));
        startup.click(HitTarget::Difficulty(3), &resources, &paths, 0.0);
        assert_eq!(startup.difficulty, 1);
        assert_eq!(startup.screen, Screen::CreateGame);
    }

    /// A row click is "select it, then Enter".
    #[test]
    fn clicking_a_difficulty_matches_selecting_it_and_pressing_enter() {
        let resources = resources();
        let paths = paths("click-difficulty");
        let mut by_key = at_difficulty(&resources, &paths);
        by_key.update(&Keys::pressed(&[Key::Down]), &resources, &paths, 0.0);
        by_key.update(&Keys::pressed(&[Key::Enter]), &resources, &paths, 5.0);
        let mut by_click = at_difficulty(&resources, &paths);
        by_click.click(HitTarget::Difficulty(2), &resources, &paths, 5.0);
        assert_eq!(by_click, by_key);
        assert_eq!(by_click.screen, Screen::Intro);
        // And a click dismisses the intro like any key.
        let choice = by_click
            .click(HitTarget::StartupSkip, &resources, &paths, 5.0)
            .expect("intro dismisses");
        assert_eq!(choice.action, StartupAction::New);
        assert_eq!(choice.difficulty, 2);
    }

    /// Entering a row moves the ">" marker there; a resting pointer does not
    /// pin it, so the arrows still work.
    #[test]
    fn hovering_a_difficulty_moves_the_marker_without_pinning_it() {
        let resources = resources();
        let paths = paths("hover-difficulty");
        let mut startup = at_difficulty(&resources, &paths);
        startup.set_hover(Some(HitTarget::Difficulty(3)));
        assert_eq!(startup.difficulty, 3);
        startup.update(&Keys::pressed(&[Key::Up]), &resources, &paths, 0.0);
        startup.set_hover(Some(HitTarget::Difficulty(3)));
        assert_eq!(startup.difficulty, 2);
    }
}
