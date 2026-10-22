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

use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::layout::Layout;
use crate::palette::{Pal, Palette};
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

#[cfg(test)]
mod tests {
    use super::*;
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
}
