pub const CAT_ID_LEN: usize = 32;
pub const CAT_TROOPS_MAX: usize = 32;

pub const TROOP_ABIL_FLY: i32 = 1 << 0;
pub const TROOP_ABIL_REGEN: i32 = 1 << 1;
pub const TROOP_ABIL_MAGIC: i32 = 1 << 2;
pub const TROOP_ABIL_IMMUNE: i32 = 1 << 3;
pub const TROOP_ABIL_ABSORB: i32 = 1 << 4;
pub const TROOP_ABIL_LEECH: i32 = 1 << 5;
pub const TROOP_ABIL_SCYTHE: i32 = 1 << 6;
pub const TROOP_ABIL_UNDEAD: i32 = 1 << 7;

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct FixedStr<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> FixedStr<N> {
    pub const fn empty() -> Self {
        Self { bytes: [0; N] }
    }

    pub fn copy_from_str(&mut self, value: &str) {
        self.bytes = [0; N];
        if N == 0 {
            return;
        }

        let src = value.as_bytes();
        let len = src.len().min(N - 1);
        self.bytes[..len].copy_from_slice(&src[..len]);
    }

    pub fn from_str(value: &str) -> Self {
        let mut out = Self::empty();
        out.copy_from_str(value);
        out
    }

    pub fn as_str(&self) -> &str {
        let len = self.bytes.iter().position(|b| *b == 0).unwrap_or(N);
        std::str::from_utf8(&self.bytes[..len]).unwrap_or("")
    }

    pub fn is_empty(&self) -> bool {
        self.as_str().is_empty()
    }
}

impl<const N: usize> Default for FixedStr<N> {
    fn default() -> Self {
        Self::empty()
    }
}

impl<const N: usize> std::fmt::Debug for FixedStr<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("FixedStr").field(&self.as_str()).finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TroopDef {
    pub index: i32,
    pub id: FixedStr<CAT_ID_LEN>,
    pub skill_level: i32,
    pub hit_points: i32,
    pub move_rate: i32,
    pub melee_min: i32,
    pub melee_max: i32,
    pub ranged_min: i32,
    pub ranged_max: i32,
    pub ranged_ammo: i32,
    pub recruit_cost: i32,
    pub spoils_factor: i32,
    pub abilities: i32,
    pub morale_group: u8,
}

impl Default for TroopDef {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            skill_level: 0,
            hit_points: 0,
            move_rate: 0,
            melee_min: 0,
            melee_max: 0,
            ranged_min: 0,
            ranged_max: 0,
            ranged_ammo: 0,
            recruit_cost: 0,
            spoils_factor: 0,
            abilities: 0,
            morale_group: b'A',
        }
    }
}

impl TroopDef {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        index: i32,
        id: &str,
        skill_level: i32,
        hit_points: i32,
        move_rate: i32,
        melee_min: i32,
        melee_max: i32,
        ranged_min: i32,
        ranged_max: i32,
        ranged_ammo: i32,
        recruit_cost: i32,
        spoils_factor: i32,
        abilities: i32,
        morale_group: u8,
    ) -> Self {
        Self {
            index,
            id: FixedStr::from_str(id),
            skill_level,
            hit_points,
            move_rate,
            melee_min,
            melee_max,
            ranged_min,
            ranged_max,
            ranged_ammo,
            recruit_cost,
            spoils_factor,
            abilities,
            morale_group,
        }
    }
}

pub fn parse_troop_abilities(value: &str) -> i32 {
    let mut mask = 0;
    for part in value.split('|') {
        match part {
            "FLY" => mask |= TROOP_ABIL_FLY,
            "REGEN" => mask |= TROOP_ABIL_REGEN,
            "MAGIC" => mask |= TROOP_ABIL_MAGIC,
            "IMMUNE" => mask |= TROOP_ABIL_IMMUNE,
            "ABSORB" => mask |= TROOP_ABIL_ABSORB,
            "LEECH" => mask |= TROOP_ABIL_LEECH,
            "SCYTHE" => mask |= TROOP_ABIL_SCYTHE,
            "UNDEAD" => mask |= TROOP_ABIL_UNDEAD,
            "" => {}
            _ => {}
        }
    }
    mask
}
