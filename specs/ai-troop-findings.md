# AI Troop Art — Findings (2026-10-20, updated 2026-10-22)

State of the troop generation at `tools/gen_ai.py` v1.24. All 25 troops have
a usable set of four frames. Read with `specs/generation-mac-instructions.md` (how to
run) and `specs/ai-art-handoff.md` (goals and rules).

## Setup that works

- ComfyUI runs on the generation Mac; this Mac drives it through an SSH tunnel
  (`ssh -N -L 8188:127.0.0.1:8188 <user>@<host>.local`), so `gen_ai.py` needs no
  `--host`. Matting (rembg, BRIA RMBG 2.0) runs on this Mac and every frame
  lands here, so frames are reviewed directly.
- Command used for every unit:
  `gen_ai.py --out game-assets/ai-out/<run> --controlnet --ipadapter batch --category troops --only <unit>`
  (default checkpoint DreamShaper XL; `--mirror <unit>` for a unit whose frame 0
  faces left).
- In this session's Claude Code sandbox, localhost is blocked: every tunnel
  call needs the sandbox bypass (or allow local network in `/sandbox`).

## What holds

- **Facing and stance** from the authored OpenPose skeletons
  (`tools/pose_rig.py`, three-quarter view): every posed unit faces right.
- **Identity across frames** from IP-Adapter (frame 0 as reference,
  `style transfer`) plus a pinned outfit in `TROOP`.
- **Size and placement** from `tools/asset_layout.json` (registration `spec`):
  each unit at the original's height, feet on the original's ground line.
- **Frame checks** (`frame_issues`): a separate second figure or baked-in
  ground triggers a reseeded retry; tuned on real frames (ground patches fill
  0.88-0.96 of the bottom band, clean figures stay under 0.66).
- **Facing consistency** (`orient_to_first`) for the units without a skeleton.
- **Unit-level fill check** (`group_checked`): a frame whose shape is 1.9x
  fuller than the median of its unit's other three is regenerated. Catches
  background discs, white effect smears and a second figure JOINED to the
  first, which the per-frame blob count cannot see. Tuned on 140 real frames:
  defects measured 2.0-2.5x, normal poses at most 1.78x.
- **Rerolling a unit**: `--variant N` redraws it from a fresh set of seeds.

## What did not work, and why

- **Props the arms cannot show.** No checkpoint drew "bow held, not drawn"
  or "arrow gone": an archer with a bow is drawn shooting. Fix used: give the
  beat a pose the arms CAN show (archer frame 3 reaches to the quiver).
- **Effect words** ("sparks", "glint", "crackling lightning") render as white
  blobs the matte keeps. All removed from `TROOP_POSES`; one remains as a
  white blob in druid frame 3.
- **"Strategy game unit"** in the prompt produced tabletop miniatures on
  stone bases; now "fantasy character illustration".
- **Generation is not repeatable** on this setup: the same seed and prompt
  can change facing between runs, so a fixed mirror list is unreliable.
- **Base SDXL vs DreamShaper XL**: same poses; DreamShaper has cleaner
  silhouettes. It drifted to a 3D look until "flat 2d hand-painted
  illustration" was added.

## Per-unit status

All 25 usable. The kept frames are in `game-assets/ai-out/<run>/art/troops/`
on this Mac (not committed); the run that holds each unit is listed so a
later pack build can collect them.

| Unit | Run | Flags used | Known flaw (minor) |
|---|---|---|---|
| archers | run-wave1 | -- | -- |
| sprites | run-wave1 | -- | -- |
| militia | run-wave6 | `--variant 1` | a second shield slung on the back in frames 0 and 2 |
| barbarians | run-wave2 | -- | frame 1 holds an axe in each hand |
| knights | run-wave2 | -- | flail never drawn (sword/spear instead) |
| pikemen | run-wave2 | -- | -- |
| peasants | run-wave2 | -- | stray basket in frame 2 |
| giants | run-wave3 | -- | frame 2 carries the boulder rather than throwing it |
| dwarves | run-wave3 | -- | weapon wavers between hammer and axe |
| wolves | run-wave3 | `--mirror wolves` | poses barely change (no skeleton) |
| archmages | run-wave4 | -- | -- |
| cavalry | run-wave4 | -- | poses barely change (no skeleton) |
| nomads | run-wave4 | -- | poses barely change (no skeleton) |
| demons | run-wave4 | -- | -- |
| ghosts | run-wave4 | -- | -- |
| zombies | run-wave4 | -- | -- |
| druids | run-wave5 | -- | -- |
| gnomes | run-wave5 | -- | -- |
| elves | run-wave5 | -- | -- |
| vampires | run-wave5 | -- | -- |
| trolls | run-wave5 | -- | -- |
| orcs | run-wave6 | `--variant 1` | shield appears in frame 3 |
| ogres | run-wave6 | `--variant 1` | club shape varies between frames |
| skeletons | run-wave6 | `--variant 1` | frame 0 also carries a short sword |
| dragons | run-wave6 | `--mirror dragons` | -- |

Facing changes between runs on this setup, so a `--mirror` that was right for
one run can be wrong for the next: check facing after every rerun of the
units without a skeleton.

## Next steps, in order

1. Collect the kept frames into one folder and review all 25 units together
   at game size, in motion (a looping preview), before promoting any of them.
2. The units without a skeleton (wolves, dragons, cavalry, nomads) need
   their own pose control to move like the originals; OpenPose has no
   animals.
3. Double figures that overlap the first (ogres frame 3 in run-wave5) are
   not caught by any check; review catches them, `--variant` fixes them.
4. Then: villains, hero, boat, classes, combat tiles, map objects.
