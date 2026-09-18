use crate::combat::{
    combat_run_headless, CombatMode, CombatResult, CombatTarget, COMBAT_MAX_ROUNDS,
};
use crate::fog::Fog;
use crate::map::Map;
use crate::resources::{Resources, RES_MAX_ZONES, RES_SPAWN_POOL_N, RES_SPAWN_TIERS};
use crate::rng::{seed_from_index, Rng};
use crate::tables::{
    class_stats_at_rank, puzzle_grid_entity, ArtifactPower, Facing, FixedStr, TroopDef,
    CAT_ARTIFACTS_MAX, CAT_ID_LEN, CAT_SPELLS_MAX, CAT_VILLAINS_MAX, TROOP_ABIL_ABSORB,
    TROOP_ABIL_FLY,
};
use crate::tile::{Interact, Terrain};

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
/// Continents that retain their own fog snapshot.
pub const GAME_CONTINENTS: usize = 4;
/// How far, on each axis, a foe may be from the hero's previous tile and
/// still give chase.
pub const GAME_FOE_FOLLOW_RANGE: i32 = 2;
/// Villain roster slots a `VillainDef` can declare.
pub const VILLAIN_ARMY_SLOTS: usize = 5;
/// Highest villain index a set-free siege records in `villains_prefought`.
/// OpenBounty hardcodes 17 here rather than using the array's own length.
pub const VILLAINS_PREFOUGHT_CAP: i32 = 17;
/// Recruit cap standing in for the home castle's bottomless pool, OpenBounty's
/// `1 << 28`. Large enough never to bind before gold or leadership do.
pub const CASTLE_POOL_CAP: i32 = 1 << 28;

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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ChestOutcome {
    #[default]
    Gold = 0,
    Leadership = 1,
    Commission = 2,
    SpellPower = 3,
    MaxSpells = 4,
    NewSpell = 5,
    Empty = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecruitOutcome {
    Ok = 0,
    NoGold = 1,
    NoSlotOrInvalid = 2,
    OverLeadership = 3,
    IllegalLocation = 4,
}

/// Outcome of a town boat rent/cancel attempt, mirroring OpenBounty's
/// `BoatActionResult`. The engine applies the state mutation; the dialog for
/// each outcome stays with the shell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoatActionOutcome {
    RentOk = 0,
    RentNoGold = 1,
    CancelOk = 2,
    CancelAtSea = 3,
}

/// Outcome of a town siege-weapons purchase, mirroring OpenBounty's
/// `SiegeBuyResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SiegeBuyOutcome {
    Ok = 0,
    Already = 1,
    NoGold = 2,
}

/// Outcome of a garrison transfer at an owned castle. OpenBounty returns bare
/// int codes here (0/1/2) rather than a named enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GarrisonOutcome {
    Ok = 0,
    Refused = 1,
    WouldLeaveNoArmy = 2,
}

/// Outcome of a villain-castle siege. OpenBounty returns only a temp-death
/// bool and builds its dialog inline; the engine reports which branch ran so
/// a shell can write the message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VillainSiegeOutcome {
    /// The hero lost. The caller must run the temporary-death handling.
    Repelled = 0,
    /// Won holding the matching contract: the castle is taken and the
    /// contract fulfilled.
    Captured = 1,
    /// Won without the contract: the Lord keeps his castle, rebuilds from his
    /// starting roster, and the defeat is remembered as pre-fought.
    SetFree = 2,
    /// Won, but no castle record matched the id, so nothing was applied.
    NoCastle = 3,
}

/// Outcome of a town spell purchase, mirroring OpenBounty's `SpellBuyResult`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellBuyOutcome {
    Ok = 0,
    NoSpell = 1,
    AtCap = 2,
    NoGold = 3,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChestPending {
    pub pending_gold: i32,
    pub pending_leadership: i32,
}

/// Everything a chest roll leaves for the shell to render, plus the deferred
/// gold/leadership choice. C's `GameRollChest` formats the message text inline
/// with the same numbers; the engine here returns the values instead.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ChestRoll {
    pub outcome: ChestOutcome,
    /// The gold outcome's pending choice; zeroed for every other outcome.
    pub pending: ChestPending,
    /// Commission / spell power / max spells: the `+N` delta.
    pub points: i32,
    /// New spell: the count and the spell's catalog id.
    pub spell_count: i32,
    pub spell_id: FixedStr<CAT_ID_LEN>,
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
    pub last_astrology_troop: i32,
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
            last_astrology_troop: 0,
            options: [1; GAME_OPTIONS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArtifactState {
    pub found: [bool; CAT_ARTIFACTS_MAX],
}

impl Default for ArtifactState {
    fn default() -> Self {
        Self {
            found: [false; CAT_ARTIFACTS_MAX],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spellbook {
    pub counts: [i32; CAT_SPELLS_MAX],
}

impl Default for Spellbook {
    fn default() -> Self {
        Self {
            counts: [0; CAT_SPELLS_MAX],
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
    /// Mirror flag for a pack whose hero art is a single strip.
    pub facing_left: bool,
    /// Which way the hero last moved. Only a pack that authors four facings
    /// distinguishes north from south, but `facing_left` is kept consistent
    /// with it so a save written by either build reads correctly in the other.
    pub facing: Facing,
    pub in_town: FixedStr<GAME_ID_LEN>,
    /// Home (audience) castle the hero is currently transacting with, or empty
    /// when not on its gate. Home-pool recruiting keys off this marker, and it
    /// has the same one-step lifetime as `in_town`.
    pub home_castle: FixedStr<GAME_ID_LEN>,
    /// Player-owned castle the hero is currently at, or empty otherwise.
    /// Garrison transfers key off this marker, with the same one-step
    /// lifetime as `in_town` and `home_castle`.
    pub own_castle: FixedStr<GAME_ID_LEN>,
    pub dwelling_troop: FixedStr<CAT_ID_LEN>,
    pub dwelling_x: i32,
    pub dwelling_y: i32,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
            last_x: 0,
            last_y: 0,
            facing_left: false,
            facing: Facing::South,
            in_town: FixedStr::empty(),
            home_castle: FixedStr::empty(),
            own_castle: FixedStr::empty(),
            dwelling_troop: FixedStr::empty(),
            dwelling_x: -1,
            dwelling_y: -1,
        }
    }
}

/// Display name length for a gate list row. Ports `GateDestination.name`.
pub const GATE_DEST_NAME_LEN: usize = 64;

/// The largest gate list either kind can produce. Ports
/// `GAME_GATE_DESTS_MAX`.
pub const GAME_GATE_DESTS_MAX: usize = if GAME_TOWNS > GAME_CASTLES {
    GAME_TOWNS
} else {
    GAME_CASTLES
};

/// One row of a gate spell's destination picker. Ports `GateDestination`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GateDestination {
    pub name: FixedStr<GATE_DEST_NAME_LEN>,
    pub zone: FixedStr<GAME_ID_LEN>,
    /// Landing tile, taken from the gate coordinates with the plain x/y as
    /// the fallback.
    pub x: i32,
    pub y: i32,
}

/// Which list a gate spell is asking for. Ports `GateDestKind`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateDestKind {
    /// Castle Gate, which excludes the home/audience castle.
    Castle,
    Town,
}

/// Case-insensitive name order, with the zone as a stable tiebreak. Ports
/// `gate_name_icmp` and `gate_dest_cmp`. Note that C lowercases only while
/// both strings still have characters: the trailing comparison that decides
/// a prefix pair uses the raw bytes.
fn gate_dest_cmp(a: &GateDestination, b: &GateDestination) -> std::cmp::Ordering {
    let (left, right) = (a.name.as_str().as_bytes(), b.name.as_str().as_bytes());
    let mut index = 0usize;
    while index < left.len() && index < right.len() {
        let ca = left[index].to_ascii_lowercase();
        let cb = right[index].to_ascii_lowercase();
        if ca != cb {
            return ca.cmp(&cb);
        }
        index += 1;
    }
    let tail_a = left.get(index).copied().unwrap_or(0);
    let tail_b = right.get(index).copied().unwrap_or(0);
    match tail_a.cmp(&tail_b) {
        std::cmp::Ordering::Equal => a.zone.as_str().cmp(b.zone.as_str()),
        ordering => ordering,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FriendlyOffer {
    pub troop_id: FixedStr<CAT_ID_LEN>,
    pub count: i32,
    pub foe_id: FixedStr<CAT_ID_LEN>,
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
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
    pub orbs_found: [bool; RES_MAX_ZONES],
    /// Fog retained per continent, so a revisited zone keeps the terrain it
    /// has already revealed. The active zone's live `Fog` is authoritative
    /// and wins over its snapshot here, which may be stale.
    pub continent_fog: [Fog; GAME_CONTINENTS],
}

impl Default for WorldProgress {
    fn default() -> Self {
        Self {
            zones_discovered: [false; RES_MAX_ZONES],
            orbs_found: [false; RES_MAX_ZONES],
            continent_fog: [Fog::new(); GAME_CONTINENTS],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScepterState {
    pub key: i32,
    pub continent: i32,
    pub zone: FixedStr<GAME_ID_LEN>,
    pub x: i32,
    pub y: i32,
}

impl Default for ScepterState {
    fn default() -> Self {
        Self {
            key: 0,
            continent: 0,
            zone: FixedStr::empty(),
            x: 0,
            y: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Game {
    pub seed_index: i32,
    pub seed_from_catalog: bool,
    pub seed: u64,
    /// Oracle/demo session flag, transient and never serialized. When set,
    /// rank promotion still fires automatically on capturing a villain -- the
    /// legacy behavior the autoplay search depends on. Real play leaves it
    /// false, so the hero is promoted only via an audience with the King.
    pub oracle_mode: bool,
    pub scepter: ScepterState,
    pub character: Character,
    pub stats: Stats,
    pub position: Position,
    pub travel_mode: TravelMode,
    pub boat: BoatState,
    pub army: [ArmyStack; GAME_ARMY_SLOTS],
    pub spells: Spellbook,
    pub contract: Contract,
    pub artifacts: ArtifactState,
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
            oracle_mode: false,
            scepter: ScepterState::default(),
            character: Character::default(),
            stats: Stats::default(),
            position: Position::default(),
            travel_mode: TravelMode::Walk,
            boat: BoatState::default(),
            army: [ArmyStack::default(); GAME_ARMY_SLOTS],
            spells: Spellbook::default(),
            contract: Contract::default(),
            artifacts: ArtifactState::default(),
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
        if let Some(resources) = resources {
            self.bury_scepter(resources);
        }

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
        if let Some(resources) = resources {
            self.populate_castles(resources);
            self.enforce_declared_dwellings(resources);
        }
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

    pub fn bury_scepter_marker(&mut self, resources: &Resources) {
        if self.scepter.continent < 0 || self.scepter.continent as usize >= resources.zone_count() {
            return;
        }
        if let Some(zone) = resources.zone_by_index(self.scepter.continent as usize) {
            self.scepter.zone.copy_from_str(zone.id.as_str());
            self.scepter.x = -1;
            self.scepter.y = -1;
        }
    }

    pub fn bury_scepter(&mut self, resources: &Resources) {
        self.bury_scepter_marker(resources);
        if self.scepter.zone.is_empty() {
            return;
        }
        if let Ok(map) = Map::load_zone(resources, self.scepter.zone.as_str()) {
            let zone = self.scepter.zone;
            self.bury_scepter_in_map(zone.as_str(), &map);
        }
    }

    pub fn bury_scepter_in_map(&mut self, zone_id: &str, map: &Map) {
        self.scepter.zone.copy_from_str(zone_id);
        self.scepter.x = -1;
        self.scepter.y = -1;

        let total = map.eligible_scepter_tile_count();
        if total == 0 {
            return;
        }

        let target = self.world_rng.next(0, total as i32 - 1) as usize;
        if let Some((x, y)) = map.nth_eligible_scepter_tile(target) {
            self.scepter.x = x as i32;
            self.scepter.y = y as i32;
        }
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

    /// Accept the alcove offer: pay the cost, learn magic, and clear the tile.
    /// Ports the success branch of `flow_apply_alcove`. Returns true when magic
    /// was granted; false when the hero cannot afford the cost, in which case
    /// nothing changes and the tile stays (C reports the no-gold message).
    pub fn accept_alcove(&mut self, map: &mut Map, resources: &Resources) -> bool {
        let cost = resources.economy.alcove_cost;
        if self.stats.gold < cost {
            return false;
        }
        self.stats.gold -= cost;
        self.stats.knows_magic = true;
        let (x, y) = (self.position.x, self.position.y);
        let zone = self.position.zone.as_str().to_owned();
        map.clear_interactive(x, y);
        self.add_consumed(&zone, x, y);
        true
    }

    pub fn claim_artifact_by_id(&mut self, resources: &Resources, id: &str) -> bool {
        let Some(artifact) = resources.artifact_by_id(id) else {
            return false;
        };
        self.claim_artifact(resources, artifact.index)
    }
    pub fn claim_artifact(&mut self, resources: &Resources, index: i32) -> bool {
        if index < 0 || index as usize >= CAT_ARTIFACTS_MAX {
            return false;
        }
        let index = index as usize;
        if index >= resources.artifacts_count() || self.artifacts.found[index] {
            return false;
        }
        self.artifacts.found[index] = true;

        let Some(artifact) = resources.artifact_by_index(index as i32) else {
            return true;
        };
        match artifact.power {
            ArtifactPower::DoubleLeadership => {
                self.stats.leadership_base *= 2;
                self.stats.leadership_current = self.stats.leadership_base;
            }
            ArtifactPower::IncreaseCommission => {
                self.stats.commission_weekly += 2000;
            }
            ArtifactPower::DoubleSpellPower => {
                self.stats.spell_power *= 2;
            }
            ArtifactPower::DoubleMaxSpells => {
                self.stats.max_spells *= 2;
            }
            ArtifactPower::Unknown
            | ArtifactPower::IncreasedDamage
            | ArtifactPower::QuarterProtection
            | ArtifactPower::CheaperBoats => {}
        }
        true
    }

    pub fn has_power(&self, resources: &Resources, power: ArtifactPower) -> bool {
        for index in 0..resources.artifacts_count().min(CAT_ARTIFACTS_MAX) {
            let Some(artifact) = resources.artifact_by_index(index as i32) else {
                continue;
            };
            if self.artifacts.found[index] && artifact.power == power {
                return true;
            }
        }
        false
    }

    pub fn artifacts_found(&self, resources: &Resources) -> i32 {
        self.artifacts.found[..resources.artifacts_count().min(CAT_ARTIFACTS_MAX)]
            .iter()
            .filter(|found| **found)
            .count() as i32
    }

    pub fn peek_chest(
        &self,
        resources: &Resources,
        zone_index: usize,
        x: i32,
        y: i32,
    ) -> ChestRoll {
        let mut copy = self.clone();
        copy.chest_outcome(resources, zone_index, x, y, false)
    }

    pub fn roll_chest(
        &mut self,
        resources: &Resources,
        zone_index: usize,
        x: i32,
        y: i32,
    ) -> ChestRoll {
        self.chest_outcome(resources, zone_index, x, y, true)
    }

    fn chest_outcome(
        &mut self,
        resources: &Resources,
        zone_index: usize,
        x: i32,
        y: i32,
        apply: bool,
    ) -> ChestRoll {
        let tier = zone_index.min(3);
        let chest = &resources.economy.chest;
        let chance = (chest_rand(self.seed, x, y, 1) % 100) as i32 + 1;

        if chance < chest.chance_gold[tier] {
            let max = if chest.gold_max[tier] > 0 {
                chest.gold_max[tier] as u32
            } else {
                1
            };
            let points = (chest_rand(self.seed, x, y, 2) % max) as i32 + 1 + chest.gold_min[tier];
            let gold = points * 100;
            let mut leadership = gold / 50;
            if self.has_power(resources, ArtifactPower::DoubleLeadership) {
                leadership *= 2;
            }
            return ChestRoll {
                outcome: ChestOutcome::Gold,
                pending: ChestPending {
                    pending_gold: gold,
                    pending_leadership: leadership,
                },
                ..ChestRoll::default()
            };
        }

        if chance < chest.chance_commission[tier] {
            let max = if chest.commission_max[tier] > 0 {
                chest.commission_max[tier] as u32
            } else {
                1
            };
            let points =
                (chest_rand(self.seed, x, y, 3) % max) as i32 + 1 + chest.commission_min[tier];
            if apply {
                self.stats.commission_weekly += points;
            }
            return ChestRoll {
                outcome: ChestOutcome::Commission,
                points,
                ..ChestRoll::default()
            };
        }

        if chance < chest.chance_spell_power[tier] {
            if apply {
                self.stats.spell_power += 1;
            }
            return ChestRoll {
                outcome: ChestOutcome::SpellPower,
                points: 1,
                ..ChestRoll::default()
            };
        }

        if chance < chest.chance_max_spells[tier] {
            let mut points = chest.max_spells_base[tier];
            if self.has_power(resources, ArtifactPower::DoubleMaxSpells) {
                points *= 2;
            }
            if apply {
                self.stats.max_spells += points;
            }
            return ChestRoll {
                outcome: ChestOutcome::MaxSpells,
                points,
                ..ChestRoll::default()
            };
        }

        if chance < chest.chance_new_spell[tier] {
            let spell_count = resources.spells_count();
            if spell_count == 0 {
                return ChestRoll {
                    outcome: ChestOutcome::Empty,
                    ..ChestRoll::default()
                };
            }
            let spell_type = (chest_rand(self.seed, x, y, 4) % spell_count as u32) as usize;
            let spell_num = (chest_rand(self.seed, x, y, 5) % (tier as u32 + 1)) as i32 + 1;
            if apply && spell_type < self.spells.counts.len() {
                self.spells.counts[spell_type] += spell_num;
            }
            let spell_id = resources
                .spell_by_index(spell_type as i32)
                .map(|spell| spell.id)
                .unwrap_or_default();
            return ChestRoll {
                outcome: ChestOutcome::NewSpell,
                spell_count: spell_num,
                spell_id,
                ..ChestRoll::default()
            };
        }

        ChestRoll {
            outcome: ChestOutcome::Empty,
            ..ChestRoll::default()
        }
    }

    pub fn accept_chest_gold(&mut self, gold: i32) {
        if gold > 0 {
            self.stats.gold += gold;
        }
    }

    pub fn accept_chest_leadership(&mut self, leadership: i32) {
        if leadership > 0 {
            self.stats.leadership_base += leadership;
            self.stats.leadership_current += leadership;
        }
    }

    pub fn activate_orb(&mut self, resources: &Resources) -> Option<usize> {
        let zone_index = resources.zone_index_by_id(self.position.zone.as_str())?;
        if zone_index >= self.world.orbs_found.len() {
            return None;
        }
        self.world.orbs_found[zone_index] = true;
        Some(zone_index)
    }

    pub fn resolve_navmap(&mut self, resources: &Resources, id: &str) -> Option<usize> {
        let current_zone = resources.zone_index_by_id(self.position.zone.as_str());
        let mut target_zone = first_number_in_id(id).and_then(|index| {
            if index >= 0 {
                Some(index as usize)
            } else {
                None
            }
        });

        let target_usable = target_zone
            .filter(|target| *target < resources.zone_count())
            .filter(|target| Some(*target) != current_zone)
            .filter(|target| {
                *target < self.world.zones_discovered.len() && !self.world.zones_discovered[*target]
            });

        target_zone = target_usable.or_else(|| {
            (0..resources.zone_count())
                .find(|index| Some(*index) != current_zone && !self.world.zones_discovered[*index])
        });

        let target = target_zone?;
        if target >= self.world.zones_discovered.len() {
            return None;
        }
        self.world.zones_discovered[target] = true;
        Some(target)
    }

    pub fn resolve_telecave(&mut self) -> Option<(i32, i32)> {
        let zone = self.position.zone.as_str();
        let x = self.position.x;
        let y = self.position.y;
        let mut self_seq = None;
        let mut seq = 0usize;
        for placement in &self.placements[..self.placement_count] {
            if placement.zone.as_str() != zone || placement.kind != Interact::Telecave {
                continue;
            }
            if placement.x == x && placement.y == y {
                self_seq = Some(seq);
            }
            seq += 1;
        }

        let self_seq = self_seq?;
        let target_seq = if self_seq % 2 == 0 {
            self_seq + 1
        } else {
            self_seq.saturating_sub(1)
        };
        let mut seq = 0usize;
        for placement in &self.placements[..self.placement_count] {
            if placement.zone.as_str() != zone || placement.kind != Interact::Telecave {
                continue;
            }
            if seq == target_seq {
                self.position.x = placement.x;
                self.position.y = placement.y;
                self.position.last_x = placement.x;
                self.position.last_y = placement.y;
                return Some((placement.x, placement.y));
            }
            seq += 1;
        }
        None
    }

    /// Stash the fog of the zone the hero is leaving. Ports the opening block
    /// of `GameSwitchZone`, which runs BEFORE the map load and therefore
    /// still happens when that load goes on to fail.
    fn snapshot_outgoing_fog(&mut self, resources: &Resources, fog: &Fog) {
        if self.position.zone.is_empty() {
            return;
        }
        let Some(outgoing) = resources.zone_index_by_id(self.position.zone.as_str()) else {
            return;
        };
        if outgoing < GAME_CONTINENTS {
            self.world.continent_fog[outgoing] = *fog;
        }
    }

    /// Finish a zone switch once the destination's tiles are in hand: place
    /// the hero at the spawn, match travel mode to the spawn terrain, and
    /// restore or initialise fog. The tail of `GameSwitchZone`.
    fn enter_loaded_zone(
        &mut self,
        resources: &Resources,
        map: &mut Map,
        fog: &mut Fog,
        zone_id: &str,
        loaded: Map,
    ) -> bool {
        let Some(zone_index) = resources.zone_index_by_id(zone_id) else {
            return false;
        };
        let (spawn_x, spawn_y) = match resources.zone_by_index(zone_index) {
            // C reads `map->hero_spawn_x`, which `MapLoadZone` copies
            // straight off the zone record; the Rust map carries no such
            // field, so read the record itself.
            Some(zone) => (zone.hero_spawn_x, zone.hero_spawn_y),
            None => return false,
        };

        *map = loaded;
        // Re-apply consumed tiles so picked-up artifacts and chests stay gone.
        map.apply_consumed(self, zone_id);

        self.position.zone.copy_from_str(zone_id);
        self.position.x = spawn_x;
        self.position.y = spawn_y;
        self.position.last_x = self.position.x;
        self.position.last_y = self.position.y;

        // Match the travel mode to the spawn terrain. A cross-continent
        // arrival can land on open ocean or on land, and without this,
        // sailing home would leave the hero in a boat standing on grass.
        let spawn = map.tile_i32(self.position.x, self.position.y);
        let on_water = spawn.is_some_and(|tile| tile.terrain == Terrain::Water && !tile.is_bridge);
        if on_water {
            self.travel_mode = TravelMode::Boat;
            self.boat.has_boat = true;
            self.boat.x = self.position.x;
            self.boat.y = self.position.y;
            self.boat.zone.copy_from_str(zone_id);
        } else {
            self.travel_mode = TravelMode::Walk;
            // Keep the boat, moored at the spawn, so the hero can sail away
            // again.
            if self.boat.has_boat {
                self.boat.x = self.position.x;
                self.boat.y = self.position.y;
                self.boat.zone.copy_from_str(zone_id);
            }
        }

        // A continent revisited by boat or gate keeps what it had revealed;
        // a first visit starts blank.
        if zone_index < GAME_CONTINENTS && self.world.zones_discovered[zone_index] {
            *fog = self.world.continent_fog[zone_index];
        } else {
            *fog = Fog::default();
        }
        fog.reveal(
            map,
            self.position.x,
            self.position.y,
            resources.world.fog_sight,
        );
        if zone_index < GAME_CONTINENTS {
            self.world.zones_discovered[zone_index] = true;
        }
        true
    }

    /// Move the hero to `zone_id`, loading that zone's map from the pack.
    /// Ports `GameSwitchZone`.
    pub fn switch_zone(
        &mut self,
        resources: &Resources,
        map: &mut Map,
        fog: &mut Fog,
        zone_id: &str,
    ) -> bool {
        if resources.zone_index_by_id(zone_id).is_none() {
            return false;
        }
        self.snapshot_outgoing_fog(resources, fog);
        let Ok(loaded) = Map::load_zone_with_placements(resources, zone_id, self) else {
            return false;
        };
        self.enter_loaded_zone(resources, map, fog, zone_id, loaded)
    }

    /// [`Game::switch_zone`] with the destination's tiles supplied directly
    /// instead of read from the pack, matching the seam `Map` already draws
    /// between its path and byte loaders.
    pub fn switch_zone_bytes(
        &mut self,
        resources: &Resources,
        map: &mut Map,
        fog: &mut Fog,
        zone_id: &str,
        bytes: &[u8],
    ) -> bool {
        let Some(zone) = resources.zone_by_id(zone_id) else {
            return false;
        };
        self.snapshot_outgoing_fog(resources, fog);
        let Ok(loaded) = Map::load_zone_bytes_with_placements(resources, zone, bytes, self) else {
            return false;
        };
        self.enter_loaded_zone(resources, map, fog, zone_id, loaded)
    }

    /// Visited, gate-eligible destinations of `kind`, sorted by display name
    /// and written into `out`. Returns how many were written. Ports
    /// `GameGateDestinations`: the engine owns visited-eligibility so the
    /// shell never inspects `towns`/`castles` itself.
    pub fn gate_destinations(
        &self,
        resources: &Resources,
        kind: GateDestKind,
        out: &mut [GateDestination],
    ) -> usize {
        if out.is_empty() {
            return 0;
        }
        let mut written = 0usize;
        match kind {
            GateDestKind::Castle => {
                let count = resources.castle_count().min(GAME_CASTLES);
                for index in 0..count {
                    if written >= out.len() {
                        break;
                    }
                    if !self.castles[index].visited {
                        continue;
                    }
                    let Some(castle) = resources.castle_by_index(index) else {
                        continue;
                    };
                    if castle.is_home() {
                        continue;
                    }
                    if castle.zone.is_empty() {
                        continue;
                    }
                    out[written].name.copy_from_str(if castle.name.is_empty() {
                        castle.id.as_str()
                    } else {
                        castle.name.as_str()
                    });
                    out[written].zone.copy_from_str(castle.zone.as_str());
                    out[written].x = if castle.gate_x >= 0 {
                        castle.gate_x
                    } else {
                        castle.x
                    };
                    out[written].y = if castle.gate_y >= 0 {
                        castle.gate_y
                    } else {
                        castle.y
                    };
                    written += 1;
                }
            }
            GateDestKind::Town => {
                let count = resources.town_count().min(GAME_TOWNS);
                for index in 0..count {
                    if written >= out.len() {
                        break;
                    }
                    if !self.towns[index].visited {
                        continue;
                    }
                    let Some(town) = resources.town_by_index(index) else {
                        continue;
                    };
                    if town.zone.is_empty() {
                        continue;
                    }
                    out[written].name.copy_from_str(if town.name.is_empty() {
                        town.id.as_str()
                    } else {
                        town.name.as_str()
                    });
                    out[written].zone.copy_from_str(town.zone.as_str());
                    out[written].x = if town.gate_x >= 0 {
                        town.gate_x
                    } else {
                        town.x
                    };
                    out[written].y = if town.gate_y >= 0 {
                        town.gate_y
                    } else {
                        town.y
                    };
                    written += 1;
                }
            }
        }
        if written > 1 {
            // C sorts with qsort, which is unstable; the zone tiebreak makes
            // that immaterial for anything but two destinations sharing both
            // a name and a zone. A stable sort keeps even that case
            // deterministic.
            out[..written].sort_by(gate_dest_cmp);
        }
        written
    }

    /// Teleport the hero through a gate spell. Ports `GameGateTeleport`: any
    /// boat is LEFT BEHIND at the cast tile in the origin zone, the hero
    /// always arrives on foot, and one charge of `spell_id` is spent.
    pub fn gate_teleport(
        &mut self,
        resources: &Resources,
        map: &mut Map,
        fog: &mut Fog,
        destination: &GateDestination,
        spell_id: &str,
    ) -> bool {
        if destination.zone.is_empty() {
            return false;
        }

        // Snapshot the abandoned boat BEFORE the zone switch, which would
        // otherwise carry it to the destination spawn, and restore it after
        // so the boat's zone stays the origin zone.
        let mut left_behind = BoatState::default();
        let mut have_left = false;
        if self.travel_mode == TravelMode::Boat {
            left_behind.has_boat = true;
            left_behind.x = self.position.x;
            left_behind.y = self.position.y;
            left_behind.zone.copy_from_str(self.position.zone.as_str());
            have_left = true;
        } else if self.boat.has_boat {
            // A docked boat, with the hero on foot, also stays where it is.
            left_behind = self.boat;
            have_left = true;
        }

        if !self.switch_zone(resources, map, fog, destination.zone.as_str()) {
            return false;
        }

        // Always arrive on foot at the gate's landing tile. Note that no fog
        // is revealed here: `switch_zone` revealed around the zone's SPAWN,
        // and C never re-reveals after moving the hero, so the landing tile
        // stays dark until the next step.
        if destination.x >= 0 && destination.y >= 0 {
            self.position.x = destination.x;
            self.position.y = destination.y;
            self.position.last_x = destination.x;
            self.position.last_y = destination.y;
        }
        self.travel_mode = TravelMode::Walk;

        // Override the re-attach `switch_zone` performed at the spawn.
        if have_left {
            self.boat = left_behind;
        }

        // The charge is spent by spell id, not by effect, and only if one is
        // actually held.
        if let Some(spell) = resources.spell_by_id(spell_id) {
            if let Ok(index) = usize::try_from(spell.index) {
                if index < self.spells.counts.len() && self.spells.counts[index] > 0 {
                    self.spells.counts[index] = self.spells.counts[index].wrapping_sub(1);
                }
            }
        }
        true
    }

    /// Whether puzzle cell `(row, col)` has been uncovered yet: its villain
    /// caught, or its artifact found. Ports `cell_revealed`.
    ///
    /// C bounds the cell nowhere: an out-of-range cell reads entity 0, which
    /// is villain index 0, so it reports revealed once that villain is
    /// caught. Callers pass 0..4 and the port keeps the same shape.
    ///
    /// Do NOT reach for `VillainDef::puzzle_cell` or `ArtifactDef::puzzle_cell`
    /// here. Those are a flat 0..24 entity numbering the extraction tool
    /// authored, unrelated to a position in this grid, and no engine code
    /// reads them.
    pub fn puzzle_cell_revealed(&self, row: i32, col: i32) -> bool {
        let entity = puzzle_grid_entity(row, col);
        if entity >= 0 {
            return usize::try_from(entity)
                .ok()
                .and_then(|entity| self.contract.villains_caught.get(entity))
                .copied()
                .unwrap_or(false);
        }
        // A negative entity is an artifact index encoded as -(index) - 1.
        let artifact = entity.wrapping_neg().wrapping_sub(1);
        usize::try_from(artifact)
            .ok()
            .and_then(|artifact| self.artifacts.found.get(artifact))
            .copied()
            .unwrap_or(false)
    }

    /// How many puzzle cells stand uncovered. Every villain caught and every
    /// artifact found turns over one more.
    pub fn puzzle_cells_revealed(&self) -> i32 {
        let mut revealed = 0_i32;
        for row in 0..5 {
            for col in 0..5 {
                if self.puzzle_cell_revealed(row, col) {
                    revealed = revealed.wrapping_add(1);
                }
            }
        }
        revealed
    }

    /// What the coming week will actually leave in the purse. Ports
    /// `GameWeeklyNetGold`.
    ///
    /// The boat fare is judged on the POST-CREDIT wallet, mirroring
    /// `apply_week_end`, which banks the commission and pays the upkeep
    /// before it looks at the fare. Judging on the pre-credit wallet would
    /// predict "repossessed, not charged" for a poor hero whose commission
    /// covers the fare, overstating weekly income by the whole fare.
    pub fn weekly_net_gold(&self, resources: &Resources) -> i32 {
        let mut net = self
            .stats
            .commission_weekly
            .wrapping_sub(self.army_weekly_upkeep(resources));
        if self.boat.has_boat {
            let boat_cost = self.boat_cost(resources);
            if self.stats.gold.wrapping_add(net) >= boat_cost {
                net = net.wrapping_sub(boat_cost);
            }
            // Otherwise the boat is repossessed rather than charged for.
        }
        net
    }

    /// What the coming week CHARGES, gross. Ports `GameWeeklyOutgoings`.
    ///
    /// Unlike [`Game::weekly_net_gold`], the fare counts unconditionally --
    /// this is the bill, not what the wallet can meet.
    pub fn weekly_outgoings(&self, resources: &Resources) -> i32 {
        let boat = if self.boat.has_boat {
            self.boat_cost(resources)
        } else {
            0
        };
        self.army_weekly_upkeep(resources).wrapping_add(boat)
    }

    /// Send a defeated hero home with nothing. Ports `GameTempDeath`.
    ///
    /// Everything is forfeit: the army, the siege weapons, and the boat --
    /// the boat explicitly, because the zone switch below would otherwise
    /// carry it to the home spawn and materialise a rental on dry land at
    /// the King's castle.
    pub fn temp_death(&mut self, resources: &Resources, map: &mut Map, fog: &mut Fog) {
        for slot in 0..GAME_ARMY_SLOTS {
            self.army[slot].id = FixedStr::empty();
            self.army[slot].count = 0;
        }
        self.stats.siege_weapons = 0;

        // The consolation stack. A configured troop the catalog lacks is
        // rejected at load, so a miss here means an empty catalog, and the
        // hero is sent home with no army at all.
        if let Some(troop) = resources.troop_by_id(resources.tuning.temp_death_troop.as_str()) {
            self.army[0].id.copy_from_str(troop.id.as_str());
            self.army[0].count = resources.tuning.temp_death_count;
        }

        self.boat.has_boat = false;
        self.boat.x = -1;
        self.boat.y = -1;
        self.boat.zone = FixedStr::empty();

        for index in 0..resources.zone_count() {
            let Some(zone) = resources.zone_by_index(index) else {
                continue;
            };
            if !zone.is_home {
                continue;
            }
            let (home_x, home_y) = (zone.home_spawn_x, zone.home_spawn_y);
            let zone_id = zone.id;
            self.switch_zone(resources, map, fog, zone_id.as_str());
            // The switch put the hero on the zone's hero_spawn; the home
            // zone carries a SECOND, separate tile for this arrival.
            self.position.x = home_x;
            self.position.y = home_y;
            self.position.last_x = self.position.x;
            self.position.last_y = self.position.y;
            break;
        }
        self.character.mount = Mount::Ride;
        self.travel_mode = TravelMode::Walk;
    }

    /// Dismiss the stack in `slot`. Ports `flow_apply_dismiss_army`.
    ///
    /// Returns true when this was the LAST occupied stack, in which case
    /// nothing is dismissed: the caller must confirm, because emptying the
    /// army entirely sends the hero back to the King in disgrace.
    pub fn dismiss_army_slot(&mut self, slot: usize) -> bool {
        if slot >= GAME_ARMY_SLOTS {
            return false;
        }
        let mut occupied = 0;
        for index in 0..GAME_ARMY_SLOTS {
            if !self.army[index].id.is_empty() && self.army[index].count > 0 {
                occupied += 1;
            }
        }
        if occupied <= 1 {
            return true;
        }
        self.army[slot].id = FixedStr::empty();
        self.army[slot].count = 0;
        self.compact_army();
        false
    }

    /// Whether the whole army can take to the air. Ports `GamePlayerCanFly`.
    ///
    /// EVERY non-empty stack must qualify, so one earthbound stack grounds
    /// the hero. Note that an EMPTY army passes vacuously: the loop finds
    /// nothing to reject and returns true.
    pub fn player_can_fly(&self, resources: &Resources) -> bool {
        for slot in 0..GAME_ARMY_SLOTS {
            if self.army[slot].id.is_empty() || self.army[slot].count == 0 {
                continue;
            }
            if !troop_flies(resources.troop_by_id(self.army[slot].id.as_str())) {
                return false;
            }
        }
        true
    }

    /// Take to the air. Ports `GameMountFly`: the transition is legal only
    /// from `Mount::Ride`, so a hero already flying or at sea is refused.
    pub fn mount_fly(&mut self, resources: &Resources) -> bool {
        if self.character.mount != Mount::Ride || !self.player_can_fly(resources) {
            return false;
        }
        self.character.mount = Mount::Fly;
        true
    }

    /// Whether `(x, y)` will take a landing. Ports `GameCanLandAt`: plain
    /// grass, nothing interactive on it, and not a visual blocker.
    ///
    /// `is_bridge` is deliberately NOT consulted, and a built bridge is grass
    /// with no interactive and no blocker, so a hero can land on one.
    pub fn can_land_at(&self, map: &Map, x: i32, y: i32) -> bool {
        let Some(tile) = map.tile_i32(x, y) else {
            return false;
        };
        tile.terrain == Terrain::Grass && tile.interactive == Interact::None && !tile.blocks_foot
    }

    /// Come down where the hero is. Ports `GameLandHere`.
    pub fn land_here(&mut self, map: &Map) -> bool {
        if self.character.mount != Mount::Fly {
            return false;
        }
        if !self.can_land_at(map, self.position.x, self.position.y) {
            return false;
        }
        self.character.mount = Mount::Ride;
        true
    }

    /// Ports `GameVillainsCaught`. The scan covers the ARRAY EXTENT, not the
    /// catalog count: `villains_caught` is keyed by pack-authored
    /// `VillainDef.index`, which load validation constrains for range and
    /// uniqueness but not for density.
    pub fn villains_caught_count(&self) -> i32 {
        let mut caught = 0_i32;
        for index in 0..CAT_VILLAINS_MAX {
            if self.contract.villains_caught[index] {
                caught = caught.wrapping_add(1);
            }
        }
        caught
    }

    /// Ports `GameArtifactsFound`, whose loop bound is a hard-coded 8 rather
    /// than the catalog's artifact count. A pack shipping more artifacts can
    /// have them found and still not scored.
    pub fn artifacts_found_count(&self) -> i32 {
        let mut found = 0_i32;
        for index in 0..8 {
            if self.artifacts.found[index] {
                found = found.wrapping_add(1);
            }
        }
        found
    }

    /// Ports `GameCastlesOwned`.
    pub fn castles_owned_count(&self) -> i32 {
        let mut owned = 0_i32;
        for index in 0..GAME_CASTLES {
            if self.castles[index].id.is_empty() {
                continue;
            }
            if self.castles[index].owner_kind == CastleOwnerKind::Player {
                owned = owned.wrapping_add(1);
            }
        }
        owned
    }

    /// The final score. Ports `GameComputeScore`.
    ///
    /// Note the difficulty branch: `easy_halves` HALVES the easy score rather
    /// than multiplying it, and `difficulty_multiplier[0]` is dead while that
    /// flag is set. With the flag cleared, difficulty 0 falls through both
    /// arms and is left unscaled -- the `d >= 1` test excludes it.
    pub fn compute_score(&self, resources: &Resources) -> i32 {
        let scoring = &resources.economy.scoring;
        let mut score = scoring
            .per_villain
            .wrapping_mul(self.villains_caught_count())
            .wrapping_add(
                scoring
                    .per_artifact
                    .wrapping_mul(self.artifacts_found_count()),
            )
            .wrapping_add(scoring.per_castle.wrapping_mul(self.castles_owned_count()))
            .wrapping_sub(
                scoring
                    .kill_penalty
                    .wrapping_mul(self.stats.followers_killed),
            );
        let difficulty = self.character.difficulty;
        if scoring.easy_halves && difficulty == 0 {
            score /= 2;
        } else if (1..5).contains(&difficulty) {
            score = score.wrapping_mul(scoring.difficulty_multiplier[difficulty as usize]);
        }
        if score < 0 {
            score = 0;
        }
        score
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

    pub fn enforce_declared_dwellings(&mut self, resources: &Resources) {
        for zi in 0..resources.zone_count() {
            let Some(zone) = resources.zone_by_index(zi) else {
                continue;
            };
            for di in 0..zone.dwelling_count {
                let dwelling = zone.dwellings[di];
                self.enforce_dwelling(
                    resources,
                    zone.id.as_str(),
                    dwelling.x,
                    dwelling.y,
                    dwelling.kind.as_str(),
                );
            }
        }
    }

    pub fn enforce_dwelling(
        &mut self,
        resources: &Resources,
        zone: &str,
        x: i32,
        y: i32,
        dwelling_kind: &str,
    ) -> Option<&mut DwellingState> {
        if zone.is_empty() || dwelling_kind.is_empty() {
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
        let troop_id = self.dwelling_troop_at(resources, dwelling_kind, x, y)?;
        self.enforce_dwelling_pinned(resources, zone, x, y, troop_id.as_str())
    }

    pub fn touch_dwelling(
        &mut self,
        resources: &Resources,
        zone: &str,
        x: i32,
        y: i32,
        dwelling_kind: &str,
    ) -> Option<DwellingState> {
        let dwelling = *self.enforce_dwelling(resources, zone, x, y, dwelling_kind)?;
        if !dwelling.troop_id.is_empty() {
            self.position
                .dwelling_troop
                .copy_from_str(dwelling.troop_id.as_str());
            self.position.dwelling_x = x;
            self.position.dwelling_y = y;
        }
        Some(dwelling)
    }

    fn dwelling_troop_at(
        &self,
        resources: &Resources,
        dwelling_kind: &str,
        x: i32,
        y: i32,
    ) -> Option<String> {
        let normalized = normalize_dwelling_kind(dwelling_kind);
        if normalized.is_empty() {
            return None;
        }

        let mut candidates = [0_i32; crate::tables::CAT_TROOPS_MAX];
        let mut count = 0usize;
        for index in 0..resources.troops_count() {
            let Some(troop) = resources.troop_by_index(index as i32) else {
                continue;
            };
            if troop.dwelling.as_str() == normalized {
                candidates[count] = index as i32;
                count += 1;
                if count >= candidates.len() {
                    break;
                }
            }
        }
        if count == 0 {
            return None;
        }

        let mut hash = self.seed as u32;
        hash ^= (x as u32).wrapping_mul(131);
        hash ^= (y as u32).wrapping_mul(97);
        hash = hash.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let pick = (hash % count as u32) as usize;
        resources
            .troop_by_index(candidates[pick])
            .map(|troop| troop.id.as_str().to_owned())
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

    /// Locate a castle record by id. Unlike `find_foe` this is NOT scoped to
    /// the hero's zone: castle ids are unique world-wide, where foe placement
    /// ids restart their numbering in every zone.
    fn find_castle(&self, castle_id: &str) -> Option<&CastleRecord> {
        if castle_id.is_empty() {
            return None;
        }
        self.castles
            .iter()
            .find(|castle| castle.id.as_str() == castle_id)
    }

    fn find_castle_index(&self, castle_id: &str) -> Option<usize> {
        if castle_id.is_empty() {
            return None;
        }
        self.castles
            .iter()
            .position(|castle| castle.id.as_str() == castle_id)
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

    pub fn recruit_location_cap(&self, resources: &Resources, troop_id: &str) -> i32 {
        let Some(troop) = resources.troop_by_id(troop_id) else {
            return -1;
        };
        // A dwelling the hero is standing on that offers this troop sells it
        // whatever the troop's catalog class: the salt roll can seed
        // castle-class troops at dwellings, and a castle-first early return
        // would refuse the very shop the world spawned.
        if !self.position.dwelling_troop.is_empty()
            && self.position.dwelling_troop.as_str() == troop_id
        {
            return self.standing_dwelling_cap(troop_id);
        }
        if troop.dwelling.as_str() == "castle" {
            // Home pool: the castle never runs dry, so the cap is only the
            // gate marker.
            return if self.position.home_castle.is_empty() {
                -1
            } else {
                CASTLE_POOL_CAP
            };
        }
        if self.position.dwelling_troop.is_empty()
            || self.position.dwelling_troop.as_str() != troop_id
        {
            return -1;
        }
        self.standing_dwelling_cap(troop_id)
    }

    /// Population of the dwelling the hero is standing on, if it offers
    /// `troop_id`, else -1.
    fn standing_dwelling_cap(&self, troop_id: &str) -> i32 {
        for dwelling in &self.dwellings[..self.dwelling_count] {
            if dwelling.x == self.position.dwelling_x
                && dwelling.y == self.position.dwelling_y
                && dwelling.zone.as_str() == self.position.zone.as_str()
                && dwelling.troop_id.as_str() == troop_id
            {
                return dwelling.count;
            }
        }
        -1
    }

    pub fn dwelling_recruit_cap(&self, resources: &Resources, troop_id: &str) -> i32 {
        let Some(troop) = resources.troop_by_id(troop_id) else {
            return 0;
        };
        let max = self.max_recruitable(resources, troop_id).max(0);
        let affordable = if troop.recruit_cost > 0 {
            self.stats.gold / troop.recruit_cost
        } else {
            0
        };
        max.min(affordable)
            .min(self.recruit_location_cap(resources, troop_id).max(0))
    }

    pub fn buy_troop(
        &mut self,
        resources: &Resources,
        troop_id: &str,
        count: i32,
    ) -> RecruitOutcome {
        if troop_id.is_empty() || count <= 0 {
            return RecruitOutcome::NoSlotOrInvalid;
        }
        let Some(troop) = resources.troop_by_id(troop_id) else {
            return RecruitOutcome::NoSlotOrInvalid;
        };
        let loc_cap = self.recruit_location_cap(resources, troop_id);
        if loc_cap < 0 || count > loc_cap {
            return RecruitOutcome::IllegalLocation;
        }
        let total_cost = troop.recruit_cost * count;
        if self.stats.gold < total_cost {
            return RecruitOutcome::NoGold;
        }
        if count > self.max_recruitable(resources, troop_id) {
            return RecruitOutcome::OverLeadership;
        }
        if self.add_troop(troop_id, count) != 0 {
            return RecruitOutcome::NoSlotOrInvalid;
        }
        self.stats.gold -= total_cost;
        RecruitOutcome::Ok
    }

    pub fn recruit_from_dwelling(
        &mut self,
        resources: &Resources,
        zone: &str,
        x: i32,
        y: i32,
        troop_id: &str,
        requested: i32,
    ) -> RecruitOutcome {
        if requested <= 0 {
            return RecruitOutcome::NoSlotOrInvalid;
        }
        let count = requested.min(self.max_recruitable(resources, troop_id).max(0));
        if count <= 0 {
            return RecruitOutcome::NoSlotOrInvalid;
        }
        let outcome = self.buy_troop(resources, troop_id, count);
        if outcome == RecruitOutcome::Ok {
            for dwelling in &mut self.dwellings[..self.dwelling_count] {
                if dwelling.x == x && dwelling.y == y && dwelling.zone.as_str() == zone {
                    dwelling.count -= count;
                    if dwelling.count < 0 {
                        dwelling.count = 0;
                    }
                    break;
                }
            }
        }
        outcome
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

    /// Build the deterministic free-troop offer for a friendly foe. This mirrors
    /// `start_foe_friendly_flow`: it deliberately does not consume world RNG.
    pub fn friendly_offer(
        &self,
        resources: &Resources,
        foe_id: &str,
        x: i32,
        y: i32,
    ) -> Option<FriendlyOffer> {
        let foe = self.find_foe(foe_id)?;
        if !foe.alive || !foe.friendly {
            return None;
        }

        let continent = resources
            .zone_index_by_id(self.position.zone.as_str())
            .unwrap_or(0);
        let tier = continent & 3;
        let kind = self.friendly_tile_hash(x, y, 131, 17, self.seed) & 3;
        let chance = (self.friendly_tile_hash(x, y, 17, 131, self.seed >> 8) % 100 + 1) as i32;
        let mut pool_slot = 0;
        while pool_slot < RES_SPAWN_POOL_N - 1
            && chance > resources.spawn.chance_curve[tier][pool_slot]
        {
            pool_slot += 1;
        }

        let troop_id = resources.spawn.troop_pool[kind as usize][pool_slot].as_str();
        let troop = resources.troop_by_id(troop_id)?;
        let count = troop.tier_counts[tier].max(2);
        if !self.has_army_slot_for(troop_id) {
            return None;
        }

        let mut offer = FriendlyOffer {
            troop_id: FixedStr::empty(),
            count,
            foe_id: FixedStr::empty(),
            zone: FixedStr::empty(),
            x,
            y,
        };
        offer.troop_id.copy_from_str(troop_id);
        offer.foe_id.copy_from_str(foe_id);
        offer.zone.copy_from_str(self.position.zone.as_str());
        Some(offer)
    }

    /// Start a friendly encounter. If no valid offer can be made, OpenBounty
    /// makes the foe flee immediately; otherwise the caller resolves the offer.
    pub fn begin_friendly_encounter(
        &mut self,
        map: &mut Map,
        resources: &Resources,
        foe_id: &str,
        x: i32,
        y: i32,
    ) -> Option<FriendlyOffer> {
        let offer = self.friendly_offer(resources, foe_id, x, y);
        let is_live_friendly = self
            .find_foe(foe_id)
            .is_some_and(|foe| foe.alive && foe.friendly);
        if offer.is_none() && is_live_friendly {
            let zone = self.position.zone;
            self.consume_friendly_foe(map, foe_id, zone.as_str(), x, y);
        }
        offer
    }

    /// Resolve the yes/no choice. A friendly foe is always consumed; accepting
    /// adds its offered troop stack without charging gold.
    pub fn resolve_friendly_offer(&mut self, map: &mut Map, offer: FriendlyOffer, accept: bool) {
        if accept && !offer.troop_id.is_empty() && offer.count > 0 {
            self.add_troop(offer.troop_id.as_str(), offer.count);
        }
        self.consume_friendly_foe(
            map,
            offer.foe_id.as_str(),
            offer.zone.as_str(),
            offer.x,
            offer.y,
        );
    }

    /// Fight the hostile foe the hero has chosen to attack, then apply the
    /// outcome. The shell owns the attack/decline prompt and passes only the
    /// resolved decision, so this is the whole engine side of the flow:
    /// OpenBounty's target build plus `combat_run_headless` plus
    /// `flow_apply_attack_foe`.
    ///
    /// Returns the battle result and the `temp_death` flag the caller must
    /// act on after a loss.
    pub fn attack_hostile_foe(
        &mut self,
        map: &mut Map,
        resources: &Resources,
        foe_id: &str,
        foe_x: i32,
        foe_y: i32,
    ) -> (CombatResult, bool) {
        // C leaves the target's garrison NULL when the foe cannot be found,
        // which fields an empty AI side; an all-empty garrison does the same.
        let garrison = self
            .find_foe(foe_id)
            .map(|foe| foe.garrison)
            .unwrap_or_default();
        let target = CombatTarget {
            // The seed key is the placement id, so the constant display name
            // never reaches the RNG.
            name: "Hostile band",
            garrison: &garrison,
            seed_key: foe_id,
        };
        let outcome =
            combat_run_headless(self, resources, CombatMode::Foe, &target, COMBAT_MAX_ROUNDS);
        let temp_death = self.resolve_hostile_foe(map, foe_id, foe_x, foe_y, outcome);
        (outcome, temp_death)
    }

    /// True when any OTHER live foe in `zone` already holds (x, y).
    fn foe_occupies(&self, zone: &str, x: i32, y: i32, except: usize) -> bool {
        self.foes[..self.foe_count]
            .iter()
            .enumerate()
            .any(|(index, foe)| {
                index != except
                    && foe.alive
                    && foe.x == x
                    && foe.y == y
                    && foe.zone.as_str() == zone
            })
    }

    /// Advance every live in-zone foe one step toward the hero's PREVIOUS
    /// tile. Ports `GameFoesFollow`.
    ///
    /// Returns the index of the foe that stepped onto the hero, which is the
    /// collision that triggers the attack flow.
    pub fn foes_follow(&mut self, map: &mut Map) -> Option<usize> {
        let (tx, ty) = (self.position.last_x, self.position.last_y);
        let (hero_x, hero_y) = (self.position.x, self.position.y);
        let zone = self.position.zone;
        let mut collided = None;

        // Stamp re-sync: a foe that stepped onto the hero is left unstamped
        // while they share a tile, and nothing else re-stamps it once the
        // hero moves away. Restore the invariant before reading the map for
        // standability below, skipping the hero's tile so a foe standing
        // there stays unstamped and the collision contract holds.
        for index in 0..self.foe_count {
            let foe = self.foes[index];
            if !foe.alive || foe.zone != zone {
                continue;
            }
            if foe.x == hero_x && foe.y == hero_y {
                continue;
            }
            map.stamp_foe(foe.x, foe.y, foe.placement_id.as_str());
        }

        // Friendly and hostile foes both follow; the distinction only matters
        // at attack time.
        for index in 0..self.foe_count {
            let foe = self.foes[index];
            if !foe.alive || foe.zone != zone {
                continue;
            }
            if (foe.x - tx).abs() > GAME_FOE_FOLLOW_RANGE
                || (foe.y - ty).abs() > GAME_FOE_FOLLOW_RANGE
            {
                continue;
            }

            // Evaluate all nine neighbourhood cells. The center is always
            // eligible, so a foe may stand still; non-center cells must be
            // real, foe-standable ground.
            let mut best_dist = u32::MAX;
            let (mut best_x, mut best_y) = (foe.x, foe.y);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (foe.x + dx, foe.y + dy);
                    if map.tile_i32(nx, ny).is_none() {
                        continue;
                    }
                    let is_center = dx == 0 && dy == 0;
                    let is_hero_tile = nx == hero_x && ny == hero_y;
                    // A flying hero is untouchable: the foe may neither step
                    // onto them nor chase across the water and trees they are
                    // flying over. The oracle session keeps the legacy
                    // behavior its search is tuned against.
                    if is_hero_tile && self.character.mount == Mount::Fly && !self.oracle_mode {
                        continue;
                    }
                    // Only for the anti-stacking exemption, never for
                    // walkability: a foe standing on the hero is the combat
                    // trigger, not a stack, so a second foe may still target
                    // that tile. Drops away in the boat, where the hero sits
                    // on water no land foe can occupy anyway.
                    let hero_reachable = is_hero_tile && self.travel_mode != TravelMode::Boat;
                    // The hero's tile gets NO standability exemption: the
                    // original tests the map byte of all eight non-center
                    // cells without caring where the player is, so a hero
                    // standing on desert, a bridge, a town or any other
                    // interactive tile simply cannot be reached.
                    if !is_center && !foe_can_stand(map, nx, ny) {
                        continue;
                    }
                    if !is_center
                        && !hero_reachable
                        && self.foe_occupies(zone.as_str(), nx, ny, index)
                    {
                        continue;
                    }
                    let dist = foe_dist_sq(nx, ny, tx, ty);
                    if dist < best_dist {
                        best_dist = dist;
                        best_x = nx;
                        best_y = ny;
                    }
                }
            }
            if best_x == foe.x && best_y == foe.y {
                continue;
            }

            // Foe stamp only: never clear a pickup the hero-tile exception
            // let the foe stand on.
            map.clear_foe_stamp(foe.x, foe.y);
            self.foes[index].x = best_x;
            self.foes[index].y = best_y;

            if best_x == hero_x && best_y == hero_y {
                // Stepped onto the hero: surface it and leave the tile
                // unstamped, since the hero is standing on it.
                collided = Some(index);
                continue;
            }
            map.stamp_foe(best_x, best_y, foe.placement_id.as_str());
        }
        collided
    }

    /// Lay siege to a monster-held castle: fight its garrison, then apply the
    /// outcome. Ports OpenBounty's castle target build plus
    /// `flow_apply_siege_monster`.
    ///
    /// Villain-held castles do NOT belong here -- their capture is gated on
    /// the active contract and rebuilds the lord's army on a no-contract win.
    ///
    /// Returns the battle result and the `temp_death` flag the caller must
    /// act on after a loss.
    pub fn siege_monster_castle(
        &mut self,
        resources: &Resources,
        castle_id: &str,
    ) -> (CombatResult, bool) {
        let outcome = self.fight_castle_garrison(resources, castle_id);
        let temp_death = self.resolve_monster_siege(castle_id, outcome);
        (outcome, temp_death)
    }

    /// Fight a castle's garrison on the siege layout. C builds this target in
    /// one branch shared by both siege flows, so both callers share it here.
    fn fight_castle_garrison(&mut self, resources: &Resources, castle_id: &str) -> CombatResult {
        let garrison = self
            .find_castle(castle_id)
            .map(|castle| castle.garrison)
            .unwrap_or_default();
        // Display name only: the seed key below is what reaches the RNG.
        let villain_id = self
            .find_castle(castle_id)
            .map(|castle| castle.villain_id)
            .unwrap_or_default();
        let name = resources
            .villain_by_id(villain_id.as_str())
            .map(|villain| villain.name.as_str())
            .filter(|name| !name.is_empty())
            .or_else(|| {
                resources
                    .castle_by_id(castle_id)
                    .map(|castle| castle.name.as_str())
                    .filter(|name| !name.is_empty())
            })
            .unwrap_or(castle_id);
        let target = CombatTarget {
            name,
            garrison: &garrison,
            seed_key: castle_id,
        };
        combat_run_headless(
            self,
            resources,
            CombatMode::Castle,
            &target,
            COMBAT_MAX_ROUNDS,
        )
    }

    /// Lay siege to a villain-held castle: fight the lord's army, then apply
    /// the outcome. Ports `flow_apply_siege_villain`.
    ///
    /// Capture is gated on the lord matching the hero's active contract.
    /// Without it the Lord is set free: he keeps his castle and rebuilds from
    /// his starting roster, the spoils already credited by the combat core
    /// stand, and the defeat is remembered so pre-weakening a late lord
    /// before his contract window stays a legitimate play.
    pub fn siege_villain_castle(
        &mut self,
        resources: &Resources,
        castle_id: &str,
    ) -> (CombatResult, VillainSiegeOutcome) {
        let result = self.fight_castle_garrison(resources, castle_id);
        let outcome = self.resolve_villain_siege(resources, castle_id, result);
        (result, outcome)
    }

    /// Apply a completed villain-siege battle. Mirrors
    /// `flow_apply_siege_villain`.
    pub fn resolve_villain_siege(
        &mut self,
        resources: &Resources,
        castle_id: &str,
        outcome: CombatResult,
    ) -> VillainSiegeOutcome {
        if outcome != CombatResult::Win {
            return VillainSiegeOutcome::Repelled;
        }
        let Some(index) = self.find_castle_index(castle_id) else {
            return VillainSiegeOutcome::NoCastle;
        };
        // Snapshot the lord's id before the castle is cleared: the contract
        // is fulfilled against it afterwards.
        let caught_vid = self.castles[index].villain_id;
        let captured = resources.villain_by_id(caught_vid.as_str()).copied();
        let contract_match = !caught_vid.is_empty()
            && !self.contract.active_id.is_empty()
            && caught_vid.as_str() == self.contract.active_id.as_str();

        if contract_match {
            let castle = &mut self.castles[index];
            castle.owner_kind = CastleOwnerKind::Player;
            castle.visited = true;
            castle.villain_id = FixedStr::empty();
            castle.garrison = [Unit::default(); GAME_ARMY_SLOTS];
            // The hero is standing at the castle it just took.
            self.position.own_castle.copy_from_str(castle_id);
        } else {
            let castle = &mut self.castles[index];
            castle.garrison = [Unit::default(); GAME_ARMY_SLOTS];
            if let Some(villain) = captured {
                for slot in 0..GAME_ARMY_SLOTS.min(VILLAIN_ARMY_SLOTS) {
                    if villain.army_troops[slot].is_empty() || villain.army_counts[slot] <= 0 {
                        continue;
                    }
                    castle.garrison[slot]
                        .id
                        .copy_from_str(villain.army_troops[slot].as_str());
                    castle.garrison[slot].count = villain.army_counts[slot];
                }
                // C guards this with a hardcoded 17, NOT the length of
                // villains_prefought (CAT_VILLAINS_MAX, 32), so a pack with
                // more than 17 villains records nothing for the later ones.
                if villain.index >= 0 && villain.index < VILLAINS_PREFOUGHT_CAP {
                    self.contract.villains_prefought[villain.index as usize] = true;
                }
            }
        }

        if contract_match {
            self.fulfill_contract(resources, caught_vid.as_str());
            // Real play promotes only at an audience with the King; the
            // oracle session keeps the legacy on-capture promotion so its
            // search stays byte-identical to the baseline.
            if self.oracle_mode {
                self.maybe_rank_up(resources);
            }
            VillainSiegeOutcome::Captured
        } else {
            VillainSiegeOutcome::SetFree
        }
    }

    /// Apply a completed monster-siege battle. Mirrors
    /// `flow_apply_siege_monster`. Returns `true` only when the caller must
    /// handle the player's loss.
    pub fn resolve_monster_siege(&mut self, castle_id: &str, outcome: CombatResult) -> bool {
        if outcome != CombatResult::Win {
            return outcome == CombatResult::Loss;
        }
        let Some(index) = self.find_castle_index(castle_id) else {
            return false;
        };
        let castle = &mut self.castles[index];
        castle.owner_kind = CastleOwnerKind::Player;
        castle.visited = true;
        castle.garrison = [Unit::default(); GAME_ARMY_SLOTS];
        // The hero just took this castle and is standing at it, so garrison
        // transfers are legal now. Cleared on the next move, like in_town.
        self.position.own_castle.copy_from_str(castle_id);
        false
    }

    /// Apply a completed hostile-foe battle. Combat itself belongs to the
    /// caller; this mirrors OpenBounty's `flow_apply_attack_foe` state change.
    /// Returns `true` only when the caller must handle the player's loss.
    pub fn resolve_hostile_foe(
        &mut self,
        map: &mut Map,
        foe_id: &str,
        foe_x: i32,
        foe_y: i32,
        outcome: CombatResult,
    ) -> bool {
        if outcome == CombatResult::Loss {
            return true;
        }

        // Only remove foe overlays. The hero may be on a different, live
        // pickup tile when a queued battle result arrives.
        map.clear_foe_stamp(foe_x, foe_y);

        let foe_index = self.foes[..self.foe_count].iter().position(|foe| {
            foe.placement_id.as_str() == foe_id
                && (self.position.zone.is_empty() || foe.zone == self.position.zone)
        });
        let Some(foe_index) = foe_index else {
            return false;
        };
        let foe = &mut self.foes[foe_index];
        foe.alive = false;
        map.clear_foe_stamp(foe.x, foe.y);
        false
    }

    pub fn stack_weekly_upkeep(resources: &Resources, troop_id: &str, count: i32) -> i32 {
        let Some(troop) = resources.troop_by_id(troop_id) else {
            return 0;
        };
        if count <= 0 {
            return 0;
        }
        count.wrapping_mul(troop.recruit_cost / 10)
    }

    pub fn army_weekly_upkeep(&self, resources: &Resources) -> i32 {
        self.army.iter().fold(0_i32, |total, stack| {
            total.wrapping_add(Self::stack_weekly_upkeep(
                resources,
                stack.id.as_str(),
                stack.count,
            ))
        })
    }

    pub fn boat_cost(&self, resources: &Resources) -> i32 {
        if self.has_power(resources, ArtifactPower::CheaperBoats) {
            resources.economy.boat_cost_cheap
        } else {
            resources.economy.boat_cost_normal
        }
    }

    /// Rent a boat at the town's dock tile. OpenBounty rents only from the
    /// town menu, so `position.in_town` must be set, and its guard is
    /// `gold <= cost`: an exact-cost purse buys nothing and leaves state
    /// untouched.
    pub fn rent_boat(
        &mut self,
        resources: &Resources,
        boat_x: i32,
        boat_y: i32,
        zone_id: &str,
    ) -> BoatActionOutcome {
        if self.position.in_town.is_empty() {
            return BoatActionOutcome::RentNoGold;
        }
        let cost = self.boat_cost(resources);
        if self.stats.gold <= cost {
            return BoatActionOutcome::RentNoGold;
        }
        self.stats.gold = self.stats.gold.wrapping_sub(cost);
        // OpenBounty clears the whole BoatState before writing the live
        // fields so two histories reaching the same boat share a byte image.
        self.boat = BoatState {
            has_boat: true,
            x: boat_x,
            y: boat_y,
            zone: FixedStr::from_str(zone_id),
        };
        BoatActionOutcome::RentOk
    }

    /// Cancel a boat rental. OpenBounty refuses while the hero is sailing, and
    /// reports `BOAT_CANCEL_OK` without touching state when the hero is not at
    /// a town.
    pub fn cancel_boat(&mut self) -> BoatActionOutcome {
        if self.position.in_town.is_empty() {
            return BoatActionOutcome::CancelOk;
        }
        if self.travel_mode == TravelMode::Boat {
            return BoatActionOutcome::CancelAtSea;
        }
        self.boat = BoatState::default();
        BoatActionOutcome::CancelOk
    }

    /// Buy siege weapons. Like the boat rental this is a town-only
    /// transaction guarded by `gold <= cost`, and OpenBounty checks ownership
    /// before affordability, so an owner who cannot afford a second set still
    /// gets `Already`.
    pub fn buy_siege(&mut self, resources: &Resources) -> SiegeBuyOutcome {
        if self.position.in_town.is_empty() {
            return SiegeBuyOutcome::NoGold;
        }
        if self.stats.siege_weapons != 0 {
            return SiegeBuyOutcome::Already;
        }
        let cost = resources.economy.siege_cost;
        if self.stats.gold <= cost {
            return SiegeBuyOutcome::NoGold;
        }
        self.stats.gold = self.stats.gold.wrapping_sub(cost);
        self.stats.siege_weapons = 1;
        SiegeBuyOutcome::Ok
    }

    /// Total spell charges carried, the sum of the whole spellbook. Ports
    /// `GameKnownSpells`, which sums counts[] rather than counting distinct
    /// spells, so a hero holding three charges of one spell reads as three.
    pub fn known_spells(&self) -> i32 {
        self.spells
            .counts
            .iter()
            .fold(0_i32, |total, count| total.wrapping_add(*count))
    }

    /// Buy the spell on sale at `town_id`. Unlike the boat and siege cores
    /// this names its town: OpenBounty requires `position.in_town` to equal
    /// `town_id`, so an off-map call and one naming a different town are both
    /// rejected.
    pub fn buy_spell(&mut self, resources: &Resources, town_id: &str) -> SpellBuyOutcome {
        if town_id.is_empty() || self.position.in_town.as_str() != town_id {
            return SpellBuyOutcome::NoSpell;
        }
        let Some(town) = self.towns.iter().find(|town| town.id.as_str() == town_id) else {
            return SpellBuyOutcome::NoSpell;
        };
        let spell_for_sale = town.spell_for_sale;
        if spell_for_sale.is_empty() {
            return SpellBuyOutcome::NoSpell;
        }
        let Some(spell) = resources.spell_by_id(spell_for_sale.as_str()) else {
            return SpellBuyOutcome::NoSpell;
        };
        let (index, cost) = (spell.index, spell.cost);
        // C indexes counts[sp->index] unchecked because its catalog bounds the
        // index; decline rather than panic on a catalog that does not.
        let Ok(index) = usize::try_from(index) else {
            return SpellBuyOutcome::NoSpell;
        };
        if index >= self.spells.counts.len() {
            return SpellBuyOutcome::NoSpell;
        }
        if self.known_spells() >= self.stats.max_spells {
            return SpellBuyOutcome::AtCap;
        }
        if self.stats.gold <= cost {
            return SpellBuyOutcome::NoGold;
        }
        self.spells.counts[index] = self.spells.counts[index].wrapping_add(1);
        self.stats.gold = self.stats.gold.wrapping_sub(cost);
        SpellBuyOutcome::Ok
    }

    pub fn week_id(&self, resources: &Resources) -> i32 {
        let difficulty = usize::try_from(self.character.difficulty)
            .ok()
            .filter(|difficulty| *difficulty < resources.time.days_per_difficulty.len())
            .unwrap_or(0);
        let passed =
            resources.time.days_per_difficulty[difficulty].wrapping_sub(self.stats.days_left);
        if resources.time.week_days > 0 {
            passed / resources.time.week_days
        } else {
            0
        }
    }

    pub fn pick_astrology_creature(&self, resources: &Resources, week_id: i32) -> i32 {
        let troop_count = resources.troops_count().max(1);
        if week_id & 3 == 0 {
            return 0;
        }
        let hash = (self.seed as u32 ^ week_id as u32)
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        let mut index = 1 + (hash % troop_count.saturating_sub(1).max(1) as u32) as i32;
        if index >= troop_count as i32 {
            index = 0;
        }
        index
    }

    pub fn grow_dwellings(&mut self, resources: &Resources) {
        for dwelling in &mut self.dwellings[..self.dwelling_count] {
            let Some(troop) = resources.troop_by_id(dwelling.troop_id.as_str()) else {
                continue;
            };
            dwelling.count = dwelling.count.wrapping_add(troop.growth_per_week);
            if dwelling.count > dwelling.max_population {
                dwelling.count = dwelling.max_population;
            }
        }
    }

    pub fn apply_astrology(
        &mut self,
        resources: &Resources,
        troop_index: i32,
    ) -> Option<FixedStr<CAT_ID_LEN>> {
        let astrology_troop = *resources.troop_by_index(troop_index)?;

        for dwelling in &mut self.dwellings[..self.dwelling_count] {
            if dwelling.troop_id == astrology_troop.id {
                dwelling.count = dwelling.max_population;
            }
        }

        if troop_index == 0 {
            for stack in &mut self.army {
                let Some(troop) = resources.troop_by_id(stack.id.as_str()) else {
                    continue;
                };
                if stack.count > 0 && troop.abilities & TROOP_ABIL_ABSORB != 0 {
                    stack.id = astrology_troop.id;
                }
            }
        }

        Some(astrology_troop.id)
    }

    /// Apply OpenBounty's entire week-boundary transaction and return the
    /// commission amount reported to the caller.
    pub fn apply_week_end(&mut self, resources: &Resources) -> i32 {
        self.stats.time_stop = 0;
        self.stats.leadership_current = self.stats.leadership_base;

        let astrology = self.pick_astrology_creature(resources, self.week_id(resources));
        self.stats.last_astrology_troop = astrology;
        self.stats.gold = self.stats.gold.wrapping_add(self.stats.commission_weekly);
        self.stats.last_commission = self.stats.commission_weekly;
        self.stats.gold = self
            .stats
            .gold
            .wrapping_sub(self.army_weekly_upkeep(resources));

        if self.boat.has_boat {
            let cost = self.boat_cost(resources);
            if self.stats.gold >= cost {
                self.stats.gold = self.stats.gold.wrapping_sub(cost);
            } else {
                self.boat.has_boat = false;
                self.boat.x = -1;
                self.boat.y = -1;
            }
        }
        if self.stats.gold < 0 {
            self.stats.gold = 0;
        }

        self.apply_astrology(resources, astrology);
        let Some(astrology_troop) = resources.troop_by_index(astrology) else {
            return self.stats.commission_weekly;
        };
        if astrology_troop.growth_per_week <= 0 {
            return self.stats.commission_weekly;
        }

        for castle in &mut self.castles {
            if castle.id.is_empty()
                || matches!(
                    castle.owner_kind,
                    CastleOwnerKind::Player | CastleOwnerKind::Special
                )
            {
                continue;
            }
            for stack in &mut castle.garrison {
                if stack.count > 0 && stack.id.as_str() == astrology_troop.id.as_str() {
                    stack.count = stack.count.wrapping_add(astrology_troop.growth_per_week);
                }
            }
        }
        for foe in &mut self.foes[..self.foe_count] {
            if !foe.alive {
                continue;
            }
            for stack in &mut foe.garrison {
                if stack.count > 0 && stack.id.as_str() == astrology_troop.id.as_str() {
                    stack.count = stack.count.wrapping_add(astrology_troop.growth_per_week);
                }
            }
        }
        self.stats.commission_weekly
    }

    fn find_foe(&self, placement_id: &str) -> Option<&FoeState> {
        if placement_id.is_empty() {
            return None;
        }
        self.foes[..self.foe_count].iter().find(|foe| {
            foe.placement_id.as_str() == placement_id
                && (self.position.zone.is_empty() || foe.zone == self.position.zone)
        })
    }

    fn has_army_slot_for(&self, troop_id: &str) -> bool {
        self.army.iter().any(|stack| {
            (stack.id.as_str() == troop_id && stack.count > 0)
                || stack.id.is_empty()
                || stack.count == 0
        })
    }

    fn friendly_tile_hash(&self, x: i32, y: i32, x_factor: i32, y_factor: i32, seed: u64) -> u32 {
        let offset = x
            .wrapping_mul(x_factor)
            .wrapping_add(y.wrapping_mul(y_factor));
        (seed.wrapping_add(offset as u64)) as u32
    }

    fn consume_friendly_foe(&mut self, map: &mut Map, foe_id: &str, zone: &str, x: i32, y: i32) {
        if map.clear_foe_stamp(x, y) {
            self.add_consumed(zone, x, y);
        }

        let foe_index = self.foes[..self.foe_count].iter().position(|foe| {
            foe.placement_id.as_str() == foe_id
                && (self.position.zone.is_empty() || foe.zone == self.position.zone)
        });
        let Some(foe_index) = foe_index else {
            return;
        };
        let foe = &mut self.foes[foe_index];
        foe.alive = false;
        let foe_zone = foe.zone;
        let (foe_x, foe_y) = (foe.x, foe.y);
        if map.clear_foe_stamp(foe_x, foe_y) {
            self.add_consumed(foe_zone.as_str(), foe_x, foe_y);
        }
    }

    /// Garrisoning is legal only at an owned castle the hero is standing at:
    /// `position.own_castle` must name THIS castle and the castle must be
    /// player-owned. Same contract as the `in_town` town cores.
    fn garrison_location_ok(&self, castle_id: &str) -> bool {
        if castle_id.is_empty() || self.position.own_castle.as_str() != castle_id {
            return false;
        }
        self.castles
            .iter()
            .find(|castle| castle.id.as_str() == castle_id)
            .is_some_and(|castle| castle.owner_kind == CastleOwnerKind::Player)
    }

    /// Move army slot `slot` into the castle's garrison. Ports
    /// `GameGarrisonTroop`.
    pub fn garrison_troop(&mut self, castle_id: &str, slot: usize) -> GarrisonOutcome {
        if slot >= GAME_ARMY_SLOTS || !self.garrison_location_ok(castle_id) {
            return GarrisonOutcome::Refused;
        }
        let src = self.army[slot];
        if src.id.is_empty() || src.count == 0 {
            return GarrisonOutcome::Refused;
        }
        // Refuse a transfer that would leave the hero with no army at all.
        let has_other = self
            .army
            .iter()
            .enumerate()
            .any(|(index, stack)| index != slot && !stack.id.is_empty() && stack.count > 0);
        if !has_other {
            return GarrisonOutcome::WouldLeaveNoArmy;
        }
        let Some(castle) = self
            .castles
            .iter_mut()
            .find(|castle| castle.id.as_str() == castle_id)
        else {
            return GarrisonOutcome::Refused;
        };
        // Destination: the first garrison stack already holding this troop,
        // else the first empty slot.
        let mut dst = None;
        for i in 0..GAME_ARMY_SLOTS {
            if castle.garrison[i].id.as_str() == src.id.as_str() && castle.garrison[i].count > 0 {
                dst = Some(i);
                break;
            }
            if (castle.garrison[i].id.is_empty() || castle.garrison[i].count == 0) && dst.is_none()
            {
                dst = Some(i);
            }
        }
        let Some(dst) = dst else {
            return GarrisonOutcome::Refused;
        };
        // The garrison id is narrower than the army id, and C's copy_id
        // truncates to the destination here.
        castle.garrison[dst].id.copy_from_str(src.id.as_str());
        castle.garrison[dst].count = castle.garrison[dst].count.wrapping_add(src.count);

        // Zeroing the source leaves a gap; OpenBounty compacts so the filled
        // slots stay contiguous for the A/B/C UI rows.
        self.army[slot] = ArmyStack::default();
        self.compact_army();
        GarrisonOutcome::Ok
    }

    /// Move garrison slot `slot` back into the army. Ports
    /// `GameUngarrisonTroop`.
    pub fn ungarrison_troop(&mut self, castle_id: &str, slot: usize) -> GarrisonOutcome {
        if slot >= GAME_ARMY_SLOTS || !self.garrison_location_ok(castle_id) {
            return GarrisonOutcome::Refused;
        }
        let Some(castle_index) = self
            .castles
            .iter()
            .position(|castle| castle.id.as_str() == castle_id)
        else {
            return GarrisonOutcome::Refused;
        };
        let src = self.castles[castle_index].garrison[slot];
        if src.id.is_empty() || src.count == 0 {
            return GarrisonOutcome::Refused;
        }
        // Destination: the first army stack already holding this troop, else
        // the first empty slot.
        let mut dst = None;
        for i in 0..GAME_ARMY_SLOTS {
            if self.army[i].id.as_str() == src.id.as_str() && self.army[i].count > 0 {
                dst = Some(i);
                break;
            }
            if (self.army[i].id.is_empty() || self.army[i].count == 0) && dst.is_none() {
                dst = Some(i);
            }
        }
        let Some(dst) = dst else {
            return GarrisonOutcome::Refused;
        };
        self.army[dst].id.copy_from_str(src.id.as_str());
        self.army[dst].count = self.army[dst].count.wrapping_add(src.count);

        // The garrison is shifted down from `slot` rather than compacted
        // wholesale, which is how C writes it.
        let garrison = &mut self.castles[castle_index].garrison;
        for i in slot..GAME_ARMY_SLOTS - 1 {
            garrison[i] = garrison[i + 1];
        }
        garrison[GAME_ARMY_SLOTS - 1] = Unit::default();
        GarrisonOutcome::Ok
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

fn chest_rand(seed: u64, x: i32, y: i32, salt: u32) -> u32 {
    let mut h = seed as u32 ^ 0x00C0_FFEE;
    h ^= (x as u32).wrapping_mul(131);
    h ^= (y as u32).wrapping_mul(97);
    h ^= salt;
    h.wrapping_mul(1_664_525).wrapping_add(1_013_904_223)
}

fn foe_dist_sq(x1: i32, y1: i32, x2: i32, y2: i32) -> u32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    (dx * dx + dy * dy) as u32
}

/// Keep foes off the eight tiles around a castle gate. OpenBounty flags this
/// as a house rule rather than a restoration: the original's bare 0x00 test
/// does not prevent it, because the approach tile below a gate is grass.
fn adjacent_to_castle_gate(map: &Map, x: i32, y: i32) -> bool {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            if map
                .tile_i32(x + dx, y + dy)
                .is_some_and(|tile| tile.interactive == Interact::CastleGate)
            {
                return true;
            }
        }
    }
    false
}

/// Whether a foe may occupy a tile. Deliberately stricter than terrain
/// walkability: the original accepts a candidate only when the map byte is
/// 0x00, i.e. plain grass. Desert is a non-zero byte, which is what makes it
/// a refuge -- foes neither cross it nor attack into it -- while the hero
/// still crosses it at full day cost. Bridges fall through to GRASS in
/// `TerrainFromArt`, so they need their own rejection or a foe walks over
/// water after the hero.
fn foe_can_stand(map: &Map, x: i32, y: i32) -> bool {
    let Some(tile) = map.tile_i32(x, y) else {
        return false;
    };
    if tile.blocks_foot {
        return false;
    }
    if tile.terrain != Terrain::Grass {
        return false;
    }
    if tile.is_bridge {
        return false;
    }
    if tile.interactive != Interact::None {
        return false;
    }
    !adjacent_to_castle_gate(map, x, y)
}

/// The per-troop half of [`Game::player_can_fly`]. Ports `GameTroopFlies`.
///
/// The FLY ability alone is not enough: the troop must ALSO be skill level 2
/// or better, so a low-tier flier cannot carry the hero.
pub fn troop_flies(troop: Option<&TroopDef>) -> bool {
    troop.is_some_and(|troop| (troop.abilities & TROOP_ABIL_FLY) != 0 && troop.skill_level >= 2)
}

fn first_number_in_id(id: &str) -> Option<i32> {
    let start = id
        .char_indices()
        .find(|(_, ch)| ch.is_ascii_digit())
        .map(|(index, _)| index)?;
    let digits = id[start..]
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    digits.parse().ok()
}

fn normalize_dwelling_kind(kind: &str) -> &str {
    if kind == "hills" {
        "hill"
    } else {
        kind
    }
}

#[cfg(test)]
mod tests {
    use super::troop_flies;
    use super::{
        ArmyStack, AudienceOutcome, BoatActionOutcome, BoatState, CastleOwnerKind, ChestOutcome,
        FoeState, Fog, Game, GarrisonOutcome, GateDestKind, GateDestination, Mount, RecruitOutcome,
        SiegeBuyOutcome, SpellBuyOutcome, TravelMode, Unit, VillainSiegeOutcome, CASTLE_POOL_CAP,
        GAME_ARMY_SLOTS, GAME_GATE_DESTS_MAX,
    };
    use crate::combat::{combat_seed_rng, Combat, CombatMode, CombatResult, CombatTarget};
    use crate::map::{Map, Tile};
    use crate::resources::ResourceError;
    use crate::resources::Resources;
    use crate::tables::{ArtifactPower, FixedStr};
    use crate::tile::{Interact, Terrain};

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
    fn claim_artifact_applies_stat_powers_once_and_tracks_passive_powers() {
        let resources = Resources::from_json_str(
            r#"{
              "troops": [],
              "artifacts": [
                {"index":0,"id":"crown","power":"double_leadership"},
                {"index":1,"id":"articles","power":"increase_commission"},
                {"index":2,"id":"amulet","power":"double_spell_power"},
                {"index":3,"id":"ring","power":"double_max_spells"},
                {"index":4,"id":"anchor","power":"cheaper_boats"}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.stats.leadership_base = 100;
        game.stats.leadership_current = 80;
        game.stats.commission_weekly = 500;
        game.stats.spell_power = 3;
        game.stats.max_spells = 2;

        assert!(game.claim_artifact_by_id(&resources, "crown"));
        assert_eq!(game.stats.leadership_base, 200);
        assert_eq!(game.stats.leadership_current, 200);
        assert!(!game.claim_artifact_by_id(&resources, "crown"));
        assert_eq!(game.stats.leadership_base, 200);

        assert!(game.claim_artifact_by_id(&resources, "articles"));
        assert_eq!(game.stats.commission_weekly, 2_500);
        assert!(game.claim_artifact_by_id(&resources, "amulet"));
        assert_eq!(game.stats.spell_power, 6);
        assert!(game.claim_artifact_by_id(&resources, "ring"));
        assert_eq!(game.stats.max_spells, 4);
        assert!(game.claim_artifact_by_id(&resources, "anchor"));
        assert!(game.has_power(&resources, ArtifactPower::CheaperBoats));
        assert_eq!(game.artifacts_found(&resources), 5);
    }

    #[test]
    fn chest_gold_roll_is_pending_until_player_accepts_choice() {
        let resources = Resources::from_json_str(
            r#"{
              "economy": {"chest": {
                "chance_gold": [101,101,101,101],
                "gold_min": [0,0,0,0],
                "gold_max": [5,5,5,5]
              }},
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game {
            seed: 0,
            ..Game::default()
        };

        let peek = game.peek_chest(&resources, 0, 4, 0);
        let rolled = game.roll_chest(&resources, 0, 4, 0);

        assert_eq!(rolled.outcome, ChestOutcome::Gold);
        assert_eq!(peek, rolled);
        assert!(rolled.pending.pending_gold > 0);
        assert_eq!(game.stats.gold, 0);
        assert_eq!(game.stats.leadership_base, 0);

        game.accept_chest_gold(rolled.pending.pending_gold);
        assert_eq!(game.stats.gold, rolled.pending.pending_gold);
        game.accept_chest_leadership(rolled.pending.pending_leadership);
        assert_eq!(
            game.stats.leadership_base,
            rolled.pending.pending_leadership
        );
        assert_eq!(
            game.stats.leadership_current,
            rolled.pending.pending_leadership
        );
    }

    #[test]
    fn chest_roll_applies_non_choice_rewards_eagerly() {
        let resources = Resources::from_json_str(
            r#"{
              "economy": {"chest": {
                "chance_gold": [0,0,0,0],
                "chance_commission": [0,0,0,0],
                "chance_spell_power": [0,0,0,0],
                "chance_max_spells": [0,0,0,0],
                "chance_new_spell": [101,101,101,101]
              }},
              "spells": [{"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100}],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game {
            seed: 0,
            ..Game::default()
        };

        let roll = game.roll_chest(&resources, 0, 4, 0);

        assert_eq!(roll.outcome, ChestOutcome::NewSpell);
        assert_eq!(roll.spell_count, 1);
        assert_eq!(roll.spell_id.as_str(), "bridge");
        assert_eq!(roll.pending.pending_gold, 0);
        assert_eq!(roll.pending.pending_leadership, 0);
        assert_eq!(game.spells.counts[0], 1);
    }

    fn castle_pool_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "castles": [
                {"index":0,"id":"castle0","zone":"home","x":4,"y":5,"special":{"flow":"audience"}},
                {"index":1,"id":"castle1","zone":"home","x":8,"y":9}
              ],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"},
                {"index":1,"id":"sprites","skill_level":1,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":5,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"forest","max_population":10}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    /// Hero on the home-castle gate with gold and leadership to spare.
    fn home_castle_game() -> Game {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.home_castle.copy_from_str("castle0");
        game.stats.gold = 1_000;
        game.stats.leadership_current = 100;
        game
    }

    #[test]
    fn audience_castle_is_identified_by_its_flow_marker() {
        let resources = castle_pool_fixture();

        assert!(resources.castle_by_id("castle0").unwrap().is_home());
        assert!(!resources.castle_by_id("castle1").unwrap().is_home());
        assert_eq!(
            resources.home_castle().map(|castle| castle.id.as_str()),
            Some("castle0")
        );
    }

    #[test]
    fn home_pool_troop_recruits_only_at_the_home_castle_gate() {
        let resources = castle_pool_fixture();
        let mut game = home_castle_game();

        assert_eq!(
            game.recruit_location_cap(&resources, "peasants"),
            CASTLE_POOL_CAP
        );
        assert_eq!(
            game.buy_troop(&resources, "peasants", 4),
            RecruitOutcome::Ok
        );
        assert_eq!(game.stats.gold, 960);
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 4);
    }

    #[test]
    fn home_pool_troop_is_illegal_away_from_the_gate() {
        let resources = castle_pool_fixture();
        let mut game = home_castle_game();
        game.position.home_castle.copy_from_str("");

        assert_eq!(game.recruit_location_cap(&resources, "peasants"), -1);
        assert_eq!(
            game.buy_troop(&resources, "peasants", 1),
            RecruitOutcome::IllegalLocation
        );
        assert_eq!(game.stats.gold, 1_000);
        assert_eq!(game.army[0].count, 0);
    }

    #[test]
    fn home_pool_never_runs_dry_but_still_yields_to_gold_and_leadership() {
        let resources = castle_pool_fixture();
        let mut game = home_castle_game();
        game.stats.gold = 35;
        game.stats.leadership_current = 100;

        // The location cap is effectively unbounded, so gold binds first.
        assert_eq!(
            game.recruit_location_cap(&resources, "peasants"),
            CASTLE_POOL_CAP
        );
        assert_eq!(
            game.buy_troop(&resources, "peasants", 4),
            RecruitOutcome::NoGold
        );

        game.stats.gold = 1_000;
        game.stats.leadership_current = 3;
        assert_eq!(
            game.buy_troop(&resources, "peasants", 4),
            RecruitOutcome::OverLeadership
        );
        assert_eq!(game.army[0].count, 0);
    }

    #[test]
    fn dwelling_the_hero_stands_on_outranks_the_castle_class_of_its_troop() {
        let resources = castle_pool_fixture();
        let mut game = home_castle_game();
        // The salt roll can seed a castle-class troop at a dwelling; standing
        // on that dwelling must sell it from the dwelling's population rather
        // than falling through to the bottomless home pool.
        game.position.home_castle.copy_from_str("");
        game.position.dwelling_troop.copy_from_str("peasants");
        game.position.dwelling_x = 2;
        game.position.dwelling_y = 3;
        game.dwellings[0].zone.copy_from_str("home");
        game.dwellings[0].x = 2;
        game.dwellings[0].y = 3;
        game.dwellings[0].troop_id.copy_from_str("peasants");
        game.dwellings[0].count = 6;
        game.dwelling_count = 1;

        assert_eq!(game.recruit_location_cap(&resources, "peasants"), 6);
        assert_eq!(
            game.buy_troop(&resources, "peasants", 7),
            RecruitOutcome::IllegalLocation
        );
        assert_eq!(
            game.buy_troop(&resources, "peasants", 6),
            RecruitOutcome::Ok
        );
    }

    #[test]
    fn dwelling_troop_stays_illegal_at_the_home_castle() {
        let resources = castle_pool_fixture();
        let mut game = home_castle_game();

        // "sprites" is a forest troop: the home-castle gate sells nothing.
        assert_eq!(game.recruit_location_cap(&resources, "sprites"), -1);
        assert_eq!(
            game.buy_troop(&resources, "sprites", 1),
            RecruitOutcome::IllegalLocation
        );
    }

    /// Hero standing at owned castle "keep" with two army stacks.
    fn garrison_game() -> Game {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.own_castle.copy_from_str("keep");
        game.castles[0].id.copy_from_str("keep");
        game.castles[0].owner_kind = CastleOwnerKind::Player;
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 4;
        game
    }

    #[test]
    fn garrison_moves_a_stack_into_the_castle_and_compacts_the_army() {
        let mut game = garrison_game();

        assert_eq!(game.garrison_troop("keep", 0), GarrisonOutcome::Ok);

        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
        assert_eq!(game.castles[0].garrison[0].count, 10);
        // The army compacts, so militia slides into slot A.
        assert_eq!(game.army[0].id.as_str(), "militia");
        assert_eq!(game.army[0].count, 4);
        assert!(game.army[1].id.is_empty());
    }

    #[test]
    fn garrison_refuses_to_leave_the_hero_with_no_army() {
        let mut game = garrison_game();
        game.army[1] = ArmyStack::default();

        assert_eq!(
            game.garrison_troop("keep", 0),
            GarrisonOutcome::WouldLeaveNoArmy
        );
        assert_eq!(game.army[0].count, 10);
        assert!(game.castles[0].garrison[0].id.is_empty());
    }

    #[test]
    fn garrison_requires_standing_at_that_owned_castle() {
        let mut game = garrison_game();

        // Marker naming a different castle than the one asked for.
        assert_eq!(game.garrison_troop("other", 0), GarrisonOutcome::Refused);

        // Marker cleared: stepping away revokes the privilege.
        game.position.own_castle.copy_from_str("");
        assert_eq!(game.garrison_troop("keep", 0), GarrisonOutcome::Refused);

        // At the castle, but it is no longer player-owned.
        game.position.own_castle.copy_from_str("keep");
        game.castles[0].owner_kind = CastleOwnerKind::Monsters;
        assert_eq!(game.garrison_troop("keep", 0), GarrisonOutcome::Refused);

        assert_eq!(game.army[0].count, 10);
    }

    #[test]
    fn garrison_refuses_an_empty_slot_or_one_out_of_range() {
        let mut game = garrison_game();

        assert_eq!(game.garrison_troop("keep", 4), GarrisonOutcome::Refused);
        assert_eq!(
            game.garrison_troop("keep", GAME_ARMY_SLOTS),
            GarrisonOutcome::Refused
        );
    }

    #[test]
    fn garrison_stacks_onto_a_matching_troop_past_an_earlier_empty_slot() {
        let mut game = garrison_game();
        game.castles[0].garrison[1].id.copy_from_str("peasants");
        game.castles[0].garrison[1].count = 3;

        // Slot 0 is empty but slot 1 already holds peasants: C scans for a
        // match first and only falls back to the remembered empty slot.
        assert_eq!(game.garrison_troop("keep", 0), GarrisonOutcome::Ok);

        assert_eq!(game.castles[0].garrison[1].count, 13);
        assert!(game.castles[0].garrison[0].id.is_empty());
    }

    #[test]
    fn ungarrison_returns_a_stack_and_shifts_the_garrison_down() {
        let mut game = garrison_game();
        game.castles[0].garrison[0].id.copy_from_str("archers");
        game.castles[0].garrison[0].count = 6;
        game.castles[0].garrison[1].id.copy_from_str("knights");
        game.castles[0].garrison[1].count = 2;

        assert_eq!(game.ungarrison_troop("keep", 0), GarrisonOutcome::Ok);

        assert_eq!(game.army[2].id.as_str(), "archers");
        assert_eq!(game.army[2].count, 6);
        // C shifts the garrison down from the vacated slot rather than
        // compacting it wholesale.
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "knights");
        assert_eq!(game.castles[0].garrison[0].count, 2);
        assert!(game.castles[0].garrison[1].id.is_empty());
    }

    #[test]
    fn ungarrison_merges_into_a_matching_army_stack() {
        let mut game = garrison_game();
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 7;

        assert_eq!(game.ungarrison_troop("keep", 0), GarrisonOutcome::Ok);

        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 17);
        assert!(game.army[2].id.is_empty());
    }

    #[test]
    fn ungarrison_refuses_an_empty_garrison_slot_and_keeps_the_location_guard() {
        let mut game = garrison_game();

        assert_eq!(game.ungarrison_troop("keep", 0), GarrisonOutcome::Refused);

        game.castles[0].garrison[0].id.copy_from_str("archers");
        game.castles[0].garrison[0].count = 6;
        game.position.own_castle.copy_from_str("");
        assert_eq!(game.ungarrison_troop("keep", 0), GarrisonOutcome::Refused);
        assert_eq!(game.castles[0].garrison[0].count, 6);
    }

    #[test]
    fn ungarrison_is_refused_when_the_army_is_full_of_other_troops() {
        let mut game = garrison_game();
        for (slot, id) in ["a", "b", "c", "d", "e"].iter().enumerate() {
            game.army[slot].id.copy_from_str(id);
            game.army[slot].count = 1;
        }
        game.castles[0].garrison[0].id.copy_from_str("archers");
        game.castles[0].garrison[0].count = 6;

        assert_eq!(game.ungarrison_troop("keep", 0), GarrisonOutcome::Refused);
        assert_eq!(game.castles[0].garrison[0].count, 6);
    }

    #[test]
    fn dwelling_visit_sets_recruit_source_and_successful_recruit_drains_population() {
        let resources = Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"sprites","skill_level":1,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":5,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"forest","max_population":10,"growth_per_week":3}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game {
            seed: 0,
            ..Game::default()
        };
        game.position.zone.copy_from_str("home");
        game.position.x = 2;
        game.position.y = 3;
        game.stats.gold = 100;
        game.stats.leadership_current = 20;

        let dwelling = game
            .touch_dwelling(&resources, "home", 2, 3, "forest")
            .expect("dwelling materializes");

        assert_eq!(dwelling.troop_id.as_str(), "sprites");
        assert_eq!(dwelling.count, 10);
        assert_eq!(game.position.dwelling_troop.as_str(), "sprites");
        assert_eq!((game.position.dwelling_x, game.position.dwelling_y), (2, 3));
        assert_eq!(game.dwelling_recruit_cap(&resources, "sprites"), 10);

        assert_eq!(
            game.recruit_from_dwelling(&resources, "home", 2, 3, "sprites", 7),
            RecruitOutcome::Ok
        );
        assert_eq!(game.stats.gold, 65);
        assert_eq!(game.army[0].id.as_str(), "sprites");
        assert_eq!(game.army[0].count, 7);
        assert_eq!(game.dwellings[0].count, 3);

        game.stats.leadership_current = 100;
        assert_eq!(
            game.recruit_from_dwelling(&resources, "home", 2, 3, "sprites", 5),
            RecruitOutcome::IllegalLocation
        );
        assert_eq!(game.dwellings[0].count, 3);
    }

    #[test]
    fn friendly_foe_offer_acceptance_and_decline_follow_openbounty_rules() {
        let resources = friendly_foe_fixture();
        let mut game = friendly_foe_game();
        let mut map = Map::new(4, 1);
        map.stamp_foe(1, 0, "friendly0");

        let offer = game
            .begin_friendly_encounter(&mut map, &resources, "friendly0", 1, 0)
            .expect("available army slot receives an offer");
        assert_eq!(offer.troop_id.as_str(), "sprites");
        assert_eq!(offer.count, 7);
        assert_eq!(game.rng_snapshot(), 0);

        game.resolve_friendly_offer(&mut map, offer, true);
        assert_eq!(game.army[0].id.as_str(), "sprites");
        assert_eq!(game.army[0].count, 7);
        assert!(!game.foes[0].alive);
        assert!(game.is_consumed("home", 1, 0));
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::None
        );

        let mut declined_game = friendly_foe_game();
        let mut declined_map = Map::new(4, 1);
        declined_map.stamp_foe(1, 0, "friendly0");
        let declined_offer = declined_game
            .begin_friendly_encounter(&mut declined_map, &resources, "friendly0", 1, 0)
            .expect("available army slot receives an offer");
        declined_game.resolve_friendly_offer(&mut declined_map, declined_offer, false);
        assert!(declined_game.army.iter().all(|stack| stack.count == 0));
        assert!(!declined_game.foes[0].alive);
        assert!(declined_game.is_consumed("home", 1, 0));
    }

    #[test]
    fn friendly_foe_without_army_capacity_flees_and_clears_current_stamp() {
        let resources = friendly_foe_fixture();
        let mut game = friendly_foe_game();
        for (index, stack) in game.army.iter_mut().enumerate() {
            stack
                .id
                .copy_from_str(if index == 0 { "other0" } else { "other" });
            stack.count = 1;
        }
        game.foes[0].x = 2;
        game.foes[0].y = 0;
        let mut map = Map::new(4, 1);
        map.stamp_foe(1, 0, "friendly0");
        map.stamp_foe(2, 0, "friendly0");

        assert!(game
            .begin_friendly_encounter(&mut map, &resources, "friendly0", 1, 0)
            .is_none());
        assert!(!game.foes[0].alive);
        assert!(game.is_consumed("home", 1, 0));
        assert!(game.is_consumed("home", 2, 0));
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::None
        );
        assert_eq!(
            map.tile_i32(2, 0).expect("tile exists").interactive,
            Interact::None
        );
    }

    #[test]
    fn hostile_foe_win_clears_stale_and_current_stamps_without_consuming_tiles() {
        let mut game = friendly_foe_game();
        game.foes[0].friendly = false;
        game.foes[0].x = 2;
        game.foes[0].y = 0;
        let mut other_zone_foe = game.foes[0];
        other_zone_foe.zone.copy_from_str("away");
        game.foes[1] = other_zone_foe;
        game.foe_count = 2;

        let mut map = Map::new(4, 1);
        map.stamp_foe(1, 0, "friendly0");
        map.stamp_foe(2, 0, "friendly0");

        assert!(!game.resolve_hostile_foe(&mut map, "friendly0", 1, 0, CombatResult::Win));
        assert!(!game.foes[0].alive);
        assert!(game.foes[1].alive);
        assert_eq!(game.consumed_count, 0);
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::None
        );
        assert_eq!(
            map.tile_i32(2, 0).expect("tile exists").interactive,
            Interact::None
        );
    }

    #[test]
    fn hostile_foe_loss_leaves_world_state_for_temporary_death_handling() {
        let mut game = friendly_foe_game();
        game.foes[0].friendly = false;
        let mut map = Map::new(2, 1);
        map.stamp_foe(1, 0, "friendly0");

        assert!(game.resolve_hostile_foe(&mut map, "friendly0", 1, 0, CombatResult::Loss));
        assert!(game.foes[0].alive);
        assert_eq!(game.consumed_count, 0);
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::Foe
        );
    }

    /// Troops for a foe encounter: ogres are the heavy, peasants the fodder.
    fn foe_combat_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":2,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    /// Hero in "home" with an army, facing one live hostile band at (1, 0).
    fn hostile_band_game(hero: &[(&str, i32)], band: &[(&str, i32)]) -> Game {
        let mut game = Game::default();
        game.seed = 4_242;
        game.position.zone.copy_from_str("home");
        game.stats.leadership_current = 10_000;
        for (slot, (id, count)) in hero.iter().enumerate() {
            game.army[slot].id.copy_from_str(id);
            game.army[slot].count = *count;
        }
        let mut foe = FoeState::default();
        foe.zone = game.position.zone;
        foe.x = 1;
        foe.y = 0;
        foe.origin_x = 1;
        foe.origin_y = 0;
        foe.placement_id.copy_from_str("band0");
        foe.alive = true;
        foe.friendly = false;
        for (slot, (id, count)) in band.iter().enumerate() {
            foe.garrison[slot].id.copy_from_str(id);
            foe.garrison[slot].count = *count;
        }
        game.foes[0] = foe;
        game.foe_count = 1;
        game
    }

    #[test]
    fn attacking_a_weak_band_wins_kills_the_foe_and_banks_its_spoils() {
        let resources = foe_combat_fixture();
        let mut game = hostile_band_game(&[("ogres", 40)], &[("peasants", 2)]);
        game.stats.gold = 500;
        let mut map = Map::new(4, 1);
        map.stamp_foe(1, 0, "band0");

        let (outcome, temp_death) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);

        assert_eq!(outcome, CombatResult::Win);
        assert!(!temp_death);
        assert!(!game.foes[0].alive);
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::None
        );
        // peasants spoils_factor 1 * 5 * 2 = 10, and survivors are written back.
        assert_eq!(game.stats.gold, 510);
        assert!(game.army[0].count > 0);
        assert!(game.army[0].count <= 40);
    }

    #[test]
    fn losing_to_a_band_leaves_it_alive_and_asks_for_temporary_death() {
        let resources = foe_combat_fixture();
        let mut game = hostile_band_game(
            &[("peasants", 1)],
            &[("ogres", 40), ("ogres", 40), ("ogres", 40)],
        );
        game.stats.gold = 500;
        let mut map = Map::new(4, 1);
        map.stamp_foe(1, 0, "band0");

        let (outcome, temp_death) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);

        assert_eq!(outcome, CombatResult::Loss);
        assert!(temp_death, "the caller must handle the defeat");
        assert!(game.foes[0].alive);
        assert_eq!(game.stats.gold, 500, "no spoils on a loss");
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::Foe
        );
    }

    #[test]
    fn a_band_the_engine_cannot_find_fields_no_defenders_and_is_walked_over() {
        let resources = foe_combat_fixture();
        let mut game = hostile_band_game(&[("ogres", 10)], &[("ogres", 99)]);
        // Same placement id, different zone: find_foe scopes to the hero's
        // zone, so this band's garrison must not defend.
        game.foes[0].zone.copy_from_str("away");
        let mut map = Map::new(4, 1);

        let (outcome, _) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);

        assert_eq!(outcome, CombatResult::Win, "an empty AI side loses at once");
        assert_eq!(game.army[0].count, 10, "nothing was there to fight back");
        assert!(game.foes[0].alive, "an out-of-zone foe is not marked dead");
    }

    #[test]
    fn the_same_band_always_resolves_the_same_way() {
        let resources = foe_combat_fixture();
        let mut results = Vec::new();
        for _ in 0..4 {
            let mut game = hostile_band_game(&[("ogres", 9)], &[("peasants", 30)]);
            let mut map = Map::new(4, 1);
            map.stamp_foe(1, 0, "band0");
            let (outcome, _) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);
            results.push((outcome, game.stats.gold, game.army[0].count));
        }
        assert!(
            results.windows(2).all(|pair| pair[0] == pair[1]),
            "a foe fight must be a pure function of seed, identity and mode: {results:?}"
        );
    }

    #[test]
    fn the_band_identity_is_what_seeds_the_fight() {
        let resources = foe_combat_fixture();
        let mut game = hostile_band_game(&[("ogres", 9)], &[("peasants", 30)]);
        let mut map = Map::new(4, 1);

        // The wiring passes the placement id as the seed key, so two bands
        // with identical armies fight from different RNG streams. Assert on
        // the seed itself: whether that divergence reaches the casualty count
        // depends on how lopsided the matchup is.
        let mut a = Combat::default();
        let mut b = Combat::default();
        let garrison = game.foes[0].garrison;
        combat_seed_rng(
            &mut a,
            &game,
            CombatMode::Foe,
            &CombatTarget {
                name: "Hostile band",
                garrison: &garrison,
                seed_key: "band0",
            },
        );
        combat_seed_rng(
            &mut b,
            &game,
            CombatMode::Foe,
            &CombatTarget {
                name: "Hostile band",
                garrison: &garrison,
                seed_key: "band_other",
            },
        );
        assert_ne!(a.rng_state, b.rng_state);

        // And the constant display name never reaches the seed.
        let (outcome, _) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);
        assert_eq!(outcome, CombatResult::Win);
    }

    #[test]
    fn a_foe_that_moved_since_the_flow_queued_has_both_stamps_cleared() {
        let resources = foe_combat_fixture();
        let mut game = hostile_band_game(&[("ogres", 40)], &[("peasants", 1)]);
        let mut map = Map::new(4, 1);
        // The flow queued at (1, 0) but the band followed the hero to (2, 0).
        map.stamp_foe(1, 0, "band0");
        map.stamp_foe(2, 0, "band0");
        game.foes[0].x = 2;

        let (outcome, _) = game.attack_hostile_foe(&mut map, &resources, "band0", 1, 0);

        assert_eq!(outcome, CombatResult::Win);
        assert_eq!(
            map.tile_i32(1, 0).expect("tile exists").interactive,
            Interact::None,
            "stale stamp cleared"
        );
        assert_eq!(
            map.tile_i32(2, 0).expect("tile exists").interactive,
            Interact::None,
            "current stamp cleared"
        );
    }

    /// Hero at a monster-held castle "keep" with the given garrison.
    fn siege_game(hero: &[(&str, i32)], garrison: &[(&str, i32)]) -> Game {
        let mut game = Game::default();
        game.seed = 777;
        game.position.zone.copy_from_str("home");
        game.stats.leadership_current = 10_000;
        for (slot, (id, count)) in hero.iter().enumerate() {
            game.army[slot].id.copy_from_str(id);
            game.army[slot].count = *count;
        }
        game.castles[0].id.copy_from_str("keep");
        game.castles[0].owner_kind = CastleOwnerKind::Monsters;
        for (slot, (id, count)) in garrison.iter().enumerate() {
            game.castles[0].garrison[slot].id.copy_from_str(id);
            game.castles[0].garrison[slot].count = *count;
        }
        game
    }

    #[test]
    fn taking_a_monster_castle_flips_ownership_empties_it_and_allows_a_garrison() {
        let resources = foe_combat_fixture();
        let mut game = siege_game(&[("ogres", 40)], &[("peasants", 2)]);
        game.stats.gold = 500;

        let (outcome, temp_death) = game.siege_monster_castle(&resources, "keep");

        assert_eq!(outcome, CombatResult::Win);
        assert!(!temp_death);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Player);
        assert!(game.castles[0].visited);
        assert!(game.castles[0]
            .garrison
            .iter()
            .all(|unit| unit.id.is_empty() && unit.count == 0));
        // The siege win is the other way own_castle gets set, so the hero can
        // garrison the castle it just took without stepping off and back on.
        assert_eq!(game.position.own_castle.as_str(), "keep");
        assert_eq!(game.stats.gold, 510, "spoils banked");
    }

    #[test]
    fn a_failed_siege_leaves_the_castle_held_and_asks_for_temporary_death() {
        let resources = foe_combat_fixture();
        let mut game = siege_game(
            &[("peasants", 1)],
            &[("ogres", 40), ("ogres", 40), ("ogres", 40)],
        );
        game.stats.gold = 500;

        let (outcome, temp_death) = game.siege_monster_castle(&resources, "keep");

        assert_eq!(outcome, CombatResult::Loss);
        assert!(temp_death);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Monsters);
        assert_eq!(game.castles[0].garrison[0].count, 40, "garrison intact");
        assert!(game.position.own_castle.is_empty(), "no marker on a loss");
        assert_eq!(game.stats.gold, 500);
    }

    #[test]
    fn a_siege_win_lets_the_hero_garrison_the_castle_immediately() {
        let resources = foe_combat_fixture();
        let mut game = siege_game(&[("ogres", 40), ("peasants", 5)], &[("peasants", 1)]);

        assert_eq!(
            game.siege_monster_castle(&resources, "keep").0,
            CombatResult::Win
        );

        // The marker set by the win is exactly what the garrison core requires.
        assert_eq!(game.garrison_troop("keep", 1), GarrisonOutcome::Ok);
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
    }

    #[test]
    fn a_castle_the_engine_cannot_find_fields_no_defenders() {
        let resources = foe_combat_fixture();
        let mut game = siege_game(&[("ogres", 10)], &[("ogres", 99)]);

        let (outcome, _) = game.siege_monster_castle(&resources, "no_such_castle");

        assert_eq!(outcome, CombatResult::Win, "an empty AI side loses at once");
        assert_eq!(game.army[0].count, 10);
        // Nothing to apply the win to: the real castle is untouched.
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Monsters);
        assert!(game.position.own_castle.is_empty());
    }

    #[test]
    fn a_castle_siege_is_fought_on_the_castle_layout_not_the_open_field() {
        let garrison = [
            Unit {
                id: FixedStr::from_str("peasants"),
                count: 3,
            },
            Unit::default(),
            Unit::default(),
            Unit::default(),
            Unit::default(),
        ];
        let game = siege_game(&[("ogres", 5)], &[("peasants", 3)]);
        let mut castle = Combat::default();
        let mut field = Combat::default();
        let target = CombatTarget {
            name: "keep",
            garrison: &garrison,
            seed_key: "keep",
        };
        combat_seed_rng(&mut castle, &game, CombatMode::Castle, &target);
        combat_seed_rng(&mut field, &game, CombatMode::Foe, &target);

        // Mode is mixed into the seed, so the same defenders fight from a
        // different RNG stream depending on where they are met.
        assert_ne!(castle.rng_state, field.rng_state);
    }

    #[test]
    fn the_same_siege_always_resolves_the_same_way() {
        let resources = foe_combat_fixture();
        let mut results = Vec::new();
        for _ in 0..4 {
            let mut game = siege_game(&[("ogres", 9)], &[("peasants", 30)]);
            let (outcome, _) = game.siege_monster_castle(&resources, "keep");
            results.push((outcome, game.stats.gold, game.army[0].count));
        }
        assert!(
            results.windows(2).all(|pair| pair[0] == pair[1]),
            "a siege must be a pure function of seed, identity and mode: {results:?}"
        );
    }

    /// Troops plus two lords with starting rosters, for villain sieges.
    fn villain_siege_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":2,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"}
              ],
              "villains": [
                {"index":0,"id":"lord0","name":"Lord Zero","zone":"home","reward":1000,
                 "army":[{"troop":"peasants","count":7},{"troop":"ogres","count":3}]},
                {"index":1,"id":"lord1","name":"Lord One","zone":"home","reward":2000,
                 "army":[{"troop":"ogres","count":9}]}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    /// Hero at villain-held castle "keep", with a beatable garrison.
    fn villain_siege_game(villain_id: &str, active_contract: &str) -> Game {
        let mut game = Game::default();
        game.seed = 31;
        game.position.zone.copy_from_str("home");
        game.stats.leadership_current = 10_000;
        game.army[0].id.copy_from_str("ogres");
        game.army[0].count = 60;
        game.castles[0].id.copy_from_str("keep");
        game.castles[0].owner_kind = CastleOwnerKind::Villain;
        game.castles[0].villain_id.copy_from_str(villain_id);
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 1;
        game.contract.active_id.copy_from_str(active_contract);
        game
    }

    #[test]
    fn capturing_a_lord_under_contract_takes_the_castle_and_fulfills_it() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("lord0", "lord0");
        let gold_before = game.stats.gold;

        let (result, outcome) = game.siege_villain_castle(&resources, "keep");

        assert_eq!(result, CombatResult::Win);
        assert_eq!(outcome, VillainSiegeOutcome::Captured);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Player);
        assert!(game.castles[0].visited);
        assert!(game.castles[0].villain_id.is_empty());
        assert!(game.castles[0].garrison.iter().all(|u| u.count == 0));
        assert_eq!(game.position.own_castle.as_str(), "keep");
        // The contract pays its bounty on top of the battle's spoils.
        assert!(game.contract.villains_caught[0]);
        assert!(game.contract.active_id.is_empty());
        assert!(game.stats.gold > gold_before + 1_000);
    }

    #[test]
    fn beating_a_lord_without_the_contract_sets_him_free_to_rebuild() {
        let resources = villain_siege_fixture();
        // Holding lord1's contract while attacking lord0.
        let mut game = villain_siege_game("lord0", "lord1");

        let (result, outcome) = game.siege_villain_castle(&resources, "keep");

        assert_eq!(result, CombatResult::Win);
        assert_eq!(outcome, VillainSiegeOutcome::SetFree);
        // He keeps the castle and his identity.
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "lord0");
        assert!(game.position.own_castle.is_empty());
        // And rebuilds from his starting roster, not the army just beaten.
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
        assert_eq!(game.castles[0].garrison[0].count, 7);
        assert_eq!(game.castles[0].garrison[1].id.as_str(), "ogres");
        assert_eq!(game.castles[0].garrison[1].count, 3);
        // The defeat is remembered; the contract is untouched.
        assert!(game.contract.villains_prefought[0]);
        assert!(!game.contract.villains_caught[0]);
        assert_eq!(game.contract.active_id.as_str(), "lord1");
    }

    #[test]
    fn a_lord_with_no_contract_at_all_is_still_only_set_free() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("lord0", "");

        let (_, outcome) = game.siege_villain_castle(&resources, "keep");

        // An empty active contract can never match, even against an empty
        // villain id -- C requires both to be non-empty.
        assert_eq!(outcome, VillainSiegeOutcome::SetFree);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert!(game.contract.villains_prefought[0]);
    }

    #[test]
    fn an_unheld_castle_with_no_lord_never_matches_an_empty_contract() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("", "");

        let (_, outcome) = game.siege_villain_castle(&resources, "keep");

        assert_eq!(outcome, VillainSiegeOutcome::SetFree);
        assert!(game.position.own_castle.is_empty());
        // No villain to rebuild from, so the castle is simply emptied.
        assert!(game.castles[0].garrison.iter().all(|u| u.count == 0));
    }

    #[test]
    fn losing_a_villain_siege_changes_nothing_and_asks_for_temporary_death() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("lord0", "lord0");
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 1;
        game.castles[0].garrison[0].id.copy_from_str("ogres");
        game.castles[0].garrison[0].count = 40;
        game.castles[0].garrison[1].id.copy_from_str("ogres");
        game.castles[0].garrison[1].count = 40;

        let (result, outcome) = game.siege_villain_castle(&resources, "keep");

        assert_eq!(result, CombatResult::Loss);
        assert_eq!(outcome, VillainSiegeOutcome::Repelled);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "lord0");
        assert!(!game.contract.villains_prefought[0]);
        assert!(!game.contract.villains_caught[0]);
    }

    #[test]
    fn rank_promotion_on_capture_is_an_oracle_only_behavior() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("lord0", "lord0");
        let rank_before = game.character.cls.rank_index;

        game.siege_villain_castle(&resources, "keep");

        // Real play promotes only at an audience with the King.
        assert_eq!(game.character.cls.rank_index, rank_before);
        assert!(!game.oracle_mode, "a real session never sets this");
    }

    #[test]
    fn an_oracle_session_still_promotes_on_capture() {
        // Same shape as villain_siege_fixture, plus a class whose second rank
        // needs two villains, so the promotion is observable.
        let resources = Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":2,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"}
              ],
              "villains": [
                {"index":0,"id":"lord0","name":"Lord Zero","zone":"home","reward":1000,
                 "army":[{"troop":"peasants","count":7}]}
              ],
              "classes": [
                {
                  "index": 0,
                  "id": "knight",
                  "starting_gold": 1000,
                  "ranks": [
                    {"id":"general","name":"General","villains_needed":0,"leadership":100,"max_spells":1,"spell_power":1,"commission":10,"knows_magic":false},
                    {"id":"captain","name":"Captain","villains_needed":1,"leadership":50,"max_spells":2,"spell_power":3,"commission":20,"knows_magic":false}
                  ]
                }
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = villain_siege_game("lord0", "lord0");
        game.character.cls.id.copy_from_str("knight");
        game.character.cls.rank_index = 0;
        game.oracle_mode = true;

        let (_, outcome) = game.siege_villain_castle(&resources, "keep");

        assert_eq!(outcome, VillainSiegeOutcome::Captured);
        // The oracle branch is live: capturing the lord promoted the hero,
        // which real play would defer to an audience with the King.
        assert_eq!(game.character.cls.rank_index, 1);
    }

    #[test]
    fn a_missing_castle_reports_nothing_to_apply() {
        let resources = villain_siege_fixture();
        let mut game = villain_siege_game("lord0", "lord0");

        let (result, outcome) = game.siege_villain_castle(&resources, "no_such_castle");

        assert_eq!(result, CombatResult::Win, "an empty AI side loses at once");
        assert_eq!(outcome, VillainSiegeOutcome::NoCastle);
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
    }

    /// An all-grass board with the hero at (hx, hy) having come from
    /// (last_x, last_y), and one live foe at (fx, fy).
    fn follow_game(hero: (i32, i32), last: (i32, i32), foe_at: (i32, i32)) -> (Game, Map) {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = hero.0;
        game.position.y = hero.1;
        game.position.last_x = last.0;
        game.position.last_y = last.1;
        let mut foe = FoeState::default();
        foe.zone = game.position.zone;
        foe.x = foe_at.0;
        foe.y = foe_at.1;
        foe.placement_id.copy_from_str("band0");
        foe.alive = true;
        game.foes[0] = foe;
        game.foe_count = 1;
        let map = Map::new(8, 8);
        (game, map)
    }

    #[test]
    fn a_foe_steps_toward_the_tile_the_hero_just_left() {
        // Hero moved from (4,4) to (5,4); the foe chases the PREVIOUS tile.
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (2, 4));

        assert_eq!(game.foes_follow(&mut map), None);

        assert_eq!((game.foes[0].x, game.foes[0].y), (3, 4));
        assert_eq!(
            map.tile_i32(3, 4).expect("tile exists").interactive,
            Interact::Foe
        );
        assert_eq!(
            map.tile_i32(3, 4).expect("tile exists").id.as_str(),
            "band0"
        );
        assert_eq!(
            map.tile_i32(2, 4).expect("tile exists").interactive,
            Interact::None,
            "old stamp cleared"
        );
    }

    #[test]
    fn a_foe_out_of_follow_range_stays_put() {
        // Range is 2 per axis, measured from the hero's previous tile.
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (7, 4));
        assert_eq!(game.foes_follow(&mut map), None);
        assert_eq!((game.foes[0].x, game.foes[0].y), (7, 4));

        // At exactly range 2 it gives chase. Here the closest cell to the
        // hero's previous tile IS the hero's current tile, so the chase ends
        // in a collision rather than a plain step.
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (6, 4));
        assert_eq!(game.foes_follow(&mut map), Some(0));
        assert_eq!((game.foes[0].x, game.foes[0].y), (5, 4));
    }

    #[test]
    fn stepping_onto_the_hero_is_reported_as_a_collision_and_left_unstamped() {
        // Foe adjacent to the hero, who has not moved.
        let (mut game, mut map) = follow_game((4, 4), (4, 4), (5, 4));

        assert_eq!(game.foes_follow(&mut map), Some(0));

        assert_eq!((game.foes[0].x, game.foes[0].y), (4, 4));
        assert_eq!(
            map.tile_i32(4, 4).expect("tile exists").interactive,
            Interact::None,
            "the hero is standing there, so the foe is not stamped"
        );
    }

    #[test]
    fn desert_is_a_refuge_because_foes_will_not_enter_it() {
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (2, 4));
        // Wall the approach with desert: walkable for the hero, not for foes.
        for y in 0..8 {
            map.tile_mut_i32(3, y).expect("tile exists").terrain = Terrain::Desert;
        }

        assert_eq!(game.foes_follow(&mut map), None);

        assert_eq!(
            (game.foes[0].x, game.foes[0].y),
            (2, 4),
            "no standable cell beats standing still"
        );
    }

    #[test]
    fn a_bridge_is_not_foe_standable_either() {
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (2, 4));
        for y in 0..8 {
            map.tile_mut_i32(3, y).expect("tile exists").is_bridge = true;
        }

        assert_eq!(game.foes_follow(&mut map), None);
        assert_eq!((game.foes[0].x, game.foes[0].y), (2, 4));
    }

    #[test]
    fn foes_keep_off_the_tiles_around_a_castle_gate() {
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (2, 4));
        // A gate at (3,3) puts the eight tiles around it off limits -- the
        // house rule that stops a foe parking on the doorstep of the hero's
        // own castle. (3,4) is the closest cell to the hero's previous tile
        // and is one of those eight, so the foe must settle for (3,5), which
        // is two rows from the gate and therefore free.
        map.tile_mut_i32(3, 3).expect("tile exists").interactive = Interact::CastleGate;

        assert_eq!(game.foes_follow(&mut map), None);

        assert_ne!((game.foes[0].x, game.foes[0].y), (3, 4), "on the doorstep");
        assert_eq!((game.foes[0].x, game.foes[0].y), (3, 5));
    }

    #[test]
    fn two_foes_never_stack_on_the_same_tile() {
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (2, 4));
        let mut other = game.foes[0];
        other.placement_id.copy_from_str("band1");
        other.x = 2;
        other.y = 5;
        game.foes[1] = other;
        game.foe_count = 2;

        game.foes_follow(&mut map);

        assert_ne!(
            (game.foes[0].x, game.foes[0].y),
            (game.foes[1].x, game.foes[1].y)
        );
    }

    #[test]
    fn a_flying_hero_cannot_be_reached_unless_the_oracle_is_driving() {
        let (mut game, mut map) = follow_game((4, 4), (4, 4), (5, 4));
        game.character.mount = Mount::Fly;

        assert_eq!(
            game.foes_follow(&mut map),
            None,
            "no collision while flying"
        );
        assert_ne!((game.foes[0].x, game.foes[0].y), (4, 4));

        // The oracle session keeps the legacy reachable-hero behavior.
        let (mut game, mut map) = follow_game((4, 4), (4, 4), (5, 4));
        game.character.mount = Mount::Fly;
        game.oracle_mode = true;
        assert_eq!(game.foes_follow(&mut map), Some(0));
    }

    #[test]
    fn only_live_foes_in_the_hero_zone_follow() {
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (3, 4));
        game.foes[0].alive = false;
        assert_eq!(game.foes_follow(&mut map), None);
        assert_eq!((game.foes[0].x, game.foes[0].y), (3, 4));

        let (mut game, mut map) = follow_game((5, 4), (4, 4), (3, 4));
        game.foes[0].zone.copy_from_str("away");
        assert_eq!(game.foes_follow(&mut map), None);
        assert_eq!((game.foes[0].x, game.foes[0].y), (3, 4));
    }

    #[test]
    fn the_resync_restamps_a_foe_the_hero_has_walked_away_from() {
        // The foe is on the tile the hero just left, unstamped from the
        // collision that put it there. Nothing else re-stamps it.
        let (mut game, mut map) = follow_game((5, 4), (4, 4), (4, 4));
        assert_eq!(
            map.tile_i32(4, 4).expect("tile exists").interactive,
            Interact::None
        );

        game.foes_follow(&mut map);

        // Wherever it ended up, it carries a stamp again rather than
        // lingering as a phantom the occupancy check cannot see.
        let (fx, fy) = (game.foes[0].x, game.foes[0].y);
        assert_eq!(
            map.tile_i32(fx, fy).expect("tile exists").interactive,
            Interact::Foe
        );
    }

    #[test]
    fn weekly_astrology_applies_economy_repopulation_and_matching_enemy_growth() {
        let resources = weekly_fixture();
        let mut game = Game::default();
        game.stats.days_left = 5;
        game.stats.gold = 50;
        game.stats.commission_weekly = 100;
        game.stats.leadership_base = 80;
        game.stats.leadership_current = 125;
        game.boat.has_boat = true;
        game.boat.x = 4;
        game.boat.y = 5;
        game.army[0].id.copy_from_str("ghosts");
        game.army[0].count = 2;
        game.dwellings[0].troop_id.copy_from_str("peasants");
        game.dwellings[0].count = 2;
        game.dwellings[0].max_population = 10;
        game.dwellings[1].troop_id.copy_from_str("ghosts");
        game.dwellings[1].count = 2;
        game.dwellings[1].max_population = 8;
        game.dwelling_count = 2;
        game.castles[0].id.copy_from_str("monster_castle");
        game.castles[0].owner_kind = CastleOwnerKind::Monsters;
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 1;
        game.castles[1].id.copy_from_str("player_castle");
        game.castles[1].owner_kind = CastleOwnerKind::Player;
        game.castles[1].garrison[0].id.copy_from_str("peasants");
        game.castles[1].garrison[0].count = 1;
        game.foes[0] = FoeState::default();
        game.foes[0].alive = true;
        game.foes[0].garrison[0].id.copy_from_str("peasants");
        game.foes[0].garrison[0].count = 2;
        game.foes[1] = game.foes[0];
        game.foes[1].alive = false;
        game.foe_count = 2;

        assert_eq!(game.week_id(&resources), 4);
        assert_eq!(game.pick_astrology_creature(&resources, 1), 1);
        assert_eq!(game.apply_week_end(&resources), 100);
        assert_eq!(game.stats.gold, 96);
        assert_eq!(game.stats.last_commission, 100);
        assert_eq!(game.stats.last_astrology_troop, 0);
        assert_eq!(game.stats.leadership_current, 80);
        assert!(game.boat.has_boat);
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 2);
        assert_eq!(game.dwellings[0].count, 10);
        assert_eq!(game.dwellings[1].count, 2);
        assert_eq!(game.castles[0].garrison[0].count, 4);
        assert_eq!(game.castles[1].garrison[0].count, 1);
        assert_eq!(game.foes[0].garrison[0].count, 5);
        assert_eq!(game.foes[1].garrison[0].count, 2);
    }

    #[test]
    fn dwelling_growth_helper_is_separate_from_astrology_repopulation() {
        let resources = weekly_fixture();
        let mut game = Game::default();
        game.dwellings[0].troop_id.copy_from_str("peasants");
        game.dwellings[0].count = 8;
        game.dwellings[0].max_population = 10;
        game.dwellings[1].troop_id.copy_from_str("ghosts");
        game.dwellings[1].count = 2;
        game.dwellings[1].max_population = 8;
        game.dwelling_count = 2;

        game.grow_dwellings(&resources);

        assert_eq!(game.dwellings[0].count, 10);
        assert_eq!(game.dwellings[1].count, 6);
    }

    #[test]
    fn weekly_boat_repossession_preserves_its_zone_like_openbounty() {
        let resources = weekly_fixture();
        let mut game = Game::default();
        game.stats.days_left = 5;
        game.stats.gold = 0;
        game.boat.has_boat = true;
        game.boat.zone.copy_from_str("home");
        game.boat.x = 4;
        game.boat.y = 5;

        game.apply_week_end(&resources);

        assert!(!game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (-1, -1));
        assert_eq!(game.boat.zone.as_str(), "home");
    }

    fn boat_town_game() -> Game {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.in_town.copy_from_str("town0");
        game
    }

    #[test]
    fn rent_boat_deducts_cost_and_records_dock_tile() {
        let resources = weekly_fixture();
        let mut game = boat_town_game();
        let cost = game.boat_cost(&resources);
        game.stats.gold = cost + 1;

        assert_eq!(
            game.rent_boat(&resources, 7, 9, "continentia"),
            BoatActionOutcome::RentOk
        );
        assert_eq!(game.stats.gold, 1);
        assert!(game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (7, 9));
        assert_eq!(game.boat.zone.as_str(), "continentia");
    }

    #[test]
    fn rent_boat_refuses_when_gold_equals_cost_like_openbounty() {
        let resources = weekly_fixture();
        let mut game = boat_town_game();
        let cost = game.boat_cost(&resources);
        game.stats.gold = cost;

        assert_eq!(
            game.rent_boat(&resources, 1, 2, "continentia"),
            BoatActionOutcome::RentNoGold
        );
        assert_eq!(game.stats.gold, cost);
        assert_eq!(game.boat, BoatState::default());
    }

    #[test]
    fn rent_boat_requires_the_hero_to_be_at_a_town() {
        let resources = weekly_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.stats.gold = 10_000;

        assert_eq!(
            game.rent_boat(&resources, 1, 2, "continentia"),
            BoatActionOutcome::RentNoGold
        );
        assert_eq!(game.stats.gold, 10_000);
        assert_eq!(game.boat, BoatState::default());
    }

    #[test]
    fn rent_boat_charges_the_cheap_price_with_the_cheaper_boats_artifact() {
        let resources = Resources::from_json_str(
            r#"{
              "economy": {"boat_cost_normal":500,"boat_cost_cheap":100},
              "artifacts": [{"index":0,"id":"anchor","power":"cheaper_boats"}],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = boat_town_game();
        game.stats.gold = 501;
        assert!(game.claim_artifact_by_id(&resources, "anchor"));
        assert_eq!(game.boat_cost(&resources), 100);

        assert_eq!(
            game.rent_boat(&resources, 3, 4, "home"),
            BoatActionOutcome::RentOk
        );
        assert_eq!(game.stats.gold, 401);
    }

    #[test]
    fn cancel_boat_is_refused_while_sailing() {
        let resources = weekly_fixture();
        let mut game = boat_town_game();
        game.stats.gold = game.boat_cost(&resources) + 1;
        assert_eq!(
            game.rent_boat(&resources, 3, 4, "home"),
            BoatActionOutcome::RentOk
        );
        game.travel_mode = TravelMode::Boat;

        assert_eq!(game.cancel_boat(), BoatActionOutcome::CancelAtSea);
        assert!(game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (3, 4));
    }

    #[test]
    fn cancel_boat_clears_state_when_ashore() {
        let resources = weekly_fixture();
        let mut game = boat_town_game();
        game.stats.gold = game.boat_cost(&resources) + 1;
        assert_eq!(
            game.rent_boat(&resources, 3, 4, "home"),
            BoatActionOutcome::RentOk
        );
        game.travel_mode = TravelMode::Walk;

        assert_eq!(game.cancel_boat(), BoatActionOutcome::CancelOk);
        assert_eq!(game.boat, BoatState::default());
        assert_eq!(game.stats.gold, 1);
    }

    #[test]
    fn cancel_boat_out_of_town_reports_ok_without_touching_state() {
        let resources = weekly_fixture();
        let mut game = boat_town_game();
        game.stats.gold = game.boat_cost(&resources) + 1;
        assert_eq!(
            game.rent_boat(&resources, 3, 4, "home"),
            BoatActionOutcome::RentOk
        );
        game.position.in_town.copy_from_str("");

        assert_eq!(game.cancel_boat(), BoatActionOutcome::CancelOk);
        assert!(game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (3, 4));
    }

    fn siege_fixture() -> Resources {
        Resources::from_json_str(r#"{"economy": {"siege_cost":3000}, "troops": []}"#)
            .expect("fixture parses")
    }

    #[test]
    fn siege_cost_defaults_to_openbounty_value_when_unset() {
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        assert_eq!(resources.economy.siege_cost, 3_000);
    }

    #[test]
    fn buy_siege_deducts_cost_and_sets_the_flag() {
        let resources = siege_fixture();
        let mut game = boat_town_game();
        game.stats.gold = 3_001;

        assert_eq!(game.buy_siege(&resources), SiegeBuyOutcome::Ok);
        assert_eq!(game.stats.gold, 1);
        assert_eq!(game.stats.siege_weapons, 1);
    }

    #[test]
    fn buy_siege_refuses_when_gold_equals_cost_like_openbounty() {
        let resources = siege_fixture();
        let mut game = boat_town_game();
        game.stats.gold = 3_000;

        assert_eq!(game.buy_siege(&resources), SiegeBuyOutcome::NoGold);
        assert_eq!(game.stats.gold, 3_000);
        assert_eq!(game.stats.siege_weapons, 0);
    }

    #[test]
    fn buy_siege_requires_the_hero_to_be_at_a_town() {
        let resources = siege_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.stats.gold = 10_000;

        assert_eq!(game.buy_siege(&resources), SiegeBuyOutcome::NoGold);
        assert_eq!(game.stats.gold, 10_000);
        assert_eq!(game.stats.siege_weapons, 0);
    }

    #[test]
    fn buy_siege_reports_already_before_checking_gold() {
        let resources = siege_fixture();
        let mut game = boat_town_game();
        game.stats.siege_weapons = 1;
        game.stats.gold = 5;

        // OpenBounty tests ownership ahead of affordability, so a broke owner
        // still gets Already rather than NoGold.
        assert_eq!(game.buy_siege(&resources), SiegeBuyOutcome::Already);
        assert_eq!(game.stats.gold, 5);
        assert_eq!(game.stats.siege_weapons, 1);
    }

    #[test]
    fn buy_siege_out_of_town_outranks_the_already_owned_check() {
        let resources = siege_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.stats.siege_weapons = 1;
        game.stats.gold = 10_000;

        // The in_town guard comes first in C, so an owner off-map sees NoGold.
        assert_eq!(game.buy_siege(&resources), SiegeBuyOutcome::NoGold);
    }

    fn spell_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "spells": [
                {"index":0,"id":"bridge","kind":"adventure","cost":100},
                {"index":1,"id":"fireball","kind":"combat","cost":200}
              ],
              "troops": []
            }"#,
        )
        .expect("fixture parses")
    }

    /// Hero standing in `town0`, which has `bridge` (cost 100) on sale.
    fn spell_town_game() -> Game {
        let mut game = boat_town_game();
        game.towns[0].id.copy_from_str("town0");
        game.towns[0].spell_for_sale.copy_from_str("bridge");
        game.stats.max_spells = 5;
        game
    }

    #[test]
    fn known_spells_sums_charges_not_distinct_spells() {
        let mut game = Game::default();
        game.spells.counts[0] = 3;
        game.spells.counts[1] = 2;

        // The header calls these "distinct spells", but GameKnownSpells sums
        // counts[]; the executable behavior wins.
        assert_eq!(game.known_spells(), 5);
    }

    #[test]
    fn buy_spell_deducts_cost_and_adds_a_charge() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 101;

        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::Ok);
        assert_eq!(game.stats.gold, 1);
        assert_eq!(game.spells.counts[0], 1);
    }

    #[test]
    fn buy_spell_refuses_when_gold_equals_cost_like_openbounty() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 100;

        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::NoGold);
        assert_eq!(game.stats.gold, 100);
        assert_eq!(game.spells.counts[0], 0);
    }

    #[test]
    fn buy_spell_requires_the_hero_to_be_in_the_named_town() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 10_000;
        game.towns[1].id.copy_from_str("town1");
        game.towns[1].spell_for_sale.copy_from_str("fireball");

        // Standing in town0, naming town1: rejected even though town1 sells a
        // spell the hero could afford.
        assert_eq!(
            game.buy_spell(&resources, "town1"),
            SpellBuyOutcome::NoSpell
        );
        assert_eq!(game.stats.gold, 10_000);

        game.position.in_town.copy_from_str("");
        assert_eq!(
            game.buy_spell(&resources, "town0"),
            SpellBuyOutcome::NoSpell
        );
        assert_eq!(game.stats.gold, 10_000);
    }

    #[test]
    fn buy_spell_reports_no_spell_for_an_empty_or_unknown_stock() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 10_000;

        game.towns[0].spell_for_sale.copy_from_str("");
        assert_eq!(
            game.buy_spell(&resources, "town0"),
            SpellBuyOutcome::NoSpell
        );

        game.towns[0].spell_for_sale.copy_from_str("not_in_catalog");
        assert_eq!(
            game.buy_spell(&resources, "town0"),
            SpellBuyOutcome::NoSpell
        );
        assert_eq!(game.stats.gold, 10_000);
    }

    #[test]
    fn buy_spell_is_capped_by_max_spells_before_checking_gold() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 10_000;
        game.stats.max_spells = 2;
        game.spells.counts[1] = 2;

        // At the cap with gold to spare, and still AtCap when broke: the cap
        // is tested ahead of affordability.
        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::AtCap);
        game.stats.gold = 0;
        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::AtCap);
        assert_eq!(game.spells.counts[0], 0);
    }

    #[test]
    fn buy_spell_stacks_repeat_charges_of_the_same_spell() {
        let resources = spell_fixture();
        let mut game = spell_town_game();
        game.stats.gold = 1_000;

        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::Ok);
        assert_eq!(game.buy_spell(&resources, "town0"), SpellBuyOutcome::Ok);

        assert_eq!(game.spells.counts[0], 2);
        assert_eq!(game.known_spells(), 2);
        assert_eq!(game.stats.gold, 800);
    }

    #[test]
    fn navmap_unlocks_target_or_first_undiscovered_zone() {
        let resources = Resources::from_json_str(
            r#"{
              "zones": [{"id":"home"},{"id":"away"},{"id":"far"}],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.world.zones_discovered[0] = true;

        assert_eq!(game.resolve_navmap(&resources, "navmap_0"), Some(1));
        assert!(game.world.zones_discovered[1]);
        assert_eq!(game.resolve_navmap(&resources, "navmap_1"), Some(2));
        assert!(game.world.zones_discovered[2]);
        assert_eq!(game.resolve_navmap(&resources, "navmap_99"), None);
    }

    #[test]
    fn orb_marks_current_zone_and_telecave_pairs_by_same_zone_sequence() {
        let resources = Resources::from_json_str(
            r#"{
              "zones": [{"id":"home"},{"id":"away"}],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 1;
        game.position.y = 0;
        game.add_placement("home", 1, 0, Interact::Telecave, "telecave_0");
        game.add_placement("home", 5, 0, Interact::Telecave, "telecave_1");
        game.add_placement("away", 7, 0, Interact::Telecave, "telecave_0");

        assert_eq!(game.activate_orb(&resources), Some(0));
        assert!(game.world.orbs_found[0]);
        assert!(!game.world.orbs_found[1]);

        assert_eq!(game.resolve_telecave(), Some((5, 0)));
        assert_eq!((game.position.x, game.position.y), (5, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (5, 0));
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
        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "villain0");
        assert!(game.castles[0]
            .garrison
            .iter()
            .all(|slot| slot.id.is_empty() && slot.count == 0));
        assert!(!game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (-1, -1));
    }

    #[test]
    fn init_records_scepter_zone_marker_when_rolled_zone_exists() {
        let resources = Resources::from_json_str(
            r#"{
              "world": {"starting_zone": "home"},
              "zones": [{"id":"home","is_home":true}],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();

        game.init_seeded_with_resources(&resources, Some("hero"), 0, 0, 0);

        assert_eq!(game.scepter.continent, 0);
        assert_eq!(game.scepter.zone.as_str(), "home");
        assert_eq!((game.scepter.x, game.scepter.y), (-1, -1));
    }

    #[test]
    fn init_with_resource_file_loads_zone_map_and_buries_scepter() {
        let root = std::env::temp_dir().join(format!(
            "openbounty-engine-map-fixture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock is after epoch")
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("maps")).expect("fixture directory is created");
        std::fs::write(root.join("maps/home.dat"), b".F..\n....\n")
            .expect("fixture map is written");
        std::fs::write(
            root.join("game.json"),
            r#"{
              "world": {"starting_zone": "home"},
              "tile_codes": {
                ".": {"art":"grass","terrain":"grass"},
                "F": {"art":"forest","terrain":"forest"}
              },
              "zones": [{
                "id":"home",
                "is_home":true,
                "map":"maps/home.dat",
                "width":4,
                "height":2,
                "chests":[{"x":0,"y":1,"id":"chest0"}],
                "dwellings":[{"x":1,"y":1,"id":"dw0","kind":"hills"}],
                "magic_alcove":{"x":2,"y":1}
              }],
              "towns": [{"index":0,"id":"town0","zone":"home","x":2,"y":0}],
              "troops": []
            }"#,
        )
        .expect("fixture game.json is written");
        let resources = Resources::from_json_file(root.join("game.json")).expect("fixture parses");
        let mut game = Game::default();

        game.init_seeded_with_resources(&resources, Some("Hero"), 0, 0, 0);

        assert_eq!(game.scepter.continent, 0);
        assert_eq!(game.scepter.zone.as_str(), "home");
        assert_eq!((game.scepter.x, game.scepter.y), (3, 0));
        assert_eq!(game.rng_snapshot(), 0xacdd_9880_3ce6_d167);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn bury_scepter_in_fixture_map_uses_row_major_plain_grass_target() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 0);
        assert_eq!(game.rng_snapshot(), 0x413f_5b10_e08e_6d4c);

        let map = Map::from_tiles(
            4,
            2,
            vec![
                Tile::plain_grass(),
                Tile::terrain(Terrain::Forest),
                Tile::blocked_grass(),
                Tile::plain_grass(),
                Tile::interactive(Interact::TreasureChest),
                Tile::terrain(Terrain::Water),
                Tile::plain_grass(),
                Tile::terrain(Terrain::Desert),
            ],
        )
        .expect("fixture dimensions match");

        game.bury_scepter_in_map("home", &map);

        assert_eq!(game.scepter.zone.as_str(), "home");
        assert_eq!((game.scepter.x, game.scepter.y), (3, 0));
        assert_eq!(game.rng_snapshot(), 0xacdd_9880_3ce6_d167);
    }

    #[test]
    fn bury_scepter_in_empty_fixture_map_does_not_advance_rng() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 0);
        let rng_before = game.rng_snapshot();
        let map = Map::from_tiles(
            2,
            2,
            vec![
                Tile::terrain(Terrain::Forest),
                Tile::blocked_grass(),
                Tile::interactive(Interact::Town),
                Tile::terrain(Terrain::Desert),
            ],
        )
        .expect("fixture dimensions match");

        game.bury_scepter_in_map("home", &map);

        assert_eq!(game.scepter.zone.as_str(), "home");
        assert_eq!((game.scepter.x, game.scepter.y), (-1, -1));
        assert_eq!(game.rng_snapshot(), rng_before);
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
    fn accept_alcove_pays_and_grants_magic_or_refuses_without_gold() {
        let resources =
            Resources::from_json_str(r#"{"economy": {"alcove_cost": 500}, "troops": []}"#)
                .expect("fixture parses");
        let mut map = Map::from_tiles(
            2,
            1,
            vec![Tile::interactive(Interact::Alcove), Tile::plain_grass()],
        )
        .expect("fixture dimensions match");

        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 0;
        game.position.y = 0;

        // Too poor: nothing changes, and the tile is not consumed.
        game.stats.gold = 0;
        assert!(!game.accept_alcove(&mut map, &resources));
        assert!(!game.stats.knows_magic);
        assert_eq!(game.stats.gold, 0);
        assert_eq!(
            map.tile(0, 0).map(|t| t.interactive),
            Some(Interact::Alcove)
        );

        game.stats.gold = 1_000;
        assert!(game.accept_alcove(&mut map, &resources));
        assert!(game.stats.knows_magic);
        assert_eq!(game.stats.gold, 500);
        assert!(game.is_consumed("home", 0, 0));
        assert_eq!(map.tile(0, 0).map(|t| t.interactive), Some(Interact::None));
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
                  "dwellings":[{"x":7,"y":8,"id":"dw0","kind":"hills"}],
                  "salt":{"artifacts":1,"navmaps":1,"orbs":1,"telecaves":1,"dwellings":1,"friendly_foes":1,"preferred_troops":["sprites"]}
                }
              ],
              "troops": [
                {"index":0,"id":"sprites","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"forest","max_population":50,"growth_per_week":10},
                {"index":1,"id":"dwarves","skill_level":3,"hit_points":20,"move_rate":1,"melee":[2,4],"ranged":[0,0,0],"recruit_cost":350,"spoils_factor":35,"abilities":"","morale_group":"C","dwelling":"hill","max_population":12,"growth_per_week":4},
                {"index":2,"id":"ogres","skill_level":4,"hit_points":40,"move_rate":1,"melee":[3,5],"ranged":[0,0,0],"recruit_cost":750,"spoils_factor":75,"abilities":"","morale_group":"D","dwelling":"hill","max_population":8,"growth_per_week":2}
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
        assert_eq!(game.dwelling_count, 2);
        assert!(game
            .dwellings
            .iter()
            .take(game.dwelling_count)
            .any(|dwelling| {
                dwelling.troop_id.as_str() == "sprites"
                    && dwelling.count == 50
                    && dwelling.max_population == 50
            }));
        assert!(game
            .dwellings
            .iter()
            .take(game.dwelling_count)
            .any(|dwelling| {
                (dwelling.x, dwelling.y) == (7, 8)
                    && dwelling.troop_id.as_str() == "ogres"
                    && dwelling.count == 8
                    && dwelling.max_population == 8
            }));
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
    fn init_places_declared_villain_army_in_matching_castle() {
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

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Villain);
        assert_eq!(game.castles[0].villain_id.as_str(), "villain0");
        assert_eq!(game.castles[0].garrison[0].id.as_str(), "peasants");
        assert_eq!(game.castles[0].garrison[0].count, 40);
    }

    #[test]
    fn init_marks_contract_excluded_castles_special_without_villain() {
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

        assert_eq!(game.castles[0].owner_kind, CastleOwnerKind::Special);
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

    fn friendly_foe_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "zones": [{"id":"home"}],
              "spawn": {
                "tier_chance_curve": [[100,100,100],[100,100,100],[100,100,100],[100,100,100]],
                "tier_troop_pool": [
                  ["sprites","sprites","sprites","sprites"],
                  ["sprites","sprites","sprites","sprites"],
                  ["sprites","sprites","sprites","sprites"],
                  ["sprites","sprites","sprites","sprites"]
                ]
              },
              "troops": [
                {"index":0,"id":"sprites","skill_level":1,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":5,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[7,8,9,10]}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    fn weekly_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "time": {"week_days":5,"days_per_difficulty":{"easy":25}},
              "economy": {"boat_cost_normal":50,"boat_cost_cheap":10},
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","growth_per_week":3},
                {"index":1,"id":"ghosts","skill_level":2,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":20,"spoils_factor":1,"abilities":"ABSORB","morale_group":"A","growth_per_week":4},
                {"index":2,"id":"orcs","skill_level":3,"hit_points":3,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":30,"spoils_factor":1,"abilities":"","morale_group":"A","growth_per_week":5},
                {"index":3,"id":"ogres","skill_level":4,"hit_points":4,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":1,"abilities":"","morale_group":"A","growth_per_week":6}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    fn friendly_foe_game() -> Game {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        let mut foe = FoeState::default();
        foe.zone = game.position.zone;
        foe.x = 1;
        foe.y = 0;
        foe.origin_x = 1;
        foe.origin_y = 0;
        foe.placement_id.copy_from_str("friendly0");
        foe.alive = true;
        foe.friendly = true;
        game.foes[0] = foe;
        game.foe_count = 1;
        game
    }

    #[test]
    fn a_puzzle_cell_uncovers_when_its_villain_is_caught() {
        let mut game = Game::default();
        // (2,2) is villain 16, the grid's centre.
        assert!(!game.puzzle_cell_revealed(2, 2));
        game.contract.villains_caught[16] = true;
        assert!(game.puzzle_cell_revealed(2, 2));
        // Its neighbours are unaffected.
        assert!(!game.puzzle_cell_revealed(2, 1));
    }

    #[test]
    fn a_puzzle_cell_uncovers_when_its_artifact_is_found() {
        let mut game = Game::default();
        // (0,0) holds -1, which encodes artifact index 0.
        assert!(!game.puzzle_cell_revealed(0, 0));
        game.artifacts.found[0] = true;
        assert!(game.puzzle_cell_revealed(0, 0));
        // (0,2) holds -2, artifact 1, still covered.
        assert!(!game.puzzle_cell_revealed(0, 2));
    }

    #[test]
    fn an_out_of_range_puzzle_cell_follows_villain_zero() {
        let mut game = Game::default();
        assert!(!game.puzzle_cell_revealed(9, 9));
        // The out-of-range entity is 0, so catching villain 0 makes even a
        // nonsense cell report revealed. C has the same hole.
        game.contract.villains_caught[0] = true;
        assert!(game.puzzle_cell_revealed(9, 9));
    }

    #[test]
    fn every_catch_and_find_uncovers_exactly_one_cell() {
        let mut game = Game::default();
        assert_eq!(game.puzzle_cells_revealed(), 0);

        for villain in 0..17 {
            game.contract.villains_caught[villain] = true;
        }
        assert_eq!(game.puzzle_cells_revealed(), 17);

        for artifact in 0..8 {
            game.artifacts.found[artifact] = true;
        }
        assert_eq!(game.puzzle_cells_revealed(), 25);
    }

    #[test]
    fn a_villain_beyond_the_grid_uncovers_nothing() {
        let mut game = Game::default();
        // The grid names villains 0..16; the catalogue holds more slots.
        game.contract.villains_caught[20] = true;
        assert_eq!(game.puzzle_cells_revealed(), 0);
    }

    fn economy_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "economy": {"boat_cost_normal": 500, "boat_cost_cheap": 100},
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    #[test]
    fn weekly_net_gold_judges_the_boat_fare_on_the_post_credit_wallet() {
        let resources = economy_fixture();
        let mut game = Game::default();
        game.stats.commission_weekly = 600;
        game.boat.has_boat = true;
        game.stats.gold = 0;

        // Broke right now, but the 600 commission lands before the 500 fare
        // is judged, so the fare IS charged: 600 - 500 = 100.
        assert_eq!(game.weekly_net_gold(&resources), 100);
    }

    #[test]
    fn a_fare_the_post_credit_wallet_cannot_meet_is_not_charged() {
        let resources = economy_fixture();
        let mut game = Game::default();
        game.stats.commission_weekly = 400;
        game.boat.has_boat = true;
        game.stats.gold = 0;

        // 0 + 400 < 500, so the boat is repossessed rather than charged for
        // and the week nets the full commission.
        assert_eq!(game.weekly_net_gold(&resources), 400);
    }

    #[test]
    fn weekly_outgoings_bills_the_fare_whatever_the_wallet_holds() {
        let resources = economy_fixture();
        let mut game = Game::default();
        game.stats.commission_weekly = 400;
        game.boat.has_boat = true;
        game.stats.gold = 0;

        // The gross bill counts the fare unconditionally, which is exactly
        // where it parts company with weekly_net_gold above.
        assert_eq!(game.weekly_outgoings(&resources), 500);
        game.boat.has_boat = false;
        assert_eq!(game.weekly_outgoings(&resources), 0);
    }

    #[test]
    fn the_weekly_figures_both_count_army_upkeep() {
        let resources = economy_fixture();
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 20;
        game.stats.commission_weekly = 1_000;
        let upkeep = game.army_weekly_upkeep(&resources);
        assert!(upkeep > 0, "the fixture troop must cost something to keep");

        assert_eq!(game.weekly_net_gold(&resources), 1_000 - upkeep);
        assert_eq!(game.weekly_outgoings(&resources), upkeep);
    }

    /// A home zone whose `home_spawn` differs from its `hero_spawn`, so a
    /// temp death can be told apart from a plain zone switch.
    fn temp_death_fixture(tag: &str, temp_death: &str) -> Resources {
        let dir = std::env::temp_dir();
        let home = dir.join(format!("openbounty-{}-{}.dat", tag, std::process::id()));
        std::fs::write(&home, b"......\n......\n").expect("home map writes");
        Resources::from_json_str(&format!(
            r##"{{
              "world": {{"fog_sight": 3}},
              "tile_codes": {{".": {{"art":"grass","terrain":"grass"}}}},
              "zones": [
                {{"id":"home","map":"{home}","width":6,"height":2,"is_home":true,
                 "hero_spawn":{{"x":1,"y":0}},"home_spawn":{{"x":4,"y":1}}}}
              ],
              "troops": [
                {{"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}},
                {{"index":1,"id":"militia","skill_level":2,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":5,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}}
              ]{temp_death}
            }}"##,
            home = home.display(),
            temp_death = temp_death,
        ))
        .expect("fixture parses")
    }

    #[test]
    fn a_temp_death_forfeits_everything_and_sends_the_hero_to_the_home_spawn() {
        let resources = temp_death_fixture("tempdeath", "");
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 50;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 30;
        game.stats.siege_weapons = 3;
        game.boat.has_boat = true;
        game.boat.x = 2;
        game.boat.y = 1;
        game.boat.zone.copy_from_str("home");
        game.character.mount = Mount::Fly;
        game.travel_mode = TravelMode::Boat;

        game.temp_death(&resources, &mut map, &mut fog);

        // The default consolation troop is the CHEAPEST by recruit cost,
        // which is militia at 5, not the catalog's first entry.
        assert_eq!(game.army[0].id.as_str(), "militia");
        assert_eq!(game.army[0].count, 20);
        assert!(game.army[1].id.is_empty());
        assert_eq!(game.stats.siege_weapons, 0);
        // The boat is forfeit outright, not carried to the home spawn.
        assert!(!game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (-1, -1));
        assert!(game.boat.zone.is_empty());
        // home_spawn (4,1), NOT the zone's hero_spawn (1,0).
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!((game.position.x, game.position.y), (4, 1));
        assert_eq!((game.position.last_x, game.position.last_y), (4, 1));
        assert_eq!(game.character.mount, Mount::Ride);
        assert_eq!(game.travel_mode, TravelMode::Walk);
    }

    #[test]
    fn a_configured_consolation_troop_wins_over_the_cheapest() {
        let resources = temp_death_fixture(
            "tempdeath-cfg",
            r#","tuning":{"temp_death":{"troop":"peasants","count":7}}"#,
        );
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        let mut game = Game::default();

        game.temp_death(&resources, &mut map, &mut fog);
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 7);
    }

    #[test]
    fn a_temp_death_troop_the_catalogue_lacks_is_refused_at_load() {
        // C fails the whole pack load rather than silently dropping the
        // knob, so this is a parse error, not a runtime fallback.
        let err = Resources::from_json_str(
            r#"{
              "tuning": {"temp_death": {"troop": "dragons", "count": 5}},
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}
              ]
            }"#,
        );
        assert!(matches!(
            err,
            Err(ResourceError::InvalidField("tuning.temp_death"))
        ));
    }

    #[test]
    fn a_non_positive_temp_death_count_is_refused_at_load() {
        let err =
            Resources::from_json_str(r#"{"tuning": {"temp_death": {"count": 0}}, "troops": []}"#);
        assert!(matches!(
            err,
            Err(ResourceError::InvalidField("tuning.temp_death"))
        ));
    }

    #[test]
    fn an_empty_catalogue_leaves_the_consolation_troop_unset() {
        // Validation passes (an empty id is allowed) and the fallback finds
        // nothing, so a defeated hero keeps an empty army.
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        assert!(resources.tuning.temp_death_troop.is_empty());
    }

    #[test]
    fn dismissing_a_stack_compacts_the_army() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 5;
        game.army[2].id.copy_from_str("archers");
        game.army[2].count = 3;

        assert!(!game.dismiss_army_slot(1));
        // The survivors slide down rather than leaving a hole.
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[1].id.as_str(), "archers");
        assert!(game.army[2].id.is_empty());
    }

    #[test]
    fn dismissing_the_last_stack_asks_first_and_dismisses_nothing() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;

        // True means "this would empty the army" -- the caller must confirm,
        // and the stack is deliberately left standing until it does.
        assert!(game.dismiss_army_slot(0));
        assert_eq!(game.army[0].id.as_str(), "peasants");
        assert_eq!(game.army[0].count, 10);
    }

    #[test]
    fn a_zero_count_stack_does_not_count_as_occupied() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;
        game.army[1].id.copy_from_str("militia");
        game.army[1].count = 0;

        // Only one stack is really occupied, so this is still the last one.
        assert!(game.dismiss_army_slot(0));
        assert_eq!(game.army[0].count, 10);
    }

    #[test]
    fn dismissing_an_out_of_range_slot_does_nothing() {
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 10;

        assert!(!game.dismiss_army_slot(GAME_ARMY_SLOTS));
        assert_eq!(game.army[0].id.as_str(), "peasants");
    }

    /// `gryphons` fly and are skill 3. `sprites` carry the FLY bit but are
    /// only skill 1. `peasants` do not fly at all.
    fn flying_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"},
                {"index":1,"id":"gryphons","skill_level":3,"hit_points":6,"move_rate":3,"melee":[2,3],"ranged":[0,0,0],"recruit_cost":50,"spoils_factor":1,"abilities":"FLY","morale_group":"B","dwelling":"hills"},
                {"index":2,"id":"sprites","skill_level":1,"hit_points":1,"move_rate":3,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":5,"spoils_factor":1,"abilities":"FLY","morale_group":"A","dwelling":"forest"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    fn with_army(stacks: &[(&str, i32)]) -> Game {
        let mut game = Game::default();
        for (slot, (id, count)) in stacks.iter().enumerate() {
            game.army[slot].id.copy_from_str(id);
            game.army[slot].count = *count;
        }
        game
    }

    #[test]
    fn a_flier_needs_the_ability_and_skill_level_two() {
        let resources = flying_fixture();

        assert!(troop_flies(resources.troop_by_id("gryphons")));
        // Sprites carry the FLY bit but are only skill 1, so they ground the
        // hero despite flying in combat.
        assert!(!troop_flies(resources.troop_by_id("sprites")));
        assert!(!troop_flies(resources.troop_by_id("peasants")));
        assert!(!troop_flies(None));
    }

    #[test]
    fn one_earthbound_stack_grounds_the_whole_army() {
        let resources = flying_fixture();

        assert!(with_army(&[("gryphons", 10)]).player_can_fly(&resources));
        assert!(!with_army(&[("gryphons", 10), ("peasants", 5)]).player_can_fly(&resources));
        // A stack at zero count is not an obstacle -- the loop skips it the
        // same way it skips an empty slot.
        assert!(with_army(&[("gryphons", 10), ("peasants", 0)]).player_can_fly(&resources));
    }

    #[test]
    fn an_empty_army_can_fly_vacuously() {
        let resources = flying_fixture();

        // GamePlayerCanFly looks for a stack to REJECT; with none, it
        // returns true. Nothing in the engine prevents this state.
        assert!(Game::default().player_can_fly(&resources));
    }

    #[test]
    fn a_stack_naming_no_catalogue_troop_grounds_the_army() {
        let resources = flying_fixture();

        // troop_by_id misses, and GameTroopFlies(NULL) is false.
        assert!(!with_army(&[("no_such_troop", 3)]).player_can_fly(&resources));
    }

    #[test]
    fn mounting_flight_is_legal_only_from_the_saddle() {
        let resources = flying_fixture();
        let mut game = with_army(&[("gryphons", 10)]);

        assert!(game.mount_fly(&resources));
        assert_eq!(game.character.mount, Mount::Fly);
        // Already airborne: refused rather than re-applied.
        assert!(!game.mount_fly(&resources));

        // At sea is not the saddle either.
        let mut sailor = with_army(&[("gryphons", 10)]);
        sailor.character.mount = Mount::Sail;
        assert!(!sailor.mount_fly(&resources));
        assert_eq!(sailor.character.mount, Mount::Sail);
    }

    #[test]
    fn a_grounded_army_cannot_take_off() {
        let resources = flying_fixture();
        let mut game = with_army(&[("peasants", 5)]);

        assert!(!game.mount_fly(&resources));
        assert_eq!(game.character.mount, Mount::Ride);
    }

    #[test]
    fn landing_needs_plain_open_grass() {
        let mut map = Map::from_tiles(
            5,
            1,
            vec![
                Tile::plain_grass(),
                Tile::terrain(Terrain::Water),
                Tile::interactive(Interact::TreasureChest),
                Tile::blocked_grass(),
                Tile::terrain(Terrain::Mountain),
            ],
        )
        .expect("fixture dimensions match");
        map.tile_mut_i32(2, 0).expect("tile exists").terrain = Terrain::Grass;
        let game = Game::default();

        assert!(game.can_land_at(&map, 0, 0));
        assert!(!game.can_land_at(&map, 1, 0), "water");
        assert!(!game.can_land_at(&map, 2, 0), "something on the tile");
        assert!(!game.can_land_at(&map, 3, 0), "a visual blocker");
        assert!(!game.can_land_at(&map, 4, 0), "mountain");
        // Off the map is not a landing site either.
        assert!(!game.can_land_at(&map, -1, 0));
        assert!(!game.can_land_at(&map, 99, 0));
    }

    #[test]
    fn a_bridge_takes_a_landing_because_it_is_grass() {
        let mut map =
            Map::from_tiles(1, 1, vec![Tile::plain_grass()]).expect("fixture dimensions match");
        map.tile_mut_i32(0, 0).expect("tile exists").is_bridge = true;
        let game = Game::default();

        // GameCanLandAt never consults is_bridge, and a built bridge is
        // grass with nothing on it, so it qualifies.
        assert!(game.can_land_at(&map, 0, 0));
    }

    #[test]
    fn landing_returns_the_hero_to_the_saddle_only_while_airborne() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();

        // On the ground already: nothing to land from.
        assert!(!game.land_here(&map));

        game.character.mount = Mount::Fly;
        game.position.x = 1;
        // Over water: the hero stays up.
        assert!(!game.land_here(&map));
        assert_eq!(game.character.mount, Mount::Fly);

        game.position.x = 0;
        assert!(game.land_here(&map));
        assert_eq!(game.character.mount, Mount::Ride);
    }

    fn scoring_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "economy": {"scoring": {
                "per_villain": 500, "per_artifact": 250, "per_castle": 100,
                "kill_penalty": 1, "difficulty_multiplier": [0,1,2,4,8],
                "easy_halves": true
              }},
              "troops": []
            }"#,
        )
        .expect("fixture parses")
    }

    /// One villain caught, two artifacts, one castle: 500 + 500 + 100.
    fn scorer() -> Game {
        let mut game = Game::default();
        game.contract.villains_caught[0] = true;
        game.artifacts.found[0] = true;
        game.artifacts.found[1] = true;
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].owner_kind = CastleOwnerKind::Player;
        game
    }

    #[test]
    fn the_score_sums_the_catalogue_and_subtracts_the_kill_penalty() {
        let resources = scoring_fixture();
        let mut game = scorer();
        game.character.difficulty = 1;
        game.stats.followers_killed = 100;

        // 500 + 500 + 100 - 100 = 1000, times multiplier[1] = 1.
        assert_eq!(game.compute_score(&resources), 1000);
        assert_eq!(game.villains_caught_count(), 1);
        assert_eq!(game.artifacts_found_count(), 2);
        assert_eq!(game.castles_owned_count(), 1);
    }

    #[test]
    fn easy_halves_the_score_rather_than_multiplying_it() {
        let resources = scoring_fixture();
        let mut game = scorer();
        game.character.difficulty = 0;

        // 1100 / 2. multiplier[0] is 0 and would zero the score if the easy
        // branch fell through to it, which is why the order matters.
        assert_eq!(game.compute_score(&resources), 550);
    }

    #[test]
    fn difficulty_zero_is_left_unscaled_when_easy_halves_is_off() {
        let resources = Resources::from_json_str(
            r#"{
              "economy": {"scoring": {"easy_halves": false}},
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut game = scorer();
        game.character.difficulty = 0;

        // With the flag off, difficulty 0 misses the halving AND misses the
        // `d >= 1` multiplier arm, so it is scored raw -- multiplier[0] is
        // dead either way.
        assert_eq!(game.compute_score(&resources), 1100);
    }

    #[test]
    fn a_score_driven_negative_by_kills_clamps_to_zero() {
        let resources = scoring_fixture();
        let mut game = scorer();
        game.character.difficulty = 3;
        game.stats.followers_killed = 5_000;

        // The clamp runs AFTER the multiplier, so a negative subtotal is
        // multiplied first and still lands at 0.
        assert_eq!(game.compute_score(&resources), 0);
    }

    #[test]
    fn only_the_first_eight_artifacts_ever_score() {
        let mut game = Game::default();
        for slot in game.artifacts.found.iter_mut() {
            *slot = true;
        }

        // GameArtifactsFound's loop bound is a hard-coded 8, not the
        // artifact count, so any artifact past the eighth is invisible to
        // scoring even once found.
        assert!(
            game.artifacts.found.len() > 8,
            "the quirk needs room to show"
        );
        assert_eq!(game.artifacts_found_count(), 8);
    }

    #[test]
    fn an_empty_castle_slot_is_not_an_owned_castle() {
        let resources = scoring_fixture();
        let mut game = Game::default();
        // Owner set but no id: the id guard comes first, so this is not a
        // castle at all.
        game.castles[0].owner_kind = CastleOwnerKind::Player;
        game.castles[1].id.copy_from_str("real");
        game.castles[1].owner_kind = CastleOwnerKind::Player;

        assert_eq!(game.castles_owned_count(), 1);
        assert_eq!(game.compute_score(&resources), 50);
    }

    /// Two zones, each 6x2. `home` spawns on grass, `isle` spawns on water.
    fn zone_fixture() -> Resources {
        Resources::from_json_str(
            r##"{
              "world": {"fog_sight": 3},
              "tile_codes": {
                ".": {"art":"grass","terrain":"grass"},
                "W": {"art":"water","terrain":"water"}
              },
              "zones": [
                {"id":"home","map":"maps/home.dat","width":6,"height":2,"hero_spawn":{"x":1,"y":0}},
                {"id":"isle","map":"maps/isle.dat","width":6,"height":2,"hero_spawn":{"x":4,"y":1}}
              ],
              "castles": [
                {"index":0,"id":"kings_castle","name":"Zeta Keep","zone":"home","x":5,"y":0,"special":{"flow":"audience"}},
                {"index":1,"id":"castle1","name":"alpha Hold","zone":"isle","x":2,"y":1,"gate":{"x":3,"y":1}},
                {"index":2,"id":"castle2","name":"Beta Tower","zone":"home","x":4,"y":0}
              ],
              "towns": [
                {"index":0,"id":"town0","name":"Riverton","zone":"home","x":1,"y":1},
                {"index":1,"id":"town1","zone":"isle","x":0,"y":0}
              ],
              "troops": [],
              "spells": [
                {"index":0,"id":"town_gate","name":"Town Gate","kind":"adventure","cost":100}
              ]
            }"##,
        )
        .expect("fixture parses")
    }

    const HOME_TILES: &[u8] = b"......\n......\n";
    const ISLE_TILES: &[u8] = b"......\nWWWWWW\n";

    #[test]
    fn switching_zones_spawns_the_hero_and_reveals_fog_there() {
        let resources = zone_fixture();
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();

        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "home", HOME_TILES));
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!((game.position.x, game.position.y), (1, 0));
        // last_x/last_y follow the spawn, so nothing chases a stale tile.
        assert_eq!((game.position.last_x, game.position.last_y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert!(game.world.zones_discovered[0]);
        assert!(fog.seen[0][1]);
    }

    #[test]
    fn a_water_spawn_puts_the_hero_in_a_boat() {
        let resources = zone_fixture();
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();

        // `isle` spawns at (4,1), which its second row makes open water.
        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "isle", ISLE_TILES));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert!(game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (4, 1));
        assert_eq!(game.boat.zone.as_str(), "isle");
    }

    #[test]
    fn a_land_spawn_moors_an_owned_boat_at_the_spawn_rather_than_sinking_it() {
        let resources = zone_fixture();
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        game.boat.has_boat = true;
        game.boat.x = 9;
        game.boat.y = 9;
        game.boat.zone.copy_from_str("elsewhere");

        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "home", HOME_TILES));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert!(game.boat.has_boat);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.boat.zone.as_str(), "home");
    }

    #[test]
    fn a_revisited_zone_keeps_the_fog_it_had_and_a_first_visit_starts_blank() {
        let resources = zone_fixture();
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();

        // home spawns at (1,0) and reveals x 0..3; isle spawns at (4,1) and
        // reveals x 2..5. Column 0 is therefore home's alone.
        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "home", HOME_TILES));
        assert!(fog.seen[1][0]);

        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "isle", ISLE_TILES));
        // A first visit to `isle` starts blank, so home's column is gone.
        assert!(!fog.seen[1][0]);

        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "home", HOME_TILES));
        // Coming back restores what home had revealed.
        assert!(fog.seen[1][0]);
    }

    #[test]
    fn an_unknown_zone_is_refused_and_leaves_the_outgoing_fog_alone() {
        let resources = zone_fixture();
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        assert!(game.switch_zone_bytes(&resources, &mut map, &mut fog, "home", HOME_TILES));
        let snapshot = game.world.continent_fog[0];

        assert!(!game.switch_zone_bytes(&resources, &mut map, &mut fog, "atlantis", HOME_TILES));
        // The zone check comes before the snapshot, so nothing was stashed.
        assert_eq!(game.world.continent_fog[0], snapshot);
        assert_eq!(game.position.zone.as_str(), "home");
    }

    #[test]
    fn gate_destinations_sort_by_name_ignoring_case_and_skip_the_home_castle() {
        let resources = zone_fixture();
        let mut game = Game::default();
        for castle in game.castles.iter_mut().take(3) {
            castle.visited = true;
        }

        let mut out = [GateDestination::default(); GAME_GATE_DESTS_MAX];
        let written = game.gate_destinations(&resources, GateDestKind::Castle, &mut out);
        // "Zeta Keep" is the audience castle and never appears, and a
        // lowercase "alpha Hold" still sorts before "Beta Tower".
        assert_eq!(written, 2);
        assert_eq!(out[0].name.as_str(), "alpha Hold");
        assert_eq!(out[1].name.as_str(), "Beta Tower");
        // castle1 declares a gate tile, so that wins.
        assert_eq!((out[0].x, out[0].y), (3, 1));
        assert_eq!(out[0].zone.as_str(), "isle");
        // castle2 declares none, but a castle's gate_y still defaults to
        // y + 1 -- the tile at its south face -- so the `gate_x < 0` x/y
        // fallback in GameGateDestinations is unreachable for castles.
        assert_eq!((out[1].x, out[1].y), (4, 1));
    }

    #[test]
    fn a_town_without_a_name_falls_back_to_its_id_and_the_list_honours_the_cap() {
        let resources = zone_fixture();
        let mut game = Game::default();
        game.towns[0].visited = true;
        game.towns[1].visited = true;

        let mut out = [GateDestination::default(); GAME_GATE_DESTS_MAX];
        let written = game.gate_destinations(&resources, GateDestKind::Town, &mut out);
        assert_eq!(written, 2);
        // "Riverton" sorts before the nameless town1, which lists as its id.
        assert_eq!(out[0].name.as_str(), "Riverton");
        assert_eq!(out[1].name.as_str(), "town1");
        // A town's gate defaults to -1 rather than to a south face, so here
        // the x/y fallback really does supply the landing tile.
        assert_eq!((out[0].x, out[0].y), (1, 1));

        // A short buffer stops early rather than overrunning.
        let mut one = [GateDestination::default(); 1];
        assert_eq!(
            game.gate_destinations(&resources, GateDestKind::Town, &mut one),
            1
        );
    }

    #[test]
    fn an_unvisited_destination_never_lists() {
        let resources = zone_fixture();
        let game = Game::default();
        let mut out = [GateDestination::default(); GAME_GATE_DESTS_MAX];
        assert_eq!(
            game.gate_destinations(&resources, GateDestKind::Castle, &mut out),
            0
        );
        assert_eq!(
            game.gate_destinations(&resources, GateDestKind::Town, &mut out),
            0
        );
    }

    /// The same two zones, but with their maps written to real files so a
    /// teleport can load them the way the pack would. `resolve_map_path`
    /// passes an absolute path through untouched.
    fn zone_fixture_on_disk(tag: &str) -> Resources {
        let dir = std::env::temp_dir();
        let home = dir.join(format!(
            "openbounty-{}-home-{}.dat",
            tag,
            std::process::id()
        ));
        let isle = dir.join(format!(
            "openbounty-{}-isle-{}.dat",
            tag,
            std::process::id()
        ));
        std::fs::write(&home, HOME_TILES).expect("home map writes");
        std::fs::write(&isle, ISLE_TILES).expect("isle map writes");
        Resources::from_json_str(&format!(
            r##"{{
              "world": {{"fog_sight": 3}},
              "tile_codes": {{
                ".": {{"art":"grass","terrain":"grass"}},
                "W": {{"art":"water","terrain":"water"}}
              }},
              "zones": [
                {{"id":"home","map":"{home}","width":6,"height":2,"hero_spawn":{{"x":1,"y":0}}}},
                {{"id":"isle","map":"{isle}","width":6,"height":2,"hero_spawn":{{"x":4,"y":1}}}}
              ],
              "towns": [{{"index":0,"id":"town0","name":"Riverton","zone":"home","x":1,"y":1}}],
              "troops": [],
              "spells": [
                {{"index":0,"id":"town_gate","name":"Town Gate","kind":"adventure","cost":100}}
              ]
            }}"##,
            home = home.display(),
            isle = isle.display(),
        ))
        .expect("fixture parses")
    }

    #[test]
    fn a_gate_strands_the_boat_in_the_origin_zone_and_lands_the_hero_on_foot() {
        let resources = zone_fixture_on_disk("gate-boat");
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        // The isle spawn is open water, so the hero starts afloat at (4,1).
        assert!(game.switch_zone(&resources, &mut map, &mut fog, "isle"));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        game.spells.counts[0] = 2;

        let mut destination = GateDestination::default();
        destination.zone.copy_from_str("home");
        destination.x = 5;
        destination.y = 1;
        assert!(game.gate_teleport(&resources, &mut map, &mut fog, &destination, "town_gate"));

        // The hero arrives on foot at the landing tile, not at home's spawn.
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!((game.position.x, game.position.y), (5, 1));
        assert_eq!((game.position.last_x, game.position.last_y), (5, 1));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        // The boat stayed where it was cast from, overriding the re-attach
        // switch_zone performed at home's spawn.
        assert!(game.boat.has_boat);
        assert_eq!(game.boat.zone.as_str(), "isle");
        assert_eq!((game.boat.x, game.boat.y), (4, 1));
        assert_eq!(game.spells.counts[0], 1);
    }

    #[test]
    fn a_gate_leaves_its_landing_tile_fogged() {
        let resources = zone_fixture_on_disk("gate-fog");
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        assert!(game.switch_zone(&resources, &mut map, &mut fog, "isle"));
        game.spells.counts[0] = 1;

        let mut destination = GateDestination::default();
        destination.zone.copy_from_str("home");
        destination.x = 5;
        destination.y = 1;
        assert!(game.gate_teleport(&resources, &mut map, &mut fog, &destination, "town_gate"));

        // switch_zone revealed around home's SPAWN at (1,0) and C never
        // re-reveals after moving the hero, so the tile actually landed on
        // stays dark until the next step.
        assert!(fog.seen[0][1]);
        assert!(!fog.seen[1][5]);
    }

    #[test]
    fn a_gate_to_an_unknown_zone_changes_nothing_and_keeps_the_charge() {
        let resources = zone_fixture_on_disk("gate-unknown");
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        assert!(game.switch_zone(&resources, &mut map, &mut fog, "home"));
        game.spells.counts[0] = 1;

        let mut destination = GateDestination::default();
        destination.zone.copy_from_str("atlantis");
        assert!(!game.gate_teleport(&resources, &mut map, &mut fog, &destination, "town_gate"));
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!(game.spells.counts[0], 1);

        // An empty destination zone is refused before anything is touched.
        let blank = GateDestination::default();
        assert!(!game.gate_teleport(&resources, &mut map, &mut fog, &blank, "town_gate"));
        assert_eq!(game.spells.counts[0], 1);
    }

    #[test]
    fn a_gate_with_no_charge_still_teleports() {
        let resources = zone_fixture_on_disk("gate-free");
        let mut game = Game::default();
        let mut map = Map::new(1, 1);
        let mut fog = Fog::default();
        assert!(game.switch_zone(&resources, &mut map, &mut fog, "isle"));

        let mut destination = GateDestination::default();
        destination.zone.copy_from_str("home");
        destination.x = 2;
        destination.y = 0;
        // C spends a charge only if one is held; it never gates the move on
        // one, because the caller has already checked.
        assert!(game.gate_teleport(&resources, &mut map, &mut fog, &destination, "town_gate"));
        assert_eq!(game.position.zone.as_str(), "home");
        assert_eq!(game.spells.counts[0], 0);
        // An unknown spell id is likewise no obstacle.
        assert!(game.gate_teleport(
            &resources,
            &mut map,
            &mut fog,
            &destination,
            "no_such_spell"
        ));
    }
}
