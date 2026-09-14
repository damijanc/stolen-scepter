pub const CAT_ID_LEN: usize = 32;
pub const CAT_NAME_LEN: usize = 48;
pub const CAT_PATH_LEN: usize = 128;
pub const CAT_TEXT_LEN: usize = 160;
pub const CLASS_MAX_RANKS: usize = 4;
pub const CLASS_MAX_STARTING_TROOPS: usize = 2;
pub const CAT_CLASSES_MAX: usize = 8;
pub const CAT_SPELLS_MAX: usize = 32;
pub const CAT_TROOPS_MAX: usize = 32;
pub const CAT_VILLAINS_MAX: usize = 32;
pub const CAT_ARTIFACTS_MAX: usize = 16;
pub const TROOP_TIER_COUNTS: usize = 4;

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
    pub dwelling: FixedStr<CAT_ID_LEN>,
    pub max_population: i32,
    pub growth_per_week: i32,
    pub morale_group: u8,
    pub tier_counts: [i32; TROOP_TIER_COUNTS],
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
            dwelling: FixedStr::empty(),
            max_population: 0,
            growth_per_week: 0,
            morale_group: b'A',
            tier_counts: [0; TROOP_TIER_COUNTS],
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
        tier_counts: [i32; TROOP_TIER_COUNTS],
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
            dwelling: FixedStr::empty(),
            max_population: 0,
            growth_per_week: 0,
            morale_group,
            tier_counts,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellKind {
    Combat = 0,
    Adventure = 1,
}

impl Default for SpellKind {
    fn default() -> Self {
        Self::Combat
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpellDef {
    pub index: i32,
    pub id: FixedStr<CAT_ID_LEN>,
    pub name: FixedStr<CAT_NAME_LEN>,
    pub kind: SpellKind,
    pub cost: i32,
    pub description: FixedStr<CAT_TEXT_LEN>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactPower {
    Unknown = 0,
    IncreasedDamage = 1,
    QuarterProtection = 2,
    DoubleLeadership = 3,
    IncreaseCommission = 4,
    DoubleSpellPower = 5,
    DoubleMaxSpells = 6,
    CheaperBoats = 7,
}

impl Default for ArtifactPower {
    fn default() -> Self {
        Self::Unknown
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactDef {
    pub index: i32,
    pub id: FixedStr<CAT_ID_LEN>,
    pub name: FixedStr<CAT_NAME_LEN>,
    pub icon: FixedStr<CAT_PATH_LEN>,
    pub power: ArtifactPower,
    pub effect: FixedStr<CAT_TEXT_LEN>,
    pub puzzle_cell: i32,
    pub zone: FixedStr<CAT_ID_LEN>,
    pub local_idx: i32,
}

impl Default for ArtifactDef {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            icon: FixedStr::empty(),
            power: ArtifactPower::Unknown,
            effect: FixedStr::empty(),
            puzzle_cell: -1,
            zone: FixedStr::empty(),
            local_idx: 0,
        }
    }
}

impl Default for SpellDef {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            kind: SpellKind::default(),
            cost: 0,
            description: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RankDef {
    pub id: FixedStr<CAT_ID_LEN>,
    pub name: FixedStr<CAT_NAME_LEN>,
    pub villains_needed: i32,
    pub leadership: i32,
    pub max_spells: i32,
    pub spell_power: i32,
    pub commission: i32,
    pub knows_magic: bool,
    pub instant_army: i32,
}

impl Default for RankDef {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            villains_needed: 0,
            leadership: 0,
            max_spells: 0,
            spell_power: 0,
            commission: 0,
            knows_magic: false,
            instant_army: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassDef {
    pub index: i32,
    pub id: FixedStr<CAT_ID_LEN>,
    pub name: FixedStr<CAT_NAME_LEN>,
    pub portrait: FixedStr<CAT_PATH_LEN>,
    pub starting_gold: i32,
    pub starting_troops: [FixedStr<CAT_ID_LEN>; CLASS_MAX_STARTING_TROOPS],
    pub starting_counts: [i32; CLASS_MAX_STARTING_TROOPS],
    pub rank_count: usize,
    pub ranks: [RankDef; CLASS_MAX_RANKS],
}

impl Default for ClassDef {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            portrait: FixedStr::empty(),
            starting_gold: 0,
            starting_troops: [FixedStr::empty(); CLASS_MAX_STARTING_TROOPS],
            starting_counts: [0; CLASS_MAX_STARTING_TROOPS],
            rank_count: 0,
            ranks: [RankDef::default(); CLASS_MAX_RANKS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ClassStats {
    pub leadership: i32,
    pub max_spells: i32,
    pub spell_power: i32,
    pub commission: i32,
}

pub fn class_stats_at_rank(class: &ClassDef, mut rank_index: i32) -> ClassStats {
    if class.rank_count == 0 {
        return ClassStats::default();
    }
    if rank_index < 0 {
        rank_index = 0;
    }
    if rank_index as usize >= class.rank_count {
        rank_index = class.rank_count as i32 - 1;
    }

    let mut stats = ClassStats::default();
    for rank in class.ranks.iter().take(rank_index as usize + 1) {
        stats.leadership += rank.leadership;
        stats.max_spells += rank.max_spells;
        stats.spell_power += rank.spell_power;
        stats.commission += rank.commission;
    }
    stats
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VillainDef {
    pub index: i32,
    pub id: FixedStr<CAT_ID_LEN>,
    pub name: FixedStr<CAT_NAME_LEN>,
    pub portrait: FixedStr<CAT_PATH_LEN>,
    pub zone: FixedStr<CAT_ID_LEN>,
    pub reward: i32,
    pub puzzle_cell: i32,
    pub army_troops: [FixedStr<CAT_ID_LEN>; 5],
    pub army_counts: [i32; 5],
}

impl Default for VillainDef {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            portrait: FixedStr::empty(),
            zone: FixedStr::empty(),
            reward: 0,
            puzzle_cell: -1,
            army_troops: [FixedStr::empty(); 5],
            army_counts: [0; 5],
        }
    }
}
