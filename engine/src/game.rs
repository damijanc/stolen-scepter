use crate::resources::{Resources, RES_MAX_ZONES, RES_SPAWN_POOL_N, RES_SPAWN_TIERS};
use crate::rng::{seed_from_index, Rng};
use crate::tables::{class_stats_at_rank, FixedStr, CAT_ID_LEN, CAT_VILLAINS_MAX};
use crate::tile::Interact;

pub const GAME_NAME_LEN: usize = 16;
pub const GAME_ID_LEN: usize = 24;
pub const GAME_ARMY_SLOTS: usize = 5;
pub const GAME_CASTLES: usize = 26;
pub const GAME_OPTIONS: usize = 7;
pub const GAME_TOWNS: usize = 26;
pub const GAME_MAX_MUTATIONS: usize = 1024;
pub const GAME_MAX_DWELLINGS: usize = 64;
pub const GAME_MAX_PLACEMENTS: usize = 128;
pub const GAME_MAX_HOSTILE_PER_ZONE: usize = 35;
pub const GAME_MAX_FOES: usize = 160;
pub const CONTRACT_CYCLE_MAX: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SaltKind {
    None = 0,
    Artifact = 1,
    Navmap = 2,
    Orb = 3,
    Telecave = 4,
    Dwelling = 5,
    Friendly = 6,
}

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
pub enum AudienceOutcome {
    MoreNeeded = 0,
    Promoted = 1,
    FinalRank = 2,
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
pub struct TownRecord {
    pub id: FixedStr<GAME_ID_LEN>,
    pub visited: bool,
    pub spell_for_sale: FixedStr<GAME_ID_LEN>,
}

impl Default for TownRecord {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            visited: false,
            spell_for_sale: FixedStr::empty(),
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
    pub in_town: FixedStr<GAME_ID_LEN>,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            last_x: 0,
            last_y: 0,
            in_town: FixedStr::empty(),
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
pub struct TileMutation {
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
}

impl Default for TileMutation {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SaltedPlacement {
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub kind: Interact,
    pub id: FixedStr<CAT_ID_LEN>,
}

impl Default for SaltedPlacement {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            kind: Interact::None,
            id: FixedStr::empty(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DwellingState {
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub troop_id: FixedStr<CAT_ID_LEN>,
    pub count: i32,
    pub max_population: i32,
}

impl Default for DwellingState {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            troop_id: FixedStr::empty(),
            count: 0,
            max_population: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoeState {
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub placement_id: FixedStr<CAT_ID_LEN>,
    pub garrison: [Unit; GAME_ARMY_SLOTS],
    pub alive: bool,
    pub friendly: bool,
}

impl Default for FoeState {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            origin_x: 0,
            origin_y: 0,
            placement_id: FixedStr::empty(),
            garrison: [Unit::default(); GAME_ARMY_SLOTS],
            alive: false,
            friendly: false,
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
    pub towns: [TownRecord; GAME_TOWNS],
    pub castles: [CastleRecord; GAME_CASTLES],
    pub consumed: [TileMutation; GAME_MAX_MUTATIONS],
    pub consumed_count: usize,
    pub placements: [SaltedPlacement; GAME_MAX_PLACEMENTS],
    pub placement_count: usize,
    pub dwellings: [DwellingState; GAME_MAX_DWELLINGS],
    pub dwelling_count: usize,
    pub foes: [FoeState; GAME_MAX_FOES],
    pub foe_count: usize,
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
            towns: [TownRecord::default(); GAME_TOWNS],
            castles: [CastleRecord::default(); GAME_CASTLES],
            consumed: [TileMutation::default(); GAME_MAX_MUTATIONS],
            consumed_count: 0,
            placements: [SaltedPlacement::default(); GAME_MAX_PLACEMENTS],
            placement_count: 0,
            dwellings: [DwellingState::default(); GAME_MAX_DWELLINGS],
            dwelling_count: 0,
            foes: [FoeState::default(); GAME_MAX_FOES],
            foe_count: 0,
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
        if let Some(resources) = resources {
            self.salt_spells(resources);
            self.consume_magic_alcoves(resources);
            self.salt_continents(resources);
        }
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

    pub fn add_consumed(&mut self, zone: &str, x: i32, y: i32) {
        if self.is_consumed(zone, x, y) || self.consumed_count >= GAME_MAX_MUTATIONS {
            return;
        }
        let mutation = &mut self.consumed[self.consumed_count];
        mutation.zone.copy_from_str(zone);
        mutation.x = x;
        mutation.y = y;
        self.consumed_count += 1;
    }

    pub fn is_consumed(&self, zone: &str, x: i32, y: i32) -> bool {
        self.consumed[..self.consumed_count]
            .iter()
            .any(|mutation| mutation.zone.as_str() == zone && mutation.x == x && mutation.y == y)
    }

    pub fn consume_magic_alcoves(&mut self, resources: &Resources) {
        if !self.stats.knows_magic {
            return;
        }
        for index in 0..resources.zone_count() {
            let Some(zone) = resources.zone_by_index(index) else {
                continue;
            };
            if zone.magic_alcove_x < 0 || zone.magic_alcove_y < 0 {
                continue;
            }
            self.add_consumed(zone.id.as_str(), zone.magic_alcove_x, zone.magic_alcove_y);
        }
    }

    pub fn add_placement(&mut self, zone: &str, x: i32, y: i32, kind: Interact, id: &str) {
        if self.placement_count >= GAME_MAX_PLACEMENTS {
            return;
        }
        let placement = &mut self.placements[self.placement_count];
        placement.zone.copy_from_str(zone);
        placement.x = x;
        placement.y = y;
        placement.kind = kind;
        placement.id.copy_from_str(id);
        self.placement_count += 1;
    }

    pub fn salt_continents(&mut self, resources: &Resources) {
        for index in 0..resources.zone_count() {
            self.salt_continent(resources, index);
        }
    }

    pub fn salt_continent(&mut self, resources: &Resources, continent: usize) {
        let Some(zone) = resources.zone_by_index(continent) else {
            return;
        };
        let army_cap = zone.army_count.min(GAME_MAX_HOSTILE_PER_ZONE);
        for index in 0..army_cap {
            let army = zone.armies[index];
            let fallback_id = format!("static_foe_{index}");
            let id = if army.id.is_empty() {
                fallback_id.as_str()
            } else {
                army.id.as_str()
            };
            self.add_foe(
                resources,
                continent,
                zone.id.as_str(),
                army.x,
                army.y,
                id,
                false,
            );
        }

        let barrel_len = zone.chest_count;
        let min_len = zone.salt.artifacts
            + zone.salt.navmaps
            + zone.salt.orbs
            + zone.salt.telecaves
            + zone.salt.dwellings
            + zone.salt.friendly_foes;
        if min_len == 0 || barrel_len < min_len as usize {
            return;
        }

        let mut barrel = vec![SaltKind::None; barrel_len];
        self.tag_salt_kind(&mut barrel, zone.salt.artifacts, SaltKind::Artifact);
        self.tag_salt_kind(&mut barrel, zone.salt.navmaps, SaltKind::Navmap);
        self.tag_salt_kind(&mut barrel, zone.salt.orbs, SaltKind::Orb);
        self.tag_salt_kind(&mut barrel, zone.salt.telecaves, SaltKind::Telecave);
        self.tag_salt_kind(&mut barrel, zone.salt.dwellings, SaltKind::Dwelling);
        self.tag_salt_kind(&mut barrel, zone.salt.friendly_foes, SaltKind::Friendly);

        let mut artifact_counter = 0;
        let mut navmap_counter = 0;
        let mut orb_counter = 0;
        let mut telecave_counter = 0;
        let mut dwelling_counter = 0;
        let mut foe_counter = 0;
        for (index, kind) in barrel.iter().enumerate() {
            let slot = zone.chests[index];
            match kind {
                SaltKind::Artifact => {
                    let Some(artifact_index) =
                        resources.artifact_index_for_tile(zone.id.as_str(), artifact_counter)
                    else {
                        artifact_counter += 1;
                        continue;
                    };
                    if let Some(artifact) = resources.artifact_by_index(artifact_index as i32) {
                        self.add_placement(
                            zone.id.as_str(),
                            slot.x,
                            slot.y,
                            Interact::Artifact,
                            artifact.id.as_str(),
                        );
                    }
                    artifact_counter += 1;
                }
                SaltKind::Navmap => {
                    let id = format!("navmap_{navmap_counter}");
                    self.add_placement(
                        zone.id.as_str(),
                        slot.x,
                        slot.y,
                        Interact::Navmap,
                        id.as_str(),
                    );
                    navmap_counter += 1;
                }
                SaltKind::Orb => {
                    let id = format!("orb_{orb_counter}");
                    self.add_placement(
                        zone.id.as_str(),
                        slot.x,
                        slot.y,
                        Interact::Orb,
                        id.as_str(),
                    );
                    orb_counter += 1;
                }
                SaltKind::Telecave => {
                    let id = format!("telecave_{telecave_counter}");
                    self.add_placement(
                        zone.id.as_str(),
                        slot.x,
                        slot.y,
                        Interact::Telecave,
                        id.as_str(),
                    );
                    telecave_counter += 1;
                }
                SaltKind::Dwelling => {
                    let Some(troop_id) =
                        self.salt_pick_dwelling_troop(resources, continent, dwelling_counter)
                    else {
                        dwelling_counter += 1;
                        continue;
                    };
                    let Some(troop) = resources.troop_by_id(troop_id.as_str()) else {
                        dwelling_counter += 1;
                        continue;
                    };
                    let kind = Interact::from_dwelling_kind(troop.dwelling.as_str());
                    let id = salted_dwelling_id(troop_id.as_str(), dwelling_counter);
                    self.add_placement(zone.id.as_str(), slot.x, slot.y, kind, id.as_str());
                    self.enforce_dwelling_pinned(
                        resources,
                        zone.id.as_str(),
                        slot.x,
                        slot.y,
                        troop_id.as_str(),
                    );
                    dwelling_counter += 1;
                }
                SaltKind::Friendly => {
                    let id = format!("salt_foe_friendly_{foe_counter}");
                    self.add_foe(
                        resources,
                        continent,
                        zone.id.as_str(),
                        slot.x,
                        slot.y,
                        id.as_str(),
                        true,
                    );
                    foe_counter += 1;
                }
                SaltKind::None => {}
            }
        }
    }

    fn salt_pick_dwelling_troop(
        &mut self,
        resources: &Resources,
        continent: usize,
        slot_index: i32,
    ) -> Option<String> {
        let zone = resources.zone_by_index(continent)?;
        if slot_index >= 0 && (slot_index as usize) < zone.salt.preferred_troop_count {
            let troop_id = zone.salt.preferred_troops[slot_index as usize].as_str();
            return (!troop_id.is_empty()).then(|| troop_id.to_owned());
        }

        let lo = zone.salt.dwelling_range_min;
        let mut hi = zone.salt.dwelling_range_max;
        if lo < 0 || hi < 0 || lo > hi {
            return None;
        }
        let total = resources.troops_count() as i32;
        if lo >= total {
            return None;
        }
        if hi >= total {
            hi = total - 1;
        }

        for _ in 0..=((hi - lo) * 20 + 20) {
            let index = self.rng_next(lo, hi);
            let Some(troop) = resources.troop_by_index(index) else {
                continue;
            };
            if troop.id.is_empty() || troop.dwelling.as_str() == "castle" {
                continue;
            }
            return Some(troop.id.as_str().to_owned());
        }
        None
    }

    pub fn enforce_dwelling_pinned(
        &mut self,
        resources: &Resources,
        zone: &str,
        x: i32,
        y: i32,
        troop_id: &str,
    ) -> Option<&mut DwellingState> {
        if zone.is_empty() || troop_id.is_empty() {
            return None;
        }
        if let Some(index) = self.dwellings[..self.dwelling_count]
            .iter()
            .position(|dwelling| {
                dwelling.zone.as_str() == zone && dwelling.x == x && dwelling.y == y
            })
        {
            return self.dwellings.get_mut(index);
        }
        if self.dwelling_count >= GAME_MAX_DWELLINGS {
            return None;
        }
        let troop = resources.troop_by_id(troop_id)?;
        let dwelling = &mut self.dwellings[self.dwelling_count];
        dwelling.zone.copy_from_str(zone);
        dwelling.x = x;
        dwelling.y = y;
        dwelling.troop_id.copy_from_str(troop.id.as_str());
        dwelling.max_population = troop.max_population;
        dwelling.count = troop.max_population;
        self.dwelling_count += 1;
        Some(dwelling)
    }

    pub fn add_foe(
        &mut self,
        resources: &Resources,
        continent: usize,
        zone: &str,
        x: i32,
        y: i32,
        placement_id: &str,
        friendly: bool,
    ) {
        if self.foe_count >= GAME_MAX_FOES {
            return;
        }
        let garrison = self.roll_hostile_garrison(resources, continent);
        let foe = &mut self.foes[self.foe_count];
        foe.zone.copy_from_str(zone);
        foe.x = x;
        foe.y = y;
        foe.origin_x = x;
        foe.origin_y = y;
        foe.placement_id.copy_from_str(placement_id);
        foe.garrison = garrison;
        foe.alive = true;
        foe.friendly = friendly;
        self.foe_count += 1;
    }

    fn roll_hostile_garrison(
        &mut self,
        resources: &Resources,
        continent: usize,
    ) -> [Unit; GAME_ARMY_SLOTS] {
        let mut out = [Unit::default(); GAME_ARMY_SLOTS];
        let tier = continent & 3;
        let stacks = (1 + self.rng_next(0, 2)).min(GAME_ARMY_SLOTS as i32);
        for slot in 0..stacks as usize {
            let kind = self.rng_next(0, 3) as usize;
            let chance = self.rng_next(1, 100);
            let mut pool_slot = 0;
            while pool_slot < RES_SPAWN_POOL_N - 1
                && chance > resources.spawn.chance_curve[tier][pool_slot]
            {
                pool_slot += 1;
            }
            let troop_id = resources.spawn.troop_pool[kind][pool_slot].as_str();
            if troop_id.is_empty() {
                continue;
            }
            let Some(troop) = resources.troop_by_id(troop_id) else {
                continue;
            };
            let mut base = troop.tier_counts[tier];
            if base < 2 {
                base = 2;
            }
            let jitter = self.rng_next(0, base / 2);
            out[slot].id.copy_from_str(troop_id);
            out[slot].count = base + jitter;
        }
        out
    }

    fn tag_salt_kind(&mut self, barrel: &mut [SaltKind], count: i32, kind: SaltKind) {
        let mut placed = 0;
        let mut guard = 0;
        while placed < count && guard < barrel.len() as i32 * 20 {
            let index = self.rng_next(0, barrel.len() as i32 - 1) as usize;
            if barrel[index] == SaltKind::None {
                barrel[index] = kind;
                placed += 1;
            }
            guard += 1;
        }
    }

    pub fn salt_spells(&mut self, resources: &Resources) {
        let spells_count = resources.spells_count();
        let towns_count = resources.town_count().min(GAME_TOWNS);
        if spells_count == 0 || towns_count == 0 {
            return;
        }

        for i in 0..towns_count {
            let Some(resource_town) = resources.town_by_index(i) else {
                continue;
            };
            self.towns[i].id.copy_from_str(resource_town.id.as_str());
            self.towns[i].visited = false;
            self.towns[i].spell_for_sale = FixedStr::empty();
        }

        let mut spell_claimed = [false; crate::tables::CAT_SPELLS_MAX];
        for i in 0..towns_count {
            let Some(resource_town) = resources.town_by_index(i) else {
                continue;
            };
            let pin = resource_town.pinned_spell.as_str();
            if pin.is_empty() {
                continue;
            }
            let Some(spell) = resources.spell_by_id(pin) else {
                continue;
            };
            self.towns[i]
                .spell_for_sale
                .copy_from_str(spell.id.as_str());
            if spell.index >= 0 && (spell.index as usize) < spell_claimed.len() {
                spell_claimed[spell.index as usize] = true;
            }
        }

        let mut spell_index = 0;
        while spell_index < spells_count as i32 {
            if spell_index >= 0
                && (spell_index as usize) < spell_claimed.len()
                && spell_claimed[spell_index as usize]
            {
                spell_index += 1;
                continue;
            }

            let town_index = self.rng_next(0, towns_count as i32 - 1) as usize;
            if self.towns[town_index].spell_for_sale.is_empty() {
                if let Some(spell) = resources.spell_by_index(spell_index) {
                    self.towns[town_index]
                        .spell_for_sale
                        .copy_from_str(spell.id.as_str());
                }
                spell_index += 1;
            }
        }

        for i in 0..towns_count {
            if !self.towns[i].spell_for_sale.is_empty() {
                continue;
            }
            let spell_index = self.rng_next(0, spells_count as i32 - 1);
            if let Some(spell) = resources.spell_by_index(spell_index) {
                self.towns[i]
                    .spell_for_sale
                    .copy_from_str(spell.id.as_str());
            }
        }
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

    pub fn villain_contract_obtainable(&self, resources: &Resources, villain_id: &str) -> bool {
        if villain_id.is_empty() {
            return false;
        }
        if !self.contract.active_id.is_empty() && self.contract.active_id.as_str() == villain_id {
            return true;
        }

        let mut cycle_len = resources.contract.cycle_length;
        if cycle_len > CONTRACT_CYCLE_MAX as i32 {
            cycle_len = CONTRACT_CYCLE_MAX as i32;
        }
        if cycle_len < 0 {
            cycle_len = 0;
        }
        self.contract
            .cycle
            .iter()
            .take(cycle_len as usize)
            .any(|slot| slot.as_str() == villain_id)
    }

    pub fn take_next_contract<'a>(&'a mut self, resources: &Resources) -> Option<&'a str> {
        if self.position.in_town.is_empty() {
            return None;
        }

        let mut cycle_len = resources.contract.cycle_length;
        if cycle_len < 1 {
            cycle_len = 1;
        }
        if cycle_len > CONTRACT_CYCLE_MAX as i32 {
            cycle_len = CONTRACT_CYCLE_MAX as i32;
        }

        self.contract.last_contract += 1;
        if self.contract.last_contract > cycle_len - 1 {
            self.contract.last_contract = 0;
        }
        let slot = self.contract.last_contract as usize;
        let villain_id = self.contract.cycle[slot].as_str();
        if villain_id.is_empty() {
            return None;
        }

        self.contract.active_id.copy_from_str(villain_id);
        Some(self.contract.active_id.as_str())
    }

    pub fn fulfill_contract(&mut self, resources: &Resources, villain_id: &str) -> bool {
        if villain_id.is_empty() {
            return false;
        }
        let Some(villain) = resources.villain_by_id(villain_id) else {
            return false;
        };

        self.stats.gold += villain.reward;
        if villain.index >= 0 && (villain.index as usize) < CAT_VILLAINS_MAX {
            self.contract.villains_caught[villain.index as usize] = true;
        }

        let mut cycle_len = resources.contract.cycle_length;
        if cycle_len > CONTRACT_CYCLE_MAX as i32 {
            cycle_len = CONTRACT_CYCLE_MAX as i32;
        }
        if cycle_len < 1 {
            cycle_len = 1;
        }

        let mut slot = None;
        for i in 0..cycle_len as usize {
            if self.contract.cycle[i].as_str() == villain_id {
                slot = Some(i);
                break;
            }
        }

        if !self.contract.active_id.is_empty() && self.contract.active_id.as_str() == villain_id {
            self.contract.active_id = FixedStr::empty();
        }

        let Some(slot) = slot else {
            return false;
        };
        self.contract.cycle[slot] = FixedStr::empty();

        for index in self.contract.max_contract..resources.villains_count() as i32 {
            let Some(candidate) = resources.villain_by_index(index) else {
                continue;
            };
            if candidate.index >= 0
                && (candidate.index as usize) < CAT_VILLAINS_MAX
                && self.contract.villains_caught[candidate.index as usize]
            {
                continue;
            }
            self.contract.cycle[slot].copy_from_str(candidate.id.as_str());
            break;
        }
        self.contract.max_contract += 1;
        true
    }

    pub fn villains_caught(&self) -> i32 {
        self.contract
            .villains_caught
            .iter()
            .filter(|caught| **caught)
            .count() as i32
    }

    pub fn castles_owned(&self) -> i32 {
        self.castles
            .iter()
            .filter(|castle| !castle.id.is_empty() && castle.owner_kind == CastleOwnerKind::Player)
            .count() as i32
    }

    pub fn maybe_rank_up(&mut self, resources: &Resources) -> bool {
        let Some(class) = resources.class_by_id(self.character.cls.id.as_str()) else {
            return false;
        };
        let caught = self.villains_caught();
        let mut rank = self.character.cls.rank_index;
        let mut changed = false;

        while rank < 3
            && (rank + 1) >= 0
            && (rank + 1) < class.rank_count as i32
            && caught >= class.ranks[(rank + 1) as usize].villains_needed
        {
            rank += 1;
            changed = true;
        }

        if !changed {
            return false;
        }

        let old_rank = self.character.cls.rank_index;
        self.character.cls.rank_index = rank;
        self.character
            .cls
            .rank_id
            .copy_from_str(class.ranks[rank as usize].id.as_str());
        self.character
            .cls
            .rank_title
            .copy_from_str(class.ranks[rank as usize].name.as_str());

        let old_stats = class_stats_at_rank(class, old_rank);
        let new_stats = class_stats_at_rank(class, rank);
        self.stats.leadership_base += new_stats.leadership - old_stats.leadership;
        self.stats.commission_weekly += new_stats.commission - old_stats.commission;
        self.stats.max_spells += new_stats.max_spells - old_stats.max_spells;
        self.stats.spell_power += new_stats.spell_power - old_stats.spell_power;
        self.stats.leadership_current += new_stats.leadership - old_stats.leadership;
        true
    }

    pub fn audience_with_king(&mut self, resources: &Resources) -> (AudienceOutcome, i32) {
        let Some(class) = resources.class_by_id(self.character.cls.id.as_str()) else {
            return (AudienceOutcome::FinalRank, 0);
        };
        let rank = self.character.cls.rank_index;
        if rank + 1 >= class.rank_count as i32 {
            return (AudienceOutcome::FinalRank, 0);
        }

        let needed = class.ranks[(rank + 1) as usize].villains_needed - self.villains_caught();
        if needed > 0 {
            return (AudienceOutcome::MoreNeeded, needed);
        }

        self.maybe_rank_up(resources);
        (AudienceOutcome::Promoted, 0)
    }

    pub fn army_total_leadership(&self, resources: &Resources) -> i32 {
        let mut total = 0;
        for stack in &self.army {
            if stack.id.is_empty() {
                continue;
            }
            let Some(troop) = resources.troop_by_id(stack.id.as_str()) else {
                continue;
            };
            total += troop.hit_points * stack.count;
        }
        total
    }

    pub fn army_stack_count(&self) -> i32 {
        self.army
            .iter()
            .filter(|stack| !stack.id.is_empty())
            .count() as i32
    }

    pub fn max_recruitable(&self, resources: &Resources, troop_id: &str) -> i32 {
        let Some(troop) = resources.troop_by_id(troop_id) else {
            return 0;
        };
        if troop.hit_points <= 0 {
            return 0;
        }

        let mut same_troop_consumed = 0;
        for stack in &self.army {
            if stack.id.as_str() == troop_id {
                same_troop_consumed += stack.count * troop.hit_points;
            }
        }

        let free_leadership = self.stats.leadership_current - same_troop_consumed;
        if free_leadership < 0 {
            return 0;
        }
        free_leadership / troop.hit_points
    }

    pub fn add_troop(&mut self, troop_id: &str, count: i32) -> i32 {
        if troop_id.is_empty() || count <= 0 {
            return 1;
        }

        let mut slot = None;
        for i in 0..GAME_ARMY_SLOTS {
            if self.army[i].id.as_str() == troop_id && self.army[i].count > 0 {
                slot = Some(i);
                break;
            }
        }
        if slot.is_none() {
            for i in 0..GAME_ARMY_SLOTS {
                if self.army[i].id.is_empty() || self.army[i].count == 0 {
                    slot = Some(i);
                    break;
                }
            }
        }

        let Some(slot) = slot else {
            return 1;
        };
        self.army[slot].id.copy_from_str(troop_id);
        self.army[slot].count += count;
        0
    }

    pub fn compact_army(&mut self) {
        let mut dst = 0;
        for src in 0..GAME_ARMY_SLOTS {
            if self.army[src].id.is_empty() || self.army[src].count == 0 {
                continue;
            }
            if dst != src {
                self.army[dst] = self.army[src];
            }
            dst += 1;
        }
        for i in dst..GAME_ARMY_SLOTS {
            self.army[i] = ArmyStack::default();
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

fn salted_dwelling_id(troop_id: &str, dwelling_counter: i32) -> String {
    let len = troop_id.len().min(20);
    format!("sd_{}_{}", &troop_id[..len], dwelling_counter)
}

#[cfg(test)]
mod tests {
    use super::{AudienceOutcome, CastleOwnerKind, Game, GAME_ARMY_SLOTS};
    use crate::resources::Resources;
    use crate::tile::Interact;

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
        assert_eq!(game.towns[0].id.as_str(), "town0");
        assert_eq!(game.towns[0].spell_for_sale.as_str(), "bridge");
        assert!(!game.towns[0].visited);
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
    fn salt_spells_initializes_towns_pins_and_fills_empty_slots() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "towns": [
                {"index":0,"id":"town0","zone":"home","x":1,"y":1,"pinned_spell":"bridge"},
                {"index":1,"id":"town1","zone":"home","x":2,"y":2},
                {"index":2,"id":"town2","zone":"home","x":3,"y":3},
                {"index":3,"id":"town3","zone":"home","x":4,"y":4}
              ],
              "troops": [],
              "spells": [
                {"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100},
                {"index":1,"id":"fireball","name":"Fireball","kind":"combat","cost":50},
                {"index":2,"id":"clone","name":"Clone","kind":"combat","cost":75}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        assert_eq!(game.towns[0].id.as_str(), "town0");
        assert_eq!(game.towns[0].spell_for_sale.as_str(), "bridge");
        assert!(!game.towns[0].visited);
        for i in 0..resources.town_count() {
            assert!(!game.towns[i].id.is_empty());
            assert!(!game.towns[i].spell_for_sale.is_empty());
        }
        for spell_id in ["bridge", "fireball", "clone"] {
            assert!(game
                .towns
                .iter()
                .any(|town| town.spell_for_sale.as_str() == spell_id));
        }
    }

    #[test]
    fn init_consumes_magic_alcoves_only_when_class_knows_magic() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [
                {"id":"home","is_home":true,"magic_alcove":{"x":5,"y":6}},
                {"id":"away","magic_alcove":{"x":7,"y":8}},
                {"id":"plain"}
              ],
              "troops": [],
              "classes": [
                {"index":0,"id":"sorceress","ranks":[{"id":"archmage","name":"Archmage","knows_magic":true}]},
                {"index":1,"id":"knight","ranks":[{"id":"general","name":"General","knows_magic":false}]}
              ]
            }"#,
        )
        .expect("fixture parses");

        let mut magic_game = Game::default();
        magic_game.init_seeded_with_resources(&resources, Some("mage"), 0, 0, 42);
        assert_eq!(magic_game.consumed_count, 2);
        assert!(magic_game.is_consumed("home", 5, 6));
        assert!(magic_game.is_consumed("away", 7, 8));

        magic_game.add_consumed("home", 5, 6);
        assert_eq!(magic_game.consumed_count, 2);

        let mut mundane_game = Game::default();
        mundane_game.init_seeded_with_resources(&resources, Some("knight"), 1, 0, 42);
        assert_eq!(mundane_game.consumed_count, 0);
    }

    #[test]
    fn init_salts_simple_zone_placements_from_chest_barrel() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [
                {
                  "id":"home",
                  "is_home":true,
                  "chests":[
                    {"x":1,"y":1,"id":"c0"},
                    {"x":2,"y":2,"id":"c1"},
                    {"x":3,"y":3,"id":"c2"},
                    {"x":4,"y":4,"id":"c3"},
                    {"x":5,"y":5,"id":"c4"},
                    {"x":6,"y":6,"id":"c5"}
                  ],
                  "wandering_armies":[{"x":9,"y":10,"id":"static0"}],
                  "salt":{"artifacts":1,"navmaps":1,"orbs":1,"telecaves":1,"dwellings":1,"friendly_foes":1,"preferred_troops":["sprites"]}
                }
              ],
              "troops": [
                {"index":0,"id":"sprites","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"forest","max_population":50,"growth_per_week":10}
              ],
              "artifacts": [
                {"index":0,"id":"sword","zone":"home","local_idx":0}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();

        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        assert_eq!(game.placement_count, 5);
        assert_eq!(game.dwelling_count, 1);
        assert_eq!(game.dwellings[0].troop_id.as_str(), "sprites");
        assert_eq!(game.dwellings[0].count, 50);
        assert_eq!(game.dwellings[0].max_population, 50);
        assert_eq!(game.foe_count, 2);
        assert!(game.foes.iter().take(game.foe_count).any(|foe| {
            !foe.friendly
                && foe.placement_id.as_str() == "static0"
                && (foe.x, foe.y) == (9, 10)
                && (foe.origin_x, foe.origin_y) == (9, 10)
                && foe.alive
        }));
        assert!(game.foes.iter().take(game.foe_count).any(|foe| {
            foe.friendly && foe.placement_id.as_str() == "salt_foe_friendly_0" && foe.alive
        }));
        assert!(game
            .placements
            .iter()
            .take(game.placement_count)
            .any(|placement| {
                placement.kind == Interact::Artifact && placement.id.as_str() == "sword"
            }));
        assert!(game
            .placements
            .iter()
            .take(game.placement_count)
            .any(|placement| {
                placement.kind == Interact::DwellingForest
                    && placement.id.as_str() == "sd_sprites_0"
            }));
        assert!(game
            .placements
            .iter()
            .take(game.placement_count)
            .any(|placement| placement.kind == Interact::Navmap
                && placement.id.as_str() == "navmap_0"));
        assert!(game
            .placements
            .iter()
            .take(game.placement_count)
            .any(|placement| placement.kind == Interact::Orb && placement.id.as_str() == "orb_0"));
        assert!(game
            .placements
            .iter()
            .take(game.placement_count)
            .any(|placement| {
                placement.kind == Interact::Telecave && placement.id.as_str() == "telecave_0"
            }));
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
    fn villain_contract_obtainable_checks_active_and_cycle() {
        let resources = Resources::from_json_str(contract_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        assert!(game.villain_contract_obtainable(&resources, "villain0"));
        assert!(game.villain_contract_obtainable(&resources, "villain1"));
        assert!(!game.villain_contract_obtainable(&resources, "villain2"));
        assert!(!game.villain_contract_obtainable(&resources, ""));

        game.contract.active_id.copy_from_str("villain2");
        assert!(game.villain_contract_obtainable(&resources, "villain2"));
    }

    #[test]
    fn take_next_contract_requires_town_and_rotates_cycle() {
        let resources = Resources::from_json_str(contract_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        assert_eq!(game.take_next_contract(&resources), None);
        game.position.in_town.copy_from_str("town0");

        assert_eq!(game.take_next_contract(&resources), Some("villain0"));
        assert_eq!(game.contract.last_contract, 0);
        assert_eq!(game.contract.active_id.as_str(), "villain0");
        assert_eq!(game.take_next_contract(&resources), Some("villain1"));
        assert_eq!(game.contract.last_contract, 1);
        assert_eq!(game.contract.active_id.as_str(), "villain1");
    }

    #[test]
    fn fulfill_contract_rewards_clears_active_and_refills_cycle() {
        let resources = Resources::from_json_str(contract_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);
        game.position.in_town.copy_from_str("town0");
        assert_eq!(game.take_next_contract(&resources), Some("villain0"));
        let gold_before = game.stats.gold;

        assert!(game.fulfill_contract(&resources, "villain0"));

        assert_eq!(game.stats.gold, gold_before + 1_000);
        assert!(game.contract.villains_caught[0]);
        assert!(game.contract.active_id.is_empty());
        assert_eq!(game.contract.cycle[0].as_str(), "villain2");
        assert_eq!(game.contract.max_contract, 3);
    }

    #[test]
    fn fulfill_contract_preserves_openbounty_reward_before_missing_slot_check() {
        let resources = Resources::from_json_str(contract_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);
        let gold_before = game.stats.gold;

        assert!(!game.fulfill_contract(&resources, "villain2"));

        assert_eq!(game.stats.gold, gold_before + 3_000);
        assert!(game.contract.villains_caught[2]);
        assert_eq!(game.contract.max_contract, 2);
    }

    #[test]
    fn count_helpers_match_openbounty_state_scans() {
        let mut game = Game::default();
        game.contract.villains_caught[0] = true;
        game.contract.villains_caught[7] = true;
        game.contract.villains_caught[31] = true;
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].owner_kind = CastleOwnerKind::Player;
        game.castles[1].id.copy_from_str("castle1");
        game.castles[1].owner_kind = CastleOwnerKind::Monsters;
        game.castles[2].owner_kind = CastleOwnerKind::Player;

        assert_eq!(game.villains_caught(), 3);
        assert_eq!(game.castles_owned(), 1);
    }

    #[test]
    fn maybe_rank_up_applies_rank_delta_without_wiping_bonuses() {
        let resources = Resources::from_json_str(rank_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);
        game.stats.leadership_base += 25;
        game.stats.leadership_current += 25;
        game.stats.spell_power += 1;
        game.contract.villains_caught[0] = true;
        game.contract.villains_caught[1] = true;

        assert!(game.maybe_rank_up(&resources));

        assert_eq!(game.character.cls.rank_index, 1);
        assert_eq!(game.character.cls.rank_id.as_str(), "captain");
        assert_eq!(game.character.cls.rank_title.as_str(), "Captain");
        assert_eq!(game.stats.leadership_base, 175);
        assert_eq!(game.stats.leadership_current, 175);
        assert_eq!(game.stats.commission_weekly, 30);
        assert_eq!(game.stats.max_spells, 3);
        assert_eq!(game.stats.spell_power, 5);
    }

    #[test]
    fn audience_with_king_reports_needed_promotes_and_final_rank() {
        let resources = Resources::from_json_str(rank_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 42);

        assert_eq!(
            game.audience_with_king(&resources),
            (AudienceOutcome::MoreNeeded, 2)
        );

        game.contract.villains_caught[0] = true;
        game.contract.villains_caught[1] = true;
        assert_eq!(
            game.audience_with_king(&resources),
            (AudienceOutcome::Promoted, 0)
        );
        assert_eq!(
            game.audience_with_king(&resources),
            (AudienceOutcome::FinalRank, 0)
        );
    }

    #[test]
    fn army_leadership_queries_match_openbounty_rules() {
        let resources = Resources::from_json_str(army_fixture()).expect("fixture parses");
        let mut game = Game::default();
        game.stats.leadership_current = 100;
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 30;
        game.army[2].id.copy_from_str("archers");
        game.army[2].count = 0;

        assert_eq!(game.army_stack_count(), 3);
        assert_eq!(game.army_total_leadership(&resources), 70);
        assert_eq!(game.max_recruitable(&resources, "archers"), 10);
        assert_eq!(game.max_recruitable(&resources, "militia"), 20);
        assert_eq!(game.max_recruitable(&resources, "unknown"), 0);
    }

    #[test]
    fn add_troop_stacks_or_uses_first_empty_like_openbounty() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 5;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 0;

        assert_eq!(game.add_troop("peasants", 3), 0);
        assert_eq!(game.army[0].count, 8);

        assert_eq!(game.add_troop("archers", 2), 0);
        assert_eq!(game.army[1].id.as_str(), "archers");
        assert_eq!(game.army[1].count, 2);

        assert_eq!(game.add_troop("", 2), 1);
        assert_eq!(game.add_troop("wolves", 0), 1);
    }

    #[test]
    fn add_troop_fails_when_no_slot_available() {
        let mut game = Game::default();
        for i in 0..GAME_ARMY_SLOTS {
            game.army[i].id.copy_from_str(&format!("troop{i}"));
            game.army[i].count = 1;
        }

        assert_eq!(game.add_troop("new_troop", 1), 1);
    }

    #[test]
    fn compact_army_slides_non_empty_stacks_down() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("empty_count");
        game.army[0].count = 0;
        game.army[2].id.copy_from_str("peasants");
        game.army[2].count = 5;
        game.army[4].id.copy_from_str("militia");
        game.army[4].count = 3;

        game.compact_army();

        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 5);
        assert_eq!(game.army[1].id.as_str(), "militia");
        assert_eq!(game.army[1].count, 3);
        assert!(game.army[2..]
            .iter()
            .all(|stack| stack.id.is_empty() && stack.count == 0));
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
          "towns": [
            {"index":0,"id":"town0","zone":"home","x":1,"y":2,"pinned_spell":"bridge"}
          ],
          "castles": [
            {"index":0,"id":"castle0","name":"Castle Zero","zone":"home","x":4,"y":5},
            {"index":1,"id":"castle1","name":"Castle One","zone":"fallback","x":8,"y":9}
          ],
          "troops": [
            {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
            {"index":1,"id":"militia","skill_level":2,"hit_points":2,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":50,"spoils_factor":5,"abilities":"","morale_group":"A"}
          ],
          "spells": [
            {"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100}
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

    fn contract_fixture() -> &'static str {
        r#"{
          "world": {"starting_zone": "home"},
          "contract": {"cycle_length": 2, "initial_last_contract": 1},
          "zones": [{"id":"home","is_home":true}],
          "troops": [],
          "villains": [
            {"index":0,"id":"villain0","zone":"home","reward":1000},
            {"index":1,"id":"villain1","zone":"home","reward":2000},
            {"index":2,"id":"villain2","zone":"home","reward":3000}
          ]
        }"#
    }

    fn rank_fixture() -> &'static str {
        r#"{
          "world": {"starting_zone": "home"},
          "troops": [],
          "classes": [
            {
              "index": 0,
              "id": "knight",
              "starting_gold": 1000,
              "ranks": [
                {"id":"general","name":"General","villains_needed":0,"leadership":100,"max_spells":1,"spell_power":1,"commission":10,"knows_magic":false},
                {"id":"captain","name":"Captain","villains_needed":2,"leadership":50,"max_spells":2,"spell_power":3,"commission":20,"knows_magic":false}
              ]
            }
          ]
        }"#
    }

    fn army_fixture() -> &'static str {
        r#"{
          "troops": [
            {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
            {"index":1,"id":"militia","skill_level":2,"hit_points":2,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":50,"spoils_factor":5,"abilities":"","morale_group":"A"},
            {"index":2,"id":"archers","skill_level":3,"hit_points":10,"move_rate":2,"melee":[1,2],"ranged":[1,3,12],"recruit_cost":250,"spoils_factor":20,"abilities":"","morale_group":"B"}
          ]
        }"#
    }
}
