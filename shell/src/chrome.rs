//! The outer frame and the status bar, porting `src/chrome.c`.
//!
//! The frame is a full-screen bitmap with a TRANSPARENT interior, so it is
//! blitted over the status bar and the middle strip rather than around them,
//! and whatever the views paint afterwards shows through the hole.

use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{Color, DrawList, Dst, Rect, Sprite};
use crate::layout::{Layout, BAR_H, BAR_Y, STATUS_H, STATUS_X, STATUS_Y};
use crate::palette::{Pal, Palette};
use crate::views::worldmap_has_orb;

/// Text C compiles in for each status-bar string, used when the pack declares
/// none. Kept verbatim, including the surrounding spaces, which are what sit
/// the text off the frame.
pub const DEFAULT_QUIT_PROMPT: &str = " Quit without saving (y/n) ";
pub const DEFAULT_EXIT_HINT: &str = "Press 'ESC' to exit";
pub const DEFAULT_DAYS_LEFT: &str = " Options / Controls / Days Left:%DAYS% ";
pub const DEFAULT_TIME_STOP: &str = " Options / Controls / Time Stop:%STEPS% ";
pub const DEFAULT_WORLDMAP_YOUR_MAP: &str = "'ESC' to exit / 'SPC' your map";
pub const DEFAULT_WORLDMAP_WHOLE_MAP: &str = "'ESC' to exit / 'SPC' whole map";

/// What the status bar says. Three modes, in priority order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusMode {
    /// Ctrl+Q: a modal prompt that overrides everything until it is answered.
    FastQuit,
    /// A special screen or an open dialog: centred "Press 'ESC' to exit".
    ExitHint,
    /// The worldmap: centred hint that switches between "Press ESC" and the
    /// SPACE toggle lines as the crystal orb and toggle state allow.
    Worldmap { whole_map: bool },
    /// Adventure mode: days left, or the time-stop countdown instead.
    Adventure,
    /// Combat and the like supply their own line, already formatted.
    Custom(&'static str),
}

pub struct Chrome<'a> {
    pub layout: &'a Layout,
    pub palette: &'a Palette,
    pub resources: &'a Resources,
    pub game: &'a Game,
    pub mode: StatusMode,
}

impl Chrome<'_> {
    fn status_rect(&self) -> Rect {
        Rect::new(STATUS_X, STATUS_Y, self.layout.status_w(), STATUS_H)
    }

    /// The difficulty colour behind the status text.
    fn status_background(&self) -> Color {
        Color::from_argb(
            self.resources
                .colors
                .difficulty_bar(self.game.character.difficulty),
        )
    }

    /// Draw the full frame: black, status bar, middle strip, frame bitmap,
    /// then the status text. Ports `chrome_draw`.
    pub fn draw(&self, list: &mut DrawList) {
        // Everything starts black. The frame bitmap paints over it, and the
        // map and views paint the interior on top.
        list.rect(
            Rect::new(0, 0, self.layout.screen_w, self.layout.screen_h),
            self.palette.clr(Pal::Black),
        );
        self.draw_shell(list);
        self.draw_status_text(list);
    }

    /// The frame WITHOUT the full-screen black, for a caller that has already
    /// painted the interior. Ports `chrome_draw_with_status`: blacking the
    /// screen here would erase the combat field it is drawn over.
    pub fn draw_over(&self, list: &mut DrawList) {
        self.draw_shell(list);
        self.draw_status_text(list);
    }

    fn draw_shell(&self, list: &mut DrawList) {
        list.rect(self.status_rect(), self.status_background());
        // The middle strip spans the whole width; the frame's side columns
        // paint over its outer 16 pixels immediately afterwards.
        list.blit(
            Sprite::HudBarStrip,
            Dst::rect(0, BAR_Y, self.layout.screen_w, BAR_H),
        );
        // Transparent interior, so the bar and strip above stay visible.
        list.blit(
            Sprite::ChromeOverworld,
            Dst::rect(0, 0, self.layout.screen_w, self.layout.screen_h),
        );
    }

    /// The status line for the current mode.
    pub fn status_text(&self) -> String {
        match self.mode {
            StatusMode::Custom(text) => text.to_owned(),
            StatusMode::FastQuit => banner::text(
                self.resources,
                "ui",
                "quit_to_dos_prompt",
                DEFAULT_QUIT_PROMPT,
            )
            .to_owned(),
            StatusMode::ExitHint => {
                banner::text(self.resources, "ui", "press_esc_to_exit", DEFAULT_EXIT_HINT)
                    .to_owned()
            }
            StatusMode::Worldmap { whole_map } => {
                if !worldmap_has_orb(self.game, self.resources) {
                    banner::text(self.resources, "ui", "press_esc_to_exit", DEFAULT_EXIT_HINT)
                        .to_owned()
                } else if whole_map {
                    banner::text(
                        self.resources,
                        "ui",
                        "worldmap_hint_your_map",
                        DEFAULT_WORLDMAP_YOUR_MAP,
                    )
                    .to_owned()
                } else {
                    banner::text(
                        self.resources,
                        "ui",
                        "worldmap_hint_whole_map",
                        DEFAULT_WORLDMAP_WHOLE_MAP,
                    )
                    .to_owned()
                }
            }
            StatusMode::Adventure => {
                // A banked time stop replaces the day count entirely: the
                // clock is not running, so days left would be misleading.
                if self.game.stats.time_stop > 0 {
                    banner::formatted(
                        self.resources,
                        "banners",
                        "status_time_stop",
                        DEFAULT_TIME_STOP,
                        &[("STEPS", &self.game.stats.time_stop.to_string())],
                    )
                } else {
                    banner::formatted(
                        self.resources,
                        "banners",
                        "status_days_left",
                        DEFAULT_DAYS_LEFT,
                        &[("DAYS", &self.game.stats.days_left.to_string())],
                    )
                }
            }
        }
    }

    fn draw_status_text(&self, list: &mut DrawList) {
        let text = self.status_text();
        if text.is_empty() {
            return;
        }
        let white = self.palette.clr(Pal::White);
        // The two modal lines are centred; the rest sit against the left of
        // the bar, one pixel in from its corner.
        match self.mode {
            StatusMode::FastQuit | StatusMode::ExitHint | StatusMode::Worldmap { .. } => list
                .text_centered(
                    &text,
                    STATUS_X + self.layout.status_w() / 2,
                    STATUS_Y + 1,
                    white,
                ),
            _ => list.text(&text, STATUS_X + 1, STATUS_Y + 1, white),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;

    fn pack(extra: &str) -> Resources {
        Resources::from_json_str(&format!(
            r#"{{"render": {{"mode": "legacy"}}, "troops": [] {extra}}}"#
        ))
        .expect("fixture parses")
    }

    fn chrome_of<'a>(
        layout: &'a Layout,
        palette: &'a Palette,
        resources: &'a Resources,
        game: &'a Game,
        mode: StatusMode,
    ) -> Chrome<'a> {
        Chrome {
            layout,
            palette,
            resources,
            game,
            mode,
        }
    }

    fn parts(extra: &str) -> (Layout, Palette, Resources, Game) {
        let resources = pack(extra);
        let layout = Layout::from_resources(&resources).expect("lays out");
        (layout, Palette::fallback(), resources, Game::default())
    }

    #[test]
    fn the_frame_is_blitted_over_the_bar_not_around_it() {
        // The frame bitmap has a transparent interior, so the status fill and
        // the middle strip have to be painted BEFORE it, or it covers them.
        let (layout, palette, resources, game) = parts("");
        let mut list = DrawList::new();
        chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure).draw(&mut list);

        let order: Vec<_> = list
            .as_slice()
            .iter()
            .map(|cmd| match cmd {
                DrawCmd::Rect { .. } => "rect",
                DrawCmd::Blit {
                    sprite: Sprite::HudBarStrip,
                    ..
                } => "strip",
                DrawCmd::Blit {
                    sprite: Sprite::ChromeOverworld,
                    ..
                } => "frame",
                DrawCmd::Text { .. } => "text",
                _ => "other",
            })
            .collect();
        assert_eq!(order, vec!["rect", "rect", "strip", "frame", "text"]);
    }

    #[test]
    fn drawing_over_a_painted_interior_skips_the_black() {
        // Combat has already drawn its field; blacking the screen here would
        // erase it.
        let (layout, palette, resources, game) = parts("");
        let mut list = DrawList::new();
        chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure).draw_over(&mut list);

        // One rect only: the status fill. No full-screen black.
        let rects: Vec<_> = list
            .as_slice()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Rect { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(rects, vec![Rect::new(STATUS_X, STATUS_Y, 288, STATUS_H)]);
    }

    #[test]
    fn the_status_bar_takes_its_colour_from_the_difficulty() {
        let (layout, palette, resources, mut game) = parts("");
        let colour_at = |game: &Game| {
            let mut list = DrawList::new();
            chrome_of(&layout, &palette, &resources, game, StatusMode::Adventure).draw(&mut list);
            match list.as_slice()[1] {
                DrawCmd::Rect { color, .. } => color,
                _ => panic!("the status fill is the second command"),
            }
        };

        game.character.difficulty = 0;
        assert_eq!(colour_at(&game), Color::rgb(0x00, 0xAA, 0xAA));
        game.character.difficulty = 1;
        assert_eq!(colour_at(&game), Color::rgb(0xAA, 0x00, 0x00));
        game.character.difficulty = 3;
        assert_eq!(colour_at(&game), Color::rgb(0xAA, 0x00, 0xAA));
        // Out of range reads as easy, which is where C's switch falls.
        game.character.difficulty = 99;
        assert_eq!(colour_at(&game), Color::rgb(0x00, 0xAA, 0xAA));
    }

    #[test]
    fn a_pack_may_restyle_the_difficulty_bar() {
        let (layout, palette, resources, mut game) =
            parts(r##", "colors": {"difficulty_bar": {"normal": "#112233"}}"##);
        game.character.difficulty = 1;
        let mut list = DrawList::new();
        chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure).draw(&mut list);

        match list.as_slice()[1] {
            DrawCmd::Rect { color, .. } => assert_eq!(color, Color::rgb(0x11, 0x22, 0x33)),
            _ => panic!("the status fill is the second command"),
        }

        // The colours it did not name keep their EGA defaults.
        game.character.difficulty = 0;
        let mut list = DrawList::new();
        chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure).draw(&mut list);
        match list.as_slice()[1] {
            DrawCmd::Rect { color, .. } => assert_eq!(color, Color::rgb(0x00, 0xAA, 0xAA)),
            _ => panic!("the status fill is the second command"),
        }
    }

    #[test]
    fn adventure_mode_counts_days_and_a_time_stop_replaces_them() {
        let (layout, palette, resources, mut game) = parts("");
        game.stats.days_left = 400;
        let chrome = chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure);
        assert_eq!(chrome.status_text(), " Options / Controls / Days Left:400 ");

        // A banked time stop replaces the day count: the clock is not running,
        // so days left would be misleading.
        game.stats.time_stop = 7;
        let chrome = chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure);
        assert_eq!(chrome.status_text(), " Options / Controls / Time Stop:7 ");
    }

    #[test]
    fn the_pack_may_reword_the_status_line() {
        let (layout, palette, resources, mut game) = parts(
            r#", "strings": {"banners": {"status_days_left": "%DAYS% days"},
                             "ui": {"press_esc_to_exit": "ESC exits"}}"#,
        );
        game.stats.days_left = 12;
        assert_eq!(
            chrome_of(&layout, &palette, &resources, &game, StatusMode::Adventure).status_text(),
            "12 days"
        );
        assert_eq!(
            chrome_of(&layout, &palette, &resources, &game, StatusMode::ExitHint).status_text(),
            "ESC exits"
        );
    }

    #[test]
    fn the_modal_lines_are_centred_and_the_day_count_is_not() {
        let (layout, palette, resources, game) = parts("");
        let x_of = |mode| {
            let mut list = DrawList::new();
            chrome_of(&layout, &palette, &resources, &game, mode).draw(&mut list);
            list.texts()[0].0
        };

        // Left-aligned, one pixel in from the bar's corner.
        assert_eq!(x_of(StatusMode::Adventure), STATUS_X + 1);
        // Centred on the bar. "Press 'ESC' to exit" is 19 glyphs = 152 wide,
        // so it starts 76 left of the bar's midpoint.
        assert_eq!(x_of(StatusMode::ExitHint), STATUS_X + 288 / 2 - 76);
    }

    #[test]
    fn a_custom_line_is_taken_as_given() {
        // Combat writes its own title and does not go through the banners.
        let (layout, palette, resources, game) = parts("");
        let chrome = chrome_of(
            &layout,
            &palette,
            &resources,
            &game,
            StatusMode::Custom("Sir Rust vs Ogres"),
        );
        assert_eq!(chrome.status_text(), "Sir Rust vs Ogres");

        let mut list = DrawList::new();
        chrome.draw(&mut list);
        assert_eq!(
            list.texts()[0],
            (STATUS_X + 1, STATUS_Y + 1, "Sir Rust vs Ogres")
        );
    }

    #[test]
    fn an_empty_status_line_draws_no_text_at_all() {
        let (layout, palette, resources, game) = parts("");
        let mut list = DrawList::new();
        chrome_of(&layout, &palette, &resources, &game, StatusMode::Custom("")).draw(&mut list);
        assert!(list.texts().is_empty());
    }

    #[test]
    fn the_worldmap_hint_is_plain_escape_without_the_orb() {
        // No zone, no orb: SPACE has nothing to toggle, so the hint is the
        // plain exit line whatever the toggle flag says.
        let (layout, palette, resources, game) = parts("");
        for whole_map in [false, true] {
            let chrome = chrome_of(
                &layout,
                &palette,
                &resources,
                &game,
                StatusMode::Worldmap { whole_map },
            );
            assert_eq!(chrome.status_text(), DEFAULT_EXIT_HINT);
        }
    }

    #[test]
    fn the_worldmap_hint_tracks_the_orb_and_the_toggle() {
        let (layout, palette, resources, mut game) = parts(r#", "zones": [{"id":"home"}]"#);
        game.position.zone.copy_from_str("home");
        game.world.orbs_found[0] = true;

        // Owning the orb: the hint advertises whichever side SPACE will show.
        let chrome = chrome_of(
            &layout,
            &palette,
            &resources,
            &game,
            StatusMode::Worldmap { whole_map: false },
        );
        assert_eq!(chrome.status_text(), DEFAULT_WORLDMAP_WHOLE_MAP);

        let chrome = chrome_of(
            &layout,
            &palette,
            &resources,
            &game,
            StatusMode::Worldmap { whole_map: true },
        );
        assert_eq!(chrome.status_text(), DEFAULT_WORLDMAP_YOUR_MAP);
    }
}
