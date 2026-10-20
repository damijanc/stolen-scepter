# AI Troop Art — Findings (2026-10-20)

State of the troop generation at `tools/gen_ai.py` v1.22, after one full pass
over all 25 troops. Read with `specs/generation-mac-instructions.md` (how to
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

## Per-unit status after the full pass

Runs: `game-assets/ai-out/run-wave1` .. `run-wave4` (this Mac, not committed).

| Unit | Status | Open issue |
|---|---|---|
| archers | usable | -- |
| sprites | usable | -- |
| giants | usable | frame 2 carries the boulder rather than throwing it |
| barbarians | usable | -- |
| knights | usable | the flail is never drawn (sword/spear instead) |
| pikemen | usable | -- |
| peasants | usable | stray basket in frame 2 |
| dwarves | usable | weapon wavers between hammer and axe |
| wolves | usable with `--mirror wolves` | poses barely change (no skeleton) |
| archmages | usable | -- |
| cavalry | usable | poses barely change (no skeleton) |
| nomads | usable | poses barely change (no skeleton) |
| demons | usable | -- |
| ghosts | usable | -- |
| zombies | usable | -- |
| militia | usable | last checked in wave 1b |
| druids | fix | frame 3 white blob (magic effect) |
| gnomes | fix | frame 3 circular background disc |
| elves | fix | frame 3 circular background disc |
| vampires | fix | circular background shapes (moon) behind frames 0, 2, 3 |
| skeletons | fix | frame 1 holds two spears |
| ogres | fix | club drawn like a wooden spoon |
| orcs | rerun | frame 2 has two orcs touching (check sees one blob) |
| trolls | rerun | frame 0 seen from behind / facing left |
| dragons | rerun | IP-Adapter step timed out: mixed facing, drifting look |

## Next fixes, in order

1. Background shapes: add "moon, halo, circle, disc, sun, magic circle" to
   `NEG`, and reword the caster frames that ask for glowing rings or bursts.
2. Two touching figures: a width check (subject much wider than the layout's
   `frac` for that unit) to catch what the blob count misses.
3. Rerun dragons (with `--mirror dragons` if frame 0 faces left), orcs,
   trolls; then the "fix" rows.
4. The units without a skeleton (wolves, dragons, cavalry, nomads) need their
   own pose control to move like the originals; OpenPose has no animals.
5. After that: villains, hero, boat, classes, combat tiles, map objects.
