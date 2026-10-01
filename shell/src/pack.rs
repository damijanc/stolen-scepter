//! The game pack reader, porting `engine/pack.c`.
//!
//! A pack is either a `.openbounty` ZIP or a loose directory tree, and
//! `open` auto-detects which -- both are first-class in C, the directory
//! being its dev-iteration mode.
//!
//! An archive is decompressed WHOLE at open time, as C does. That is not an
//! oversight to optimise away: `pack_read` hands out borrowed bytes that stay
//! valid until close, and the whole asset manifest is read during startup
//! anyway.
//!
//! This lives in the shell, not the engine. The engine takes bytes:
//! `Resources::from_json_str` for the manifest and
//! `Resources::set_zone_map_bytes` for every zone's tiles, which
//! [`load_resources`] supplies up front. The engine never learns what an
//! archive is.

use std::borrow::Cow;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

pub enum Pack {
    /// Loose directory tree. C's dev-mode iteration.
    Dir { root: PathBuf },
    /// A `.openbounty` archive, fully decompressed at open time.
    Archive {
        entries: HashMap<String, Vec<u8>>,
        hash: String,
    },
}

/// C's `pack_hash`: FNV1a-64 over the archive bytes, as hex. Directory packs
/// have none, and C returns "" for them.
fn fnv1a64_hex(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

impl Pack {
    /// Open a pack. A directory is a loose tree; a regular file is a ZIP.
    pub fn open(path: &Path) -> std::io::Result<Self> {
        if path.is_dir() {
            return Ok(Self::Dir {
                root: path.to_path_buf(),
            });
        }
        let raw = std::fs::read(path)?;
        let hash = fnv1a64_hex(&raw);
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&raw[..]))
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;

        let mut entries = HashMap::new();
        for index in 0..archive.len() {
            let mut file = archive
                .by_index(index)
                .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
            if file.is_dir() {
                continue;
            }
            let name = file.name().to_owned();
            let mut bytes = Vec::with_capacity(file.size() as usize);
            file.read_to_end(&mut bytes)?;
            entries.insert(name, bytes);
        }
        Ok(Self::Archive { entries, hash })
    }

    /// Bytes for a pack-relative entry, e.g. `"art/font/kb-font.png"`.
    ///
    /// A directory pack re-reads the file each call, which is what makes
    /// dev-mode iteration useful; an archive borrows from the map it slurped.
    pub fn read(&self, rel: &str) -> Option<Cow<'_, [u8]>> {
        match self {
            Self::Dir { root } => std::fs::read(root.join(rel)).ok().map(Cow::Owned),
            Self::Archive { entries, .. } => entries.get(rel).map(|b| Cow::Borrowed(b.as_slice())),
        }
    }

    /// C's `pack_hash`. Empty for a directory pack.
    pub fn hash(&self) -> &str {
        match self {
            Self::Dir { .. } => "",
            Self::Archive { hash, .. } => hash,
        }
    }

    /// The directory a loose pack reads from, `None` for an archive. The
    /// engine's path-based loaders still need this for a directory pack.
    pub fn root(&self) -> Option<&Path> {
        match self {
            Self::Dir { root } => Some(root.as_path()),
            Self::Archive { .. } => None,
        }
    }
}

/// Where a command-line path points. A file named `game.json` is the old
/// spelling of a directory pack, so it names its parent; anything else is
/// the pack itself.
pub fn pack_location(path: &Path) -> PathBuf {
    if path.file_name().is_some_and(|name| name == "game.json") && path.is_file() {
        if let Some(parent) = path.parent() {
            return parent.to_path_buf();
        }
    }
    path.to_path_buf()
}

/// Parse the pack's `game.json` and hand the engine every zone's map bytes.
///
/// Both pack modes go through here, so a directory and an archive cannot
/// drift. The maps are supplied up front because the engine loads some of
/// them itself -- scepter burial at init, the zone switch behind a gate or a
/// defeat -- where no shell call site could pass bytes in.
pub fn load_resources(pack: &Pack) -> Result<openbounty_engine::resources::Resources, String> {
    let json = pack
        .read("game.json")
        .ok_or_else(|| "pack has no game.json".to_owned())?;
    let json = std::str::from_utf8(&json).map_err(|err| format!("game.json: {err}"))?;
    let mut resources = openbounty_engine::resources::Resources::from_json_str(json)
        .map_err(|err| format!("game.json: {err:?}"))?;

    for index in 0..resources.zone_count() {
        let Some(zone) = resources.zone_by_index(index) else {
            continue;
        };
        let id = zone.id;
        // A map the pack lacks stays unsupplied; the zone then fails to load
        // when something asks for it, as it would in C.
        if let Some(bytes) = pack.read(zone.map_path.as_str()) {
            resources.set_zone_map_bytes(id.as_str(), bytes.into_owned());
        }
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a real .openbounty in a temp dir so the archive path is tested
    /// against the actual zip reader, not a mock.
    fn write_archive(dir: &Path) -> PathBuf {
        let path = dir.join("test.openbounty");
        let file = std::fs::File::create(&path).expect("create archive");
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("game.json", opts).expect("start");
        zip.write_all(br#"{"pack_id":"test"}"#).expect("write");
        zip.start_file("art/tile.png", opts).expect("start");
        zip.write_all(b"PNGBYTES").expect("write");
        zip.finish().expect("finish");
        path
    }

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ob_pack_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    const ZONE_PACK: &str = r#"{
      "pack_id": "zoned",
      "tile_codes": {
        ".": {"art":"grass","terrain":"grass"},
        "F": {"art":"forest","terrain":"forest"}
      },
      "zones": [{"id":"home","map":"maps/home.dat","width":4,"height":2}],
      "troops": []
    }"#;
    const HOME_TILES: &[u8] = b".F..\n....\n";

    /// Load the resources, then a zone map BY ID -- the way the engine's own
    /// callers (scepter burial, gates, defeat) reach it.
    fn second_tile_terrain(pack: &Pack) -> openbounty_engine::tile::Terrain {
        let resources = load_resources(pack).expect("resources load");
        assert_eq!(resources.pack_id.as_str(), "zoned");
        let map = openbounty_engine::map::Map::load_zone(&resources, "home").expect("zone loads");
        map.tile(1, 0).expect("in bounds").terrain
    }

    #[test]
    fn a_directory_pack_feeds_the_engine_its_zone_maps() {
        let dir = temp_dir("zoned_dir");
        std::fs::create_dir_all(dir.join("maps")).expect("maps dir");
        std::fs::write(dir.join("game.json"), ZONE_PACK).expect("write");
        std::fs::write(dir.join("maps/home.dat"), HOME_TILES).expect("write");

        let pack = Pack::open(&dir).expect("dir pack");
        assert_eq!(
            second_tile_terrain(&pack),
            openbounty_engine::tile::Terrain::Forest
        );
    }

    /// The archive has no directory the engine could resolve `maps/home.dat`
    /// against, so this passes only if the bytes were handed over.
    #[test]
    fn an_archive_pack_feeds_the_engine_its_zone_maps() {
        let path = temp_dir("zoned_zip").join("zoned.openbounty");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).expect("create"));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("game.json", opts).expect("start");
        zip.write_all(ZONE_PACK.as_bytes()).expect("write");
        zip.start_file("maps/home.dat", opts).expect("start");
        zip.write_all(HOME_TILES).expect("write");
        zip.finish().expect("finish");

        let pack = Pack::open(&path).expect("archive pack");
        assert_eq!(
            second_tile_terrain(&pack),
            openbounty_engine::tile::Terrain::Forest
        );
    }

    #[test]
    fn a_pack_without_game_json_does_not_load() {
        let dir = temp_dir("no_json");
        let pack = Pack::open(&dir).expect("dir pack");
        assert!(load_resources(&pack).is_err());
    }

    /// Every documented invocation passes `.../game.json`; that names the
    /// directory pack holding it.
    #[test]
    fn a_game_json_path_names_its_directory_pack() {
        let dir = temp_dir("legacy_arg");
        let json = dir.join("game.json");
        std::fs::write(&json, b"{}").expect("write");
        assert_eq!(pack_location(&json), dir);

        let archive = dir.join("x.openbounty");
        assert_eq!(pack_location(&archive), archive);
        assert_eq!(pack_location(&dir), dir);
    }

    #[test]
    fn a_directory_opens_as_a_loose_tree() {
        let dir = temp_dir("dir");
        std::fs::write(dir.join("game.json"), br#"{"pack_id":"loose"}"#).expect("write");
        let pack = Pack::open(&dir).expect("dir pack opens");

        assert_eq!(
            pack.read("game.json").as_deref(),
            Some(&br#"{"pack_id":"loose"}"#[..])
        );
        // C: pack_hash is "" for dir packs.
        assert_eq!(pack.hash(), "");
        assert_eq!(pack.root(), Some(dir.as_path()));
    }

    #[test]
    fn a_file_opens_as_an_archive() {
        let dir = temp_dir("zip");
        let path = write_archive(&dir);
        let pack = Pack::open(&path).expect("archive opens");

        assert_eq!(
            pack.read("game.json").as_deref(),
            Some(&br#"{"pack_id":"test"}"#[..])
        );
        assert_eq!(pack.read("art/tile.png").as_deref(), Some(&b"PNGBYTES"[..]));
        // C hashes the archive bytes; a directory has no hash.
        assert_eq!(pack.hash().len(), 16, "FNV1a-64 as hex");
        assert_eq!(pack.root(), None);
    }

    #[test]
    fn a_missing_entry_reads_none_in_both_modes() {
        let dir = temp_dir("missing");
        std::fs::write(dir.join("game.json"), b"{}").expect("write");
        assert!(Pack::open(&dir).expect("dir").read("nope.png").is_none());

        let archive = write_archive(&temp_dir("missing2"));
        assert!(Pack::open(&archive)
            .expect("zip")
            .read("nope.png")
            .is_none());
    }

    #[test]
    fn a_file_that_is_not_a_zip_fails_to_open() {
        let dir = temp_dir("bad");
        let path = dir.join("broken.openbounty");
        std::fs::write(&path, b"not a zip at all").expect("write");
        assert!(Pack::open(&path).is_err());
    }

    /// The same bytes always hash the same; different bytes do not.
    #[test]
    fn the_archive_hash_is_stable_and_content_dependent() {
        let a = Pack::open(&write_archive(&temp_dir("h1"))).expect("a");
        let b = Pack::open(&write_archive(&temp_dir("h2"))).expect("b");
        assert_eq!(a.hash(), b.hash(), "same content, same hash");

        let other = temp_dir("h3").join("other.openbounty");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&other).expect("create"));
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("game.json", opts).expect("start");
        zip.write_all(br#"{"pack_id":"other"}"#).expect("write");
        zip.finish().expect("finish");
        let c = Pack::open(&other).expect("c");
        assert_ne!(a.hash(), c.hash(), "different content, different hash");
    }
}
