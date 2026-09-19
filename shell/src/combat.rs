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
    combat_ai_action, combat_cast_spell, combat_cell_passes_filter, combat_compact,
    combat_fly_unit, combat_hit_unit, combat_in_bounds, combat_init, combat_move_unit,
    combat_next_turn, combat_next_unit, combat_prepare_castle, combat_prepare_foe,
    combat_prepare_player, combat_reset_match, combat_reset_turn, combat_seed_rng,
    combat_spell_target_filter, combat_test_dead, combat_unit_surrounded, CastResult, Combat,
    CombatMode, CombatTarget, PickFilter, COMBAT_H, COMBAT_SIDES, COMBAT_SIDE_AI,
    COMBAT_SIDE_PLAYER, COMBAT_SLOTS, COMBAT_SPELL_TELEPORT, COMBAT_W,
};
use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;
use openbounty_engine::tables::TROOP_ABIL_FLY;

use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::input::{Key, KeySource};
use crate::layout::{MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
use crate::sprites::SpriteCatalog;
use crate::views::troop_sprite;

/// The A-G keys the cast menu reads, in order.
const COMBAT_SPELL_KEYS: [Key; 7] = [Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G];

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

/// What a pick is for, porting `CombatPickReason`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickReason {
    None,
    Shoot,
    Fly,
    SpellTarget,
    TeleportDest,
}

/// The cast workflow phase, porting `COMBAT_CAST_*`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastPhase {
    None,
    PickSpell,
    PickTarget,
    Apply,
}

/// Where a combat step left the fight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatStatus {
    Running,
    Win,
    Loss,
}

/// The shell-side combat loop: the engine's [`Combat`] plus the cursor, picker
/// and cast state C keeps on the same struct. Ports `RunCombat`'s per-frame
/// input dispatch, minus the animation tick, the give-up prompt and the
/// end-of-combat dialog, which arrive with the flow wiring.
pub struct CombatLoop {
    pub combat: Combat,
    picker_active: bool,
    cursor_x: i32,
    cursor_y: i32,
    pick_filter: PickFilter,
    pick_reason: PickReason,
    cast_phase: CastPhase,
    cast_spell_idx: i32,
    pick_t1_side: usize,
    pick_t1_slot: usize,
    cast_dest_x: i32,
    cast_dest_y: i32,
}

impl CombatLoop {
    /// Set up a fight and prime the first acting unit. Ports the head of
    /// `RunCombat`.
    pub fn new(
        game: &Game,
        resources: &Resources,
        mode: CombatMode,
        target: &CombatTarget,
    ) -> CombatLoop {
        let mut combat = Combat::default();
        combat_init(&mut combat, game, resources, mode);
        combat_seed_rng(&mut combat, game, mode, target);
        combat_prepare_player(&mut combat, game, resources);
        if mode == CombatMode::Castle {
            combat_prepare_castle(&mut combat, resources, target);
        } else {
            combat_prepare_foe(&mut combat, resources, target);
        }
        combat_reset_match(&mut combat, resources);
        combat_reset_turn(&mut combat, resources, COMBAT_SIDE_AI);
        combat.unit_id = -1;
        let next = combat_next_unit(&mut combat);
        if next >= 0 {
            combat.unit_id = next;
        }
        CombatLoop {
            combat,
            picker_active: false,
            cursor_x: 0,
            cursor_y: 0,
            pick_filter: PickFilter::Any,
            pick_reason: PickReason::None,
            cast_phase: CastPhase::None,
            cast_spell_idx: -1,
            pick_t1_side: 0,
            pick_t1_slot: 0,
            cast_dest_x: 0,
            cast_dest_y: 0,
        }
    }

    /// Drive one frame of combat input. The player's actions dispatch through
    /// the engine's move/hit/fly/cast cores; on the AI's turn one AI action is
    /// taken per call.
    pub fn step(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
    ) -> CombatStatus {
        if self.combat.result != 0 {
            return self.status();
        }

        let acted = if self.cast_phase != CastPhase::None {
            self.cast_step(keys, game, resources)
        } else if self.picker_active {
            self.pick_dispatch(keys, game, resources)
        } else {
            let player_turn = self.combat.side == COMBAT_SIDE_PLAYER
                && self.combat.unit_id >= 0
                && !self.combat.units[self.combat.side][self.combat.unit_id as usize]
                    .out_of_control;
            if player_turn {
                self.player_action(keys, game, resources)
            } else {
                combat_ai_action(&mut self.combat, resources) != 0
            }
        };

        if !acted {
            return self.status();
        }

        combat_compact(&mut self.combat);
        if combat_test_dead(&self.combat, COMBAT_SIDE_AI) {
            self.combat.result = 1;
            return self.status();
        }
        if combat_test_dead(&self.combat, COMBAT_SIDE_PLAYER) {
            self.combat.result = 2;
            return self.status();
        }

        // Advance to the next unit only when the acting one is truly done.
        let done = self.combat.unit_id < 0 || {
            let unit = &self.combat.units[self.combat.side][self.combat.unit_id as usize];
            unit.troop_idx < 0 || unit.count == 0 || unit.acted
        };
        if done {
            if self.combat.unit_id >= 0 {
                let side = self.combat.side;
                self.combat.units[side][self.combat.unit_id as usize].frame = 0;
            }
            let next = combat_next_unit(&mut self.combat);
            if next < 0 {
                combat_next_turn(&mut self.combat, resources);
            } else {
                self.combat.unit_id = next;
            }
        }
        self.status()
    }

    fn status(&self) -> CombatStatus {
        match self.combat.result {
            1 => CombatStatus::Win,
            2 => CombatStatus::Loss,
            _ => CombatStatus::Running,
        }
    }

    fn player_action(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
    ) -> bool {
        if let Some((dx, dy)) = combat_read_dir(keys) {
            if self.combat.unit_id < 0 {
                return false;
            }
            let side = self.combat.side;
            let id = self.combat.unit_id as usize;
            return combat_move_unit(&mut self.combat, resources, side, id, dx, dy) != 0;
        }
        if keys.pressed(Key::Space) || keys.pressed(Key::W) {
            if self.combat.unit_id >= 0 {
                let side = self.combat.side;
                self.combat.units[side][self.combat.unit_id as usize].acted = true;
            }
            return true;
        }
        if keys.pressed(Key::S) {
            if self.combat.unit_id < 0 {
                return false;
            }
            let (side, id) = (self.combat.side, self.combat.unit_id as usize);
            let unit = &self.combat.units[side][id];
            if unit.shots <= 0 {
                return false;
            }
            if combat_unit_surrounded(&self.combat, side, id) {
                return false;
            }
            self.cursor_x = unit.x;
            self.cursor_y = unit.y;
            self.pick_filter = PickFilter::Enemy;
            self.pick_reason = PickReason::Shoot;
            self.picker_active = true;
            return false;
        }
        if keys.pressed(Key::F) {
            if self.combat.unit_id < 0 {
                return false;
            }
            let (side, id) = (self.combat.side, self.combat.unit_id as usize);
            let unit = &self.combat.units[side][id];
            let flies = resources
                .troop_by_index(unit.troop_idx)
                .is_some_and(|troop| troop.abilities & TROOP_ABIL_FLY != 0);
            if !flies || unit.flights <= 0 {
                return false;
            }
            self.cursor_x = unit.x;
            self.cursor_y = unit.y;
            self.pick_filter = PickFilter::Empty;
            self.pick_reason = PickReason::Fly;
            self.picker_active = true;
            return false;
        }
        if keys.pressed(Key::U) {
            if self.combat.spells_this_round >= 1 {
                return false;
            }
            if !game.stats.knows_magic {
                return false;
            }
            self.cast_phase = CastPhase::PickSpell;
            return false;
        }
        false
    }

    /// The shoot/fly picker: one `combat_pick_step` per frame, then dispatch.
    fn pick_dispatch(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
    ) -> bool {
        let mut tx = 0usize;
        let mut ty = 0usize;
        let mut cancelled = false;
        if !self.pick_step(keys, resources, &mut tx, &mut ty, &mut cancelled) {
            if cancelled {
                self.picker_active = false;
                self.pick_reason = PickReason::None;
            }
            return false;
        }

        let (side, id) = (self.combat.side, self.combat.unit_id as usize);
        let reason = self.pick_reason;
        let acted = match reason {
            PickReason::Shoot => {
                let uid = self.combat.umap[ty][tx];
                let target_side = (uid - 1) as usize / COMBAT_SLOTS;
                let target_slot = (uid - 1) as usize % COMBAT_SLOTS;
                combat_hit_unit(
                    &mut self.combat,
                    resources,
                    side,
                    id,
                    target_side,
                    target_slot,
                    true,
                );
                self.combat.units[side][id].acted = true;
                true
            }
            PickReason::Fly => {
                combat_fly_unit(&mut self.combat, side, id, tx as i32, ty as i32) != 0
            }
            _ => false,
        };
        self.picker_active = false;
        self.pick_reason = PickReason::None;
        let _ = game;
        acted
    }

    /// One target-picker step, porting `combat_pick_step`.
    fn pick_step(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        out_x: &mut usize,
        out_y: &mut usize,
        out_cancelled: &mut bool,
    ) -> bool {
        *out_cancelled = false;
        let mut dx = 0;
        let mut dy = 0;
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            dy = -1;
        } else if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            dy = 1;
        }
        if keys.pressed(Key::Left) || keys.pressed(Key::Kp4) {
            dx = -1;
        } else if keys.pressed(Key::Right) || keys.pressed(Key::Kp6) {
            dx = 1;
        }
        if keys.pressed(Key::Kp7) || keys.pressed(Key::Home) {
            dx = -1;
            dy = -1;
        } else if keys.pressed(Key::Kp9) || keys.pressed(Key::PageUp) {
            dx = 1;
            dy = -1;
        } else if keys.pressed(Key::Kp1) || keys.pressed(Key::End) {
            dx = -1;
            dy = 1;
        } else if keys.pressed(Key::Kp3) || keys.pressed(Key::PageDown) {
            dx = 1;
            dy = 1;
        }
        if dx != 0 || dy != 0 {
            let nx = self.cursor_x + dx;
            let ny = self.cursor_y + dy;
            if combat_in_bounds(nx, ny) {
                self.cursor_x = nx;
                self.cursor_y = ny;
            }
            return false;
        }

        if keys.pressed(Key::Enter)
            || keys.pressed(Key::KpEnter)
            || keys.pressed(Key::Space)
            || keys.pressed(Key::A)
            || keys.pressed(Key::C)
        {
            if combat_cell_passes_filter(
                &self.combat,
                resources,
                self.cursor_x as usize,
                self.cursor_y as usize,
                self.combat.side,
                self.pick_filter,
            ) {
                *out_x = self.cursor_x as usize;
                *out_y = self.cursor_y as usize;
                return true;
            }
        }
        if keys.pressed(Key::Escape) {
            *out_cancelled = true;
        }
        false
    }

    /// The cast state machine, porting `combat_cast_step`.
    fn cast_step(&mut self, keys: &dyn KeySource, game: &mut Game, resources: &Resources) -> bool {
        match self.cast_phase {
            CastPhase::PickSpell => {
                if keys.pressed(Key::Escape) {
                    self.cast_phase = CastPhase::None;
                    return false;
                }
                let mut picked = -1;
                for (i, key) in COMBAT_SPELL_KEYS.iter().enumerate() {
                    if keys.pressed(*key) {
                        picked = i as i32;
                        break;
                    }
                }
                if picked < 0 {
                    return false;
                }
                if game.spells.counts[picked as usize] <= 0 {
                    self.cast_phase = CastPhase::None;
                    return false;
                }
                self.cast_spell_idx = picked;
                self.pick_filter = combat_spell_target_filter(picked);
                self.pick_reason = PickReason::SpellTarget;
                self.picker_active = true;
                if self.combat.unit_id >= 0 {
                    let unit = &self.combat.units[self.combat.side][self.combat.unit_id as usize];
                    self.cursor_x = unit.x;
                    self.cursor_y = unit.y;
                }
                self.cast_phase = CastPhase::PickTarget;
                false
            }
            CastPhase::PickTarget => {
                let mut tx = 0usize;
                let mut ty = 0usize;
                let mut cancelled = false;
                if !self.pick_step(keys, resources, &mut tx, &mut ty, &mut cancelled) {
                    if cancelled {
                        self.cast_phase = CastPhase::None;
                        self.pick_reason = PickReason::None;
                        self.picker_active = false;
                    }
                    return false;
                }
                if self.cast_spell_idx == COMBAT_SPELL_TELEPORT
                    && self.pick_reason == PickReason::SpellTarget
                {
                    let uid = self.combat.umap[ty][tx];
                    self.pick_t1_side = (uid - 1) as usize / COMBAT_SLOTS;
                    self.pick_t1_slot = (uid - 1) as usize % COMBAT_SLOTS;
                    self.pick_filter = PickFilter::Empty;
                    self.pick_reason = PickReason::TeleportDest;
                    if self.combat.unit_id >= 0 {
                        let unit =
                            &self.combat.units[self.combat.side][self.combat.unit_id as usize];
                        self.cursor_x = unit.x;
                        self.cursor_y = unit.y;
                    }
                    return false;
                }
                if self.pick_reason == PickReason::TeleportDest {
                    self.cast_dest_x = tx as i32;
                    self.cast_dest_y = ty as i32;
                } else {
                    let uid = self.combat.umap[ty][tx];
                    self.pick_t1_side = (uid - 1) as usize / COMBAT_SLOTS;
                    self.pick_t1_slot = (uid - 1) as usize % COMBAT_SLOTS;
                }
                self.picker_active = false;
                self.pick_reason = PickReason::None;
                self.cast_phase = CastPhase::Apply;
                self.apply_cast(game, resources)
            }
            CastPhase::Apply => self.apply_cast(game, resources),
            CastPhase::None => false,
        }
    }

    fn apply_cast(&mut self, game: &mut Game, resources: &Resources) -> bool {
        let side = self.combat.side;
        let result = combat_cast_spell(
            &mut self.combat,
            resources,
            game,
            side,
            self.cast_spell_idx,
            self.pick_t1_side,
            self.pick_t1_slot,
            self.cast_dest_x,
            self.cast_dest_y,
        );
        self.cast_phase = CastPhase::None;
        result == CastResult::Ok
    }
}

/// Read one directional step, porting `combat_read_dir`.
fn combat_read_dir(keys: &dyn KeySource) -> Option<(i32, i32)> {
    if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
        return Some((0, -1));
    }
    if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
        return Some((0, 1));
    }
    if keys.pressed(Key::Left) || keys.pressed(Key::Kp4) {
        return Some((-1, 0));
    }
    if keys.pressed(Key::Right) || keys.pressed(Key::Kp6) {
        return Some((1, 0));
    }
    if keys.pressed(Key::Home) || keys.pressed(Key::Kp7) {
        return Some((-1, -1));
    }
    if keys.pressed(Key::PageUp) || keys.pressed(Key::Kp9) {
        return Some((1, -1));
    }
    if keys.pressed(Key::End) || keys.pressed(Key::Kp1) {
        return Some((-1, 1));
    }
    if keys.pressed(Key::PageDown) || keys.pressed(Key::Kp3) {
        return Some((1, 1));
    }
    None
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

    use crate::input::Keys;

    /// A player militia (0,0) with moves and shots, facing an AI archer at
    /// (4,4). The player acts first.
    fn loop_fixture() -> (CombatLoop, Resources, Game) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,
                  "move_rate":3,"melee":[1,1],"ranged":[2,2,4],"recruit_cost":50,
                  "spoils_factor":1,"abilities":"","morale_group":"A"},
                 {"index":1,"id":"archers","name":"Archers","skill_level":1,"hit_points":3,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":250,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ]}"#,
        )
        .expect("fixture parses");

        let mut combat = Combat::default();
        let mut player = CombatUnit::empty();
        player.troop_idx = 0;
        player.count = 10;
        player.max_count = 10;
        player.x = 0;
        player.y = 0;
        player.moves = 3;
        player.shots = 2;
        combat.units[COMBAT_SIDE_PLAYER][0] = player;
        combat.umap[0][0] = 1;

        let mut foe = CombatUnit::empty();
        foe.troop_idx = 1;
        foe.count = 8;
        foe.max_count = 8;
        foe.x = 4;
        foe.y = 4;
        combat.units[COMBAT_SIDE_AI][0] = foe;
        combat.umap[4][4] = 6;

        combat.unit_id = 0;
        combat.side = COMBAT_SIDE_PLAYER;
        combat.heroes_present[COMBAT_SIDE_PLAYER] = true;

        let game = Game::default();
        let loop_ = CombatLoop {
            combat,
            picker_active: false,
            cursor_x: 0,
            cursor_y: 0,
            pick_filter: PickFilter::Any,
            pick_reason: PickReason::None,
            cast_phase: CastPhase::None,
            cast_spell_idx: -1,
            pick_t1_side: 0,
            pick_t1_slot: 0,
            cast_dest_x: 0,
            cast_dest_y: 0,
        };
        (loop_, resources, game)
    }

    #[test]
    fn a_player_move_steps_the_unit() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(&Keys::pressed(&[Key::Right]), &mut game, &resources);
        assert_eq!(status, CombatStatus::Running);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].x, 1);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].y, 0);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].moves, 2);
    }

    #[test]
    fn waiting_ends_the_players_turn() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.step(&Keys::pressed(&[Key::Space]), &mut game, &resources);
        // The player's lone stack is done, so the turn rolls over to the AI.
        assert_eq!(loop_.combat.side, COMBAT_SIDE_AI);
        assert_eq!(loop_.combat.unit_id, 0);
    }

    #[test]
    fn shooting_arms_the_picker() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.step(&Keys::pressed(&[Key::S]), &mut game, &resources);
        assert!(loop_.picker_active);
        assert_eq!(loop_.pick_reason, PickReason::Shoot);
        assert_eq!(loop_.cursor_x, 0);
        assert_eq!(loop_.cursor_y, 0);
    }

    #[test]
    fn a_unit_with_no_shots_cannot_shoot() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].shots = 0;
        loop_.step(&Keys::pressed(&[Key::S]), &mut game, &resources);
        assert!(!loop_.picker_active);
    }
}
