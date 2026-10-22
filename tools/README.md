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
- `import_troops.py`: box-filters the kept AI troop frames (6x, local
  `game-assets/ai-out/`) down to 48x34 into the pack.
- `gen_terrain.py`, `gen_objects.py`, `gen_combat.py`, `gen_sprites.py`,
  `gen_villains.py`, `gen_ui.py`: procedural placeholder art for every other
  slot, plus the palette. `gen_font.py` packs `glyphs.py`, the project's own
  8x8 glyphs, into the font strip. All write into `packs/stolen-scepter/` via
  `packart.py`, which also records each file in the pack's `provenance.json`.
  `artlib.py` and `modern_proof.py` are the earlier HD experiments.

## The pack

`packs/stolen-scepter/` is built entirely by these scripts, in this order:

    python3 tools/gen_maps.py
    python3 tools/build_pack.py
    python3 tools/gen_terrain.py && python3 tools/gen_objects.py
    python3 tools/gen_combat.py && python3 tools/gen_sprites.py
    python3 tools/gen_villains.py && python3 tools/gen_ui.py
    python3 tools/gen_font.py
    python3 tools/import_troops.py   # needs the local AI runs
    python3 tools/build_pack.py --check

`build_pack.py` turns `data/rules.json` (numbers), `data/text.json` (names
and strings) and the generated maps into `game.json`, and `--check` validates
it against what the shell reads.

## Development

- `engine_sweep.sh`: lists every function the OpenBounty C headers declare as
  HIT or MISS against `engine/src` and `shell/src`. Needs the local
  `reference/` clone.
