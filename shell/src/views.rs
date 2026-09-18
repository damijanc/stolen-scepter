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
    CastleOwnerKind, Game, GAME_ARMY_SLOTS, GAME_CASTLES, GAME_CONTINENTS,
};
use openbounty_engine::map::Map;
use openbounty_engine::resources::{ResColors, Resources};
use openbounty_engine::tables::puzzle_grid_entity;
use openbounty_engine::tile::Terrain;

use crate::banner;
use crate::draw::{Color, DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::layout::{Layout, MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
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
}

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

    /// True when the active view swaps the status bar to "Press 'ESC' to
    /// exit". The character, army and puzzle views always do; the contract
    /// view only when a contract is actually active. The worldmap is handled
    /// separately by [`StatusMode::Worldmap`](crate::chrome::StatusMode).
    pub fn wants_exit_hint(&self, game: &Game) -> bool {
        match self.active() {
            Some(ViewKind::Character | ViewKind::Army | ViewKind::Puzzle) => true,
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
        time: f64,
    ) {
        // The puzzle reveal animation resets its clock each time the view
        // becomes active.
        let puzzle_active = self.active() == Some(ViewKind::Puzzle);
        if puzzle_active && !self.puzzle_was_active {
            self.puzzle_open_time = time;
        }
        self.puzzle_was_active = puzzle_active;

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
fn troop_sprite(catalog: &SpriteCatalog, index: i32, tick: i32) -> Option<Sprite> {
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
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
        );
        assert!(list.is_empty());
    }

    #[test]
    fn a_zero_value_draws_no_number() {
        let (resources, layout, catalog) = fixtures();
        let game = knight_game();
        let palette = Palette::fallback();
        let (map, fog) = map_and_fog();
        let mut views = Views::new();
        views.set(ViewKind::Character);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();
        views.set(ViewKind::Army);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();
        views.set(ViewKind::Contract);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let mut views = Views::new();
        views.set(ViewKind::Contract);

        let mut list = DrawList::new();
        views.draw(
            &mut list, &layout, &palette, &resources, &catalog, &map, &fog, &game, 0.0,
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
        let catalog = SpriteCatalog::from_resources(resources);
        let mut list = DrawList::new();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, 0.0,
        );
        list.clear();
        views.draw(
            &mut list, layout, &palette, resources, &catalog, &map, &fog, game, 100.0,
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
        views.draw(
            &mut list, layout, palette, resources, catalog, map, fog, game, time,
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
}
