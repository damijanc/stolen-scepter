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
- `gen_terrain.py`, `gen_objects.py`, `gen_sprites.py`, `gen_troops.py`,
  `gen_villains.py`, `gen_combat.py`, with `artlib.py` and `modern_proof.py`:
  procedural placeholder art. They still write to the removed `packs/open/`
  and are retargeted when the new pack exists (`specs/publish-plan.md`,
  Phase 2).

## Development

- `engine_sweep.sh`: lists every function the OpenBounty C headers declare as
  HIT or MISS against `engine/src` and `shell/src`. Needs the local
  `reference/` clone.
