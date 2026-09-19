//! The combat field renderer, porting `src/combat_render.c`.
//!
//! Combat owns the full inner area of the screen -- no sidebar -- on a fixed
//! 6x5 grid of 48x34 cells, whatever the overworld tile size. The engine's
//! [`Combat`] is the only state: this module reads it and emits
//! [`crate::draw::DrawList`] commands, so the field is testable headlessly the
//! way the views are.
//!
//! The interactive loop (cursor, target picker, spell cast, the AI turn) is
//! not here yet; this slice draws a ready-made field and its title bar.

use openbounty_engine::combat::{
    Combat, COMBAT_H, COMBAT_SIDES, COMBAT_SIDE_AI, COMBAT_SIDE_PLAYER, COMBAT_SLOTS, COMBAT_W,
};
use openbounty_engine::resources::Resources;

use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::layout::{MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
use crate::sprites::SpriteCatalog;
use crate::views::troop_sprite;

/// Combat field origin, C's `CL_COMBAT_X` / `CL_COMBAT_Y`.
pub const COMBAT_X: i32 = MAP_X;
pub const COMBAT_Y: i32 = MAP_Y;

/// Combat cell size, C's `CL_COMBAT_CELL_W` / `CL_COMBAT_CELL_H`. Combat is
/// always 6x5 at 48x34 even in a modern pack: the field fills the inner area
/// instead of the overworld's tile grid.
pub const COMBAT_CELL_W: i32 = 48;
pub const COMBAT_CELL_H: i32 = 34;

fn cell_origin(gx: i32, gy: i32) -> (i32, i32) {
    (COMBAT_X + gx * COMBAT_CELL_W, COMBAT_Y + gy * COMBAT_CELL_H)
}

/// A troop's display name, or `fallback` when the catalog has none.
fn troop_name(resources: &Resources, troop_index: i32, fallback: &str) -> String {
    resources
        .troop_by_index(troop_index)
        .map(|troop| {
            if troop.name.is_empty() {
                fallback.to_owned()
            } else {
                troop.name.as_str().to_owned()
            }
        })
        .unwrap_or_else(|| fallback.to_owned())
}

/// The status-bar title, porting `combat_format_title`. Before the first kill
/// it names the acting unit and its remaining moves/shots; after, it names the
/// largest stack on each side and the kill count.
pub fn combat_title(combat: &Combat, resources: &Resources) -> String {
    if !combat.first_kill_seen {
        let (name, moves, shots) = if combat.unit_id >= 0 {
            let unit = &combat.units[combat.side][combat.unit_id as usize];
            (
                troop_name(resources, unit.troop_idx, "-"),
                unit.moves,
                unit.shots,
            )
        } else {
            ("-".to_owned(), 0, 0)
        };
        if shots > 0 {
            format!(" Options / {name} M{moves},S{shots}")
        } else {
            format!(" Options / {name} M{moves}")
        }
    } else {
        let mut player_name = "Army".to_owned();
        let mut foe_name = "Foe".to_owned();
        let mut player_max = 0;
        let mut foe_max = 0;
        for slot in 0..COMBAT_SLOTS {
            let player = &combat.units[COMBAT_SIDE_PLAYER][slot];
            if player.count > player_max {
                player_max = player.count;
                player_name = troop_name(resources, player.troop_idx, "Army");
            }
            let foe = &combat.units[COMBAT_SIDE_AI][slot];
            if foe.count > foe_max {
                foe_max = foe.count;
                foe_name = troop_name(resources, foe.troop_idx, "Foe");
            }
        }
        format!(
            " {player_name} vs {foe_name} killing {}",
            combat.stacks_destroyed
        )
    }
}

/// Draw the combat field: the grass tiles, the obstacles, the units with their
/// count badges, and the damage bursts. Ports `combat_render_frame` minus the
/// cursor (which the interactive loop will own).
pub fn combat_render(
    list: &mut DrawList,
    combat: &Combat,
    palette: &Palette,
    catalog: &SpriteCatalog,
) {
    // Grass field, one tile per cell.
    for y in 0..COMBAT_H as i32 {
        for x in 0..COMBAT_W as i32 {
            let (px, py) = cell_origin(x, y);
            list.blit(
                Sprite::CombatTile(0),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }

    // Obstacles: omap codes 1..3 are field obstacles, 5..10 castle wall pieces.
    for y in 0..COMBAT_H as i32 {
        for x in 0..COMBAT_W as i32 {
            let code = combat.omap[y as usize][x as usize];
            if code == 0 {
                continue;
            }
            let (px, py) = cell_origin(x, y);
            list.blit(
                Sprite::CombatTile(code as usize),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }

    // Units, drawn over the obstacles. The AI side faces left (mirrored).
    for side in 0..COMBAT_SIDES {
        for slot in 0..COMBAT_SLOTS {
            let unit = &combat.units[side][slot];
            if unit.troop_idx < 0 || unit.count == 0 {
                continue;
            }
            let (px, py) = cell_origin(unit.x, unit.y);
            if let Some(sprite) = troop_sprite(catalog, unit.troop_idx, unit.frame) {
                list.blit_flipped(
                    sprite,
                    Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
                    side == COMBAT_SIDE_AI,
                );
            }
            // Count badge: white digits on a black band, bottom-centre.
            let digits = unit.count.to_string();
            let w = font::measure_width(&digits);
            let bx = px + (COMBAT_CELL_W - w) / 2;
            let by = py + COMBAT_CELL_H - font::GLYPH_H - 1;
            list.rect(
                Rect::new(bx - 1, by - 1, w + 2, font::GLYPH_H + 2),
                palette.clr(Pal::Black),
            );
            list.text(&digits, bx, by, palette.clr(Pal::White));
        }
    }

    // Damage burst (combat tile frame 4) over any unit that just took a hit.
    for side in 0..COMBAT_SIDES {
        for slot in 0..COMBAT_SLOTS {
            let unit = &combat.units[side][slot];
            if unit.troop_idx < 0 || unit.hit_flash <= 0 {
                continue;
            }
            let (px, py) = cell_origin(unit.x, unit.y);
            list.blit(
                Sprite::CombatTile(4),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::combat::{CombatUnit, COMBAT_SIDE_AI, COMBAT_SIDE_PLAYER};

    fn fixtures() -> (Resources, SpriteCatalog, Palette, Combat) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":50,
                  "spoils_factor":1,"abilities":"","morale_group":"A"},
                 {"index":1,"id":"archers","name":"Archers","skill_level":1,"hit_points":3,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":250,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ]}"#,
        )
        .expect("fixture parses");
        let catalog = SpriteCatalog::from_resources(&resources);
        let palette = Palette::fallback();
        let mut combat = Combat::default();

        let mut player = CombatUnit::empty();
        player.troop_idx = 0;
        player.count = 10;
        player.x = 0;
        player.y = 0;
        player.moves = 5;
        player.shots = 2;
        combat.units[COMBAT_SIDE_PLAYER][0] = player;

        let mut foe = CombatUnit::empty();
        foe.troop_idx = 1;
        foe.count = 8;
        foe.x = 4;
        foe.y = 4;
        combat.units[COMBAT_SIDE_AI][0] = foe;

        combat.unit_id = 0;
        combat.side = COMBAT_SIDE_PLAYER;
        combat.omap[2][3] = 2;

        (resources, catalog, palette, combat)
    }

    #[test]
    fn the_title_names_the_acting_unit_before_the_first_kill() {
        let (resources, _, _, combat) = fixtures();
        assert_eq!(
            combat_title(&combat, &resources),
            " Options / Militia M5,S2"
        );
    }

    #[test]
    fn the_title_names_the_largest_stack_after_the_first_kill() {
        let (resources, _, _, mut combat) = fixtures();
        combat.first_kill_seen = true;
        combat.stacks_destroyed = 3;
        assert_eq!(
            combat_title(&combat, &resources),
            " Militia vs Archers killing 3"
        );
    }

    #[test]
    fn a_melee_only_unit_omits_the_shot_count() {
        let (resources, _, _, mut combat) = fixtures();
        combat.units[COMBAT_SIDE_PLAYER][0].shots = 0;
        assert_eq!(combat_title(&combat, &resources), " Options / Militia M5");
    }

    #[test]
    fn the_field_draws_grass_obstacles_and_count_badges() {
        let (_, catalog, palette, combat) = fixtures();
        let mut list = DrawList::new();
        combat_render(&mut list, &combat, &palette, &catalog);

        // 6x5 grass tiles, one obstacle tile, and both count badges.
        assert_eq!(list.blits_of(Sprite::CombatTile(0)).len(), 30);
        assert!(!list.blits_of(Sprite::CombatTile(2)).is_empty());
        assert!(list.has_text_containing("10"));
        assert!(list.has_text_containing("8"));
    }
}
