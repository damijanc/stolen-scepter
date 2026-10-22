use crate::tables::{
    parse_troop_abilities, ArtifactDef, ArtifactPower, ClassDef, Facing, FixedStr, RankDef,
    SpellDef, SpellKind, TroopDef, VillainDef, ANIM_FRAMES_MAX, CAT_ARTIFACTS_MAX, CAT_CLASSES_MAX,
    CAT_ID_LEN, CAT_PATH_LEN, CAT_SPELLS_MAX, CAT_TROOPS_MAX, CAT_VILLAINS_MAX, CLASS_MAX_RANKS,
    CLASS_MAX_STARTING_TROOPS, FACE_COUNT, TROOP_TIER_COUNTS,
};
use crate::tile::Terrain;

pub const RES_MAX_CASTLES: usize = 32;
pub const RES_MAX_NEIGHBORS: usize = 8;
pub const RES_MAX_PREFERRED_TROOPS: usize = 16;
pub const RES_MAX_TOWNS: usize = 32;
pub const RES_MAX_ZONE_OBJECTS: usize = 256;
pub const RES_MAX_ZONES: usize = 8;
pub const RES_ID_LEN: usize = 32;
pub const RES_NAME_LEN: usize = 48;
pub const RES_VDESC_TEXT_LEN: usize = 320;
pub const RES_SIGN_BODY_LEN: usize = 128;
pub const RES_SIGN_TITLE_LEN: usize = 64;
pub const RES_MAX_CASTLE_DECOR: usize = 32;
pub const RES_TILE_ART_LEN: usize = 24;
pub const RES_TILE_CODE_COUNT: usize = 256;
pub const RES_SPAWN_TIERS: usize = 4;
pub const RES_SPAWN_POOL_N: usize = 5;
pub const RES_CHEST_TIERS: usize = 4;
pub const RES_MAX_CONTROLS: usize = 8;
pub const RES_CONTROL_LABEL_LEN: usize = 24;
pub const RES_CONTROL_TYPE_LEN: usize = 16;
pub const RES_NUMBER_NAME_MAX: usize = 8;
pub const RES_NUMBER_NAME_LABEL_LEN: usize = 24;
pub const RES_CASTLE_AUDIENCE_LEN: usize = 320;
/// Length of an art path in the sprite manifest, C's `RES_PATH_LEN`.
pub const RES_PATH_LEN: usize = 128;
/// The combat tileset, in the fixed role order the renderer indexes by.
pub const RES_COMBAT_TILES: usize = 15;
/// Cap on `ui.view_icons_extra`.
pub const RES_EXTRA_ICONS: usize = 8;

/// One animation, authored either as a single strip or as four facings.
///
/// `directional` false means the pack declared a flat array: the frames sit in
/// [`Facing::South`] and a renderer mirrors them east and west, which is how
/// every pack worked before facings existed. True means the pack declared the
/// object form, so a renderer picks a facing and never mirrors -- an
/// asymmetric figure then keeps its shield on the right arm walking west.
///
/// `count` is per facing because nothing requires the four to match: a pack
/// may ship a six-frame walk east and a four-frame walk north.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResAnimSet {
    pub directional: bool,
    pub count: [usize; FACE_COUNT],
    pub frames: [[FixedStr<RES_PATH_LEN>; ANIM_FRAMES_MAX]; FACE_COUNT],
}

impl ResAnimSet {
    /// Frames declared for `facing`, which is empty when the pack authored
    /// none for it.
    pub fn frames_for(&self, facing: Facing) -> &[FixedStr<RES_PATH_LEN>] {
        let slot = facing as usize;
        &self.frames[slot][..self.count[slot]]
    }

    /// True when the pack declared any frames at all, for any facing.
    pub fn is_present(&self) -> bool {
        self.count.iter().any(|count| *count > 0)
    }
}

/// The role-fixed art manifest: everything that is not per-catalog-entry.
///
/// These paths used to be compiled into the C shell, which meant a pack could
/// not name its own font, palette or battle art. They are pack data now, with
/// the old hardcoded values kept as defaults so a pack that declares nothing
/// still loads.
///
/// Nothing in the engine reads any of this; it is recorded here because
/// `Resources` is the pack reader, exactly as C's `resources.c` is.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResSprites {
    pub hero_walk: ResAnimSet,
    pub hero_idle: ResAnimSet,
    pub hero_boat: ResAnimSet,

    pub combat_count: usize,
    pub combat: [FixedStr<RES_PATH_LEN>; RES_COMBAT_TILES],

    pub font: FixedStr<RES_PATH_LEN>,
    pub palette: FixedStr<RES_PATH_LEN>,

    pub puzzle_cover: FixedStr<RES_PATH_LEN>,
    pub town_backdrop: FixedStr<RES_PATH_LEN>,
    pub castle_backdrop: FixedStr<RES_PATH_LEN>,
    pub plains_backdrop: FixedStr<RES_PATH_LEN>,
    pub forest_backdrop: FixedStr<RES_PATH_LEN>,
    pub hillcave_backdrop: FixedStr<RES_PATH_LEN>,
    pub dungeon_backdrop: FixedStr<RES_PATH_LEN>,
    pub ending_win: FixedStr<RES_PATH_LEN>,
    pub ending_lose: FixedStr<RES_PATH_LEN>,
    pub orb: FixedStr<RES_PATH_LEN>,
    pub view_icons_extra_count: usize,
    pub view_icons_extra: [FixedStr<RES_PATH_LEN>; RES_EXTRA_ICONS],
    pub chrome_overworld: FixedStr<RES_PATH_LEN>,
    pub splash_logo: FixedStr<RES_PATH_LEN>,
    pub splash_title: FixedStr<RES_PATH_LEN>,
    pub class_picker: FixedStr<RES_PATH_LEN>,
    pub class_highlight: FixedStr<RES_PATH_LEN>,

    pub hud_contract_silhouette: FixedStr<RES_PATH_LEN>,
    pub hud_siege_silhouette: FixedStr<RES_PATH_LEN>,
    pub hud_siege_animation_count: usize,
    pub hud_siege_animation: [FixedStr<RES_PATH_LEN>; ANIM_FRAMES_MAX],
    pub hud_magic_silhouette: FixedStr<RES_PATH_LEN>,
    pub hud_magic_animation_count: usize,
    pub hud_magic_animation: [FixedStr<RES_PATH_LEN>; ANIM_FRAMES_MAX],
    pub hud_puzzle_grid: FixedStr<RES_PATH_LEN>,
    pub hud_gold_purse: FixedStr<RES_PATH_LEN>,
    pub hud_bar_strip: FixedStr<RES_PATH_LEN>,
}

/// The pack's audio manifest: two music tracks and four one-shot tunes,
/// porting C's `res->audio`.
///
/// Nothing in the engine reads any of this; it is recorded here because
/// `Resources` is the pack reader, exactly as C's `resources.c` is. The shell
/// decides what to play and when.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResAudio {
    pub track_openworld: FixedStr<RES_PATH_LEN>,
    pub track_combat: FixedStr<RES_PATH_LEN>,
    pub tune_walk: FixedStr<RES_PATH_LEN>,
    pub tune_bump: FixedStr<RES_PATH_LEN>,
    pub tune_chest: FixedStr<RES_PATH_LEN>,
    pub tune_defeat: FixedStr<RES_PATH_LEN>,
}

impl Default for ResSprites {
    /// Everything empty, matching the zeroed struct C starts a load with. The
    /// defaults below are installed by the parser, not by this, because C
    /// installs them there.
    fn default() -> Self {
        Self {
            hero_walk: ResAnimSet::default(),
            hero_idle: ResAnimSet::default(),
            hero_boat: ResAnimSet::default(),
            combat_count: 0,
            combat: [FixedStr::empty(); RES_COMBAT_TILES],
            font: FixedStr::empty(),
            palette: FixedStr::empty(),
            puzzle_cover: FixedStr::empty(),
            town_backdrop: FixedStr::empty(),
            castle_backdrop: FixedStr::empty(),
            plains_backdrop: FixedStr::empty(),
            forest_backdrop: FixedStr::empty(),
            hillcave_backdrop: FixedStr::empty(),
            dungeon_backdrop: FixedStr::empty(),
            ending_win: FixedStr::empty(),
            ending_lose: FixedStr::empty(),
            orb: FixedStr::empty(),
            view_icons_extra_count: 0,
            view_icons_extra: [FixedStr::empty(); RES_EXTRA_ICONS],
            chrome_overworld: FixedStr::empty(),
            splash_logo: FixedStr::empty(),
            splash_title: FixedStr::empty(),
            class_picker: FixedStr::empty(),
            class_highlight: FixedStr::empty(),
            hud_contract_silhouette: FixedStr::empty(),
            hud_siege_silhouette: FixedStr::empty(),
            hud_siege_animation_count: 0,
            hud_siege_animation: [FixedStr::empty(); ANIM_FRAMES_MAX],
            hud_magic_silhouette: FixedStr::empty(),
            hud_magic_animation_count: 0,
            hud_magic_animation: [FixedStr::empty(); ANIM_FRAMES_MAX],
            hud_puzzle_grid: FixedStr::empty(),
            hud_gold_purse: FixedStr::empty(),
            hud_bar_strip: FixedStr::empty(),
        }
    }
}

/// Pack-declared colours, as packed `0xAARRGGBB`.
///
/// The defaults are the EGA values the shell used before a pack could name
/// its own, so a pack that declares no `colors` block looks unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResColors {
    pub minimap_grass: u32,
    pub minimap_forest: u32,
    pub minimap_mountain: u32,
    pub minimap_water: u32,
    pub minimap_desert: u32,
    pub minimap_fog: u32,
    pub difficulty_easy: u32,
    pub difficulty_normal: u32,
    pub difficulty_hard: u32,
    pub difficulty_impossible: u32,
}

impl Default for ResColors {
    fn default() -> Self {
        Self {
            minimap_grass: 0xFF00_AA00,
            minimap_forest: 0xFF55_FF55,
            minimap_mountain: 0xFFAA_5500,
            minimap_water: 0xFF55_55FF,
            minimap_desert: 0xFFFF_FF55,
            minimap_fog: 0xFF00_0000,
            difficulty_easy: 0xFF00_AAAA,
            difficulty_normal: 0xFFAA_0000,
            difficulty_hard: 0xFF55_55FF,
            difficulty_impossible: 0xFFAA_00AA,
        }
    }
}

impl ResColors {
    /// The status-bar colour for a difficulty, C's
    /// `status_bg_for_difficulty`. Anything outside 0..3 reads as easy, which
    /// is what C's switch falls through to.
    pub fn difficulty_bar(&self, difficulty: i32) -> u32 {
        match difficulty {
            1 => self.difficulty_normal,
            2 => self.difficulty_hard,
            3 => self.difficulty_impossible,
            _ => self.difficulty_easy,
        }
    }
}

/// One row of the controls settings panel, C's `ResControls.items` entry.
///
/// `kind` is C's `type` field ("bool" or "numeric"); the field name changes
/// because `type` is a Rust keyword. `def` and `id` are parsed but nothing
/// reads them -- the live option values come from `world.default_options` and
/// are stored in `Game.stats.options`, exactly as in C.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResControl {
    pub id: FixedStr<RES_ID_LEN>,
    pub label: FixedStr<RES_CONTROL_LABEL_LEN>,
    pub kind: FixedStr<RES_CONTROL_TYPE_LEN>,
    pub range: i32,
    pub def: i32,
    pub hidden: bool,
}

impl Default for ResControl {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            label: FixedStr::empty(),
            kind: FixedStr::empty(),
            range: 2,
            def: 0,
            hidden: false,
        }
    }
}

/// The controls settings block, C's `Resources.controls`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct ResControls {
    pub count: usize,
    pub items: [ResControl; RES_MAX_CONTROLS],
}

impl ResControls {
    pub fn by_index(&self, index: usize) -> Option<&ResControl> {
        self.items.get(index).filter(|_| index < self.count)
    }
}

/// The fuzzy-number labels (`combat.number_names`), C's `number_name_*` fields.
/// Thresholds are listed high-to-low; the first whose threshold is at or below
/// a count wins.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct NumberNames {
    pub count: usize,
    pub thresholds: [i32; RES_NUMBER_NAME_MAX],
    pub labels: [FixedStr<RES_NUMBER_NAME_LABEL_LEN>; RES_NUMBER_NAME_MAX],
}

impl NumberNames {
    /// C's `GameNumberName`: the first label whose threshold is `<= count`, or
    /// empty for a count below 1 or a pack that declares none.
    pub fn label(&self, count: i32) -> &str {
        if count < 1 || self.count == 0 {
            return "";
        }
        for index in 0..self.count {
            if count >= self.thresholds[index] {
                return self.labels[index].as_str();
            }
        }
        self.labels[self.count - 1].as_str()
    }
}

/// Parse `#RRGGBB` or `#AARRGGBB` into a packed `0xAARRGGBB`, porting
/// `parse_color_hex`. Alpha defaults to opaque when the string omits it, and
/// anything malformed reads as `None` so the caller keeps its default.
pub fn parse_color_hex(value: &str) -> Option<u32> {
    let digits = value.strip_prefix('#')?;
    if digits.len() != 6 && digits.len() != 8 {
        return None;
    }
    let mut packed = 0u32;
    for byte in digits.bytes() {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'a'..=b'f' => u32::from(byte - b'a') + 10,
            b'A'..=b'F' => u32::from(byte - b'A') + 10,
            _ => return None,
        };
        packed = (packed << 4) | digit;
    }
    if digits.len() == 6 {
        packed |= 0xFF00_0000;
    }
    Some(packed)
}

/// The victory cartoon: its four tile images and the parameters that animate
/// them. The tiles are sized to one map cell, so the cartoon lines up with
/// the viewport.
///
/// The numeric defaults come from the original's `display_cartoon` and are
/// installed even when the block is absent, so a pack that overrides only the
/// tile paths still animates correctly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResEnding {
    pub grass_tile: FixedStr<RES_PATH_LEN>,
    pub carpet_tile: FixedStr<RES_PATH_LEN>,
    pub hero_tile: FixedStr<RES_PATH_LEN>,
    pub throne_backdrop: FixedStr<RES_PATH_LEN>,
    pub grid_width: i32,
    pub grid_height: i32,
    pub carpet_column: i32,
    pub carpet_length: i32,
    pub frame_count: i32,
    pub ticks_per_step: i32,
    pub troop_border: bool,
}

impl Default for ResEnding {
    fn default() -> Self {
        Self {
            grass_tile: FixedStr::empty(),
            carpet_tile: FixedStr::empty(),
            hero_tile: FixedStr::empty(),
            throne_backdrop: FixedStr::empty(),
            grid_width: 6,
            grid_height: 5,
            carpet_column: 4,
            carpet_length: 5,
            frame_count: 10,
            ticks_per_step: 2,
            troop_border: true,
        }
    }
}

/// The battle art the C shell carried as a static array before a pack could
/// name its own. Installed before parsing, so a pack that declares no
/// `sprites.combat` still gets a full tileset.
pub const COMBAT_TILE_DEFAULTS: [&str; RES_COMBAT_TILES] = [
    "art/combat/field_grass.png",
    "art/combat/obstacle_01.png",
    "art/combat/obstacle_02.png",
    "art/combat/obstacle_03.png",
    "art/combat/castle_spike.png",
    "art/combat/castle_wall_01.png",
    "art/combat/castle_wall_02.png",
    "art/combat/castle_wall_03.png",
    "art/combat/castle_wall_04.png",
    "art/combat/castle_wall_05.png",
    "art/combat/castle_wall_06.png",
    "art/combat/cursor_01.png",
    "art/combat/cursor_02.png",
    "art/combat/cursor_03.png",
    "art/combat/cursor_04.png",
];

/// Likewise compiled into the shell before the manifest existed.
pub const FONT_DEFAULT: &str = "art/font/kb-font.png";
pub const PALETTE_DEFAULT: &str = "palettes/palette.bin";

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
    /// Sight radius handed to fog reveals. OpenBounty carries it through the
    /// call and then ignores it -- see `Fog::reveal`.
    pub fog_sight: i32,
}

impl Default for ResWorld {
    fn default() -> Self {
        Self {
            starting_zone: FixedStr::from_str("continentia"),
            default_name: FixedStr::from_str("Hero"),
            max_army_slots: 5,
            default_options: [4, 1, 1, 1, 1, 0, 5],
            fog_sight: 3,
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

/// Which geometry a pack is authored for. Ports `RenderMode`.
///
/// There is no default and no guess: legacy art is drawn for a 48x34 tile
/// and modern art for a square one, so a pack must say which it means.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderMode {
    /// Nothing declared, which is fatal at load.
    #[default]
    None,
    /// 320x200, 48x34 tiles, a 5x5 viewport.
    Legacy,
    /// Pack-declared tile size and viewport.
    Modern,
}

/// Render geometry declared by the pack. Ports `ResRender`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResRender {
    pub mode: RenderMode,
    /// Modern only. Legacy forces 48x34 whatever the pack declares.
    pub tile_w: i32,
    pub tile_h: i32,
    /// Viewport in tiles. Must be odd on both axes.
    pub tiles_w: i32,
    pub tiles_h: i32,
    /// The pack's art pixel multiple. `1` is the DOS original; a pack shipping
    /// art upscaled `N` times declares `N` so the shell presents at least that
    /// scale and never downsamples the art below 1:1. This is a Rust-side
    /// extension: C has no such field, but it is the same presentation
    /// contract as `mode`, so it lives in the render block.
    pub scale: i32,
}

impl Default for ResRender {
    fn default() -> Self {
        Self {
            mode: RenderMode::None,
            tile_w: 0,
            tile_h: 0,
            tiles_w: 0,
            tiles_h: 0,
            // A pack that declares no render block is not HD; the DOS original
            // is the baseline.
            scale: 1,
        }
    }
}

/// The final-score formula's knobs. Ports `ResScoring`.
///
/// `difficulty_multiplier[0]` is unused while `easy_halves` is set, and is
/// kept only for layout parity with the original.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResScoring {
    pub per_villain: i32,
    pub per_artifact: i32,
    pub per_castle: i32,
    pub kill_penalty: i32,
    pub difficulty_multiplier: [i32; 5],
    pub easy_halves: bool,
}

impl Default for ResScoring {
    fn default() -> Self {
        Self {
            per_villain: 500,
            per_artifact: 250,
            per_castle: 100,
            kill_penalty: 1,
            difficulty_multiplier: [0, 1, 2, 4, 8],
            easy_halves: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResEconomy {
    pub alcove_cost: i32,
    pub boat_cost_normal: i32,
    pub boat_cost_cheap: i32,
    pub siege_cost: i32,
    pub chest: ResChest,
    pub scoring: ResScoring,
}

impl Default for ResEconomy {
    fn default() -> Self {
        Self {
            alcove_cost: 5_000,
            boat_cost_normal: 500,
            boat_cost_cheap: 100,
            siege_cost: 3_000,
            chest: ResChest::default(),
            scoring: ResScoring::default(),
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
    pub name: FixedStr<RES_NAME_LEN>,
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
    pub neighbor_count: usize,
    pub neighbors: [FixedStr<RES_ID_LEN>; RES_MAX_NEIGHBORS],
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
            name: FixedStr::empty(),
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
            neighbor_count: 0,
            neighbors: [FixedStr::empty(); RES_MAX_NEIGHBORS],
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
    pub excluded_from_intel: bool,
    /// Parsed but never read, exactly as in C: no code consults it.
    pub excluded_from_siege: bool,
    /// Pack marker for a quest castle: "" is a normal castle, "audience" is
    /// the home castle. OpenBounty keeps the flow id in resources and tests it
    /// only through `resources_castle_is_home`.
    pub flow: FixedStr<RES_ID_LEN>,
    /// Audience-flow text templates, with %NAME%/%RANK%/%NEEDED%/%S%
    /// substitutions. Populated only for the audience castle.
    pub audience_intro: FixedStr<RES_CASTLE_AUDIENCE_LEN>,
    pub audience_rank_up: FixedStr<RES_CASTLE_AUDIENCE_LEN>,
    pub audience_more_needed: FixedStr<RES_CASTLE_AUDIENCE_LEN>,
    pub audience_final_rank: FixedStr<RES_CASTLE_AUDIENCE_LEN>,
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

impl ResCastle {
    /// True for the audience castle. Ports `resources_castle_is_home`, which
    /// is the only place OpenBounty tests the flow id.
    pub fn is_home(&self) -> bool {
        self.special.flow.as_str() == "audience"
    }
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

/// The `tuning` block of `game.json`. Only the knob the adventure spells
/// need is ported: C's block also carries `search_cost_days` and a
/// `temp_death` pair, and those belong with the cores that read them.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResTuning {
    pub instant_army_multiplier: [i32; 4],
    pub search_cost_days: i32,
    /// The consolation stack a defeated hero is sent home with. Left empty
    /// by the pack, this resolves after the troop catalog is parsed to the
    /// cheapest troop by recruit cost.
    pub temp_death_troop: FixedStr<CAT_ID_LEN>,
    pub temp_death_count: i32,
}

impl Default for ResTuning {
    fn default() -> Self {
        Self {
            instant_army_multiplier: [3, 2, 1, 1],
            search_cost_days: 10,
            temp_death_troop: FixedStr::empty(),
            temp_death_count: 20,
        }
    }
}

/// A wanted-poster description, keyed by villain id. The `features` and
/// `crimes` blocks are drawn verbatim, newlines and all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResVillainDesc {
    pub id: FixedStr<RES_ID_LEN>,
    pub alias: FixedStr<RES_NAME_LEN>,
    pub features: FixedStr<RES_VDESC_TEXT_LEN>,
    pub crimes: FixedStr<RES_VDESC_TEXT_LEN>,
}

impl Default for ResVillainDesc {
    fn default() -> Self {
        Self {
            id: FixedStr::empty(),
            alias: FixedStr::empty(),
            features: FixedStr::empty(),
            crimes: FixedStr::empty(),
        }
    }
}

pub const RES_CREDIT_GROUPS: usize = 6;
pub const RES_CREDIT_NAMES: usize = 4;
pub const RES_CREDIT_COPYRIGHT: usize = 4;
pub const RES_CREDIT_TEXT_LEN: usize = 64;
pub const RES_MAX_KEYBINDS: usize = 24;
pub const RES_KEYBIND_KEY_LEN: usize = 8;
pub const RES_KEYBIND_LABEL_LEN: usize = 24;

/// One keybinding reference row, C's `ResKeybind`. The options screen lists
/// these, skipping the mount-conditional ones.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResKeybind {
    pub key: FixedStr<RES_KEYBIND_KEY_LEN>,
    pub label: FixedStr<RES_KEYBIND_LABEL_LEN>,
}

impl Default for ResKeybind {
    fn default() -> Self {
        Self {
            key: FixedStr::empty(),
            label: FixedStr::empty(),
        }
    }
}

/// One credits group: a heading and its indented names. C's `credits.groups`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResCreditGroup {
    pub label: FixedStr<RES_CREDIT_TEXT_LEN>,
    pub names: [FixedStr<RES_CREDIT_TEXT_LEN>; RES_CREDIT_NAMES],
    pub name_count: usize,
}

impl Default for ResCreditGroup {
    fn default() -> Self {
        Self {
            label: FixedStr::empty(),
            names: [FixedStr::empty(); RES_CREDIT_NAMES],
            name_count: 0,
        }
    }
}

/// The credits screen's content, porting C's `credits` block. A pack overrides
/// it; an empty one means the screen is skipped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResCredits {
    /// The pack's declared inset path. C actually draws the class-select
    /// highlight sprite, so this only gates whether an inset is drawn.
    pub image: FixedStr<RES_PATH_LEN>,
    pub groups: [ResCreditGroup; RES_CREDIT_GROUPS],
    pub group_count: usize,
    pub copyright: [FixedStr<RES_CREDIT_TEXT_LEN>; RES_CREDIT_COPYRIGHT],
    pub copyright_count: usize,
}

impl Default for ResCredits {
    fn default() -> Self {
        Self {
            image: FixedStr::empty(),
            groups: [ResCreditGroup::default(); RES_CREDIT_GROUPS],
            group_count: 0,
            copyright: [FixedStr::empty(); RES_CREDIT_COPYRIGHT],
            copyright_count: 0,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Resources {
    resource_root: FixedStr<CAT_PATH_LEN>,
    /// Pack identity. The save loader refuses a save written against a
    /// different pack, since its ids belong to that pack's catalog.
    pub pack_id: FixedStr<RES_ID_LEN>,
    /// Pack version, substituted into credits lines as `%VERSION%`.
    pub version: i32,
    pub time: ResTime,
    pub world: ResWorld,
    pub contract: ResContract,
    pub economy: ResEconomy,
    pub render: ResRender,
    /// Boxed because the manifest is about 35KB: three 8KB animation sets
    /// plus the path tables. `Resources` is built inline, so an unboxed one
    /// overflows the stack.
    pub sprites: Box<ResSprites>,
    pub audio: ResAudio,
    pub ending: ResEnding,
    pub credits: ResCredits,
    pub colors: ResColors,
    pub controls: ResControls,
    /// Every string the pack declares, as `group -> key -> text`.
    ///
    /// C names all ~250 of them as struct fields with compiled-in defaults.
    /// A map carries the same data without the wall of declarations, and the
    /// DEFAULT moves to the one call site that needs it -- which is also what
    /// lets a view be ported without first transcribing every string a view
    /// that does not exist yet would want.
    pub strings: std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    /// The options screen's keybinding reference, from `strings.keybinds`.
    pub keybinds: [ResKeybind; RES_MAX_KEYBINDS],
    pub keybind_count: usize,
    pub tuning: ResTuning,
    pub spawn: ResSpawn,
    towns: [ResTown; RES_MAX_TOWNS],
    town_count: usize,
    zones: [ResZone; RES_MAX_ZONES],
    zone_count: usize,
    /// Map tiles a host supplied directly, parallel to `zones`. A pack read
    /// from an archive has no directory for `resolve_map_path` to join, so
    /// the host hands the bytes over instead; see `set_zone_map_bytes`.
    zone_maps: [Option<Vec<u8>>; RES_MAX_ZONES],
    castles: [ResCastle; RES_MAX_CASTLES],
    castle_count: usize,
    classes: [ClassDef; CAT_CLASSES_MAX],
    classes_count: usize,
    spells: [SpellDef; CAT_SPELLS_MAX],
    spells_count: usize,
    artifacts: [ArtifactDef; CAT_ARTIFACTS_MAX],
    artifacts_count: usize,
    tile_codes: Box<[ResTileCode; RES_TILE_CODE_COUNT]>,
    // Boxed, like `tile_codes` and the zone object arrays: a troop and a
    // villain each carry a 16-slot animation of 128-byte paths, so the two
    // catalogs together are about 140KB and overflow the stack if `Resources`
    // is built inline.
    troops: Box<[TroopDef; CAT_TROOPS_MAX]>,
    troops_count: usize,
    villains: Box<[VillainDef; CAT_VILLAINS_MAX]>,
    villains_count: usize,
    // Boxed like the catalogs above: 32 entries of ~720 bytes each.
    villain_descs: Box<[ResVillainDesc; CAT_VILLAINS_MAX]>,
    villain_desc_count: usize,
    morale_chart: [[u8; 5]; 5],
    number_names: NumberNames,
}

impl Default for Resources {
    fn default() -> Self {
        Self {
            resource_root: FixedStr::empty(),
            pack_id: FixedStr::empty(),
            version: 0,
            time: ResTime::default(),
            world: ResWorld::default(),
            contract: ResContract::default(),
            economy: ResEconomy::default(),
            render: ResRender::default(),
            sprites: Box::new(ResSprites::default()),
            audio: ResAudio::default(),
            ending: ResEnding::default(),
            credits: ResCredits::default(),
            colors: ResColors::default(),
            controls: ResControls::default(),
            strings: std::collections::BTreeMap::new(),
            keybinds: [ResKeybind::default(); RES_MAX_KEYBINDS],
            keybind_count: 0,
            tuning: ResTuning::default(),
            spawn: ResSpawn::default(),
            towns: [ResTown::default(); RES_MAX_TOWNS],
            town_count: 0,
            zones: std::array::from_fn(|_| ResZone::default()),
            zone_count: 0,
            zone_maps: Default::default(),
            castles: [ResCastle::default(); RES_MAX_CASTLES],
            castle_count: 0,
            classes: [ClassDef::default(); CAT_CLASSES_MAX],
            classes_count: 0,
            spells: [SpellDef::default(); CAT_SPELLS_MAX],
            spells_count: 0,
            artifacts: [ArtifactDef::default(); CAT_ARTIFACTS_MAX],
            artifacts_count: 0,
            tile_codes: Box::new([ResTileCode::default(); RES_TILE_CODE_COUNT]),
            troops: Box::new([TroopDef::default(); CAT_TROOPS_MAX]),
            troops_count: 0,
            villains: Box::new([VillainDef::default(); CAT_VILLAINS_MAX]),
            villains_count: 0,
            villain_descs: Box::new([ResVillainDesc::default(); CAT_VILLAINS_MAX]),
            villain_desc_count: 0,
            morale_chart: default_morale_chart(),
            number_names: NumberNames::default(),
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
        if let Some(pack_id) = optional_string_field(&root, "pack_id")? {
            resources.pack_id.copy_from_str(pack_id);
        }
        resources.version = root
            .get("version")
            .and_then(serde_json::Value::as_i64)
            .map_or(0, |value| value as i32);
        resources.parse_time(&root)?;
        resources.parse_world(&root)?;
        resources.parse_contract(&root)?;
        resources.parse_economy(&root)?;
        resources.parse_render(&root)?;
        resources.parse_sprites(&root);
        resources.parse_audio(&root);
        resources.parse_ending(&root);
        resources.parse_credits(&root);
        resources.parse_colors(&root);
        resources.parse_controls(&root)?;
        resources.parse_strings(&root);
        resources.parse_difficulty_labels(&root);
        resources.parse_keybinds(&root);
        resources.parse_tuning(&root)?;
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
        resources.parse_villain_descs(&root)?;
        resources.resolve_temp_death()?;
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

    pub fn controls_count(&self) -> usize {
        self.controls.count
    }

    pub fn control_by_index(&self, index: usize) -> Option<&ResControl> {
        self.controls.by_index(index)
    }

    /// C's `GameNumberName`: the fuzzy label for a count, or empty.
    pub fn number_name(&self, count: i32) -> &str {
        self.number_names.label(count)
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

    /// One pack string, or `None` when the pack declares no such entry. Every
    /// caller supplies its own fallback, which is the text C compiles in.
    pub fn string(&self, group: &str, key: &str) -> Option<&str> {
        Some(self.strings.get(group)?.get(key)?.as_str())
    }

    pub fn resolve_map_path(&self, zone: &ResZone) -> Option<std::path::PathBuf> {
        self.resolve_path(zone.map_path.as_str())
    }

    /// Supply a zone's map tiles directly. Returns false for an unknown zone.
    ///
    /// C reads every map through the pack stack, which reaches into an
    /// archive as readily as a directory. This engine has no pack reader --
    /// that stays in the host -- so a host that opened an archive reads each
    /// zone's `map_path` itself and hands the bytes over once. Every map load
    /// then prefers them, including the ones the engine makes on its own
    /// (scepter burial, zone switching from a gate or a defeat), which no
    /// byte-taking call site could have reached.
    pub fn set_zone_map_bytes(&mut self, zone_id: &str, bytes: Vec<u8>) -> bool {
        let Some(index) = self.zone_index_by_id(zone_id) else {
            return false;
        };
        self.zone_maps[index] = Some(bytes);
        true
    }

    /// The tiles [`Resources::set_zone_map_bytes`] supplied for a zone.
    pub fn zone_map_bytes(&self, zone_id: &str) -> Option<&[u8]> {
        self.zone_maps[self.zone_index_by_id(zone_id)?].as_deref()
    }

    /// Turn a pack-relative path into one that can be opened, porting
    /// `resources_resolve_path`. An empty path is nothing to open.
    ///
    /// In C this is an identity copy, because a pack-relative path resolves
    /// against the global pack stack. Here there is no such stack, so it joins
    /// the directory the pack was loaded from -- which is why a pack loaded
    /// from a string rather than a file resolves nothing.
    pub fn resolve_path(&self, relative: &str) -> Option<std::path::PathBuf> {
        if relative.is_empty() {
            return None;
        }
        let path = std::path::Path::new(relative);
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

    /// The audience (home) castle, or None if the pack defines none. Ports
    /// `resources_home_castle`.
    pub fn home_castle(&self) -> Option<&ResCastle> {
        self.castles[..self.castle_count]
            .iter()
            .find(|castle| castle.is_home())
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

    /// The wanted-poster description for a villain, keyed by id.
    pub fn villain_desc(&self, villain_id: &str) -> Option<&ResVillainDesc> {
        self.villain_descs[..self.villain_desc_count]
            .iter()
            .find(|desc| desc.id.as_str() == villain_id)
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
        self.economy.siege_cost =
            optional_i32_field(economy, "siege_cost", self.economy.siege_cost)?;
        self.economy.alcove_cost =
            optional_i32_field(economy, "alcove_cost", self.economy.alcove_cost)?;
        if let Some(scoring) = economy.get("scoring") {
            // C guards the block with cJSON_IsObject and skips a non-object
            // silently, leaving the canonical defaults in place.
            if scoring.is_object() {
                let current = self.economy.scoring;
                self.economy.scoring.per_villain =
                    optional_i32_field(scoring, "per_villain", current.per_villain)?;
                self.economy.scoring.per_artifact =
                    optional_i32_field(scoring, "per_artifact", current.per_artifact)?;
                self.economy.scoring.per_castle =
                    optional_i32_field(scoring, "per_castle", current.per_castle)?;
                self.economy.scoring.kill_penalty =
                    optional_i32_field(scoring, "kill_penalty", current.kill_penalty)?;
                parse_i32_array_into(
                    scoring,
                    "difficulty_multiplier",
                    &mut self.economy.scoring.difficulty_multiplier,
                )?;
                if let Some(easy_halves) = scoring.get("easy_halves") {
                    // Only a real bool overrides; C tests cJSON_IsBool first.
                    if let Some(easy_halves) = easy_halves.as_bool() {
                        self.economy.scoring.easy_halves = easy_halves;
                    }
                }
            }
        }
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

    /// Record the pack's `render` declaration. Ports the reading half of
    /// C's `render` block parser.
    ///
    /// DIVERGENCE, deliberate. C REJECTS a pack that declares no valid
    /// `render.mode`, because its `resources_load` serves a shell that
    /// cannot lay out a screen without one. This engine is render-free and
    /// never reads these values, so refusing a pack over them would put a
    /// presentation contract in the wrong crate. The declaration is recorded
    /// as-is -- `RenderMode::None` for a pack that declares nothing -- and
    /// `shell::layout` enforces the contract where C's `layout_init`
    /// consumes it. A real run still refuses the same packs, one layer later.
    ///
    /// The viewport validation (odd axes, minimum sizes) moves with it, for
    /// the same reason.
    /// Read the pack's colours. Anything missing or malformed keeps the EGA
    /// default rather than failing the load.
    fn parse_colors(&mut self, root: &serde_json::Value) {
        self.colors = ResColors::default();
        let Some(obj) = root.get("colors") else {
            return;
        };
        let colors = &mut self.colors;
        if let Some(minimap) = obj.get("minimap_terrain") {
            for (key, slot) in [
                ("grass", &mut colors.minimap_grass),
                ("forest", &mut colors.minimap_forest),
                ("mountain", &mut colors.minimap_mountain),
                ("water", &mut colors.minimap_water),
                ("desert", &mut colors.minimap_desert),
                ("fog", &mut colors.minimap_fog),
            ] {
                if let Some(packed) = json_color(minimap, key) {
                    *slot = packed;
                }
            }
        }
        if let Some(bar) = obj.get("difficulty_bar") {
            for (key, slot) in [
                ("easy", &mut colors.difficulty_easy),
                ("normal", &mut colors.difficulty_normal),
                ("hard", &mut colors.difficulty_hard),
                ("impossible", &mut colors.difficulty_impossible),
            ] {
                if let Some(packed) = json_color(bar, key) {
                    *slot = packed;
                }
            }
        }
    }

    fn parse_controls(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        self.controls = ResControls::default();
        let Some(settings) = root.get("controls").and_then(|c| c.get("settings")) else {
            return Ok(());
        };
        let settings = settings
            .as_array()
            .ok_or(ResourceError::InvalidField("controls.settings"))?;

        let mut n = 0;
        for value in settings.iter() {
            if n >= RES_MAX_CONTROLS {
                break;
            }
            // C skips a non-object entry without consuming a slot.
            if !value.is_object() {
                continue;
            }
            let mut control = ResControl::default();
            if let Some(id) = optional_string_field(value, "id")? {
                control.id.copy_from_str(id);
            }
            if let Some(label) = optional_string_field(value, "label")? {
                control.label.copy_from_str(label);
            }
            if let Some(kind) = optional_string_field(value, "type")? {
                control.kind.copy_from_str(kind);
            } else {
                control.kind.copy_from_str("bool");
            }
            control.range = optional_i32_field(value, "range", 2)?;
            control.def = optional_i32_field(value, "default", 0)?;
            control.hidden = optional_bool_field(value, "hidden", false)?;
            self.controls.items[n] = control;
            n += 1;
        }
        self.controls.count = n;
        Ok(())
    }

    /// Read every string group the pack declares.
    ///
    /// Only string values are kept; `villain_descriptions` and the like that
    /// hold objects are skipped here and read by whichever view owns them.
    fn parse_strings(&mut self, root: &serde_json::Value) {
        self.strings.clear();
        let Some(groups) = root.get("strings").and_then(serde_json::Value::as_object) else {
            return;
        };
        for (group, entries) in groups {
            let Some(entries) = entries.as_object() else {
                continue;
            };
            let table: std::collections::BTreeMap<String, String> = entries
                .iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_str()?.to_owned())))
                .collect();
            if !table.is_empty() {
                self.strings.insert(group.clone(), table);
            }
        }
    }

    /// Flatten `strings.difficulty.<name>.{label,score_mult}` into the generic
    /// string map as `"<name>.label"` / `"<name>.score_mult"`. It is the one
    /// nested UI group a startup screen reads (C's `ResUI.difficulty`), so it
    /// is flattened here rather than given a struct of its own.
    fn parse_difficulty_labels(&mut self, root: &serde_json::Value) {
        let Some(difficulty) = root
            .get("strings")
            .and_then(|strings| strings.get("difficulty"))
            .and_then(serde_json::Value::as_object)
        else {
            return;
        };
        let mut table = std::collections::BTreeMap::new();
        for (name, entry) in difficulty {
            let Some(entry) = entry.as_object() else {
                continue;
            };
            for (field, value) in entry {
                if let Some(text) = value.as_str() {
                    table.insert(format!("{name}.{field}"), text.to_owned());
                }
            }
        }
        if !table.is_empty() {
            self.strings.insert("difficulty".to_owned(), table);
        }
    }

    /// Read `strings.keybinds` into the options-screen reference list,
    /// porting `parse_ui`'s keybind loop. Bounded at `RES_MAX_KEYBINDS`.
    fn parse_keybinds(&mut self, root: &serde_json::Value) {
        self.keybind_count = 0;
        let Some(items) = root
            .get("strings")
            .and_then(|strings| strings.get("keybinds"))
            .and_then(serde_json::Value::as_array)
        else {
            return;
        };
        for item in items {
            if self.keybind_count >= RES_MAX_KEYBINDS {
                break;
            }
            let Some(item) = item.as_object() else {
                continue;
            };
            let slot = self.keybind_count;
            if let Some(key) = item.get("key").and_then(serde_json::Value::as_str) {
                self.keybinds[slot].key.copy_from_str(key);
            }
            if let Some(label) = item.get("label").and_then(serde_json::Value::as_str) {
                self.keybinds[slot].label.copy_from_str(label);
            }
            self.keybind_count += 1;
        }
    }

    /// Read the victory cartoon's art and parameters, porting `parse_ending`.
    /// Infallible, like the sprite manifest: the defaults stand for anything
    /// the pack leaves out or mis-types.
    fn parse_ending(&mut self, root: &serde_json::Value) {
        self.ending = ResEnding::default();
        let Some(obj) = root.get("ending").filter(|v| v.is_object()) else {
            return;
        };
        for (key, slot) in [
            ("grass_tile", &mut self.ending.grass_tile),
            ("carpet_tile", &mut self.ending.carpet_tile),
            ("hero_tile", &mut self.ending.hero_tile),
            ("throne_backdrop", &mut self.ending.throne_backdrop),
        ] {
            slot.copy_from_str(json_str_or(obj, key, ""));
        }
        let int = |key: &str, fallback: i32| {
            obj.get(key)
                .and_then(serde_json::Value::as_i64)
                .map_or(fallback, |v| v as i32)
        };
        self.ending.grid_width = int("grid_width", self.ending.grid_width);
        self.ending.grid_height = int("grid_height", self.ending.grid_height);
        self.ending.carpet_column = int("carpet_column", self.ending.carpet_column);
        self.ending.carpet_length = int("carpet_length", self.ending.carpet_length);
        self.ending.frame_count = int("frame_count", self.ending.frame_count);
        self.ending.ticks_per_step = int("ticks_per_step", self.ending.ticks_per_step);
        if let Some(border) = obj.get("troop_border").and_then(serde_json::Value::as_bool) {
            self.ending.troop_border = border;
        }
    }

    /// Read the credits screen's content, porting `parse_credits`. Infallible,
    /// like the sprite manifest: it is presentation data, and a wrong type
    /// leaves the default (an empty credits block, which the shell skips).
    fn parse_credits(&mut self, root: &serde_json::Value) {
        self.credits = ResCredits::default();
        let Some(obj) = root.get("credits").filter(|v| v.is_object()) else {
            return;
        };
        if let Some(image) = obj.get("image").and_then(serde_json::Value::as_str) {
            self.credits.image.copy_from_str(image);
        }
        if let Some(groups) = obj.get("groups").and_then(serde_json::Value::as_array) {
            for group in groups {
                if self.credits.group_count >= RES_CREDIT_GROUPS {
                    break;
                }
                let Some(group) = group.as_object() else {
                    continue;
                };
                let slot = self.credits.group_count;
                if let Some(label) = group.get("label").and_then(serde_json::Value::as_str) {
                    self.credits.groups[slot].label.copy_from_str(label);
                }
                if let Some(names) = group.get("names").and_then(serde_json::Value::as_array) {
                    for name in names {
                        if self.credits.groups[slot].name_count >= RES_CREDIT_NAMES {
                            break;
                        }
                        if let Some(name) = name.as_str() {
                            let index = self.credits.groups[slot].name_count;
                            self.credits.groups[slot].names[index].copy_from_str(name);
                            self.credits.groups[slot].name_count += 1;
                        }
                    }
                }
                self.credits.group_count += 1;
            }
        }
        if let Some(copyright) = obj.get("copyright").and_then(serde_json::Value::as_array) {
            for line in copyright {
                if self.credits.copyright_count >= RES_CREDIT_COPYRIGHT {
                    break;
                }
                if let Some(line) = line.as_str() {
                    let index = self.credits.copyright_count;
                    self.credits.copyright[index].copy_from_str(line);
                    self.credits.copyright_count += 1;
                }
            }
        }
    }

    /// Read the audio manifest, porting `parse_audio`. Absent keys leave
    /// empty slots; a pack that ships no audio is legal, as is one that ships
    /// only some of it.
    fn parse_audio(&mut self, root: &serde_json::Value) {
        let Some(audio) = root.get("audio").filter(|v| v.is_object()) else {
            return;
        };
        if let Some(tracks) = audio.get("tracks").filter(|v| v.is_object()) {
            for (key, slot) in [
                ("openworld", &mut self.audio.track_openworld),
                ("combat", &mut self.audio.track_combat),
            ] {
                slot.copy_from_str(json_str_or(tracks, key, ""));
            }
        }
        if let Some(tunes) = audio.get("tunes").filter(|v| v.is_object()) {
            for (key, slot) in [
                ("walk", &mut self.audio.tune_walk),
                ("bump", &mut self.audio.tune_bump),
                ("chest", &mut self.audio.tune_chest),
                ("defeat", &mut self.audio.tune_defeat),
            ] {
                slot.copy_from_str(json_str_or(tunes, key, ""));
            }
        }
    }

    /// Read the role-fixed art manifest, porting `parse_sprites`.
    ///
    /// Infallible on purpose, as C's is: every entry is optional presentation
    /// data, and a wrong type leaves the default standing rather than refusing
    /// the pack. Defaults are installed FIRST and unconditionally, so they
    /// survive a pack with no `sprites` block at all.
    fn parse_sprites(&mut self, root: &serde_json::Value) {
        for (slot, path) in COMBAT_TILE_DEFAULTS.iter().enumerate() {
            self.sprites.combat[slot].copy_from_str(path);
        }
        self.sprites.combat_count = RES_COMBAT_TILES;
        self.sprites.font.copy_from_str(FONT_DEFAULT);
        self.sprites.palette.copy_from_str(PALETTE_DEFAULT);

        let Some(obj) = root.get("sprites").filter(|v| v.is_object()) else {
            return;
        };

        if let Some(hero) = obj.get("hero").filter(|v| v.is_object()) {
            parse_anim_set(hero.get("walk"), &mut self.sprites.hero_walk);
            parse_anim_set(hero.get("idle"), &mut self.sprites.hero_idle);
            parse_anim_set(hero.get("boat"), &mut self.sprites.hero_boat);
        }

        // Guarded on the key being an array. `parse_path_array` reports zero
        // for a missing key, and writing that through would WIPE the defaults
        // installed above rather than leave them alone.
        if obj.get("combat").is_some_and(serde_json::Value::is_array) {
            self.sprites.combat_count = parse_path_array(obj, "combat", &mut self.sprites.combat);
        }

        // The fallback is the current value, so an absent or malformed key
        // keeps the default.
        let font = json_str_or(obj, "font", self.sprites.font.as_str()).to_owned();
        self.sprites.font.copy_from_str(&font);
        let palette = json_str_or(obj, "palette", self.sprites.palette.as_str()).to_owned();
        self.sprites.palette.copy_from_str(&palette);

        if let Some(ui) = obj.get("ui").filter(|v| v.is_object()) {
            let sprites = &mut self.sprites;
            for (key, slot) in [
                ("puzzle_cover", &mut sprites.puzzle_cover),
                ("town_backdrop", &mut sprites.town_backdrop),
                ("castle_backdrop", &mut sprites.castle_backdrop),
                ("plains_backdrop", &mut sprites.plains_backdrop),
                ("forest_backdrop", &mut sprites.forest_backdrop),
                ("hillcave_backdrop", &mut sprites.hillcave_backdrop),
                ("dungeon_backdrop", &mut sprites.dungeon_backdrop),
                ("ending_win", &mut sprites.ending_win),
                ("ending_lose", &mut sprites.ending_lose),
                ("orb", &mut sprites.orb),
                ("chrome_overworld", &mut sprites.chrome_overworld),
                ("splash_logo", &mut sprites.splash_logo),
                ("splash_title", &mut sprites.splash_title),
                ("class_picker", &mut sprites.class_picker),
                ("class_highlight", &mut sprites.class_highlight),
            ] {
                slot.copy_from_str(json_str_or(ui, key, ""));
            }
            sprites.view_icons_extra_count =
                parse_path_array(ui, "view_icons_extra", &mut sprites.view_icons_extra);
        }

        if let Some(hud) = obj.get("hud").filter(|v| v.is_object()) {
            let sprites = &mut self.sprites;
            for (key, slot) in [
                ("contract_silhouette", &mut sprites.hud_contract_silhouette),
                ("siege_silhouette", &mut sprites.hud_siege_silhouette),
                ("magic_silhouette", &mut sprites.hud_magic_silhouette),
                ("puzzle_grid", &mut sprites.hud_puzzle_grid),
                ("gold_purse", &mut sprites.hud_gold_purse),
                ("bar_strip", &mut sprites.hud_bar_strip),
            ] {
                slot.copy_from_str(json_str_or(hud, key, ""));
            }
            sprites.hud_siege_animation_count =
                parse_path_array(hud, "siege_animation", &mut sprites.hud_siege_animation);
            sprites.hud_magic_animation_count =
                parse_path_array(hud, "magic_animation", &mut sprites.hud_magic_animation);
        }
    }

    fn parse_render(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(render) = root.get("render") else {
            return Ok(());
        };
        // Read the art pixel multiple for either mode; absent means 1, the
        // DOS original. Layout validates it.
        self.render.scale = optional_i32_field(render, "scale", 1)?;
        let mode = render
            .get("mode")
            .and_then(|mode| mode.as_str())
            .unwrap_or("");
        match mode {
            // Legacy IGNORES any declared tile or viewport size. A pack that
            // sets tile_w in legacy mode gets no effect from it.
            "legacy" => {
                self.render.mode = RenderMode::Legacy;
                self.render.tile_w = 48;
                self.render.tile_h = 34;
                self.render.tiles_w = 5;
                self.render.tiles_h = 5;
            }
            "modern" => {
                self.render.mode = RenderMode::Modern;
                self.render.tile_w = optional_i32_field(render, "tile_w", 96)?;
                self.render.tile_h = optional_i32_field(render, "tile_h", 96)?;
                self.render.tiles_w = optional_i32_field(render, "tiles_w", 7)?;
                self.render.tiles_h = optional_i32_field(render, "tiles_h", 7)?;
            }
            _ => self.render.mode = RenderMode::None,
        }
        Ok(())
    }

    fn parse_tuning(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(tuning) = root.get("tuning") else {
            return Ok(());
        };
        // C guards this block with cJSON_IsObject and silently skips a
        // non-object, where the surrounding parsers reject one. Keep C's
        // tolerance: a malformed `tuning` leaves the defaults standing.
        if !tuning.is_object() {
            return Ok(());
        }
        parse_i32_array_into(
            tuning,
            "instant_army_multiplier",
            &mut self.tuning.instant_army_multiplier,
        )?;
        self.tuning.search_cost_days =
            optional_i32_field(tuning, "search_cost_days", self.tuning.search_cost_days)?;
        if let Some(temp_death) = tuning.get("temp_death") {
            if temp_death.is_object() {
                // C reads the troop with json_str's "" default, so a block
                // that names no troop CLEARS any earlier value rather than
                // leaving it standing.
                self.tuning
                    .temp_death_troop
                    .copy_from_str(optional_string_field(temp_death, "troop")?.unwrap_or(""));
                self.tuning.temp_death_count =
                    optional_i32_field(temp_death, "count", self.tuning.temp_death_count)?;
            }
        }
        Ok(())
    }

    /// Validate the temp-death knobs and fill in the default troop. Ports
    /// the tail of OpenBounty's resource load, which runs this AFTER the
    /// troop catalog exists.
    ///
    /// The order matters: validation comes first and an EMPTY troop id
    /// passes it, so the fallback below is reached. A pack that names a
    /// troop the catalog lacks is rejected instead.
    fn resolve_temp_death(&mut self) -> Result<(), ResourceError> {
        let configured = self.tuning.temp_death_troop;
        let troop_ok = configured.is_empty()
            || self.troops[..self.troops_count]
                .iter()
                .any(|troop| troop.id.as_str() == configured.as_str());
        if self.tuning.temp_death_count <= 0 || !troop_ok {
            return Err(ResourceError::InvalidField("tuning.temp_death"));
        }
        if !configured.is_empty() {
            return Ok(());
        }
        // The cheapest troop by recruit cost, first in catalog order on a
        // tie. A catalog with no troops leaves the id empty, which the
        // temp-death transition then treats as "no consolation stack".
        let mut best: Option<&TroopDef> = None;
        for troop in &self.troops[..self.troops_count] {
            if troop.id.is_empty() {
                continue;
            }
            if best.is_none_or(|current| troop.recruit_cost < current.recruit_cost) {
                best = Some(troop);
            }
        }
        if let Some(best) = best {
            self.tuning.temp_death_troop.copy_from_str(best.id.as_str());
        }
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
        self.world.fog_sight = optional_i32_field(world, "fog_sight", self.world.fog_sight)?;
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
        if let Some(chart) = combat.get("morale_chart") {
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
                    self.morale_chart[row_idx][col_idx] =
                        s.as_bytes().first().copied().unwrap_or(b'N');
                }
            }
        }

        // number_names: array of {"min": N, "label": "..."}, high-to-low.
        if let Some(names) = combat.get("number_names") {
            let names = names
                .as_array()
                .ok_or(ResourceError::InvalidField("combat.number_names"))?;
            let mut n = 0;
            for value in names.iter() {
                if n >= RES_NUMBER_NAME_MAX {
                    break;
                }
                if !value.is_object() {
                    continue;
                }
                self.number_names.thresholds[n] = optional_i32_field(value, "min", 1)?;
                if let Some(label) = optional_string_field(value, "label")? {
                    self.number_names.labels[n].copy_from_str(label);
                }
                n += 1;
            }
            self.number_names.count = n;
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

    fn parse_villain_descs(&mut self, root: &serde_json::Value) -> Result<(), ResourceError> {
        let Some(descs) = root.get("villain_descriptions") else {
            return Ok(());
        };
        let Some(descs) = descs.as_object() else {
            return Ok(());
        };
        for (id, value) in descs {
            if self.villain_desc_count >= CAT_VILLAINS_MAX {
                break;
            }
            let desc = &mut self.villain_descs[self.villain_desc_count];
            desc.id.copy_from_str(id);
            if let Some(alias) = optional_string_field(value, "alias")? {
                desc.alias.copy_from_str(alias);
            }
            if let Some(features) = optional_string_field(value, "features")? {
                desc.features.copy_from_str(features);
            }
            if let Some(crimes) = optional_string_field(value, "crimes")? {
                desc.crimes.copy_from_str(crimes);
            }
            self.villain_desc_count += 1;
        }
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
    if let Some(name) = optional_string_field(value, "name")? {
        troop.name.copy_from_str(name);
    }
    troop.max_population = optional_i32_field(value, "max_population", 0)?;
    troop.growth_per_week = optional_i32_field(value, "growth_per_week", 0)?;
    if let Some(sprite) = optional_string_field(value, "sprite")? {
        troop.sprite.copy_from_str(sprite);
    }
    troop.anim_count = parse_path_array(value, "anim", &mut troop.anim);
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
    zone.name
        .copy_from_str(optional_string_field(value, "name")?.unwrap_or(""));
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
    // C skips a non-array `neighbors` and any non-string entry silently,
    // keeping the defaults, so a stray value never burns a slot.
    if let Some(neighbors) = value.get("neighbors").and_then(serde_json::Value::as_array) {
        for neighbor in neighbors.iter().take(RES_MAX_NEIGHBORS) {
            if let Some(id) = neighbor.as_str() {
                zone.neighbors[zone.neighbor_count].copy_from_str(id);
                zone.neighbor_count += 1;
            }
        }
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
        castle.special.excluded_from_intel =
            optional_bool_field(special, "excluded_from_intel", false)?;
        castle.special.excluded_from_siege =
            optional_bool_field(special, "excluded_from_siege", false)?;
        if let Some(flow) = optional_string_field(special, "flow")? {
            castle.special.flow.copy_from_str(flow);
        }
        if let Some(audience) = special.get("audience") {
            if let Some(intro) = optional_string_field(audience, "intro")? {
                castle.special.audience_intro.copy_from_str(intro);
            }
            if let Some(rank_up) = optional_string_field(audience, "rank_up")? {
                castle.special.audience_rank_up.copy_from_str(rank_up);
            }
            if let Some(more_needed) = optional_string_field(audience, "more_needed")? {
                castle
                    .special
                    .audience_more_needed
                    .copy_from_str(more_needed);
            }
            if let Some(final_rank) = optional_string_field(audience, "final_rank")? {
                castle.special.audience_final_rank.copy_from_str(final_rank);
            }
        }
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
    villain.anim_count = parse_path_array(value, "anim", &mut villain.anim);
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

/// Read an array of file paths into a fixed-size slot array, porting
/// `parse_path_array`, and return how many landed.
///
/// A non-string entry is SKIPPED rather than consuming a slot. C's comment
/// records why: the hand-rolled loop it replaced advanced its index outside
/// the type check, so one bad entry left an empty frame in the middle of an
/// animation cycle. Entries past the cap are dropped. Anything that is not an
/// array at all reads as zero paths, never an error -- an animation is always
/// optional.
///
/// For an animation the returned count IS the cycle length, so a consumer
/// folds a tick with it rather than with the array's capacity.
fn parse_path_array<const N: usize, const SLOTS: usize>(
    value: &serde_json::Value,
    key: &str,
    out: &mut [FixedStr<N>; SLOTS],
) -> usize {
    parse_path_array_value(value.get(key), out)
}

/// [`parse_path_array`] against a value already in hand.
fn parse_path_array_value<const N: usize, const SLOTS: usize>(
    value: Option<&serde_json::Value>,
    out: &mut [FixedStr<N>; SLOTS],
) -> usize {
    let Some(entries) = value.and_then(serde_json::Value::as_array) else {
        return 0;
    };
    let mut count = 0;
    for entry in entries {
        if count >= SLOTS {
            break;
        }
        let Some(path) = entry.as_str() else {
            continue;
        };
        out[count].copy_from_str(path);
        count += 1;
    }
    count
}

/// One animation, in either authoring form, porting `parse_anim_set`.
///
/// A flat array lands in [`Facing::South`] with `directional` false, so a pack
/// written before facings existed parses exactly as it always did. An object
/// names up to four facings and sets `directional`; a facing it omits simply
/// keeps count 0 and a renderer falls back for it. Anything else leaves the
/// set empty.
fn parse_anim_set(value: Option<&serde_json::Value>, out: &mut ResAnimSet) {
    *out = ResAnimSet::default();
    let Some(value) = value else {
        return;
    };
    if value.is_array() {
        let south = Facing::South as usize;
        out.count[south] = parse_path_array_value(Some(value), &mut out.frames[south]);
        return;
    }
    if !value.is_object() {
        return;
    }
    out.directional = true;
    for (slot, key) in ["south", "east", "west", "north"].into_iter().enumerate() {
        out.count[slot] = parse_path_array_value(value.get(key), &mut out.frames[slot]);
    }
}

/// Read a string, falling back for an absent key OR a non-string value. Ports
/// C's `json_str`, which never treats a wrong type as an error.
fn json_str_or<'a>(value: &'a serde_json::Value, key: &str, fallback: &'a str) -> &'a str {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or(fallback)
}

/// Read a hex colour under `key`. Missing or malformed reads as `None`, so
/// the caller keeps whatever default it had. Ports `json_color`.
fn json_color(value: &serde_json::Value, key: &str) -> Option<u32> {
    parse_color_hex(value.get(key)?.as_str()?)
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
    use super::{RenderMode, Resources};
    use super::{FONT_DEFAULT, PALETTE_DEFAULT, RES_COMBAT_TILES};
    use crate::tables::{
        parse_troop_abilities, Facing, ANIM_FRAMES_MAX, TROOP_ABIL_ABSORB, TROOP_ABIL_FLY,
        TROOP_ABIL_LEECH, TROOP_ABIL_UNDEAD,
    };

    #[test]
    fn the_audio_block_parses_tracks_and_tunes() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "audio": {
                  "tracks": {"openworld": "audio/open.ogg", "combat": "audio/fight.ogg"},
                  "tunes": {"walk": "audio/walk.wav", "bump": "audio/bump.wav",
                            "chest": "audio/chest.wav", "defeat": "audio/defeat.wav"}
                }}"#,
        )
        .expect("fixture parses");

        let audio = &resources.audio;
        assert_eq!(audio.track_openworld.as_str(), "audio/open.ogg");
        assert_eq!(audio.track_combat.as_str(), "audio/fight.ogg");
        assert_eq!(audio.tune_walk.as_str(), "audio/walk.wav");
        assert_eq!(audio.tune_bump.as_str(), "audio/bump.wav");
        assert_eq!(audio.tune_chest.as_str(), "audio/chest.wav");
        assert_eq!(audio.tune_defeat.as_str(), "audio/defeat.wav");
    }

    /// A pack that declares no audio leaves every slot empty rather than
    /// failing, exactly as the sprite manifest does.
    #[test]
    fn a_pack_without_audio_leaves_every_slot_empty() {
        let resources = Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses");

        assert!(resources.audio.track_openworld.is_empty());
        assert!(resources.audio.tune_walk.is_empty());
    }

    /// Half a block is not an error either; the declared half is kept.
    #[test]
    fn a_partial_audio_block_keeps_what_it_declares() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "audio": {"tunes": {"walk": "audio/walk.wav"}}}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.audio.tune_walk.as_str(), "audio/walk.wav");
        assert!(resources.audio.tune_bump.is_empty());
        assert!(resources.audio.track_combat.is_empty());
    }

    #[test]
    fn legacy_render_mode_forces_the_dos_geometry() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy", "tile_w": 96, "tiles_w": 9}, "troops": []}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.render.mode, RenderMode::Legacy);
        // The declared tile_w and tiles_w are IGNORED: legacy is 48x34 tiles
        // on a 5x5 viewport whatever the pack asks for.
        assert_eq!((resources.render.tile_w, resources.render.tile_h), (48, 34));
        assert_eq!((resources.render.tiles_w, resources.render.tiles_h), (5, 5));
    }

    #[test]
    fn modern_render_mode_reads_the_pack_geometry() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "modern", "tile_w": 64, "tiles_h": 9}, "troops": []}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.render.mode, RenderMode::Modern);
        assert_eq!((resources.render.tile_w, resources.render.tile_h), (64, 96));
        assert_eq!((resources.render.tiles_w, resources.render.tiles_h), (7, 9));
    }

    #[test]
    fn the_art_pixel_scale_defaults_to_one_and_is_recorded_as_declared() {
        let plain = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        assert_eq!(plain.render.scale, 1);

        // A legacy HD pack keeps the DOS geometry but declares its art is 4x.
        let hd =
            Resources::from_json_str(r#"{"render": {"mode": "legacy", "scale": 4}, "troops": []}"#)
                .expect("fixture parses");
        assert_eq!(hd.render.mode, RenderMode::Legacy);
        assert_eq!((hd.render.tile_w, hd.render.tile_h), (48, 34));
        assert_eq!(hd.render.scale, 4);

        // An invalid value is recorded, not clamped; layout rejects it.
        let bad =
            Resources::from_json_str(r#"{"render": {"mode": "legacy", "scale": 0}, "troops": []}"#)
                .expect("fixture parses");
        assert_eq!(bad.render.scale, 0);
    }

    #[test]
    fn an_undeclared_render_mode_loads_as_none_rather_than_failing() {
        // C rejects the pack here. This engine never reads the block, so it
        // records None and leaves the contract to shell::layout, which is
        // where C's layout_init consumes it.
        let resources =
            Resources::from_json_str(r#"{"troops": []}"#).expect("a pack without render loads");
        assert_eq!(resources.render.mode, RenderMode::None);

        let resources = Resources::from_json_str(r#"{"render": {"mode": "vga"}, "troops": []}"#)
            .expect("an unknown mode loads");
        assert_eq!(resources.render.mode, RenderMode::None);
    }

    #[test]
    fn an_even_modern_viewport_is_recorded_not_refused_here() {
        // The odd-axis rule belongs to layout, so the loader keeps what the
        // pack said and lets the shell reject it.
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "modern", "tiles_w": 4}, "troops": []}"#,
        )
        .expect("fixture parses");
        assert_eq!(resources.render.tiles_w, 4);
    }

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
                  "name":"Home Land",
                  "is_home":true,
                  "neighbors":["fallback"],
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
                {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","tier_counts":[5,10,15,20],"dwelling":"plains","max_population":100,"growth_per_week":20},
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
                {"index":0,"id":"sword","name":"Blade of Testing","icon":"sword.png","power":"increased_damage","effect":"More damage","puzzle_cell":3,"zone":"home","local_idx":0}
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
        assert_eq!(
            resources.zone_by_id("home").unwrap().name.as_str(),
            "Home Land"
        );
        // A zone that declares no name keeps an empty one, not a panic.
        assert_eq!(resources.zone_by_id("fallback").unwrap().name.as_str(), "");
        let home = resources.zone_by_id("home").unwrap();
        assert_eq!(home.neighbor_count, 1);
        assert_eq!(home.neighbors[0].as_str(), "fallback");
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
        assert_eq!(peasants.name.as_str(), "Peasants");
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
        assert_eq!(artifact.name.as_str(), "Blade of Testing");
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

    /// A pack whose troops and villains declare their art.
    fn art_pack(troops: &str, villains: &str) -> Resources {
        Resources::from_json_str(&format!(
            r#"{{
              "world": {{"starting_zone": "home"}},
              "zones": [{{"id":"home"}}],
              "troops": {troops},
              "villains": {villains}
            }}"#
        ))
        .expect("fixture parses")
    }

    fn plain_troop(extra: &str) -> String {
        format!(
            r#"[{{"index":0,"id":"peasants","skill_level":1,"hit_points":1,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,
                  "spoils_factor":1,"abilities":"","morale_group":"A"{extra}}}]"#
        )
    }

    #[test]
    fn a_troop_carries_its_still_sprite_and_its_idle_cycle() {
        let resources = art_pack(
            &plain_troop(r#","sprite":"troops/peasants.png","anim":["p0.png","p1.png","p2.png"]"#),
            "[]",
        );
        let troop = resources.troop_by_id("peasants").expect("troop parses");

        assert_eq!(troop.sprite.as_str(), "troops/peasants.png");
        // The count IS the cycle length, so a tick folds with 3 rather than
        // with the array's 16 slots.
        assert_eq!(troop.anim_count, 3);
        assert_eq!(troop.anim[0].as_str(), "p0.png");
        assert_eq!(troop.anim[2].as_str(), "p2.png");
        assert_eq!(troop.anim[3].as_str(), "");
    }

    #[test]
    fn a_troop_without_an_animation_reports_no_frames() {
        // Count 0 is how a consumer knows to draw the still sprite instead.
        let resources = art_pack(&plain_troop(r#","sprite":"s.png""#), "[]");
        let troop = resources.troop_by_id("peasants").expect("troop parses");
        assert_eq!(troop.anim_count, 0);
        assert_eq!(troop.sprite.as_str(), "s.png");

        // Neither declared at all is still a clean parse: art is optional.
        let bare = art_pack(&plain_troop(""), "[]");
        let troop = bare.troop_by_id("peasants").expect("troop parses");
        assert_eq!(troop.anim_count, 0);
        assert!(troop.sprite.is_empty());
    }

    #[test]
    fn a_non_string_frame_is_skipped_rather_than_left_blank() {
        // C fixed a bug here: the loop it replaced advanced its index outside
        // the type check, so one bad entry left a hole in the middle of the
        // cycle and the animation flickered. The good frames must close up.
        let resources = art_pack(
            &plain_troop(r#","anim":["a.png",7,"b.png",null,"c.png"]"#),
            "[]",
        );
        let troop = resources.troop_by_id("peasants").expect("troop parses");

        assert_eq!(troop.anim_count, 3);
        assert_eq!(troop.anim[0].as_str(), "a.png");
        assert_eq!(troop.anim[1].as_str(), "b.png");
        assert_eq!(troop.anim[2].as_str(), "c.png");
    }

    #[test]
    fn frames_past_the_cap_are_dropped_and_a_non_array_is_no_frames() {
        let frames: Vec<String> = (0..20).map(|i| format!("\"f{i}.png\"")).collect();
        let resources = art_pack(
            &plain_troop(&format!(r#","anim":[{}]"#, frames.join(","))),
            "[]",
        );
        let troop = resources.troop_by_id("peasants").expect("troop parses");
        assert_eq!(troop.anim_count, ANIM_FRAMES_MAX);
        assert_eq!(troop.anim[ANIM_FRAMES_MAX - 1].as_str(), "f15.png");

        // A wrong type where an array belongs reads as no animation, not as
        // an error: every art declaration is optional.
        let odd = art_pack(&plain_troop(r#","anim":"nope.png""#), "[]");
        assert_eq!(
            odd.troop_by_id("peasants")
                .expect("troop parses")
                .anim_count,
            0
        );
    }

    #[test]
    fn a_villain_declares_its_wanted_poster_the_same_way_a_troop_does() {
        let resources = art_pack(
            "[]",
            r#"[{"index":0,"id":"v0","name":"Zero","portrait":"villains/v0.png",
                 "anim":["v0_00.png","v0_01.png"],"zone":"home","reward":1000},
                {"index":1,"id":"v1","portrait":"villains/v1.png","zone":"home"}]"#,
        );

        let zero = resources.villain_by_id("v0").expect("villain parses");
        assert_eq!(zero.portrait.as_str(), "villains/v0.png");
        assert_eq!(zero.anim_count, 2);
        assert_eq!(zero.anim[1].as_str(), "v0_01.png");

        // Undeclared means the shell derives `<portrait-stem>_NN` siblings,
        // so a count of 0 does NOT mean "no animation" for a villain the way
        // it does for a troop.
        let one = resources.villain_by_id("v1").expect("villain parses");
        assert_eq!(one.anim_count, 0);
        assert_eq!(one.portrait.as_str(), "villains/v1.png");
    }

    /// A pack whose whole `sprites` block is `block` (or absent when empty).
    fn sprite_pack(block: &str) -> Resources {
        let sprites = if block.is_empty() {
            String::new()
        } else {
            format!(r#","sprites": {block}"#)
        };
        Resources::from_json_str(&format!(
            r#"{{
              "world": {{"starting_zone": "home"}},
              "zones": [{{"id":"home"}}],
              "troops": []
              {sprites}
            }}"#
        ))
        .expect("fixture parses")
    }

    #[test]
    fn a_pack_that_declares_no_art_still_gets_the_shells_old_defaults() {
        // These paths were compiled into the C shell before the manifest
        // existed, so a pack from before it must keep working untouched.
        let resources = sprite_pack("");
        let sprites = &resources.sprites;

        assert_eq!(sprites.font.as_str(), FONT_DEFAULT);
        assert_eq!(sprites.palette.as_str(), PALETTE_DEFAULT);
        assert_eq!(sprites.combat_count, RES_COMBAT_TILES);
        assert_eq!(sprites.combat[0].as_str(), "art/combat/field_grass.png");
        assert_eq!(sprites.combat[14].as_str(), "art/combat/cursor_04.png");
        // Everything else has no default; an absent entry is simply empty.
        assert!(sprites.chrome_overworld.is_empty());
        assert!(!sprites.hero_walk.is_present());
    }

    #[test]
    fn a_sprites_block_without_combat_leaves_the_tileset_alone() {
        // The trap C's comment records: reading a missing key reports zero
        // frames, and writing that count through would WIPE the defaults
        // rather than leave them standing.
        let resources = sprite_pack(r#"{"font":"art/font/mine.png"}"#);
        assert_eq!(resources.sprites.combat_count, RES_COMBAT_TILES);
        assert_eq!(
            resources.sprites.combat[0].as_str(),
            "art/combat/field_grass.png"
        );
        assert_eq!(resources.sprites.font.as_str(), "art/font/mine.png");
        // Declared keys override; undeclared ones keep their default.
        assert_eq!(resources.sprites.palette.as_str(), PALETTE_DEFAULT);
    }

    #[test]
    fn a_declared_combat_tileset_replaces_the_defaults() {
        let resources = sprite_pack(r#"{"combat":["a.png","b.png"]}"#);
        assert_eq!(resources.sprites.combat_count, 2);
        assert_eq!(resources.sprites.combat[0].as_str(), "a.png");
        assert_eq!(resources.sprites.combat[1].as_str(), "b.png");
    }

    #[test]
    fn a_flat_hero_animation_is_a_single_mirrored_strip() {
        // How every pack was authored before facings existed: the frames land
        // in south and a renderer mirrors them going west.
        let resources = sprite_pack(r#"{"hero":{"walk":["w0.png","w1.png","w2.png"]}}"#);
        let walk = &resources.sprites.hero_walk;

        assert!(!walk.directional);
        assert!(walk.is_present());
        assert_eq!(walk.count[Facing::South as usize], 3);
        assert_eq!(walk.frames_for(Facing::South).len(), 3);
        assert_eq!(walk.frames_for(Facing::South)[2].as_str(), "w2.png");
        // The other three stay empty, which is what tells a renderer to
        // mirror rather than to select.
        assert!(walk.frames_for(Facing::West).is_empty());
        assert!(walk.frames_for(Facing::North).is_empty());
    }

    #[test]
    fn four_authored_facings_are_selected_rather_than_mirrored() {
        let resources = sprite_pack(
            r#"{"hero":{"walk":{"south":["s0.png"],"east":["e0.png","e1.png"],
                               "west":["w0.png"],"north":["n0.png"]}}}"#,
        );
        let walk = &resources.sprites.hero_walk;

        assert!(walk.directional);
        assert_eq!(walk.frames_for(Facing::West)[0].as_str(), "w0.png");
        // Nothing requires the four to be the same length.
        assert_eq!(walk.frames_for(Facing::East).len(), 2);
        assert_eq!(walk.frames_for(Facing::South).len(), 1);
    }

    #[test]
    fn an_omitted_facing_stays_empty_but_the_set_is_still_directional() {
        // A pack may author only some facings. The set stays directional, so
        // a renderer falls back for the missing one instead of mirroring the
        // whole animation.
        let resources = sprite_pack(r#"{"hero":{"boat":{"south":["b0.png"]}}}"#);
        let boat = &resources.sprites.hero_boat;

        assert!(boat.directional);
        assert!(boat.is_present());
        assert_eq!(boat.frames_for(Facing::South).len(), 1);
        assert!(boat.frames_for(Facing::East).is_empty());
    }

    #[test]
    fn an_undeclared_hero_animation_is_simply_absent() {
        // `idle` is optional: without it the hero holds frame 0 standing
        // still, which is how packs behaved before it existed.
        let resources = sprite_pack(r#"{"hero":{"walk":["w0.png"]}}"#);
        assert!(resources.sprites.hero_walk.is_present());
        assert!(!resources.sprites.hero_idle.is_present());
        assert!(!resources.sprites.hero_idle.directional);
    }

    #[test]
    fn the_ui_and_hud_groups_land_in_their_own_slots() {
        let resources = sprite_pack(
            r#"{"ui":{"chrome_overworld":"ui/chrome.png","town_backdrop":"ui/town.png",
                      "orb":"ui/orb.png","view_icons_extra":["i0.png","i1.png"]},
                "hud":{"bar_strip":"hud/bar.png","gold_purse":"hud/purse.png",
                       "siege_animation":["s0.png","s1.png","s2.png"]}}"#,
        );
        let sprites = &resources.sprites;

        assert_eq!(sprites.chrome_overworld.as_str(), "ui/chrome.png");
        assert_eq!(sprites.town_backdrop.as_str(), "ui/town.png");
        assert_eq!(sprites.orb.as_str(), "ui/orb.png");
        assert_eq!(sprites.view_icons_extra_count, 2);
        assert_eq!(sprites.view_icons_extra[1].as_str(), "i1.png");
        assert_eq!(sprites.hud_bar_strip.as_str(), "hud/bar.png");
        assert_eq!(sprites.hud_gold_purse.as_str(), "hud/purse.png");
        assert_eq!(sprites.hud_siege_animation_count, 3);
        assert_eq!(sprites.hud_magic_animation_count, 0);
        // An entry the group omits is empty, not defaulted to anything.
        assert!(sprites.castle_backdrop.is_empty());
    }

    #[test]
    fn a_malformed_art_entry_keeps_the_default_instead_of_failing_the_pack() {
        // Every entry here is optional presentation data, and C never refuses
        // a pack over one. A number where a path belongs falls back.
        let resources = sprite_pack(r#"{"font":7,"palette":null,"ui":"not an object"}"#);
        assert_eq!(resources.sprites.font.as_str(), FONT_DEFAULT);
        assert_eq!(resources.sprites.palette.as_str(), PALETTE_DEFAULT);
        assert!(resources.sprites.chrome_overworld.is_empty());

        // The whole block being the wrong type is fine too: defaults stand.
        let odd = sprite_pack(r#""nope""#);
        assert_eq!(odd.sprites.combat_count, RES_COMBAT_TILES);
        assert_eq!(odd.sprites.font.as_str(), FONT_DEFAULT);
    }

    #[test]
    fn difficulty_labels_are_flattened_into_the_string_map() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "strings": {"difficulty": {
                  "easy": {"label": "Trivial", "score_mult": "x.1"},
                  "normal": {"label": "Normal", "score_mult": " x1"}
                }}}"#,
        )
        .expect("fixture parses");

        assert_eq!(
            resources.string("difficulty", "easy.label"),
            Some("Trivial")
        );
        assert_eq!(
            resources.string("difficulty", "easy.score_mult"),
            Some("x.1")
        );
        assert_eq!(
            resources.string("difficulty", "normal.label"),
            Some("Normal")
        );
        // A difficulty the pack omits stays absent, so the shell falls back.
        assert_eq!(resources.string("difficulty", "impossible.label"), None);
    }

    #[test]
    fn credits_are_parsed_into_bounded_groups() {
        let resources = Resources::from_json_str(
            r#"{"troops": [], "version": 7,
                "credits": {
                  "image": "art/ui/highlight.png",
                  "groups": [
                    {"label": "Programmed By:", "names": ["A", "B"]},
                    {"label": "Graphics By:"}
                  ],
                  "copyright": ["Copyright 1990", "New World Computing"]
                }}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.version, 7);
        assert_eq!(resources.credits.image.as_str(), "art/ui/highlight.png");
        assert_eq!(resources.credits.group_count, 2);
        assert_eq!(resources.credits.groups[0].label.as_str(), "Programmed By:");
        assert_eq!(resources.credits.groups[0].name_count, 2);
        assert_eq!(resources.credits.groups[0].names[1].as_str(), "B");
        assert_eq!(resources.credits.groups[1].name_count, 0);
        assert_eq!(resources.credits.copyright_count, 2);
        assert_eq!(
            resources.credits.copyright[1].as_str(),
            "New World Computing"
        );
    }

    #[test]
    fn keybinds_are_parsed_from_the_strings_group() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "strings": {"keybinds": [
                  {"key": "A", "label": "View Army"},
                  {"key": "F", "label": "Fly"}
                ]}}"#,
        )
        .expect("fixture parses");
        assert_eq!(resources.keybind_count, 2);
        assert_eq!(resources.keybinds[0].key.as_str(), "A");
        assert_eq!(resources.keybinds[0].label.as_str(), "View Army");
        assert_eq!(resources.keybinds[1].key.as_str(), "F");
    }

    #[test]
    fn villain_descriptions_are_keyed_by_villain_id() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "villain_descriptions": {
                  "varn": {"alias":"None","features":"Bald.","crimes":"Treason."},
                  "hack": {"alias":"The Spitter","features":"Beard."}
                }}"#,
        )
        .expect("fixture parses");

        let murray = resources.villain_desc("varn").expect("present");
        assert_eq!(murray.alias.as_str(), "None");
        assert_eq!(murray.features.as_str(), "Bald.");
        assert_eq!(murray.crimes.as_str(), "Treason.");

        let hack = resources.villain_desc("hack").expect("present");
        assert_eq!(hack.alias.as_str(), "The Spitter");
        // A missing block stays empty, not a panic.
        assert!(hack.crimes.is_empty());

        assert!(resources.villain_desc("absent").is_none());
    }

    #[test]
    fn controls_parse_the_settings_rows_in_order() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "controls": {"settings": [
                  {"id":"delay","label":"Delay","type":"numeric","range":10,"default":5},
                  {"id":"sounds","label":"Sounds"},
                  {"id":"cga","label":"CGA","type":"numeric","range":8,"hidden":true}
                ]}}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.controls_count(), 3);

        let delay = resources.control_by_index(0).expect("present");
        assert_eq!(delay.id.as_str(), "delay");
        assert_eq!(delay.label.as_str(), "Delay");
        assert_eq!(delay.kind.as_str(), "numeric");
        assert_eq!(delay.range, 10);
        assert_eq!(delay.def, 5);
        assert!(!delay.hidden);

        // A missing `type` defaults to "bool", C's json_str fallback.
        let sounds = resources.control_by_index(1).expect("present");
        assert_eq!(sounds.kind.as_str(), "bool");
        assert_eq!(sounds.range, 2);
        assert_eq!(sounds.def, 0);

        let cga = resources.control_by_index(2).expect("present");
        assert!(cga.hidden);

        assert!(resources.control_by_index(3).is_none());
    }

    #[test]
    fn a_missing_or_empty_controls_block_yields_no_rows() {
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        assert_eq!(resources.controls_count(), 0);

        let resources = Resources::from_json_str(r#"{"troops": [], "controls": {"settings": []}}"#)
            .expect("fixture parses");
        assert_eq!(resources.controls_count(), 0);
    }

    #[test]
    fn number_names_pick_the_first_threshold_at_or_below_the_count() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "combat": {"number_names": [
                  {"min":100,"label":"A horde of"},
                  {"min":10,"label":"Some"},
                  {"min":1,"label":"A few"}
                ]}}"#,
        )
        .expect("fixture parses");

        assert_eq!(resources.number_name(150), "A horde of");
        assert_eq!(resources.number_name(100), "A horde of");
        assert_eq!(resources.number_name(99), "Some");
        assert_eq!(resources.number_name(10), "Some");
        assert_eq!(resources.number_name(1), "A few");
        // Below the lowest threshold or no names reads empty, as C does.
        assert_eq!(resources.number_name(0), "");
        assert_eq!(resources.number_name(-1), "");

        let bare = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        assert_eq!(bare.number_name(5), "");
    }

    #[test]
    fn castle_special_flags_and_audience_text_parse() {
        let resources = Resources::from_json_str(
            r#"{"troops": [],
                "castles": [{"id":"home","zone":"home","x":0,"y":0,
                  "special":{"flow":"audience","excluded_from_intel":true,"excluded_from_siege":true,
                    "audience":{"intro":"Hi %NAME%","rank_up":"Up %RANK%",
                                "more_needed":"Need %NEEDED%","final_rank":"Done"}}}]}"#,
        )
        .expect("fixture parses");

        let castle = resources.castle_by_id("home").expect("present");
        assert!(castle.special.excluded_from_intel);
        assert!(castle.special.excluded_from_siege);
        assert_eq!(castle.special.audience_intro.as_str(), "Hi %NAME%");
        assert_eq!(castle.special.audience_rank_up.as_str(), "Up %RANK%");
        assert_eq!(
            castle.special.audience_more_needed.as_str(),
            "Need %NEEDED%"
        );
        assert_eq!(castle.special.audience_final_rank.as_str(), "Done");
    }
}
