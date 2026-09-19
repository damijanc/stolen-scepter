use crate::game::{ArmyStack, Game, Unit};
use crate::resources::Resources;
use crate::tables::{
    ArtifactPower, TROOP_ABIL_ABSORB, TROOP_ABIL_FLY, TROOP_ABIL_IMMUNE, TROOP_ABIL_LEECH,
    TROOP_ABIL_MAGIC, TROOP_ABIL_REGEN, TROOP_ABIL_SCYTHE, TROOP_ABIL_UNDEAD,
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

/// One structured line a combat action appended to the log. The engine
/// records DATA (troop indices, counts), never text: a render-free crate must
/// not format a `strings.combat_log` banner. A shell maps each event to text.
///
/// Hits and the IMMUNE no-effect are NOT here: `combat_hit_unit` already
/// returns the kill count (`-1` for IMMUNE), so the shell formats those at its
/// own call sites. Only [`CombatEvent::Retaliate`] is emitted here, because it
/// happens inside the recursive retaliation deep in `combat_deal_damage`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatEvent {
    /// `retaliate`: the defender struck back, killing `kills` of the attacker.
    Retaliate { target: i32, kills: i32 },
    /// `cloned` / `resurrected`: count + troop index.
    CountTroop {
        count: i32,
        troop: i32,
        resurrected: bool,
    },
}

/// How many lines the combat log retains, C's `COMBAT_LOG_LINES`.
pub const COMBAT_LOG_LINES: usize = 8;

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
    /// The scrolling action log, oldest first. Output data only: it never
    /// feeds the rules, so it is excluded from the digest fingerprint.
    pub log: Vec<CombatEvent>,
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
            log: Vec::new(),
        }
    }
}

/// Append a structured line to the combat log, scrolling the oldest out past
/// [`COMBAT_LOG_LINES`]. Ports `combat_log`'s ring behaviour (minus the text).
pub fn combat_log_event(c: &mut Combat, event: CombatEvent) {
    if c.log.len() >= COMBAT_LOG_LINES {
        c.log.remove(0);
    }
    c.log.push(event);
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

/// Resolve one attack, returning the kill count (`-1` for an IMMUNE target).
/// Ports `combat_hit_unit`; the banner is the shell's, so the count is left
/// for the caller to turn into text.
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
                    let retaliator_troop = c.units[t_side][t_id].troop_idx;
                    let retal_kills = combat_deal_damage(
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
                    if retal_kills > 0 {
                        combat_log_event(
                            c,
                            CombatEvent::Retaliate {
                                target: retaliator_troop,
                                kills: retal_kills,
                            },
                        );
                    }
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

// ----- Combat spells ---------------------------------------------------------
//
// Spell ids match the pack's spell catalog ordering: 0 Clone, 1 Teleport,
// 2 Fireball, 3 Lightning, 4 Freeze, 5 Resurrect, 6 Turn Undead, with the
// adventure spells following at 7 and beyond.
//
// C reaches the caster through `Combat.heroes[side]`, a `Game *`. This port
// replaced that back-pointer with the `leadership`/`powers`/`heroes_present`
// snapshots taken at prepare time, because nothing else in combat needed to
// write back to the Game. A cast does: it spends a charge. So the caster is
// passed in explicitly here rather than reached through `Combat`.

pub const COMBAT_SPELL_CLONE: i32 = 0;
pub const COMBAT_SPELL_TELEPORT: i32 = 1;
pub const COMBAT_SPELL_FIREBALL: i32 = 2;
pub const COMBAT_SPELL_LIGHTNING: i32 = 3;
pub const COMBAT_SPELL_FREEZE: i32 = 4;
pub const COMBAT_SPELL_RESURRECT: i32 = 5;
pub const COMBAT_SPELL_TURN_UNDEAD: i32 = 6;

/// Which cells a pick may land on. Ports `PICK_FILTER_*`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickFilter {
    Any = 0,
    Empty = 1,
    AnyUnit = 2,
    Friendly = 3,
    Enemy = 4,
    Undead = 5,
}

/// Whether cell `(x, y)` is a legal pick under `filter`. Ports
/// `combat_cell_passes_filter`.
///
/// Friend-or-foe is judged against the CURRENTLY ACTING unit
/// (`Combat.unit_id`), not against `caster_side` alone, which is what lets
/// an out-of-control stack be targeted by its own side.
pub fn combat_cell_passes_filter(
    c: &Combat,
    resources: &Resources,
    x: usize,
    y: usize,
    caster_side: usize,
    filter: PickFilter,
) -> bool {
    let obstacle = c.omap[y][x];
    let uid = c.umap[y][x];
    let occupant = if uid != 0 {
        Some(unpack_uid(uid))
    } else {
        None
    };
    match filter {
        PickFilter::Any => true,
        PickFilter::Empty => uid == 0 && obstacle == 0,
        PickFilter::AnyUnit => uid != 0,
        PickFilter::Friendly => match occupant {
            None => false,
            Some((u_side, u_slot)) => {
                units_are_friendly(c, caster_side, c.unit_id as usize, u_side, u_slot)
            }
        },
        PickFilter::Enemy => match occupant {
            None => false,
            Some((u_side, u_slot)) => {
                !units_are_friendly(c, caster_side, c.unit_id as usize, u_side, u_slot)
            }
        },
        PickFilter::Undead => match occupant {
            None => false,
            Some((u_side, u_slot)) => {
                if units_are_friendly(c, caster_side, c.unit_id as usize, u_side, u_slot) {
                    return false;
                }
                // An undead ENEMY only: turn undead cannot be aimed at your
                // own skeletons. C reads the catalog through a global here;
                // this port threads Resources like everywhere else.
                resources
                    .troop_by_index(c.units[u_side][u_slot].troop_idx)
                    .is_some_and(|troop| troop.abilities & TROOP_ABIL_UNDEAD != 0)
            }
        },
    }
}

/// What a cast attempt did. Ports the `COMBAT_CAST_*` return codes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastResult {
    /// The cast was refused outright and nothing was touched: no hero, no
    /// magic, a spell already cast this round, an unknown spell, or no
    /// charge held.
    Illegal,
    /// The cast was legal but the target shrugged it off. NO charge is
    /// spent and the once-per-round latch does NOT move.
    NoEffect,
    Ok,
}

/// The direct-damage base for the three attack spells. Ports
/// `combat_spell_damage_base`, the single copy of 25/10/50.
fn combat_spell_damage_base(spell_idx: i32) -> i32 {
    match spell_idx {
        COMBAT_SPELL_FIREBALL => 25,
        COMBAT_SPELL_LIGHTNING => 10,
        COMBAT_SPELL_TURN_UNDEAD => 50,
        _ => 0,
    }
}

/// `base * spell_power`, with a spell power below 1 treated as 1. Ports
/// `spell_damage_value`.
pub fn spell_damage_value(base: i32, spell_power: i32) -> i32 {
    let spell_power = spell_power.max(1);
    base.wrapping_mul(spell_power)
}

/// The damage one cast deals to a non-IMMUNE target. Ports
/// `combat_spell_direct_damage`.
///
/// 0 for the enhancer spells, whose value has no closed form, and 0 for
/// turn undead against a living target.
pub fn combat_spell_direct_damage(spell_idx: i32, spell_power: i32, target_undead: bool) -> i32 {
    let base = combat_spell_damage_base(spell_idx);
    if base == 0 {
        return 0;
    }
    if spell_idx == COMBAT_SPELL_TURN_UNDEAD && !target_undead {
        return 0;
    }
    spell_damage_value(base, spell_power)
}

/// The legal target filter for each combat spell. Ports
/// `combat_spell_target_filter`, whose default arm is Enemy.
pub fn combat_spell_target_filter(spell_idx: i32) -> PickFilter {
    match spell_idx {
        COMBAT_SPELL_CLONE => PickFilter::Friendly,
        COMBAT_SPELL_TELEPORT => PickFilter::AnyUnit,
        COMBAT_SPELL_FIREBALL => PickFilter::Enemy,
        COMBAT_SPELL_LIGHTNING => PickFilter::Enemy,
        COMBAT_SPELL_FREEZE => PickFilter::Enemy,
        COMBAT_SPELL_RESURRECT => PickFilter::Friendly,
        COMBAT_SPELL_TURN_UNDEAD => PickFilter::Undead,
        _ => PickFilter::Enemy,
    }
}

/// Deal spell damage, which routes through the ordinary damage path as an
/// external, non-retaliating hit from no attacker. Ports `spell_damage`.
pub fn spell_damage(
    c: &mut Combat,
    resources: &Resources,
    t_side: usize,
    t_slot: usize,
    dmg: i32,
) -> i32 {
    combat_deal_damage(
        c, resources, -1, -1, t_side, t_slot, false, true, dmg, false,
    )
}

/// Clone a friendly stack. Ports `spell_clone`.
///
/// The existing injury is FOLDED IN before the division, so a hurt stack
/// converts its accumulated damage into bodies too, and the remainder is
/// carried back as the new injury rather than discarded.
pub fn spell_clone(c: &mut Combat, resources: &Resources, t_side: usize, t_slot: usize, sp: i32) {
    let unit = &c.units[t_side][t_slot];
    let Some(troop) = resources.troop_by_index(unit.troop_idx) else {
        return;
    };
    if troop.hit_points <= 0 {
        return;
    }
    let damage = sp.wrapping_mul(10).wrapping_add(unit.injury);
    let clones = damage / troop.hit_points;
    let residual = damage % troop.hit_points;
    let troop_idx = c.units[t_side][t_slot].troop_idx;
    let unit = &mut c.units[t_side][t_slot];
    unit.count = unit.count.wrapping_add(clones);
    unit.max_count = unit.max_count.wrapping_add(clones);
    unit.injury = residual;
    combat_log_event(
        c,
        CombatEvent::CountTroop {
            count: clones,
            troop: troop_idx,
            resurrected: false,
        },
    );
}

/// Move a stack to `(to_x, to_y)`. Ports `spell_teleport`.
///
/// Like C, this trusts the destination: it clears the old cell and stamps
/// the new one with no bounds or occupancy check of its own, because the
/// caller's target filter has already chosen the cell.
pub fn spell_teleport(c: &mut Combat, from_side: usize, from_slot: usize, to_x: i32, to_y: i32) {
    let (old_x, old_y) = (
        c.units[from_side][from_slot].x,
        c.units[from_side][from_slot].y,
    );
    c.umap[old_y as usize][old_x as usize] = 0;
    let unit = &mut c.units[from_side][from_slot];
    unit.x = to_x;
    unit.y = to_y;
    c.umap[to_y as usize][to_x as usize] = pack_uid(from_side, from_slot);
}

/// Freeze a stack so it cannot act. Ports `spell_freeze`, which returns -1
/// for an IMMUNE target and 1 otherwise.
pub fn spell_freeze(c: &mut Combat, resources: &Resources, t_side: usize, t_slot: usize) -> i32 {
    let troop = resources.troop_by_index(c.units[t_side][t_slot].troop_idx);
    if troop.is_some_and(|troop| troop.abilities & TROOP_ABIL_IMMUNE != 0) {
        return -1;
    }
    c.units[t_side][t_slot].frozen = true;
    1
}

/// Bring back fallen members of a stack. Ports `spell_resurrect`.
///
/// A wholly destroyed stack stays dead: the `count == 0` guard runs first.
/// Revival is capped at the stack's starting size, and clears the injury.
pub fn spell_resurrect(c: &mut Combat, t_side: usize, t_slot: usize, sp: i32) -> i32 {
    let unit = &c.units[t_side][t_slot];
    if unit.count == 0 {
        return 0;
    }
    let mut revived = sp;
    if revived <= 0 {
        revived = 1;
    }
    if unit.count.wrapping_add(revived) > unit.max_count {
        revived = unit.max_count.wrapping_sub(unit.count);
    }
    if revived <= 0 {
        return 0;
    }
    let troop_idx = c.units[t_side][t_slot].troop_idx;
    let unit = &mut c.units[t_side][t_slot];
    unit.count = unit.count.wrapping_add(revived);
    unit.injury = 0;
    combat_log_event(
        c,
        CombatEvent::CountTroop {
            count: revived,
            troop: troop_idx,
            resurrected: true,
        },
    );
    1
}

/// Cast a combat spell. Ports `combat_cast_spell`, the single dispatcher
/// that both a shell's apply phase and an autoplay policy go through.
///
/// `caster` is the Game of `side`; C reaches it through `Combat.heroes[]`.
/// The charge is spent and the once-per-round latch advances only on
/// [`CastResult::Ok`], so a spell an IMMUNE target shrugs off is free.
#[allow(clippy::too_many_arguments)]
pub fn combat_cast_spell(
    c: &mut Combat,
    resources: &Resources,
    caster: &mut Game,
    side: usize,
    spell_idx: i32,
    t_side: usize,
    t_slot: usize,
    dest_x: i32,
    dest_y: i32,
) -> CastResult {
    // Every precondition mutates nothing. The heroes_present guard is what
    // keeps AI-vs-AI combat inert: a cast there is impossible by
    // construction, which is why the digest goldens never see one.
    if side >= COMBAT_SIDES || !c.heroes_present[side] {
        return CastResult::Illegal;
    }
    if !caster.stats.knows_magic {
        return CastResult::Illegal;
    }
    if c.spells_this_round >= 1 {
        return CastResult::Illegal;
    }
    if !(0..=COMBAT_SPELL_TURN_UNDEAD).contains(&spell_idx) {
        return CastResult::Illegal;
    }
    let Ok(slot) = usize::try_from(spell_idx) else {
        return CastResult::Illegal;
    };
    if slot >= caster.spells.counts.len() || caster.spells.counts[slot] <= 0 {
        return CastResult::Illegal;
    }

    let sp = caster.stats.spell_power.max(1);
    let target_immune = resources
        .troop_by_index(c.units[t_side][t_slot].troop_idx)
        .is_some_and(|troop| troop.abilities & TROOP_ABIL_IMMUNE != 0);

    let casted = match spell_idx {
        COMBAT_SPELL_CLONE => {
            spell_clone(c, resources, t_side, t_slot, sp);
            true
        }
        COMBAT_SPELL_TELEPORT => {
            spell_teleport(c, t_side, t_slot, dest_x, dest_y);
            true
        }
        COMBAT_SPELL_FIREBALL | COMBAT_SPELL_LIGHTNING | COMBAT_SPELL_TURN_UNDEAD => {
            if target_immune {
                false
            } else {
                spell_damage(
                    c,
                    resources,
                    t_side,
                    t_slot,
                    spell_damage_value(combat_spell_damage_base(spell_idx), sp),
                );
                true
            }
        }
        COMBAT_SPELL_FREEZE => spell_freeze(c, resources, t_side, t_slot) == 1,
        COMBAT_SPELL_RESURRECT => spell_resurrect(c, t_side, t_slot, sp) == 1,
        _ => false,
    };

    if casted {
        caster.spells.counts[slot] = caster.spells.counts[slot].wrapping_sub(1);
        c.spells_this_round = c.spells_this_round.wrapping_add(1);
        return CastResult::Ok;
    }
    CastResult::NoEffect
}

#[cfg(test)]
mod spell_tests {
    use super::*;
    use crate::tables::FixedStr;

    /// peasants: 1hp. ogres: 5hp. dragons: IMMUNE. skeletons: UNDEAD.
    fn spell_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":1,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"},
                {"index":2,"id":"dragons","skill_level":5,"hit_points":200,"move_rate":3,"melee":[25,50],"ranged":[0,0,0],"recruit_cost":500,"spoils_factor":5,"abilities":"IMMUNE","morale_group":"A"},
                {"index":3,"id":"skeletons","skill_level":2,"hit_points":3,"move_rate":2,"melee":[2,3],"ranged":[0,0,0],"recruit_cost":20,"spoils_factor":2,"abilities":"UNDEAD","morale_group":"A"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    /// A caster who knows magic, holds a charge of every combat spell, and
    /// faces one enemy stack of `foe` at slot 0.
    fn duel(foe_idx: i32, foe_count: i32) -> (Combat, Game) {
        let mut c = Combat::default();
        c.heroes_present[0] = true;
        c.unit_id = 0;
        c.units[0][0].troop_idx = 0;
        c.units[0][0].count = 10;
        c.units[0][0].max_count = 10;
        c.units[0][0].x = 0;
        c.units[0][0].y = 0;
        c.umap[0][0] = pack_uid(0, 0);
        c.units[1][0].troop_idx = foe_idx;
        c.units[1][0].count = foe_count;
        c.units[1][0].max_count = foe_count;
        c.units[1][0].x = 3;
        c.units[1][0].y = 0;
        c.umap[0][3] = pack_uid(1, 0);

        let mut caster = Game::default();
        caster.stats.knows_magic = true;
        caster.stats.spell_power = 2;
        for slot in 0..=COMBAT_SPELL_TURN_UNDEAD as usize {
            caster.spells.counts[slot] = 1;
        }
        caster.army[0].id = FixedStr::from_str("peasants");
        caster.army[0].count = 10;
        (c, caster)
    }

    #[test]
    fn the_damage_bases_are_twenty_five_ten_and_fifty_times_spell_power() {
        assert_eq!(
            combat_spell_direct_damage(COMBAT_SPELL_FIREBALL, 2, false),
            50
        );
        assert_eq!(
            combat_spell_direct_damage(COMBAT_SPELL_LIGHTNING, 3, false),
            30
        );
        // Turn undead bites only the undead.
        assert_eq!(
            combat_spell_direct_damage(COMBAT_SPELL_TURN_UNDEAD, 2, true),
            100
        );
        assert_eq!(
            combat_spell_direct_damage(COMBAT_SPELL_TURN_UNDEAD, 2, false),
            0
        );
        // The enhancers have no closed-form damage.
        assert_eq!(combat_spell_direct_damage(COMBAT_SPELL_CLONE, 5, false), 0);
        assert_eq!(
            combat_spell_direct_damage(COMBAT_SPELL_RESURRECT, 5, false),
            0
        );
    }

    #[test]
    fn a_spell_power_below_one_is_treated_as_one() {
        // The floor is in spell_damage_value, so a rank with no spell power
        // still lands the base rather than nothing.
        assert_eq!(spell_damage_value(25, 0), 25);
        assert_eq!(spell_damage_value(25, -4), 25);
        assert_eq!(spell_damage_value(25, 1), 25);
    }

    #[test]
    fn each_spell_has_its_own_target_filter() {
        assert_eq!(
            combat_spell_target_filter(COMBAT_SPELL_CLONE),
            PickFilter::Friendly
        );
        assert_eq!(
            combat_spell_target_filter(COMBAT_SPELL_RESURRECT),
            PickFilter::Friendly
        );
        assert_eq!(
            combat_spell_target_filter(COMBAT_SPELL_TELEPORT),
            PickFilter::AnyUnit
        );
        assert_eq!(
            combat_spell_target_filter(COMBAT_SPELL_TURN_UNDEAD),
            PickFilter::Undead
        );
        assert_eq!(
            combat_spell_target_filter(COMBAT_SPELL_FIREBALL),
            PickFilter::Enemy
        );
        // The default arm is Enemy, so an unknown id is not a wildcard.
        assert_eq!(combat_spell_target_filter(99), PickFilter::Enemy);
    }

    #[test]
    fn cloning_folds_the_existing_injury_into_the_new_bodies() {
        let resources = spell_fixture();
        let (mut c, _) = duel(1, 4);
        // Target the 5hp ogres with 3 damage already taken. sp 2 gives
        // 20 + 3 = 23 damage-equivalent, which is 4 whole ogres remainder 3.
        c.units[1][0].injury = 3;
        spell_clone(&mut c, &resources, 1, 0, 2);

        assert_eq!(c.units[1][0].count, 8);
        // max_count rises too, so resurrect can later refill to the new size.
        assert_eq!(c.units[1][0].max_count, 8);
        // The remainder is carried back, not discarded.
        assert_eq!(c.units[1][0].injury, 3);
    }

    #[test]
    fn cloning_a_troop_with_no_hit_points_does_nothing() {
        let resources = spell_fixture();
        let (mut c, _) = duel(1, 4);
        // An index the catalogue does not hold.
        c.units[1][0].troop_idx = 99;
        spell_clone(&mut c, &resources, 1, 0, 2);
        assert_eq!(c.units[1][0].count, 4);
    }

    #[test]
    fn teleporting_moves_the_stack_and_its_map_stamp() {
        let (mut c, _) = duel(0, 4);
        assert_eq!(c.umap[0][3], pack_uid(1, 0));

        spell_teleport(&mut c, 1, 0, 5, 2);
        assert_eq!((c.units[1][0].x, c.units[1][0].y), (5, 2));
        // The old cell is cleared and the new one stamped.
        assert_eq!(c.umap[0][3], 0);
        assert_eq!(c.umap[2][5], pack_uid(1, 0));
    }

    #[test]
    fn freezing_is_refused_by_an_immune_target() {
        let resources = spell_fixture();
        let (mut c, _) = duel(2, 1);

        assert_eq!(spell_freeze(&mut c, &resources, 1, 0), -1);
        assert!(!c.units[1][0].frozen);
        // A mortal target freezes.
        c.units[1][0].troop_idx = 0;
        assert_eq!(spell_freeze(&mut c, &resources, 1, 0), 1);
        assert!(c.units[1][0].frozen);
    }

    #[test]
    fn resurrection_refills_to_the_starting_size_and_never_raises_the_dead() {
        let (mut c, _) = duel(0, 10);
        c.units[1][0].count = 6;
        c.units[1][0].injury = 2;

        // sp 3 revives 3.
        assert_eq!(spell_resurrect(&mut c, 1, 0, 3), 1);
        assert_eq!(c.units[1][0].count, 9);
        assert_eq!(c.units[1][0].injury, 0);

        // Capped at max_count: only 1 of the requested 5 is revived.
        assert_eq!(spell_resurrect(&mut c, 1, 0, 5), 1);
        assert_eq!(c.units[1][0].count, 10);
        // Already full: nothing to do.
        assert_eq!(spell_resurrect(&mut c, 1, 0, 5), 0);

        // A wholly destroyed stack stays destroyed.
        c.units[1][0].count = 0;
        assert_eq!(spell_resurrect(&mut c, 1, 0, 5), 0);
        assert_eq!(c.units[1][0].count, 0);
    }

    #[test]
    fn a_cast_spends_one_charge_and_latches_the_round() {
        let resources = spell_fixture();
        let (mut c, mut caster) = duel(0, 20);

        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_FIREBALL,
                1,
                0,
                0,
                0
            ),
            CastResult::Ok
        );
        assert_eq!(caster.spells.counts[COMBAT_SPELL_FIREBALL as usize], 0);
        assert_eq!(c.spells_this_round, 1);

        // One spell per round, whatever charges remain elsewhere.
        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_LIGHTNING,
                1,
                0,
                0,
                0
            ),
            CastResult::Illegal
        );
        assert_eq!(caster.spells.counts[COMBAT_SPELL_LIGHTNING as usize], 1);
    }

    #[test]
    fn a_spell_an_immune_target_shrugs_off_is_free() {
        let resources = spell_fixture();
        let (mut c, mut caster) = duel(2, 3);

        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_FIREBALL,
                1,
                0,
                0,
                0
            ),
            CastResult::NoEffect
        );
        // Neither the charge nor the once-per-round latch moved, so the
        // caster may try something else this round.
        assert_eq!(caster.spells.counts[COMBAT_SPELL_FIREBALL as usize], 1);
        assert_eq!(c.spells_this_round, 0);
        assert_eq!(c.units[1][0].count, 3);
    }

    #[test]
    fn a_cast_is_refused_without_a_hero_magic_or_a_charge() {
        let resources = spell_fixture();

        // No hero on this side: what keeps AI-vs-AI combat inert.
        let (mut c, mut caster) = duel(0, 20);
        c.heroes_present[0] = false;
        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_FIREBALL,
                1,
                0,
                0,
                0
            ),
            CastResult::Illegal
        );

        // Knows no magic.
        let (mut c, mut caster) = duel(0, 20);
        caster.stats.knows_magic = false;
        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_FIREBALL,
                1,
                0,
                0,
                0
            ),
            CastResult::Illegal
        );

        // Holds no charge.
        let (mut c, mut caster) = duel(0, 20);
        caster.spells.counts[COMBAT_SPELL_FIREBALL as usize] = 0;
        assert_eq!(
            combat_cast_spell(
                &mut c,
                &resources,
                &mut caster,
                0,
                COMBAT_SPELL_FIREBALL,
                1,
                0,
                0,
                0
            ),
            CastResult::Illegal
        );

        // An adventure spell id is out of combat's range.
        let (mut c, mut caster) = duel(0, 20);
        assert_eq!(
            combat_cast_spell(&mut c, &resources, &mut caster, 0, 7, 1, 0, 0, 0),
            CastResult::Illegal
        );
        assert_eq!(c.spells_this_round, 0);
    }

    #[test]
    fn the_cell_filter_separates_friend_foe_and_empty_ground() {
        let resources = spell_fixture();
        let (mut c, _) = duel(0, 4);
        c.omap[1][1] = 1;

        // (0,0) holds the acting friendly stack, (3,0) the enemy.
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            0,
            0,
            0,
            PickFilter::Friendly
        ));
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            0,
            0,
            0,
            PickFilter::Enemy
        ));
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            3,
            0,
            0,
            PickFilter::Enemy
        ));
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            3,
            0,
            0,
            PickFilter::Friendly
        ));
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            3,
            0,
            0,
            PickFilter::AnyUnit
        ));

        // Empty wants no unit AND no obstacle; Any takes anything at all.
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            2,
            0,
            0,
            PickFilter::Empty
        ));
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            1,
            1,
            0,
            PickFilter::Empty
        ));
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            1,
            1,
            0,
            PickFilter::Any
        ));
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            2,
            0,
            0,
            PickFilter::AnyUnit
        ));
    }

    #[test]
    fn the_undead_filter_wants_an_enemy_that_is_also_undead() {
        let resources = spell_fixture();
        let (mut c, _) = duel(3, 4);

        // An undead enemy passes.
        assert!(combat_cell_passes_filter(
            &c,
            &resources,
            3,
            0,
            0,
            PickFilter::Undead
        ));
        // A living enemy does not.
        c.units[1][0].troop_idx = 0;
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            3,
            0,
            0,
            PickFilter::Undead
        ));
        // Nor do your OWN skeletons: the friendly check runs first.
        c.units[0][0].troop_idx = 3;
        assert!(!combat_cell_passes_filter(
            &c,
            &resources,
            0,
            0,
            0,
            PickFilter::Undead
        ));
    }

    #[test]
    fn cloning_appends_a_count_troop_event() {
        let resources = spell_fixture();
        let (mut c, _) = duel(0, 5);
        spell_clone(&mut c, &resources, 0, 0, 2);
        assert!(matches!(
            c.log.last(),
            Some(&CombatEvent::CountTroop {
                resurrected: false,
                ..
            })
        ));
    }

    #[test]
    fn a_retaliation_appends_a_retaliate_event() {
        let resources = spell_fixture();
        let (mut c, _) = duel(1, 10); // 5hp ogres survive a peasant swing.
                                      // Ensure the defender survives to retaliate: use a weak melee roll by
                                      // giving the attacker a single peasant.
        c.units[0][0].count = 1;
        c.units[0][0].turn_count = 1;
        c.units[1][0].count = 10;
        combat_hit_unit(&mut c, &resources, 0, 0, 1, 0, false);
        assert!(c
            .log
            .iter()
            .any(|event| matches!(event, CombatEvent::Retaliate { .. })));
    }
}
