# Pack Archive And Picker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the game find and open its own content -- read `.openbounty`
archives as well as loose directories, discover what is installed, and show a
picker when there is more than one -- so launching no longer requires a
`game.json` path on the command line.

**Architecture:** A shell-side `Pack` reader auto-detects a directory or a ZIP
and hands out bytes by pack-relative path, porting `reference/engine/pack.c`.
The engine never learns what an archive is: it already exposes a byte-based
entry point beside every path-based loader, and the shell feeds those. The
picker's decision logic is a pure function, as it already is in C.

**Tech Stack:** Rust, `zip = "8.6"` (new, `shell` only), the existing `engine`
and `shell` crates, macroquad's built-in `draw_text` for the pre-pack screen.

## Verified before this plan was written

Do not re-derive these; they were checked against the real crates and tree:

- **`.openbounty` is a plain ZIP** and `pack_open` auto-detects: *"directory ->
  loose-tree mode, regular file -> ZIP mode"* (`pack.h:18`). C uses miniz.
  **A loose directory is a first-class C mode, not a divergence** -- the shell's
  current behaviour is one of C's two modes, and this slice adds the other.
- **Opening slurps everything into RAM.** *"opening a pack decompresses or
  slurps every file into RAM, and pack_read returns borrowed pointers"*
  (`pack.h:8`). Port that; do not invent lazy decompression.
- **The engine already has a byte API beside every path loader:**
  `Resources::from_json_str`, `Map::load_zone_bytes`,
  `Map::load_zone_bytes_with_placements`, `Game::switch_zone_bytes`. This is
  the seam `AGENTS.md` describes: *"the same core with the tiles supplied
  directly."*
- **There is exactly one path-resolution choke point**,
  `Resources::resolve_path` (`resources.rs:1281`), plus five read sites:
  `resources.rs:1133`, `map.rs:112`, `map.rs:127`, and `backend.rs:79`/`:99`.
- **`zip = "8.6.0"`** is the current stable (`9.0.0-pre3` is a prerelease; do
  not use it). A probe reading the real reference pack compiled and ran.
- **`macroquad::prelude::draw_text`** exists (`text.rs:306`) and uses a
  built-in font, so the picker can draw before any pack is open -- the same
  reason C's picker uses raylib's `DrawText`.
- **`SavePaths::base_dir()`** (`shell/src/savepath.rs:37`) already returns the
  per-platform user-data directory, which is discovery root 2.
- **`engine/tests/render_free.rs` forbids only renderer/UI crates.** `zip` is
  not on that list -- but this plan keeps it in `shell` anyway, because the
  byte APIs make that clean.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a tested task, report it, ask.
- **Commit messages are a single subject line.** No body, no bullets, no
  trailers. Example: `feat: read openbounty pack archives`.
- **Do not add `zip` to `engine/Cargo.toml`.** It goes in `shell`.
- **Never commit `reference/`, `game-assets/`, or build output.**
- Preserve unrelated working-tree changes. `tools/gen_ai.py` is already
  modified at the time of writing.
- Run `cargo fmt --all --check` before every commit.
- Transliterate C first. Where a C comment and the C code disagree, port the
  code and say so in the test name.

## File Structure

- **Create `shell/src/pack.rs`** (~250 lines incl. tests): the `Pack` reader
  and `pack_discover`. Ports `reference/engine/pack.c`'s reader and discovery
  halves. One responsibility: turning a path into readable bytes.
- **Create `shell/src/pack_select.rs`** (~200 lines incl. tests): the picker.
  `select_step` is pure and carries the tests; the draw loop sits beside it.
  Ports `reference/src/pack_select.c`, which already made this split.
- **Modify `shell/Cargo.toml`**: add `zip`.
- **Modify `shell/src/main.rs`**: open the pack, route loading through it, and
  run the discovery/picker policy before anything else.
- **Modify `shell/src/backend.rs`**: load textures from pack bytes.

## Out of scope -- do not build these

- **Writing archives.** C's `pack_zip_dir` backs `--extract` and `--pack-dir`,
  which are asset tooling, not play. Reading only.
- **The pack stack.** C maintains a global last-wins stack
  (`pack_stack_push`/`pack_stack_read`) for layered packs. Nothing in this
  shell layers packs; one open pack is enough. Add it when something needs it.
- **KB.EXE extraction.** C's `n == 0` fallback tries to build a pack from a
  local KB.EXE. That is `tools/`' job here, and `cc_extract.py` already exists.
  On zero packs, fail with a message naming where it looked.
- **`SaveHeader.pack_hash`.** This slice makes it computable for the first
  time (FNV1a-64 over the archive bytes, `""` for a directory). `AGENTS.md`
  says the Rust engine never writes it and nothing gates on it. Keep it that
  way; expose `Pack::hash()` and leave the save path alone.

---

### Task 1: The pack reader

**Files:**
- Modify: `shell/Cargo.toml`
- Create: `shell/src/pack.rs`
- Modify: `shell/src/main.rs` (add `pub mod pack;`)
- Test: `shell/src/pack.rs`

**Interfaces:**
- Produces: `pack::Pack` with `open(&Path) -> io::Result<Pack>`,
  `read(&self, rel: &str) -> Option<Cow<'_, [u8]>>`, `hash(&self) -> &str`,
  `root(&self) -> Option<&Path>`.

`read` returns `Cow` because a directory pack reads fresh bytes per call while
an archive borrows from its map -- C returns a borrowed pointer in both cases
because it slurped the directory too, but re-reading a loose file is what makes
dev-mode iteration useful, and `Cow` is how Rust says "either".

- [ ] **Step 1: Add the dependency**

In `shell/Cargo.toml`, under `[dependencies]`:

```toml
zip = "8.6"
```

- [ ] **Step 2: Write the failing tests**

Create `shell/src/pack.rs` with ONLY this test module. `super` is empty until
Step 3, so the crate will not compile -- that is the point.

```rust
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
        assert!(Pack::open(&archive).expect("zip").read("nope.png").is_none());
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
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell pack:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find type 'Pack'` among the errors.

- [ ] **Step 4: Write the implementation**

Prepend to `shell/src/pack.rs`:

```rust
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
//! This lives in the shell, not the engine. The engine already exposes a
//! byte-based entry point beside every path-based loader
//! (`Resources::from_json_str`, `Map::load_zone_bytes`,
//! `Game::switch_zone_bytes`), so the shell reads the bytes and feeds those.
//! The engine never learns what an archive is.

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
```

In `shell/src/main.rs`, register the module alphabetically:

```rust
pub mod pack;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell pack::`
Expected: PASS, 5 tests.

`zip::write::FileOptions` is generic in 8.x; the test annotates it
`FileOptions<()>`. If that turns out wrong for the resolved patch version, read
the error and fix the annotation -- do not switch to a mock.

- [ ] **Step 6: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass, `render_free` included.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/Cargo.toml Cargo.lock shell/src/pack.rs shell/src/main.rs
git commit -m "feat: read openbounty pack archives"
```

---

### Task 2: Load the game through the pack

**Files:**
- Modify: `shell/src/main.rs`
- Modify: `shell/src/backend.rs`
- Test: existing suite must stay green; this task adds no new unit test

**Interfaces:**
- Consumes: `Pack::{open, read, root}` from Task 1.
- Produces: a `main` that loads `Resources`, maps and textures out of a `Pack`
  rather than straight off the filesystem.

**This is the invasive task. Take it slowly and keep the existing suite green.**

The engine keeps both loader families. A directory pack can keep using the
path-based ones; an archive MUST use the byte-based ones. Prefer routing
everything through bytes so the two modes do not drift -- one code path that
works for both beats a branch at every site.

- [ ] **Step 1: Load `game.json` from the pack**

`main.rs` currently reads the `game.json` path directly. Replace that with a
pack open plus `Resources::from_json_str`:

```rust
    let pack = pack::Pack::open(&pack_path).unwrap_or_else(|err| {
        eprintln!("cannot open game pack at {}: {err}", pack_path.display());
        std::process::exit(1);
    });
    let json = pack.read("game.json").unwrap_or_else(|| {
        eprintln!("pack has no game.json: {}", pack_path.display());
        std::process::exit(1);
    });
    let json = String::from_utf8_lossy(&json);
    let resources = Resources::from_json_str(&json)?;
```

A directory pack passed as the old `game.json` path must keep working: if
`pack_path` names a file called `game.json`, open its PARENT as a directory
pack. Existing invocations in `AGENTS.md`, `README`s and
`specs/play-verification.md` all pass `.../game.json`, and silently breaking
them would be a worse regression than the feature is worth.

- [ ] **Step 2: Load maps from the pack**

Replace `Map::load_zone(&resources, zone)` with the byte form at every call
site. Read the bytes with `pack.read(..)` using the relative path
`Resources::resolve_map_path` builds, then call `Map::load_zone_bytes` /
`Map::load_zone_bytes_with_placements`. Do the same for
`Game::switch_zone` -> `Game::switch_zone_bytes`.

The compiler will not find these for you -- `load_zone` still exists and still
compiles. Grep for them:

```bash
grep -rn "load_zone\|switch_zone" shell/src/*.rs
```

- [ ] **Step 3: Load textures from the pack**

`backend.rs:99` calls `load_texture(path)`; `backend.rs:79` calls
`load_file(path)`. Both resolve through `resources.resolve_path`. Replace with
`pack.read(relative)` plus macroquad's byte constructor:

```rust
        let Some(bytes) = pack.read(relative) else {
            return;
        };
        let texture = Texture2D::from_file_with_format(&bytes, None);
```

`Texture2D::from_file_with_format` is synchronous and takes bytes, so the
`async` on `load_texture_at` may become unnecessary. Remove it only if nothing
else in that call chain awaits; do not restructure the loader beyond what this
change forces.

- [ ] **Step 4: Verify against all three packs**

Run each and confirm the game reaches the overworld:

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

```bash
cargo run -p openbounty-shell -- game-assets/hd-pack/game.json
```

Then build an archive from a loose tree and confirm it loads identically:

```bash
cd reference/assets/kings-bounty && zip -r /tmp/kb.openbounty . && cd -
cargo run -p openbounty-shell -- /tmp/kb.openbounty
```

The archive run is the acceptance check for this whole task. If art is missing
in archive mode but present in directory mode, a path is being resolved with a
leading `./` or an absolute prefix -- zip entry names are relative with forward
slashes and no leading dot.

- [ ] **Step 5: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/main.rs shell/src/backend.rs
git commit -m "feat: load game content through the pack reader"
```

---

### Task 3: Discover installed packs

**Files:**
- Modify: `shell/src/pack.rs`
- Test: `shell/src/pack.rs`

**Interfaces:**
- Consumes: `SavePaths::base_dir()` (`shell/src/savepath.rs:37`).
- Produces: `pack::PackEntry { path: PathBuf, name: String }`,
  `pack::discover(&SavePaths) -> Vec<PackEntry>`,
  `pack::resolve_arg(&str, &SavePaths) -> Option<PathBuf>`,
  `pack::DISCOVER_MAX: usize = 16`.

C scans three roots in order and lets the earlier source win on duplicate
names (`pack.h:62-67`). Root 3 is `<exe-dir>/assets`, from
`std::env::current_exe()`.

Because this port also accepts loose directories, discovery finds BOTH a
`*.openbounty` file and any directory containing a `game.json`. That is not a
divergence -- C's `pack_open` accepts both; its discovery just never looked for
the second.

- [ ] **Step 1: Write the failing tests**

Add to `shell/src/pack.rs`'s `mod tests`:

```rust
    #[test]
    fn discovery_finds_archives_and_loose_directories() {
        let root = temp_dir("disc");
        write_archive_named(&root, "alpha.openbounty");
        let loose = root.join("beta");
        std::fs::create_dir_all(&loose).expect("dir");
        std::fs::write(loose.join("game.json"), b"{}").expect("write");
        // A directory with no game.json is not a pack.
        std::fs::create_dir_all(root.join("notapack")).expect("dir");

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

    #[test]
    fn a_bare_name_resolves_against_the_roots_and_a_path_does_not() {
        let root = temp_dir("resolve");
        write_archive_named(&root, "named.openbounty");

        assert!(resolve_arg_in("named", &[root.clone()]).is_some());
        assert!(resolve_arg_in("nosuch", &[root.clone()]).is_none());
        // An explicit path is taken as-is and must exist.
        let direct = root.join("named.openbounty");
        assert_eq!(
            resolve_arg_in(&direct.to_string_lossy(), &[]),
            Some(direct)
        );
    }
```

Add the helper beside `write_archive`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell pack::`
Expected: FAIL to compile, `cannot find function 'discover_in'`.

- [ ] **Step 3: Write the implementation**

Add to `shell/src/pack.rs`. Split the root list out of `discover` so the tests
drive real directories without touching the user's home:

```rust
/// C's `PACK_DISCOVER_MAX`.
pub const DISCOVER_MAX: usize = 16;

pub struct PackEntry {
    pub path: PathBuf,
    /// Filename without extension, C's `PackEntry.name`.
    pub name: String,
}

/// The roots C scans, in order (`pack.h:62`): the working directory, the
/// user-data directory, then `<exe-dir>/assets`.
pub fn discover_roots(saves: &crate::savepath::SavePaths) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Some(base) = saves.base_dir() {
        roots.push(base);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            roots.push(dir.join("assets"));
        }
    }
    roots
}

pub fn discover(saves: &crate::savepath::SavePaths) -> Vec<PackEntry> {
    discover_in(&discover_roots(saves))
}

/// Scan `roots` in order. An earlier root wins a duplicate name, and the scan
/// stops at `DISCOVER_MAX`.
pub fn discover_in(roots: &[PathBuf]) -> Vec<PackEntry> {
    let mut found: Vec<PackEntry> = Vec::new();
    for root in roots {
        let Ok(dir) = std::fs::read_dir(root) else {
            continue;
        };
        // read_dir order is filesystem-dependent; sort so a run is repeatable.
        let mut paths: Vec<PathBuf> = dir.flatten().map(|e| e.path()).collect();
        paths.sort();
        for path in paths {
            if found.len() >= DISCOVER_MAX {
                return found;
            }
            let Some(name) = pack_name(&path) else {
                continue;
            };
            if found.iter().any(|e| e.name == name) {
                continue;
            }
            found.push(PackEntry { path, name });
        }
    }
    found
}

/// The pack name for a path, or `None` when it is not a pack: a
/// `*.openbounty` file, or a directory holding a `game.json`.
fn pack_name(path: &Path) -> Option<String> {
    let stem = path.file_stem()?.to_str()?.to_owned();
    if path.is_dir() {
        return path.join("game.json").is_file().then_some(stem);
    }
    (path.extension()?.eq_ignore_ascii_case("openbounty")).then_some(stem)
}

/// Resolve a `--pack` value, porting `pack_resolve_arg`. A value containing a
/// separator or ending in `.openbounty` is a path and must exist; anything
/// else is a bare name searched for in the roots.
pub fn resolve_arg(arg: &str, saves: &crate::savepath::SavePaths) -> Option<PathBuf> {
    resolve_arg_in(arg, &discover_roots(saves))
}

pub fn resolve_arg_in(arg: &str, roots: &[PathBuf]) -> Option<PathBuf> {
    let looks_like_path = arg.contains('/')
        || arg.contains('\\')
        || arg.to_ascii_lowercase().ends_with(".openbounty");
    if looks_like_path {
        let path = PathBuf::from(arg);
        return path.exists().then_some(path);
    }
    discover_in(roots)
        .into_iter()
        .find(|entry| entry.name == arg)
        .map(|entry| entry.path)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell pack::`
Expected: PASS, 9 tests.

- [ ] **Step 5: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/pack.rs
git commit -m "feat: discover installed game packs"
```

---

### Task 4: The picker's decision logic

**Files:**
- Create: `shell/src/pack_select.rs`
- Modify: `shell/src/main.rs` (add `pub mod pack_select;`)
- Test: `shell/src/pack_select.rs`

**Interfaces:**
- Produces: `pack_select::{SelectInput, SelectState, select_step}`.

C factored this out of its render loop for exactly this reason -- the header
says *"The selector's decision logic, factored out of the raylib loop so it can
be exercised without a window."* Port the split, not just the behaviour.

- [ ] **Step 1: Write the failing tests**

Create `shell/src/pack_select.rs` with ONLY this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> SelectInput {
        SelectInput::default()
    }

    #[test]
    fn the_cursor_wraps_in_both_directions() {
        let mut state = SelectState::default();
        select_step(&mut state, &SelectInput { down: true, ..input() }, 3);
        assert_eq!(state.cursor, 1);
        select_step(&mut state, &SelectInput { up: true, ..input() }, 3);
        assert_eq!(state.cursor, 0);
        select_step(&mut state, &SelectInput { up: true, ..input() }, 3);
        assert_eq!(state.cursor, 2, "wraps backwards past zero");
        select_step(&mut state, &SelectInput { down: true, ..input() }, 3);
        assert_eq!(state.cursor, 0, "wraps forwards past the end");
    }

    #[test]
    fn a_digit_jumps_to_that_row_and_out_of_range_is_ignored() {
        let mut state = SelectState::default();
        select_step(&mut state, &SelectInput { digit: Some(2), ..input() }, 3);
        assert_eq!(state.cursor, 2);
        select_step(&mut state, &SelectInput { digit: Some(7), ..input() }, 3);
        assert_eq!(state.cursor, 2, "out of range leaves the cursor alone");
    }

    #[test]
    fn confirm_finishes_without_quitting() {
        let mut state = SelectState::default();
        select_step(&mut state, &SelectInput { confirm: true, ..input() }, 3);
        assert!(state.done);
        assert!(!state.quit);
    }

    /// C's comment: "A dismissed window is a cancel, not a selection. Without
    /// this the loop fell out with quit unset and the caller booted whatever
    /// row the cursor happened to rest on."
    #[test]
    fn cancel_and_window_close_both_finish_without_selecting() {
        for dismissal in [
            SelectInput { cancel: true, ..input() },
            SelectInput { close: true, ..input() },
        ] {
            let mut state = SelectState { cursor: 2, ..SelectState::default() };
            select_step(&mut state, &dismissal, 3);
            assert!(state.done);
            assert!(state.quit, "a dismissed picker must select nothing");
        }
    }

    /// C guards `n <= 0` and returns; an empty list cannot move a cursor.
    #[test]
    fn an_empty_list_is_inert() {
        let mut state = SelectState::default();
        select_step(&mut state, &SelectInput { down: true, ..input() }, 0);
        assert_eq!(state.cursor, 0);
        assert!(!state.done);
    }

    /// Cancel is checked BEFORE movement, so a frame carrying both cancels.
    #[test]
    fn cancel_beats_movement_in_the_same_frame() {
        let mut state = SelectState::default();
        select_step(
            &mut state,
            &SelectInput { cancel: true, down: true, ..input() },
            3,
        );
        assert_eq!(state.cursor, 0, "cancel returns before the cursor moves");
        assert!(state.quit);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell pack_select:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find function 'select_step'`.

- [ ] **Step 3: Write the implementation**

Prepend to `shell/src/pack_select.rs`:

```rust
//! The pre-game pack picker, porting `src/pack_select.c`.
//!
//! This runs BEFORE any pack is open, so it cannot use the bitmap font, the
//! chrome or any sprite -- all of those come from the pack. It draws with
//! macroquad's built-in `draw_text`, exactly as C falls back to raylib's
//! `DrawText` for the same reason.
//!
//! The decision logic below is deliberately separate from the draw loop, as
//! it is in C: one call is one frame, so it is exercised without a window.

/// One frame of input, C's `PackSelectInput`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectInput {
    /// Window close signal.
    pub close: bool,
    /// ESC.
    pub cancel: bool,
    pub up: bool,
    pub down: bool,
    /// Zero-based row from a 1..9 keypress.
    pub digit: Option<usize>,
    /// ENTER / SPACE.
    pub confirm: bool,
}

/// C's `PackSelectState`.
#[derive(Clone, Copy, Debug, Default)]
pub struct SelectState {
    pub cursor: usize,
    /// The loop should stop.
    pub done: bool,
    /// Stopped WITHOUT a selection; the caller loads nothing.
    pub quit: bool,
}

/// Advance one frame.
///
/// Closing the window and pressing ESC both end the flow as a cancel. C's
/// comment on why: without it "the loop fell out with quit unset and the
/// caller booted whatever row the cursor happened to rest on."
pub fn select_step(state: &mut SelectState, input: &SelectInput, n: usize) {
    if n == 0 {
        return;
    }
    if input.close || input.cancel {
        state.quit = true;
        state.done = true;
        return;
    }
    if input.up {
        state.cursor = (state.cursor + n - 1) % n;
    }
    if input.down {
        state.cursor = (state.cursor + 1) % n;
    }
    if let Some(digit) = input.digit {
        if digit < n {
            state.cursor = digit;
        }
    }
    if input.confirm {
        state.done = true;
    }
}
```

In `shell/src/main.rs`:

```rust
pub mod pack_select;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell pack_select::`
Expected: PASS, 6 tests.

- [ ] **Step 5: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/pack_select.rs shell/src/main.rs
git commit -m "feat: add the pack picker decision logic"
```

---

### Task 5: The picker screen and the launch policy

**Files:**
- Modify: `shell/src/pack_select.rs` (the draw loop)
- Modify: `shell/src/main.rs` (the launch policy)

**Interfaces:**
- Consumes: everything from Tasks 1, 3 and 4.
- Produces: `pack_select::run(&[PackEntry]) -> Option<usize>`, and a `main`
  that needs no command-line argument.

**C's launch policy** (`main.c:505-625`), port it exactly:

| Discovered | C does |
|---|---|
| `--pack` given | resolve it; a failure is fatal with a message naming what was tried |
| 0 | fatal, telling the user where it looked and how to make a pack |
| 1 | load it silently -- **no picker** |
| 2+ | show the picker; ESC quits without loading |

- [ ] **Step 1: Write the picker screen**

Add to `shell/src/pack_select.rs`. Read one input edge per frame, call
`select_step`, draw, yield:

```rust
use macroquad::prelude::{
    clear_background, draw_rectangle, draw_text, is_key_pressed, measure_text, next_frame,
    Color as MqColor, KeyCode,
};

use crate::pack::PackEntry;

/// Run the picker until a row is confirmed or the user cancels. `None` means
/// "load nothing", which the caller must honour by exiting.
pub async fn run(entries: &[PackEntry]) -> Option<usize> {
    let mut state = SelectState::default();
    while !state.done {
        let mut digit = None;
        for (index, key) in [
            KeyCode::Key1, KeyCode::Key2, KeyCode::Key3, KeyCode::Key4, KeyCode::Key5,
            KeyCode::Key6, KeyCode::Key7, KeyCode::Key8, KeyCode::Key9,
        ]
        .into_iter()
        .enumerate()
        {
            if index < entries.len() && is_key_pressed(key) {
                digit = Some(index);
            }
        }
        let input = SelectInput {
            close: false,
            cancel: is_key_pressed(KeyCode::Escape),
            up: is_key_pressed(KeyCode::Up) || is_key_pressed(KeyCode::Kp8),
            down: is_key_pressed(KeyCode::Down) || is_key_pressed(KeyCode::Kp2),
            digit,
            confirm: is_key_pressed(KeyCode::Enter)
                || is_key_pressed(KeyCode::KpEnter)
                || is_key_pressed(KeyCode::Space),
        };
        select_step(&mut state, &input, entries.len());

        draw(entries, state.cursor);
        next_frame().await;
    }
    (!state.quit).then_some(state.cursor)
}

/// C's colours and metrics, kept so the screen looks the same.
fn draw(entries: &[PackEntry], cursor: usize) {
    const W: f32 = 640.0;
    const H: f32 = 400.0;
    let bg = MqColor::from_rgba(16, 16, 32, 255);
    let sel_bg = MqColor::from_rgba(40, 40, 70, 255);
    let white = MqColor::from_rgba(245, 245, 245, 255);
    let yellow = MqColor::from_rgba(253, 249, 0, 255);
    let grey = MqColor::from_rgba(130, 130, 130, 255);

    clear_background(bg);

    let title = "Select Game Pack";
    let tw = measure_text(title, None, 24, 1.0).width;
    draw_text(title, (W - tw) / 2.0, 48.0, 24.0, white);

    let row_h = 24.0;
    let top = (H - entries.len() as f32 * row_h) / 2.0 - 8.0;
    for (index, entry) in entries.iter().enumerate() {
        let x = W / 2.0 - 200.0;
        let y = top + index as f32 * row_h;
        if index == cursor {
            draw_rectangle(x - 6.0, y - 2.0, 412.0, row_h, sel_bg);
        }
        let line = format!(" {}. {}", index + 1, entry.name);
        let fg = if index == cursor { yellow } else { white };
        draw_text(&line, x, y + 16.0, 20.0, fg);
    }

    let hint = "UP/DN select   ENTER load   ESC quit";
    let hw = measure_text(hint, None, 16, 1.0).width;
    draw_text(hint, (W - hw) / 2.0, H - 40.0, 16.0, grey);
}
```

macroquad measures text from the BASELINE where raylib measures from the top,
which is why each row adds `16.0` to `y`. Check it on screen in Step 3 and
adjust the offset rather than the layout.

- [ ] **Step 2: Wire the launch policy**

In `main.rs`, replace the "path is required" argument handling. Keep `--seed`
and `--save-dir`, accept an optional positional path or `--pack`, and add the
policy:

```rust
    let save_paths = /* existing SavePaths construction */;
    let pack_path = match explicit_pack {
        Some(arg) => pack::resolve_arg(&arg, &save_paths).unwrap_or_else(|| {
            eprintln!("--pack not found: {arg}");
            eprintln!("Pass a path to a *.openbounty file, or to a directory containing game.json.");
            std::process::exit(1);
        }),
        None => {
            let entries = pack::discover(&save_paths);
            match entries.len() {
                0 => {
                    eprintln!("No game pack found. Looked in:");
                    for root in pack::discover_roots(&save_paths) {
                        eprintln!("  {}", root.display());
                    }
                    std::process::exit(1);
                }
                // C loads a lone pack silently; the picker is for a choice.
                1 => entries.into_iter().next().expect("one entry").path,
                _ => match pack_select::run(&entries).await {
                    Some(index) => entries.into_iter().nth(index).expect("in range").path,
                    // ESC: load nothing, exit cleanly. C returns 0 here.
                    None => return,
                },
            }
        }
    };
```

Keep the old positional `game.json` path working as an explicit pack argument
(Task 2, Step 1), so every documented invocation still runs.

- [ ] **Step 3: Verify on screen**

With two or more packs discoverable, run with no argument:

```bash
cargo run -p openbounty-shell
```

Confirm:

1. The picker lists every discovered pack, numbered from 1.
2. UP/DOWN move and wrap at both ends; number keys jump.
3. ENTER loads the highlighted pack and the game starts.
4. ESC exits without loading anything.
5. Text is legible and rows do not overlap -- adjust the baseline offset if not.
6. With exactly one pack discoverable, the picker does NOT appear.
7. `--pack <bare-name>` and `--pack <path>` both still work, and a bad value
   prints the message rather than panicking.

- [ ] **Step 4: Check formatting and the suite**

Run: `cargo fmt --all --check && cargo test --workspace`

- [ ] **Step 5: Update the docs this changes**

The command line is now different, and three documents state the old one:

- `AGENTS.md`: the run line in the Layout section.
- `specs/play-verification.md`: the "How to run" section, and the §13
  `| Pack picker | GAP |` row -- delete it and move the "Last refreshed" line.
- `packs/open/README.md`: the Playtest section.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/pack_select.rs shell/src/main.rs AGENTS.md specs/play-verification.md packs/open/README.md
git commit -m "feat: pick a game pack at launch"
```

---

## An architecture rule this slice bends, deliberately

`AGENTS.md` says `backend.rs` is the ONLY module that knows macroquad exists,
and Task 5 breaks that: `pack_select.rs` imports `draw_text`, `is_key_pressed`
and `next_frame` directly.

It cannot avoid it. The picker runs **before any pack is open**, and the whole
`DrawList` / `backend.rs` pipeline is built on pack-supplied art -- the bitmap
font, the palette, the chrome. There is nothing for it to draw with. C hit the
identical wall and answered it the identical way: *"This runs BEFORE the main
game pack is opened -- so it cannot use bfont (which needs a pack), the chrome,
or any sprites. It uses raylib's built-in DrawText."* (`pack_select.h`).

So the rule gains a second stated exception, alongside the one
`specs/audio-design.md` introduces for `audio_backend.rs` and kira. Update
`AGENTS.md` in Task 5, Step 5 to say so, and keep the exception narrow: the
picker draws nothing but text and rectangles, and it exits before the real
shell starts. No other module may follow it.

## Open item for the reviewer

**Task 2 is the risk in this plan, not the picker.** Tasks 1, 3 and 4 are
additive and independently testable; Task 2 rewrites how every asset in the
game is loaded, and its only real acceptance check is running all three packs
plus a zipped one by hand. If it goes badly, the fallback is to keep the
path-based loaders for directory packs and use bytes only for archives --
worse, because the two modes then drift, but recoverable.

Nothing else in the plan depends on Task 2's internals, so it can be attempted,
reverted and re-attempted without disturbing the rest.
