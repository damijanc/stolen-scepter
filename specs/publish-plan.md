# Publish Plan

Goal: a public GitHub repository and a signed, notarized macOS DMG that anyone
can download and play, free, with no file from the original game anywhere in
the repository, its history, or the download.

Not legal advice. The aim is that everything shipped is either our own work,
OpenBounty's MIT-licensed code (with its notice kept), or material under a
licence that allows redistribution.

## What ships, and its state

| Part | Today | Needed |
|---|---|---|
| Engine and shell | Rust port of OpenBounty (MIT) | Keep `NOTICES.md` and OpenBounty's copyright notice |
| Troop art (100 images) | Generated, all 25 units usable (`specs/ai-troop-findings.md`) | Collect into the pack |
| Other art (~210 images + font) | Original | Villains (68), map tiles and edges (~80), UI (43), combat (15), hero and boat (12), classes (4), bitmap font |
| Audio (6 tunes) | Original | New tunes |
| Maps (4 continents) | Original level design | New maps |
| `game.json` text | Original names, bios and strings | New writing |
| `game.json` numbers | Original tuning | Keep the rules, retune values lightly |
| Title | "King's Bounty" (a trademark) | Our own name |

## Phase 1 — Repository cleanup (before anything goes public)

Audit run 2026-10-22 over 376 tracked files and the full history:

- **Media: clean.** No image, audio, map, archive or binary from the original
  was ever committed. The only images in the history are the 267 procedurally
  generated PNGs of the old `packs/open/` attempt, which are our own.
  `reference/` and `game-assets/` have always been ignored.
- **Original names in tests.** A town, an artifact and a villain name from the
  original were used as test fixtures across 11 files. Done: renamed to
  neutral test names.
- **Tools aimed at the original's files.** Remove before publishing:
  - `tools/manual_word.py` answered the original's manual-lookup copy
    protection. Done: removed.
  - `tools/cc_extract.py`, `cc_output_map.py`, `upscale.py`,
    `make_hd_pack.py`, `hd_manifest.py`, `open_pack.py`: readers and
    upscalers of the original archives, not needed once the pack is ours.
    Done; the PNG reader and writer moved to `tools/pngio.py`.
    (`modern_proof.py` stays: it is our own procedural art code.)
  - `tools/hd_assets.json` listed the original archive entries. Done: the
    archive names are dropped; the slot list `gen_ai.py` needs stays.
  - `tools/asset_layout.py` reads the originals; its OUTPUT
    (`asset_layout.json`, numbers only) stays.
- **`specs/manual-reference-notes.md`** described the local copies of the
  original game and manual. Done: removed.
- **Old `packs/open/` attempt.** Done: deleted; the new pack takes its place
  (Phase 2). The procedural generators that wrote there are retargeted then.
- **Commit author emails.** The history has two author addresses, one a work
  address. Either accept them as public, or rewrite authorship with
  `git filter-repo --mailmap` before the first public push (this rewrites
  every commit hash; do it once, before going public, never after).
- **Name.** "King's Bounty" appears in 7 files (11 mentions) and in the repo
  and folder name. Keep factual credit ("a port of OpenBounty, a
  reimplementation of King's Bounty"), remove it as our product's name, and
  rename the GitHub repository to the new title.
- **Final gate.** Done: `tools/check_independent.py`, run by
  `.github/workflows/quality.yml` with `cargo fmt --check` and the tests. It
  found 85 more lines on its first run: continent, villain and town names in
  tests and specs (renamed), ~20 shell fallback texts that were the original's
  wording (rewritten in our own words), and the engine's default starting
  zone (now "home"). Temporary exemption, text check only: `tools/hd_assets.json`,
  `tools/asset_layout.json`, `tools/gen_villains.py`, whose slot names follow
  the original pack until Phase 2 regenerates them.

## Phase 2 — Independent pack, playable end to end

Decided 2026-10-22: maps are **generated, then curated**; music is **CC0**.
The pack is `packs/stolen-scepter/`, built by scripts from our own data, so
it can be regenerated and nothing is hand-copied from the original.

What a pack holds (measured from the original pack's structure, which the
engine expects): `game.json` (rules, 4 zones with their placements, 26 towns,
27 castles, 25 troops, 14 spells, 4 classes, 17 villains, 8 artifacts, tile
codes, ~280 strings), 4 maps, 311 images and a font, 2 music tracks and 4
effect tunes, and a 768-byte palette.

Tasks, in order:

1. **Map generator** (`tools/gen_maps.py`). Maps are 64x64 text grids; each
   character is a tile code the pack's own `tile_codes` table defines
   (terrain, art, passability), and terrain edges are separate tiles chosen
   from the neighbours (auto-tiling). Per continent: a terrain theme (home
   grassland, forest, islands, desert), coasts, then placements that match
   the terrain -- castles (3x2 on land, gate reachable), towns on the coast
   with a boat dock, signs, chests, wandering armies, spawn points, the magic
   alcove -- and a reachability check (every placement reachable on foot or
   by boat). Generate several seeds, keep and hand-fix the best, commit them.
2. **Names and text** (`packs/stolen-scepter/` source data): the King,
   continents, towns, castles, villains with bios, artifacts, and every
   string section, written new. The CI gate catches anything that slips
   through from the original.
3. **Pack builder** (`tools/build_pack.py`): assembles `game.json` from the
   rules (kept, lightly retuned), the generated maps and placements, and the
   text; writes the pack folder.
4. **Art**: the 100 generated troop frames, then placeholders for every other
   slot from the procedural generators (retargeted from the old `packs/open/`),
   and our own palette. Upgraded in Phase 5.
5. **Audio**: 2 tracks and 4 tunes picked from CC0 sources (OpenGameArt),
   each source recorded in the credits; downloads confirmed by hand.
6. **Font**: an OFL-licensed pixel font converted to the bitmap-font slot.
7. **Shell**: discovers `packs/` (and later the app bundle's own pack).
8. **Proof**: a test that loads `packs/stolen-scepter/` with no `reference/`
   or `game-assets/` present and plays a full headless game (combat, a
   contract, the win check); added to CI.

## Phase 3 — DMG build (GitHub Actions)

Adapted from `damaian-client/.github/workflows/macos-dmg.yml`, minus Tauri.

Kept: tag (`v*`) and manual triggers with preview/stable channels; a quality
gate (`cargo fmt --check`, `cargo test --workspace`) before any build; the
up-front check for Apple secrets (same names: `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_TEAM_ID`,
`APPLE_API_ISSUER`, `APPLE_API_KEY`, `APPLE_API_KEY_CONTENT`); the API key in
the runner temp dir; DMG notarize-and-staple with `notarytool log` on failure;
the `codesign` / `stapler` / `spctl` / `hdiutil verify` checks; the
`publish-release` job with notes from the commit log.

New, because Tauri did it there:

1. `cargo build --release -p openbounty-shell` (Apple Silicon; add an x86_64
   build and `lipo` later if wanted).
2. Assemble the `.app`: `Info.plist`, icon, binary, and the pack under
   `Contents/Resources/`.
3. Shell change: discover a pack inside its own bundle (`shell/src/pack.rs`).
4. Temporary keychain from `APPLE_CERTIFICATE`; `codesign --options runtime
   --timestamp` for the app.
5. Zip and notarize the `.app`, staple it.
6. `hdiutil create` the DMG, sign, notarize and staple it (the existing step).

Dropped for now: the Tauri updater, the changelog commit, the website dispatch.

## Phase 4 — Go public

Started 2026-10-22: the history was rewritten to one author address, the
public `stolen-scepter` repository was created, the CI triggers were restored,
and `LICENSE` (MIT, OpenBounty's notice kept) and the README were added. Still
to do: finish the README (screenshots, credits for every bundled part), then (what it is, how to play, credits to
OpenBounty, licences of every bundled part), flip it to public, tag `v0.1.0`.

## Phase 5 — Upgrade releases

Replace placeholders in order of screen time: map tiles, villains, UI, hero and
boat, combat tiles, classes; then better audio.

## Decisions needed

### Title -- decided 2026-10-22

**The Quest for a Stolen Scepter** (US spelling). In-game title, README and
release names use the full title; the macOS app and DMG are **Stolen Scepter**
(it fits under a Dock icon); the repository is
`github.com/damijanc/stolen-scepter` (public). Clash check: no game by that
name on Steam, itch.io or the web; nearest is "Myths of Minerva: Stolen
Sceptre", a card game with a series prefix. A stolen scepter is a common
fantasy plot, so the name is descriptive rather than unique. A direct
USPTO/EUIPO register search is still to do by hand.

### Maps

- **A. Generated, then curated (recommended).** A tool generates continents
  from a seed (terrain, coasts, placement rules from `game.json`), we keep the
  best seeds and hand-fix them, and the result is committed as fixed files.
  Fast, and the rules keep the game balanced.
- **B. Hand-drawn.** Full control, but four 64x64 maps is a lot of tile work.

### Music (6 short tunes)

- **A. CC0 tracks from OpenGameArt (recommended for v0.1).** No attribution
  required; record each source in the pack's credits anyway.
- **B. Composed in code.** Short synthesized tunes generated by a script:
  fully ours, but simple.
- **C. AI music tools.** Check each tool's licence for redistribution first.

## Order of work

1. Phase 1 cleanup items that are pure deletes and renames.
2. Title, maps and music decisions.
3. Phase 2 pack, ending with the no-original playthrough test.
4. Phase 3 workflow, first as a preview (unsigned) build.
5. Apple signing secrets in the repository (by you), then a stable build.
6. Phase 4.
