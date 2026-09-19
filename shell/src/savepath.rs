//! Save-file locations, porting `engine/savepath.c`.
//!
//! A slot lives at `<user-data>/openbounty/saves/<pack-id>/save_<N>.dat`, or
//! flat under a `--save-dir` override (`<override>/save_<N>.dat`), exactly as
//! C's `SavePathGetSlot`. Directories are created on demand. This is a host
//! concern, so it lives in the shell, not the engine.

use std::path::PathBuf;

use openbounty_engine::savegame::{save_read_header, SaveHeader};

/// Number of save slots exposed to the picker, C's `SAVE_SLOT_COUNT`.
pub const SAVE_SLOT_COUNT: usize = 10;

/// Where a pack's saves live. A `--save-dir` override makes the layout flat
/// with no pack-id segregation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SavePaths {
    override_dir: Option<PathBuf>,
}

impl SavePaths {
    pub fn new() -> Self {
        Self::default()
    }

    /// Flat override, used by `--save-dir` and by tests so they never touch
    /// the real user-data tree.
    pub fn with_override(dir: impl Into<PathBuf>) -> Self {
        Self {
            override_dir: Some(dir.into()),
        }
    }

    /// The base save directory, or `None` when the platform has no home. On a
    /// `--save-dir` override this is the override itself.
    pub fn base_dir(&self) -> Option<PathBuf> {
        if let Some(dir) = &self.override_dir {
            return Some(dir.clone());
        }
        #[cfg(target_os = "macos")]
        {
            let home = std::env::var_os("HOME")?;
            Some(PathBuf::from(home).join("Library/Application Support/OpenBounty"))
        }
        #[cfg(target_os = "windows")]
        {
            let appdata = std::env::var_os("APPDATA")?;
            Some(PathBuf::from(appdata).join("OpenBounty"))
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
                Some(PathBuf::from(xdg).join("openbounty"))
            } else {
                let home = std::env::var_os("HOME")?;
                Some(PathBuf::from(home).join(".local/share/openbounty"))
            }
        }
    }

    /// The full path for `slot`, creating the directory. `None` for a bad
    /// slot or an unusable base dir. An empty `pack_id` uses C's `_unknown`
    /// bucket so saves never collide across packs.
    pub fn slot(&self, pack_id: &str, slot: usize) -> Option<PathBuf> {
        if slot >= SAVE_SLOT_COUNT {
            return None;
        }
        let base = self.base_dir()?;
        let dir = if self.override_dir.is_some() {
            base
        } else {
            let pid = if pack_id.is_empty() {
                "_unknown"
            } else {
                pack_id
            };
            base.join("saves").join(pid)
        };
        std::fs::create_dir_all(&dir).ok()?;
        Some(dir.join(format!("save_{slot}.dat")))
    }
}

/// The picker's view of the slots: each slot's header when a valid save is
/// there, and how many exist. Ports C's `SlotSet`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SlotSet {
    pub headers: [Option<SaveHeader>; SAVE_SLOT_COUNT],
    pub existing: usize,
}

/// Read every slot's header, porting `scan_slots`. A missing or unreadable
/// file is an empty slot.
pub fn scan_slots(paths: &SavePaths, pack_id: &str) -> SlotSet {
    let mut set = SlotSet::default();
    for slot in 0..SAVE_SLOT_COUNT {
        let Some(path) = paths.slot(pack_id, slot) else {
            continue;
        };
        if let Ok(header) = save_read_header(&path) {
            if header.exists {
                set.headers[slot] = Some(header);
                set.existing += 1;
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::state_serialize::SAVE_VERSION;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openbounty-savepath-{tag}"));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A minimal valid save the header reader accepts, without building a
    /// whole Game.
    fn write_header_only(path: &std::path::Path, name: &str, days: i32) {
        let body = format!(
            r#"{{"version": {SAVE_VERSION},
                 "character": {{"name": "{name}", "rank_title": "General"}},
                 "stats": {{"days_left": {days}}},
                 "position": {{"zone": "home"}}}}"#
        );
        std::fs::write(path, body).expect("writes");
    }

    #[test]
    fn an_override_slot_is_flat_and_creates_the_directory() {
        let dir = temp_dir("flat");
        let paths = SavePaths::with_override(&dir);
        let path = paths.slot("openbounty", 3).expect("slot");
        assert_eq!(path, dir.join("save_3.dat"));
        assert!(dir.is_dir());
    }

    #[test]
    fn a_slot_out_of_range_is_none() {
        let dir = temp_dir("range");
        let paths = SavePaths::with_override(&dir);
        assert_eq!(paths.slot("openbounty", SAVE_SLOT_COUNT), None);
    }

    #[test]
    fn scan_slots_counts_only_readable_saves() {
        let dir = temp_dir("scan");
        let paths = SavePaths::with_override(&dir);
        write_header_only(&paths.slot("openbounty", 0).unwrap(), "Murray", 123);
        write_header_only(&paths.slot("openbounty", 4).unwrap(), "Hack", 50);
        // A file that is not a save is ignored, not counted.
        std::fs::write(paths.slot("openbounty", 7).unwrap(), "not json").unwrap();

        let set = scan_slots(&paths, "openbounty");
        assert_eq!(set.existing, 2);
        assert_eq!(set.headers[0].unwrap().name.as_str(), "Murray");
        assert_eq!(set.headers[0].unwrap().days_left, 123);
        assert_eq!(set.headers[4].unwrap().name.as_str(), "Hack");
        assert!(set.headers[7].is_none());
    }
}
