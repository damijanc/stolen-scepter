use crate::game::{ArmyStack, Game, Unit};
use crate::resources::Resources;
use crate::tables::{
    ArtifactPower, TROOP_ABIL_ABSORB, TROOP_ABIL_FLY, TROOP_ABIL_IMMUNE, TROOP_ABIL_LEECH,
    TROOP_ABIL_MAGIC, TROOP_ABIL_REGEN, TROOP_ABIL_SCYTHE,
};

const MULT: u64 = 25_214_903_917;
const INC: u64 = 11;

pub const COMBAT_SIDES: usize = 2;
pub const COMBAT_SLOTS: usize = 5;
pub const COMBAT_SIDE_PLAYER: usize = 0;
pub const COMBAT_SIDE_AI: usize = 1;
pub const COMBAT_W: usize = 6;
pub const COMBAT_H: usize = 5;
/// DETERMINISM-COUPLED: every resolution path must pass the same cap, since
/// a cap-hitting fight resolves differently under a different one.
pub const COMBAT_MAX_ROUNDS: i32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatMode {
    Castle = 0,
    Foe = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatResult {
    Win = 0,
    Loss = 1,
}

pub const COMBAT_POWER_INCREASED_DAMAGE: u8 = 1 << 0;
pub const COMBAT_POWER_QUARTER_PROTECTION: u8 = 1 << 1;

const FNV64_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
const FNV64_PRIME: u64 = 0x0000_0100_0000_01B3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CombatUnit {
    pub troop_idx: i32,
    pub count: i32,
    pub turn_count: i32,
    pub max_count: i32,
    pub dead: bool,
    pub frame: i32,
    pub injury: i32,
    pub acted: bool,
    pub retaliated: bool,
    pub moves: i32,
    pub shots: i32,
    pub flights: i32,
    pub frozen: bool,
    pub out_of_control: bool,
    pub x: i32,
    pub y: i32,
    pub hit_flash: i32,
}

impl CombatUnit {
    pub const fn empty() -> Self {
        Self {
            troop_idx: -1,
            count: 0,
            turn_count: 0,
            max_count: 0,
            dead: false,
            frame: 0,
            injury: 0,
            acted: false,
            retaliated: false,
            moves: 0,
            shots: 0,
            flights: 0,
            frozen: false,
            out_of_control: false,
            x: 0,
            y: 0,
            hit_flash: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Combat {
    pub units: [[CombatUnit; COMBAT_SLOTS]; COMBAT_SIDES],
    /// Obstacle tile codes, 0 = open. Sized H+1 x W+1 like C, whose extra
    /// row and column guard the distance and adjacency math.
    pub omap: [[u8; COMBAT_W + 1]; COMBAT_H + 1],
    /// Packed UID per cell: `side * 5 + slot + 1`, 0 = empty.
    pub umap: [[u8; COMBAT_W + 1]; COMBAT_H + 1],
    pub spoils: [i32; COMBAT_SIDES],
    pub rng_state: u64,
    pub powers: [u8; COMBAT_SIDES],
    pub heroes_present: [bool; COMBAT_SIDES],
    /// Leadership captured at prepare time, standing in for C's
    /// `Game *heroes[]` back-pointer. Leadership cannot change mid-fight, so
    /// a snapshot reads the same as C's live deref.
    pub leadership: [i32; COMBAT_SIDES],
    pub turn: i32,
    pub phase: i32,
    pub spells_this_round: i32,
    pub side: usize,
    /// Currently acting slot, or -1 before the first unit is primed.
    pub unit_id: i32,
    pub castle: bool,
    pub first_kill_seen: bool,
    pub stacks_destroyed: i32,
    /// 0 = running, 1 = player win, 2 = AI win.
    pub result: i32,
}

impl Default for Combat {
    fn default() -> Self {
        Self {
            units: [[CombatUnit::empty(); COMBAT_SLOTS]; COMBAT_SIDES],
            omap: [[0; COMBAT_W + 1]; COMBAT_H + 1],
            umap: [[0; COMBAT_W + 1]; COMBAT_H + 1],
            spoils: [0; COMBAT_SIDES],
            rng_state: 0,
            powers: [0; COMBAT_SIDES],
            heroes_present: [false; COMBAT_SIDES],
            leadership: [0; COMBAT_SIDES],
            turn: 0,
            phase: 0,
            spells_this_round: 0,
            side: COMBAT_SIDE_PLAYER,
            unit_id: -1,
            castle: false,
            first_kill_seen: false,
            stacks_destroyed: 0,
            result: 0,
        }
    }
}

/// The defender in an encounter. `seed_key` is a stable per-encounter
/// identity (a foe's placement id, a castle id) so a fight's RNG is a pure
/// function of (world seed, identity, mode) and never of casualty history.
#[derive(Clone, Copy, Debug, Default)]
pub struct CombatTarget<'a> {
    pub name: &'a str,
    pub garrison: &'a [Unit],
    pub seed_key: &'a str,
}

pub fn combat_rand(c: &mut Combat, min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }

    c.rng_state = c.rng_state.wrapping_mul(MULT).wrapping_add(INC);
    let r = (c.rng_state >> 32) as u32;
    min + (r % ((max - min + 1) as u32)) as i32
}

pub fn combat_init_unit(u: &mut CombatUnit, troop_idx: i32, count: i32) {
    u.troop_idx = troop_idx;
    u.count = count;
    u.max_count = count;
    u.turn_count = count;
    u.dead = false;
    u.frame = 0;
    u.injury = 0;
    u.acted = false;
    u.retaliated = false;
    u.moves = 0;
    u.shots = 0;
    u.flights = 0;
    u.frozen = false;
    u.out_of_control = false;
    u.x = 0;
    u.y = 0;
    u.hit_flash = 0;
}

/// Pack a (side, slot) pair into the 1-based UID stored in `umap`.
fn pack_uid(side: usize, slot: usize) -> u8 {
    (side * COMBAT_SLOTS + slot + 1) as u8
}

/// Split a packed UID back into (side, slot). Only call on a non-zero UID.
fn unpack_uid(uid: u8) -> (usize, usize) {
    let index = (uid - 1) as usize;
    (index / COMBAT_SLOTS, index % COMBAT_SLOTS)
}

/// Artifact powers the player carries into a fight. C scans only the first
/// 8 artifact slots.
pub fn combat_player_powers(game: &Game, resources: &Resources) -> u8 {
    let mut powers = 0;
    for index in 0..8 {
        if !game.artifacts.found[index] {
            continue;
        }
        let Some(artifact) = resources.artifact_by_index(index as i32) else {
            continue;
        };
        if artifact.power == ArtifactPower::IncreasedDamage {
            powers |= COMBAT_POWER_INCREASED_DAMAGE;
        }
        if artifact.power == ArtifactPower::QuarterProtection {
            powers |= COMBAT_POWER_QUARTER_PROTECTION;
        }
    }
    powers
}

/// Per-combat LCG seed. Mixes the world seed with the mode and a stable
/// encounter identity, deliberately NOT with cumulative casualty history:
/// that made a fight's seed depend on the order of every prior fight.
pub fn combat_seed_rng(c: &mut Combat, game: &Game, mode: CombatMode, target: &CombatTarget) {
    let mut s = game.seed;
    s ^= 0x5DEE_CE66D;
    s ^= (mode as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let key = if !target.seed_key.is_empty() {
        target.seed_key
    } else {
        target.name
    };
    for byte in key.as_bytes() {
        s = s.wrapping_mul(131).wrapping_add(*byte as u64);
    }
    c.rng_state = if s != 0 { s } else { 1 };
}

/// A stack is out of control when `hit_points * count` exceeds leadership,
/// strictly: filling to exactly leadership is still controlled, matching the
/// recruit cap.
pub fn unit_under_control(
    leadership: i32,
    resources: &Resources,
    troop_idx: i32,
    count: i32,
) -> bool {
    let Some(troop) = resources.troop_by_index(troop_idx) else {
        return false;
    };
    troop.hit_points.wrapping_mul(count) <= leadership
}

/// Two units are friendly only if neither is out of control and they share
/// a side. An out-of-control stack is an enemy of its own side.
fn units_are_friendly(c: &Combat, s_a: usize, i_a: usize, s_b: usize, i_b: usize) -> bool {
    if c.units[s_a][i_a].out_of_control || c.units[s_b][i_b].out_of_control {
        return false;
    }
    s_a == s_b
}

/// Integer square root, fixed-point 16.16. The AI's tie-breaking depends on
/// this exact bit-walk; do not replace it with a float sqrt.
fn isqrt32(h_in: u32) -> u32 {
    let mut h = h_in;
    let mut x: u32 = 0;
    let mut y: u32 = 0;
    for _ in 0..32 {
        x = (x << 1) | 1;
        if y < x {
            x = x.wrapping_sub(2);
        } else {
            y = y.wrapping_sub(x);
        }
        x = x.wrapping_add(1);
        y <<= 1;
        if h & 0x8000_0000 != 0 {
            y |= 1;
        }
        h <<= 1;
        y <<= 1;
        if h & 0x8000_0000 != 0 {
            y |= 1;
        }
        h <<= 1;
    }
    x
}

/// Distance as `isqrt32((dx*dx + dy*dy) << 16)`, so the AI compares without
/// floating point.
fn calc_distance(x1: i32, y1: i32, x2: i32, y2: i32) -> u32 {
    let dx = (x2 - x1) as i64;
    let dy = (y2 - y1) as i64;
    let d2 = (dx * dx + dy * dy) as u32;
    isqrt32(d2 << 16)
}

/// 8-neighbour adjacency.
fn unit_touching(a: &CombatUnit, b: &CombatUnit) -> bool {
    (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1
}

pub fn combat_in_bounds(x: i32, y: i32) -> bool {
    x >= 0 && (x as usize) < COMBAT_W && y >= 0 && (y as usize) < COMBAT_H
}

/// Zero the state, capture mode metadata, translate hero powers, and mark
/// every slot empty. Ports `combat_init`.
pub fn combat_init(c: &mut Combat, game: &Game, resources: &Resources, mode: CombatMode) {
    *c = Combat::default();
    c.castle = mode == CombatMode::Castle;
    c.heroes_present[COMBAT_SIDE_PLAYER] = true;
    c.heroes_present[COMBAT_SIDE_AI] = false;
    c.powers[COMBAT_SIDE_PLAYER] = combat_player_powers(game, resources);
    c.powers[COMBAT_SIDE_AI] = 0;
    c.leadership[COMBAT_SIDE_PLAYER] = game.stats.leadership_current;
    c.leadership[COMBAT_SIDE_AI] = 0;
}

/// Player army into column 0, rows 0..4, accruing the player's spoils line.
pub fn combat_prepare_player(c: &mut Combat, game: &Game, resources: &Resources) {
    for i in 0..COMBAT_SLOTS {
        let stack = game.army[i];
        if stack.id.is_empty() || stack.count == 0 {
            continue;
        }
        let Some(troop) = resources.troop_by_id(stack.id.as_str()) else {
            continue;
        };
        let (index, spoils_factor) = (troop.index, troop.spoils_factor);
        let controlled = unit_under_control(
            c.leadership[COMBAT_SIDE_PLAYER],
            resources,
            index,
            stack.count,
        );
        let u = &mut c.units[COMBAT_SIDE_PLAYER][i];
        combat_init_unit(u, index, stack.count);
        u.x = 0;
        u.y = i as i32;
        u.out_of_control = !controlled;
        c.spoils[COMBAT_SIDE_PLAYER] += spoils_factor.wrapping_mul(5).wrapping_mul(stack.count);
    }
}

/// Open-field foe band: the first 3 non-empty garrison stacks only, at
/// column W-1, rows 0..2.
pub fn combat_prepare_foe(c: &mut Combat, resources: &Resources, target: &CombatTarget) {
    let mut placed = 0;
    let slots = target.garrison.len().min(COMBAT_SLOTS);
    const MAX_BAND: usize = 3;
    for i in 0..slots {
        if placed >= MAX_BAND {
            break;
        }
        let src = target.garrison[i];
        if src.id.is_empty() || src.count == 0 {
            continue;
        }
        let Some(troop) = resources.troop_by_id(src.id.as_str()) else {
            continue;
        };
        let (index, spoils_factor) = (troop.index, troop.spoils_factor);
        let u = &mut c.units[COMBAT_SIDE_AI][placed];
        combat_init_unit(u, index, src.count);
        u.x = (COMBAT_W - 1) as i32;
        u.y = placed as i32;
        c.spoils[COMBAT_SIDE_AI] += spoils_factor.wrapping_mul(5).wrapping_mul(src.count);
        placed += 1;
    }
}

/// Castle siege: up to 5 garrison stacks. Positions are stamped later by
/// `combat_reset_match` from the castle layout table.
pub fn combat_prepare_castle(c: &mut Combat, resources: &Resources, target: &CombatTarget) {
    let mut placed = 0;
    let slots = target.garrison.len().min(COMBAT_SLOTS);
    for i in 0..slots {
        if placed >= COMBAT_SLOTS {
            break;
        }
        let src = target.garrison[i];
        if src.id.is_empty() || src.count == 0 {
            continue;
        }
        let Some(troop) = resources.troop_by_id(src.id.as_str()) else {
            continue;
        };
        let (index, spoils_factor) = (troop.index, troop.spoils_factor);
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][placed], index, src.count);
        c.spoils[COMBAT_SIDE_AI] += spoils_factor.wrapping_mul(5).wrapping_mul(src.count);
        placed += 1;
    }
}

/// Castle layout, decoded literally. `castle_umap` holds packed UIDs;
/// `castle_omap` codes 5..10 are decorative wall tiles, kept >= 4 so a
/// passability check stays "obstacle != 0".
const CASTLE_UMAP: [[u8; COMBAT_W]; COMBAT_H] = [
    [0, 8, 6, 7, 9, 0],
    [0, 0, 10, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 5, 3, 4, 0, 0],
    [0, 0, 1, 2, 0, 0],
];
const CASTLE_OMAP: [[u8; COMBAT_W]; COMBAT_H] = [
    [8, 0, 0, 0, 0, 9],
    [8, 0, 0, 0, 0, 9],
    [8, 0, 0, 0, 0, 9],
    [8, 0, 0, 0, 0, 9],
    [5, 7, 0, 0, 10, 6],
];

/// Stamp obstacles, place units, refresh per-match counters, prime the turn
/// machinery. Ports `combat_reset_match`.
pub fn combat_reset_match(c: &mut Combat, resources: &Resources) {
    c.omap = [[0; COMBAT_W + 1]; COMBAT_H + 1];
    c.umap = [[0; COMBAT_W + 1]; COMBAT_H + 1];

    if c.castle {
        for y in 0..COMBAT_H {
            for x in 0..COMBAT_W {
                c.omap[y][x] = CASTLE_OMAP[y][x];
                let uid = CASTLE_UMAP[y][x];
                if uid == 0 {
                    continue;
                }
                let (side, slot) = unpack_uid(uid);
                let u = &mut c.units[side][slot];
                if u.troop_idx < 0 || u.count == 0 {
                    continue;
                }
                u.x = x as i32;
                u.y = y as i32;
            }
        }
    } else {
        // Open-field random obstacles: columns 1..=W-3, ~10% per cell.
        // The RNG call order here is part of the fight's determinism.
        for j in 0..COMBAT_H {
            for i in 1..=COMBAT_W - 3 {
                if combat_rand(c, 0, 9) == 0 {
                    c.omap[j][i] = combat_rand(c, 1, 3) as u8;
                }
            }
        }
    }

    for s in 0..COMBAT_SIDES {
        for i in 0..COMBAT_SLOTS {
            let u = c.units[s][i];
            if u.troop_idx < 0 || u.count == 0 {
                continue;
            }
            let shots = resources
                .troop_by_index(u.troop_idx)
                .map(|troop| troop.ranged_ammo)
                .unwrap_or(0);
            let u = &mut c.units[s][i];
            u.shots = shots;
            u.injury = 0;
            u.frozen = false;
            u.turn_count = u.count;
            let (x, y) = (u.x as usize, u.y as usize);
            c.umap[y][x] = pack_uid(s, i);
        }
    }

    c.turn = 0;
    c.phase = 0;
    c.side = COMBAT_SIDE_PLAYER;
    c.unit_id = -1;
    c.spells_this_round = 0;
    c.first_kill_seen = false;
    c.stacks_destroyed = 0;
}

/// Resolve one attack. Ports `combat_hit_unit` minus its log lines, which
/// belong to the shell.
pub fn combat_hit_unit(
    c: &mut Combat,
    resources: &Resources,
    a_side: usize,
    a_id: usize,
    t_side: usize,
    t_id: usize,
    is_ranged: bool,
) -> i32 {
    c.units[a_side][a_id].turn_count = c.units[a_side][a_id].count;
    c.units[t_side][t_id].turn_count = c.units[t_side][t_id].count;
    combat_deal_damage(
        c,
        resources,
        a_side as i32,
        a_id as i32,
        t_side,
        t_id,
        is_ranged,
        false,
        0,
        false,
    )
}

/// Step one cell. Returns 0 (blocked), 1 (moved) or 2 (attacked).
pub fn combat_move_unit(
    c: &mut Combat,
    resources: &Resources,
    side: usize,
    id: usize,
    dx: i32,
    dy: i32,
) -> i32 {
    let (nx, ny) = (c.units[side][id].x + dx, c.units[side][id].y + dy);
    if !combat_in_bounds(nx, ny) {
        return 0;
    }
    let (nxu, nyu) = (nx as usize, ny as usize);
    if c.omap[nyu][nxu] != 0 {
        return 0;
    }
    let other = c.umap[nyu][nxu];
    if other != 0 {
        let (o_side, o_slot) = unpack_uid(other);
        if units_are_friendly(c, side, id, o_side, o_slot) {
            return 0;
        }
        combat_hit_unit(c, resources, side, id, o_side, o_slot, false);
        c.units[side][id].acted = true;
        return 2;
    }
    let (ox, oy) = (c.units[side][id].x as usize, c.units[side][id].y as usize);
    c.umap[oy][ox] = 0;
    let u = &mut c.units[side][id];
    u.x = nx;
    u.y = ny;
    c.umap[nyu][nxu] = pack_uid(side, id);
    let u = &mut c.units[side][id];
    if u.moves > 0 {
        u.moves -= 1;
    }
    if u.moves == 0 {
        u.acted = true;
    }
    1
}

/// Fly to an arbitrary free cell. Returns 0 (blocked) or 1 (moved).
pub fn combat_fly_unit(c: &mut Combat, side: usize, id: usize, nx: i32, ny: i32) -> i32 {
    if !combat_in_bounds(nx, ny) {
        return 0;
    }
    let (nxu, nyu) = (nx as usize, ny as usize);
    if c.omap[nyu][nxu] != 0 || c.umap[nyu][nxu] != 0 {
        return 0;
    }
    let (ox, oy) = (c.units[side][id].x as usize, c.units[side][id].y as usize);
    c.umap[oy][ox] = 0;
    let u = &mut c.units[side][id];
    u.x = nx;
    u.y = ny;
    c.umap[nyu][nxu] = pack_uid(side, id);
    let u = &mut c.units[side][id];
    if u.flights > 0 {
        u.flights -= 1;
    }
    if u.flights == 0 {
        u.acted = true;
    }
    1
}

/// Clear dead stacks and rebuild `umap`. Slot indices stay stable; C
/// deliberately does not reorder.
pub fn combat_compact(c: &mut Combat) {
    let mut any_died = false;
    for s in 0..COMBAT_SIDES {
        for i in 0..COMBAT_SLOTS {
            let u = &mut c.units[s][i];
            if u.dead {
                any_died = true;
                u.dead = false;
                u.count = 0;
                u.troop_idx = -1;
            }
        }
    }
    if !any_died {
        return;
    }
    c.umap = [[0; COMBAT_W + 1]; COMBAT_H + 1];
    for s in 0..COMBAT_SIDES {
        for i in 0..COMBAT_SLOTS {
            let u = c.units[s][i];
            if u.troop_idx < 0 || u.count == 0 {
                continue;
            }
            c.umap[u.y as usize][u.x as usize] = pack_uid(s, i);
        }
    }
    if !c.first_kill_seen {
        c.first_kill_seen = true;
    }
    c.stacks_destroyed += 1;
}

/// Refresh per-turn counters. `started_at` is the side whose turn just
/// ended; only the player-side wakeup clears frozen and regenerates.
pub fn combat_reset_turn(c: &mut Combat, resources: &Resources, started_at: usize) {
    for s in 0..COMBAT_SIDES {
        for i in 0..COMBAT_SLOTS {
            let u = c.units[s][i];
            if u.troop_idx < 0 || u.count == 0 {
                continue;
            }
            let Some(troop) = resources.troop_by_index(u.troop_idx) else {
                continue;
            };
            let (move_rate, abilities) = (troop.move_rate, troop.abilities);
            let u = &mut c.units[s][i];
            u.turn_count = u.count;
            u.retaliated = false;
            u.acted = false;
            u.moves = move_rate;
            u.flights = if abilities & TROOP_ABIL_FLY != 0 {
                2
            } else {
                0
            };
            if started_at == COMBAT_SIDE_AI {
                u.frozen = false;
                if abilities & TROOP_ABIL_REGEN != 0 {
                    u.injury = 0;
                }
            }
        }
    }
    c.phase = 0;
    c.spells_this_round = 0;
}

/// Next actable slot on the current side: scan forward, then wrap with
/// `phase++`. Returns -1 when the side is spent.
pub fn combat_next_unit(c: &mut Combat) -> i32 {
    for i in (c.unit_id + 1) as usize..COMBAT_SLOTS {
        let u = c.units[c.side][i];
        if u.troop_idx >= 0 && u.count > 0 && !u.acted {
            return i as i32;
        }
    }
    c.phase += 1;
    // `unit_id == -1` makes this an empty range, exactly as C's
    // `i <= c->unit_id` skips the wrap on the first prime.
    for i in 0..=c.unit_id {
        let u = c.units[c.side][i as usize];
        if u.troop_idx >= 0 && u.count > 0 && !u.acted {
            return i;
        }
    }
    -1
}

/// Switch side, reset, and prime the acting slot.
pub fn combat_next_turn(c: &mut Combat, resources: &Resources) {
    let prior = c.side;
    c.side = if c.side == COMBAT_SIDE_PLAYER {
        COMBAT_SIDE_AI
    } else {
        COMBAT_SIDE_PLAYER
    };
    combat_reset_turn(c, resources, prior);
    if c.side == COMBAT_SIDE_PLAYER {
        c.turn += 1;
    }
    c.unit_id = -1;
    let next = combat_next_unit(c);
    c.unit_id = if next >= 0 { next } else { 0 };
}

/// True when `side` has no living stacks left.
pub fn combat_test_dead(c: &Combat, side: usize) -> bool {
    !c.units[side]
        .iter()
        .any(|u| u.troop_idx >= 0 && u.count > 0)
}

/// True if any 8-neighbour cell holds a hostile unit. Ranged attacks are
/// blocked while surrounded.
pub fn combat_unit_surrounded(c: &Combat, side: usize, slot: usize) -> bool {
    let u = c.units[side][slot];
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (u.x + dx, u.y + dy);
            if !combat_in_bounds(nx, ny) {
                continue;
            }
            let other = c.umap[ny as usize][nx as usize];
            if other == 0 {
                continue;
            }
            let (o_side, o_slot) = unpack_uid(other);
            if !units_are_friendly(c, side, slot, o_side, o_slot) {
                return true;
            }
        }
    }
    false
}

/// Pick a target for the acting unit, returning a packed UID or 0. Far
/// targets prefer shooters; otherwise lower HP-per-creature wins.
pub fn ai_pick_target(
    c: &Combat,
    resources: &Resources,
    side: usize,
    slot: usize,
    nearby: bool,
) -> u8 {
    let self_unit = c.units[side][slot];
    let ooc = self_unit.out_of_control;
    let mut best_uid = 0;
    let mut best_score = -1;
    for s in 0..COMBAT_SIDES {
        if s == side && !ooc {
            continue;
        }
        for i in 0..COMBAT_SLOTS {
            if s == side && i == slot && !ooc {
                continue;
            }
            let t = c.units[s][i];
            if t.troop_idx < 0 || t.count == 0 {
                continue;
            }
            if nearby && !unit_touching(&self_unit, &t) {
                continue;
            }
            let score = if !nearby && t.shots > 0 {
                10_000
            } else if let Some(td) = resources.troop_by_index(t.troop_idx) {
                1_000 - td.hit_points
            } else {
                0
            };
            if score > best_score {
                best_score = score;
                best_uid = pack_uid(s, i);
            }
        }
    }
    best_uid
}

/// One step toward (tx, ty) that minimizes distance and is passable.
/// Iteration order is outer dy = +1, 0, -1 and inner dx = -1, 0, +1; the
/// resulting tie-break is observable in AI movement.
pub fn unit_move_offset(c: &Combat, side: usize, slot: usize, tx: i32, ty: i32) -> (i32, i32) {
    let self_unit = c.units[side][slot];
    let mut best_dx = 0;
    let mut best_dy = 0;
    let mut best = u32::MAX;
    for dy in (-1..=1).rev() {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (self_unit.x + dx, self_unit.y + dy);
            if !combat_in_bounds(nx, ny) {
                continue;
            }
            if c.omap[ny as usize][nx as usize] != 0 {
                continue;
            }
            let uid = c.umap[ny as usize][nx as usize];
            if uid != 0 {
                let (o_side, o_slot) = unpack_uid(uid);
                if units_are_friendly(c, side, slot, o_side, o_slot) {
                    continue;
                }
            }
            let d = calc_distance(nx, ny, tx, ty);
            if d < best {
                best = d;
                best_dx = dx;
                best_dy = dy;
            }
        }
    }
    (best_dx, best_dy)
}

/// One AI turn. Returns non-zero when the turn was consumed.
pub fn combat_ai_action(c: &mut Combat, resources: &Resources) -> i32 {
    if c.unit_id < 0 {
        return 0;
    }
    let side = c.side;
    let slot = c.unit_id as usize;
    let u = c.units[side][slot];
    let Some(troop) = resources.troop_by_index(u.troop_idx) else {
        c.units[side][slot].acted = true;
        return 1;
    };
    let abilities = troop.abilities;

    if u.frozen {
        c.units[side][slot].acted = true;
        return 1;
    }

    let close_uid = ai_pick_target(c, resources, side, slot, true);

    // Ranged attack on a far target.
    if close_uid == 0 && u.shots > 0 && !combat_unit_surrounded(c, side, slot) {
        let far_uid = ai_pick_target(c, resources, side, slot, false);
        if far_uid != 0 {
            let (t_side, t_slot) = unpack_uid(far_uid);
            combat_hit_unit(c, resources, side, slot, t_side, t_slot, true);
            c.units[side][slot].acted = true;
            return 1;
        }
    }

    // Fly to a free cell adjacent to a far target.
    if close_uid == 0 && abilities & TROOP_ABIL_FLY != 0 && u.flights > 0 {
        let far_uid = ai_pick_target(c, resources, side, slot, false);
        if far_uid != 0 {
            let (t_side, t_slot) = unpack_uid(far_uid);
            let target = c.units[t_side][t_slot];
            for dy in (-1..=1).rev() {
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let (nx, ny) = (target.x + dx, target.y + dy);
                    if !combat_in_bounds(nx, ny) {
                        continue;
                    }
                    if c.omap[ny as usize][nx as usize] != 0
                        || c.umap[ny as usize][nx as usize] != 0
                    {
                        continue;
                    }
                    if combat_fly_unit(c, side, slot, nx, ny) != 0 {
                        c.units[side][slot].acted = true;
                        return 1;
                    }
                }
            }
        }
    }

    // Walk toward the close-or-far target.
    let mut target_uid = close_uid;
    if target_uid == 0 {
        target_uid = ai_pick_target(c, resources, side, slot, false);
    }
    if target_uid != 0 {
        let (t_side, t_slot) = unpack_uid(target_uid);
        let target = c.units[t_side][t_slot];
        let (ox, oy) = unit_move_offset(c, side, slot, target.x, target.y);
        if ox != 0 || oy != 0 {
            let moved = combat_move_unit(c, resources, side, slot, ox, oy);
            if moved != 0 {
                c.units[side][slot].acted = true;
                return 1;
            }
        }
    }

    c.units[side][slot].acted = true;
    1
}

/// Resolve a whole fight with no renderer, driving both sides with the AI
/// policy. Ports `combat_run_headless`: on a win the player's army is
/// written back with its losses and gold gains the defender's spoils.
pub fn combat_run_headless(
    game: &mut Game,
    resources: &Resources,
    mode: CombatMode,
    target: &CombatTarget,
    cap_rounds: i32,
) -> CombatResult {
    let mut c = Combat::default();
    combat_init(&mut c, game, resources, mode);
    combat_seed_rng(&mut c, game, mode, target);
    combat_prepare_player(&mut c, game, resources);
    if mode == CombatMode::Castle {
        combat_prepare_castle(&mut c, resources, target);
    } else {
        combat_prepare_foe(&mut c, resources, target);
    }
    combat_reset_match(&mut c, resources);

    combat_reset_turn(&mut c, resources, COMBAT_SIDE_AI);
    c.unit_id = -1;
    let next = combat_next_unit(&mut c);
    if next < 0 {
        return CombatResult::Loss;
    }
    c.unit_id = next;

    let mut actions = 0;
    let max_actions = if cap_rounds > 0 {
        cap_rounds * 64
    } else {
        4096
    };
    // No-progress cutoff: when nothing damage-relevant changes across 8
    // rounds' worth of actions, the two sides are circling and never will
    // trade damage. Positions are deliberately excluded -- pure movement IS
    // the stall. The headless policy never casts, so C's spellbook term in
    // this fingerprint is constant here and is omitted.
    let mut stall_fp: i64 = -1;
    let mut stall_n = 0;
    let stall_cap = 8 * 64;
    while c.result == 0 && actions < max_actions {
        let mut fp: i64 = 0;
        for s in 0..COMBAT_SIDES {
            for i in 0..COMBAT_SLOTS {
                let u = c.units[s][i];
                fp = fp.wrapping_mul(31).wrapping_add(u.count as i64);
                fp = fp.wrapping_mul(31).wrapping_add(u.injury as i64);
                fp = fp.wrapping_mul(31).wrapping_add(u.shots as i64);
            }
        }
        if fp != stall_fp {
            stall_fp = fp;
            stall_n = 0;
        } else {
            stall_n += 1;
            if stall_n >= stall_cap {
                break;
            }
        }

        // Both sides use the AI policy: C's headless entry point passes
        // combat_ai_action as the default player policy.
        let acted = combat_ai_action(&mut c, resources);
        if acted != 0 {
            combat_compact(&mut c);
            if combat_test_dead(&c, COMBAT_SIDE_AI) {
                c.result = 1;
                break;
            }
            if combat_test_dead(&c, COMBAT_SIDE_PLAYER) {
                c.result = 2;
                break;
            }
            let n = combat_next_unit(&mut c);
            if n < 0 {
                combat_next_turn(&mut c, resources);
            } else {
                c.unit_id = n;
            }
        }
        actions += 1;
    }

    if c.result == 1 {
        game.stats.gold = game.stats.gold.wrapping_add(c.spoils[COMBAT_SIDE_AI]);
        for i in 0..COMBAT_SLOTS {
            let u = c.units[COMBAT_SIDE_PLAYER][i];
            if u.troop_idx < 0 || u.count == 0 {
                game.army[i] = ArmyStack::default();
            } else if let Some(troop) = resources.troop_by_index(u.troop_idx) {
                game.army[i].id.copy_from_str(troop.id.as_str());
                game.army[i].count = u.count;
            }
        }
        game.compact_army();
        return CombatResult::Win;
    }
    // A cap-hitting or stalled fight resolves as a loss so callers always
    // see a definitive outcome.
    CombatResult::Loss
}

pub fn combat_deal_damage(
    c: &mut Combat,
    resources: &Resources,
    a_side: i32,
    a_id: i32,
    t_side: usize,
    t_id: usize,
    is_ranged: bool,
    is_external: bool,
    external_damage: i32,
    retaliation: bool,
) -> i32 {
    let tt = match resources.troop_by_index(c.units[t_side][t_id].troop_idx) {
        Some(t) => t,
        None => return 0,
    };

    let mut demon_kills = 0;
    let mut final_damage;
    let mut retaliation = retaliation;
    let attacker = if a_side >= 0 && a_id >= 0 {
        Some((a_side as usize, a_id as usize))
    } else {
        None
    };

    if is_external {
        final_damage = external_damage;
    } else {
        let (as_idx, ai_idx) = match attacker {
            Some(pair) => pair,
            None => return 0,
        };
        let ut = match resources.troop_by_index(c.units[as_idx][ai_idx].troop_idx) {
            Some(t) => t,
            None => return 0,
        };

        if (ut.abilities & TROOP_ABIL_SCYTHE) != 0 && combat_rand(c, 1, 100) > 89 {
            let target_count = c.units[t_side][t_id].count;
            demon_kills = target_count / 2 + if target_count % 2 != 0 { 1 } else { 0 };
        }

        let dmg = if is_ranged {
            retaliation = true;
            if c.units[as_idx][ai_idx].shots > 0 {
                c.units[as_idx][ai_idx].shots -= 1;
            }

            if (ut.abilities & TROOP_ABIL_MAGIC) != 0 {
                if (tt.abilities & TROOP_ABIL_IMMUNE) != 0 {
                    return -1;
                }
                ut.ranged_min
            } else {
                combat_rand(c, ut.ranged_min, ut.ranged_max)
            }
        } else {
            combat_rand(c, ut.melee_min, ut.melee_max)
        };

        let total = dmg.wrapping_mul(c.units[as_idx][ai_idx].turn_count);
        let skill_diff = ut.skill_level + 5 - tt.skill_level;
        final_damage = total.wrapping_mul(skill_diff) / 10;

        if c.heroes_present[as_idx] && !c.units[as_idx][ai_idx].out_of_control {
            match troop_morale_for_unit(c, resources, as_idx, ai_idx) {
                CombatMorale::Low => final_damage /= 2,
                CombatMorale::High => final_damage += final_damage / 2,
                CombatMorale::Normal => {}
            }
        }

        if (c.powers[as_idx] & COMBAT_POWER_INCREASED_DAMAGE) != 0 {
            final_damage += final_damage / 2;
        }
        if (c.powers[t_side] & COMBAT_POWER_QUARTER_PROTECTION) != 0 {
            final_damage = (final_damage / 4).wrapping_mul(3);
        }
    }

    final_damage = final_damage.wrapping_add(c.units[t_side][t_id].injury);
    final_damage = final_damage.wrapping_add(tt.hit_points.wrapping_mul(demon_kills));

    let mut kills = if tt.hit_points > 0 {
        final_damage / tt.hit_points
    } else {
        c.units[t_side][t_id].count
    };
    c.units[t_side][t_id].injury = if tt.hit_points > 0 {
        final_damage % tt.hit_points
    } else {
        0
    };

    if kills < c.units[t_side][t_id].count {
        c.units[t_side][t_id].count -= kills;
    } else {
        c.units[t_side][t_id].dead = true;
        c.units[t_side][t_id].count = 0;
        kills = c.units[t_side][t_id].turn_count;
        final_damage = kills.wrapping_mul(tt.hit_points);
    }

    if !is_external {
        if let Some((as_idx, ai_idx)) = attacker {
            if let Some(ut) = resources.troop_by_index(c.units[as_idx][ai_idx].troop_idx) {
                if (ut.abilities & TROOP_ABIL_ABSORB) != 0 {
                    c.units[as_idx][ai_idx].count += kills;
                }

                if (ut.abilities & TROOP_ABIL_LEECH) != 0 {
                    let hp = ut.hit_points;
                    if hp > 0 {
                        c.units[as_idx][ai_idx].count += final_damage / hp;
                        if c.units[as_idx][ai_idx].count > c.units[as_idx][ai_idx].max_count {
                            c.units[as_idx][ai_idx].count = c.units[as_idx][ai_idx].max_count;
                            c.units[as_idx][ai_idx].injury = 0;
                        }
                    }
                }

                if !retaliation
                    && !c.units[t_side][t_id].retaliated
                    && c.units[t_side][t_id].count > 0
                {
                    c.units[t_side][t_id].retaliated = true;
                    combat_deal_damage(
                        c,
                        resources,
                        t_side as i32,
                        t_id as i32,
                        as_idx,
                        ai_idx,
                        false,
                        false,
                        0,
                        true,
                    );
                }
            }
        }
    }

    kills
}

pub fn combat_test_digest(
    resources: &Resources,
    seed: u64,
    attacker_id: &str,
    attacker_count: i32,
    defender_id: &str,
    defender_count: i32,
    rounds: i32,
) -> u64 {
    let mut c = Combat {
        rng_state: if seed != 0 { seed } else { 1 },
        ..Combat::default()
    };

    let Some(at) = resources.troop_by_id(attacker_id) else {
        return 0;
    };
    let Some(dt) = resources.troop_by_id(defender_id) else {
        return 0;
    };

    combat_init_unit(
        &mut c.units[COMBAT_SIDE_PLAYER][0],
        at.index,
        attacker_count,
    );
    c.units[COMBAT_SIDE_PLAYER][0].x = 2;
    c.units[COMBAT_SIDE_PLAYER][0].y = 2;
    c.units[COMBAT_SIDE_PLAYER][0].shots = at.ranged_ammo;

    combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], dt.index, defender_count);
    c.units[COMBAT_SIDE_AI][0].x = 3;
    c.units[COMBAT_SIDE_AI][0].y = 2;
    c.units[COMBAT_SIDE_AI][0].shots = dt.ranged_ammo;

    let mut digest = FNV64_OFFSET;
    for _ in 0..rounds {
        c.units[0][0].turn_count = c.units[0][0].count;
        c.units[1][0].turn_count = c.units[1][0].count;

        let kills = combat_deal_damage(&mut c, resources, 0, 0, 1, 0, false, false, 0, false);
        let fields = [
            c.units[0][0].count as u64,
            c.units[0][0].injury as u64,
            c.units[1][0].count as u64,
            c.units[1][0].injury as u64,
            if kills < 0 {
                0xFFFF_FFFF
            } else {
                kills as u32 as u64
            },
            c.rng_state,
        ];

        for field in fields {
            digest ^= field;
            digest = digest.wrapping_mul(FNV64_PRIME);
        }

        c.units[0][0].retaliated = false;
        c.units[1][0].retaliated = false;
        if c.units[0][0].count == 0 || c.units[1][0].count == 0 {
            break;
        }
    }

    digest
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CombatMorale {
    Normal = 0,
    Low = 1,
    High = 2,
}

fn morale_to_rank(r: u8) -> CombatMorale {
    match r {
        b'L' => CombatMorale::Low,
        b'H' => CombatMorale::High,
        _ => CombatMorale::Normal,
    }
}

fn troop_morale_for_unit(
    c: &Combat,
    resources: &Resources,
    side: usize,
    slot: usize,
) -> CombatMorale {
    let self_unit = c.units[side][slot];
    if self_unit.troop_idx < 0 {
        return CombatMorale::Normal;
    }

    let Some(ts) = resources.troop_by_index(self_unit.troop_idx) else {
        return CombatMorale::Normal;
    };
    let mut morale = CombatMorale::High as i32;

    for other in c.units[side] {
        if other.troop_idx < 0 || other.count == 0 {
            continue;
        }
        let Some(to) = resources.troop_by_index(other.troop_idx) else {
            continue;
        };
        let rank = morale_to_rank(resources.morale_result(to.morale_group, ts.morale_group)) as i32;
        if rank < morale {
            morale = rank;
        }
    }

    match morale {
        1 => CombatMorale::Low,
        2 => CombatMorale::High,
        _ => CombatMorale::Normal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GAME_ARMY_SLOTS;
    use crate::tables::FixedStr;

    /// peasants: 1hp cheap fodder. archers: ranged, 2 ammo. gargoyles: FLY.
    /// ogres: 5hp, slow. All move_rate 2 unless noted.
    fn combat_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"archers","skill_level":2,"hit_points":2,"move_rate":2,"melee":[1,2],"ranged":[2,3,2],"recruit_cost":20,"spoils_factor":2,"abilities":"","morale_group":"A"},
                {"index":2,"id":"gargoyles","skill_level":3,"hit_points":3,"move_rate":2,"melee":[2,4],"ranged":[0,0,0],"recruit_cost":30,"spoils_factor":3,"abilities":"FLY","morale_group":"A"},
                {"index":3,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":1,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    fn unit(id: &str, count: i32) -> Unit {
        Unit {
            id: FixedStr::from_str(id),
            count,
        }
    }

    fn game_with_army(stacks: &[(&str, i32)]) -> Game {
        let mut game = Game::default();
        game.seed = 12_345;
        game.stats.leadership_current = 10_000;
        for (slot, (id, count)) in stacks.iter().enumerate() {
            game.army[slot].id.copy_from_str(id);
            game.army[slot].count = *count;
        }
        game
    }

    fn target<'a>(garrison: &'a [Unit], key: &'a str) -> CombatTarget<'a> {
        CombatTarget {
            name: "Foe Band",
            garrison,
            seed_key: key,
        }
    }

    // ----- seeding -----

    #[test]
    fn seed_is_a_pure_function_of_world_seed_identity_and_mode() {
        let mut a = Combat::default();
        let mut b = Combat::default();
        let mut game = game_with_army(&[("peasants", 10)]);
        let garrison = [unit("ogres", 3)];

        combat_seed_rng(&mut a, &game, CombatMode::Foe, &target(&garrison, "foe_7"));
        // Casualty history and purse must not perturb the seed.
        game.stats.gold = 9_999;
        game.stats.followers_killed = 512;
        combat_seed_rng(&mut b, &game, CombatMode::Foe, &target(&garrison, "foe_7"));
        assert_eq!(a.rng_state, b.rng_state);

        let mut c = Combat::default();
        combat_seed_rng(&mut c, &game, CombatMode::Foe, &target(&garrison, "foe_8"));
        assert_ne!(a.rng_state, c.rng_state);

        let mut d = Combat::default();
        combat_seed_rng(
            &mut d,
            &game,
            CombatMode::Castle,
            &target(&garrison, "foe_7"),
        );
        assert_ne!(a.rng_state, d.rng_state);
    }

    #[test]
    fn seed_falls_back_to_the_display_name_without_a_seed_key() {
        let game = game_with_army(&[("peasants", 1)]);
        let garrison = [unit("ogres", 1)];
        let named = CombatTarget {
            name: "foe_7",
            garrison: &garrison,
            seed_key: "",
        };
        let keyed = target(&garrison, "foe_7");

        let mut a = Combat::default();
        let mut b = Combat::default();
        combat_seed_rng(&mut a, &game, CombatMode::Foe, &named);
        combat_seed_rng(&mut b, &game, CombatMode::Foe, &keyed);
        assert_eq!(a.rng_state, b.rng_state);
    }

    // ----- setup -----

    #[test]
    fn player_army_lines_up_in_column_zero_and_accrues_spoils() {
        let resources = combat_fixture();
        let game = game_with_army(&[("peasants", 10), ("archers", 4)]);
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Foe);
        combat_prepare_player(&mut c, &game, &resources);

        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].troop_idx, 0);
        assert_eq!(
            (
                c.units[COMBAT_SIDE_PLAYER][0].x,
                c.units[COMBAT_SIDE_PLAYER][0].y
            ),
            (0, 0)
        );
        assert_eq!(
            (
                c.units[COMBAT_SIDE_PLAYER][1].x,
                c.units[COMBAT_SIDE_PLAYER][1].y
            ),
            (0, 1)
        );
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][2].troop_idx, -1);
        // spoils_factor * 5 * count, summed: 1*5*10 + 2*5*4 = 90.
        assert_eq!(c.spoils[COMBAT_SIDE_PLAYER], 90);
    }

    #[test]
    fn a_stack_over_leadership_enters_combat_out_of_control() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[("ogres", 3)]);
        // 5hp * 3 = 15; exactly 15 is still controlled, 14 is not.
        game.stats.leadership_current = 15;
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Foe);
        combat_prepare_player(&mut c, &game, &resources);
        assert!(!c.units[COMBAT_SIDE_PLAYER][0].out_of_control);

        game.stats.leadership_current = 14;
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Foe);
        combat_prepare_player(&mut c, &game, &resources);
        assert!(c.units[COMBAT_SIDE_PLAYER][0].out_of_control);
    }

    #[test]
    fn out_of_control_units_are_hostile_to_their_own_side() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[("ogres", 3), ("peasants", 1)]);
        game.stats.leadership_current = 5;
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Foe);
        combat_prepare_player(&mut c, &game, &resources);

        assert!(c.units[COMBAT_SIDE_PLAYER][0].out_of_control);
        assert!(!units_are_friendly(
            &c,
            COMBAT_SIDE_PLAYER,
            0,
            COMBAT_SIDE_PLAYER,
            1
        ));
        assert!(units_are_friendly(
            &c,
            COMBAT_SIDE_PLAYER,
            1,
            COMBAT_SIDE_PLAYER,
            1
        ));
    }

    #[test]
    fn open_field_foe_band_is_capped_at_three_stacks() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        let garrison = [
            unit("peasants", 5),
            unit("archers", 5),
            unit("ogres", 5),
            unit("gargoyles", 5),
            unit("peasants", 5),
        ];
        combat_prepare_foe(&mut c, &resources, &target(&garrison, "k"));

        assert_eq!(c.units[COMBAT_SIDE_AI][2].troop_idx, 3);
        assert_eq!(c.units[COMBAT_SIDE_AI][3].troop_idx, -1);
        for slot in 0..3 {
            assert_eq!(c.units[COMBAT_SIDE_AI][slot].x, (COMBAT_W - 1) as i32);
            assert_eq!(c.units[COMBAT_SIDE_AI][slot].y, slot as i32);
        }
        // Only the three placed stacks accrue spoils: (1+2+4) * 5 * 5.
        assert_eq!(c.spoils[COMBAT_SIDE_AI], 175);
    }

    #[test]
    fn empty_garrison_entries_are_skipped_without_consuming_a_band_slot() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        let garrison = [
            unit("", 0),
            unit("peasants", 0),
            unit("archers", 7),
            unit("ogres", 2),
        ];
        combat_prepare_foe(&mut c, &resources, &target(&garrison, "k"));

        assert_eq!(c.units[COMBAT_SIDE_AI][0].troop_idx, 1);
        assert_eq!(c.units[COMBAT_SIDE_AI][0].count, 7);
        assert_eq!(c.units[COMBAT_SIDE_AI][1].troop_idx, 3);
        assert_eq!(c.units[COMBAT_SIDE_AI][2].troop_idx, -1);
    }

    #[test]
    fn castle_mode_takes_five_stacks_and_stamps_the_fixed_layout() {
        let resources = combat_fixture();
        let game = game_with_army(&[("peasants", 4), ("archers", 4)]);
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Castle);
        combat_prepare_player(&mut c, &game, &resources);
        let garrison = [
            unit("ogres", 3),
            unit("ogres", 3),
            unit("ogres", 3),
            unit("ogres", 3),
            unit("ogres", 3),
        ];
        combat_prepare_castle(&mut c, &resources, &target(&garrison, "castle0"));
        combat_reset_match(&mut c, &resources);

        assert_eq!(c.units[COMBAT_SIDE_AI][4].troop_idx, 3);
        // UID 1 is player slot 0, at (2, 4) in the layout table.
        assert_eq!(
            (
                c.units[COMBAT_SIDE_PLAYER][0].x,
                c.units[COMBAT_SIDE_PLAYER][0].y
            ),
            (2, 4)
        );
        // UID 6 is AI slot 0, at (2, 0).
        assert_eq!(
            (c.units[COMBAT_SIDE_AI][0].x, c.units[COMBAT_SIDE_AI][0].y),
            (2, 0)
        );
        // Walls stamped down both flanks.
        assert_ne!(c.omap[0][0], 0);
        assert_ne!(c.omap[0][5], 0);
        assert_eq!(c.omap[2][2], 0);
        assert_eq!(c.umap[4][2], pack_uid(COMBAT_SIDE_PLAYER, 0));
    }

    #[test]
    fn open_field_obstacles_only_land_in_the_middle_columns() {
        let resources = combat_fixture();
        let game = game_with_army(&[("peasants", 4)]);
        let garrison = [unit("ogres", 3)];
        // Sweep seeds so the random stamp is exercised broadly.
        for seed in 0..40u64 {
            let mut game = game.clone();
            game.seed = seed;
            let mut c = Combat::default();
            combat_init(&mut c, &game, &resources, CombatMode::Foe);
            combat_seed_rng(&mut c, &game, CombatMode::Foe, &target(&garrison, "k"));
            combat_prepare_player(&mut c, &game, &resources);
            combat_prepare_foe(&mut c, &resources, &target(&garrison, "k"));
            combat_reset_match(&mut c, &resources);

            for y in 0..COMBAT_H {
                for x in 0..COMBAT_W {
                    if !(1..=COMBAT_W - 3).contains(&x) {
                        assert_eq!(c.omap[y][x], 0, "obstacle outside columns 1..=3 at {x},{y}");
                    } else {
                        assert!(c.omap[y][x] <= 3);
                    }
                }
            }
        }
    }

    #[test]
    fn reset_match_loads_ammo_and_stamps_the_unit_map() {
        let resources = combat_fixture();
        let game = game_with_army(&[("archers", 4)]);
        let garrison = [unit("peasants", 3)];
        let mut c = Combat::default();
        combat_init(&mut c, &game, &resources, CombatMode::Foe);
        combat_prepare_player(&mut c, &game, &resources);
        combat_prepare_foe(&mut c, &resources, &target(&garrison, "k"));
        combat_reset_match(&mut c, &resources);

        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].shots, 2);
        assert_eq!(c.units[COMBAT_SIDE_AI][0].shots, 0);
        assert_eq!(c.umap[0][0], pack_uid(COMBAT_SIDE_PLAYER, 0));
        assert_eq!(c.umap[0][5], pack_uid(COMBAT_SIDE_AI, 0));
        assert_eq!(c.side, COMBAT_SIDE_PLAYER);
        assert_eq!(c.unit_id, -1);
    }

    // ----- movement -----

    fn two_unit_board(resources: &Resources) -> Combat {
        let mut c = Combat::default();
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 0, 10);
        c.units[COMBAT_SIDE_PLAYER][0].x = 0;
        c.units[COMBAT_SIDE_PLAYER][0].y = 0;
        c.units[COMBAT_SIDE_PLAYER][0].moves = 2;
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], 0, 10);
        c.units[COMBAT_SIDE_AI][0].x = 2;
        c.units[COMBAT_SIDE_AI][0].y = 0;
        c.umap[0][0] = pack_uid(COMBAT_SIDE_PLAYER, 0);
        c.umap[0][2] = pack_uid(COMBAT_SIDE_AI, 0);
        let _ = resources;
        c
    }

    #[test]
    fn move_relocates_spends_a_move_and_marks_acted_when_spent() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);

        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, 0, 1),
            1
        );
        assert_eq!(
            (
                c.units[COMBAT_SIDE_PLAYER][0].x,
                c.units[COMBAT_SIDE_PLAYER][0].y
            ),
            (0, 1)
        );
        assert_eq!(c.umap[0][0], 0);
        assert_eq!(c.umap[1][0], pack_uid(COMBAT_SIDE_PLAYER, 0));
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].moves, 1);
        assert!(!c.units[COMBAT_SIDE_PLAYER][0].acted);

        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, 0, 1),
            1
        );
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].moves, 0);
        assert!(c.units[COMBAT_SIDE_PLAYER][0].acted);
    }

    #[test]
    fn move_is_blocked_by_edges_obstacles_and_friends_but_attacks_hostiles() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);

        // Off the edge.
        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, -1, 0),
            0
        );
        // Into an obstacle.
        c.omap[0][1] = 1;
        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, 1, 0),
            0
        );
        c.omap[0][1] = 0;
        // Into a friend.
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][1], 0, 5);
        c.units[COMBAT_SIDE_PLAYER][1].x = 1;
        c.units[COMBAT_SIDE_PLAYER][1].y = 0;
        c.umap[0][1] = pack_uid(COMBAT_SIDE_PLAYER, 1);
        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, 1, 0),
            0
        );
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].x, 0);

        // Into a hostile: resolves as an attack and consumes the turn.
        c.umap[0][1] = 0;
        c.units[COMBAT_SIDE_PLAYER][1] = CombatUnit::empty();
        c.units[COMBAT_SIDE_AI][0].x = 1;
        c.umap[0][2] = 0;
        c.umap[0][1] = pack_uid(COMBAT_SIDE_AI, 0);
        c.rng_state = 99;
        assert_eq!(
            combat_move_unit(&mut c, &resources, COMBAT_SIDE_PLAYER, 0, 1, 0),
            2
        );
        assert!(c.units[COMBAT_SIDE_PLAYER][0].acted);
        assert_eq!(
            c.units[COMBAT_SIDE_PLAYER][0].x, 0,
            "attacker does not enter the cell"
        );
    }

    #[test]
    fn fly_needs_a_cell_free_of_both_obstacles_and_units() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        c.units[COMBAT_SIDE_PLAYER][0].flights = 2;

        assert_eq!(
            combat_fly_unit(&mut c, COMBAT_SIDE_PLAYER, 0, 2, 0),
            0,
            "occupied"
        );
        c.omap[3][3] = 1;
        assert_eq!(
            combat_fly_unit(&mut c, COMBAT_SIDE_PLAYER, 0, 3, 3),
            0,
            "obstacle"
        );
        assert_eq!(
            combat_fly_unit(&mut c, COMBAT_SIDE_PLAYER, 0, 9, 9),
            0,
            "out of bounds"
        );

        assert_eq!(combat_fly_unit(&mut c, COMBAT_SIDE_PLAYER, 0, 4, 4), 1);
        assert_eq!(
            (
                c.units[COMBAT_SIDE_PLAYER][0].x,
                c.units[COMBAT_SIDE_PLAYER][0].y
            ),
            (4, 4)
        );
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].flights, 1);
        assert_eq!(c.umap[0][0], 0);
        assert_eq!(c.umap[4][4], pack_uid(COMBAT_SIDE_PLAYER, 0));
    }

    // ----- turn machinery -----

    #[test]
    fn compact_frees_dead_slots_rebuilds_the_map_and_counts_kills() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        c.units[COMBAT_SIDE_AI][0].dead = true;

        combat_compact(&mut c);

        assert_eq!(c.units[COMBAT_SIDE_AI][0].troop_idx, -1);
        assert_eq!(c.units[COMBAT_SIDE_AI][0].count, 0);
        assert!(!c.units[COMBAT_SIDE_AI][0].dead, "dead is one-shot");
        assert_eq!(c.umap[0][2], 0);
        assert_eq!(c.umap[0][0], pack_uid(COMBAT_SIDE_PLAYER, 0));
        assert!(c.first_kill_seen);
        assert_eq!(c.stacks_destroyed, 1);
    }

    #[test]
    fn compact_is_a_noop_when_nothing_died() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        let before = c.clone();

        combat_compact(&mut c);

        assert_eq!(c.umap, before.umap);
        assert_eq!(c.stacks_destroyed, 0);
        assert!(!c.first_kill_seen);
    }

    #[test]
    fn next_unit_scans_forward_then_wraps_with_a_phase_bump() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][2], 0, 5);

        c.unit_id = -1;
        assert_eq!(combat_next_unit(&mut c), 0);
        assert_eq!(c.phase, 0);

        c.unit_id = 0;
        assert_eq!(combat_next_unit(&mut c), 2);
        assert_eq!(c.phase, 0);

        // Slot 2 acted, so the forward scan fails and the wrap finds slot 0.
        c.units[COMBAT_SIDE_PLAYER][2].acted = true;
        c.unit_id = 2;
        assert_eq!(combat_next_unit(&mut c), 0);
        assert_eq!(c.phase, 1, "wrap bumps the phase");

        c.units[COMBAT_SIDE_PLAYER][0].acted = true;
        c.unit_id = 2;
        assert_eq!(combat_next_unit(&mut c), -1);
    }

    #[test]
    fn next_turn_swaps_sides_refreshes_budgets_and_counts_player_rounds() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        c.side = COMBAT_SIDE_PLAYER;
        c.units[COMBAT_SIDE_PLAYER][0].acted = true;
        c.units[COMBAT_SIDE_PLAYER][0].moves = 0;

        combat_next_turn(&mut c, &resources);
        assert_eq!(c.side, COMBAT_SIDE_AI);
        assert_eq!(c.turn, 0, "the turn counter advances on the player wakeup");
        // peasants move_rate 2, no FLY.
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].moves, 2);
        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].flights, 0);
        assert!(!c.units[COMBAT_SIDE_PLAYER][0].acted);

        combat_next_turn(&mut c, &resources);
        assert_eq!(c.side, COMBAT_SIDE_PLAYER);
        assert_eq!(c.turn, 1);
    }

    #[test]
    fn only_the_player_wakeup_thaws_frozen_stacks() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        c.units[COMBAT_SIDE_PLAYER][0].frozen = true;

        // Player turn ending -> AI acts: no thaw.
        combat_reset_turn(&mut c, &resources, COMBAT_SIDE_PLAYER);
        assert!(c.units[COMBAT_SIDE_PLAYER][0].frozen);

        // AI turn ending -> player wakeup: thaw.
        combat_reset_turn(&mut c, &resources, COMBAT_SIDE_AI);
        assert!(!c.units[COMBAT_SIDE_PLAYER][0].frozen);
    }

    #[test]
    fn flying_stacks_get_two_flights_each_turn() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 2, 3);

        combat_reset_turn(&mut c, &resources, COMBAT_SIDE_AI);

        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].flights, 2);
    }

    #[test]
    fn test_dead_reports_a_wiped_side() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        assert!(!combat_test_dead(&c, COMBAT_SIDE_AI));

        c.units[COMBAT_SIDE_AI][0].count = 0;
        assert!(combat_test_dead(&c, COMBAT_SIDE_AI));
        assert!(!combat_test_dead(&c, COMBAT_SIDE_PLAYER));
    }

    #[test]
    fn surrounded_counts_hostile_neighbours_only() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        assert!(!combat_unit_surrounded(&c, COMBAT_SIDE_PLAYER, 0));

        // Friendly neighbour does not block the bow.
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][1], 0, 5);
        c.units[COMBAT_SIDE_PLAYER][1].x = 1;
        c.units[COMBAT_SIDE_PLAYER][1].y = 0;
        c.umap[0][1] = pack_uid(COMBAT_SIDE_PLAYER, 1);
        assert!(!combat_unit_surrounded(&c, COMBAT_SIDE_PLAYER, 0));

        // Hostile neighbour does.
        c.umap[0][1] = 0;
        c.units[COMBAT_SIDE_PLAYER][1] = CombatUnit::empty();
        c.units[COMBAT_SIDE_AI][0].x = 1;
        c.units[COMBAT_SIDE_AI][0].y = 1;
        c.umap[0][2] = 0;
        c.umap[1][1] = pack_uid(COMBAT_SIDE_AI, 0);
        assert!(
            combat_unit_surrounded(&c, COMBAT_SIDE_PLAYER, 0),
            "diagonal counts"
        );
    }

    // ----- AI -----

    #[test]
    fn far_targeting_prefers_shooters_over_soft_targets() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 0, 10);
        c.units[COMBAT_SIDE_PLAYER][0].x = 0;
        c.units[COMBAT_SIDE_PLAYER][0].y = 0;
        // Slot 0 peasants (1hp, softest), slot 1 archers holding ammo.
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], 0, 5);
        c.units[COMBAT_SIDE_AI][0].x = 5;
        c.units[COMBAT_SIDE_AI][0].y = 0;
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][1], 1, 5);
        c.units[COMBAT_SIDE_AI][1].x = 5;
        c.units[COMBAT_SIDE_AI][1].y = 1;
        c.units[COMBAT_SIDE_AI][1].shots = 2;

        let uid = ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, false);
        assert_eq!(
            unpack_uid(uid),
            (COMBAT_SIDE_AI, 1),
            "shooter outranks the 1hp stack"
        );

        // Out of ammo, the soft target wins on lower hit points.
        c.units[COMBAT_SIDE_AI][1].shots = 0;
        let uid = ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, false);
        assert_eq!(unpack_uid(uid), (COMBAT_SIDE_AI, 0));
    }

    #[test]
    fn nearby_targeting_only_sees_adjacent_stacks() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        assert_eq!(
            ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, true),
            0
        );

        c.units[COMBAT_SIDE_AI][0].x = 1;
        assert_ne!(
            ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, true),
            0
        );
    }

    #[test]
    fn under_control_units_never_target_their_own_side() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        // Self is ogres (5hp, score 995); the ally is peasants (1hp, 999).
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 3, 10);
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][1], 0, 10);
        c.units[COMBAT_SIDE_PLAYER][1].x = 1;

        assert_eq!(
            ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, false),
            0
        );

        // Out of control, its own ally becomes a valid target.
        c.units[COMBAT_SIDE_PLAYER][0].out_of_control = true;
        let uid = ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, false);
        assert_eq!(unpack_uid(uid), (COMBAT_SIDE_PLAYER, 1));
    }

    #[test]
    fn an_out_of_control_stack_is_even_eligible_to_target_itself() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 0, 10);
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][1], 0, 10);
        c.units[COMBAT_SIDE_PLAYER][1].x = 1;
        c.units[COMBAT_SIDE_PLAYER][0].out_of_control = true;

        // C drops the self-skip along with the own-side skip once a stack is
        // out of control, and the tie-break is a strict `>`, so an all-equal
        // scan keeps the first candidate -- itself.
        let uid = ai_pick_target(&c, &resources, COMBAT_SIDE_PLAYER, 0, false);
        assert_eq!(unpack_uid(uid), (COMBAT_SIDE_PLAYER, 0));
    }

    #[test]
    fn move_offset_steps_toward_the_target_and_refuses_blocked_ground() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);

        // Target dead east: step east.
        assert_eq!(unit_move_offset(&c, COMBAT_SIDE_PLAYER, 0, 5, 0), (1, 0));

        // Wall off the whole eastern column of neighbours; the best remaining
        // step is whatever the C iteration order reaches first.
        c.omap[0][1] = 1;
        c.omap[1][1] = 1;
        let (ox, oy) = unit_move_offset(&c, COMBAT_SIDE_PLAYER, 0, 5, 0);
        assert_eq!(
            c.omap[oy as usize][ox as usize], 0,
            "never steps into a wall"
        );
    }

    #[test]
    fn ai_shoots_a_far_target_when_it_has_ammo_and_room() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        c.rng_state = 7;
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 1, 6);
        c.units[COMBAT_SIDE_PLAYER][0].x = 0;
        c.units[COMBAT_SIDE_PLAYER][0].y = 0;
        c.units[COMBAT_SIDE_PLAYER][0].shots = 2;
        c.umap[0][0] = pack_uid(COMBAT_SIDE_PLAYER, 0);
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], 0, 20);
        c.units[COMBAT_SIDE_AI][0].x = 5;
        c.units[COMBAT_SIDE_AI][0].y = 0;
        c.umap[0][5] = pack_uid(COMBAT_SIDE_AI, 0);
        c.side = COMBAT_SIDE_PLAYER;
        c.unit_id = 0;

        assert_eq!(combat_ai_action(&mut c, &resources), 1);

        assert!(c.units[COMBAT_SIDE_AI][0].count < 20, "the volley landed");
        assert_eq!(
            c.units[COMBAT_SIDE_PLAYER][0].x, 0,
            "shooters do not advance"
        );
        assert!(c.units[COMBAT_SIDE_PLAYER][0].acted);
    }

    #[test]
    fn a_surrounded_shooter_walks_instead_of_shooting() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        c.rng_state = 7;
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 1, 6);
        c.units[COMBAT_SIDE_PLAYER][0].x = 2;
        c.units[COMBAT_SIDE_PLAYER][0].y = 2;
        c.units[COMBAT_SIDE_PLAYER][0].shots = 2;
        c.units[COMBAT_SIDE_PLAYER][0].moves = 2;
        c.umap[2][2] = pack_uid(COMBAT_SIDE_PLAYER, 0);
        // Adjacent hostile blocks the bow; it is also the close target, so the
        // AI melees it.
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], 0, 20);
        c.units[COMBAT_SIDE_AI][0].x = 3;
        c.units[COMBAT_SIDE_AI][0].y = 2;
        c.umap[2][3] = pack_uid(COMBAT_SIDE_AI, 0);
        c.side = COMBAT_SIDE_PLAYER;
        c.unit_id = 0;

        assert_eq!(combat_ai_action(&mut c, &resources), 1);

        assert_eq!(c.units[COMBAT_SIDE_PLAYER][0].shots, 2, "ammo untouched");
        assert!(c.units[COMBAT_SIDE_AI][0].count < 20, "melee connected");
    }

    #[test]
    fn a_frozen_stack_passes_its_turn() {
        let resources = combat_fixture();
        let mut c = two_unit_board(&resources);
        c.units[COMBAT_SIDE_PLAYER][0].frozen = true;
        c.side = COMBAT_SIDE_PLAYER;
        c.unit_id = 0;

        assert_eq!(combat_ai_action(&mut c, &resources), 1);

        assert!(c.units[COMBAT_SIDE_PLAYER][0].acted);
        assert_eq!(
            c.units[COMBAT_SIDE_PLAYER][0].x, 0,
            "no movement while frozen"
        );
        assert_eq!(
            c.units[COMBAT_SIDE_AI][0].count, 10,
            "no attack while frozen"
        );
    }

    #[test]
    fn a_flier_closes_on_a_distant_target() {
        let resources = combat_fixture();
        let mut c = Combat::default();
        c.rng_state = 3;
        combat_init_unit(&mut c.units[COMBAT_SIDE_PLAYER][0], 2, 4);
        c.units[COMBAT_SIDE_PLAYER][0].x = 0;
        c.units[COMBAT_SIDE_PLAYER][0].y = 0;
        c.units[COMBAT_SIDE_PLAYER][0].flights = 2;
        c.umap[0][0] = pack_uid(COMBAT_SIDE_PLAYER, 0);
        combat_init_unit(&mut c.units[COMBAT_SIDE_AI][0], 0, 10);
        c.units[COMBAT_SIDE_AI][0].x = 5;
        c.units[COMBAT_SIDE_AI][0].y = 2;
        c.umap[2][5] = pack_uid(COMBAT_SIDE_AI, 0);
        c.side = COMBAT_SIDE_PLAYER;
        c.unit_id = 0;

        assert_eq!(combat_ai_action(&mut c, &resources), 1);

        let flier = c.units[COMBAT_SIDE_PLAYER][0];
        assert!(
            unit_touching(&flier, &c.units[COMBAT_SIDE_AI][0]),
            "landed adjacent"
        );
        assert_eq!(flier.flights, 1);
    }

    // ----- headless resolution -----

    #[test]
    fn a_winnable_fight_writes_losses_back_and_pays_spoils() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[("ogres", 50)]);
        game.stats.gold = 100;
        let garrison = [unit("peasants", 2)];

        let result = combat_run_headless(
            &mut game,
            &resources,
            CombatMode::Foe,
            &target(&garrison, "weak_foe"),
            COMBAT_MAX_ROUNDS,
        );

        assert_eq!(result, CombatResult::Win);
        assert_eq!(game.army[0].id.as_str(), "ogres");
        assert!(game.army[0].count <= 50, "survivors written back");
        assert!(game.army[0].count > 0);
        // peasants spoils_factor 1 * 5 * 2 = 10.
        assert_eq!(game.stats.gold, 110);
    }

    #[test]
    fn a_hopeless_fight_resolves_as_a_loss_and_leaves_the_army_alone() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[("peasants", 1)]);
        game.stats.gold = 100;
        let garrison = [unit("ogres", 40), unit("ogres", 40), unit("ogres", 40)];

        let result = combat_run_headless(
            &mut game,
            &resources,
            CombatMode::Foe,
            &target(&garrison, "doom"),
            COMBAT_MAX_ROUNDS,
        );

        assert_eq!(result, CombatResult::Loss);
        assert_eq!(game.stats.gold, 100, "no spoils on a loss");
        assert_eq!(game.army[0].count, 1, "no writeback on a loss");
    }

    #[test]
    fn the_same_encounter_always_resolves_the_same_way() {
        let resources = combat_fixture();
        let garrison = [unit("archers", 12), unit("peasants", 30)];
        let mut outcomes = Vec::new();
        for _ in 0..4 {
            let mut game = game_with_army(&[("ogres", 12), ("archers", 9)]);
            game.stats.gold = 0;
            let result = combat_run_headless(
                &mut game,
                &resources,
                CombatMode::Foe,
                &target(&garrison, "repeatable"),
                COMBAT_MAX_ROUNDS,
            );
            outcomes.push((
                result,
                game.stats.gold,
                game.army[0].count,
                game.army[1].count,
            ));
        }
        assert!(
            outcomes.windows(2).all(|pair| pair[0] == pair[1]),
            "same inputs must resolve identically: {outcomes:?}"
        );
    }

    #[test]
    fn writeback_compacts_the_army_when_a_middle_stack_is_wiped() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[("ogres", 60), ("peasants", 1), ("ogres", 60)]);
        let garrison = [unit("peasants", 1)];

        let result = combat_run_headless(
            &mut game,
            &resources,
            CombatMode::Foe,
            &target(&garrison, "tiny"),
            COMBAT_MAX_ROUNDS,
        );
        assert_eq!(result, CombatResult::Win);

        // Whatever died, the surviving stacks stay contiguous from slot 0.
        let live = game.army.iter().filter(|s| s.count > 0).count();
        for slot in 0..live {
            assert!(game.army[slot].count > 0, "gap at slot {slot}");
        }
        for slot in live..GAME_ARMY_SLOTS {
            assert_eq!(game.army[slot].count, 0);
            assert!(game.army[slot].id.is_empty());
        }
    }

    #[test]
    fn an_empty_army_loses_before_the_first_action() {
        let resources = combat_fixture();
        let mut game = game_with_army(&[]);
        let garrison = [unit("peasants", 5)];

        let result = combat_run_headless(
            &mut game,
            &resources,
            CombatMode::Foe,
            &target(&garrison, "k"),
            COMBAT_MAX_ROUNDS,
        );

        assert_eq!(result, CombatResult::Loss);
        assert_eq!(game.stats.gold, 0);
    }

    #[test]
    fn a_fight_neither_side_can_win_resolves_as_a_loss() {
        let resources = combat_fixture();
        // Both sides immune to each other is not expressible here, so use the
        // degenerate case: a defender with no resolvable troops leaves the AI
        // side empty, which the player wins; invert it by giving the player an
        // unresolvable stack instead.
        let mut game = Game::default();
        game.seed = 1;
        game.stats.leadership_current = 1_000;
        game.army[0].id.copy_from_str("not_in_catalog");
        game.army[0].count = 5;
        let garrison = [unit("peasants", 5)];

        let result = combat_run_headless(
            &mut game,
            &resources,
            CombatMode::Foe,
            &target(&garrison, "k"),
            COMBAT_MAX_ROUNDS,
        );

        assert_eq!(result, CombatResult::Loss);
    }

    // ----- distance helpers -----

    #[test]
    fn distance_is_euclidean_in_fixed_point() {
        // isqrt32 returns sqrt in 16.16 fixed point and calc_distance
        // pre-shifts its argument by 16, so the combined scale is 2^24 per
        // cell. Only the ordering matters to the AI, but pinning the scale
        // catches a shift being dropped in translation.
        assert_eq!(calc_distance(0, 0, 1, 0), 1 << 24);
        assert_eq!(calc_distance(0, 0, 3, 4), 5 << 24);
        assert!(calc_distance(0, 0, 1, 1) > calc_distance(0, 0, 1, 0));
        assert_eq!(calc_distance(2, 2, 2, 2), 0);
    }
}
