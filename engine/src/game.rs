use crate::resources::{Resources, RES_MAX_ZONES, RES_SPAWN_TIERS};
use crate::rng::{seed_from_index, Rng};
use crate::tables::{class_stats_at_rank, FixedStr, CAT_ID_LEN, CAT_VILLAINS_MAX};

pub const GAME_NAME_LEN: usize = 16;
pub const GAME_ID_LEN: usize = 24;
pub const GAME_ARMY_SLOTS: usize = 5;
pub const GAME_CASTLES: usize = 26;
pub const GAME_OPTIONS: usize = 7;
pub const CONTRACT_CYCLE_MAX: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mount {
    Ride = 0,
    Sail = 1,
    Fly = 2,
}

impl Default for Mount {
    fn default() -> Self {
        Self::Ride
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TravelMode {
    Walk = 0,
    Boat = 1,
}

impl Default for TravelMode {
    fn default() -> Self {
        Self::Walk
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArmyStack {
    pub id: FixedStr<CAT_ID_LEN>,
    pub count: i32,
}

impl Default for ArmyStack {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Unit {
    pub id: FixedStr<GAME_ID_LEN>,
    pub count: i32,
}

impl Default for Unit {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            count: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastleOwnerKind {
    Player = 0,
    Monsters = 1,
    Villain = 2,
    Special = 3,
}

impl Default for CastleOwnerKind {
    fn default() -> Self {
        Self::Player
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastleRecord {
    pub id: FixedStr<GAME_ID_LEN>,
    pub visited: bool,
    pub known: bool,
    pub owner_kind: CastleOwnerKind,
    pub villain_id: FixedStr<GAME_ID_LEN>,
    pub garrison: [Unit; GAME_ARMY_SLOTS],
}

impl Default for CastleRecord {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            visited: false,
            known: false,
            owner_kind: CastleOwnerKind::default(),
            villain_id: FixedStr::empty(),
            garrison: [Unit::default(); GAME_ARMY_SLOTS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Contract {
    pub active_id: FixedStr<GAME_ID_LEN>,
    pub cycle: [FixedStr<GAME_ID_LEN>; CONTRACT_CYCLE_MAX],
    pub last_contract: i32,
    pub max_contract: i32,
    pub villains_caught: [bool; CAT_VILLAINS_MAX],
    pub villains_prefought: [bool; CAT_VILLAINS_MAX],
}

impl Default for Contract {
    fn default() -> Self {
        Self {
            active_id: FixedStr::empty(),
            cycle: [FixedStr::empty(); CONTRACT_CYCLE_MAX],
            last_contract: 0,
            max_contract: 0,
            villains_caught: [false; CAT_VILLAINS_MAX],
            villains_prefought: [false; CAT_VILLAINS_MAX],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClassState {
    pub id: FixedStr<GAME_ID_LEN>,
    pub rank_index: i32,
    pub rank_id: FixedStr<GAME_ID_LEN>,
    pub rank_title: FixedStr<CAT_ID_LEN>,
}

impl Default for ClassState {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            rank_index: 0,
            rank_id: FixedStr::empty(),
            rank_title: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Character {
    pub name: FixedStr<GAME_NAME_LEN>,
    pub cls: ClassState,
    pub difficulty: i32,
    pub mount: Mount,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            name: FixedStr::empty(),
            cls: ClassState::default(),
            difficulty: 0,
            mount: Mount::Ride,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stats {
    pub gold: i32,
    pub commission_weekly: i32,
    pub leadership_base: i32,
    pub leadership_current: i32,
    pub followers_killed: i32,
    pub score: i32,
    pub spell_power: i32,
    pub max_spells: i32,
    pub knows_magic: bool,
    pub siege_weapons: i32,
    pub time_stop: i32,
    pub steps_left_today: i32,
    pub days_left: i32,
    pub game_over: bool,
    pub won: bool,
    pub last_commission: i32,
    pub options: [i32; GAME_OPTIONS],
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            gold: 0,
            commission_weekly: 0,
            leadership_base: 0,
            leadership_current: 0,
            followers_killed: 0,
            score: 0,
            spell_power: 0,
            max_spells: 0,
            knows_magic: false,
            siege_weapons: 0,
            time_stop: 0,
            steps_left_today: 40,
            days_left: 900,
            game_over: false,
            won: false,
            last_commission: 0,
            options: [1; GAME_OPTIONS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Position {
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub last_x: i32,
    pub last_y: i32,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            last_x: 0,
            last_y: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoatState {
    pub has_boat: bool,
    pub x: i32,
    pub y: i32,
    pub zone: FixedStr<GAME_ID_LEN>,
}

impl Default for BoatState {
    fn default() -> Self {
        Self {
            has_boat: false,
            x: -1,
            y: -1,
            zone: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldProgress {
    pub zones_discovered: [bool; RES_MAX_ZONES],
}

impl Default for WorldProgress {
    fn default() -> Self {
        Self {
            zones_discovered: [false; RES_MAX_ZONES],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScepterState {
    pub key: i32,
    pub continent: i32,
}

impl Default for ScepterState {
    fn default() -> Self {
        Self {
            key: 0,
            continent: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Game {
    pub seed_index: i32,
    pub seed_from_catalog: bool,
    pub seed: u64,
    pub scepter: ScepterState,
    pub character: Character,
    pub stats: Stats,
    pub position: Position,
    pub travel_mode: TravelMode,
    pub boat: BoatState,
    pub army: [ArmyStack; GAME_ARMY_SLOTS],
    pub contract: Contract,
    pub castles: [CastleRecord; GAME_CASTLES],
    pub world: WorldProgress,
    world_rng: Rng,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            seed_index: 0,
            seed_from_catalog: false,
            seed: 0,
            scepter: ScepterState::default(),
            character: Character::default(),
            stats: Stats::default(),
            position: Position::default(),
            travel_mode: TravelMode::Walk,
            boat: BoatState::default(),
            army: [ArmyStack::default(); GAME_ARMY_SLOTS],
            contract: Contract::default(),
            castles: [CastleRecord::default(); GAME_CASTLES],
            world: WorldProgress::default(),
            world_rng: Rng::from_state(0),
        }
    }
}

impl Game {
    pub fn with_raw_seed(seed: u64) -> Self {
        Self {
            seed,
            ..Self::default()
        }
    }

    pub fn init_seeded(
        &mut self,
        name: Option<&str>,
        player_class: i32,
        difficulty: i32,
        seed_index: i32,
    ) {
        self.init_seeded_inner(None, name, player_class, difficulty, seed_index);
    }

    pub fn init_seeded_with_resources(
        &mut self,
        resources: &Resources,
        name: Option<&str>,
        player_class: i32,
        difficulty: i32,
        seed_index: i32,
    ) {
        self.init_seeded_inner(Some(resources), name, player_class, difficulty, seed_index);
    }

    fn init_seeded_inner(
        &mut self,
        resources: Option<&Resources>,
        name: Option<&str>,
        player_class: i32,
        difficulty: i32,
        seed_index: i32,
    ) {
        let seed_saved = self.seed;
        *self = Self::default();

        if seed_index >= 0 {
            self.seed_index = seed_index & 0xFF;
            self.seed_from_catalog = true;
            self.seed = seed_from_index(self.seed_index as u8);
        } else if seed_saved != 0 {
            self.seed = seed_saved;
            self.seed_from_catalog = false;
        } else {
            let generated_index = generated_seed_index(name, player_class);
            self.seed_index = generated_index;
            self.seed_from_catalog = true;
            self.seed = seed_from_index(generated_index as u8);
        }

        self.world_rng = Rng::seed(self.seed);
        self.scepter.key = self.world_rng.next(0, 255);
        self.scepter.continent = self.world_rng.next(0, 3);

        self.init_character(resources, name, player_class, difficulty);
        self.init_position(resources);
        self.character.mount = Mount::Ride;
        self.travel_mode = TravelMode::Walk;
        self.boat = BoatState::default();
        self.position.last_x = self.position.x;
        self.position.last_y = self.position.y;
        self.stats.time_stop = 0;
        self.init_contract(resources);
        self.init_starting_army(resources, player_class);
        self.init_options(resources);
        self.init_castles(resources);
    }

    pub fn rng_next(&mut self, min: i32, max: i32) -> i32 {
        self.world_rng.next(min, max)
    }

    pub fn rng_snapshot(&self) -> u64 {
        self.world_rng.state()
    }

    pub fn rng_restore(&mut self, state: u64) {
        self.world_rng = Rng::from_state(state);
    }

    pub fn salt_villains(&mut self, resources: &Resources) {
        let villain_count = resources.villains_count();
        let castle_count = resources.castle_count().min(GAME_CASTLES);
        if villain_count == 0 || castle_count == 0 {
            return;
        }

        for vi in 0..villain_count {
            let Some(villain) = resources.villain_by_slot(vi) else {
                continue;
            };
            if villain.zone.is_empty() {
                continue;
            }

            let mut guard = 0;
            while guard < castle_count * 20 {
                let castle_index = self.rng_next(0, castle_count as i32 - 1) as usize;
                let Some(resource_castle) = resources.castle_by_index(castle_index) else {
                    guard += 1;
                    continue;
                };
                if resource_castle.zone.as_str() != villain.zone.as_str() {
                    guard += 1;
                    continue;
                }
                if resource_castle.special.excluded_from_contract {
                    guard += 1;
                    continue;
                }

                let castle = &mut self.castles[castle_index];
                if castle.owner_kind != CastleOwnerKind::Monsters {
                    guard += 1;
                    continue;
                }

                castle.owner_kind = CastleOwnerKind::Villain;
                castle.villain_id.copy_from_str(villain.id.as_str());
                for slot in 0..GAME_ARMY_SLOTS.min(5) {
                    castle.garrison[slot]
                        .id
                        .copy_from_str(villain.army_troops[slot].as_str());
                    castle.garrison[slot].count = villain.army_counts[slot];
                }
                break;
            }
        }
    }

    pub fn roll_creature(&mut self, resources: &Resources, mut tier: i32) -> Unit {
        if tier < 0 || tier as usize >= RES_SPAWN_TIERS {
            tier = 0;
        }

        let kind = self.rng_next(0, 3) as usize;
        let chance = self.rng_next(1, 100);
        let mut slot = 0;
        while slot < 4 && chance > resources.spawn.chance_curve[tier as usize][slot] {
            slot += 1;
        }

        let troop_id = resources.spawn.troop_pool[kind][slot].as_str();
        if troop_id.is_empty() {
            return Unit::default();
        }

        let mut unit = Unit::default();
        unit.id.copy_from_str(troop_id);
        unit.count = resources
            .troop_by_id(troop_id)
            .map(|troop| troop.tier_counts[tier as usize])
            .unwrap_or(0);
        if unit.count <= 1 {
            unit.count = 2;
        }
        unit
    }

    pub fn repopulate_castle(&mut self, resources: &Resources, castle_id: usize) {
        if castle_id >= GAME_CASTLES || castle_id >= resources.castle_count() {
            return;
        }

        let tier = resources
            .castle_by_index(castle_id)
            .map(|castle| castle.difficulty_tier)
            .unwrap_or(0);
        for slot in 0..GAME_ARMY_SLOTS.min(5) {
            let unit = self.roll_creature(resources, tier);
            self.castles[castle_id].garrison[slot] = unit;
        }
    }

    pub fn populate_castles(&mut self, resources: &Resources) {
        self.salt_villains(resources);

        for i in 0..GAME_CASTLES {
            if self.castles[i].id.is_empty() {
                continue;
            }
            if i < resources.castle_count()
                && resources
                    .castle_by_index(i)
                    .map(|castle| castle.special.excluded_from_contract)
                    .unwrap_or(false)
            {
                self.castles[i].owner_kind = CastleOwnerKind::Special;
                continue;
            }
            if self.castles[i].owner_kind == CastleOwnerKind::Monsters {
                self.repopulate_castle(resources, i);
            }
        }
    }

    fn init_character(
        &mut self,
        resources: Option<&Resources>,
        name: Option<&str>,
        player_class: i32,
        difficulty: i32,
    ) {
        let chosen_name = match name {
            Some(value) if !value.is_empty() => value,
            _ => resources
                .map(|res| res.world.default_name.as_str())
                .filter(|value| !value.is_empty())
                .unwrap_or("Hero"),
        };
        self.character.name.copy_from_str(chosen_name);
        capitalize_first_ascii(&mut self.character.name);
        self.character.difficulty = difficulty;

        let Some(resources) = resources else {
            self.stats.days_left = 900;
            self.stats.steps_left_today = 40;
            return;
        };

        let class_index = if (0..4).contains(&player_class) {
            player_class
        } else {
            0
        };
        if let Some(class) = resources
            .class_by_index(class_index)
            .or_else(|| resources.class_by_index(0))
        {
            self.character.cls.id.copy_from_str(class.id.as_str());
            self.character.cls.rank_index = 0;
            if class.rank_count > 0 {
                self.character
                    .cls
                    .rank_id
                    .copy_from_str(class.ranks[0].id.as_str());
                self.character
                    .cls
                    .rank_title
                    .copy_from_str(class.ranks[0].name.as_str());
            }

            let stats = class_stats_at_rank(class, 0);
            self.stats.gold = class.starting_gold;
            self.stats.commission_weekly = stats.commission;
            self.stats.leadership_base = stats.leadership;
            self.stats.leadership_current = stats.leadership;
            self.stats.spell_power = stats.spell_power;
            self.stats.max_spells = stats.max_spells;
            self.stats.knows_magic = class.rank_count > 0 && class.ranks[0].knows_magic;
            self.stats.siege_weapons = 0;
        }

        let difficulty_index = if (0..4).contains(&difficulty) {
            difficulty as usize
        } else {
            0
        };
        self.stats.days_left = resources.time.days_per_difficulty[difficulty_index];
        self.stats.steps_left_today = resources.time.day_steps;
        self.stats.last_commission = 0;
    }

    fn init_position(&mut self, resources: Option<&Resources>) {
        let Some(resources) = resources else {
            return;
        };

        let mut home_zone_index = None;
        for zi in 0..resources.zone_count() {
            let Some(zone) = resources.zone_by_index(zi) else {
                continue;
            };
            if !zone.is_home {
                continue;
            }
            self.position.zone.copy_from_str(zone.id.as_str());
            self.position.x = zone.home_spawn_x;
            self.position.y = zone.home_spawn_y;
            home_zone_index = Some(zi);
            break;
        }

        if self.position.zone.is_empty() {
            self.position
                .zone
                .copy_from_str(resources.world.starting_zone.as_str());
            for zi in 0..resources.zone_count() {
                let Some(zone) = resources.zone_by_index(zi) else {
                    continue;
                };
                if zone.id.as_str() == resources.world.starting_zone.as_str() {
                    self.position.x = zone.hero_spawn_x;
                    self.position.y = zone.hero_spawn_y;
                    home_zone_index = Some(zi);
                    break;
                }
            }
        }

        if let Some(index) = home_zone_index {
            if index < self.world.zones_discovered.len() {
                self.world.zones_discovered[index] = true;
            }
        }
    }

    fn init_starting_army(&mut self, resources: Option<&Resources>, player_class: i32) {
        let Some(resources) = resources else {
            return;
        };
        let class_index = if (0..4).contains(&player_class) {
            player_class
        } else {
            0
        };
        let Some(class) = resources
            .class_by_index(class_index)
            .or_else(|| resources.class_by_index(0))
        else {
            return;
        };

        for i in 0..2 {
            let troop = class.starting_troops[i].as_str();
            let count = class.starting_counts[i];
            if troop.is_empty() || count <= 0 {
                continue;
            }
            self.army[i].id.copy_from_str(troop);
            self.army[i].count = count;
        }
    }

    fn init_options(&mut self, resources: Option<&Resources>) {
        if let Some(resources) = resources {
            self.stats.options = resources.world.default_options;
        }
    }

    fn init_contract(&mut self, resources: Option<&Resources>) {
        self.contract.active_id = FixedStr::empty();
        let mut cycle_len = resources
            .map(|resources| resources.contract.cycle_length)
            .unwrap_or(5);
        if cycle_len < 1 {
            cycle_len = 1;
        }
        if cycle_len > CONTRACT_CYCLE_MAX as i32 {
            cycle_len = CONTRACT_CYCLE_MAX as i32;
        }

        self.contract.last_contract = resources
            .map(|resources| resources.contract.initial_last_contract)
            .unwrap_or(cycle_len - 1);
        self.contract.max_contract = cycle_len;

        let Some(resources) = resources else {
            return;
        };
        for i in 0..cycle_len as usize {
            if let Some(villain) = resources.villain_by_index(i as i32) {
                self.contract.cycle[i].copy_from_str(villain.id.as_str());
            } else {
                self.contract.cycle[i] = FixedStr::empty();
            }
        }
    }

    fn init_castles(&mut self, resources: Option<&Resources>) {
        let Some(resources) = resources else {
            return;
        };
        let count = resources.castle_count().min(GAME_CASTLES);
        for i in 0..count {
            let Some(resource_castle) = resources.castle_by_index(i) else {
                continue;
            };
            self.castles[i]
                .id
                .copy_from_str(resource_castle.id.as_str());
            self.castles[i].visited = false;
            self.castles[i].known = false;
            self.castles[i].owner_kind = CastleOwnerKind::Monsters;
            self.castles[i].villain_id = FixedStr::empty();
            for slot in &mut self.castles[i].garrison {
                slot.id = FixedStr::empty();
                slot.count = 0;
            }
        }
    }
}

fn capitalize_first_ascii<const N: usize>(value: &mut FixedStr<N>) {
    let current = value.as_str().as_bytes().to_vec();
    if current.is_empty() {
        return;
    }
    let mut capitalized = current;
    if capitalized[0].is_ascii_lowercase() {
        capitalized[0] = capitalized[0].to_ascii_uppercase();
    }
    if let Ok(text) = std::str::from_utf8(&capitalized) {
        value.copy_from_str(text);
    }
}

fn generated_seed_index(name: Option<&str>, player_class: i32) -> i32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut h = seconds;
    h ^= (player_class as u64).wrapping_mul(2_654_435_761);
    for byte in name.unwrap_or("").bytes() {
        h = h.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (h & 0xFF) as i32
}

#[cfg(test)]
mod tests {
    use super::{CastleOwnerKind, Game};
    use crate::resources::Resources;

    #[test]
    fn init_seeded_matches_openbounty_first_world_rng_rolls() {
        let cases = [
            (0, 0xe220_a839_7b1d_cdaf, 249, 0, 0x413f_5b10_e08e_6d4c),
            (1, 0x910a_2dec_8902_5cc1, 172, 3, 0x80ff_c0ab_e8e3_2f46),
            (2, 0x9758_35de_1c97_56ce, 176, 1, 0xe80d_b8f1_06ef_b795),
            (42, 0xbdd7_3226_2feb_6e95, 107, 2, 0x9f2a_ee8a_789e_c472),
            (255, 0x338c_5071_4628_3fb4, 133, 1, 0x3af1_ff29_32cd_babb),
        ];

        for (seed_index, seed, key, continent, rng_state) in cases {
            let mut game = Game::default();
            game.init_seeded(Some("Hero"), 0, 0, seed_index);

            assert_eq!(game.seed_index, seed_index);
            assert!(game.seed_from_catalog);
            assert_eq!(game.seed, seed);
            assert_eq!(game.scepter.key, key);
            assert_eq!(game.scepter.continent, continent);
            assert_eq!(game.rng_snapshot(), rng_state);
        }
    }

    #[test]
    fn init_seeded_masks_catalog_seed_index_to_u8() {
        let mut game = Game::default();
        game.init_seeded(None, 0, 0, 257);

        assert_eq!(game.seed_index, 1);
        assert_eq!(game.seed, 0x910a_2dec_8902_5cc1);
    }

    #[test]
    fn raw_seed_path_preserves_caller_supplied_seed() {
        let mut game = Game {
            seed: 12_345,
            ..Game::default()
        };

        game.init_seeded(Some("Hero"), 0, 0, -1);

        assert_eq!(game.seed, 12_345);
        assert!(!game.seed_from_catalog);
        assert_eq!(game.scepter.key, 159);
        assert_eq!(game.scepter.continent, 1);
    }

    #[test]
    fn rng_snapshot_restore_round_trips_world_rng_state() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 42);

        let snap = game.rng_snapshot();
        let first = game.rng_next(0, 99);
        assert_ne!(game.rng_snapshot(), snap);

        game.rng_restore(snap);
        assert_eq!(game.rng_next(0, 99), first);
    }

    #[test]
    fn init_seeded_with_resources_populates_early_game_state() {
        let resources = Resources::from_json_str(init_fixture()).expect("fixture parses");
        let mut game = Game::default();

        game.init_seeded_with_resources(&resources, Some("arthur"), 0, 2, 42);

        assert_eq!(game.character.name.as_str(), "Arthur");
        assert_eq!(game.character.difficulty, 2);
        assert_eq!(game.character.cls.id.as_str(), "knight");
        assert_eq!(game.character.cls.rank_id.as_str(), "general");
        assert_eq!(game.character.cls.rank_title.as_str(), "General");
        assert_eq!(game.stats.gold, 1_000);
        assert_eq!(game.stats.commission_weekly, 100);
        assert_eq!(game.stats.leadership_base, 120);
        assert_eq!(game.stats.leadership_current, 120);
        assert_eq!(game.stats.spell_power, 1);
        assert_eq!(game.stats.max_spells, 2);
        assert!(!game.stats.knows_magic);
        assert_eq!(game.stats.days_left, 401);
        assert_eq!(game.stats.steps_left_today, 33);
        assert_eq!(game.stats.options, [4, 1, 1, 1, 1, 0, 5]);
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!((game.position.x, game.position.y), (3, 4));
        assert_eq!((game.position.last_x, game.position.last_y), (3, 4));
        assert!(game.world.zones_discovered[0]);
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 25);
        assert_eq!(game.army[1].id.as_str(), "militia");
        assert_eq!(game.army[1].count, 10);
        assert!(game.contract.active_id.is_empty());
        assert_eq!(game.contract.last_contract, 2);
        assert_eq!(game.contract.max_contract, 3);
        assert_eq!(game.contract.cycle[0].as_str(), "villain0");
        assert_eq!(game.contract.cycle[1].as_str(), "villain1");
        assert_eq!(game.contract.cycle[2].as_str(), "");
        assert_eq!(game.castles[0].id.as_str(), "castle0");
        assert!(!game.castles[0].visited);
        assert!(!game.castles[0].known);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Monsters);
        assert!(game.castles[0].villain_id.is_empty());
        assert!(game.castles[0]
            .garrison
            .iter()
            .all(|slot| slot.id.is_empty() && slot.count == 0));
        assert!(!game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (-1, -1));
    }

    #[test]
    fn salt_villains_places_declared_army_in_matching_castle() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "castles": [{"index":0,"id":"castle0","zone":"home","x":4,"y":5}],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"}
              ],
              "villains": [
                {"index":0,"id":"villain0","zone":"home","army":[{"troop":"peasants","count":40}]}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        game.salt_villains(&resources);

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "villain0");
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
        assert_eq!(game.castles[0].garrison[0].count, 40);
    }

    #[test]
    fn salt_villains_skips_contract_excluded_castles() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "castles": [{"index":0,"id":"castle0","zone":"home","x":4,"y":5,"special":{"excluded_from_contract":true}}],
              "troops": [],
              "villains": [
                {"index":0,"id":"villain0","zone":"home","army":[{"troop":"peasants","count":40}]}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        game.salt_villains(&resources);

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Monsters);
        assert!(game.castles[0].villain_id.is_empty());
        assert!(game.castles[0].garrison[0].id.is_empty());
        assert_eq!(game.castles[0].garrison[0].count, 0);
    }

    #[test]
    fn repopulate_castle_rolls_spawn_troops_for_difficulty_tier() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "spawn": {
                "tier_chance_curve": [[100,100,100,100],[100,100,100,100],[100,100,100,100],[100,100,100,100]],
                "tier_troop_pool": [
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"]
                ]
              },
              "castles": [{"index":0,"id":"castle0","zone":"home","x":4,"y":5,"difficulty_tier":2}],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[5,10,15,20]}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        game.repopulate_castle(&resources, 0);

        for slot in &game.castles[0].garrison {
            assert_eq!(slot.id.as_str(), "peasants");
            assert_eq!(slot.count, 15);
        }
    }

    #[test]
    fn populate_castles_marks_special_and_repopulates_monsters() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "spawn": {
                "tier_chance_curve": [[100,100,100,100],[100,100,100,100],[100,100,100,100],[100,100,100,100]],
                "tier_troop_pool": [
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"],
                  ["peasants","peasants","peasants","peasants","peasants"]
                ]
              },
              "castles": [
                {"index":0,"id":"castle0","zone":"home","x":4,"y":5,"special":{"excluded_from_contract":true}},
                {"index":1,"id":"castle1","zone":"home","x":6,"y":7,"difficulty_tier":1}
              ],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[5,10,15,20]}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        game.populate_castles(&resources);

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Special);
        assert!(game.castles[0]
            .garrison
            .iter()
            .all(|unit| unit.id.is_empty() && unit.count == 0));
        assert_eq!(game.castles[1].owner_kind, CastleOwnerKind::Monsters);
        for slot in &game.castles[1].garrison {
            assert_eq!(slot.id.as_str(), "peasants");
            assert_eq!(slot.count, 10);
        }
    }

    #[test]
    fn populate_castles_keeps_villain_garrison() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "spawn": {
                "tier_chance_curve": [[100,100,100,100],[100,100,100,100],[100,100,100,100],[100,100,100,100]],
                "tier_troop_pool": [
                  ["militia","militia","militia","militia","militia"],
                  ["militia","militia","militia","militia","militia"],
                  ["militia","militia","militia","militia","militia"],
                  ["militia","militia","militia","militia","militia"]
                ]
              },
              "castles": [{"index":0,"id":"castle0","zone":"home","x":4,"y":5}],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[5,10,15,20]},
                {"index":1,"id":"militia","skill_level":2,"hit_points":2,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":50,"spoils_factor":5,"abilities":"","morale_group":"A","tier_counts":[6,11,16,21]}
              ],
              "villains": [
                {"index":0,"id":"villain0","zone":"home","army":[{"troop":"peasants","count":40}]}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        game.populate_castles(&resources);

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "villain0");
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
        assert_eq!(game.castles[0].garrison[0].count, 40);
        assert!(game.castles[0].garrison[1].id.is_empty());
    }

    #[test]
    fn init_seeded_with_resources_uses_default_name_and_starting_zone_fallback() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "fallback", "default_name": "Champion"},
              "zones": [{"id":"fallback","hero_spawn":{"x":8,"y":9}}],
              "troops": [],
              "classes": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();

        game.init_seeded_with_resources(&resources, None, 0, 0, 1);

        assert_eq!(game.character.name.as_str(), "Champion");
        assert_eq!(game.position.zone.as_str(), "fallback");
        assert_eq!((game.position.x, game.position.y), (8, 9));
        assert!(game.world.zones_discovered[0]);
        assert_eq!(game.stats.days_left, 900);
        assert_eq!(game.stats.steps_left_today, 40);
    }

    fn init_fixture() -> &'static str {
        r#"{
          "time": {"day_steps": 33, "week_days": 6, "days_per_difficulty": {"easy": 901, "normal": 601, "hard": 401, "impossible": 201}},
          "world": {"starting_zone": "fallback", "default_name": "Champion", "max_army_slots": 5, "default_options": [4,1,1,1,1,0,5]},
          "contract": {"cycle_length": 3, "initial_last_contract": 2},
          "zones": [
            {"id":"home","is_home":true,"hero_spawn":{"x":1,"y":2},"home_spawn":{"x":3,"y":4}},
            {"id":"fallback","hero_spawn":{"x":8,"y":9}}
          ],
          "castles": [
            {"index":0,"id":"castle0","name":"Castle Zero","zone":"home","x":4,"y":5},
            {"index":1,"id":"castle1","name":"Castle One","zone":"fallback","x":8,"y":9}
          ],
          "troops": [
            {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
            {"index":1,"id":"militia","skill_level":2,"hit_points":2,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":50,"spoils_factor":5,"abilities":"","morale_group":"A"}
          ],
          "classes": [
            {"index":0,"id":"knight","name":"Knight","portrait":"knight.png","starting_gold":1000,"starting_troops":[{"id":"peasants","count":25},{"id":"militia","count":10}],"ranks":[{"id":"general","name":"General","leadership":120,"max_spells":2,"spell_power":1,"commission":100,"knows_magic":false}]}
          ],
          "villains": [
            {"index":0,"id":"villain0","name":"Villain Zero","zone":"home","reward":1000},
            {"index":1,"id":"villain1","name":"Villain One","zone":"fallback","reward":2000}
          ]
        }"#
    }
}
