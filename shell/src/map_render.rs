//! The overworld viewport, porting `src/map_render.c`.
//!
//! The hero is held in the middle of the viewport except at a map edge, where
//! the camera clamps and he walks towards the corner instead. Everything here
//! emits draw commands, so a test reads the list back rather than a screen.

use openbounty_engine::fog::Fog;
use openbounty_engine::game::{Game, Mount, TravelMode, GAME_ARMY_SLOTS};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::tile::Interact;

use crate::draw::{Color, DrawList, Dst, HeroAnim, Rect, Sprite};
use crate::layout::{Layout, MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
use crate::sprites::SpriteCatalog;

/// Fog-edge darkening: three two-pixel strips, each half as opaque as the
/// one before, drawn inside a seen tile on every edge facing an unseen one.
const FOG_STRIPS: i32 = 3;
const FOG_STRIP_PX: i32 = 2;
const FOG_ALPHA_TOP: u8 = 128;

/// Cardinal neighbours, in C's order: north, south, west, east.
const NEIGHBOURS: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

/// Everything the viewport needs for one frame.
pub struct MapScene<'a> {
    pub layout: &'a Layout,
    pub catalog: &'a SpriteCatalog,
    pub palette: &'a Palette,
    pub resources: &'a Resources,
    pub game: &'a Game,
    pub map: &'a Map,
    pub fog: &'a Fog,
    /// Free-running animation counter; each strip folds it onto its own
    /// declared cycle, so a six-frame walk and a four-frame boat coexist.
    pub tick: i32,
    /// Whether the hero is mid-step. He holds still between steps, and a pack
    /// that ships an idle animation keeps breathing while he does.
    pub moving: bool,
}

impl MapScene<'_> {
    /// Top-left visible tile. The hero sits at the viewport's centre until the
    /// camera runs into a map edge and clamps.
    ///
    /// DIVERGENCE, deliberate: C derives one `RADIUS` from the viewport WIDTH
    /// and uses it on both axes, so a pack with a non-square viewport would
    /// sit off-centre vertically. This uses each axis's own radius. Every
    /// mode that exists is square (5x5 legacy, 7x7 modern), so the two agree
    /// for any pack today; the per-axis form is what C's own comment
    /// describes.
    pub fn camera(&self) -> (i32, i32) {
        let clamp = |pos: i32, radius: i32, tiles: i32, extent: usize| {
            (pos - radius).clamp(0, (extent as i32 - tiles).max(0))
        };
        (
            clamp(
                self.game.position.x,
                self.layout.viewport_radius_x(),
                self.layout.tiles_w,
                self.map.width,
            ),
            clamp(
                self.game.position.y,
                self.layout.viewport_radius_y(),
                self.layout.tiles_h,
                self.map.height,
            ),
        )
    }

    fn map_rect(&self) -> Rect {
        Rect::new(MAP_X, MAP_Y, self.layout.map_w, self.layout.map_h)
    }

    /// Where a viewport cell lands, in logical pixels.
    fn cell(&self, tx: i32, ty: i32) -> Dst {
        Dst::rect(
            MAP_X + tx * self.layout.tile_w,
            MAP_Y + ty * self.layout.tile_h,
            self.layout.tile_w,
            self.layout.tile_h,
        )
    }

    pub fn draw(&self, list: &mut DrawList) {
        let (cam_x, cam_y) = self.camera();

        // Clip so a tile at the map boundary cannot spill into the chrome.
        list.clip_begin(self.map_rect());
        // Unseen ground is black, which is also what shows through wherever a
        // tile has no art.
        list.rect(self.map_rect(), self.palette.clr(Pal::Black));

        self.draw_tiles(list, cam_x, cam_y);
        self.draw_idle_boat(list, cam_x, cam_y);
        self.draw_hero(list, cam_x, cam_y);
        self.draw_fog_edges(list, cam_x, cam_y);

        list.clip_end();
    }

    fn draw_tiles(&self, list: &mut DrawList, cam_x: i32, cam_y: i32) {
        for ty in 0..self.layout.tiles_h {
            for tx in 0..self.layout.tiles_w {
                let (mx, my) = (cam_x + tx, cam_y + ty);
                if !self.fog.seen(mx, my) {
                    continue;
                }
                let Some(tile) = self.map.tile_i32(mx, my) else {
                    continue;
                };
                let dst = self.cell(tx, ty);

                // A wandering army shows its lead troop rather than a generic
                // silhouette. That sprite is transparent, unlike the opaque
                // wandering-army tile, so the terrain underneath is painted
                // first or the foe stands on the black map fill.
                let foe_sprite = (tile.interactive == Interact::Foe)
                    .then(|| self.foe_sprite(tile.id.as_str()))
                    .flatten();
                if let Some(foe) = foe_sprite {
                    list.blit(Sprite::tile(tile.terrain.as_str()), dst);
                    list.blit(foe, dst);
                    continue;
                }
                if !tile.art.is_empty() {
                    list.blit(Sprite::tile(tile.art.as_str()), dst);
                }
            }
        }
    }

    /// The lead troop of the foe standing on a tile, or `None` when the foe
    /// or its troop cannot be resolved and the generic tile art should show.
    fn foe_sprite(&self, placement_id: &str) -> Option<Sprite> {
        if placement_id.is_empty() {
            return None;
        }
        let foe = self.game.foes[..self.game.foe_count]
            .iter()
            .find(|foe| foe.placement_id.as_str() == placement_id)?;
        let stack = foe
            .garrison
            .iter()
            .find(|unit| !unit.id.is_empty() && unit.count > 0)?;
        let troop = self.resources.troop_by_id(stack.id.as_str())?;
        self.troop_sprite(troop.index)
    }

    /// A troop's animated frame, falling back to its still sprite.
    fn troop_sprite(&self, index: i32) -> Option<Sprite> {
        let index = usize::try_from(index).ok()?;
        let frames = self.catalog.troop_anim_len(index);
        if frames > 0 {
            return Some(Sprite::TroopAnim {
                troop: index,
                frame: crate::sprites::frame_index(self.tick, frames) as i32,
            });
        }
        // `troop_frame` reports whether a still sprite exists at all, so a
        // troop with no art draws nothing rather than an empty slot.
        self.catalog
            .troop_frame(index, self.tick)
            .map(|_| Sprite::TroopStill(index))
    }

    /// A boat the hero left behind, which only shows in the zone he is in --
    /// one parked by a gate spell must not bleed through from another.
    fn draw_idle_boat(&self, list: &mut DrawList, cam_x: i32, cam_y: i32) {
        let boat = &self.game.boat;
        if !boat.has_boat || self.game.travel_mode != TravelMode::Walk {
            return;
        }
        if !boat.zone.is_empty() && boat.zone.as_str() != self.game.position.zone.as_str() {
            return;
        }
        if !self.fog.seen(boat.x, boat.y) {
            return;
        }
        let (bx, by) = (boat.x - cam_x, boat.y - cam_y);
        if bx < 0 || by < 0 || bx >= self.layout.tiles_w || by >= self.layout.tiles_h {
            return;
        }
        // A parked boat sits still: frame 0, facing south, never mirrored.
        list.blit(
            Sprite::Hero {
                anim: HeroAnim::Boat,
                facing: openbounty_engine::tables::Facing::South,
                tick: 0,
            },
            self.cell(bx, by),
        );
    }

    fn draw_hero(&self, list: &mut DrawList, cam_x: i32, cam_y: i32) {
        let sailing = self.game.travel_mode == TravelMode::Boat;
        // The hero holds still between steps, so a pack that shipped an idle
        // set breathes instead of freezing on frame 0.
        let anim = if sailing {
            HeroAnim::Boat
        } else if !self.moving && self.catalog.hero(HeroAnim::Idle).is_present() {
            HeroAnim::Idle
        } else {
            HeroAnim::Walk
        };

        let facing = self.game.position.facing;
        let mut sprite = Sprite::Hero {
            anim,
            facing,
            tick: self.tick,
        };
        // Whether to mirror is the ANIMATION's business, not the hero's: a
        // pack with four authored facings is drawn unflipped, while a
        // single-strip pack still mirrors going west.
        let mut mirror = self
            .catalog
            .hero(anim)
            .frame(facing, self.tick)
            .map(|(_, mirror)| mirror)
            .unwrap_or(false);

        // Flying shows what the hero is riding: a knight on horseback gliding
        // over a mountain looked wrong. It falls back to the walking hero when
        // the army is empty or the lead troop has no sprite.
        if !sailing && self.game.character.mount == Mount::Fly {
            if let Some(lead) = self.lead_troop_sprite() {
                sprite = lead;
                // Troop sprites are single-strip, so flight goes back to the
                // mirror whatever the hero's own art declares.
                mirror = facing == openbounty_engine::tables::Facing::West;
            }
        }

        let (hx, hy) = (self.game.position.x - cam_x, self.game.position.y - cam_y);
        list.blit_flipped(sprite, self.cell(hx, hy), mirror);
    }

    /// The first non-empty army slot's sprite. C reads only that slot and
    /// stops, whether or not it resolved to anything.
    fn lead_troop_sprite(&self) -> Option<Sprite> {
        let stack = self.game.army[..GAME_ARMY_SLOTS]
            .iter()
            .find(|stack| !stack.id.is_empty() && stack.count > 0)?;
        let troop = self.resources.troop_by_id(stack.id.as_str())?;
        self.troop_sprite(troop.index)
    }

    /// Darken the edges of seen tiles that face unseen ones, so the explored
    /// area fades out instead of ending on a hard line.
    fn draw_fog_edges(&self, list: &mut DrawList, cam_x: i32, cam_y: i32) {
        for ty in 0..self.layout.tiles_h {
            for tx in 0..self.layout.tiles_w {
                let (mx, my) = (cam_x + tx, cam_y + ty);
                if !self.fog.seen(mx, my) {
                    continue;
                }
                let px = MAP_X + tx * self.layout.tile_w;
                let py = MAP_Y + ty * self.layout.tile_h;
                for (dx, dy) in NEIGHBOURS {
                    if self.fog.seen(mx + dx, my + dy) {
                        continue;
                    }
                    for step in 0..FOG_STRIPS {
                        let alpha = FOG_ALPHA_TOP >> step;
                        let inset = step * FOG_STRIP_PX;
                        let rect = match (dx, dy) {
                            (0, -1) => Rect::new(px, py + inset, self.layout.tile_w, FOG_STRIP_PX),
                            (0, 1) => Rect::new(
                                px,
                                py + self.layout.tile_h - inset - FOG_STRIP_PX,
                                self.layout.tile_w,
                                FOG_STRIP_PX,
                            ),
                            (-1, 0) => Rect::new(px + inset, py, FOG_STRIP_PX, self.layout.tile_h),
                            _ => Rect::new(
                                px + self.layout.tile_w - inset - FOG_STRIP_PX,
                                py,
                                FOG_STRIP_PX,
                                self.layout.tile_h,
                            ),
                        };
                        list.rect(rect, Color::rgba(0, 0, 0, alpha));
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::map::Tile;
    use openbounty_engine::tables::Facing;
    use openbounty_engine::tile::Terrain;

    use crate::draw::DrawCmd;

    fn pack() -> Resources {
        Resources::from_json_str(
            r#"{
              "render": {"mode": "legacy"},
              "world": {"starting_zone": "home"},
              "zones": [{"id": "home"}],
              "sprites": {"hero": {"walk": ["walk0.png", "walk1.png"],
                                   "boat": ["boat0.png"]}},
              "troops": [{"index":0,"id":"ogres","skill_level":1,"hit_points":1,
                          "move_rate":1,"melee":[1,1],"ranged":[0,0,0],
                          "recruit_cost":1,"spoils_factor":1,"abilities":"",
                          "morale_group":"A","sprite":"ogres.png"}]
            }"#,
        )
        .expect("fixture parses")
    }

    /// Mark an inclusive rectangle of tiles seen.
    ///
    /// `Fog::reveal` cannot be used for a partial reveal: it IGNORES its
    /// radius and always stamps a fixed 5x5, which is exactly the legacy
    /// viewport. Writing the field is the only way to get a viewport that is
    /// partly fogged.
    fn reveal_rect(fog: &mut Fog, x0: i32, y0: i32, x1: i32, y1: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                fog.seen[y as usize][x as usize] = true;
            }
        }
    }

    /// An 11x11 of plain grass, fully revealed, hero in the middle.
    fn scene_parts() -> (Layout, SpriteCatalog, Palette, Resources, Game, Map, Fog) {
        let resources = pack();
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        let map = Map::from_tiles(11, 11, vec![Tile::plain_grass(); 121]).expect("11x11");
        let mut fog = Fog::new();
        reveal_rect(&mut fog, 0, 0, 10, 10);
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 5;
        game.position.y = 5;
        (
            layout,
            catalog,
            Palette::fallback(),
            resources,
            game,
            map,
            fog,
        )
    }

    macro_rules! scene {
        ($layout:ident, $catalog:ident, $palette:ident, $resources:ident,
         $game:ident, $map:ident, $fog:ident) => {
            MapScene {
                layout: &$layout,
                catalog: &$catalog,
                palette: &$palette,
                resources: &$resources,
                game: &$game,
                map: &$map,
                fog: &$fog,
                tick: 0,
                moving: false,
            }
        };
    }

    #[test]
    fn the_camera_centres_the_hero_until_it_hits_an_edge() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();

        // Mid-map: the hero sits two tiles in from each side of the 5x5.
        let scene = scene!(layout, catalog, palette, resources, game, map, fog);
        assert_eq!(scene.camera(), (3, 3));

        // At the top-left corner the camera cannot go negative, so the hero
        // walks towards the corner instead of the view scrolling past it.
        game.position.x = 0;
        game.position.y = 1;
        let scene = scene!(layout, catalog, palette, resources, game, map, fog);
        assert_eq!(scene.camera(), (0, 0));

        // Same at the far edge: 11 wide minus a 5-wide viewport stops at 6.
        game.position.x = 10;
        game.position.y = 10;
        let scene = scene!(layout, catalog, palette, resources, game, map, fog);
        assert_eq!(scene.camera(), (6, 6));
    }

    #[test]
    fn a_map_smaller_than_the_viewport_does_not_scroll_backwards() {
        // 3x3 inside a 5x5 viewport: the clamp's upper bound would go
        // negative, which must not push the camera off the map.
        let resources = pack();
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        let map = Map::from_tiles(3, 3, vec![Tile::plain_grass(); 9]).expect("3x3");
        let fog = Fog::new();
        let palette = Palette::fallback();
        let mut game = Game::default();
        game.position.x = 2;
        game.position.y = 2;

        let scene = scene!(layout, catalog, palette, resources, game, map, fog);
        assert_eq!(scene.camera(), (0, 0));
    }

    #[test]
    fn the_viewport_is_clipped_and_floored_before_anything_is_drawn() {
        let (layout, catalog, palette, resources, game, map, fog) = scene_parts();
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        let rect = Rect::new(MAP_X, MAP_Y, layout.map_w, layout.map_h);
        assert_eq!(list.as_slice()[0], DrawCmd::ClipBegin(rect));
        assert_eq!(
            list.as_slice()[1],
            DrawCmd::Rect {
                rect,
                color: palette.clr(Pal::Black)
            }
        );
        assert_eq!(list.clip_balance(), 0, "the clip must always be closed");
    }

    #[test]
    fn each_visible_tile_is_drawn_at_its_own_cell() {
        let (layout, catalog, palette, resources, game, map, fog) = scene_parts();
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        // 5x5 of grass, all seen.
        let grass = list.blits_of(Sprite::tile("grass"));
        assert_eq!(grass.len(), 25);
        // The first cell is the viewport origin; cells step by a whole tile.
        assert_eq!(grass[0], Dst::rect(MAP_X, MAP_Y, 48, 34));
        assert_eq!(grass[1], Dst::rect(MAP_X + 48, MAP_Y, 48, 34));
        assert_eq!(grass[5], Dst::rect(MAP_X, MAP_Y + 34, 48, 34));
    }

    #[test]
    fn an_unseen_tile_draws_nothing_at_all() {
        let (layout, catalog, palette, resources, game, map, _) = scene_parts();
        // A 3x3 around the hero, so the 5x5 viewport has a ring of unseen
        // tiles around a seen core.
        let mut fog = Fog::new();
        reveal_rect(&mut fog, 4, 4, 6, 6);
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        assert_eq!(list.blits_of(Sprite::tile("grass")).len(), 9);
    }

    #[test]
    fn the_hero_is_drawn_on_his_own_cell_and_mirrored_going_west() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();
        game.position.facing = Facing::West;
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        let hero = Sprite::Hero {
            anim: HeroAnim::Walk,
            facing: Facing::West,
            tick: 0,
        };
        // Centred: two tiles in from the viewport's top-left.
        assert_eq!(
            list.blits_of(hero),
            vec![Dst::rect(MAP_X + 96, MAP_Y + 68, 48, 34)]
        );
        // The pack ships one strip, so west is drawn flipped.
        let flipped = list.as_slice().iter().any(
            |cmd| matches!(cmd, DrawCmd::Blit { sprite, flip_x, .. } if *sprite == hero && *flip_x),
        );
        assert!(flipped, "a single-strip hero mirrors going west");
    }

    #[test]
    fn the_hero_sails_when_he_is_in_the_boat() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();
        game.travel_mode = TravelMode::Boat;
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        assert!(!list
            .blits_of(Sprite::Hero {
                anim: HeroAnim::Boat,
                facing: Facing::South,
                tick: 0
            })
            .is_empty());
        assert!(list
            .blits_of(Sprite::Hero {
                anim: HeroAnim::Walk,
                facing: Facing::South,
                tick: 0
            })
            .is_empty());
    }

    #[test]
    fn a_parked_boat_only_shows_in_the_zone_the_hero_is_looking_at() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();
        game.boat.has_boat = true;
        game.boat.x = 4;
        game.boat.y = 5;
        game.boat.zone.copy_from_str("home");
        let parked = Sprite::Hero {
            anim: HeroAnim::Boat,
            facing: Facing::South,
            tick: 0,
        };

        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);
        assert_eq!(
            list.blits_of(parked),
            vec![Dst::rect(MAP_X + 48, MAP_Y + 68, 48, 34)]
        );

        // A boat left in another zone by a gate spell must not bleed through
        // just because the coordinates match here.
        game.boat.zone.copy_from_str("away");
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);
        assert!(list.blits_of(parked).is_empty());
    }

    #[test]
    fn a_flying_hero_shows_the_troop_he_is_riding() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();
        game.character.mount = Mount::Fly;
        game.army[0].id.copy_from_str("ogres");
        game.army[0].count = 3;

        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        // The lead troop replaces the hero entirely -- a knight on horseback
        // gliding over a mountain looked wrong.
        assert!(!list.blits_of(Sprite::TroopStill(0)).is_empty());
        assert!(list
            .blits_of(Sprite::Hero {
                anim: HeroAnim::Walk,
                facing: Facing::South,
                tick: 0
            })
            .is_empty());
    }

    #[test]
    fn a_flying_hero_with_no_army_stays_the_walking_hero() {
        let (layout, catalog, palette, resources, mut game, map, fog) = scene_parts();
        game.character.mount = Mount::Fly;

        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        assert!(!list
            .blits_of(Sprite::Hero {
                anim: HeroAnim::Walk,
                facing: Facing::South,
                tick: 0
            })
            .is_empty());
    }

    #[test]
    fn a_foe_shows_its_lead_troop_over_the_ground_it_stands_on() {
        let (layout, catalog, palette, resources, mut game, mut map, fog) = scene_parts();
        let tile = map.tile_mut_i32(6, 5).expect("in bounds");
        tile.interactive = Interact::Foe;
        tile.id.copy_from_str("foe0");
        tile.art.copy_from_str("wandering_army");
        tile.terrain = Terrain::Forest;

        game.foes[0].alive = true;
        game.foes[0].zone.copy_from_str("home");
        game.foes[0].x = 6;
        game.foes[0].y = 5;
        game.foes[0].placement_id.copy_from_str("foe0");
        game.foes[0].garrison[0].id.copy_from_str("ogres");
        game.foes[0].garrison[0].count = 4;
        game.foe_count = 1;

        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        // The troop sprite is transparent, unlike the opaque wandering-army
        // tile it replaces, so the terrain is painted under it first.
        let cell = Dst::rect(MAP_X + 144, MAP_Y + 68, 48, 34);
        assert_eq!(list.blits_of(Sprite::tile("forest")), vec![cell]);
        assert_eq!(list.blits_of(Sprite::TroopStill(0)), vec![cell]);
        // The generic tile art is not drawn at all.
        assert!(list.blits_of(Sprite::tile("wandering_army")).is_empty());
    }

    #[test]
    fn a_foe_whose_troop_cannot_be_resolved_keeps_the_generic_tile() {
        let (layout, catalog, palette, resources, game, mut map, fog) = scene_parts();
        let tile = map.tile_mut_i32(6, 5).expect("in bounds");
        tile.interactive = Interact::Foe;
        tile.id.copy_from_str("ghost_foe");
        tile.art.copy_from_str("wandering_army");

        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        assert_eq!(
            list.blits_of(Sprite::tile("wandering_army")),
            vec![Dst::rect(MAP_X + 144, MAP_Y + 68, 48, 34)]
        );
    }

    #[test]
    fn the_fog_edge_fades_over_three_strips_facing_each_unseen_neighbour() {
        let (layout, catalog, palette, resources, game, map, _) = scene_parts();
        // One tile only, so it has four unseen neighbours.
        let mut fog = Fog::new();
        reveal_rect(&mut fog, 5, 5, 5, 5);
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        let strips: Vec<_> = list
            .as_slice()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Rect { rect, color } if color.a != 255 => Some((*rect, color.a)),
                _ => None,
            })
            .collect();
        // Four edges, three strips each, halving in opacity outward.
        assert_eq!(strips.len(), 12);
        assert_eq!(
            strips.iter().map(|(_, a)| *a).collect::<Vec<_>>(),
            vec![128, 64, 32, 128, 64, 32, 128, 64, 32, 128, 64, 32]
        );
        // The hero's cell is at (2,2) of the viewport: 16+96, 22+68.
        let (px, py) = (MAP_X + 96, MAP_Y + 68);
        // North edge: strips march DOWN from the top of the tile.
        assert_eq!(strips[0].0, Rect::new(px, py, 48, 2));
        assert_eq!(strips[1].0, Rect::new(px, py + 2, 48, 2));
        // South edge: strips march UP from the bottom.
        assert_eq!(strips[3].0, Rect::new(px, py + 32, 48, 2));
        // West edge: strips march RIGHT from the left.
        assert_eq!(strips[6].0, Rect::new(px, py, 2, 34));
        // East edge: strips march LEFT from the right.
        assert_eq!(strips[9].0, Rect::new(px + 46, py, 2, 34));
    }

    #[test]
    fn a_fully_explored_viewport_has_no_fog_edges_in_it() {
        // The whole 11x11 is seen, so no tile in the viewport borders an
        // unseen one -- not even the viewport's own edge tiles.
        let (layout, catalog, palette, resources, game, map, fog) = scene_parts();
        let mut list = DrawList::new();
        scene!(layout, catalog, palette, resources, game, map, fog).draw(&mut list);

        assert!(!list
            .as_slice()
            .iter()
            .any(|cmd| matches!(cmd, DrawCmd::Rect { color, .. } if color.a != 255)));
    }
}
