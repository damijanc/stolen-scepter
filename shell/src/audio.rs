//! What should be heard, and when. Ports the decisions in `src/audio.c`
//! without any of its raylib calls.
//!
//! This module knows nothing about an audio library, so every rule here is
//! testable with no device open -- the same trade `draw.rs` makes so a view
//! is testable with no window. `audio_backend.rs` turns these decisions into
//! sound.
//!
//! NOT ported: `options[2]` walk_beep and `options[4]` cga. Both are parsed,
//! drawn and toggleable, and C reads neither. See `specs/audio-design.md`.

use openbounty_engine::combat::{
    Combat, COMBAT_SIDES, COMBAT_SLOTS, COMBAT_SPELL_CLONE, COMBAT_SPELL_FIREBALL,
    COMBAT_SPELL_FREEZE, COMBAT_SPELL_LIGHTNING, COMBAT_SPELL_RESURRECT, COMBAT_SPELL_TELEPORT,
    COMBAT_SPELL_TURN_UNDEAD,
};
use openbounty_engine::game::{Game, Mount, TravelMode};
use openbounty_engine::map::Map;
use openbounty_engine::step::{StepInteraction, StepResult};
use openbounty_engine::tile::Terrain;

use crate::pointer::HitTarget;

/// The one-shot events. The first four are C's `AudioTuneId`; the rest are
/// a port extension, derived from what the engine returns: the combat five
/// (C's fights are silent but for the defeat tune), the walk tune's variants
/// for the ground and the mount, and the overworld's purchases and finds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tune {
    Walk,
    Bump,
    Chest,
    Defeat,
    /// A blow landed: a melee strike or its retaliation.
    Hit,
    /// A ranged unit loosed a shot.
    Shoot,
    /// A stack was wiped out.
    Slain,
    /// A combat spell took effect.
    Spell,
    /// The player won the fight.
    Victory,
    /// The walk tune ridden over sand.
    WalkDesert,
    /// The walk tune ridden over a bridge's planks.
    WalkBridge,
    /// The walk tune afloat.
    Sail,
    /// The walk tune in the air.
    Fly,
    /// Gold spent: a boat, siege weapons, a spell, recruits.
    Buy,
    /// An artifact, orb or navigation map picked up.
    Discover,
    /// Each combat spell's own sound, in `COMBAT_SPELL_*` order; any the
    /// pack lacks plays the generic `Spell`.
    SpellClone,
    SpellTeleport,
    SpellFireball,
    SpellLightning,
    SpellFreeze,
    SpellResurrect,
    SpellTurnUndead,
    /// A button tapped on mobile (`tap_sounds`).
    Tap,
}

impl Tune {
    pub const ALL: [Tune; 23] = [
        Tune::Walk,
        Tune::Bump,
        Tune::Chest,
        Tune::Defeat,
        Tune::Hit,
        Tune::Shoot,
        Tune::Slain,
        Tune::Spell,
        Tune::Victory,
        Tune::WalkDesert,
        Tune::WalkBridge,
        Tune::Sail,
        Tune::Fly,
        Tune::Buy,
        Tune::Discover,
        Tune::SpellClone,
        Tune::SpellTeleport,
        Tune::SpellFireball,
        Tune::SpellLightning,
        Tune::SpellFreeze,
        Tune::SpellResurrect,
        Tune::SpellTurnUndead,
        Tune::Tap,
    ];

    /// The tune's slot in [`Tune::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// Whether the music ducks under it. C ducks for every tune it has; the
    /// combat blows land several times a second, and ducking for each would
    /// pump the music, so only the victory sting joins C's four.
    pub fn ducks(self) -> bool {
        // A variant ducks as the tune it stands in for.
        let base = self.fallback().unwrap_or(self);
        !matches!(
            base,
            Tune::Hit | Tune::Shoot | Tune::Slain | Tune::Spell | Tune::Tap
        )
    }

    /// The sound of combat spell `spell` (a `COMBAT_SPELL_*` id), or the
    /// generic one for an id outside the stock seven.
    pub fn for_spell(spell: i32) -> Tune {
        match spell {
            COMBAT_SPELL_CLONE => Tune::SpellClone,
            COMBAT_SPELL_TELEPORT => Tune::SpellTeleport,
            COMBAT_SPELL_FIREBALL => Tune::SpellFireball,
            COMBAT_SPELL_LIGHTNING => Tune::SpellLightning,
            COMBAT_SPELL_FREEZE => Tune::SpellFreeze,
            COMBAT_SPELL_RESURRECT => Tune::SpellResurrect,
            COMBAT_SPELL_TURN_UNDEAD => Tune::SpellTurnUndead,
            _ => Tune::Spell,
        }
    }

    /// What plays instead when the pack lacks this tune: the walk variants
    /// fall back to the plain walk tune, so an older pack still ticks.
    pub fn fallback(self) -> Option<Tune> {
        match self {
            Tune::WalkDesert | Tune::WalkBridge | Tune::Sail | Tune::Fly => Some(Tune::Walk),
            Tune::SpellClone
            | Tune::SpellTeleport
            | Tune::SpellFireball
            | Tune::SpellLightning
            | Tune::SpellFreeze
            | Tune::SpellResurrect
            | Tune::SpellTurnUndead => Some(Tune::Spell),
            _ => None,
        }
    }
}

/// Whether a tap on `target` plays the tap tune on mobile. Every button
/// does; a sheet's blank area is a deliberate dead zone, and a map tile or a
/// combat cell is a move or an attack, which make their own sounds.
pub fn tap_sounds(target: HitTarget) -> bool {
    !matches!(
        target,
        HitTarget::SheetBlank | HitTarget::MapTile(..) | HitTarget::CombatCell(..)
    )
}

/// The walk tune for how the hero travels and what they stand on. Flying
/// beats sailing (a flyer never boards), and both beat the ground; a bridge
/// beats the terrain under it.
pub fn travel_tune(flying: bool, sailing: bool, terrain: Terrain, bridge: bool) -> Tune {
    if flying {
        Tune::Fly
    } else if sailing {
        Tune::Sail
    } else if bridge {
        Tune::WalkBridge
    } else if terrain == Terrain::Desert {
        Tune::WalkDesert
    } else {
        Tune::Walk
    }
}

/// The walk tune `tune` becomes for the hero's current tile and mount;
/// every other tune passes through.
pub fn on_ground(tune: Tune, game: &Game, map: &Map) -> Tune {
    if tune != Tune::Walk {
        return tune;
    }
    let tile = map.tile_i32(game.position.x, game.position.y);
    travel_tune(
        game.character.mount == Mount::Fly,
        game.travel_mode == TravelMode::Boat || game.character.mount == Mount::Sail,
        tile.map_or(Terrain::Grass, |t| t.terrain),
        tile.is_some_and(|t| t.is_bridge),
    )
}

/// The few combat fields that tell which sound an action made, captured
/// before and after one action. Comparing two snapshots covers the player
/// and the AI alike, without the engine raising anything.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CombatSnap {
    flashes: [[i32; COMBAT_SLOTS]; COMBAT_SIDES],
    shots: [[i32; COMBAT_SLOTS]; COMBAT_SIDES],
    dead: usize,
    spells: i32,
}

impl CombatSnap {
    pub fn of(combat: &Combat) -> Self {
        let mut snap = CombatSnap {
            spells: combat.spells_this_round,
            ..Default::default()
        };
        for side in 0..COMBAT_SIDES {
            for slot in 0..COMBAT_SLOTS {
                let unit = &combat.units[side][slot];
                snap.flashes[side][slot] = unit.hit_flash;
                snap.shots[side][slot] = unit.shots;
                snap.dead += usize::from(unit.dead);
            }
        }
        snap
    }
}

/// The sounds one combat action made, read from the state before and after
/// it (and before `combat_compact` clears the dead flags). Slot 0 is the
/// action -- a spell, else a shot, else a blow -- and slot 1 a stack dying.
/// A spell that kills reads as the spell, a shot as the loose rather than
/// the landing, and a blow an IMMUNE target shrugs off leaves no flash and
/// no sound.
pub fn combat_tunes(before: &CombatSnap, after: &CombatSnap) -> [Option<Tune>; 2] {
    let cells = || (0..COMBAT_SIDES).flat_map(|s| (0..COMBAT_SLOTS).map(move |i| (s, i)));
    let shot = cells().any(|(s, i)| after.shots[s][i] < before.shots[s][i]);
    // `combat_hit_unit` stamps a fresh flash of 3; the animation only
    // decays it, so a rise is a new hit.
    let hit = cells().any(|(s, i)| after.flashes[s][i] > before.flashes[s][i]);
    let action = if after.spells > before.spells {
        Some(Tune::Spell)
    } else if shot {
        Some(Tune::Shoot)
    } else if hit {
        Some(Tune::Hit)
    } else {
        None
    };
    [action, (after.dead > before.dead).then_some(Tune::Slain)]
}

/// The background music selection, C's `AudioTrack`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Track {
    #[default]
    None,
    Openworld,
    Combat,
}

/// Where the session's audio decisions go: `audio_backend::KiraAudio` plays
/// them, [`Silent`] (the headless playtest) drops them.
pub trait AudioOut {
    fn is_available(&self) -> bool;
    fn set_sounds_enabled(&mut self, on: bool);
    fn set_music_enabled(&mut self, on: bool);
    fn set_track(&mut self, track: Track);
    fn set_master_volume(&mut self, value: i32);
    fn play_tune(&mut self, tune: Tune);
    fn tick(&mut self);
}

/// No device and no sound, but reported as available so the Controls panel
/// and the saved options behave as they do with speakers.
#[derive(Debug, Default)]
pub struct Silent;

impl AudioOut for Silent {
    fn is_available(&self) -> bool {
        true
    }
    fn set_sounds_enabled(&mut self, _on: bool) {}
    fn set_music_enabled(&mut self, _on: bool) {}
    fn set_track(&mut self, _track: Track) {}
    fn set_master_volume(&mut self, _value: i32) {}
    fn play_tune(&mut self, _tune: Tune) {}
    fn tick(&mut self) {}
}

/// Anything at or below this is inaudible. kira has its own `Decibels::SILENCE`;
/// this module stays library-free, so it names its own floor.
pub const SILENCE_DB: f32 = -100.0;

/// C's `MUSIC_HEADROOM`, 0.50 linear.
pub const MUSIC_HEADROOM_DB: f32 = -6.02;
/// C's `SFX_HEADROOM`, 0.80 linear.
pub const SFX_HEADROOM_DB: f32 = -1.94;
/// C's `DUCK_FACTOR`, 0.20 linear.
pub const DUCK_FACTOR_DB: f32 = -13.98;

/// C smooths the duck by 0.40 per frame and releases at 0.10. At 60fps those
/// reach ~90% in about 5 and 22 frames, so they become these tween durations.
/// The timing matches; the envelope SHAPE does not. See `specs/audio-design.md`.
pub const DUCK_ATTACK_MS: u64 = 80;
pub const DUCK_RELEASE_MS: u64 = 370;

/// `stats.options` slots this reads. The pack's `controls.settings` declares
/// them in this order, so the order is load-bearing -- C indexes them the same
/// way at `main.c:904-906`.
pub const OPTION_SOUNDS: usize = 1;
pub const OPTION_MUSIC: usize = 5;
pub const OPTION_VOLUME: usize = 6;

/// With no playback device, C pins Sounds / Music / Volume to 0 so the
/// Controls panel and the live audio push agree (`main.c:896-902`).
pub fn pin_options_without_device(options: &mut [i32]) {
    for slot in [OPTION_SOUNDS, OPTION_MUSIC, OPTION_VOLUME] {
        if let Some(value) = options.get_mut(slot) {
            *value = 0;
        }
    }
}

/// The tunes one engine step produces, in C's firing order.
///
/// C raises these from inside the engine (`step.c:43`, `step.c:88`,
/// `step.c:424`, `game.c:1149`); this port derives them from what the engine
/// returns, so the engine needs no audio callback. Slot 0 is the bump or the
/// chest, which fire inside `GameStep`; slot 1 is the walk tune, which fires
/// at the end of `GameOnStep`.
///
/// `absorbed` is whether `GameOnStep` returned early -- a time stop was
/// banked, or the game was over -- sampled BEFORE the step, since the step
/// itself spends the time stop.
pub fn tunes_for_step(result: &StepResult, absorbed: bool) -> [Option<Tune>; 2] {
    match result {
        StepResult::NoDirection => [None, None],
        StepResult::BlockedEdge | StepResult::BlockedTerrain { .. } => [Some(Tune::Bump), None],
        StepResult::Moved {
            interaction,
            bounced,
            ..
        } => {
            // The chest is C's; a find is the port's own.
            let first = match interaction {
                StepInteraction::TreasureChest { .. } => Some(Tune::Chest),
                StepInteraction::Artifact { .. }
                | StepInteraction::Orb { .. }
                | StepInteraction::Navmap { .. } => Some(Tune::Discover),
                _ => None,
            };
            // A bounced step skips `GameOnStep`, and with it the walk tune.
            [
                first,
                if *bounced {
                    None
                } else {
                    on_step_tune(absorbed)
                },
            ]
        }
    }
}

/// The tune `GameOnStep` itself plays, `game.c:1149`: the walk tune, unless
/// the step was absorbed. Resting reaches `GameOnStep` directly
/// (`shell_actions.c:212`), so a rest plays it too.
pub fn on_step_tune(absorbed: bool) -> Option<Tune> {
    (!absorbed).then_some(Tune::Walk)
}

/// C's 0..9 master slider as decibels. 0 is silence, 9 is unity; out-of-range
/// values clamp, as `audio_set_master_volume` does.
pub fn slider_to_db(value: i32) -> f32 {
    if value <= 0 {
        return SILENCE_DB;
    }
    let fraction = value.min(9) as f32 / 9.0;
    20.0 * fraction.log10()
}

/// One change to the music's duck gain: head for `target_db` over `tween_ms`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuckMove {
    pub target_db: f32,
    pub tween_ms: u64,
}

/// The duck half of `audio_tick`: music dips while any tune plays.
///
/// C re-smooths a level every frame; kira tweens, so this only reports the
/// frames where the TARGET changes.
#[derive(Clone, Copy, Debug, Default)]
pub struct Duck {
    ducked: bool,
}

impl Duck {
    pub fn update(&mut self, any_tune_playing: bool) -> Option<DuckMove> {
        if any_tune_playing == self.ducked {
            return None;
        }
        self.ducked = any_tune_playing;
        Some(if any_tune_playing {
            DuckMove {
                target_db: DUCK_FACTOR_DB,
                tween_ms: DUCK_ATTACK_MS,
            }
        } else {
            DuckMove {
                target_db: 0.0,
                tween_ms: DUCK_RELEASE_MS,
            }
        })
    }

    /// The level the duck is heading for, so a track started mid-duck joins
    /// it rather than starting loud.
    pub fn level_db(&self) -> f32 {
        if self.ducked {
            DUCK_FACTOR_DB
        } else {
            0.0
        }
    }
}

/// What the backend must do to the music stream. `stop` comes first.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MusicCmd {
    pub stop: bool,
    /// Start this track from the top.
    pub start: Option<Track>,
}

/// The music half of `audio.c`: the active selection and the toggle.
#[derive(Clone, Copy, Debug)]
pub struct Music {
    enabled: bool,
    active: Track,
}

impl Default for Music {
    /// C's `s_music_enabled = true`, `s_active_track = AUDIO_TRACK_NONE`.
    fn default() -> Self {
        Self {
            enabled: true,
            active: Track::None,
        }
    }
}

impl Music {
    pub fn active(&self) -> Track {
        self.active
    }

    /// `audio_set_track`: a hard cut, and a selection made while music is
    /// off is recorded but not started.
    pub fn set_track(&mut self, track: Track) -> MusicCmd {
        if track == self.active {
            return MusicCmd::default();
        }
        self.active = track;
        MusicCmd {
            stop: true,
            start: (self.enabled && track != Track::None).then_some(track),
        }
    }

    /// `audio_set_music_enabled`. Off stops the stream; on starts the active
    /// track from the top, because C's `StopMusicStream` rewinds.
    pub fn set_enabled(&mut self, on: bool) -> MusicCmd {
        if on == self.enabled {
            return MusicCmd::default();
        }
        self.enabled = on;
        if on {
            MusicCmd {
                stop: false,
                start: (self.active != Track::None).then_some(self.active),
            }
        } else {
            MusicCmd {
                stop: true,
                start: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::game::ChestRoll;
    use openbounty_engine::map::Tile;
    use openbounty_engine::step::StepAdvance;
    use openbounty_engine::tile::{Interact, Terrain};

    fn moved(interaction: StepInteraction) -> StepResult {
        moved_bounced(interaction, false)
    }

    fn moved_bounced(interaction: StepInteraction, bounced: bool) -> StepResult {
        StepResult::Moved {
            terrain: Terrain::Grass,
            move_cost: 1,
            advance: StepAdvance::default(),
            interaction,
            collision: None,
            bounced,
        }
    }

    #[test]
    fn a_completed_step_plays_the_walk_tune() {
        assert_eq!(
            tunes_for_step(&moved(StepInteraction::None), false),
            [None, Some(Tune::Walk)]
        );
    }

    #[test]
    fn both_blocked_paths_play_the_bump_tune() {
        assert_eq!(
            tunes_for_step(&StepResult::BlockedEdge, false),
            [Some(Tune::Bump), None]
        );
        assert_eq!(
            tunes_for_step(
                &StepResult::BlockedTerrain {
                    terrain: Terrain::Water
                },
                false
            ),
            [Some(Tune::Bump), None]
        );
    }

    /// C's bump sites both sit PAST the direction check, and `step.c:38` says
    /// the blocked trace exists to distinguish "no move attempted" from "move
    /// attempted and blocked". A rejected key is not a bump.
    #[test]
    fn no_direction_plays_nothing() {
        assert_eq!(
            tunes_for_step(&StepResult::NoDirection, false),
            [None, None]
        );
    }

    /// The plan said the chest tune REPLACES the walk tune. C's code does not:
    /// `step.c:424` plays the chest, the step is not bounced, and `GameOnStep`
    /// then reaches `game.c:1149`. Both play, chest first.
    #[test]
    fn opening_a_chest_plays_the_chest_tune_then_the_walk_tune() {
        let result = moved(StepInteraction::TreasureChest {
            id: "chest0".to_owned(),
            roll: ChestRoll::default(),
        });
        assert_eq!(
            tunes_for_step(&result, false),
            [Some(Tune::Chest), Some(Tune::Walk)]
        );
    }

    /// Towns, castle gates, dwellings, the alcove and a hostile foe set
    /// `bounce_back`, and a bounced step never calls `GameOnStep`, so C plays
    /// no walk tune.
    #[test]
    fn a_step_c_bounces_plays_no_walk_tune() {
        for interaction in [
            StepInteraction::Town {
                id: "t".to_owned(),
                boat_x: 0,
                boat_y: 0,
            },
            StepInteraction::CastleGate { id: "c".to_owned() },
            StepInteraction::Dwelling {
                id: "d".to_owned(),
                kind: Interact::DwellingPlains,
                troop_id: String::new(),
                count: 0,
            },
            StepInteraction::Alcove,
            StepInteraction::Foe {
                id: "f".to_owned(),
                friendly: false,
            },
        ] {
            assert_eq!(
                tunes_for_step(&moved_bounced(interaction.clone(), true), false),
                [None, None],
                "{interaction:?}"
            );
        }
    }

    /// A friendly foe opens the recruit flow WITHOUT bouncing.
    #[test]
    fn stepping_onto_a_friendly_foe_still_walks() {
        let result = moved(StepInteraction::Foe {
            id: "f".to_owned(),
            friendly: true,
        });
        assert_eq!(tunes_for_step(&result, false), [None, Some(Tune::Walk)]);
    }

    /// `GameOnStep` returns before `game.c:1149` while a time stop is banked
    /// or the game is over, so an absorbed step is silent. The chest still
    /// sounds: it fires in `step.c` before `GameOnStep` is reached.
    #[test]
    fn a_step_absorbed_by_time_stop_plays_no_walk_tune() {
        assert_eq!(
            tunes_for_step(&moved(StepInteraction::None), true),
            [None, None]
        );
        let chest = moved(StepInteraction::TreasureChest {
            id: "chest0".to_owned(),
            roll: ChestRoll::default(),
        });
        assert_eq!(tunes_for_step(&chest, true), [Some(Tune::Chest), None]);
    }

    #[test]
    fn no_device_pins_only_the_three_audio_options_to_zero() {
        let mut options = [1, 1, 1, 1, 1, 1, 9];
        pin_options_without_device(&mut options);
        assert_eq!(options, [1, 0, 1, 1, 1, 0, 0]);
    }

    #[test]
    fn resting_plays_the_walk_tune_unless_absorbed() {
        assert_eq!(on_step_tune(false), Some(Tune::Walk));
        assert_eq!(on_step_tune(true), None);
    }

    /// Zero must be silence, not unity gain. This is the boundary that would
    /// make a muted game loud.
    #[test]
    fn the_slider_maps_zero_to_silence_and_nine_to_unity() {
        assert!(slider_to_db(0) <= SILENCE_DB);
        assert!((slider_to_db(9) - 0.0).abs() < 0.001);
    }

    #[test]
    fn the_slider_clamps_above_nine_rather_than_exceeding_unity() {
        assert!((slider_to_db(99) - 0.0).abs() < 0.001);
        assert!(slider_to_db(-5) <= SILENCE_DB);
    }

    #[test]
    fn a_midpoint_slider_attenuates_without_silencing() {
        let db = slider_to_db(5);
        assert!(db < 0.0, "5/9 must attenuate");
        assert!(db > SILENCE_DB, "5/9 must not be silent");
    }

    /// C's linear constants, converted once. 20*log10(x).
    #[test]
    fn the_gain_constants_match_the_c_linear_values() {
        assert!(
            (MUSIC_HEADROOM_DB - (-6.02)).abs() < 0.01,
            "C MUSIC_HEADROOM 0.50"
        );
        assert!(
            (SFX_HEADROOM_DB - (-1.94)).abs() < 0.01,
            "C SFX_HEADROOM 0.80"
        );
        assert!(
            (DUCK_FACTOR_DB - (-13.98)).abs() < 0.01,
            "C DUCK_FACTOR 0.20"
        );
    }

    /// `audio_tick`: while any tune plays, music heads for DUCK_FACTOR on the
    /// fast attack, and recovers to unity on the slow release once it ends.
    #[test]
    fn a_tune_ducks_the_music_and_it_recovers_afterwards() {
        let mut duck = Duck::default();
        assert_eq!(duck.update(false), None, "idle music is not ducked");
        assert_eq!(
            duck.update(true),
            Some(DuckMove {
                target_db: DUCK_FACTOR_DB,
                tween_ms: DUCK_ATTACK_MS
            })
        );
        assert_eq!(duck.update(true), None, "already heading down");
        assert_eq!(duck.level_db(), DUCK_FACTOR_DB);
        assert_eq!(
            duck.update(false),
            Some(DuckMove {
                target_db: 0.0,
                tween_ms: DUCK_RELEASE_MS
            })
        );
        assert_eq!(duck.level_db(), 0.0);
    }

    /// `audio_set_track` stops the old track and starts the new, a hard cut.
    #[test]
    fn switching_track_stops_the_old_and_starts_the_new() {
        let mut music = Music::default();
        assert_eq!(
            music.set_track(Track::Openworld),
            MusicCmd {
                stop: true,
                start: Some(Track::Openworld)
            }
        );
        assert_eq!(
            music.set_track(Track::Combat),
            MusicCmd {
                stop: true,
                start: Some(Track::Combat)
            }
        );
        assert_eq!(music.active(), Track::Combat);
    }

    #[test]
    fn reselecting_the_active_track_does_nothing() {
        let mut music = Music::default();
        music.set_track(Track::Openworld);
        assert_eq!(music.set_track(Track::Openworld), MusicCmd::default());
    }

    #[test]
    fn track_none_silences_music_without_starting_anything() {
        let mut music = Music::default();
        music.set_track(Track::Openworld);
        assert_eq!(
            music.set_track(Track::None),
            MusicCmd {
                stop: true,
                start: None
            }
        );
    }

    /// `audio.h` says toggling music back on RESUMES. `audio.c` calls
    /// `StopMusicStream`, which rewinds, then `PlayMusicStream` from the top.
    /// Ported from the code: the track restarts.
    #[test]
    fn music_off_then_on_restarts_the_track_despite_the_header_comment() {
        let mut music = Music::default();
        music.set_track(Track::Openworld);
        assert_eq!(
            music.set_enabled(false),
            MusicCmd {
                stop: true,
                start: None
            }
        );
        assert_eq!(
            music.set_enabled(true),
            MusicCmd {
                stop: false,
                start: Some(Track::Openworld)
            }
        );
    }

    /// The options are pushed every frame, so an unchanged toggle must not
    /// restart the track 60 times a second. C's `if (s_music_enabled == on)`.
    #[test]
    fn an_unchanged_music_toggle_does_nothing() {
        let mut music = Music::default();
        music.set_track(Track::Openworld);
        assert_eq!(music.set_enabled(true), MusicCmd::default());
    }

    /// With music off, `audio_set_track` records the selection and starts
    /// nothing; turning music on then starts the recorded track.
    #[test]
    fn a_track_chosen_while_music_is_off_waits_for_the_toggle() {
        let mut music = Music::default();
        music.set_enabled(false);
        assert_eq!(
            music.set_track(Track::Combat),
            MusicCmd {
                stop: true,
                start: None
            }
        );
        assert_eq!(
            music.set_enabled(true),
            MusicCmd {
                stop: false,
                start: Some(Track::Combat)
            }
        );
    }

    #[test]
    fn the_tune_list_is_in_index_order() {
        for (i, tune) in Tune::ALL.iter().enumerate() {
            assert_eq!(tune.index(), i);
        }
    }

    /// C ducks the music for its four tunes, and the walk's variants are the
    /// walk; the combat blows and the taps would pump it.
    #[test]
    fn only_the_combat_blows_and_taps_skip_the_duck() {
        let quiet: Vec<Tune> = Tune::ALL.into_iter().filter(|t| !t.ducks()).collect();
        assert_eq!(
            quiet,
            [
                Tune::Hit,
                Tune::Shoot,
                Tune::Slain,
                Tune::Spell,
                Tune::SpellClone,
                Tune::SpellTeleport,
                Tune::SpellFireball,
                Tune::SpellLightning,
                Tune::SpellFreeze,
                Tune::SpellResurrect,
                Tune::SpellTurnUndead,
                Tune::Tap,
            ]
        );
    }

    fn snaps(edit: impl FnOnce(&mut Combat)) -> (CombatSnap, CombatSnap) {
        let mut combat = Box::<Combat>::default();
        combat.units[0][0].shots = 2;
        let before = CombatSnap::of(&combat);
        edit(&mut combat);
        (before, CombatSnap::of(&combat))
    }

    #[test]
    fn nothing_changed_is_silent() {
        let (a, b) = snaps(|_| {});
        assert_eq!(combat_tunes(&a, &b), [None, None]);
    }

    #[test]
    fn a_fresh_flash_is_a_hit_and_a_fading_one_is_not() {
        let (a, b) = snaps(|c| c.units[1][2].hit_flash = 3);
        assert_eq!(combat_tunes(&a, &b), [Some(Tune::Hit), None]);
        let (a, b) = snaps(|c| c.units[1][2].hit_flash = 3);
        assert_eq!(combat_tunes(&b, &a), [None, None]);
    }

    #[test]
    fn a_spent_shot_outranks_the_flash_it_caused() {
        let (a, b) = snaps(|c| {
            c.units[0][0].shots = 1;
            c.units[1][0].hit_flash = 3;
        });
        assert_eq!(combat_tunes(&a, &b), [Some(Tune::Shoot), None]);
    }

    #[test]
    fn a_spell_outranks_everything_and_a_kill_adds_slain() {
        let (a, b) = snaps(|c| {
            c.spells_this_round = 1;
            c.units[1][0].hit_flash = 3;
            c.units[1][0].dead = true;
        });
        assert_eq!(combat_tunes(&a, &b), [Some(Tune::Spell), Some(Tune::Slain)]);
    }

    /// Artifacts, orbs and navigation maps play the find sound where a chest
    /// plays its own, and the ride on still ticks.
    #[test]
    fn picking_up_a_find_plays_the_discover_tune() {
        for interaction in [
            StepInteraction::Artifact { id: "a".to_owned() },
            StepInteraction::Orb {
                id: "o".to_owned(),
                zone_index: None,
            },
            StepInteraction::Navmap {
                id: "n".to_owned(),
                discovered_zone: None,
            },
        ] {
            assert_eq!(
                tunes_for_step(&moved(interaction), false),
                [Some(Tune::Discover), Some(Tune::Walk)]
            );
        }
    }

    #[test]
    fn the_walk_tune_follows_the_mount_then_the_ground() {
        use Terrain::{Desert, Grass, Water};
        // flying, sailing, terrain, bridge
        assert_eq!(travel_tune(false, false, Grass, false), Tune::Walk);
        assert_eq!(travel_tune(false, false, Desert, false), Tune::WalkDesert);
        assert_eq!(travel_tune(false, false, Water, true), Tune::WalkBridge);
        assert_eq!(travel_tune(false, true, Water, false), Tune::Sail);
        assert_eq!(travel_tune(true, false, Desert, true), Tune::Fly);
    }

    #[test]
    fn on_ground_reads_the_hero_tile_and_leaves_other_tunes_alone() {
        let mut tiles = vec![Tile::default(); 3];
        tiles[1].terrain = Terrain::Desert;
        tiles[2].terrain = Terrain::Water;
        tiles[2].is_bridge = true;
        let map = Map::from_tiles(3, 1, tiles).expect("rectangular");
        let mut game = Box::<Game>::default();
        for (x, tune) in [
            (0, Tune::Walk),
            (1, Tune::WalkDesert),
            (2, Tune::WalkBridge),
        ] {
            game.position.x = x;
            assert_eq!(on_ground(Tune::Walk, &game, &map), tune);
        }
        game.travel_mode = TravelMode::Boat;
        assert_eq!(on_ground(Tune::Walk, &game, &map), Tune::Sail);
        game.character.mount = Mount::Fly;
        assert_eq!(on_ground(Tune::Walk, &game, &map), Tune::Fly);
        assert_eq!(on_ground(Tune::Chest, &game, &map), Tune::Chest);
    }

    /// An older pack without the variants keeps ticking on the plain walk,
    /// and casting on the generic spell sound.
    #[test]
    fn only_the_walk_variants_fall_back_and_only_to_walk() {
        for tune in Tune::ALL {
            let expected = match tune {
                Tune::WalkDesert | Tune::WalkBridge | Tune::Sail | Tune::Fly => Some(Tune::Walk),
                Tune::SpellClone
                | Tune::SpellTeleport
                | Tune::SpellFireball
                | Tune::SpellLightning
                | Tune::SpellFreeze
                | Tune::SpellResurrect
                | Tune::SpellTurnUndead => Some(Tune::Spell),
                _ => None,
            };
            assert_eq!(tune.fallback(), expected, "{tune:?}");
        }
    }

    /// Each stock combat spell has its own sound, in `COMBAT_SPELL_*` order;
    /// an id past them is the generic one.
    #[test]
    fn each_combat_spell_has_its_own_sound() {
        let own: Vec<Tune> = (0..7).map(Tune::for_spell).collect();
        assert_eq!(own, Tune::ALL[15..22]);
        assert_eq!(Tune::for_spell(7), Tune::Spell);
        assert_eq!(Tune::for_spell(-1), Tune::Spell);
    }

    /// Buttons tick; the sheet's dead zone, a map tile and a combat cell
    /// do not (the last two make their own sounds).
    #[test]
    fn buttons_tick_and_moves_do_not() {
        assert!(tap_sounds(HitTarget::PromptYes));
        assert!(tap_sounds(HitTarget::SheetNext));
        assert!(tap_sounds(HitTarget::DialogDismiss));
        assert!(!tap_sounds(HitTarget::SheetBlank));
        assert!(!tap_sounds(HitTarget::MapTile(3, 4)));
        assert!(!tap_sounds(HitTarget::CombatCell(1, 1)));
    }
}
