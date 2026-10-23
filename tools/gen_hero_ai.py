"""AI art for the hero on the map: the mounted knight and the boat.

    python3 tools/gen_hero_ai.py render --run hero1 [--seeds 4] [--only knight,boat]
    python3 tools/gen_hero_ai.py compose

`render` draws each on ComfyUI (through the tunnel), matted with rembg, into
local, ignored `game-assets/ai-out/<run>/hero/`, with a contact sheet.

`compose` needs no tunnel. The pack's hero animations are single strips that
the shell mirrors going west (`AnimPaths::frame`), so each picture must face
east; a pick that faces west is mirrored here. Four AI renders of one horse
mid-stride would not match, so the four frames are one picture with a small
bob -- a trot for the knight, a swell for the boat -- on a transparent cell.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from gen_objects_ai import subject  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "hero1"
# name -> (seed, mirror): mirror a pick that faces west.
PICKS = {"knight": (101, False), "boat": (101, True)}
AUTHOR = "AI-generated (tools/gen_hero_ai.py: SDXL DreamShaper XL), animated by " \
         "tools/gen_hero_ai.py"

STYLE = ("painterly classic high fantasy strategy game art, rich saturated colours, crisp "
         "detail, single subject, plain flat light grey background")
NEG = ("text, letters, watermark, signature, border, frame, landscape, scenery, ground, "
       "water surface, sky, multiple subjects, cropped, cut off, blurry, low quality, deformed")

# name -> (prompt, box: fraction of the cell the subject may fill, foot line,
# bob in logical pixels per frame).
SUBJECTS = {
    "knight": ("a knight in polished steel plate armour with a red plume and a red "
               "surcoat riding a white warhorse, side view, horse walking to the right, "
               "whole horse from hooves to ears and the whole rider in frame",
               (0.92, 0.94), 0.97, [0, -1, 0, 1]),
    "boat": ("a small wooden sailing ship with one white sail and a red pennant, side view, "
             "sailing to the right, the whole hull and mast in frame",
             (0.92, 0.94), 0.95, [0, -1, -1, 0]),
}
SLOTS = {"knight": "walk", "boat": "boat"}


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "hero")


def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, gen_ai.resolve_rembg("rembg"))
    if args.rembg is None:
        sys.exit("rembg not found; see gen_ai.py --rembg")
    gen_ai.clear_queue(args.host)
    only = set(filter(None, cli.only.split(",")))
    for name, (prompt, *_rest) in SUBJECTS.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
            render(args, f"{prompt}, {STYLE}", NEG, (1216, 832), seed, True,
                   os.path.join(out, f"{name}_{seed}.png"))
    contact_sheet(out)
    return 0


def mirrored(w, h, px):
    out = bytearray(len(px))
    for y in range(h):
        for x in range(w):
            s = (y * w + x) * 4
            d = (y * w + (w - 1 - x)) * 4
            out[d:d + 4] = px[s:s + 4]
    return bytes(out)


def cmd_compose(cli):
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    scale = pack["render"].get("scale", 1)
    tw, th = 48 * scale, 34 * scale
    written = []
    for name, (_, box, foot, bob) in SUBJECTS.items():
        seed, mirror = PICKS[name]
        w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{seed}.png"))
        sw, sh, spx = subject(w, h, px)
        if mirror:
            spx = mirrored(sw, sh, spx)
        k = min(tw * box[0] / sw, th * box[1] / sh)
        nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
        _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
        ox = (tw - nw) // 2
        for frame, rel in enumerate(pack["sprites"]["hero"][SLOTS[name]]):
            canvas = bytearray(tw * th * 4)
            oy = int(th * foot) - nh + bob[frame % len(bob)] * scale
            over_transparent(tw, canvas, nw, nh, scaled, ox, oy)
            write_png(os.path.join(PACK, rel), tw, th, bytes(canvas))
            written.append(rel)
    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} hero frames at {tw}x{th}")
    return 0


def over_transparent(dst_w, dst, w, h, px, ox, oy):
    """Copy RGBA `px` onto a transparent canvas, clipped to it."""
    dst_h = len(dst) // (dst_w * 4)
    for y in range(h):
        dy = oy + y
        if not 0 <= dy < dst_h:
            continue
        for x in range(w):
            dx = ox + x
            if 0 <= dx < dst_w:
                s = (y * w + x) * 4
                d = (dy * dst_w + dx) * 4
                dst[d:d + 4] = px[s:s + 4]


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=4)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="", help="comma list: knight, boat")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
