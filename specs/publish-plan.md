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
- **Original names in tests.** "Riverton" and "Sword of Prowess" appear as
  fixtures in `engine/src/game.rs`, `engine/src/resources.rs`,
  `shell/src/interact.rs` and `shell/src/views.rs`. Rename to neutral test
  names; one line each in two specs as well.
- **Tools aimed at the original's files.** Remove before publishing:
  - `tools/manual_word.py` answers the original's manual-lookup copy
    protection. Do not publish it.
  - `tools/cc_extract.py`, `cc_output_map.py`, `upscale.py`,
    `make_hd_pack.py`, `hd_manifest.py`, `modern_proof.py`: readers and
    upscalers of the original archives, not needed once the pack is ours.
    `gen_ai.py` imports `read_png` / `write_png` from `upscale.py`; move those
    into a small `pngio.py` first.
  - `tools/hd_assets.json` lists the original archive entries; keep the slot
    list `gen_ai.py` needs and drop the archive names.
  - `tools/asset_layout.py` reads the originals; its OUTPUT
    (`asset_layout.json`, numbers only) stays.
- **`specs/manual-reference-notes.md`** describes the local copies of the
  original game and manual. Remove, or cut to the policy section.
- **Old `packs/open/` attempt.** Delete; the new pack takes its place (Phase 2).
- **Commit author emails.** The history has two author addresses, one a work
  address. Either accept them as public, or rewrite authorship with
  `git filter-repo --mailmap` before the first public push (this rewrites
  every commit hash; do it once, before going public, never after).
- **Name.** "King's Bounty" appears in 7 files (11 mentions) and in the repo
  and folder name. Keep factual credit ("a port of OpenBounty, a
  reimplementation of King's Bounty"), remove it as our product's name, and
  rename the GitHub repository to the new title.
- **Final gate.** A script (`tools/check_independent.sh`) that fails if any
  tracked file matches the original's names, archive entries or file hashes;
  run in CI from then on.

## Phase 2 — Independent pack, playable end to end

A committed pack under `packs/<title>/` that the shell loads with nothing from
the original on disk.

1. **Title and text.** New title, villain names and bios, artifact, town and
   castle names, every UI string, contract, intro, ending, credits.
2. **Maps.** Four continents in the engine's 64x64 tile format, with castles,
   towns, dwellings and spawn points placed to satisfy `game.json`.
3. **Art.** The generated troops, plus placeholders for every other slot so
   the pack is complete; upgrade in Phase 5.
4. **Font.** An OFL-licensed pixel font converted to the bitmap-font slot.
5. **Audio.** Six short tunes (see options below).
6. **Proof.** A test that starts a game from the new pack and runs a full
   headless playthrough (combat, a contract, the win check), on a machine with
   no `reference/` or `game-assets/`.

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

Rename the repository, write the README (what it is, how to play, credits to
OpenBounty, licences of every bundled part), flip it to public, tag `v0.1.0`.

## Phase 5 — Upgrade releases

Replace placeholders in order of screen time: map tiles, villains, UI, hero and
boat, combat tiles, classes; then better audio.

## Decisions needed

### Title

Avoid "King's Bounty" and close variants ("Queen's Bounty"). The game is about
hunting a realm's villains under royal contract to recover a stolen sceptre.
Candidates, unchecked for existing games or trademarks:

- **Sceptre of the Realm**
- **Writ of the Crown**
- **The Lost Sceptre**
- **Crown Contracts**
- **Warlord's Writ**

Before settling: search Steam, itch.io and the USPTO/EUIPO databases for the
chosen name.

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
