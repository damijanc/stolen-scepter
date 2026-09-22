# Audio Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the shell sound -- C's four one-shot tunes and two music tracks,
driven by the Controls rows that currently toggle nothing.

**Architecture:** `shell/src/audio.rs` holds the vocabulary and every decision
and knows nothing about any audio library, so it tests with no device.
`shell/src/audio_backend.rs` is the only module that knows `kira` exists. This
is the `draw.rs` / `backend.rs` seam applied to a subsystem with the same
problem: a test cannot listen. The engine gains no dependency and no callback.

**Tech Stack:** Rust, `kira = "0.12"` (new, `shell` only), the existing `engine`
and `shell` crates.

**READ FIRST:** `specs/audio-design.md`. It carries the decisions, the rejected
alternatives and the C citations behind every choice here. This plan is the
how; that document is the why, and a reviewer needs both.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a tested task, report it, ask. Do not
  chain commits.
- **Commit messages are a single subject line.** No body, no bullets, no
  `Co-Authored-By` or other trailers. Example: `feat: parse the pack audio block`.
- **Never add a dependency to `engine/Cargo.toml`.** `kira` is a `shell`
  dependency only. `engine/tests/render_free.rs` enforces the boundary and must
  keep passing.
- **Never commit `reference/`, `game-assets/`, or build output.**
- Preserve unrelated working-tree changes. Do not reset, checkout, or clean.
  `tools/gen_ai.py` is already modified at the time of writing.
- Run `cargo fmt --all --check` before every commit.
- Transliterate C first. Where a C comment and the C code disagree, port the
  code and say so in the test name.

## File Structure

- **Modify `engine/src/resources.rs`**: add `ResAudio` and `parse_audio`. Paths
  only -- the engine never plays anything. This mirrors `ResSprites`, whose own
  doc comment says *"Nothing in the engine reads any of this; it is recorded
  here because `Resources` is the pack reader."*
- **Create `shell/src/audio.rs`** (~200 lines incl. tests): `Tune`, `Track`,
  the gain constants, `tune_for_step`, `slider_to_db`, and `AudioPolicy`. No
  `kira` import. One responsibility: deciding what should be heard.
- **Create `shell/src/audio_backend.rs`** (~150 lines): `KiraAudio`. The only
  module that imports `kira`. One responsibility: making it audible.
- **Modify `shell/src/Cargo.toml`**: add `kira`.
- **Modify `shell/src/views.rs`**: `audio_available()` stops being a stub.
- **Modify `shell/src/main.rs`**: construction, the per-frame options push, and
  the event call sites.
- **Modify `packs/open/CHECKLIST.md` + `tools/open_pack.py`**: the content
  contract (Task 6).

## Out of scope -- do not build these

- **`options[2]` walk_beep and `options[4]` cga.** Parsed, drawn, toggleable,
  and **C reads neither** -- the only readers of `stats.options` in the C tree
  are `[0]` Delay, `[1]` Sounds, `[3]` Animation, `[5]` Music, `[6]` Volume.
  DEAD. Do not wire them.
- **New event types.** C has four tunes and two tracks. Combat hits, spell
  casts and UI clicks are a later slice.
- **Crossfade.** C hard-cuts; so does this.
- **Audio files.** Sourcing is the user's, on their schedule. Every task below
  passes with no audio files and no audio device present.

---

### Task 1: Parse the pack's audio block

**Files:**
- Modify: `engine/src/resources.rs`
- Test: `engine/src/resources.rs` (inline `#[cfg(test)]`, the crate's convention)

**Interfaces:**
- Consumes: `FixedStr<RES_PATH_LEN>` (`RES_PATH_LEN = 128`), `json_str_or`.
- Produces: `ResAudio` with public fields `track_openworld`, `track_combat`,
  `tune_walk`, `tune_bump`, `tune_chest`, `tune_defeat`, all
  `FixedStr<RES_PATH_LEN>`; `Resources.audio: ResAudio`.

- [ ] **Step 1: Write the failing test**

Add to `engine/src/resources.rs`'s test module:

```rust
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
        let resources =
            Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
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
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p openbounty-engine --lib audio 2>&1 | head -20`
Expected: FAIL to compile, `no field 'audio' on type 'Resources'`.

- [ ] **Step 3: Add the struct**

In `engine/src/resources.rs`, beside `ResSprites`:

```rust
/// The pack's audio manifest: two music tracks and four one-shot tunes.
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
```

Add the field to `Resources`, beside `pub sprites: Box<ResSprites>,`:

```rust
    pub audio: ResAudio,
```

`Resources` builds through `Self::default()`, and `ResAudio` derives `Default`,
so no other construction site changes.

- [ ] **Step 4: Add the parser**

Add the method beside `parse_sprites`:

```rust
    /// Read the audio manifest. Absent keys leave empty slots; a pack that
    /// ships no audio is legal, as is one that ships only some of it.
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
```

Call it in `from_json_str`'s dispatch, immediately after
`resources.parse_sprites(&root);`:

```rust
        resources.parse_audio(&root);
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-engine --lib audio`
Expected: PASS, 3 tests.

- [ ] **Step 6: Confirm the boundary and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass, including `render_free`.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add engine/src/resources.rs
git commit -m "feat: parse the pack audio block"
```

---

### Task 2: The audio policy module

**Files:**
- Create: `shell/src/audio.rs`
- Modify: `shell/src/main.rs` (add `pub mod audio;`)
- Test: `shell/src/audio.rs`

**Interfaces:**
- Consumes: `openbounty_engine::step::{StepResult, StepInteraction}`.
- Produces: `audio::Tune` (`Walk`, `Bump`, `Chest`, `Defeat`), `audio::Track`
  (`None`, `Openworld`, `Combat`), `audio::tune_for_step(&StepResult) ->
  Option<Tune>`, `audio::slider_to_db(i32) -> f32`, and the constants
  `MUSIC_HEADROOM_DB`, `SFX_HEADROOM_DB`, `DUCK_FACTOR_DB`,
  `DUCK_ATTACK_MS`, `DUCK_RELEASE_MS`. `slider_to_db` returns a plain `f32`
  so this module stays free of `kira`; Task 3 wraps it in `Decibels`.

- [ ] **Step 1: Register the module**

In `shell/src/main.rs`, add to the module list (alphabetical, before `backend`):

```rust
pub mod audio;
```

- [ ] **Step 2: Write the failing tests**

Create `shell/src/audio.rs` containing ONLY this test module. It reaches
everything through `use super::*`, and `super` is empty until Step 3, so the
crate will not compile after this step. That is the point.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::map::Terrain;

    #[test]
    fn a_completed_step_plays_the_walk_tune() {
        let result = StepResult::Moved {
            terrain: Terrain::Grass,
            move_cost: 1,
            advance: Default::default(),
            interaction: StepInteraction::None,
            collision: None,
        };
        assert_eq!(tune_for_step(&result), Some(Tune::Walk));
    }

    #[test]
    fn both_blocked_paths_play_the_bump_tune() {
        assert_eq!(tune_for_step(&StepResult::BlockedEdge), Some(Tune::Bump));
        assert_eq!(
            tune_for_step(&StepResult::BlockedTerrain { terrain: Terrain::Water }),
            Some(Tune::Bump)
        );
    }

    /// C's bump sites both sit PAST the direction check, and `step.c:38` says
    /// the blocked trace exists to distinguish "no move attempted" from "move
    /// attempted and blocked". A rejected key is not a bump.
    #[test]
    fn no_direction_plays_nothing() {
        assert_eq!(tune_for_step(&StepResult::NoDirection), None);
    }

    /// The chest tune rides on a completed step, so it REPLACES the walk tune
    /// rather than stacking with it -- C reaches `step.c:424` and returns
    /// before `game.c:1149` fires.
    #[test]
    fn opening_a_chest_plays_the_chest_tune_not_the_walk_tune() {
        let result = StepResult::Moved {
            terrain: Terrain::Grass,
            move_cost: 1,
            advance: Default::default(),
            interaction: StepInteraction::TreasureChest {
                id: "chest0".to_owned(),
                roll: Default::default(),
            },
            collision: None,
        };
        assert_eq!(tune_for_step(&result), Some(Tune::Chest));
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
        assert!((MUSIC_HEADROOM_DB - (-6.02)).abs() < 0.01, "C MUSIC_HEADROOM 0.50");
        assert!((SFX_HEADROOM_DB - (-1.94)).abs() < 0.01, "C SFX_HEADROOM 0.80");
        assert!((DUCK_FACTOR_DB - (-13.98)).abs() < 0.01, "C DUCK_FACTOR 0.20");
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell audio:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find function 'tune_for_step'` among the
errors (every name reached through `super::*` is unresolved, so expect several).

- [ ] **Step 4: Write the implementation**

Prepend to `shell/src/audio.rs`:

```rust
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

use openbounty_engine::step::{StepInteraction, StepResult};

/// The four one-shot events, C's `AudioTuneId`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tune {
    Walk,
    Bump,
    Chest,
    Defeat,
}

/// The background music selection, C's `AudioTrack`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Track {
    #[default]
    None,
    Openworld,
    Combat,
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

/// The tune a completed engine step should produce, if any.
///
/// C raises these from inside the engine (`game.c:1149`, `step.c:43`,
/// `step.c:88`, `step.c:424`); this port derives them from what the engine
/// returns, so the engine needs no audio callback.
pub fn tune_for_step(result: &StepResult) -> Option<Tune> {
    match result {
        StepResult::NoDirection => None,
        StepResult::BlockedEdge | StepResult::BlockedTerrain { .. } => Some(Tune::Bump),
        StepResult::Moved { interaction, .. } => match interaction {
            // C returns at the chest before reaching the walk tune, so the
            // chest REPLACES the step sound rather than layering over it.
            StepInteraction::TreasureChest { .. } => Some(Tune::Chest),
            _ => Some(Tune::Walk),
        },
    }
}

/// C's 0..9 master slider as decibels. 0 is silence, 9 is unity.
pub fn slider_to_db(value: i32) -> f32 {
    if value <= 0 {
        return SILENCE_DB;
    }
    let fraction = value.min(9) as f32 / 9.0;
    20.0 * fraction.log10()
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell audio::`
Expected: PASS, 8 tests.

If `StepAdvance` or `ChestRoll` does not implement `Default`, the two
`Default::default()` calls in the tests will not compile. Construct the real
value instead of deriving `Default` on an engine type -- the engine's shape is
not this task's to change.

- [ ] **Step 6: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/src/audio.rs shell/src/main.rs
git commit -m "feat: add the shell audio policy"
```

---

### Task 3: The kira backend

**Files:**
- Modify: `shell/Cargo.toml`
- Create: `shell/src/audio_backend.rs`
- Modify: `shell/src/main.rs` (add `pub mod audio_backend;`)
- Test: `shell/src/audio_backend.rs`

**Interfaces:**
- Consumes: everything Task 2 produced.
- Produces: `audio_backend::KiraAudio` with `new(&ResAudio, &Path) -> KiraAudio`,
  `is_available(&self) -> bool`, `play_tune(&mut self, Tune)`,
  `set_track(&mut self, Track)`, `set_sounds_enabled(&mut self, bool)`,
  `set_music_enabled(&mut self, bool)`, `set_master_volume(&mut self, i32)`.

**Verified API.** Every kira call below was compiled against 0.12.4 before this
plan was written. `AudioManager::new` returns `Result`, so a missing device is
an `Err` -- unlike raylib, no stderr redirection is needed.

- [ ] **Step 1: Add the dependency**

In `shell/Cargo.toml`, under `[dependencies]`:

```toml
kira = "0.12"
```

Run: `cargo build -p openbounty-shell`
Expected: kira and symphonia compile. On macOS this also pulls `coreaudio-rs`
and the `objc2` audio crates; that is expected and takes under a minute.

- [ ] **Step 2: Write the failing test**

Create `shell/src/audio_backend.rs` with ONLY this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::resources::ResAudio;

    /// The smoke test must pass on a machine with NO audio device -- CI boxes
    /// have none. Construction never panics and never fails the run; it
    /// degrades to silent and says so through `is_available`.
    #[test]
    fn construction_survives_a_machine_with_no_device() {
        let audio = KiraAudio::new(&ResAudio::default(), Path::new("."));
        // Either answer is legal; the point is that it returned at all.
        let _ = audio.is_available();
    }

    /// Every call is a no-op when no device opened, rather than a panic.
    #[test]
    fn playback_calls_are_safe_without_a_device() {
        let mut audio = KiraAudio::new(&ResAudio::default(), Path::new("."));
        audio.set_master_volume(5);
        audio.set_sounds_enabled(true);
        audio.set_music_enabled(true);
        audio.play_tune(Tune::Walk);
        audio.set_track(Track::Openworld);
        audio.set_track(Track::None);
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test -p openbounty-shell audio_backend:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find struct ... 'KiraAudio'`.

- [ ] **Step 4: Write the implementation**

Prepend to `shell/src/audio_backend.rs`:

```rust
//! The only module that knows `kira` exists.
//!
//! `backend.rs` is the only module that knows macroquad exists; this is its
//! sibling for audio. Keep every `kira` type behind this boundary so
//! `audio.rs` stays testable with no device.
//!
//! A machine with no playback device is not an error. `AudioManager::new`
//! returns `Err`, this stores `None`, and every call becomes a no-op -- which
//! is what C achieves with an `IsAudioDeviceReady` check plus an stderr
//! redirect to hide ALSA's noise.

use std::path::{Path, PathBuf};
use std::time::Duration;

use kira::sound::static_sound::StaticSoundData;
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Tween,
};

use openbounty_engine::resources::ResAudio;

use crate::audio::{
    slider_to_db, Track, Tune, DUCK_ATTACK_MS, DUCK_FACTOR_DB, DUCK_RELEASE_MS,
    MUSIC_HEADROOM_DB, SFX_HEADROOM_DB, SILENCE_DB,
};

/// The live device, absent when none opened.
struct Device {
    manager: AudioManager<DefaultBackend>,
    music_track: TrackHandle,
    sfx_track: TrackHandle,
    music: Option<StreamingSoundHandle<kira::sound::FromFileError>>,
}

pub struct KiraAudio {
    device: Option<Device>,
    tunes: [Option<StaticSoundData>; 4],
    track_paths: [PathBuf; 2],
    active: Track,
    sounds_enabled: bool,
    music_enabled: bool,
    master: i32,
}

fn tween(ms: u64) -> Tween {
    Tween {
        duration: Duration::from_millis(ms),
        easing: Easing::Linear,
        ..Default::default()
    }
}

fn tune_slot(tune: Tune) -> usize {
    match tune {
        Tune::Walk => 0,
        Tune::Bump => 1,
        Tune::Chest => 2,
        Tune::Defeat => 3,
    }
}

impl KiraAudio {
    /// Open the device and load what the pack declares. `root` is the
    /// directory the pack's paths resolve against.
    pub fn new(audio: &ResAudio, root: &Path) -> Self {
        let device = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .ok()
            .and_then(|mut manager| {
                let music_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
                let sfx_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
                Some(Device {
                    manager,
                    music_track,
                    sfx_track,
                    music: None,
                })
            });

        // A declared path that fails to load leaves its slot empty and its
        // events are skipped, exactly as C does.
        let load = |name: &str| -> Option<StaticSoundData> {
            if name.is_empty() {
                return None;
            }
            StaticSoundData::from_file(root.join(name)).ok()
        };

        Self {
            device,
            tunes: [
                load(audio.tune_walk.as_str()),
                load(audio.tune_bump.as_str()),
                load(audio.tune_chest.as_str()),
                load(audio.tune_defeat.as_str()),
            ],
            track_paths: [
                root.join(audio.track_openworld.as_str()),
                root.join(audio.track_combat.as_str()),
            ],
            active: Track::None,
            sounds_enabled: true,
            music_enabled: true,
            master: 9,
        }
    }

    /// True when a device opened. C's `audio_is_available`, which the Controls
    /// panel reads to gray out the Sounds / Music / Volume rows.
    pub fn is_available(&self) -> bool {
        self.device.is_some()
    }

    pub fn set_sounds_enabled(&mut self, on: bool) {
        self.sounds_enabled = on;
        self.apply_volumes();
    }

    /// Music off PAUSES rather than stops, so toggling back on resumes where
    /// it was instead of restarting. `audio.h` documents that as deliberate.
    pub fn set_music_enabled(&mut self, on: bool) {
        self.music_enabled = on;
        let fade = tween(DUCK_RELEASE_MS);
        if let Some(device) = self.device.as_mut() {
            if let Some(music) = device.music.as_mut() {
                if on {
                    music.resume(fade);
                } else {
                    music.pause(fade);
                }
            }
        }
        self.apply_volumes();
    }

    pub fn set_master_volume(&mut self, value: i32) {
        self.master = value;
        self.apply_volumes();
    }

    fn apply_volumes(&mut self) {
        let master = slider_to_db(self.master);
        let music_db = if self.music_enabled {
            master + MUSIC_HEADROOM_DB
        } else {
            SILENCE_DB
        };
        let sfx_db = if self.sounds_enabled {
            master + SFX_HEADROOM_DB
        } else {
            SILENCE_DB
        };
        let fade = tween(DUCK_ATTACK_MS);
        if let Some(device) = self.device.as_mut() {
            device.music_track.set_volume(Decibels(music_db), fade);
            device.sfx_track.set_volume(Decibels(sfx_db), fade);
        }
    }

    pub fn play_tune(&mut self, tune: Tune) {
        if !self.sounds_enabled {
            return;
        }
        let Some(data) = self.tunes[tune_slot(tune)].clone() else {
            return;
        };
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let _ = device.sfx_track.play(data);
    }

    /// Hard cut, as C does. `Track::None` silences music without touching the
    /// enabled toggle.
    pub fn set_track(&mut self, track: Track) {
        if track == self.active {
            return;
        }
        self.active = track;
        let fade = tween(DUCK_ATTACK_MS);
        // Resolve the path BEFORE borrowing the device. `self.device.as_mut()`
        // borrows all of `self` mutably, so reading `self.track_paths` after
        // it does not compile.
        let path = match track {
            Track::None => None,
            Track::Openworld => Some(self.track_paths[0].clone()),
            Track::Combat => Some(self.track_paths[1].clone()),
        };
        let Some(device) = self.device.as_mut() else {
            return;
        };
        if let Some(mut music) = device.music.take() {
            music.stop(fade);
        }
        let Some(path) = path else {
            return;
        };
        if let Ok(data) = StreamingSoundData::from_file(path) {
            if let Ok(handle) = device.music_track.play(data.loop_region(..)) {
                device.music = Some(handle);
            }
        }
    }
}
```

In `shell/src/main.rs`, register the module after `pub mod audio;`:

```rust
pub mod audio_backend;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell audio_backend::`
Expected: PASS, 2 tests. They pass whether or not this machine has a device.

If `Device.manager` warns as never read, keep the field -- dropping the manager
stops all playback, so it must be held. Silence it with a doc comment saying
so, not by deleting it.

- [ ] **Step 6: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass, `render_free` included.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/Cargo.toml Cargo.lock shell/src/audio_backend.rs shell/src/main.rs
git commit -m "feat: add the kira audio backend"
```

---

### Task 4: Construct it and honour the Controls rows

**Files:**
- Modify: `shell/src/main.rs`
- Modify: `shell/src/views.rs:2401`
- Test: `shell/src/views.rs`

**Interfaces:**
- Consumes: `KiraAudio` from Task 3.
- Produces: `views::controls_row_disabled` answering from a real device state;
  a live `KiraAudio` in `main`.

**Where the pack root comes from.** The shell already resolves art paths
relative to the `game.json` it was given. Reuse that same base for
`KiraAudio::new`; do not invent a second root. Find it by following how
`SpriteCatalog` paths are resolved in `backend.rs` and pass the identical value.

- [ ] **Step 1: Write the failing test**

`audio_available()` is a free function with no inputs, so the test pins the
threading instead. Add to `shell/src/views.rs`'s test module:

```rust
    /// The three audio rows gray out on DEVICE state, not on whether the pack
    /// ships audio -- C's `controls_row_is_audio(g, row) && !audio_is_available()`.
    #[test]
    fn the_audio_rows_follow_device_availability() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "controls": {"settings": [
                  {"id":"delay","label":"Delay","type":"numeric","range":10},
                  {"id":"sounds","label":"Sounds","type":"bool"},
                  {"id":"music","label":"Music","type":"bool"},
                  {"id":"volume","label":"Volume","type":"numeric","range":10}
                ]}}"#,
        )
        .expect("fixture parses");

        assert!(!controls_row_disabled(&resources, 0, true), "Delay never grays");
        assert!(!controls_row_disabled(&resources, 1, true), "device present");
        assert!(controls_row_disabled(&resources, 1, false), "no device");
        assert!(controls_row_disabled(&resources, 2, false), "no device");
        assert!(controls_row_disabled(&resources, 3, false), "no device");
        assert!(!controls_row_disabled(&resources, 0, false), "Delay still never grays");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p openbounty-shell the_audio_rows_follow 2>&1 | head -20`
Expected: FAIL to compile, `this function takes 2 arguments but 3 were supplied`.

- [ ] **Step 3: Thread availability through**

In `shell/src/views.rs`, delete the stub entirely:

```rust
/// The shell ships no audio, so the three audio-dependent rows are always
/// grayed out, exactly as C grays them when its audio init fails.
fn audio_available() -> bool {
    false
}
```

and change `controls_row_disabled` to take the answer:

```rust
fn controls_row_disabled(resources: &Resources, row: usize, audio_available: bool) -> bool {
    controls_row_is_audio(resources, row) && !audio_available
}
```

Thread `audio_available: bool` down from `Views::draw` to `draw_controls` and
on to each `controls_row_disabled` call. The compiler will name every site.

- [ ] **Step 4: Construct the audio and push the options**

In `shell/src/main.rs`, after `resources` and the pack root are known:

```rust
    let mut audio = audio_backend::KiraAudio::new(&resources.audio, pack_root);
```

Then once, before the main loop, and again each frame inside it -- C does
exactly this at `main.c:904-906` and again at `main.c:955-957`, because the
Controls panel mutates `stats.options` under the player's hands:

```rust
        audio.set_sounds_enabled(game.stats.options[audio::OPTION_SOUNDS] != 0);
        audio.set_music_enabled(game.stats.options[audio::OPTION_MUSIC] != 0);
        audio.set_master_volume(game.stats.options[audio::OPTION_VOLUME]);
```

Name the indices rather than scattering literals. `shell/src/anim.rs:24` already
sets this precedent for the two options it reads, comment included, so add the
matching three to `audio.rs` (Task 2's module) in the same style:

```rust
/// `stats.options` slots this reads. The pack's `controls.settings` declares
/// them in this order, so the order is load-bearing -- C indexes them the same
/// way at `main.c:904-906`.
pub const OPTION_SOUNDS: usize = 1;
pub const OPTION_MUSIC: usize = 5;
pub const OPTION_VOLUME: usize = 6;
```

Pass `audio.is_available()` into the `views.draw(..)` call added in Step 3.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell`
Expected: PASS. Existing controls tests that called the old two-argument
`controls_row_disabled` need their call sites updated; the compiler names them.

- [ ] **Step 6: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/src/main.rs shell/src/views.rs
git commit -m "feat: honour the audio controls rows"
```

---

### Task 5: Wire the events

**Files:**
- Modify: `shell/src/main.rs`
- Test: none new -- Task 2 already pins the mapping; this is call-site wiring.

**Interfaces:**
- Consumes: `audio::tune_for_step`, `KiraAudio::{play_tune, set_track}`.
- Produces: audible play. This is the task where a run first makes sound.

- [ ] **Step 1: Play the step tunes**

Wherever `main.rs` handles the `StepResult` from `game_step_basic` / `consume_step`:

```rust
            if let Some(tune) = audio::tune_for_step(&result) {
                audio.play_tune(tune);
            }
```

- [ ] **Step 2: Play the defeat tune**

At BOTH `game.temp_death(..)` call sites (`main.rs:262` and `main.rs:424`),
immediately before the `temp_death` banner is queued:

```rust
        audio.play_tune(audio::Tune::Defeat);
```

C plays it at `combat_loop.c:454` and `705`, before switching back to the
openworld track.

- [ ] **Step 3: Switch the music track**

Where a combat session opens (`combat = Some(..)`):

```rust
        audio.set_track(audio::Track::Combat);
```

Where it closes, and after a defeat:

```rust
        audio.set_track(audio::Track::Openworld);
```

And once at startup, after construction, matching `main.c:907`:

```rust
    audio.set_track(audio::Track::Openworld);
```

`set_track` early-returns when the track is unchanged, so calling it every
frame would also be correct -- but call it on the transitions, as C does.

- [ ] **Step 4: Verify the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 5: Verify by ear**

This is the first task whose result a test cannot check. A pack with audio is
needed:

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

Confirm, in order:

1. Openworld music starts and loops without a gap or click.
2. Walking plays the walk tune once per step; walking into water or off the map
   edge plays the bump tune instead.
3. Opening a treasure chest plays the chest tune, NOT the walk tune.
4. Entering combat cuts to the combat track; winning cuts back.
5. Losing a battle plays the defeat tune, then returns to the openworld track.
6. In Controls: Sounds off silences tunes but not music; Music off silences
   music; **Music back on resumes mid-track rather than restarting** -- the
   behaviour `audio.h` documents and quad-snd could not have provided.
7. Volume 0 is silent, 9 is loudest, and the rows are NOT grayed out (a device
   is present).

If the walk tune is irritating at 40 steps a day, that is a content problem
before it is a code problem. Note it; do not change the gating.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/main.rs
git commit -m "feat: play the audio events"
```

---

### Task 6: The open pack's audio contract

**Files:**
- Modify: `packs/open/CHECKLIST.md`
- Modify: `tools/open_pack.py`
- Modify: `packs/open/README.md`

**Interfaces:**
- Consumes: nothing from earlier tasks. This task is independent of 1-5 and may
  be done before, after, or in parallel.
- Produces: the slot list and verification rules whoever sources the audio
  works against. **It authors no audio.**

- [ ] **Step 1: Add the audio slots to the checklist**

`CHECKLIST.md` is generated by `tools/open_pack.py checklist`, so edit the
GENERATOR, not the output, then regenerate.

**Do NOT add the audio slots to `tools/hd_assets.json`.** That file is a
manifest of art *extracted from the original game* -- every entry carries a
`source: {archive, entry, frame}` naming the DOS archive it came out of, plus
`native: {w, h}` and an `alpha` convention. An audio slot has none of those
fields and, by design, no original to be extracted from: this audio is
third-party CC0, not a re-render of the original. Forcing it in would mean
inventing a fake `source` for every entry.

Declare the six audio slots as their own list in `open_pack.py` and emit them
as a separate `## audio` section, so the two manifests stay honest about where
their contents come from.

The six slots:

```
audio/openworld.ogg   - music_loop,   ogg vorbis
audio/combat.ogg      - music_loop,   ogg vorbis
audio/tune_walk.wav   - sfx_oneshot,  wav
audio/tune_bump.wav   - sfx_oneshot,  wav
audio/tune_chest.wav  - sfx_oneshot,  wav
audio/tune_defeat.wav - sfx_oneshot,  wav
```

- [ ] **Step 2: Add audio rules to `verify`**

`verify` currently checks coverage, dimensions, alpha, provenance presence,
license match, and that nothing is a near-match for the original art. The
dimension, alpha and near-match checks are image-only and must not run on audio.
Audio gets its own checks:

- The file decodes, and its container matches its extension.
- Sample rate and channel count are within the declared contract.
- Duration is within bounds: a one-shot tune is short, a music loop is not.
- A size budget, so a music file cannot quietly add tens of MB to the repo.
- The provenance entry exists and its `license` matches the pack license,
  reusing the existing check rather than duplicating it.

- [ ] **Step 3: Record what verification CANNOT do**

Add to `packs/open/README.md`, beside the clean-room rule:

> The near-match check that rejects traced or upscaled art has no audio
> equivalent. Nothing automated can tell an original composition from a rip of
> the original soundtrack, so every audio file's licence rests on human review
> at the point of sourcing. The clean-room rule covers melodies: re-rendering
> the original game's PC-speaker tunes with a better synth produces a
> derivative of the original composition, not original work.

- [ ] **Step 4: Verify**

Run: `python3 tools/open_pack.py checklist && python3 tools/open_pack.py verify --scale 1`
Expected: the checklist gains an audio section reporting 0/6 present; verify
reports the art unchanged at 267/311 and does not error on the absent audio.

- [ ] **Step 5: Ask for approval, then commit**

```bash
git add packs/open/CHECKLIST.md packs/open/README.md tools/open_pack.py
git commit -m "feat: add the open pack audio contract"
```

---

## When this is done

Update `specs/play-verification.md`: the §13 `| Audio | GAP |` row closes for
the code half, but the open pack still ships no audio -- reword rather than
delete, and move the "Last refreshed" line to the new tip.

Also add a line to `AGENTS.md`'s shell layout section: `audio.rs` holds the
audio policy and `audio_backend.rs` is the only module that knows kira exists,
the sibling of `backend.rs`'s macroquad rule.
