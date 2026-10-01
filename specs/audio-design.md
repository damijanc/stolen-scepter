# Audio Design

Status: agreed 2026-09-22. Covers sound in the shell: the four one-shot tunes,
the two music tracks, and the Controls rows that drive them. Authoring the
actual audio files is explicitly out of scope and is listed under Non-Goals.

**Built 2026-10-01** (`54c2526`..`96da117`). The C code disagreed with this
design and with `specs/audio-plan.md` in the places listed under "As built"
below; the code follows C, as `AGENTS.md` requires. Where the body of this
document says otherwise, "As built" wins.

Read `AGENTS.md` and `specs/shell-renderer-plan.md` first. Everything there
still applies -- parity-first translation, one-line commit messages, approval
before every commit.

## Why this is a deliberate divergence

The original is PC-speaker: monophonic square-wave beeps. OpenBounty already
moved past that -- its `src/audio.c` plays pre-rendered WAVs through raylib and
adds OGG music the DOS release never had -- so this port is not choosing to
diverge from the original, it is inheriting a divergence OpenBounty already
made.

What IS new here is the backend. C uses raylib because raylib is already
linked for rendering; this shell renders with macroquad, whose audio support is
five functions and cannot express what `audio.c` does. So this design takes a
dedicated audio library rather than porting `audio.c`'s raylib calls.

**DIVERGENCE, deliberate**, and confined to the shell by construction:

- The engine gains nothing. `engine/tests/render_free.rs` must still pass, and
  `engine/Cargo.toml` gains no dependency.
- `Resources` learns the audio *paths* and nothing else. That is the same
  arrangement `sprites` already has: the engine carries the path, the shell
  decides what to do with it.
- No sound changes a game outcome. Audio is fire-and-forget presentation, as
  `reference/engine/include/ui_host.h` says in as many words: *"There is no
  engine-side audio state -- events are fire-and-forget."*

## What C does, and what macroquad cannot

`reference/src/audio.c` (297 lines) is four `Sound` one-shots plus two `Music`
streams, hard-cut between them, with the music ducked while a tune plays.

macroquad 0.4.16 exposes `load_sound`, `play_sound`, `play_sound_once`,
`stop_sound`, `set_sound_volume`. Measured against `audio.c`:

| `audio.c` needs | macroquad + quad-snd |
|---|---|
| `Music` + `UpdateMusicStream` | No streaming. A sound decodes fully to PCM in RAM; a 90s stereo track is ~16MB |
| `IsAudioDeviceReady()` to gray the audio rows | No device query. `AudioContext::new()` cannot report failure |
| Music off then on RESUMES mid-track | No pause/resume. Only stop and play-from-start |
| Per-stream volume for ducking | Available (`set_sound_volume`) |

Two of those four are load-bearing: the resume behaviour is documented in
`audio.h` as deliberate, and the gray-out is already half-built in this shell
(`shell/src/views.rs:2401` is a stub `fn audio_available() -> bool { false }`
whose comment says the shell ships no audio).

Note also that macroquad's audio was never enabled here. `default-features =
false` in `shell/Cargo.toml` is not what disables it -- macroquad 0.4.16
declares `default = []`, so `audio` was always opt-in.

## Decisions

### 1. kira 0.12.4 is the audio backend

Chosen over enabling macroquad's `audio` feature and over `rodio`.

Every claim below was verified by compiling a probe against the real crate, not
from documentation or memory:

- `AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())`
  returns `Result`, so **a missing device is a clean `Err`**. C needs an
  fd-juggling hack around `InitAudioDevice` to suppress ALSA's stderr noise
  (`audio.c:93-112`); this port needs none of it, and `audio_available()`
  becomes an honest answer rather than a proxy.
- `manager.add_sub_track(TrackBuilder::new()) -> TrackHandle` gives the two
  mixer buses (music, sfx) that C emulates with per-sound volume arithmetic.
- `StaticSoundData::from_file(path)` for the tunes; it is `Clone`, so one
  decode serves every replay.
- `StreamingSoundData::from_file(path).loop_region(..)` streams the music, so
  the ~16MB-per-track figure above never applies.
- `track.set_volume(Decibels, Tween)` covers both the 0..9 slider and ducking.
- `handle.pause(tween)` / `resume(tween)` **preserve position**, matching what
  `audio.h` documents. `stop(tween)` gives C's hard cut between tracks.

kira also pulls `symphonia` with the `vorbis`, `ogg` and `riff` features, so
OGG music and WAV tunes both decode -- exactly the formats the pack already
declares.

Rejected, with reasons, so this is not relitigated:

- **macroquad's `audio` feature.** No new crate and `AGENTS.md`'s
  one-library-aware-module rule stays literally true. Rejected because it
  cannot do resume-at-position or answer the device query, both of which are
  behaviours this design is meant to reproduce, and because "modern, for a
  future upgrade" is the stated goal and quad-snd has no headroom for it.
- **`rodio`.** Streams, detects device failure, widely used. Rejected because
  ducking, crossfade and mixer buses would all be hand-rolled on top of raw
  sinks -- work kira already did, in a crate whose whole subject is game audio.

### 2. Policy is separated from the backend, as `draw.rs` is from `backend.rs`

`shell/src/audio.rs` holds the vocabulary and every decision: which tune an
event produces, what the slider maps to, how the duck envelope moves, what the
mute toggles do. It does not know kira exists and needs no device to test.

`shell/src/audio_backend.rs` is the only module that knows kira, and turns
those decisions into calls.

This is the seam `draw.rs` already establishes and `draw.rs`'s own module docs
defend: view code emits a vocabulary, one backend executes it, and the
vocabulary is testable with no window open. Audio has the identical problem --
a test cannot listen -- so it gets the identical answer.

Rejected: **one `audio.rs` that calls kira directly.** Fewer files, and for
four sounds it is tempting. Rejected because it makes every policy test need an
audio device, which means CI either grows a device or the policy goes untested.
The mapping from `StepResult` to tune is exactly the kind of rule this
repository tests everywhere else.

`AGENTS.md` says `backend.rs` is the ONLY module that knows macroquad exists.
That stays true. The rule gains a sibling clause: `audio_backend.rs` is the
only module that knows kira exists.

### 3. The engine stays audio-free; the shell derives the events

C emits tunes from inside the engine through a host callback -- `game.c:1149`,
`step.c:43`, `step.c:88`, `step.c:424` all call `audio_play_tune`, and
`engine/host_noop.c` supplies a no-op for headless consumers.

This port does not add that callback. Every trigger is already derivable from
what the engine returns:

| C call site | Tune | Rust source |
|---|---|---|
| `game.c:1149` | Walk | `StepResult::Moved { .. }` |
| `step.c:43`, `step.c:88` | Bump | `StepResult::BlockedEdge`, `StepResult::BlockedTerrain { .. }` |
| `step.c:424` | Chest | `StepInteraction::TreasureChest { .. }` |
| `combat_loop.c:454`, `705` | Defeat | the two `game.temp_death(..)` call sites, `main.rs:262` and `main.rs:424` |

Music follows the same rule: `Track::Combat` when a combat session opens
(`combat_loop.c:445`), `Track::Openworld` when it closes or the hero is
defeated (`combat_loop.c:455`, `696`, `712`), and `Track::Openworld` once at
startup (`main.c:907`).

This is the arrangement `AGENTS.md` already describes for `player_io.c`: the
engine returns outcome values where C calls into the host, and the shell calls
the thing it means. Adding an audio callback would mirror a global this port
deliberately does not have.

### 4. C's linear gain constants are converted to decibels once, in policy

`audio.c` works in linear multipliers; kira works in `Decibels`. The
conversions belong in `audio.rs` as named constants with the C value in the
comment, not scattered at call sites:

| `audio.c` | Linear | Decibels |
|---|---|---|
| `MUSIC_HEADROOM` | 0.50 | -6.02 |
| `SFX_HEADROOM` | 0.80 | -1.94 |
| `DUCK_FACTOR` | 0.20 | -13.98 |

The master slider is C's 0..9 UI scale. `slider_to_db(0)` is
`Decibels::SILENCE` -- not 0 dB, which is unity gain and would make a muted
game loud. That boundary gets its own test.

C's `DUCK_ATTACK` (0.40) and `DUCK_RELEASE` (0.10) are per-frame smoothing
coefficients, which kira's tweens replace. At 60fps a 0.40 coefficient reaches
~90% in about 5 frames and 0.10 in about 22, so they become tween durations of
~80ms attack and ~370ms release. **This is an approximation, not a port** --
the envelope shape differs (exponential vs kira's easing) even though the
timing matches. Recorded here so it is not mistaken for parity.

## As built: where C overrode this design

Each was found by reading `reference/src/audio.c`, `reference/engine/step.c`
and `reference/src/combat_loop.c` while porting, and each has a test named for
it in `shell/src/audio.rs`.

1. **The chest tune does not replace the walk tune.** `step.c:424` plays the
   chest, the step is not bounced, and `GameOnStep` then reaches
   `game.c:1149`. Both play, chest first. `tunes_for_step` returns two slots.
2. **A bounced step plays no walk tune.** Town, castle gate, dwelling, alcove
   and a hostile foe set `bounce_back`, and a bounced step never calls
   `GameOnStep`. This engine does not bounce yet, but the tune follows C.
3. **An absorbed step plays no walk tune.** `GameOnStep` returns before
   `game.c:1149` while a time stop is banked or the game is over. The shell
   samples that before the step, which spends the time stop.
4. **Resting plays the walk tune.** Rest reaches `GameOnStep` directly
   (`shell_actions.c:212`).
5. **The defeat tune is a combat loss, not `temp_death`.** C plays it in the
   combat loop (`combat_loop.c:454`, `705`). Dismissing the last stack also
   reaches `GameTempDeath`, through `perform_temp_death`, which plays nothing.
6. **Music off then on RESTARTS.** `audio.h` says it resumes; `audio.c` calls
   `StopMusicStream`, which rewinds, and later `PlayMusicStream` from the top.
   Decision 1's resume-at-position rationale therefore does not apply: kira's
   real advantages here are streaming, the clean device `Err`, and buses.
   A track chosen while music is off is recorded and started when it comes
   back on, as `audio_set_track` does.
7. **Sounds off gates future tunes only.** A tune already playing finishes,
   as `audio.h` says and `audio_play_tune` does; the backend never mutes the
   sfx bus. A replayed tune restarts its own instance, as raylib's `PlaySound`
   does, rather than layering a copy.
8. **No device pins the three options to 0** (`main.c:896-902`), so the panel
   and the live push agree.

Three shape changes, none a C divergence:

- `KiraAudio::new` takes a loader (`Fn(&str) -> Option<Vec<u8>>`), C's
  `LoadAssetBytes`, not a root path. `main.rs` passes `Pack::read`, so a
  `.openbounty` archive plays exactly as a directory does. Nothing is loaded
  when no device opened, as in C.
- Device availability is stored on `Views` (`set_audio_available`) rather than
  threaded through `Views::draw`, because `controls_advance` needs it too.
- Ducking is a per-frame `KiraAudio::tick`, driven by `audio::Duck`; the plan
  had omitted it. During combat the shell only ticks and does not push the
  options, as C's combat loop does.

## Components

- `engine/src/resources.rs`: `ResAudio`, six `FixedStr<RES_PATH_LEN>` paths
  (`tracks.openworld`, `tracks.combat`, `tunes.{walk,bump,chest,defeat}`),
  parsed alongside the existing `sprites.hud` block at `resources.rs:1839`.
  Paths only. A missing key leaves an empty slot, as every other path does.
- `shell/src/audio.rs`: `Tune`, `Track`, the `AudioPolicy` state (enabled
  flags, master volume, duck level, active track), `tune_for_step`,
  `slider_to_db`, and the gain constants above.
- `shell/src/audio_backend.rs`: `KiraAudio`, holding the `AudioManager`, the
  two `TrackHandle`s, the four `StaticSoundData`, and the live
  `StreamingSoundHandle`. Constructed fallibly; a failure degrades to silent
  rather than aborting the run.
- `shell/src/views.rs`: `audio_available()` stops being a `false` stub and
  takes the real answer.
- `shell/src/main.rs`: construction, the per-frame push of `options[1]`,
  `options[5]`, `options[6]`, and the event call sites from Decision 3.

## Data flow

```
engine returns StepResult / StepInteraction
        |
        v
main.rs  --(Tune, Track)-->  audio.rs (policy: enabled? volume? duck?)
                                  |
                                  v
                        audio_backend.rs  --> kira --> device
```

Nothing flows back. A sound that fails to load, a device that never opened, and
a pack that declares no audio all produce the same observable behaviour:
silence, and `audio_available() == false`.

## Failure modes

Each degrades to silence and none aborts the run:

- **No device.** `AudioManager::new` returns `Err`. `audio_available()` is
  false, the three audio Controls rows gray out, every play call is a no-op.
- **Pack declares no audio block.** All six paths empty. The device may still
  open; nothing plays. The rows stay enabled, because the device IS available
  -- this matches C, which grays on device state, not on content.
- **A declared file is missing or undecodable.** That slot stays empty and its
  events are skipped. C does the same: *"Missing entries leave their slot
  empty; audio_play_tune skips silently."*
- **Music file missing but tunes present.** Tunes play, music does not. The
  two are independent, as in C.

## Testing

`audio.rs` is unit-tested with no device, in the crate's inline `#[cfg(test)]`
style:

- `tune_for_step` over every `StepResult` variant, including that
  `StepResult::NoDirection` produces no tune. C confirms this directly: both
  bump sites sit past the direction check, and the comment at `step.c:38` says
  the blocked trace exists so a consumer can *"distinguish 'no move attempted'
  from 'move attempted and blocked'"*. A rejected key is not a bump.
- `slider_to_db` at 0 (silence, the boundary that matters), 9 (unity), and a
  midpoint; and that values above 9 clamp rather than exceed unity.
- Mute gating: sounds disabled suppresses future tunes; music disabled pauses
  rather than stops, so position survives.
- The duck envelope: a tune ducks the music bus and it recovers afterward.

`audio_backend.rs` gets one construction smoke test that passes whether or not
a device exists -- a CI box with no audio must not fail the suite. No test
asserts on sound.

## Content contract

For whoever sources the files. Not built by this design.

- **Music**: OGG Vorbis, two slots (`openworld`, `combat`), looping cleanly.
- **Tunes**: WAV, four slots (`walk`, `bump`, `chest`, `defeat`), short
  one-shots. `walk` fires on every step, so it must be brief and unobtrusive.
- `packs/open/CHECKLIST.md` gains a 6-slot audio section.
- `tools/open_pack.py verify` gains audio rules: decodable, sample rate,
  channel count, duration bounds, a size budget, a provenance entry, and the
  existing license-match check.
- The provenance schema needs no change. `packs/open/README.md` already allows
  `source` to be *"a specific permissive asset id"* rather than `original`,
  which is exactly the third-party CC0 case.
- **The clean-room rule covers melodies.** Re-rendering the original game's
  PC-speaker tunes with a better synth is a derivative of the original
  composition. Tunes must be newly written or independently licensed, not
  transcribed.
- **No automated check can catch a rip.** `verify`'s near-match test compares
  images and has no audio equivalent, so the license claim on each file rests
  on human review at the point of sourcing.

## Non-goals

- **Authoring the audio.** Sourcing and license confirmation are the user's,
  on their own schedule. The code half ships and is testable with no files.
- **New event types.** C has four tunes and two tracks; this ports exactly
  those. Combat hits, spell casts, UI clicks and per-zone music are a later
  slice, and the vocabulary in `audio.rs` is where they would be added.
- **Crossfade.** C hard-cuts between tracks and so does this. kira can
  crossfade; the seam permits adding it without touching call sites.
- **`options[2]` walk_beep and `options[4]` cga.** Both are parsed, drawn and
  toggleable, and **C reads neither** -- the only readers of `stats.options`
  in the C tree are `[0]` Delay, `[1]` Sounds, `[3]` Animation, `[5]` Music,
  `[6]` Volume. They are DEAD, the same store-never-read tell `AGENTS.md`
  warns about. Do not wire them.
- **Web/wasm.** C defers device init to a user gesture under Emscripten. This
  shell has no web target; if one appears, that is when to handle it.

## Risks

- **kira is a large dependency.** It pulls `symphonia` and a platform audio
  stack (on macOS, `coreaudio-rs` and the `objc2` audio crates). Verified to
  build clean on this machine in 6.5s. It lands in `shell` only; the engine's
  dependency boundary test is unaffected.
- **The duck envelope is an approximation** (Decision 4). If it sounds wrong,
  the tween durations are the dial, and the C coefficients are recorded above
  to re-derive from.
- **`walk` fires on every step.** At 40 steps a day this is the most-played
  sound in the game and the one most likely to irritate. C gates it behind the
  Sounds toggle only. If it grates, that is a content problem before it is a
  code problem.
- **Device loss mid-session** (headphones unplugged) is not handled. C does not
  handle it either. Out of scope, noted so it is not mistaken for an oversight.

## Work order

1. `ResAudio` and its parse, with tests. No shell change.
2. `audio.rs`: vocabulary, constants, `tune_for_step`, `slider_to_db`, policy
   state, with tests. No kira, no device.
3. `audio_backend.rs`: kira construction, loading, playback. Smoke test only.
4. Wire `main.rs`: construct, push the three options each frame, and the
   real `audio_available()`.
5. Wire the events from Decision 3: the four tunes and the track switches.
6. `packs/open` content contract: CHECKLIST audio section and `verify` rules.

Steps 1-3 are independently testable and land green on a machine with no audio
device. Step 4 is where a run first makes sound.
