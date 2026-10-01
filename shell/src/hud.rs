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
use crate::layout::Layout;
use crate::palette::Palette;
use crate::sprites::{frame_index, SpriteCatalog};
use crate::views::villain_sprite;

/// Panels stacked down the sidebar, from C's five `blit_panel` calls.
pub const PANEL_COUNT: i32 = 5;

/// The sidebar animations tick at ~2Hz, C's `(int)(GetTime() * 2.0)`.
const ANIM_HZ: f64 = 2.0;

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
                  {"index":0,"id":"murray","name":"Murray","castle":"castle0",
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
        game.contract.active_id.copy_from_str("murray");
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
}
