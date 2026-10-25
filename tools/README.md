# Tools

Python scripts for building the game's own art. Stdlib only unless noted;
PNG I/O is `pngio.py`.

## Art generation

- `gen_ai.py`: drives a ComfyUI server (SDXL + ControlNet OpenPose +
  IP-Adapter) to generate pack art slot by slot, mattes it with `rembg`,
  checks each frame, and places it in its cell. Run instructions:
  `specs/generation-mac-instructions.md`; results and status:
  `specs/ai-troop-findings.md`.
- `pose_rig.py`: the authored three-quarter-view pose skeletons `gen_ai.py --controlnet`
  draws each troop frame to.
- `hd_assets.json`: every art slot a pack must provide (path, native size,
  alpha convention, whether the engine draws it).
- `asset_layout.json`: per slot, where the subject sits in its cell (box,
  ground line, foot centre) as numbers. `asset_layout.py` measured it once
  from a locally owned copy of the original art; the generators read only the
  JSON.
- `gen_startup.py` / `compose_startup.py`: the startup screens. The first
  renders the raw AI pieces on ComfyUI (a full-body figure and a portrait per
  class, an empty hall; several seeds each, with a contact sheet); the second
  places the picked seeds into the class picker, the class portraits and the
  credits image. The picker's four columns are `CLASS_FIGURES` in
  `shell/src/startup.rs`; change both together.
- `gen_terrain_ai.py`: the map terrain. `render` draws each base texture
  (grass, water, desert, forest, mountain) on ComfyUI, several seeds each;
  `compose` makes the picked seeds tileable, then builds every base tile, the
  twelve edge tiles per terrain, and the grass under the combat field and the
  end screen.
- `gen_objects_ai.py`: the map objects (castle, town, chests, dwellings,
  sign, camp, bridges). `render` draws each on ComfyUI, matted; `compose`
  sets the picked seeds on the pack's grass (bridges on water), the castle
  as one picture sliced into its six pieces.
- `gen_hero_ai.py`: the hero on the map, a mounted knight and a boat, both
  facing east (the shell mirrors them going west). One picture each,
  animated with a small bob over the four frames.
- `gen_villains_ai.py`: the seventeen villain portraits, one prompt each from
  the villain's description in the pack text, cropped to the 48x34 face slot
  and animated with a slow breathing zoom over the four frames.
- `gen_icons_ai.py`: the eight artifact icons and the four continent maps,
  matted and centred in a transparent 48x34 cell.
- `gen_combat_ai.py`: the combat field pieces. `render` draws the three field
  obstacles and the hit burst on ComfyUI (matted) plus one seamless castle-stone
  wall texture; `compose` mattes the obstacles and burst into the transparent
  cell, then draws the six castle wall pieces from the one texture so they tile
  into a continuous wall, and draws the four-frame target ring in code.
- `gen_hud_ai.py`: the sidebar HUD art. `render` draws the hooded contract
  figure, the wooden catapult, the arcane star and the coin purse on ComfyUI
  (matted) plus aged-parchment and dark-wood textures; `compose` places them
  into the five 48x34 panels -- the siege and magic four-frame loops and their
  matching grey silhouettes, the wanted-poster contract silhouette, the gold
  purse, and the parchment puzzle grid with its wood cover tile.
- `gen_chrome_ai.py`: the last four procedural pieces. `render` draws three
  seamless textures (castle stone, red velvet, red carpet) and a matted
  back-view knight on ComfyUI; `compose` turns them into the 4x overworld
  frame (its new alpha is asserted to equal the 4x-upscaled old alpha, only
  the colours change), the deep-red bar strip with its gold bead row, the
  end-cartoon carpet runner, and the hero walking up it.
- `measure_maps.py`: continent maps as numbers (water, land mix, path
  width, wall size). `--reference --write` measures the original maps in the
  local `reference/` clone into `map_targets.json`, numbers only, which
  `gen_maps.py` is tuned toward; without flags it measures ours.
- `gen_worldmap.py`: bakes each continent's world map picture from the pack's
  own terrain tiles, edges rebuilt at the small size and bases graded apart.
  The M view draws it under fog and markers, falling back to colour blocks.
- `preview_map.py`: renders a patch of a continent from the pack's own tiles,
  to judge seams, edges and repetition without starting the game.
- `import_troops.py`: box-filters the kept AI troop frames (6x, local
  `game-assets/ai-out/`) down to the pack's art scale (4x: 192x136).
- `gen_terrain.py`, `gen_objects.py`, `gen_combat.py`, `gen_sprites.py`,
  `gen_villains.py`, `gen_ui.py`: procedural placeholder art for every other
  slot, plus the palette. `gen_font.py` packs `glyphs.py`, the project's own
  8x8 glyphs, into the font strip. All write into `packs/stolen-scepter/` via
  `packart.py`, which also records each file in the pack's `provenance.json`.
  `gen_tunes.py` synthesises the four effect tunes. `artlib.py` and
  `modern_proof.py` are the earlier HD experiments.

## The pack

`packs/stolen-scepter/` is built entirely by these scripts, in this order:

    python3 tools/gen_maps.py
    python3 tools/build_pack.py
    python3 tools/gen_terrain.py && python3 tools/gen_objects.py
    python3 tools/gen_worldmap.py
    python3 tools/gen_combat.py && python3 tools/gen_sprites.py
    python3 tools/gen_villains.py && python3 tools/gen_ui.py
    python3 tools/gen_font.py && python3 tools/gen_tunes.py
    python3 tools/import_troops.py   # needs the local AI runs
    python3 tools/build_pack.py --check

`build_pack.py` turns `data/rules.json` (numbers), `data/text.json` (names
and strings) and the generated maps into `game.json`, and `--check` validates
it against what the shell reads.

## Release

- `make_dmg.sh [VERSION] [CHANNEL] [MODE]`: builds
  `dist/Stolen Scepter.app` and its DMG. `CHANNEL` is `preview` (default) or
  `stable`; `MODE` is `--all` (default), `--app-only` (build, assemble and
  sign the app, then stop) or `--dmg-only` (pack an existing signed app,
  without rebuilding or re-signing). A preview is ad-hoc signed; setting
  `CODESIGN_IDENTITY` signs the inner binary and then the bundle with that
  Developer ID, the hardened runtime and a timestamp. The stable DMG drops
  the channel from its name: `Stolen-Scepter-<VERSION>-macos-arm64.dmg`. Run
  outside a sandbox: `iconutil` and `hdiutil` need the system temp directory.
  `.github/workflows/macos-dmg.yml` is the only caller of the stable path,
  and it runs only on a pushed `v*` tag, stamping that version into
  `shell/Cargo.toml` before it builds.
- `gen_icon.py`: the app icon, written as a macOS iconset.

## Development

- `engine_sweep.sh`: lists every function the OpenBounty C headers declare as
  HIT or MISS against `engine/src` and `shell/src`. Needs the local
  `reference/` clone.
