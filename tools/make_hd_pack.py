"""Assemble a launchable HD pack from the reference pack plus generated art.

Copies `reference/assets/kings-bounty` to a scratch directory, overlays the
upscaled `game-assets/generated` tree on top of it, and patches the render
block with the art pixel multiple so the shell presents at that scale:

    python3 tools/make_hd_pack.py --scale 4
    # -> game-assets/hd-pack/game.json

Then run the shell against it:

    cargo run -p openbounty-shell -- game-assets/hd-pack/game.json

The output lives under the gitignored `game-assets/`, so an HD pack is a local
build product, never committed.
"""
import argparse
import json
import os
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACK = os.path.join(ROOT, "reference", "assets", "kings-bounty")
GENERATED = os.path.join(ROOT, "game-assets", "generated")
DEFAULT_OUT = os.path.join(ROOT, "game-assets", "hd-pack")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--scale", type=int, default=4)
    ap.add_argument("--out", default=DEFAULT_OUT)
    ap.add_argument("--keep", action="store_true", help="do not delete an existing output tree")
    args = ap.parse_args()

    if not os.path.isdir(PACK):
        ap.error(f"reference pack not found at {PACK}")
    if not os.path.isdir(GENERATED):
        ap.error(f"no generated art at {GENERATED}; run tools/upscale.py first")

    if os.path.isdir(args.out) and not args.keep:
        shutil.rmtree(args.out)
    shutil.copytree(PACK, args.out, dirs_exist_ok=True)

    copied = 0
    for root, _, files in os.walk(GENERATED):
        for name in files:
            src = os.path.join(root, name)
            rel = os.path.relpath(src, GENERATED)
            dst = os.path.join(args.out, rel)
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copy2(src, dst)
            copied += 1

    game_json = os.path.join(args.out, "game.json")
    with open(game_json) as fh:
        data = json.load(fh)
    render = data.setdefault("render", {})
    render.setdefault("mode", "legacy")
    render["scale"] = args.scale
    with open(game_json, "w") as fh:
        json.dump(data, fh, indent=1)

    print(f"HD pack -> {os.path.relpath(args.out, ROOT)} "
          f"(scale {args.scale}, {copied} art files overlaid)")
    print(f"run: cargo run -p openbounty-shell -- {os.path.relpath(game_json, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
