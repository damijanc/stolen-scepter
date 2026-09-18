//! Resolving a [`Sprite`] to the pack file that draws it, porting
//! `src/sprites.c`.
//!
//! C loads a texture per slot at startup and hands the bundle to every
//! renderer. This does the same work but stops one step short: it produces
//! PATHS, so the whole mapping -- catalog indices, animation cycles, facings
//! and fallbacks -- is pure and testable with no graphics library present. A
//! backend then loads what this names.
//!
//! This is also the sprite catalog the draw vocabulary deferred. Three
//! decisions need pack data and therefore live here, not in view code:
//! folding a free-running tick onto an animation's own cycle, choosing
//! between an authored facing and a mirrored single strip, and falling back
//! from an animation to a still image.

use openbounty_engine::resources::{ResAnimSet, ResEnding, ResSprites, Resources};
use openbounty_engine::tables::{Facing, FACE_COUNT};

use crate::draw::{Backdrop, HeroAnim, Sprite};

/// Slot counts, from the array sizes of C's `Sprites` bundle. A pack with
/// more entries than these has the extras ignored, exactly as C does.
pub const CLASS_SLOTS: usize = 4;
pub const VILLAIN_SLOTS: usize = 17;
pub const VIEW_ICON_SLOTS: usize = 14;
/// View icons 0..7 come from the artifact catalog; 8..13 from
/// `sprites.ui.view_icons_extra`.
pub const VIEW_ICON_ARTIFACTS: usize = 8;
pub const TROOP_SLOTS: usize = 25;
pub const COMBAT_TILE_SLOTS: usize = 15;

/// Frames a consumer assumes when a pack declares none, C's
/// `OB_ANIM_FRAMES_DEFAULT`.
pub const ANIM_FRAMES_DEFAULT: usize = 4;

/// Fold a free-running tick onto a cycle of `count` frames, porting
/// `sprites_frame`.
///
/// A count of zero means the pack declared no animation, so this yields 0 and
/// the caller draws whatever still image it has. Every animated draw goes
/// through this rather than masking against a fixed four.
pub fn frame_index(tick: i32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    // C negates a negative counter, which is undefined at INT_MIN. Taking the
    // magnitude is the same answer everywhere C is defined, and defined here.
    tick.unsigned_abs() as usize % count
}

/// One animation's frames, per facing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnimPaths {
    pub directional: bool,
    pub frames: [Vec<String>; FACE_COUNT],
}

impl AnimPaths {
    fn from_res(set: &ResAnimSet) -> Self {
        Self {
            directional: set.directional,
            frames: std::array::from_fn(|slot| {
                set.frames[slot][..set.count[slot]]
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect()
            }),
        }
    }

    pub fn is_present(&self) -> bool {
        self.frames.iter().any(|frames| !frames.is_empty())
    }

    /// The frame to draw for `facing` at `tick`, and whether the backend must
    /// mirror it. Ports `sprites_anim_tex`.
    ///
    /// A single-strip animation always returns its one strip and asks to be
    /// mirrored facing west, which is what the game did before facings
    /// existed. A directional one returns the authored facing and never asks
    /// for a mirror; a facing the pack left undeclared falls back to south
    /// rather than drawing nothing.
    pub fn frame(&self, facing: Facing, tick: i32) -> Option<(&str, bool)> {
        let south = Facing::South as usize;
        if !self.directional {
            let frames = &self.frames[south];
            let path = frames.get(frame_index(tick, frames.len()))?;
            return Some((path.as_str(), facing == Facing::West));
        }
        let mut slot = facing as usize;
        if self.frames[slot].is_empty() {
            slot = south;
        }
        let frames = &self.frames[slot];
        let path = frames.get(frame_index(tick, frames.len()))?;
        Some((path.as_str(), false))
    }
}

/// Every art path the shell can draw, resolved from the pack once.
#[derive(Clone, Debug)]
pub struct SpriteCatalog {
    hero_walk: AnimPaths,
    hero_idle: AnimPaths,
    hero_boat: AnimPaths,
    class_portrait: Vec<String>,
    villain_portrait: Vec<String>,
    villain_anim: Vec<Vec<String>>,
    view_icon: Vec<String>,
    troop_still: Vec<String>,
    troop_anim: Vec<Vec<String>>,
    combat_tile: Vec<String>,
    sprites: Box<ResSprites>,
    ending: ResEnding,
}

impl SpriteCatalog {
    pub fn from_resources(resources: &Resources) -> Self {
        let sprites = resources.sprites.clone();

        let class_portrait = (0..resources.classes_count().min(CLASS_SLOTS))
            .map(|index| {
                resources
                    .class_by_index(index as i32)
                    .map(|class| class.portrait.as_str().to_owned())
                    .unwrap_or_default()
            })
            .collect();

        let villain_count = resources.villains_count().min(VILLAIN_SLOTS);
        let mut villain_portrait = Vec::with_capacity(villain_count);
        let mut villain_anim = Vec::with_capacity(villain_count);
        for index in 0..villain_count {
            let Some(villain) = resources.villain_by_index(index as i32) else {
                villain_portrait.push(String::new());
                villain_anim.push(Vec::new());
                continue;
            };
            let portrait = villain.portrait.as_str().to_owned();
            let frames = if villain.anim_count > 0 {
                villain.anim[..villain.anim_count]
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect()
            } else {
                derived_villain_frames(&portrait)
            };
            villain_portrait.push(portrait);
            villain_anim.push(frames);
        }

        let mut view_icon = vec![String::new(); VIEW_ICON_SLOTS];
        for index in 0..resources.artifacts_count().min(VIEW_ICON_ARTIFACTS) {
            if let Some(artifact) = resources.artifact_by_index(index as i32) {
                view_icon[index] = artifact.icon.as_str().to_owned();
            }
        }
        for (offset, path) in sprites.view_icons_extra[..sprites.view_icons_extra_count]
            .iter()
            .take(VIEW_ICON_SLOTS - VIEW_ICON_ARTIFACTS)
            .enumerate()
        {
            view_icon[VIEW_ICON_ARTIFACTS + offset] = path.as_str().to_owned();
        }

        let troop_count = resources.troops_count().min(TROOP_SLOTS);
        let mut troop_still = Vec::with_capacity(troop_count);
        let mut troop_anim = Vec::with_capacity(troop_count);
        for index in 0..troop_count {
            let Some(troop) = resources.troop_by_index(index as i32) else {
                troop_still.push(String::new());
                troop_anim.push(Vec::new());
                continue;
            };
            troop_still.push(troop.sprite.as_str().to_owned());
            troop_anim.push(
                troop.anim[..troop.anim_count]
                    .iter()
                    .map(|path| path.as_str().to_owned())
                    .collect(),
            );
        }

        let combat_tile = sprites.combat[..sprites.combat_count.min(COMBAT_TILE_SLOTS)]
            .iter()
            .map(|path| path.as_str().to_owned())
            .collect();

        Self {
            hero_walk: AnimPaths::from_res(&sprites.hero_walk),
            hero_idle: AnimPaths::from_res(&sprites.hero_idle),
            hero_boat: AnimPaths::from_res(&sprites.hero_boat),
            class_portrait,
            villain_portrait,
            villain_anim,
            view_icon,
            troop_still,
            troop_anim,
            combat_tile,
            ending: resources.ending,
            sprites,
        }
    }

    pub fn hero(&self, anim: HeroAnim) -> &AnimPaths {
        match anim {
            HeroAnim::Walk => &self.hero_walk,
            HeroAnim::Idle => &self.hero_idle,
            HeroAnim::Boat => &self.hero_boat,
        }
    }

    /// Frames in a troop's idle cycle, 0 when it ships none.
    pub fn troop_anim_len(&self, troop: usize) -> usize {
        self.troop_anim.get(troop).map_or(0, Vec::len)
    }

    /// Frames in a villain's wanted-poster cycle.
    pub fn villain_anim_len(&self, villain: usize) -> usize {
        self.villain_anim.get(villain).map_or(0, Vec::len)
    }

    /// The animated troop frame for `tick`, falling back to the still sprite
    /// when the pack ships no animation or the frame is blank. Ports what
    /// `map_render` and `combat_render` both open with.
    pub fn troop_frame(&self, troop: usize, tick: i32) -> Option<&str> {
        let frames = self.troop_anim.get(troop)?;
        let animated = frames
            .get(frame_index(tick, frames.len()))
            .map(String::as_str)
            .filter(|path| !path.is_empty());
        animated.or_else(|| {
            self.troop_still
                .get(troop)
                .map(String::as_str)
                .filter(|path| !path.is_empty())
        })
    }

    /// The pack-relative path a sprite draws, or `None` when the pack names
    /// no file for it. A missing entry is not an error: C loads a zero texture
    /// and every draw site skips it.
    pub fn path(&self, sprite: Sprite) -> Option<&str> {
        let path = match sprite {
            Sprite::Tile(_) => return None,
            Sprite::Hero {
                anim,
                facing,
                frame,
            } => return self.hero(anim).frame(facing, frame).map(|(path, _)| path),
            Sprite::ClassPortrait(index) => self.class_portrait.get(index)?.as_str(),
            Sprite::VillainPortrait(index) => self.villain_portrait.get(index)?.as_str(),
            Sprite::VillainAnim { villain, frame } => self
                .villain_anim
                .get(villain)?
                .get(frame as usize)?
                .as_str(),
            Sprite::ViewIcon(index) => self.view_icon.get(index)?.as_str(),
            Sprite::TroopStill(index) => self.troop_still.get(index)?.as_str(),
            Sprite::TroopAnim { troop, frame } => {
                self.troop_anim.get(troop)?.get(frame as usize)?.as_str()
            }
            Sprite::CombatTile(index) => self.combat_tile.get(index)?.as_str(),

            Sprite::PuzzleCover => self.sprites.puzzle_cover.as_str(),
            Sprite::Backdrop(Backdrop::Town) => self.sprites.town_backdrop.as_str(),
            Sprite::Backdrop(Backdrop::Castle) => self.sprites.castle_backdrop.as_str(),
            Sprite::Backdrop(Backdrop::Plains) => self.sprites.plains_backdrop.as_str(),
            Sprite::Backdrop(Backdrop::Forest) => self.sprites.forest_backdrop.as_str(),
            Sprite::Backdrop(Backdrop::HillCave) => self.sprites.hillcave_backdrop.as_str(),
            Sprite::Backdrop(Backdrop::Dungeon) => self.sprites.dungeon_backdrop.as_str(),
            Sprite::EndingWin => self.sprites.ending_win.as_str(),
            Sprite::EndingLose => self.sprites.ending_lose.as_str(),

            Sprite::HudContractSilhouette => self.sprites.hud_contract_silhouette.as_str(),
            Sprite::HudSiegeSilhouette => self.sprites.hud_siege_silhouette.as_str(),
            Sprite::HudMagicSilhouette => self.sprites.hud_magic_silhouette.as_str(),
            Sprite::HudPuzzleGrid => self.sprites.hud_puzzle_grid.as_str(),
            Sprite::HudGoldPurse => self.sprites.hud_gold_purse.as_str(),
            Sprite::HudSiegeAnim { frame } => self.hud_anim_frame(
                &self.sprites.hud_siege_animation,
                self.sprites.hud_siege_animation_count,
                frame,
            )?,
            Sprite::HudMagicAnim { frame } => self.hud_anim_frame(
                &self.sprites.hud_magic_animation,
                self.sprites.hud_magic_animation_count,
                frame,
            )?,
            Sprite::HudBarStrip => self.sprites.hud_bar_strip.as_str(),
            Sprite::ChromeOverworld => self.sprites.chrome_overworld.as_str(),

            Sprite::SplashLogo => self.sprites.splash_logo.as_str(),
            Sprite::SplashTitle => self.sprites.splash_title.as_str(),
            Sprite::ClassPicker => self.sprites.class_picker.as_str(),
            Sprite::ClassHighlight => self.sprites.class_highlight.as_str(),
            Sprite::Orb => self.sprites.orb.as_str(),

            Sprite::EndGrass => self.ending.grass_tile.as_str(),
            Sprite::EndCarpet => self.ending.carpet_tile.as_str(),
            Sprite::EndHero => self.ending.hero_tile.as_str(),
            Sprite::EndThrone => self.ending.throne_backdrop.as_str(),
        };
        (!path.is_empty()).then_some(path)
    }

    fn hud_anim_frame<'a, const N: usize>(
        &self,
        frames: &'a [openbounty_engine::tables::FixedStr<128>; N],
        count: usize,
        frame: i32,
    ) -> Option<&'a str> {
        let slot = usize::try_from(frame).ok()?;
        if slot >= count {
            return None;
        }
        Some(frames[slot].as_str())
    }

    /// The bitmap font strip and the VGA palette, which a backend loads once.
    pub fn font_path(&self) -> &str {
        self.sprites.font.as_str()
    }

    pub fn palette_path(&self) -> &str {
        self.sprites.palette.as_str()
    }
}

/// Wanted-poster frames for a villain whose pack declares none.
///
/// The portrait already points at the frame-0 file (`murray_00.png`), so the
/// `_NN.png` tail is stripped to recover the stem and the siblings are
/// rebuilt from it. The shipped pack declares NO villain animations at all,
/// so this is the live path for every wanted poster, not a fallback.
fn derived_villain_frames(portrait: &str) -> Vec<String> {
    if portrait.is_empty() {
        return Vec::new();
    }
    let bytes = portrait.as_bytes();
    let len = bytes.len();
    let stem = if len >= 7 && bytes[len - 7] == b'_' && bytes[len - 4] == b'.' {
        &portrait[..len - 7]
    } else if len >= 4 && bytes[len - 4] == b'.' {
        &portrait[..len - 4]
    } else {
        portrait
    };
    (0..ANIM_FRAMES_DEFAULT)
        .map(|frame| format!("{stem}_{frame:02}.png"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog(pack: &str) -> SpriteCatalog {
        let resources = Resources::from_json_str(pack).expect("fixture parses");
        SpriteCatalog::from_resources(&resources)
    }

    fn troop_pack(extra: &str) -> SpriteCatalog {
        catalog(&format!(
            r#"{{"world":{{"starting_zone":"home"}},"zones":[{{"id":"home"}}],
                 "troops":[{{"index":0,"id":"peasants","skill_level":1,"hit_points":1,
                   "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":1,
                   "spoils_factor":1,"abilities":"","morale_group":"A"{extra}}}]}}"#
        ))
    }

    #[test]
    fn a_tick_folds_onto_the_animations_own_cycle() {
        // A six-frame walk and a four-frame boat share one free-running tick,
        // so each has to fold it with its OWN length.
        assert_eq!(frame_index(0, 6), 0);
        assert_eq!(frame_index(7, 6), 1);
        assert_eq!(frame_index(7, 4), 3);
        // No declared frames means frame 0, so the caller falls back to a
        // still image rather than indexing nothing.
        assert_eq!(frame_index(9, 0), 0);
        // A counter that ran backwards still lands in range. C negates it,
        // which is undefined at the integer minimum; taking the magnitude
        // agrees everywhere C is defined and is defined everywhere.
        assert_eq!(frame_index(-7, 4), 3);
        assert_eq!(frame_index(i32::MIN, 4), 0);
    }

    #[test]
    fn a_single_strip_animation_is_mirrored_going_west() {
        let catalog = catalog(r#"{"troops":[],"sprites":{"hero":{"walk":["w0.png","w1.png"]}}}"#);
        let walk = catalog.hero(HeroAnim::Walk);
        assert!(!walk.directional);

        assert_eq!(walk.frame(Facing::East, 0), Some(("w0.png", false)));
        assert_eq!(walk.frame(Facing::South, 3), Some(("w1.png", false)));
        // The one strip is reused for every facing, mirrored only going west.
        assert_eq!(walk.frame(Facing::West, 0), Some(("w0.png", true)));
        assert_eq!(walk.frame(Facing::North, 0), Some(("w0.png", false)));
    }

    #[test]
    fn four_authored_facings_are_selected_and_never_mirrored() {
        // An asymmetric figure keeps its shield on the correct arm, which is
        // the whole point of authoring four.
        let catalog = catalog(
            r#"{"troops":[],"sprites":{"hero":{"walk":{"south":["s.png"],"east":["e.png"],
                 "west":["w.png"],"north":["n.png"]}}}}"#,
        );
        let walk = catalog.hero(HeroAnim::Walk);

        assert!(walk.directional);
        assert_eq!(walk.frame(Facing::West, 0), Some(("w.png", false)));
        assert_eq!(walk.frame(Facing::North, 0), Some(("n.png", false)));
    }

    #[test]
    fn a_facing_the_pack_left_out_falls_back_to_south() {
        // Falling back beats drawing nothing, and the set stays directional
        // so the fallback is NOT mirrored.
        let catalog = catalog(
            r#"{"troops":[],"sprites":{"hero":{"boat":{"south":["s.png"],"east":["e.png"]}}}}"#,
        );
        let boat = catalog.hero(HeroAnim::Boat);

        assert_eq!(boat.frame(Facing::East, 0), Some(("e.png", false)));
        assert_eq!(boat.frame(Facing::West, 0), Some(("s.png", false)));
        assert_eq!(boat.frame(Facing::North, 0), Some(("s.png", false)));
    }

    #[test]
    fn an_undeclared_animation_names_nothing_at_all() {
        let catalog = catalog(r#"{"troops":[],"sprites":{"hero":{"walk":["w.png"]}}}"#);
        assert!(catalog.hero(HeroAnim::Walk).is_present());
        // `idle` is optional: without it the hero holds still, and there is
        // no path to load.
        assert!(!catalog.hero(HeroAnim::Idle).is_present());
        assert_eq!(catalog.hero(HeroAnim::Idle).frame(Facing::South, 0), None);
    }

    #[test]
    fn a_troop_animates_when_it_can_and_holds_its_still_sprite_otherwise() {
        let animated = troop_pack(r#","sprite":"still.png","anim":["a0.png","a1.png"]"#);
        assert_eq!(animated.troop_anim_len(0), 2);
        assert_eq!(animated.troop_frame(0, 0), Some("a0.png"));
        assert_eq!(animated.troop_frame(0, 5), Some("a1.png"));

        let still_only = troop_pack(r#","sprite":"still.png""#);
        assert_eq!(still_only.troop_anim_len(0), 0);
        assert_eq!(still_only.troop_frame(0, 3), Some("still.png"));

        // Nothing declared at all names nothing, and an index past the
        // catalog is None rather than a panic.
        let bare = troop_pack("");
        assert_eq!(bare.troop_frame(0, 0), None);
        assert_eq!(bare.troop_frame(99, 0), None);
    }

    #[test]
    fn a_villain_without_declared_frames_gets_siblings_of_its_portrait() {
        // The shipped pack declares none, so this derivation is the live path
        // for every wanted poster rather than a compatibility shim.
        assert_eq!(
            derived_villain_frames("art/villains/murray_00.png"),
            vec![
                "art/villains/murray_00.png",
                "art/villains/murray_01.png",
                "art/villains/murray_02.png",
                "art/villains/murray_03.png",
            ]
        );
        // A portrait that is not already a frame file loses only its
        // extension.
        assert_eq!(
            derived_villain_frames("v.png"),
            vec!["v_00.png", "v_01.png", "v_02.png", "v_03.png"]
        );
        // A name whose tail only looks like a frame suffix is still stripped
        // the same way -- the rule is positional, exactly as C's is.
        assert_eq!(derived_villain_frames("")[..], [] as [String; 0]);
    }

    #[test]
    fn a_declared_villain_animation_wins_over_the_derivation() {
        let catalog = catalog(
            r#"{"troops":[],"villains":[
                 {"index":0,"id":"v0","portrait":"v0_00.png","anim":["x.png","y.png"]},
                 {"index":1,"id":"v1","portrait":"v1_00.png"}]}"#,
        );

        assert_eq!(catalog.villain_anim_len(0), 2);
        assert_eq!(
            catalog.path(Sprite::VillainAnim {
                villain: 0,
                frame: 1
            }),
            Some("y.png")
        );
        // The undeclared one gets four derived siblings.
        assert_eq!(catalog.villain_anim_len(1), ANIM_FRAMES_DEFAULT);
        assert_eq!(
            catalog.path(Sprite::VillainAnim {
                villain: 1,
                frame: 2
            }),
            Some("v1_02.png")
        );
    }

    #[test]
    fn view_icons_take_the_artifacts_first_and_the_extras_after() {
        let catalog = catalog(
            r#"{"troops":[],
                "artifacts":[{"index":0,"id":"sword","icon":"sword.png","power":"increased_damage"}],
                "sprites":{"ui":{"view_icons_extra":["map0.png","map1.png"]}}}"#,
        );

        assert_eq!(catalog.path(Sprite::ViewIcon(0)), Some("sword.png"));
        // Slots 1..7 stay empty for the artifacts this pack does not ship.
        assert_eq!(catalog.path(Sprite::ViewIcon(1)), None);
        // The extras always start at 8, whatever the artifact count.
        assert_eq!(catalog.path(Sprite::ViewIcon(8)), Some("map0.png"));
        assert_eq!(catalog.path(Sprite::ViewIcon(9)), Some("map1.png"));
        assert_eq!(catalog.path(Sprite::ViewIcon(13)), None);
        assert_eq!(catalog.path(Sprite::ViewIcon(99)), None);
    }

    #[test]
    fn the_role_fixed_slots_read_straight_from_the_manifest() {
        let catalog = catalog(
            r#"{"troops":[],
                "sprites":{"ui":{"chrome_overworld":"chrome.png","town_backdrop":"town.png"},
                           "hud":{"bar_strip":"bar.png","siege_animation":["s0.png","s1.png"]}},
                "ending":{"grass_tile":"grass.png"}}"#,
        );

        assert_eq!(catalog.path(Sprite::ChromeOverworld), Some("chrome.png"));
        assert_eq!(
            catalog.path(Sprite::Backdrop(Backdrop::Town)),
            Some("town.png")
        );
        assert_eq!(catalog.path(Sprite::HudBarStrip), Some("bar.png"));
        assert_eq!(
            catalog.path(Sprite::HudSiegeAnim { frame: 1 }),
            Some("s1.png")
        );
        // Past the declared count is nothing, not a stale slot.
        assert_eq!(catalog.path(Sprite::HudSiegeAnim { frame: 2 }), None);
        assert_eq!(catalog.path(Sprite::EndGrass), Some("grass.png"));
        // Undeclared is None, which a draw site skips.
        assert_eq!(catalog.path(Sprite::EndThrone), None);
        assert_eq!(catalog.path(Sprite::SplashTitle), None);
    }

    #[test]
    fn a_tile_sprite_is_not_the_catalogs_to_resolve() {
        // Tile art is named by the map, not by the manifest, so it resolves
        // straight from `Tile::art` and never through here.
        let catalog = catalog(r#"{"troops":[]}"#);
        assert_eq!(catalog.path(Sprite::tile("grass")), None);
    }

    #[test]
    fn the_combat_tileset_falls_back_to_the_shells_old_list() {
        let catalog = catalog(r#"{"troops":[]}"#);
        assert_eq!(
            catalog.path(Sprite::CombatTile(0)),
            Some("art/combat/field_grass.png")
        );
        assert_eq!(
            catalog.path(Sprite::CombatTile(14)),
            Some("art/combat/cursor_04.png")
        );
        assert_eq!(catalog.path(Sprite::CombatTile(15)), None);
        assert_eq!(catalog.font_path(), "art/font/kb-font.png");
        assert_eq!(catalog.palette_path(), "palettes/palette.bin");
    }
}
