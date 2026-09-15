use crate::resources::Resources;
use crate::tables::{
    TROOP_ABIL_ABSORB, TROOP_ABIL_IMMUNE, TROOP_ABIL_LEECH, TROOP_ABIL_MAGIC, TROOP_ABIL_SCYTHE,
};

const MULT: u64 = 25_214_903_917;
const INC: u64 = 11;

pub const COMBAT_SIDES: usize = 2;
pub const COMBAT_SLOTS: usize = 5;
pub const COMBAT_SIDE_PLAYER: usize = 0;
pub const COMBAT_SIDE_AI: usize = 1;

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
    pub rng_state: u64,
    pub powers: [u8; COMBAT_SIDES],
    pub heroes_present: [bool; COMBAT_SIDES],
}

impl Default for Combat {
    fn default() -> Self {
        Self {
            units: [[CombatUnit::empty(); COMBAT_SLOTS]; COMBAT_SIDES],
            rng_state: 0,
            powers: [0; COMBAT_SIDES],
            heroes_present: [false; COMBAT_SIDES],
        }
    }
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
