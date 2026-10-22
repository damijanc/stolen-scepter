"""Copy the kept AI troop frames into the pack at its art scale.

`gen_ai.py` renders each troop frame at 6x (288x204) into
`game-assets/ai-out/<run>/art/troops/`. The run kept for each unit is the one
`specs/ai-troop-findings.md` records; this script area-averages those frames
down to the pack's art scale (`render.scale`, 4x: 192x136), weighting colour
by alpha so the matte edge does not darken, and writes them to
`packs/stolen-scepter/art/troops/`.

`game-assets/` is local and ignored; the PNGs this writes are what the repo
keeps.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from packart import PACK, ROOT, out_dir, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

AI_OUT = os.path.join(ROOT, "game-assets", "ai-out")
CELL_W, CELL_H = 48, 34
FRAMES = 4
AUTHOR = "AI-generated (tools/gen_ai.py: SDXL DreamShaper XL, ControlNet OpenPose)"

# The run each unit's kept frames come from (specs/ai-troop-findings.md).
KEPT_RUN = {
    "archers": "run-wave1",
    "sprites": "run-wave1",
    "militia": "run-wave6",
    "barbarians": "run-wave2",
    "knights": "run-wave2",
    "pikemen": "run-wave2",
    "peasants": "run-wave2",
    "giants": "run-wave3",
    "dwarves": "run-wave3",
    "wolves": "run-wave3",
    "archmages": "run-wave4",
    "cavalry": "run-wave4",
    "nomads": "run-wave4",
    "demons": "run-wave4",
    "ghosts": "run-wave4",
    "zombies": "run-wave4",
    "druids": "run-wave5",
    "gnomes": "run-wave5",
    "elves": "run-wave5",
    "vampires": "run-wave5",
    "trolls": "run-wave5",
    "orcs": "run-wave6",
    "ogres": "run-wave6",
    "skeletons": "run-wave6",
    "dragons": "run-wave6",
}


def upsample(w, h, px, k):
    """Nearest-neighbour `px` by an integer `k`, so a fractional area average
    becomes a whole-block one."""
    out = bytearray(w * k * h * k * 4)
    for y in range(h * k):
        for x in range(w * k):
            o = ((y // k) * w + x // k) * 4
            d = (y * w * k + x) * 4
            out[d:d + 4] = px[o:o + 4]
    return w * k, h * k, bytes(out)


def resample(w, h, px, ow, oh):
    """Area-average RGBA `px` (w x h) to ow x oh: upsample until the ratio is
    whole, then box-filter. 288 -> 192 is x2 then /3."""
    k = 1
    while (w * k) % ow or (h * k) % oh:
        k += 1
    if k > 1:
        w, h, px = upsample(w, h, px, k)
    return downscale(w, h, px, ow, oh)


def downscale(w, h, px, ow, oh):
    """Box-filter RGBA `px` (w x h) to ow x oh, weighting colour by alpha."""
    fx, fy = w // ow, h // oh
    assert fx * ow == w and fy * oh == h, (w, h, ow, oh)
    out = bytearray(ow * oh * 4)
    n = fx * fy
    for oy in range(oh):
        for ox in range(ow):
            r = g = b = a = 0
            for y in range(oy * fy, oy * fy + fy):
                o = (y * w + ox * fx) * 4
                for _ in range(fx):
                    pa = px[o + 3]
                    r += px[o] * pa
                    g += px[o + 1] * pa
                    b += px[o + 2] * pa
                    a += pa
                    o += 4
            d = (oy * ow + ox) * 4
            if a:
                out[d] = round(r / a)
                out[d + 1] = round(g / a)
                out[d + 2] = round(b / a)
            out[d + 3] = round(a / n)
    return bytes(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--only", nargs="*", help="units to import (default: all)")
    args = ap.parse_args()

    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    ow, oh = CELL_W * scale, CELL_H * scale
    out = out_dir("troops")
    written = []
    for unit in sorted(args.only or KEPT_RUN):
        run = KEPT_RUN[unit]
        for frame in range(FRAMES):
            name = f"{unit}_{frame:02d}.png"
            src = os.path.join(AI_OUT, run, "art", "troops", name)
            w, h, px = read_png(src)
            write_png(os.path.join(out, name), ow, oh, resample(w, h, px, ow, oh))
            written.append("art/troops/" + name)
    register(written, AUTHOR, method="ai-generated")
    print(f"imported {len(written)} troop frames at {ow}x{oh} to {os.path.relpath(out, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
