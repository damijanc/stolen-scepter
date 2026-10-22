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

/// C's `PACK_DISCOVER_MAX`.
pub const DISCOVER_MAX: usize = 16;

pub struct PackEntry {
    pub path: PathBuf,
    /// Filename without extension, C's `PackEntry.name`.
    pub name: String,
}

const PACK_EXT: &str = ".openbounty";

/// The roots C scans, in order (`pack.h:62`): the working directory, the
/// user-data directory, then `<exe-dir>/assets`. A root that does not exist
/// is harmless -- it scans as empty -- so unlike C this does not stat
/// `assets` before listing it.
///
/// DIVERGENCE: two roots C does not have, for the game's own pack. `packs/`
/// under the working directory, right after it, so `cargo run` in a checkout
/// finds `packs/stolen-scepter/`; and a macOS app bundle's
/// `Contents/Resources/packs/`, last, which is where the DMG ships it.
pub fn discover_roots(saves: &crate::savepath::SavePaths) -> Vec<PathBuf> {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    roots_for(saves.base_dir(), exe_dir.as_deref())
}

/// [`discover_roots`] over explicit inputs, so the order is testable.
fn roots_for(base: Option<PathBuf>, exe_dir: Option<&Path>) -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("."), PathBuf::from("packs")];
    if let Some(base) = base {
        roots.push(base);
    }
    if let Some(dir) = exe_dir {
        roots.push(dir.join("assets"));
        roots.push(dir.join("../Resources/packs"));
    }
    roots
}

pub fn discover(saves: &crate::savepath::SavePaths) -> Vec<PackEntry> {
    discover_in(&discover_roots(saves))
}

/// Scan `roots` in order, porting `pack_discover`/`scan_one_dir`. An earlier
/// root wins a duplicate name, and the scan stops at `DISCOVER_MAX`.
///
/// Two deliberate differences from C. It also lists a directory holding a
/// `game.json`, because `pack_open` accepts one and this shell's packs ship
/// loose; C's scan only ever matched `*.openbounty`. And each root is listed
/// in sorted order, where C takes `readdir`'s, which is filesystem-dependent.
pub fn discover_in(roots: &[PathBuf]) -> Vec<PackEntry> {
    let mut found: Vec<PackEntry> = Vec::new();
    for root in roots {
        let Ok(dir) = std::fs::read_dir(root) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = dir.flatten().map(|entry| entry.path()).collect();
        paths.sort();
        for path in paths {
            if found.len() >= DISCOVER_MAX {
                return found;
            }
            let Some(name) = pack_name(&path) else {
                continue;
            };
            if found.iter().any(|entry| entry.name == name) {
                continue;
            }
            found.push(PackEntry { path, name });
        }
    }
    found
}

/// The pack name for a path, or `None` when it is not a pack: a
/// `*.openbounty` file (matched case-sensitively, as C's `ends_with` does)
/// named without its extension, or a directory holding a `game.json` named
/// as it stands.
fn pack_name(path: &Path) -> Option<String> {
    let file_name = path.file_name()?.to_str()?;
    if path.is_dir() {
        return path
            .join("game.json")
            .is_file()
            .then(|| file_name.to_owned());
    }
    let stem = file_name.strip_suffix(PACK_EXT)?;
    Some(stem.to_owned())
}

/// The window title for this launch: the `title` of the pack that will load
/// under C's launch policy (an explicit pack, else the single discovered
/// one), or "OpenBounty" when that is not known before the window opens --
/// no pack, a picker, or a pack that will not open.
pub fn launch_title(explicit: Option<&str>, saves: &crate::savepath::SavePaths) -> String {
    let path = match explicit {
        Some(arg) => resolve_arg(arg, saves),
        None => {
            let entries = discover(saves);
            (entries.len() == 1).then(|| entries[0].path.clone())
        }
    };
    path.and_then(|path| pack_title(&pack_location(&path)))
        .unwrap_or_else(|| "OpenBounty".to_owned())
}

/// A pack's `title`, or `None` when it will not open or names none.
fn pack_title(path: &Path) -> Option<String> {
    let pack = Pack::open(path).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&pack.read("game.json")?).ok()?;
    json.get("title")?
        .as_str()
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
}

/// Resolve a `--pack` value, porting `pack_resolve_arg`.
pub fn resolve_arg(arg: &str, saves: &crate::savepath::SavePaths) -> Option<PathBuf> {
    resolve_arg_in(arg, &discover_roots(saves))
}

/// A value containing a separator or ending in `.openbounty` is a path and
/// must exist. Anything else is a bare name: each root in turn is tried for
/// `<arg>.openbounty`, then `<arg>`, and the first that exists wins. Like C,
/// this stats candidates directly rather than searching the capped discovery
/// list, and does not check that a bare directory holds a `game.json` -- a
/// wrong one fails at open, with the open's own message.
pub fn resolve_arg_in(arg: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    if arg.is_empty() {
        return None;
    }
    let has_sep = arg.contains('/') || arg.contains('\\');
    if has_sep || arg.ends_with(PACK_EXT) {
        let path = PathBuf::from(arg);
        return path.exists().then_some(path);
    }
    for root in roots {
        for suffix in [PACK_EXT, ""] {
            let candidate = root.join(format!("{arg}{suffix}"));
            if candidate.exists() {
                return Some(candidate);
            }
        }
    }
    None
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

    fn write_archive_named(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        let file = std::fs::File::create(&path).expect("create archive");
        let mut zip = zip::ZipWriter::new(file);
        let opts: zip::write::FileOptions<()> = zip::write::FileOptions::default();
        zip.start_file("game.json", opts).expect("start");
        zip.write_all(b"{}").expect("write");
        zip.finish().expect("finish");
        path
    }

    #[test]
    fn the_window_title_comes_from_the_pack_that_will_load() {
        let saves = crate::savepath::SavePaths::with_override("/nonexistent");
        let ours = concat!(env!("CARGO_MANIFEST_DIR"), "/../packs/stolen-scepter");
        assert_eq!(
            launch_title(Some(ours), &saves),
            "The Quest for a Stolen Scepter"
        );
        assert_eq!(launch_title(Some("/no/such/pack"), &saves), "OpenBounty");
    }

    #[test]
    fn the_roots_add_the_checkout_and_bundle_pack_folders_to_cs_three() {
        let exe = Path::new("/Applications/Game.app/Contents/MacOS");
        let roots = roots_for(Some(PathBuf::from("/data")), Some(exe));
        assert_eq!(
            roots,
            vec![
                PathBuf::from("."),
                PathBuf::from("packs"),
                PathBuf::from("/data"),
                exe.join("assets"),
                exe.join("../Resources/packs"),
            ]
        );
        assert_eq!(
            roots_for(None, None),
            vec![PathBuf::from("."), PathBuf::from("packs")]
        );
    }

    #[test]
    fn a_checkout_discovers_exactly_the_games_own_pack() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs");
        let found = discover_in(&[root]);
        let names: Vec<&str> = found.iter().map(|entry| entry.name.as_str()).collect();
        assert_eq!(names, vec!["stolen-scepter"]);
    }

    #[test]
    fn discovery_finds_archives_and_loose_directories() {
        let root = temp_dir("disc");
        write_archive_named(&root, "alpha.openbounty");
        let loose = root.join("beta");
        std::fs::create_dir_all(&loose).expect("dir");
        std::fs::write(loose.join("game.json"), b"{}").expect("write");
        // A directory with no game.json is not a pack.
        std::fs::create_dir_all(root.join("notapack")).expect("dir");
        // Nor is a stray file.
        std::fs::write(root.join("readme.txt"), b"hi").expect("write");

        let found = discover_in(&[root.clone()]);
        let mut names: Vec<&str> = found.iter().map(|e| e.name.as_str()).collect();
        names.sort_unstable();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    /// C: "Earlier source wins on duplicate names."
    #[test]
    fn an_earlier_root_wins_a_duplicate_name() {
        let first = temp_dir("dup1");
        let second = temp_dir("dup2");
        write_archive_named(&first, "same.openbounty");
        write_archive_named(&second, "same.openbounty");

        let found = discover_in(&[first.clone(), second]);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.starts_with(&first));
    }

    #[test]
    fn discovery_stops_at_the_cap() {
        let root = temp_dir("cap");
        for index in 0..(DISCOVER_MAX + 4) {
            write_archive_named(&root, &format!("pack{index:02}.openbounty"));
        }
        assert_eq!(discover_in(&[root]).len(), DISCOVER_MAX);
    }

    /// C's `ends_with(".openbounty")` is case-sensitive.
    #[test]
    fn discovery_matches_the_extension_case_sensitively() {
        let root = temp_dir("case");
        write_archive_named(&root, "shout.OPENBOUNTY");
        assert!(discover_in(&[root]).is_empty());
    }

    #[test]
    fn a_bare_name_resolves_against_the_roots_and_a_path_does_not() {
        let root = temp_dir("resolve");
        write_archive_named(&root, "named.openbounty");

        assert!(resolve_arg_in("named", &[root.clone()]).is_some());
        assert!(resolve_arg_in("nosuch", &[root.clone()]).is_none());
        // An explicit path is taken as-is and must exist.
        let direct = root.join("named.openbounty");
        assert_eq!(resolve_arg_in(&direct.to_string_lossy(), &[]), Some(direct));
        assert!(resolve_arg_in("/no/such/pack", &[root]).is_none());
    }

    /// C tries `<root>/<arg>.openbounty` before `<root>/<arg>` in each root,
    /// and only then moves to the next root.
    #[test]
    fn a_bare_name_prefers_the_archive_then_the_earlier_root() {
        let first = temp_dir("pref1");
        let second = temp_dir("pref2");
        std::fs::create_dir_all(first.join("kb")).expect("dir");
        write_archive_named(&first, "kb.openbounty");
        write_archive_named(&second, "kb.openbounty");

        assert_eq!(
            resolve_arg_in("kb", &[first.clone(), second]),
            Some(first.join("kb.openbounty"))
        );
    }

    /// C's bare-name lookup stats candidates directly; it is not a search of
    /// the capped discovery list, so a pack past the cap still resolves.
    #[test]
    fn a_bare_name_resolves_past_the_discovery_cap() {
        let root = temp_dir("past_cap");
        for index in 0..(DISCOVER_MAX + 4) {
            write_archive_named(&root, &format!("pack{index:02}.openbounty"));
        }
        let last = format!("pack{:02}", DISCOVER_MAX + 3);
        assert!(resolve_arg_in(&last, &[root]).is_some());
    }
}
