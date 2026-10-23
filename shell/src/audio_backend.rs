//! The only module that knows `kira` exists.
//!
//! `backend.rs` is the only module that knows macroquad exists; this is its
//! sibling for audio. Every decision -- which tune, what volume, when to duck,
//! what the toggles do -- is made in `audio.rs`; this executes them.
//!
//! A machine with no playback device is not an error. `AudioManager::new`
//! returns `Err`, this stores `None`, and every call becomes a no-op -- which
//! is what C achieves with an `IsAudioDeviceReady` check plus an stderr
//! redirect to hide ALSA's noise.

use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::streaming::{StreamingSoundData, StreamingSoundHandle};
use kira::sound::{FromFileError, PlaybackState};
use kira::track::{TrackBuilder, TrackHandle};
use kira::{AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Tween};

use openbounty_engine::resources::ResAudio;

use crate::audio::{
    slider_to_db, Duck, Music, MusicCmd, Track, Tune, MUSIC_HEADROOM_DB, SFX_HEADROOM_DB,
};

/// The live device, absent when none opened.
struct Device {
    /// Never read, but held: dropping the manager stops all playback.
    _manager: AudioManager<DefaultBackend>,
    music_track: TrackHandle,
    sfx_track: TrackHandle,
    stream: Option<StreamingSoundHandle<FromFileError>>,
    /// The last instance of each tune. raylib's `PlaySound` on a playing
    /// `Sound` restarts it rather than layering a second copy, so a replay
    /// stops the previous instance first.
    playing: [Option<StaticSoundHandle>; 4],
}

pub struct KiraAudio {
    device: Option<Device>,
    tunes: [Option<StaticSoundData>; 4],
    /// The music files' bytes, kept so a restart can decode from the top.
    /// C likewise holds the pack bytes for the life of its streams.
    tracks: [Option<Arc<[u8]>>; 2],
    music: Music,
    duck: Duck,
    sounds_enabled: bool,
    /// `None` until the first push, so that push always applies.
    master: Option<i32>,
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

fn track_slot(track: Track) -> Option<usize> {
    match track {
        Track::None => None,
        Track::Openworld => Some(0),
        Track::Combat => Some(1),
    }
}

impl KiraAudio {
    /// Open the device and load what the pack declares, porting
    /// `audio_init_device`. `load` returns a pack file's bytes by its
    /// pack-relative path, C's `LoadAssetBytes`.
    ///
    /// As in C, nothing is loaded when no device opened, and a declared file
    /// that is missing or undecodable leaves its slot empty.
    pub fn new(audio: &ResAudio, load: impl Fn(&str) -> Option<Vec<u8>>) -> Self {
        let device = AudioManager::<DefaultBackend>::new(AudioManagerSettings::default())
            .ok()
            .and_then(|mut manager| {
                let music_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
                let sfx_track = manager.add_sub_track(TrackBuilder::new()).ok()?;
                Some(Device {
                    _manager: manager,
                    music_track,
                    sfx_track,
                    stream: None,
                    playing: Default::default(),
                })
            });

        let mut out = Self {
            device,
            tunes: Default::default(),
            tracks: Default::default(),
            music: Music::default(),
            duck: Duck::default(),
            sounds_enabled: true,
            master: None,
        };
        if out.device.is_none() {
            return out;
        }

        let bytes = |path: &str| (!path.is_empty()).then(|| load(path)).flatten();
        for (slot, path) in [
            audio.tune_walk.as_str(),
            audio.tune_bump.as_str(),
            audio.tune_chest.as_str(),
            audio.tune_defeat.as_str(),
        ]
        .into_iter()
        .enumerate()
        {
            out.tunes[slot] =
                bytes(path).and_then(|b| StaticSoundData::from_cursor(Cursor::new(b)).ok());
        }
        for (slot, path) in [audio.track_openworld.as_str(), audio.track_combat.as_str()]
            .into_iter()
            .enumerate()
        {
            // Probe-decode once, as C's `s_have_*` checks the stream opened,
            // so a bad file is skipped here rather than on every start.
            out.tracks[slot] = bytes(path).map(Arc::from).filter(|b: &Arc<[u8]>| {
                StreamingSoundData::from_cursor(Cursor::new(b.clone())).is_ok()
            });
        }
        out
    }

    /// True when a device opened. C's `audio_is_available`, which the Controls
    /// panel reads to gray out the Sounds / Music / Volume rows.
    pub fn is_available(&self) -> bool {
        self.device.is_some()
    }

    /// Gates FUTURE tunes only; one already playing is left to finish.
    pub fn set_sounds_enabled(&mut self, on: bool) {
        self.sounds_enabled = on;
    }

    pub fn set_music_enabled(&mut self, on: bool) {
        let cmd = self.music.set_enabled(on);
        self.apply(cmd);
    }

    pub fn set_track(&mut self, track: Track) {
        let cmd = self.music.set_track(track);
        self.apply(cmd);
    }

    /// The 0..9 slider. Music follows at once, as C's per-tick
    /// `apply_music_volume` does; a tune takes the level it started with.
    pub fn set_master_volume(&mut self, value: i32) {
        if self.master == Some(value) {
            return;
        }
        self.master = Some(value);
        if let Some(device) = self.device.as_mut() {
            device.music_track.set_volume(
                Decibels(slider_to_db(value) + MUSIC_HEADROOM_DB),
                Tween::default(),
            );
        }
    }

    /// `audio_play_tune`: gated on the Sounds toggle, skipped when the slot
    /// is empty.
    pub fn play_tune(&mut self, tune: Tune) {
        if !self.sounds_enabled {
            return;
        }
        let slot = tune_slot(tune);
        let Some(data) = self.tunes[slot].as_ref() else {
            return;
        };
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let volume = slider_to_db(self.master.unwrap_or(9)) + SFX_HEADROOM_DB;
        if let Some(mut previous) = device.playing[slot].take() {
            previous.stop(Tween::default());
        }
        device.playing[slot] = device.sfx_track.play(data.volume(Decibels(volume))).ok();
    }

    /// The per-frame half of `audio_tick`: duck the music while any tune is
    /// playing. kira streams on its own thread, so there is no
    /// `UpdateMusicStream` to drive.
    pub fn tick(&mut self) {
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let any_tune = device
            .playing
            .iter()
            .flatten()
            .any(|handle| handle.state() != PlaybackState::Stopped);
        if let Some(step) = self.duck.update(any_tune) {
            if let Some(stream) = device.stream.as_mut() {
                stream.set_volume(Decibels(step.target_db), tween(step.tween_ms));
            }
        }
    }

    fn apply(&mut self, cmd: MusicCmd) {
        let Some(device) = self.device.as_mut() else {
            return;
        };
        if cmd.stop {
            // A hard cut; kira's default tween is a few milliseconds, which
            // only spares the click.
            if let Some(mut stream) = device.stream.take() {
                stream.stop(Tween::default());
            }
        }
        let Some(bytes) = cmd
            .start
            .and_then(track_slot)
            .and_then(|slot| self.tracks[slot].clone())
        else {
            return;
        };
        let Ok(data) = StreamingSoundData::from_cursor(Cursor::new(bytes)) else {
            return;
        };
        let data = data.loop_region(..).volume(Decibels(self.duck.level_db()));
        device.stream = device.music_track.play(data).ok();
    }
}

impl crate::audio::AudioOut for KiraAudio {
    fn is_available(&self) -> bool {
        KiraAudio::is_available(self)
    }
    fn set_sounds_enabled(&mut self, on: bool) {
        KiraAudio::set_sounds_enabled(self, on)
    }
    fn set_music_enabled(&mut self, on: bool) {
        KiraAudio::set_music_enabled(self, on)
    }
    fn set_track(&mut self, track: Track) {
        KiraAudio::set_track(self, track)
    }
    fn set_master_volume(&mut self, value: i32) {
        KiraAudio::set_master_volume(self, value)
    }
    fn play_tune(&mut self, tune: Tune) {
        KiraAudio::play_tune(self, tune)
    }
    fn tick(&mut self) {
        KiraAudio::tick(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::resources::ResAudio;

    fn no_files(_: &str) -> Option<Vec<u8>> {
        None
    }

    /// The smoke test must pass on a machine with NO audio device -- CI boxes
    /// have none. Construction never panics and never fails the run; it
    /// degrades to silent and says so through `is_available`.
    #[test]
    fn construction_survives_a_machine_with_no_device() {
        let audio = KiraAudio::new(&ResAudio::default(), no_files);
        // Either answer is legal; the point is that it returned at all.
        let _ = audio.is_available();
    }

    /// Every call is a no-op when no device opened or nothing loaded, rather
    /// than a panic.
    #[test]
    fn playback_calls_are_safe_without_a_device_or_files() {
        let mut audio = KiraAudio::new(&ResAudio::default(), no_files);
        audio.set_master_volume(5);
        audio.set_sounds_enabled(true);
        audio.set_music_enabled(true);
        audio.play_tune(Tune::Walk);
        audio.set_track(Track::Openworld);
        audio.tick();
        audio.set_music_enabled(false);
        audio.set_track(Track::None);
        audio.tick();
    }

    /// A declared file that does not decode leaves its slot empty, as C's
    /// `LoadWaveFromMemory` failure does, instead of failing construction.
    #[test]
    fn an_undecodable_file_is_skipped() {
        let mut declared = ResAudio::default();
        declared.tune_walk.copy_from_str("audio/walk.wav");
        declared.track_openworld.copy_from_str("audio/open.ogg");
        let mut audio = KiraAudio::new(&declared, |_| Some(b"not audio".to_vec()));
        audio.play_tune(Tune::Walk);
        audio.set_track(Track::Openworld);
        audio.tick();
    }
}
