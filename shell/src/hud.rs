//! The sidebar panels, porting `src/hud.c`.
//!
//! Five 48x34 panels stack down the column `layout.rs` reserves to the right
//! of the map: contract, siege weapons, magic star, puzzle grid, gold purse.
//! C reads `GetTime()` inside `hud_draw`; this takes the clock as a field so
//! every panel is testable against a `DrawList` with no window open.
//!
//! NOT ported: `game.hud_visible`. C writes it when an overlay opens and
//! closes (`main.c:974`) and `draw_frame` never reads it. It is dead there,
//! so it is absent here.

use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;

use crate::banner;
use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::input::{Action, Key};
use crate::layout::{Layout, MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};
use crate::sprites::{frame_index, SpriteCatalog};
use crate::views::villain_sprite;

/// Panels stacked down the sidebar, from C's five `blit_panel` calls.
pub const PANEL_COUNT: i32 = 5;

/// The sidebar animations tick at ~2Hz, C's `(int)(GetTime() * 2.0)`.
const ANIM_HZ: f64 = 2.0;

/// The puzzle cover's native pixel size. C reads it off the loaded texture;
/// the draw vocabulary is headless and never learns a texture's size, so the
/// legacy native 9x6 is fixed here -- the same trade `views.rs` makes for
/// `FACE_W`/`FACE_H`.
const COVER_W: i32 = 9;
const COVER_H: i32 = 6;
/// C insets the 5x5 cover block 2px inside the 48x34 panel.
const COVER_INSET: i32 = 2;

pub struct Hud<'a> {
    pub layout: &'a Layout,
    pub palette: &'a Palette,
    pub catalog: &'a SpriteCatalog,
    pub resources: &'a Resources,
    pub game: &'a Game,
    /// Wall-clock seconds, as `get_time()` reports them.
    pub time: f64,
}

impl Hud<'_> {
    /// Panel `index`'s rectangle. C walks `y += CL_TILE_H` after each blit;
    /// this indexes the same stack so a caller can name one panel.
    ///
    /// An associated function rather than a method so tests can ask for a
    /// rectangle without standing up a whole `Hud`.
    pub(crate) fn panel_rect(layout: &Layout, index: i32) -> Rect {
        Rect::new(
            layout.sidebar_x(),
            layout.sidebar_y() + index * layout.tile_h,
            layout.sidebar_w,
            layout.tile_h,
        )
    }

    fn panel(&self, index: i32) -> Rect {
        Self::panel_rect(self.layout, index)
    }

    /// The shared animation counter. C recomputes it per panel from the same
    /// clock, so one value here is the same answer.
    fn tick(&self) -> i32 {
        (self.time * ANIM_HZ) as i32
    }

    pub fn draw(&self, list: &mut DrawList) {
        self.draw_contract(list);
        self.draw_siege(list);
        self.draw_magic(list);
        self.draw_puzzle(list);
        self.draw_gold(list);
    }

    /// The clickable panels: each is the rect [`Hud::panel_rect`] just drew
    /// it on, so a hit cannot drift. Panel 1 (siege) is omitted: siege weapons
    /// are a TOWN transaction, so there is no single overworld key for them
    /// and the panel stays inert -- the same nothing a click on it does today.
    pub fn publish_hits(&self, hits: &mut HitList) {
        for index in [0, 2, 3, 4] {
            hits.push(
                Self::panel_rect(self.layout, index),
                HitTarget::HudPanel(index),
            );
        }
    }

    /// Panel 0: the silhouette always, with the active contract's animated
    /// villain face blitted over the top of it.
    fn draw_contract(&self, list: &mut DrawList) {
        let rect = self.panel(0);
        list.blit(Sprite::HudContractSilhouette, Dst::Stretch(rect));

        if self.game.contract.active_id.is_empty() {
            return;
        }
        let Some(villain) = self
            .resources
            .villain_by_id(self.game.contract.active_id.as_str())
        else {
            return;
        };
        // C bounds the index at 17 before indexing its fixed arrays;
        // `villain_sprite` answers None out of range, which is the same guard.
        if let Some(sprite) = villain_sprite(self.catalog, villain.index, self.tick()) {
            list.blit(sprite, Dst::Stretch(rect));
        }
    }

    /// Panel 1: silhouette when no siege weapon is owned, the animated cart
    /// when one is. C tests the int count, so any non-zero lights it.
    fn draw_siege(&self, list: &mut DrawList) {
        let rect = self.panel(1);
        if self.game.stats.siege_weapons != 0 {
            let frame = frame_index(self.tick(), self.catalog.hud_siege_anim_len()) as i32;
            list.blit(Sprite::HudSiegeAnim { frame }, Dst::Stretch(rect));
        } else {
            list.blit(Sprite::HudSiegeSilhouette, Dst::Stretch(rect));
        }
    }

    /// Panel 2: silhouette until the hero knows magic, then the animated star.
    fn draw_magic(&self, list: &mut DrawList) {
        let rect = self.panel(2);
        if self.game.stats.knows_magic {
            let frame = frame_index(self.tick(), self.catalog.hud_magic_anim_len()) as i32;
            list.blit(Sprite::HudMagicAnim { frame }, Dst::Stretch(rect));
        } else {
            list.blit(Sprite::HudMagicSilhouette, Dst::Stretch(rect));
        }
    }

    /// Panel 3: the empty grid, with a cover stamped over every cell whose
    /// villain is still free or whose artifact is still lost. Covers vanish
    /// as the player progresses, so the grid fills in toward the scepter.
    ///
    /// C re-declares `tables.c`'s `PUZZLE_GRID` here as `PUZZLE_MAP` and
    /// re-tests caught/found inline; the engine already ports both as
    /// `Game::puzzle_cell_revealed`, so the table is not copied again.
    fn draw_puzzle(&self, list: &mut DrawList) {
        let rect = self.panel(3);
        list.blit(Sprite::HudPuzzleGrid, Dst::Stretch(rect));

        for row in 0..5 {
            for col in 0..5 {
                if self.game.puzzle_cell_revealed(row, col) {
                    continue;
                }
                list.blit(
                    Sprite::PuzzleCover,
                    Dst::rect(
                        rect.x + COVER_INSET + col * COVER_W,
                        rect.y + COVER_INSET + row * COVER_H,
                        COVER_W,
                        COVER_H,
                    ),
                );
            }
        }
    }

    /// Panel 4: the purse, with the gold figure right-aligned 2px off the
    /// panel's right edge and 2px up from its bottom.
    fn draw_gold(&self, list: &mut DrawList) {
        let rect = self.panel(4);
        list.blit(Sprite::HudGoldPurse, Dst::Stretch(rect));

        let gold = self.game.stats.gold.to_string();
        list.text(
            &gold,
            rect.x + self.layout.sidebar_w - font::measure_width(&gold) - 2,
            rect.y + self.layout.tile_h - font::GLYPH_H - 2,
            self.palette.clr(Pal::Yellow),
        );
    }
}

/// The `Action` a sidebar panel's click is a shortcut for, or `None` for the
/// inert siege panel. These are shortcuts, not the completeness guarantee --
/// that is the hero action menu below.
pub fn panel_action(index: i32) -> Option<Action> {
    match index {
        0 => Some(Action::ViewContract),
        2 => Some(Action::CastSpell),
        3 => Some(Action::ViewPuzzle),
        4 => Some(Action::ViewCharacter),
        _ => None,
    }
}

/// The hero action menu's entries, in drawn order. Each is exactly one
/// overworld key: `key` is the `Key` that produces `action` in
/// [`crate::input::poll`], `label_key` the `strings.overworld_menu` key, and
/// `default` the shell default label. `Ctrl+Q` (fast quit) is deliberately
/// absent -- it is one left click via the Game Menu's Quit.
pub const OVERWORLD_ACTIONS: [(Action, Key, &str, &str); 17] = [
    (Action::ViewArmy, Key::A, "army", "Army"),
    (Action::ViewCharacter, Key::V, "character", "Character"),
    (Action::ViewContract, Key::I, "contract", "Contract"),
    (Action::ViewPuzzle, Key::P, "puzzle", "Puzzle"),
    (Action::ViewMap, Key::M, "map", "Map"),
    (Action::ViewControls, Key::C, "controls", "Controls"),
    (Action::OptionsMenu, Key::O, "options", "Options"),
    (Action::CastSpell, Key::U, "cast_spell", "Cast Spell"),
    (Action::Search, Key::S, "search", "Search"),
    (Action::EndWeek, Key::W, "end_week", "End Week"),
    (Action::DismissArmy, Key::D, "dismiss_army", "Dismiss Army"),
    (Action::Rest, Key::Kp5, "rest", "Rest"),
    (Action::Fly, Key::F, "fly", "Fly"),
    (Action::Land, Key::L, "land", "Land"),
    (
        Action::NewContinent,
        Key::N,
        "new_continent",
        "New Continent",
    ),
    (Action::SaveQuit, Key::Q, "save_quit", "Save & Quit"),
    (Action::GameMenu, Key::Escape, "game_menu", "Game Menu"),
];

/// The hero's overworld action menu, the click-to-move twin of combat's
/// `combat_action_menu` (`combat.rs`). Shell UI that C does not have. It sits
/// beside the hero's cell (`hero_rect`), clamped inside the map viewport, over
/// a full-map ESC backdrop, and lays out in two columns when 17 rows would not
/// fit the map height (legacy 240x170 splits 9+8), else one column.
pub fn action_menu(
    list: &mut DrawList,
    hits: &mut HitList,
    layout: &Layout,
    palette: &Palette,
    resources: &Resources,
    hero_rect: Rect,
    hover: Option<Action>,
) {
    let pad = 4;
    let row_h = font::GLYPH_H + 2;
    let map = Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h);
    let labels: Vec<&str> = OVERWORLD_ACTIONS
        .iter()
        .map(|(_, _, key, default)| banner::text(resources, "overworld_menu", key, default))
        .collect();
    let widest = labels.iter().map(|l| l.chars().count()).max().unwrap_or(0) as i32;
    let col_w = widest * font::GLYPH_W + 2 * pad;
    let rows = OVERWORLD_ACTIONS.len() as i32;

    // Two columns when one column would not fit the map height -- a pure
    // function of the layout, so legacy splits and modern does not.
    let cols = if rows * row_h + 2 * pad > map.h { 2 } else { 1 };
    let rows_per_col = (rows + cols - 1) / cols;
    let h = rows_per_col * row_h + 2 * pad;
    let w = cols * col_w;

    // The menu sits in the half of the map the hero is NOT in (above or below),
    // centred horizontally and clamped inside the map rect, so it does not
    // cover the hero's tile when it fits. Slice 1 placed it beside the hero
    // like combat's menu does, which covered the hero in legacy -- there the
    // menu is nearly as wide as the map, so a horizontal neighbour always
    // overlaps.
    let hero_cx = hero_rect.x + hero_rect.w / 2;
    let hero_cy = hero_rect.y + hero_rect.h / 2;
    let x = (hero_cx - w / 2).clamp(map.x, map.x + (map.w - w).max(0));
    let y = if hero_cy < map.y + map.h / 2 {
        // Hero in the top half: menu below.
        hero_rect.y + hero_rect.h
    } else {
        // Hero in the bottom half (or dead centre): menu above.
        hero_rect.y - h
    };
    let y = y.clamp(map.y, map.y + (map.h - h).max(0));

    hits.push(map, HitTarget::OverworldMenuClose);
    list.rect(Rect::new(x, y, w, h), palette.clr(Pal::DBlue));
    list.rect_lines(Rect::new(x, y, w, h), palette.clr(Pal::Yellow));
    for (i, ((action, _, _, _), label)) in OVERWORLD_ACTIONS.iter().zip(&labels).enumerate() {
        let col = i as i32 / rows_per_col;
        let row = i as i32 % rows_per_col;
        let rx = x + col * col_w;
        let ry = y + pad + row * row_h;
        let fg = if hover == Some(*action) {
            palette.clr(Pal::Yellow)
        } else {
            palette.clr(Pal::White)
        };
        list.text(label, rx + pad, ry + 1, fg);
        hits.push(
            Rect::new(rx, ry, col_w, row_h),
            HitTarget::OverworldAction(*action),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Keys;
    use openbounty_engine::resources::Resources;

    /// A pack declaring every HUD slot, so `SpriteCatalog::path` resolves and
    /// the blits actually land in the list.
    fn fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "villains": [
                  {"index":0,"id":"varn","name":"Varn","castle":"castle0",
                   "portrait":"murray.png"}
                ],
                "sprites": {
                  "ui": {"puzzle_cover": "cover.png"},
                  "hud": {
                    "contract_silhouette": "contract.png",
                    "siege_silhouette": "siege_off.png",
                    "siege_animation": ["siege0.png", "siege1.png"],
                    "magic_silhouette": "magic_off.png",
                    "magic_animation": ["magic0.png", "magic1.png"],
                    "puzzle_grid": "grid.png",
                    "gold_purse": "purse.png"
                  }
                }}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    /// The face at t=0. A declared portrait makes the catalog derive the
    /// `_00.._NN` animation frames, as C's loader does, so the face is the
    /// first anim frame rather than the still portrait.
    const MURRAY_FACE: Sprite = Sprite::VillainAnim {
        villain: 0,
        frame: 0,
    };

    fn render(
        resources: &Resources,
        layout: &Layout,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) -> DrawList {
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        Hud {
            layout,
            palette: &palette,
            catalog,
            resources,
            game,
            time,
        }
        .draw(&mut list);
        list
    }

    #[test]
    fn the_five_panels_stack_one_tile_apart_down_the_sidebar() {
        let (_, layout, _) = fixtures();
        let hud_x = layout.sidebar_x();
        let hud_y = layout.sidebar_y();
        for index in 0..PANEL_COUNT {
            let rect = Hud::panel_rect(&layout, index);
            assert_eq!(rect.x, hud_x);
            assert_eq!(rect.y, hud_y + index * layout.tile_h);
            assert_eq!(rect.w, layout.sidebar_w);
            assert_eq!(rect.h, layout.tile_h);
        }
        // Legacy: five 34px panels exactly fill the 170px column.
        assert_eq!(PANEL_COUNT * layout.tile_h, layout.sidebar_h());
    }

    #[test]
    fn the_contract_panel_draws_the_silhouette_with_no_contract() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudContractSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 0)));
        // No contract, so no face on top of it.
        assert!(list.blits_of(MURRAY_FACE).is_empty());
    }

    #[test]
    fn the_contract_panel_overlays_the_villain_face_on_the_silhouette() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.active_id.copy_from_str("varn");
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        // C blits the silhouette FIRST, then the face over it.
        let silhouette = list.blits_of(Sprite::HudContractSilhouette);
        let face = list.blits_of(MURRAY_FACE);
        assert_eq!(silhouette.len(), 1);
        assert_eq!(face.len(), 1);
        assert_eq!(face[0], Dst::Stretch(Hud::panel_rect(&layout, 0)));
    }

    #[test]
    fn an_unknown_contract_id_leaves_the_silhouette_bare() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.active_id.copy_from_str("nobody");
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::HudContractSilhouette).len(), 1);
        assert!(list.blits_of(MURRAY_FACE).is_empty());
    }

    #[test]
    fn the_siege_panel_shows_the_silhouette_until_a_weapon_is_owned() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudSiegeSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 1)));
        assert!(list.blits_of(Sprite::HudSiegeAnim { frame: 0 }).is_empty());
    }

    #[test]
    fn the_siege_panel_animates_once_a_weapon_is_owned() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.siege_weapons = 1;

        // Two frames declared, ~2Hz: t=0.0 is frame 0, t=0.5 is frame 1.
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        assert_eq!(
            list.blits_of(Sprite::HudSiegeAnim { frame: 0 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 1))]
        );
        assert!(list.blits_of(Sprite::HudSiegeSilhouette).is_empty());

        let list = render(&resources, &layout, &catalog, &game, 0.5);
        assert_eq!(
            list.blits_of(Sprite::HudSiegeAnim { frame: 1 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 1))]
        );
    }

    #[test]
    fn the_magic_panel_shows_the_silhouette_until_the_hero_knows_magic() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudMagicSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 2)));
    }

    #[test]
    fn the_magic_panel_animates_once_the_hero_knows_magic() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.knows_magic = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(
            list.blits_of(Sprite::HudMagicAnim { frame: 0 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 2))]
        );
        assert!(list.blits_of(Sprite::HudMagicSilhouette).is_empty());
    }

    /// C tests `if (g->stats.siege_weapons)`, an int, so any non-zero count
    /// lights the panel -- not just 1.
    #[test]
    fn several_siege_weapons_still_light_the_panel_once() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.siege_weapons = 3;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::HudSiegeAnim { frame: 0 }).len(), 1);
    }

    #[test]
    fn the_puzzle_panel_covers_all_twenty_five_cells_at_the_start() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let grid = list.blits_of(Sprite::HudPuzzleGrid);
        assert_eq!(grid.len(), 1);
        assert_eq!(grid[0], Dst::Stretch(Hud::panel_rect(&layout, 3)));
        // Nothing caught or found yet, so every cell is still covered.
        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 25);
    }

    #[test]
    fn catching_a_villain_lifts_exactly_one_cover() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.villains_caught[0] = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 24);
    }

    #[test]
    fn finding_an_artifact_lifts_exactly_one_cover() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.artifacts.found[0] = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 24);
    }

    /// The covers tile the panel from a 2px inset, 9x6 each, row-major.
    #[test]
    fn the_covers_tile_the_panel_from_the_inset() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let panel = Hud::panel_rect(&layout, 3);

        let covers = list.blits_of(Sprite::PuzzleCover);
        // First cell: top-left, inset by 2 on both axes.
        assert_eq!(
            covers[0],
            Dst::Stretch(Rect::new(
                panel.x + COVER_INSET,
                panel.y + COVER_INSET,
                COVER_W,
                COVER_H
            ))
        );
        // Last cell: row 4, col 4.
        assert_eq!(
            covers[24],
            Dst::Stretch(Rect::new(
                panel.x + COVER_INSET + 4 * COVER_W,
                panel.y + COVER_INSET + 4 * COVER_H,
                COVER_W,
                COVER_H
            ))
        );
    }

    #[test]
    fn the_gold_panel_draws_the_purse_and_the_figure() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.gold = 7500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let purse = list.blits_of(Sprite::HudGoldPurse);
        assert_eq!(purse.len(), 1);
        assert_eq!(purse[0], Dst::Stretch(Hud::panel_rect(&layout, 4)));
        assert!(list.has_text_containing("7500"));
    }

    /// C right-aligns the figure 2px off the panel's right edge and sits it
    /// 2px up from the bottom, so a wider number grows leftward.
    #[test]
    fn the_gold_figure_is_right_aligned_in_the_panel() {
        let (resources, layout, catalog) = fixtures();
        let panel = Hud::panel_rect(&layout, 4);

        let mut game = Game::default();
        game.stats.gold = 7500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let (x, y, text) = list.texts().pop().expect("a gold figure");
        assert_eq!(text, "7500");
        assert_eq!(
            x,
            panel.x + layout.sidebar_w - font::measure_width("7500") - 2
        );
        assert_eq!(y, panel.y + layout.tile_h - font::GLYPH_H - 2);

        // One more digit starts one glyph further left, same baseline.
        game.stats.gold = 17500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let (wide_x, wide_y, _) = list.texts().pop().expect("a gold figure");
        assert_eq!(wide_x, x - font::GLYPH_W);
        assert_eq!(wide_y, y);
    }

    /// Zero gold still draws; C formats the int unconditionally.
    #[test]
    fn zero_gold_still_draws_the_figure() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert!(list.has_text_containing("0"));
    }

    #[test]
    fn panel_action_maps_each_clickable_panel_to_its_key_action() {
        assert_eq!(panel_action(0), Some(Action::ViewContract));
        assert_eq!(panel_action(2), Some(Action::CastSpell));
        assert_eq!(panel_action(3), Some(Action::ViewPuzzle));
        assert_eq!(panel_action(4), Some(Action::ViewCharacter));
        // The siege panel and out-of-range indices are inert.
        assert_eq!(panel_action(1), None);
        assert_eq!(panel_action(5), None);
        assert_eq!(panel_action(-1), None);
    }

    #[test]
    fn each_clickable_panel_publishes_the_rect_it_is_drawn_on() {
        let (resources, layout, catalog) = fixtures();
        let palette = Palette::fallback();
        let game = Game::default();
        let hud = Hud {
            layout: &layout,
            palette: &palette,
            catalog: &catalog,
            resources: &resources,
            game: &game,
            time: 0.0,
        };
        let mut hits = HitList::new();
        hud.publish_hits(&mut hits);

        assert_eq!(
            hits.regions().iter().map(|r| r.target).collect::<Vec<_>>(),
            vec![
                HitTarget::HudPanel(0),
                HitTarget::HudPanel(2),
                HitTarget::HudPanel(3),
                HitTarget::HudPanel(4),
            ]
        );
        for index in [0, 2, 3, 4] {
            let region = hits
                .regions()
                .iter()
                .find(|r| r.target == HitTarget::HudPanel(index))
                .expect("panel published");
            // Anti-drift: the hit region is the rect the panel drew on.
            assert_eq!(region.rect, Hud::panel_rect(&layout, index));
        }
    }

    /// Every action-menu row is exactly the overworld key it labels: a click
    /// resolves to the same `Action` `input::poll` produces for that key, so
    /// both paths converge on the one `match input.action` in `session.rs`.
    #[test]
    fn every_action_menu_row_is_exactly_its_key() {
        for (action, key, _, _) in OVERWORLD_ACTIONS {
            assert_eq!(
                crate::input::poll(&Keys::pressed(&[key])).action,
                action,
                "row {action:?} should match key {key:?}"
            );
        }
    }

    /// Anti-drift for the hero action menu: the backdrop is the full map (and
    /// first, so rows win), each row's region holds its label, and every row
    /// stays inside the map. Legacy is 240x170, so 17 rows split two columns.
    #[test]
    fn the_action_menu_splits_two_columns_in_legacy_and_anti_drifts() {
        let (resources, layout, _) = fixtures();
        let palette = Palette::fallback();
        let map = Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h);

        // The hero cell in the centre of the legacy viewport (radius 2).
        let hero = Rect::new(
            MAP_X + 2 * layout.tile_w,
            MAP_Y + 2 * layout.tile_h,
            layout.tile_w,
            layout.tile_h,
        );
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        action_menu(
            &mut list, &mut hits, &layout, &palette, &resources, hero, None,
        );

        assert_eq!(hits.regions()[0].target, HitTarget::OverworldMenuClose);
        assert_eq!(hits.regions()[0].rect, map);
        assert_eq!(hits.regions().len(), 1 + OVERWORLD_ACTIONS.len());

        for (action, _, _, label) in OVERWORLD_ACTIONS {
            let region = hits
                .regions()
                .iter()
                .find(|r| r.target == HitTarget::OverworldAction(action))
                .expect("row published");
            let r = region.rect;
            assert!(
                r.x >= map.x && r.x + r.w <= map.x + map.w,
                "{action:?} off map x"
            );
            assert!(
                r.y >= map.y && r.y + r.h <= map.y + map.h,
                "{action:?} off map y"
            );
            let (tx, ty, _) = list
                .texts()
                .into_iter()
                .find(|(_, _, t)| *t == label)
                .unwrap_or_else(|| panic!("{action:?} label {label:?} drawn"));
            assert!(tx >= r.x && tx < r.x + r.w && ty >= r.y && ty < r.y + r.h);
            assert_eq!(
                hits.hit(r.x + r.w / 2, r.y + r.h / 2),
                Some(HitTarget::OverworldAction(action))
            );
        }

        // Two columns: Army (row 0) and End Week (row 9) share a column count
        // of 9+8, so the second column's first row sits beside the first
        // column's first row at the same height.
        let army = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::OverworldAction(Action::ViewArmy))
            .unwrap()
            .rect;
        let end_week = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::OverworldAction(Action::EndWeek))
            .unwrap()
            .rect;
        assert_eq!(army.y, end_week.y, "column 2 starts at the same height");
        assert!(army.x < end_week.x, "the two columns sit side by side");
    }

    /// The menu's panel rect, reconstructed from the row regions it published:
    /// the rows sit `pad` inside the panel, so widen the bounding box by that
    /// on the top and bottom edges.
    fn menu_panel_rect(hits: &HitList) -> Rect {
        let mut x0 = i32::MAX;
        let mut y0 = i32::MAX;
        let mut x1 = i32::MIN;
        let mut y1 = i32::MIN;
        for region in hits.regions() {
            if matches!(region.target, HitTarget::OverworldAction(_)) {
                x0 = x0.min(region.rect.x);
                y0 = y0.min(region.rect.y);
                x1 = x1.max(region.rect.x + region.rect.w);
                y1 = y1.max(region.rect.y + region.rect.h);
            }
        }
        const PAD: i32 = 4;
        Rect::new(x0, y0 - PAD, x1 - x0, (y1 - y0) + 2 * PAD)
    }

    /// Slice 2's fix: the menu sits in the half of the map the hero is NOT in,
    /// so it never covers the hero's tile when that is possible. In legacy the
    /// centred (middle) hero leaves 68px above and below, less than the menu's
    /// 98px height, so a full clearance there is geometrically impossible and
    /// the menu merely overlaps the hero's edge; the top and bottom rows --
    /// where a camera clamp can actually push the hero off centre -- clear it.
    #[test]
    fn the_action_menu_does_not_cover_the_hero_tile_when_it_fits() {
        let (resources, layout, _) = fixtures();
        let palette = Palette::fallback();
        let map = Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h);

        let hero_at = |row: i32| {
            Rect::new(
                MAP_X + 2 * layout.tile_w,
                MAP_Y + row * layout.tile_h,
                layout.tile_w,
                layout.tile_h,
            )
        };

        for row in [0, 4] {
            let hero = hero_at(row);
            let mut list = DrawList::new();
            let mut hits = HitList::new();
            action_menu(
                &mut list, &mut hits, &layout, &palette, &resources, hero, None,
            );

            let panel = menu_panel_rect(&hits);
            assert!(
                panel.x >= map.x && panel.x + panel.w <= map.x + map.w,
                "menu stays inside the map (x)"
            );
            assert!(
                panel.y >= map.y && panel.y + panel.h <= map.y + map.h,
                "menu stays inside the map (y)"
            );
            let overlaps = panel.x < hero.x + hero.w
                && panel.x + panel.w > hero.x
                && panel.y < hero.y + hero.h
                && panel.y + panel.h > hero.y;
            assert!(!overlaps, "menu must not cover the hero at row {row}");
        }
    }

    /// In a tall modern map one column fits, so the menu does not split.
    #[test]
    fn the_action_menu_stays_one_column_in_a_tall_map() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "modern", "tile_w": 96, "tile_h": 96,
                            "tiles_w": 7, "tiles_h": 7}, "troops": []}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let palette = Palette::fallback();
        let hero = Rect::new(MAP_X + 3 * 96, MAP_Y + 3 * 96, 96, 96);
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        action_menu(
            &mut list, &mut hits, &layout, &palette, &resources, hero, None,
        );

        let army = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::OverworldAction(Action::ViewArmy))
            .unwrap()
            .rect;
        let menu = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::OverworldAction(Action::GameMenu))
            .unwrap()
            .rect;
        assert_eq!(army.x, menu.x, "one column shares an x");
        assert!(army.y < menu.y, "rows stack downward");
    }

    /// The pack may rename the rows under `strings.overworld_menu`, and the
    /// shell defaults kick in when it does not.
    #[test]
    fn the_action_menu_labels_come_from_the_pack() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "strings": {"overworld_menu": {"search": "Dig"}}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let palette = Palette::fallback();
        let hero = Rect::new(MAP_X, MAP_Y, layout.tile_w, layout.tile_h);
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        action_menu(
            &mut list, &mut hits, &layout, &palette, &resources, hero, None,
        );

        assert!(list.texts().iter().any(|(_, _, t)| *t == "Dig"));
        assert!(list.texts().iter().any(|(_, _, t)| *t == "Army"));
    }
}
