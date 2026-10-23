"""Render a patch of a continent from the pack's own tiles, to judge terrain
art (seams, edges, repetition) without starting the game.

Usage:
    python3 tools/preview_map.py OUT.png [--zone highmoor] [--x 10 --y 10]
                                [--w 10 --h 8] [--half]

Draws terrain only, from the map and `tile_codes`; placed objects (castles,
towns, chests) are stamped by the engine at runtime and are not shown.
`--half` halves the output size.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_ai import box_downscale  # noqa: E402
from packart import PACK  # noqa: E402
from pngio import read_png, write_png  # noqa: E402


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("out")
    ap.add_argument("--zone", default="")
    ap.add_argument("--x", type=int, default=-1)
    ap.add_argument("--y", type=int, default=-1)
    ap.add_argument("--w", type=int, default=10)
    ap.add_argument("--h", type=int, default=8)
    ap.add_argument("--half", action="store_true")
    a = ap.parse_args()

    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    zone = next(z for z in pack["zones"] if not a.zone or z["id"] == a.zone)
    with open(os.path.join(PACK, zone["map"])) as fh:
        rows = [line.rstrip("\n") for line in fh if not line.startswith("#")]
    codes = pack["tile_codes"]

    # Default: the patch around the hero's spawn, which has coast and land.
    x0 = a.x if a.x >= 0 else max(0, zone["hero_spawn"]["x"] - a.w // 2)
    y0 = a.y if a.y >= 0 else max(0, zone["hero_spawn"]["y"] - a.h // 2)

    cache = {}
    tw = th = None
    for ty in range(a.h):
        for tx in range(a.w):
            art = codes[rows[y0 + ty][x0 + tx]]["art"]
            if art not in cache:
                cache[art] = read_png(os.path.join(PACK, "art", "tiles", art + ".png"))
            tw, th = cache[art][0], cache[art][1]
    W, H = tw * a.w, th * a.h
    out = bytearray(W * H * 4)
    for ty in range(a.h):
        for tx in range(a.w):
            w, h, px = cache[codes[rows[y0 + ty][x0 + tx]]["art"]]
            for y in range(h):
                d = ((ty * th + y) * W + tx * tw) * 4
                out[d:d + w * 4] = px[y * w * 4:(y + 1) * w * 4]
    if a.half:
        W, H, out = box_downscale(W, H, bytes(out), W // 2, H // 2)
    write_png(a.out, W, H, bytes(out))
    print(f"{a.out}: {zone['id']} tiles ({x0},{y0}) {a.w}x{a.h}, {W}x{H}px")
    return 0


if __name__ == "__main__":
    sys.exit(main())
