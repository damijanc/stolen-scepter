use crate::tables::{
    parse_troop_abilities, ArtifactDef, ArtifactPower, ClassDef, FixedStr, RankDef, SpellDef,
    SpellKind, TroopDef, VillainDef, CAT_ARTIFACTS_MAX, CAT_CLASSES_MAX, CAT_PATH_LEN,
    CAT_SPELLS_MAX, CAT_TROOPS_MAX, CAT_VILLAINS_MAX, CLASS_MAX_RANKS, CLASS_MAX_STARTING_TROOPS,
    TROOP_TIER_COUNTS,
};
use crate::tile::Terrain;

pub const RES_MAX_CASTLES: usize = 32;
pub const RES_MAX_PREFERRED_TROOPS: usize = 16;
pub const RES_MAX_TOWNS: usize = 32;
pub const RES_MAX_ZONE_OBJECTS: usize = 256;
pub const RES_MAX_ZONES: usize = 8;
pub const RES_ID_LEN: usize = 32;
pub const RES_NAME_LEN: usize = 48;
pub const RES_SIGN_BODY_LEN: usize = 128;
pub const RES_SIGN_TITLE_LEN: usize = 64;
pub const RES_MAX_CASTLE_DECOR: usize = 32;
pub const RES_TILE_ART_LEN: usize = 24;
pub const RES_TILE_CODE_COUNT: usize = 256;
pub const RES_SPAWN_TIERS: usize = 4;
pub const RES_SPAWN_POOL_N: usize = 5;
pub const RES_CHEST_TIERS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResTileCode {
    pub present: bool,
    pub art: FixedStr<RES_TILE_ART_LEN>,
    pub terrain: Terrain,
    pub blocks_foot: bool,
    pub is_bridge: bool,
}

impl Default for ResTileCode {
    fn default() -> Self {
        Self {
            present: false,
            art: FixedStr::empty(),
            terrain: Terrain::Grass,
            blocks_foot: false,
            is_bridge: false,
        }
    }
}

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
pub struct ResChest {
    pub chance_gold: [i32; RES_CHEST_TIERS],
    pub chance_commission: [i32; RES_CHEST_TIERS],
    pub chance_spell_power: [i32; RES_CHEST_TIERS],
    pub chance_max_spells: [i32; RES_CHEST_TIERS],
    pub chance_new_spell: [i32; RES_CHEST_TIERS],
    pub gold_min: [i32; RES_CHEST_TIERS],
    pub gold_max: [i32; RES_CHEST_TIERS],
    pub commission_min: [i32; RES_CHEST_TIERS],
    pub commission_max: [i32; RES_CHEST_TIERS],
    pub max_spells_base: [i32; RES_CHEST_TIERS],
}

impl Default for ResChest {
    fn default() -> Self {
        Self {
            chance_gold: [0x3d, 0x42, 0x4c, 0x47],
            chance_commission: [0x51, 0x56, 0x56, 0x51],
            chance_spell_power: [0x53, 0x59, 0x59, 0x56],
            chance_max_spells: [0x56, 0x5c, 0x5d, 0x5b],
            chance_new_spell: [0x65, 0x65, 0x65, 0x65],
            gold_min: [0x00, 0x04, 0x09, 0x13],
            gold_max: [0x05, 0x10, 0x15, 0x1f],
            commission_min: [0x09, 0x31, 0x63, 0xc7],
            commission_max: [0x29, 0x33, 0x65, 0x12d],
            max_spells_base: [0x01, 0x01, 0x02, 0x02],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResEconomy {
    pub boat_cost_normal: i32,
    pub boat_cost_cheap: i32,
    pub chest: ResChest,
}

impl Default for ResEconomy {
    fn default() -> Self {
        Self {
            boat_cost_normal: 500,
            boat_cost_cheap: 100,
            chest: ResChest::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResSpawn {
    pub chance_curve: [[i32; RES_SPAWN_POOL_N - 1]; RES_SPAWN_TIERS],
    pub troop_pool: [[FixedStr<RES_ID_LEN>; RES_SPAWN_POOL_N]; RES_SPAWN_TIERS],
}

impl Default for ResSpawn {
    fn default() -> Self {
        Self {
            chance_curve: [[0; RES_SPAWN_POOL_N - 1]; RES_SPAWN_TIERS],
            troop_pool: [[FixedStr::empty(); RES_SPAWN_POOL_N]; RES_SPAWN_TIERS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResSign {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
    pub title: FixedStr<RES_SIGN_TITLE_LEN>,
    pub body: FixedStr<RES_SIGN_BODY_LEN>,
}

impl Default for ResSign {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
            title: FixedStr::empty(),
            body: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResCastleDecor {
    pub dx: i32,
    pub dy: i32,
    pub art: FixedStr<RES_ID_LEN>,
}

impl Default for ResCastleDecor {
    fn default() -> Self {
        Self {
            dx: 0,
            dy: 0,
            art: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneCastle {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
    pub decor_count: usize,
    pub decorations: [ResCastleDecor; RES_MAX_CASTLE_DECOR],
}

impl Default for ResZoneCastle {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
            decor_count: 0,
            decorations: [ResCastleDecor::default(); RES_MAX_CASTLE_DECOR],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneChest {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
}

impl Default for ResZoneChest {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneArtifact {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
}

impl Default for ResZoneArtifact {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneArmy {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
}

impl Default for ResZoneArmy {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneDwelling {
    pub x: i32,
    pub y: i32,
    pub id: FixedStr<RES_ID_LEN>,
    pub kind: FixedStr<RES_ID_LEN>,
}

impl Default for ResZoneDwelling {
    fn default() -> Self {
        Self {
            x: 0,
            y: 0,
            id: FixedStr::empty(),
            kind: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResZoneSalt {
    pub artifacts: i32,
    pub navmaps: i32,
    pub orbs: i32,
    pub telecaves: i32,
    pub dwellings: i32,
    pub friendly_foes: i32,
    pub preferred_troop_count: usize,
    pub preferred_troops: [FixedStr<RES_ID_LEN>; RES_MAX_PREFERRED_TROOPS],
    pub dwelling_range_min: i32,
    pub dwelling_range_max: i32,
}

impl Default for ResZoneSalt {
    fn default() -> Self {
        Self {
            artifacts: 0,
            navmaps: 0,
            orbs: 0,
            telecaves: 0,
            dwellings: 0,
            friendly_foes: 0,
            preferred_troop_count: 0,
            preferred_troops: [FixedStr::empty(); RES_MAX_PREFERRED_TROOPS],
            dwelling_range_min: -1,
            dwelling_range_max: -1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResZone {
    pub id: FixedStr<RES_ID_LEN>,
    pub map_path: FixedStr<CAT_PATH_LEN>,
    pub width: i32,
    pub height: i32,
    pub is_home: bool,
    pub hero_spawn_x: i32,
    pub hero_spawn_y: i32,
    pub home_spawn_x: i32,
    pub home_spawn_y: i32,
    pub magic_alcove_x: i32,
    pub magic_alcove_y: i32,
    pub sign_count: usize,
    pub signs: Box<[ResSign; RES_MAX_ZONE_OBJECTS]>,
    pub castle_count: usize,
    pub castles: Box<[ResZoneCastle; RES_MAX_ZONE_OBJECTS]>,
    pub chest_count: usize,
    pub chests: Box<[ResZoneChest; RES_MAX_ZONE_OBJECTS]>,
    pub artifact_count: usize,
    pub artifacts: Box<[ResZoneArtifact; RES_MAX_ZONE_OBJECTS]>,
    pub army_count: usize,
    pub armies: Box<[ResZoneArmy; RES_MAX_ZONE_OBJECTS]>,
    pub dwelling_count: usize,
    pub dwellings: Box<[ResZoneDwelling; RES_MAX_ZONE_OBJECTS]>,
    pub salt: ResZoneSalt,
}

impl Default for ResZone {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            map_path: FixedStr::empty(),
            width: 64,
            height: 64,
            is_home: false,
            hero_spawn_x: 0,
            hero_spawn_y: 0,
            home_spawn_x: -1,
            home_spawn_y: -1,
            magic_alcove_x: -1,
            magic_alcove_y: -1,
            sign_count: 0,
            signs: Box::new([ResSign::default(); RES_MAX_ZONE_OBJECTS]),
            castle_count: 0,
            castles: Box::new([ResZoneCastle::default(); RES_MAX_ZONE_OBJECTS]),
            chest_count: 0,
            chests: Box::new([ResZoneChest::default(); RES_MAX_ZONE_OBJECTS]),
            artifact_count: 0,
            artifacts: Box::new([ResZoneArtifact::default(); RES_MAX_ZONE_OBJECTS]),
            army_count: 0,
            armies: Box::new([ResZoneArmy::default(); RES_MAX_ZONE_OBJECTS]),
            dwelling_count: 0,
            dwellings: Box::new([ResZoneDwelling::default(); RES_MAX_ZONE_OBJECTS]),
            salt: ResZoneSalt::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResTown {
    pub index: i32,
    pub id: FixedStr<RES_ID_LEN>,
    pub name: FixedStr<RES_NAME_LEN>,
    pub zone: FixedStr<RES_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub boat_x: i32,
    pub boat_y: i32,
    pub gate_x: i32,
    pub gate_y: i32,
    pub intel_castle: FixedStr<RES_ID_LEN>,
    pub pinned_spell: FixedStr<RES_ID_LEN>,
}

impl Default for ResTown {
    fn default() -> Self {
        Self {
            index: -1,
            id: FixedStr::empty(),
            name: FixedStr::empty(),
            zone: FixedStr::empty(),
            x: -1,
            y: -1,
            boat_x: -1,
            boat_y: -1,
            gate_x: -1,
            gate_y: -1,
            intel_castle: FixedStr::empty(),
            pinned_spell: FixedStr::empty(),
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
    resource_root: FixedStr<CAT_PATH_LEN>,
    pub time: ResTime,
    pub world: ResWorld,
    pub contract: ResContract,
    pub economy: ResEconomy,
    pub spawn: ResSpawn,
    towns: [ResTown; RES_MAX_TOWNS],
    town_count: usize,
    zones: [ResZone; RES_MAX_ZONES],
    zone_count: usize,
    castles: [ResCastle; RES_MAX_CASTLES],
    castle_count: usize,
    classes: [ClassDef; CAT_CLASSES_MAX],
    classes_count: usize,
    spells: [SpellDef; CAT_SPELLS_MAX],
    spells_count: usize,
    artifacts: [ArtifactDef; CAT_ARTIFACTS_MAX],
    artifacts_count: usize,
    tile_codes: Box<[ResTileCode; RES_TILE_CODE_COUNT]>,
    troops: [TroopDef; CAT_TROOPS_MAX],
    troops_count: usize,
    villains: [VillainDef; CAT_VILLAINS_MAX],
    villains_count: usize,
    morale_chart: [[u8; 5]; 5],
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            resource_root: FixedStr::empty(),
            time: ResTime::default(),
            world: ResWorld::default(),
            contract: ResContract::default(),
            economy: ResEconomy::default(),
            spawn: ResSpawn::default(),
            towns: [ResTown::default(); RES_MAX_TOWNS],
            town_count: 0,
            zones: std::array::from_fn(|_| ResZone::default()),
            zone_count: 0,
            castles: [ResCastle::default(); RES_MAX_CASTLES],
            castle_count: 0,
            classes: [ClassDef::default(); CAT_CLASSES_MAX],
            classes_count: 0,
            spells: [SpellDef::default(); CAT_SPELLS_MAX],
            spells_count: 0,
            artifacts: [ArtifactDef::default(); CAT_ARTIFACTS_MAX],
            artifacts_count: 0,
            tile_codes: Box::new([ResTileCode::default(); RES_TILE_CODE_COUNT]),
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
        let path = path.as_ref();
        let input = std::fs::read_to_string(path).map_err(ResourceError::Io)?;
        let mut resources = Self::from_json_str(&input)?;
        if let Some(parent) = path.parent() {
            resources
                .resource_root
                .copy_from_str(parent.to_string_lossy().as_ref());
        }
        Ok(resources)
    }

    pub fn from_json_str(input: &str) -> Result<Self, ResourceError> {
        let root: serde_json::Value = serde_json::from_str(input).map_err(ResourceError::Json)?;
        let mut resources = Self::default();
        resources.parse_time(&root)?;
        resources.parse_world(&root)?;
        resources.parse_contract(&root)?;
        resources.parse_economy(&root)?;
        resources.parse_spawn(&root)?;
        resources.parse_combat(&root)?;
        resources.parse_towns(&root)?;
        resources.parse_zones(&root)?;
        resources.parse_tile_codes(&root)?;
        resources.parse_castles(&root)?;
        resources.parse_troops(&root)?;
        resources.parse_spells(&root)?;
        resources.parse_artifacts(&root)?;
        resources.parse_classes(&root)?;
        resources.parse_villains(&root)?;
        Ok(resources)
    }

    pub fn town_count(&self) -> usize {
        self.town_count
    }

    pub fn town_by_index(&self, index: usize) -> Option<&ResTown> {
        self.towns.get(index).filter(|_| index < self.town_count)
    }

    pub fn town_by_id(&self, id: &str) -> Option<&ResTown> {
        self.towns[..self.town_count]
            .iter()
            .find(|town| town.id.as_str() == id)
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

    pub fn zone_index_by_id(&self, id: &str) -> Option<usize> {
        self.zones[..self.zone_count]
            .iter()
            .position(|zone| zone.id.as_str() == id)
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

    pub fn spells_count(&self) -> usize {
        self.spells_count
    }

    pub fn artifacts_count(&self) -> usize {
        self.artifacts_count
    }

    pub fn tile_code(&self, code: u8) -> Option<&ResTileCode> {
        self.tile_codes
            .get(code as usize)
            .filter(|tile_code| tile_code.present)
    }

    pub fn resolve_map_path(&self, zone: &ResZone) -> Option<std::path::PathBuf> {
        let raw = zone.map_path.as_str();
        if raw.is_empty() {
            return None;
        }
        let path = std::path::Path::new(raw);
        if path.is_absolute() || self.resource_root.is_empty() {
            Some(path.to_path_buf())
        } else {
            Some(std::path::Path::new(self.resource_root.as_str()).join(path))
        }
    }

    pub fn artifact_by_index(&self, index: i32) -> Option<&ArtifactDef> {
        if index < 0 || index as usize >= self.artifacts_count {
            return None;
        }
        self.artifacts.get(index as usize)
    }

    pub fn artifact_by_id(&self, id: &str) -> Option<&ArtifactDef> {
        self.artifacts[..self.artifacts_count]
            .iter()
            .find(|artifact| artifact.id.as_str() == id)
    }

    pub fn artifact_index_for_tile(&self, zone: &str, local_idx: i32) -> Option<usize> {
        self.artifacts[..self.artifacts_count]
            .iter()
            .position(|artifact| artifact.zone.as_str() == zone && artifact.local_idx == local_idx)
    }

    pub fn spell_by_index(&self, index: i32) -> Option<&SpellDef> {
        if index < 0 || index as usize >= self.spells_count {
            return None;
        }
        self.spells.get(index as usize)
    }

    pub fn spell_by_id(&self, id: &str) -> Option<&SpellDef> {
        self.spells[..self.spells_count]
            .iter()
            .find(|spell| spell.id.as_str() == id)
    }

    pub fn class_by_index(&self, index: i32) -> Option<&ClassDef> {
        if index < 0 || index as usize >= self.classes_count {
            return None;
        }
        self.classes.get(index as usize)
    }

    pub fn class_by_id(&self, id: &str) -> Option<&ClassDef> {
        self.classes[..self.classes_count]
            .iter()
            .find(|class| class.id.as_str() == id)
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

    pub fn villain_by_slot(&self, slot: usize) -> Option<&VillainDef> {
        self.villains
            .get(slot)
            .filter(|_| slot < self.villains_count)
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

    fn parse_economy(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(economy) = root.get("economy") else {
            return Ok(());
        };
        self.economy.boat_cost_normal =
            optional_i32_field(economy, "boat_cost_normal", self.economy.boat_cost_normal)?;
        self.economy.boat_cost_cheap =
            optional_i32_field(economy, "boat_cost_cheap", self.economy.boat_cost_cheap)?;
        let Some(chest) = economy.get("chest") else {
            return Ok(());
        };
        if !chest.is_object() {
            return Err(ResourceError::InvalidField("economy.chest"));
        }
        parse_i32_array_into(chest, "chance_gold", &mut self.economy.chest.chance_gold)?;
        parse_i32_array_into(
            chest,
            "chance_commission",
            &mut self.economy.chest.chance_commission,
        )?;
        parse_i32_array_into(
            chest,
            "chance_spell_power",
            &mut self.economy.chest.chance_spell_power,
        )?;
        parse_i32_array_into(
            chest,
            "chance_max_spells",
            &mut self.economy.chest.chance_max_spells,
        )?;
        parse_i32_array_into(
            chest,
            "chance_new_spell",
            &mut self.economy.chest.chance_new_spell,
        )?;
        parse_i32_array_into(chest, "gold_min", &mut self.economy.chest.gold_min)?;
        parse_i32_array_into(chest, "gold_max", &mut self.economy.chest.gold_max)?;
        parse_i32_array_into(
            chest,
            "commission_min",
            &mut self.economy.chest.commission_min,
        )?;
        parse_i32_array_into(
            chest,
            "commission_max",
            &mut self.economy.chest.commission_max,
        )?;
        parse_i32_array_into(
            chest,
            "max_spells_base",
            &mut self.economy.chest.max_spells_base,
        )?;
        Ok(())
    }

    fn parse_spawn(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(spawn) = root.get("spawn") else {
            return Ok(());
        };
        let spawn = spawn
            .as_object()
            .ok_or(ResourceError::InvalidField("spawn"))?;

        if let Some(curves) = spawn.get("tier_chance_curve") {
            let rows = curves
                .as_array()
                .ok_or(ResourceError::InvalidField("spawn.tier_chance_curve"))?;
            for (tier, row) in rows.iter().take(RES_SPAWN_TIERS).enumerate() {
                let values = row
                    .as_array()
                    .ok_or(ResourceError::InvalidField("spawn.tier_chance_curve row"))?;
                for (slot, value) in values.iter().take(RES_SPAWN_POOL_N - 1).enumerate() {
                    let n = value
                        .as_i64()
                        .ok_or(ResourceError::InvalidField("spawn.tier_chance_curve value"))?;
                    self.spawn.chance_curve[tier][slot] = i32::try_from(n).map_err(|_| {
                        ResourceError::InvalidField("spawn.tier_chance_curve value")
                    })?;
                }
            }
        }

        if let Some(pool) = spawn.get("tier_troop_pool") {
            let rows = pool
                .as_array()
                .ok_or(ResourceError::InvalidField("spawn.tier_troop_pool"))?;
            for (tier, row) in rows.iter().take(RES_SPAWN_TIERS).enumerate() {
                let values = row
                    .as_array()
                    .ok_or(ResourceError::InvalidField("spawn.tier_troop_pool row"))?;
                for (slot, value) in values.iter().take(RES_SPAWN_POOL_N).enumerate() {
                    let troop = value
                        .as_str()
                        .ok_or(ResourceError::InvalidField("spawn.tier_troop_pool value"))?;
                    self.spawn.troop_pool[tier][slot].copy_from_str(troop);
                }
            }
        }

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

    fn parse_spells(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(spells) = root.get("spells") else {
            return Ok(());
        };
        let spells = spells
            .as_array()
            .ok_or(ResourceError::InvalidField("spells"))?;
        if spells.len() > CAT_SPELLS_MAX {
            return Err(ResourceError::TooManySpells {
                count: spells.len(),
                max: CAT_SPELLS_MAX,
            });
        }

        for (slot, value) in spells.iter().enumerate() {
            self.spells[slot] = parse_spell(value)?;
        }
        self.spells_count = spells.len();
        Ok(())
    }

    fn parse_artifacts(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(artifacts) = root.get("artifacts") else {
            return Ok(());
        };
        let artifacts = artifacts
            .as_array()
            .ok_or(ResourceError::InvalidField("artifacts"))?;
        for (slot, value) in artifacts.iter().take(CAT_ARTIFACTS_MAX).enumerate() {
            self.artifacts[slot] = parse_artifact(value)?;
            self.artifacts_count += 1;
        }
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

    fn parse_tile_codes(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(tile_codes) = root.get("tile_codes") else {
            return Ok(());
        };
        let tile_codes = tile_codes
            .as_object()
            .ok_or(ResourceError::InvalidField("tile_codes"))?;

        for (key, value) in tile_codes {
            let Some(code) = key.as_bytes().first().copied() else {
                continue;
            };
            let slot = &mut self.tile_codes[code as usize];
            slot.present = true;
            if let Some(art) = optional_string_field(value, "art")? {
                slot.art.copy_from_str(art);
            }
            let terrain = optional_string_field(value, "terrain")?.unwrap_or("grass");
            slot.terrain = Terrain::from_name(terrain);
            slot.blocks_foot = optional_bool_field(value, "blocks_foot", false)?;
            slot.is_bridge = optional_bool_field(value, "is_bridge", false)?;
        }

        Ok(())
    }

    fn parse_towns(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(towns) = root.get("towns") else {
            return Ok(());
        };
        let towns = towns
            .as_array()
            .ok_or(ResourceError::InvalidField("towns"))?;
        if towns.len() > RES_MAX_TOWNS {
            return Err(ResourceError::TooManyTowns {
                count: towns.len(),
                max: RES_MAX_TOWNS,
            });
        }

        for (slot, value) in towns.iter().enumerate() {
            self.towns[slot] = parse_town(value)?;
        }
        self.town_count = towns.len();
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
    TooManyTowns { count: usize, max: usize },
    TooManySpells { count: usize, max: usize },
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
            Self::TooManyTowns { count, max } => {
                write!(f, "too many towns: got {count}, max {max}")
            }
            Self::TooManySpells { count, max } => {
                write!(f, "too many spells: got {count}, max {max}")
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
    let tier_counts = optional_i32_array::<TROOP_TIER_COUNTS>(value, "tier_counts")?;

    let mut troop = TroopDef::new(
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
        tier_counts,
    );
    if let Some(dwelling) = optional_string_field(value, "dwelling")? {
        troop.dwelling.copy_from_str(dwelling);
    }
    troop.max_population = optional_i32_field(value, "max_population", 0)?;
    troop.growth_per_week = optional_i32_field(value, "growth_per_week", 0)?;
    Ok(troop)
}

fn parse_spell(value: &serde_json::Value) -> Result<SpellDef, ResourceError> {
    let mut spell = SpellDef {
        index: optional_i32_field(value, "index", -1)?,
        kind: match optional_string_field(value, "kind")? {
            Some("adventure") => SpellKind::Adventure,
            _ => SpellKind::Combat,
        },
        cost: optional_i32_field(value, "cost", 0)?,
        ..SpellDef::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        spell.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        spell.name.copy_from_str(name);
    }
    if let Some(description) = optional_string_field(value, "description")? {
        spell.description.copy_from_str(description);
    }
    Ok(spell)
}

fn parse_artifact(value: &serde_json::Value) -> Result<ArtifactDef, ResourceError> {
    let mut artifact = ArtifactDef {
        index: optional_i32_field(value, "index", -1)?,
        power: match optional_string_field(value, "power")? {
            Some("increased_damage") => ArtifactPower::IncreasedDamage,
            Some("quarter_protection") => ArtifactPower::QuarterProtection,
            Some("double_leadership") => ArtifactPower::DoubleLeadership,
            Some("increase_commission") => ArtifactPower::IncreaseCommission,
            Some("double_spell_power") => ArtifactPower::DoubleSpellPower,
            Some("double_max_spells") => ArtifactPower::DoubleMaxSpells,
            Some("cheaper_boats") => ArtifactPower::CheaperBoats,
            _ => ArtifactPower::Unknown,
        },
        puzzle_cell: optional_i32_field(value, "puzzle_cell", -1)?,
        local_idx: optional_i32_field(value, "local_idx", 0)?,
        ..ArtifactDef::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        artifact.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        artifact.name.copy_from_str(name);
    }
    if let Some(icon) = optional_string_field(value, "icon")? {
        artifact.icon.copy_from_str(icon);
    }
    if let Some(effect) = optional_string_field(value, "effect")? {
        artifact.effect.copy_from_str(effect);
    }
    if let Some(zone) = optional_string_field(value, "zone")? {
        artifact.zone.copy_from_str(zone);
    }
    Ok(artifact)
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
    if let Some(map) = optional_string_field(value, "map")? {
        let legacy = "assets/kings-bounty/";
        let map = map.strip_prefix(legacy).unwrap_or(map);
        zone.map_path.copy_from_str(map);
    }
    zone.width = optional_i32_field(value, "width", 64)?;
    zone.height = optional_i32_field(value, "height", 64)?;
    zone.is_home = optional_bool_field(value, "is_home", false)?;

    if let Some(hero_spawn) = value.get("hero_spawn") {
        zone.hero_spawn_x = optional_i32_field(hero_spawn, "x", 0)?;
        zone.hero_spawn_y = optional_i32_field(hero_spawn, "y", 0)?;
    }
    if let Some(home_spawn) = value.get("home_spawn") {
        zone.home_spawn_x = optional_i32_field(home_spawn, "x", -1)?;
        zone.home_spawn_y = optional_i32_field(home_spawn, "y", -1)?;
    }
    if let Some(magic_alcove) = value.get("magic_alcove") {
        zone.magic_alcove_x = optional_i32_field(magic_alcove, "x", -1)?;
        zone.magic_alcove_y = optional_i32_field(magic_alcove, "y", -1)?;
    }
    if let Some(signs) = value.get("signs") {
        let signs = signs
            .as_array()
            .ok_or(ResourceError::InvalidField("signs"))?;
        for (slot, sign) in signs.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.signs[slot] = parse_sign(sign)?;
            zone.sign_count += 1;
        }
    }
    if let Some(castles) = value.get("castles") {
        let castles = castles
            .as_array()
            .ok_or(ResourceError::InvalidField("castles"))?;
        for (slot, castle) in castles.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.castles[slot] = parse_zone_castle(castle)?;
            zone.castle_count += 1;
        }
    }
    if let Some(chests) = value.get("chests") {
        let chests = chests
            .as_array()
            .ok_or(ResourceError::InvalidField("chests"))?;
        for (slot, chest) in chests.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.chests[slot] = parse_zone_chest(chest)?;
            zone.chest_count += 1;
        }
    }
    if let Some(artifacts) = value.get("artifacts") {
        let artifacts = artifacts
            .as_array()
            .ok_or(ResourceError::InvalidField("artifacts"))?;
        for (slot, artifact) in artifacts.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.artifacts[slot] = parse_zone_artifact(artifact)?;
            zone.artifact_count += 1;
        }
    }
    if let Some(armies) = value.get("wandering_armies") {
        let armies = armies
            .as_array()
            .ok_or(ResourceError::InvalidField("wandering_armies"))?;
        for (slot, army) in armies.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.armies[slot] = parse_zone_army(army)?;
            zone.army_count += 1;
        }
    }
    if let Some(dwellings) = value.get("dwellings") {
        let dwellings = dwellings
            .as_array()
            .ok_or(ResourceError::InvalidField("dwellings"))?;
        for (slot, dwelling) in dwellings.iter().take(RES_MAX_ZONE_OBJECTS).enumerate() {
            zone.dwellings[slot] = parse_zone_dwelling(dwelling)?;
            zone.dwelling_count += 1;
        }
    }
    if let Some(salt) = value.get("salt") {
        zone.salt = parse_zone_salt(salt)?;
    }

    Ok(zone)
}

fn parse_sign(value: &serde_json::Value) -> Result<ResSign, ResourceError> {
    let mut sign = ResSign {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResSign::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        sign.id.copy_from_str(id);
    }
    if let Some(title) = optional_string_field(value, "title")? {
        sign.title.copy_from_str(title);
    }
    if let Some(body) = optional_string_field(value, "body")? {
        sign.body.copy_from_str(body);
    }
    Ok(sign)
}

fn parse_zone_castle(value: &serde_json::Value) -> Result<ResZoneCastle, ResourceError> {
    let mut castle = ResZoneCastle {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResZoneCastle::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        castle.id.copy_from_str(id);
    }
    if let Some(decorations) = value.get("decorations") {
        let decorations = decorations
            .as_array()
            .ok_or(ResourceError::InvalidField("decorations"))?;
        for (slot, decoration) in decorations.iter().take(RES_MAX_CASTLE_DECOR).enumerate() {
            castle.decorations[slot] = parse_castle_decor(decoration)?;
            castle.decor_count += 1;
        }
    }
    Ok(castle)
}

fn parse_castle_decor(value: &serde_json::Value) -> Result<ResCastleDecor, ResourceError> {
    let mut decor = ResCastleDecor {
        dx: optional_i32_field(value, "dx", 0)?,
        dy: optional_i32_field(value, "dy", 0)?,
        ..ResCastleDecor::default()
    };
    if let Some(art) = optional_string_field(value, "art")? {
        decor.art.copy_from_str(art);
    }
    Ok(decor)
}

fn parse_zone_chest(value: &serde_json::Value) -> Result<ResZoneChest, ResourceError> {
    let mut chest = ResZoneChest {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResZoneChest::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        chest.id.copy_from_str(id);
    }
    Ok(chest)
}

fn parse_zone_artifact(value: &serde_json::Value) -> Result<ResZoneArtifact, ResourceError> {
    let mut artifact = ResZoneArtifact {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResZoneArtifact::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        artifact.id.copy_from_str(id);
    }
    Ok(artifact)
}

fn parse_zone_army(value: &serde_json::Value) -> Result<ResZoneArmy, ResourceError> {
    let mut army = ResZoneArmy {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResZoneArmy::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        army.id.copy_from_str(id);
    }
    Ok(army)
}

fn parse_zone_dwelling(value: &serde_json::Value) -> Result<ResZoneDwelling, ResourceError> {
    let mut dwelling = ResZoneDwelling {
        x: optional_i32_field(value, "x", 0)?,
        y: optional_i32_field(value, "y", 0)?,
        ..ResZoneDwelling::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        dwelling.id.copy_from_str(id);
    }
    if let Some(kind) = optional_string_field(value, "kind")? {
        dwelling.kind.copy_from_str(kind);
    }
    Ok(dwelling)
}

fn parse_zone_salt(value: &serde_json::Value) -> Result<ResZoneSalt, ResourceError> {
    let mut salt = ResZoneSalt {
        artifacts: optional_i32_field(value, "artifacts", 0)?,
        navmaps: optional_i32_field(value, "navmaps", 0)?,
        orbs: optional_i32_field(value, "orbs", 0)?,
        telecaves: optional_i32_field(value, "telecaves", 0)?,
        dwellings: optional_i32_field(value, "dwellings", 0)?,
        friendly_foes: optional_i32_field(value, "friendly_foes", 0)?,
        ..ResZoneSalt::default()
    };

    if let Some(preferred) = value.get("preferred_troops") {
        let preferred = preferred
            .as_array()
            .ok_or(ResourceError::InvalidField("preferred_troops"))?;
        for (slot, troop) in preferred.iter().take(RES_MAX_PREFERRED_TROOPS).enumerate() {
            let troop = troop
                .as_str()
                .ok_or(ResourceError::InvalidField("preferred_troops"))?;
            salt.preferred_troops[slot].copy_from_str(troop);
            salt.preferred_troop_count += 1;
        }
    }

    if let Some(range) = value.get("dwelling_range") {
        let range = range
            .as_array()
            .ok_or(ResourceError::InvalidField("dwelling_range"))?;
        if range.len() != 2 {
            return Err(ResourceError::InvalidField("dwelling_range"));
        }
        salt.dwelling_range_min = json_array_i32(range, "dwelling_range", 0)?;
        salt.dwelling_range_max = json_array_i32(range, "dwelling_range", 1)?;
    }

    Ok(salt)
}

fn parse_town(value: &serde_json::Value) -> Result<ResTown, ResourceError> {
    let mut town = ResTown {
        index: optional_i32_field(value, "index", -1)?,
        x: optional_i32_field(value, "x", -1)?,
        y: optional_i32_field(value, "y", -1)?,
        ..ResTown::default()
    };
    if let Some(id) = optional_string_field(value, "id")? {
        town.id.copy_from_str(id);
    }
    if let Some(name) = optional_string_field(value, "name")? {
        town.name.copy_from_str(name);
    }
    if let Some(zone) = optional_string_field(value, "zone")? {
        town.zone.copy_from_str(zone);
    }
    if let Some(boat) = value.get("boat") {
        town.boat_x = optional_i32_field(boat, "x", -1)?;
        town.boat_y = optional_i32_field(boat, "y", -1)?;
    }
    if let Some(gate) = value.get("gate") {
        town.gate_x = optional_i32_field(gate, "x", -1)?;
        town.gate_y = optional_i32_field(gate, "y", -1)?;
    }
    if let Some(intel_castle) = optional_string_field(value, "intel_castle")? {
        town.intel_castle.copy_from_str(intel_castle);
    }
    if let Some(pinned_spell) = optional_string_field(value, "pinned_spell")? {
        town.pinned_spell.copy_from_str(pinned_spell);
    }
    Ok(town)
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

fn optional_i32_array<const N: usize>(
    value: &serde_json::Value,
    key: &'static str,
) -> Result<[i32; N], ResourceError> {
    let mut out = [0; N];
    let Some(values) = value.get(key) else {
        return Ok(out);
    };
    let values = values.as_array().ok_or(ResourceError::InvalidField(key))?;
    for (index, value) in values.iter().take(N).enumerate() {
        let n = value.as_i64().ok_or(ResourceError::InvalidField(key))?;
        out[index] = i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))?;
    }
    Ok(out)
}

fn parse_i32_array_into(
    value: &serde_json::Value,
    key: &'static str,
    out: &mut [i32],
) -> Result<(), ResourceError> {
    let Some(values) = value.get(key) else {
        return Ok(());
    };
    let values = values.as_array().ok_or(ResourceError::InvalidField(key))?;
    for (index, value) in values.iter().take(out.len()).enumerate() {
        let n = value.as_i64().ok_or(ResourceError::InvalidField(key))?;
        out[index] = i32::try_from(n).map_err(|_| ResourceError::InvalidField(key))?;
    }
    Ok(())
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
              "economy": {"chest": {"chance_gold": [1,2,3,4], "gold_min": [9]}},
              "spawn": {
                "tier_chance_curve": [[50,75,90,100],[45,70,90,100],[40,65,85,100],[35,60,80,100]],
                "tier_troop_pool": [
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["vampires","peasants","peasants","peasants","peasants"],
                  ["peasants","vampires","peasants","peasants","peasants"],
                  ["peasants","peasants","vampires","peasants","peasants"]
                ]
              },
              "combat": {"morale_chart": [["N","N","N","N","N"],["N","N","N","N","N"],["N","N","H","N","N"],["L","N","L","H","N"],["L","L","L","N","N"]]},
              "zones": [
                {
                  "id":"home",
                  "is_home":true,
                  "hero_spawn":{"x":1,"y":2},
                  "home_spawn":{"x":3,"y":4},
                  "magic_alcove":{"x":5,"y":6},
                  "chests":[{"x":10,"y":11,"id":"chest0"}],
                  "wandering_armies":[{"x":12,"y":13,"id":"foe0"}],
                  "dwellings":[{"x":14,"y":15,"id":"dw0","kind":"hills"}],
                  "salt":{
                    "navmaps":1,
                    "orbs":2,
                    "telecaves":3,
                    "dwellings":4,
                    "friendly_foes":5,
                    "preferred_troops":["peasants","vampires"],
                    "dwelling_range":[0,1]
                  }
                },
                {"id":"fallback","hero_spawn":{"x":8,"y":9}}
              ],
              "towns": [
                {"index":0,"id":"town0","name":"Town Zero","zone":"home","x":1,"y":2,"gate":{"x":1,"y":3},"boat":{"x":2,"y":2},"intel_castle":"castle0","pinned_spell":"bridge"},
                {"index":1,"id":"town1","zone":"fallback","x":4,"y":5}
              ],
              "castles": [
                {"index":0,"id":"castle0","name":"Castle Zero","zone":"home","x":4,"y":5,"difficulty_tier":1},
                {"index":1,"id":"castle1","zone":"fallback","x":8,"y":9,"gate":{"x":10,"y":11},"special":{"excluded_from_contract":true}}
              ],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[5,10,15,20],"dwelling":"plains","max_population":100,"growth_per_week":20},
                {"index":1,"id":"vampires","skill_level":5,"hit_points":30,"move_rate":1,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":1500,"spoils_factor":150,"abilities":"FLY|LEECH|UNDEAD","morale_group":"E","dwelling":"castle"}
              ],
              "classes": [
                {"index":0,"id":"knight","name":"Knight","portrait":"knight.png","starting_gold":1000,"starting_troops":[{"id":"peasants","count":25},{"id":"militia","count":10}],"ranks":[{"id":"general","name":"General","leadership":100,"max_spells":2,"spell_power":1,"commission":1000,"knows_magic":false}]}
              ],
              "spells": [
                {"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100,"description":"Build bridge"},
                {"index":1,"id":"fireball","name":"Fireball","kind":"combat","cost":50}
              ],
              "artifacts": [
                {"index":0,"id":"sword","name":"Sword of Prowess","icon":"sword.png","power":"increased_damage","effect":"More damage","puzzle_cell":3,"zone":"home","local_idx":0}
              ],
              "villains": [
                {"index":0,"id":"villain0","name":"Villain Zero","portrait":"v0.png","zone":"home","reward":1000,"puzzle_cell":7,"army":[{"troop":"peasants","count":40}]},
                {"index":1,"id":"villain1","zone":"fallback","reward":2000}
              ]
            }"#,
        )
        .expect("fixture should parse");

        assert_eq!(resources.troops_count(), 2);
        assert_eq!(resources.town_count(), 2);
        assert_eq!(resources.zone_count(), 2);
        assert_eq!(resources.castle_count(), 2);
        assert_eq!(resources.spells_count(), 2);
        assert_eq!(resources.artifacts_count(), 1);
        assert_eq!(resources.classes_count(), 1);
        assert_eq!(resources.villains_count(), 2);
        assert_eq!(resources.time.day_steps, 33);
        assert_eq!(resources.world.default_name.as_str(), "Champion");
        assert_eq!(resources.contract.cycle_length, 3);
        assert_eq!(resources.contract.initial_last_contract, 2);
        assert_eq!(resources.economy.chest.chance_gold, [1, 2, 3, 4]);
        assert_eq!(resources.economy.chest.gold_min, [9, 4, 9, 19]);
        assert_eq!(resources.spawn.chance_curve[0], [50, 75, 90, 100]);
        assert_eq!(resources.spawn.troop_pool[1][0].as_str(), "vampires");
        let town0 = resources.town_by_id("town0").unwrap();
        assert_eq!(town0.gate_x, 1);
        assert_eq!(town0.gate_y, 3);
        assert_eq!(town0.boat_x, 2);
        assert_eq!(town0.boat_y, 2);
        assert_eq!(town0.pinned_spell.as_str(), "bridge");
        assert_eq!(resources.zone_by_id("home").unwrap().home_spawn_x, 3);
        assert_eq!(resources.zone_by_id("home").unwrap().magic_alcove_x, 5);
        assert_eq!(resources.zone_by_id("home").unwrap().magic_alcove_y, 6);
        let home = resources.zone_by_id("home").unwrap();
        assert_eq!(home.chest_count, 1);
        assert_eq!((home.chests[0].x, home.chests[0].y), (10, 11));
        assert_eq!(home.chests[0].id.as_str(), "chest0");
        assert_eq!(home.army_count, 1);
        assert_eq!((home.armies[0].x, home.armies[0].y), (12, 13));
        assert_eq!(home.armies[0].id.as_str(), "foe0");
        assert_eq!(home.dwelling_count, 1);
        assert_eq!((home.dwellings[0].x, home.dwellings[0].y), (14, 15));
        assert_eq!(home.dwellings[0].id.as_str(), "dw0");
        assert_eq!(home.dwellings[0].kind.as_str(), "hills");
        assert_eq!(home.salt.navmaps, 1);
        assert_eq!(home.salt.orbs, 2);
        assert_eq!(home.salt.telecaves, 3);
        assert_eq!(home.salt.dwellings, 4);
        assert_eq!(home.salt.friendly_foes, 5);
        assert_eq!(home.salt.preferred_troop_count, 2);
        assert_eq!(home.salt.preferred_troops[0].as_str(), "peasants");
        assert_eq!(home.salt.preferred_troops[1].as_str(), "vampires");
        assert_eq!(home.salt.dwelling_range_min, 0);
        assert_eq!(home.salt.dwelling_range_max, 1);
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
        assert_eq!(peasants.tier_counts, [5, 10, 15, 20]);
        assert_eq!(peasants.dwelling.as_str(), "plains");
        assert_eq!(peasants.max_population, 100);
        assert_eq!(peasants.growth_per_week, 20);

        let vampires = resources.troop_by_index(1).unwrap();
        assert_eq!(vampires.id.as_str(), "vampires");
        assert_eq!(vampires.dwelling.as_str(), "castle");
        assert_eq!(
            vampires.abilities,
            TROOP_ABIL_FLY | TROOP_ABIL_LEECH | TROOP_ABIL_UNDEAD
        );
        let bridge = resources.spell_by_id("bridge").unwrap();
        assert_eq!(bridge.name.as_str(), "Bridge");
        assert_eq!(bridge.kind, crate::tables::SpellKind::Adventure);
        assert_eq!(bridge.cost, 100);
        assert_eq!(bridge.description.as_str(), "Build bridge");
        let artifact = resources.artifact_by_id("sword").unwrap();
        assert_eq!(artifact.name.as_str(), "Sword of Prowess");
        assert_eq!(artifact.icon.as_str(), "sword.png");
        assert_eq!(
            artifact.power,
            crate::tables::ArtifactPower::IncreasedDamage
        );
        assert_eq!(artifact.effect.as_str(), "More damage");
        assert_eq!(artifact.puzzle_cell, 3);
        assert_eq!(artifact.zone.as_str(), "home");
        assert_eq!(artifact.local_idx, 0);
        assert_eq!(resources.artifact_index_for_tile("home", 0), Some(0));
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
