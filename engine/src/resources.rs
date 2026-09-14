use crate::tables::{
    parse_troop_abilities, ClassDef, FixedStr, RankDef, TroopDef, VillainDef, CAT_CLASSES_MAX,
    CAT_TROOPS_MAX, CAT_VILLAINS_MAX, CLASS_MAX_RANKS, CLASS_MAX_STARTING_TROOPS,
};

pub const RES_MAX_CASTLES: usize = 32;
pub const RES_MAX_ZONES: usize = 8;
pub const RES_ID_LEN: usize = 32;
pub const RES_NAME_LEN: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResTime {
    pub day_steps: i32,
    pub week_days: i32,
    pub days_per_difficulty: [i32; 4],
}

impl Default for ResTime {
    fn default() -> Self {
        Self {
            day_steps: 40,
            week_days: 5,
            days_per_difficulty: [900, 600, 400, 200],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResWorld {
    pub starting_zone: FixedStr<RES_ID_LEN>,
    pub default_name: FixedStr<RES_NAME_LEN>,
    pub max_army_slots: i32,
    pub default_options: [i32; 7],
}

impl Default for ResWorld {
    fn default() -> Self {
        Self {
            starting_zone: FixedStr::from_str("continentia"),
            default_name: FixedStr::from_str("Hero"),
            max_army_slots: 5,
            default_options: [4, 1, 1, 1, 1, 0, 5],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResContract {
    pub cycle_length: i32,
    pub initial_last_contract: i32,
}

impl Default for ResContract {
    fn default() -> Self {
        Self {
            cycle_length: 5,
            initial_last_contract: 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZone {
    pub id: FixedStr<RES_ID_LEN>,
    pub is_home: bool,
    pub hero_spawn_x: i32,
    pub hero_spawn_y: i32,
    pub home_spawn_x: i32,
    pub home_spawn_y: i32,
}

impl Default for ResZone {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            is_home: false,
            hero_spawn_x: 0,
            hero_spawn_y: 0,
            home_spawn_x: -1,
            home_spawn_y: -1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ResCastleSpecial {
    pub excluded_from_contract: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResCastle {
    pub index: i32,
    pub id: FixedStr<RES_ID_LEN>,
    pub name: FixedStr<RES_NAME_LEN>,
    pub zone: FixedStr<RES_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub gate_x: i32,
    pub gate_y: i32,
    pub difficulty_tier: i32,
    pub special: ResCastleSpecial,
}

impl Default for ResCastle {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            zone: FixedStr::empty(),
            x: -1,
            y: -1,
            gate_x: -1,
            gate_y: -1,
            difficulty_tier: 0,
            special: ResCastleSpecial::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resources {
    pub time: ResTime,
    pub world: ResWorld,
    pub contract: ResContract,
    zones: [ResZone; RES_MAX_ZONES],
    zone_count: usize,
    castles: [ResCastle; RES_MAX_CASTLES],
    castle_count: usize,
    classes: [ClassDef; CAT_CLASSES_MAX],
    classes_count: usize,
    troops: [TroopDef; CAT_TROOPS_MAX],
    troops_count: usize,
    villains: [VillainDef; CAT_VILLAINS_MAX],
    villains_count: usize,
    morale_chart: [[u8; 5]; 5],
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            time: ResTime::default(),
            world: ResWorld::default(),
            contract: ResContract::default(),
            zones: [ResZone::default(); RES_MAX_ZONES],
            zone_count: 0,
            castles: [ResCastle::default(); RES_MAX_CASTLES],
            castle_count: 0,
            classes: [ClassDef::default(); CAT_CLASSES_MAX],
            classes_count: 0,
            troops: [TroopDef::default(); CAT_TROOPS_MAX],
            troops_count: 0,
            villains: [VillainDef::default(); CAT_VILLAINS_MAX],
            villains_count: 0,
            morale_chart: default_morale_chart(),
        }
    }
}

impl Resources {
    pub fn from_json_file(path: impl AsRef<std::path::Path>) -> Result<Self, ResourceError> {
        let input = std::fs::read_to_string(path).map_err(ResourceError::Io)?;
        Self::from_json_str(&input)
    }

    pub fn from_json_str(input: &str) -> Result<Self, ResourceError> {
        let root: serde_json::Value = serde_json::from_str(input).map_err(ResourceError::Json)?;
        let mut resources = Self::default();
        resources.parse_time(&root)?;
        resources.parse_world(&root)?;
        resources.parse_contract(&root)?;
        resources.parse_combat(&root)?;
        resources.parse_zones(&root)?;
        resources.parse_castles(&root)?;
        resources.parse_troops(&root)?;
        resources.parse_classes(&root)?;
        resources.parse_villains(&root)?;
        Ok(resources)
    }

    pub fn zone_count(&self) -> usize {
        self.zone_count
    }

    pub fn zone_by_index(&self, index: usize) -> Option<&ResZone> {
        self.zones.get(index).filter(|_| index < self.zone_count)
    }

    pub fn zone_by_id(&self, id: &str) -> Option<&ResZone> {
        self.zones[..self.zone_count]
            .iter()
            .find(|zone| zone.id.as_str() == id)
    }

    pub fn castle_count(&self) -> usize {
        self.castle_count
    }

    pub fn castle_by_index(&self, index: usize) -> Option<&ResCastle> {
        self.castles
            .get(index)
            .filter(|_| index < self.castle_count)
    }

    pub fn castle_by_id(&self, id: &str) -> Option<&ResCastle> {
        self.castles[..self.castle_count]
            .iter()
            .find(|castle| castle.id.as_str() == id)
    }

    pub fn classes_count(&self) -> usize {
        self.classes_count
    }

    pub fn class_by_index(&self, index: i32) -> Option<&ClassDef> {
        if index < 0 || index as usize >= self.classes_count {
            return None;
        }
        self.classes.get(index as usize)
    }

    pub fn troops_count(&self) -> usize {
        self.troops_count
    }

    pub fn troop_by_id(&self, id: &str) -> Option<&TroopDef> {
        self.troops[..self.troops_count]
            .iter()
            .find(|troop| troop.id.as_str() == id)
    }

    pub fn troop_by_index(&self, index: i32) -> Option<&TroopDef> {
        if index < 0 || index as usize >= self.troops_count {
            return None;
        }
        self.troops.get(index as usize)
    }

    pub fn villains_count(&self) -> usize {
        self.villains_count
    }

    pub fn villain_by_index(&self, index: i32) -> Option<&VillainDef> {
        if index < 0 {
            return None;
        }
        self.villains[..self.villains_count]
            .iter()
            .find(|villain| villain.index == index)
    }

    pub fn villain_by_id(&self, id: &str) -> Option<&VillainDef> {
        self.villains[..self.villains_count]
            .iter()
            .find(|villain| villain.id.as_str() == id)
    }

    pub fn morale_result(&self, my_group: u8, their_group: u8) -> u8 {
        let a = my_group.wrapping_sub(b'A');
        let b = their_group.wrapping_sub(b'A');
        if a > 4 || b > 4 {
            return b'N';
        }
        self.morale_chart[a as usize][b as usize]
    }

    fn parse_time(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(time) = root.get("time") else {
            return Ok(());
        };
        self.time.day_steps = optional_i32_field(time, "day_steps", self.time.day_steps)?;
        self.time.week_days = optional_i32_field(time, "week_days", self.time.week_days)?;
        if let Some(days) = time.get("days_per_difficulty") {
            self.time.days_per_difficulty[0] =
                optional_i32_field(days, "easy", self.time.days_per_difficulty[0])?;
            self.time.days_per_difficulty[1] =
                optional_i32_field(days, "normal", self.time.days_per_difficulty[1])?;
            self.time.days_per_difficulty[2] =
                optional_i32_field(days, "hard", self.time.days_per_difficulty[2])?;
            self.time.days_per_difficulty[3] =
                optional_i32_field(days, "impossible", self.time.days_per_difficulty[3])?;
        }
        Ok(())
    }

    fn parse_contract(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(contract) = root.get("contract") else {
            return Ok(());
        };
        self.contract.cycle_length =
            optional_i32_field(contract, "cycle_length", self.contract.cycle_length)?;
        self.contract.initial_last_contract = optional_i32_field(
            contract,
            "initial_last_contract",
            self.contract.initial_last_contract,
        )?;
        Ok(())
    }

    fn parse_world(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(world) = root.get("world") else {
            return Ok(());
        };
        if let Some(value) = optional_string_field(world, "starting_zone")? {
            self.world.starting_zone.copy_from_str(value);
        }
        if let Some(value) = optional_string_field(world, "default_name")? {
            self.world.default_name.copy_from_str(value);
        }
        self.world.max_army_slots =
            optional_i32_field(world, "max_army_slots", self.world.max_army_slots)?;
        if let Some(options) = world.get("default_options") {
            let values = options
                .as_array()
                .ok_or(ResourceError::InvalidField("world.default_options"))?;
            for (idx, value) in values.iter().take(7).enumerate() {
                let n = value
                    .as_i64()
                    .ok_or(ResourceError::InvalidField("world.default_options"))?;
                self.world.default_options[idx] = i32::try_from(n)
                    .map_err(|_| ResourceError::InvalidField("world.default_options"))?;
            }
        }
        Ok(())
    }

    fn parse_combat(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(combat) = root.get("combat") else {
            return Ok(());
        };
        let Some(chart) = combat.get("morale_chart") else {
            return Ok(());
        };

        let rows = chart
            .as_array()
            .ok_or(ResourceError::InvalidField("combat.morale_chart"))?;
        if rows.len() != 5 {
            return Err(ResourceError::InvalidField("combat.morale_chart"));
        }

        for (row_idx, row) in rows.iter().enumerate() {
            let cols = row
                .as_array()
                .ok_or(ResourceError::InvalidField("combat.morale_chart row"))?;
            if cols.len() != 5 {
                return Err(ResourceError::InvalidField("combat.morale_chart row"));
            }
            for (col_idx, value) in cols.iter().enumerate() {
                let s = value
                    .as_str()
                    .ok_or(ResourceError::InvalidField("combat.morale_chart value"))?;
                self.morale_chart[row_idx][col_idx] = s.as_bytes().first().copied().unwrap_or(b'N');
            }
        }

        Ok(())
    }

    fn parse_troops(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let troops = root
            .get("troops")
            .ok_or(ResourceError::MissingField("troops"))?
            .as_array()
            .ok_or(ResourceError::InvalidField("troops"))?;

        if troops.len() > CAT_TROOPS_MAX {
            return Err(ResourceError::TooManyTroops {
                count: troops.len(),
                max: CAT_TROOPS_MAX,
            });
        }

        for (slot, value) in troops.iter().enumerate() {
            self.troops[slot] = parse_troop(value)?;
        }
        self.troops_count = troops.len();
        Ok(())
    }

    fn parse_classes(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(classes) = root.get("classes") else {
            return Ok(());
        };
        let classes = classes
            .as_array()
            .ok_or(ResourceError::InvalidField("classes"))?;
        if classes.len() > CAT_CLASSES_MAX {
            return Err(ResourceError::TooManyClasses {
                count: classes.len(),
                max: CAT_CLASSES_MAX,
            });
        }

        for (slot, value) in classes.iter().enumerate() {
            self.classes[slot] = parse_class(value)?;
        }
        self.classes_count = classes.len();
        Ok(())
    }

    fn parse_zones(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(zones) = root.get("zones") else {
            return Ok(());
        };
        let zones = zones
            .as_array()
            .ok_or(ResourceError::InvalidField("zones"))?;
        if zones.len() > RES_MAX_ZONES {
            return Err(ResourceError::TooManyZones {
                count: zones.len(),
                max: RES_MAX_ZONES,
            });
        }

        for (slot, value) in zones.iter().enumerate() {
            self.zones[slot] = parse_zone(value)?;
        }
        self.zone_count = zones.len();
        Ok(())
    }

    fn parse_castles(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(castles) = root.get("castles") else {
            return Ok(());
        };
        let castles = castles
            .as_array()
            .ok_or(ResourceError::InvalidField("castles"))?;

        for (slot, value) in castles.iter().take(RES_MAX_CASTLES).enumerate() {
            self.castles[slot] = parse_castle(value)?;
            self.castle_count += 1;
        }
        Ok(())
    }

    fn parse_villains(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(villains) = root.get("villains") else {
            return Ok(());
        };
        let villains = villains
            .as_array()
            .ok_or(ResourceError::InvalidField("villains"))?;
        if villains.len() > CAT_VILLAINS_MAX {
            return Err(ResourceError::TooManyVillains {
                count: villains.len(),
                max: CAT_VILLAINS_MAX,
            });
        }

        let mut seen = [false; CAT_VILLAINS_MAX];
        for (slot, value) in villains.iter().enumerate() {
            let villain = parse_villain(value)?;
            if villain.index < 0
                || villain.index as usize >= CAT_VILLAINS_MAX
                || seen[villain.index as usize]
            {
                return Err(ResourceError::InvalidVillainIndex {
                    id: villain.id.as_str().to_owned(),
                    index: villain.index,
                    max: CAT_VILLAINS_MAX,
                });
            }
            seen[villain.index as usize] = true;
            self.villains[slot] = villain;
        }
        self.villains_count = villains.len();
        Ok(())
    }
}

#[derive(Debug)]
pub enum ResourceError {
    Io(std::io::Error),
    Json(serde_json::Error),
    MissingField(&'static str),
    InvalidField(&'static str),
    TooManyTroops { count: usize, max: usize },
    TooManyClasses { count: usize, max: usize },
    TooManyZones { count: usize, max: usize },
    TooManyVillains { count: usize, max: usize },
    InvalidVillainIndex { id: String, index: i32, max: usize },
}

impl std::fmt::Display for ResourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "could not read resource file: {err}"),
            Self::Json(err) => write!(f, "invalid JSON: {err}"),
            Self::MissingField(field) => write!(f, "missing required field `{field}`"),
            Self::InvalidField(field) => write!(f, "invalid field `{field}`"),
            Self::TooManyTroops { count, max } => {
                write!(f, "too many troops: got {count}, max {max}")
            }
            Self::TooManyClasses { count, max } => {
                write!(f, "too many classes: got {count}, max {max}")
            }
            Self::TooManyZones { count, max } => {
                write!(f, "too many zones: got {count}, max {max}")
            }
            Self::TooManyVillains { count, max } => {
                write!(f, "too many villains: got {count}, max {max}")
            }
            Self::InvalidVillainIndex { id, index, max } => {
                write!(
                    f,
                    "villain `{id}` has invalid or duplicate index {index}, max {}",
                    max.saturating_sub(1)
                )
            }
        }
    }
}

impl std::error::Error for ResourceError {}

fn parse_troop(value: &serde_json::Value) -> Result<TroopDef, ResourceError> {
    let melee = required_i32_array2(value, "melee")?;
    let ranged = required_i32_array3(value, "ranged")?;
    let morale_group = string_field(value, "morale_group")?
        .as_bytes()
        .first()
        .copied()
        .unwrap_or(b'A');

    Ok(TroopDef::new(
        i32_field(value, "index")?,
        string_field(value, "id")?,
        i32_field(value, "skill_level")?,
        i32_field(value, "hit_points")?,
        i32_field(value, "move_rate")?,
        melee[0],
        melee[1],
        ranged[0],
        ranged[1],
        ranged[2],
        i32_field(value, "recruit_cost")?,
        i32_field(value, "spoils_factor")?,
        parse_troop_abilities(string_field(value, "abilities")?),
        morale_group,
    ))
}

fn parse_class(value: &serde_json::Value) -> Result<ClassDef, ResourceError> {
    let mut class = ClassDef {
        index: i32_field(value, "index")?,
        ..ClassDef::default()
    };
    class.id.copy_from_str(string_field(value, "id")?);
    if let Some(name) = optional_string_field(value, "name")? {
        class.name.copy_from_str(name);
    }
    if let Some(portrait) = optional_string_field(value, "portrait")? {
        class.portrait.copy_from_str(portrait);
    }
    class.starting_gold = optional_i32_field(value, "starting_gold", 0)?;

    if let Some(starting_troops) = value.get("starting_troops") {
        let entries = starting_troops
            .as_array()
            .ok_or(ResourceError::InvalidField("starting_troops"))?;
        for (slot, entry) in entries.iter().take(CLASS_MAX_STARTING_TROOPS).enumerate() {
            if let Some(id) = optional_string_field(entry, "id")? {
                class.starting_troops[slot].copy_from_str(id);
            }
            class.starting_counts[slot] = optional_i32_field(entry, "count", 0)?;
        }
    }

    if let Some(ranks) = value.get("ranks") {
        let ranks = ranks
            .as_array()
            .ok_or(ResourceError::InvalidField("ranks"))?;
        for (slot, rank) in ranks.iter().take(CLASS_MAX_RANKS).enumerate() {
            class.ranks[slot] = parse_rank(rank)?;
            class.rank_count += 1;
        }
    }

    Ok(class)
}

fn parse_rank(value: &serde_json::Value) -> Result<RankDef, ResourceError> {
    let mut rank = RankDef::default();
    if let Some(id) = optional_string_field(value, "id")? {
        rank.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        rank.name.copy_from_str(name);
    }
    rank.villains_needed = optional_i32_field(value, "villains_needed", 0)?;
    rank.leadership = optional_i32_field(value, "leadership", 0)?;
    rank.max_spells = optional_i32_field(value, "max_spells", 0)?;
    rank.spell_power = optional_i32_field(value, "spell_power", 0)?;
    rank.commission = optional_i32_field(value, "commission", 0)?;
    rank.knows_magic = optional_bool_field(value, "knows_magic", false)?;
    rank.instant_army = optional_i32_field(value, "instant_army", 0)?;
    Ok(rank)
}

fn parse_zone(value: &serde_json::Value) -> Result<ResZone, ResourceError> {
    let mut zone = ResZone::default();
    zone.id.copy_from_str(string_field(value, "id")?);
    zone.is_home = optional_bool_field(value, "is_home", false)?;

    if let Some(hero_spawn) = value.get("hero_spawn") {
        zone.hero_spawn_x = optional_i32_field(hero_spawn, "x", 0)?;
        zone.hero_spawn_y = optional_i32_field(hero_spawn, "y", 0)?;
    }
    if let Some(home_spawn) = value.get("home_spawn") {
        zone.home_spawn_x = optional_i32_field(home_spawn, "x", -1)?;
        zone.home_spawn_y = optional_i32_field(home_spawn, "y", -1)?;
    }

    Ok(zone)
}

fn parse_castle(value: &serde_json::Value) -> Result<ResCastle, ResourceError> {
    let mut castle = ResCastle {
        index: optional_i32_field(value, "index", -1)?,
        x: optional_i32_field(value, "x", -1)?,
        y: optional_i32_field(value, "y", -1)?,
        difficulty_tier: optional_i32_field(value, "difficulty_tier", 0)?,
        ..ResCastle::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        castle.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        castle.name.copy_from_str(name);
    }
    if let Some(zone) = optional_string_field(value, "zone")? {
        castle.zone.copy_from_str(zone);
    }

    castle.gate_x = if castle.x >= 0 { castle.x } else { -1 };
    castle.gate_y = if castle.y >= 0 { castle.y + 1 } else { -1 };
    if let Some(gate) = value.get("gate") {
        castle.gate_x = optional_i32_field(gate, "x", castle.gate_x)?;
        castle.gate_y = optional_i32_field(gate, "y", castle.gate_y)?;
    }
    if let Some(special) = value.get("special") {
        castle.special.excluded_from_contract =
            optional_bool_field(special, "excluded_from_contract", false)?;
    }

    Ok(castle)
}

fn parse_villain(value: &serde_json::Value) -> Result<VillainDef, ResourceError> {
    let mut villain = VillainDef {
        index: optional_i32_field(value, "index", -1)?,
        reward: optional_i32_field(value, "reward", 0)?,
        puzzle_cell: optional_i32_field(value, "puzzle_cell", -1)?,
        ..VillainDef::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        villain.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        villain.name.copy_from_str(name);
    }
    if let Some(portrait) = optional_string_field(value, "portrait")? {
        villain.portrait.copy_from_str(portrait);
    }
    if let Some(zone) = optional_string_field(value, "zone")? {
        villain.zone.copy_from_str(zone);
    }

    if let Some(army) = value.get("army") {
        let entries = army.as_array().ok_or(ResourceError::InvalidField("army"))?;
        for (slot, entry) in entries.iter().take(5).enumerate() {
            if let Some(troop) = optional_string_field(entry, "troop")? {
                villain.army_troops[slot].copy_from_str(troop);
            }
            villain.army_counts[slot] = optional_i32_field(entry, "count", 0)?;
        }
    }

    Ok(villain)
}

fn string_field<'a>(
    value: &'a serde_json::Value,
    key: &'static str,
) -> Result<&'a str, ResourceError> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or(ResourceError::InvalidField(key))
}

fn optional_string_field<'a>(
    value: &'a serde_json::Value,
    key: &'static str,
) -> Result<Option<&'a str>, ResourceError> {
    match value.get(key) {
        Some(value) => value
            .as_str()
            .map(Some)
            .ok_or(ResourceError::InvalidField(key)),
        None => Ok(None),
    }
}

fn i32_field(value: &serde_json::Value, key: &'static str) -> Result<i32, ResourceError> {
    let n = value
        .get(key)
        .and_then(serde_json::Value::as_i64)
        .ok_or(ResourceError::InvalidField(key))?;
    i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))
}

fn optional_i32_field(
    value: &serde_json::Value,
    key: &'static str,
    default: i32,
) -> Result<i32, ResourceError> {
    let Some(value) = value.get(key) else {
        return Ok(default);
    };
    let n = value.as_i64().ok_or(ResourceError::InvalidField(key))?;
    i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))
}

fn optional_bool_field(
    value: &serde_json::Value,
    key: &'static str,
    default: bool,
) -> Result<bool, ResourceError> {
    match value.get(key) {
        Some(value) => value.as_bool().ok_or(ResourceError::InvalidField(key)),
        None => Ok(default),
    }
}

fn required_i32_array2(
    value: &serde_json::Value,
    key: &'static str,
) -> Result<[i32; 2], ResourceError> {
    let values = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .ok_or(ResourceError::InvalidField(key))?;
    if values.len() != 2 {
        return Err(ResourceError::InvalidField(key));
    }
    Ok([
        json_array_i32(values, key, 0)?,
        json_array_i32(values, key, 1)?,
    ])
}

fn required_i32_array3(
    value: &serde_json::Value,
    key: &'static str,
) -> Result<[i32; 3], ResourceError> {
    let values = value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .ok_or(ResourceError::InvalidField(key))?;
    if values.len() != 3 {
        return Err(ResourceError::InvalidField(key));
    }
    Ok([
        json_array_i32(values, key, 0)?,
        json_array_i32(values, key, 1)?,
        json_array_i32(values, key, 2)?,
    ])
}

fn json_array_i32(
    values: &[serde_json::Value],
    key: &'static str,
    index: usize,
) -> Result<i32, ResourceError> {
    let n = values[index]
        .as_i64()
        .ok_or(ResourceError::InvalidField(key))?;
    i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))
}

const fn default_morale_chart() -> [[u8; 5]; 5] {
    [*b"NNNNN", *b"NNNNN", *b"NNHNN", *b"LNLHN", *b"LLLNN"]
}

#[cfg(test)]
mod tests {
    use super::Resources;
    use crate::tables::{
        parse_troop_abilities, TROOP_ABIL_ABSORB, TROOP_ABIL_FLY, TROOP_ABIL_LEECH,
        TROOP_ABIL_UNDEAD,
    };

    #[test]
    fn parses_troop_catalog_subset() {
        let resources = Resources::from_json_str(
            r#"{
              "time": {"day_steps": 33, "week_days": 6, "days_per_difficulty": {"easy": 901, "normal": 601, "hard": 401, "impossible": 201}},
              "world": {"starting_zone": "fallback", "default_name": "Champion", "max_army_slots": 5, "default_options": [4,1,1,1,1,0,5]},
              "contract": {"cycle_length": 3, "initial_last_contract": 2},
              "combat": {"morale_chart": [["N","N","N","N","N"],["N","N","N","N","N"],["N","N","H","N","N"],["L","N","L","H","N"],["L","L","L","N","N"]]},
              "zones": [
                {"id":"home","is_home":true,"hero_spawn":{"x":1,"y":2},"home_spawn":{"x":3,"y":4}},
                {"id":"fallback","hero_spawn":{"x":8,"y":9}}
              ],
              "castles": [
                {"index":0,"id":"castle0","name":"Castle Zero","zone":"home","x":4,"y":5,"difficulty_tier":1},
                {"index":1,"id":"castle1","zone":"fallback","x":8,"y":9,"gate":{"x":10,"y":11},"special":{"excluded_from_contract":true}}
              ],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"vampires","skill_level":5,"hit_points":30,"move_rate":1,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":1500,"spoils_factor":150,"abilities":"FLY|LEECH|UNDEAD","morale_group":"E"}
              ],
              "classes": [
                {"index":0,"id":"knight","name":"Knight","portrait":"knight.png","starting_gold":1000,"starting_troops":[{"id":"peasants","count":25},{"id":"militia","count":10}],"ranks":[{"id":"general","name":"General","leadership":100,"max_spells":2,"spell_power":1,"commission":1000,"knows_magic":false}]}
              ],
              "villains": [
                {"index":0,"id":"villain0","name":"Villain Zero","portrait":"v0.png","zone":"home","reward":1000,"puzzle_cell":7,"army":[{"troop":"peasants","count":40}]},
                {"index":1,"id":"villain1","zone":"fallback","reward":2000}
              ]
            }"#,
        )
        .expect("fixture should parse");

        assert_eq!(resources.troops_count(), 2);
        assert_eq!(resources.zone_count(), 2);
        assert_eq!(resources.castle_count(), 2);
        assert_eq!(resources.classes_count(), 1);
        assert_eq!(resources.villains_count(), 2);
        assert_eq!(resources.time.day_steps, 33);
        assert_eq!(resources.world.default_name.as_str(), "Champion");
        assert_eq!(resources.contract.cycle_length, 3);
        assert_eq!(resources.contract.initial_last_contract, 2);
        assert_eq!(resources.zone_by_id("home").unwrap().home_spawn_x, 3);
        let castle0 = resources.castle_by_id("castle0").unwrap();
        assert_eq!(castle0.gate_x, 4);
        assert_eq!(castle0.gate_y, 6);
        assert_eq!(castle0.difficulty_tier, 1);
        assert!(
            resources
                .castle_by_id("castle1")
                .unwrap()
                .special
                .excluded_from_contract
        );
        assert_eq!(resources.class_by_index(0).unwrap().starting_gold, 1000);
        let peasants = resources.troop_by_id("peasants").unwrap();
        assert_eq!(peasants.hit_points, 1);
        assert_eq!(peasants.id.as_str(), "peasants");

        let vampires = resources.troop_by_index(1).unwrap();
        assert_eq!(vampires.id.as_str(), "vampires");
        assert_eq!(
            vampires.abilities,
            TROOP_ABIL_FLY | TROOP_ABIL_LEECH | TROOP_ABIL_UNDEAD
        );
        let villain = resources.villain_by_index(0).unwrap();
        assert_eq!(villain.id.as_str(), "villain0");
        assert_eq!(villain.reward, 1000);
        assert_eq!(villain.puzzle_cell, 7);
        assert_eq!(villain.army_troops[0].as_str(), "peasants");
        assert_eq!(villain.army_counts[0], 40);
    }

    #[test]
    fn ability_parser_matches_openbounty_tokens() {
        assert_eq!(
            parse_troop_abilities("FLY|ABSORB|LEECH|UNDEAD"),
            TROOP_ABIL_FLY | TROOP_ABIL_ABSORB | TROOP_ABIL_LEECH | TROOP_ABIL_UNDEAD
        );
    }
}
