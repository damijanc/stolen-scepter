"""AI terrain for the map: base textures from ComfyUI, edges composed here.

Two steps:

    python3 tools/gen_terrain_ai.py render --run terrain1 [--seeds 3]
    python3 tools/gen_terrain_ai.py compose

`render` asks ComfyUI (through the tunnel, as `gen_ai.py` does) for a
top-down texture per base terrain, several seeds each, into local, ignored
`game-assets/ai-out/<run>/terrain/`, with a contact sheet.

`compose` needs no tunnel. For each base it takes the picked seed, crops it to
the tile's aspect, scales it to the pack's tile (48x34 times `render.scale`)
and makes it tile seamlessly by blending in a half-offset copy of itself. It
then builds the 48 edge tiles by blending each terrain into grass along a wavy
shoreline. The wave repeats exactly over the tile's width and height, so side
tiles meet each other, and the outer corners meet the sides, without a step.
The combat field and the ending's grass tile get the same grass.

Edge numbering is the pack's (`tools/gen_terrain.py`): land 01-12 and water
00-11, outer corners, inner corners, then sides.
"""
import argparse
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from gen_startup import Args, contact_sheet  # noqa: E402
from gen_terrain import SIDES, WATER_SIDES  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "terrain1"
# The picked seed per base. A pick may instead be (base, seed, (fx, fy)): the
# grass variant comes from another region of the same grass render, so it
# matches the grass's look and only breaks up its repetition.
PICKS = {"grass": 102, "grass_variant": ("grass", 102, (0.1, 0.1)), "water": 103,
         "desert": 202, "forest": 101, "mountain": 203}
AUTHOR = "AI-generated (tools/gen_terrain_ai.py: SDXL DreamShaper XL), made tileable " \
         "and edged by tools/gen_terrain_ai.py"

STYLE = ("seamless tileable texture, flat orthographic top-down view from directly above, "
         "even soft lighting, no shadows from a single direction, painterly fantasy "
         "strategy game map art, high detail")
BASES = {
    "grass": "a lush green grass meadow",
    "grass_variant": "a green grass meadow with small tufts and a few tiny wildflowers",
    "water": "deep blue sea water with gentle ripples and small highlights",
    "desert": "flat smooth golden desert sand with faint small ripples running in "
              "every direction and a few scattered pebbles",
    "forest": "a dense forest canopy of round green tree tops seen from above",
    "mountain": "a dense range of many small grey rocky mountain peaks seen from "
                "directly above, no valleys, no rivers",
}
NEG = ("text, watermark, signature, border, frame, horizon, sky, perspective, "
       "people, animals, buildings, blurry, low quality")
GEN_SIZE = (1024, 1024)

# How much of the generated texture one tile shows: the fraction of the
# render's width. Smaller means larger features on the map.
CROP = {"grass": 0.6, "grass_variant": 0.6, "water": 0.7, "desert": 0.95,
        "forest": 0.9, "mountain": 0.6}

# How much of each texture's large-scale brightness variation to remove.
FLATTEN = {"grass": 0.9, "grass_variant": 0.9, "water": 0.8, "desert": 0.7,
           "forest": 0.6, "mountain": 0.4}

# Contrast kept in each texture's fine detail (1 = as rendered). The desert's
# ripples line up from tile to tile into stripes at full strength.
DETAIL = {"grass": 1.0, "grass_variant": 1.0, "water": 1.0, "desert": 0.45,
          "forest": 1.0, "mountain": 1.0}

BAND = 0.31       # the shoreline's mean distance from the tile edge, as a fraction
WAVE = 0.08       # its wave amplitude, as a fraction of the tile height
CORNER = 0.34     # the inner corner's diagonal, as a fraction of the tile width


def tile_size():
    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    return 48 * scale, 34 * scale, scale


# ---- render -------------------------------------------------------------

def cmd_render(cli):
    out = os.path.join(ROOT, "game-assets", "ai-out", cli.run, "terrain")
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, None)
    args.matte = "none"
    gen_ai.clear_queue(args.host)
    for name in cli.only.split(",") if cli.only else BASES:
        print(name, flush=True)
        for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
            w, h = GEN_SIZE
            graph = gen_ai.build_graph(args.ckpt, f"{BASES[name]}, {STYLE}", NEG,
                                       seed, w, h, None)
            rgba = gen_ai.run_graph_matted(args, graph, False)
            write_png(os.path.join(out, f"{name}_{seed}.png"), w, h, rgba)
    contact_sheet(out)
    return 0


# ---- compose ------------------------------------------------------------

def crop(w, h, px, x0, y0, cw, ch):
    out = bytearray(cw * ch * 4)
    for y in range(ch):
        s = ((y0 + y) * w + x0) * 4
        out[y * cw * 4:(y + 1) * cw * 4] = px[s:s + cw * 4]
    return bytes(out)


def box_blur_wrap(w, h, vals, r):
    """Box blur of one channel with radius `r`, wrapping at the edges."""
    tmp = [0.0] * (w * h)
    n = 2 * r + 1
    for y in range(h):
        row = vals[y * w:(y + 1) * w]
        acc = sum(row[(x) % w] for x in range(-r, r + 1))
        for x in range(w):
            tmp[y * w + x] = acc / n
            acc += row[(x + r + 1) % w] - row[(x - r) % w]
    out = [0.0] * (w * h)
    for x in range(w):
        col = [tmp[y * w + x] for y in range(h)]
        acc = sum(col[y % h] for y in range(-r, r + 1))
        for y in range(h):
            out[y * w + x] = acc / n
            acc += col[(y + r + 1) % h] - col[(y - r) % h]
    return out


def soften(w, h, px, detail):
    """Scale every pixel's distance from the mean colour by `detail`."""
    if detail >= 1.0:
        return px
    out = bytearray(px)
    n = w * h
    for c in range(3):
        mean = sum(px[i * 4 + c] for i in range(n)) / n
        for i in range(n):
            out[i * 4 + c] = round(mean + (px[i * 4 + c] - mean) * detail)
    return bytes(out)


def flatten(w, h, px, strength):
    """Remove the texture's large-scale brightness patches: move each pixel by
    `strength` of the gap between its blurred neighbourhood and the mean. Fine
    detail stays; the blotches that would show the tile grid go."""
    out = bytearray(px)
    r = max(2, min(w, h) // 5)
    for c in range(3):
        vals = [float(px[i * 4 + c]) for i in range(w * h)]
        mean = sum(vals) / len(vals)
        blur = box_blur_wrap(w, h, vals, r)
        for i in range(w * h):
            out[i * 4 + c] = max(0, min(255, round(vals[i] - strength * (blur[i] - mean))))
    return bytes(out)


def seamless(w, h, px):
    """Blend in a copy rolled by half the tile, weighted to own the edges, so
    the tile wraps: at its border it shows its own middle, which continues
    across the seam into the neighbour's middle."""
    fade_x, fade_y = w * 0.22, h * 0.22
    out = bytearray(len(px))
    for y in range(h):
        wy = min(1.0, min(y, h - 1 - y) / fade_y)
        ry = (y + h // 2) % h
        for x in range(w):
            wx = min(1.0, min(x, w - 1 - x) / fade_x)
            a = wx * wy
            a = a * a * (3 - 2 * a)
            o = (y * w + x) * 4
            r = (ry * w + (x + w // 2) % w) * 4
            for c in range(3):
                out[o + c] = round(px[o + c] * a + px[r + c] * (1 - a))
            out[o + 3] = 255
    return bytes(out)


def base_tile(name, tw, th):
    pick = PICKS[name]
    source, seed, (fx, fy) = pick if isinstance(pick, tuple) else (name, pick, (0.5, 0.5))
    path = os.path.join(ROOT, "game-assets", "ai-out", RUN, "terrain",
                        f"{source}_{seed}.png")
    w, h, px = read_png(path)
    cw = int(w * CROP[name])
    ch = cw * th // tw
    x0 = round((w - cw) * min(1.0, fx * 2)) if fx != 0.5 else (w - cw) // 2
    y0 = round((h - ch) * min(1.0, fy * 2)) if fy != 0.5 else (h - ch) // 2
    px = crop(w, h, px, x0, y0, cw, ch)
    _, _, px = gen_ai.box_downscale(cw, ch, px, tw, th)
    px = soften(tw, th, flatten(tw, th, px, FLATTEN[name]), DETAIL[name])
    return seamless(tw, th, px)


def wave(t, period, seed):
    """A wave over [0, period] that repeats exactly, so it is the same at
    both ends of a tile and at the start of the next."""
    v = 0.0
    for k, (amp, phase) in enumerate(((1.0, 0.3), (0.55, 1.7), (0.3, 4.1)), 1):
        v += amp * math.sin(2 * math.pi * k * t / period + phase + seed)
    return v / 1.85


def inside(kind, arg, w, h):
    """Signed distance into the terrain (> 0) from the grass, for one edge."""
    band_x, band_y = BAND * w, BAND * h
    amp = WAVE * h

    def side(s, x, y):
        if s == "N":
            return y - (band_y + amp * wave(x, w, 0.0))
        if s == "S":
            return (h - band_y - amp * wave(x, w, 2.0)) - y
        if s == "W":
            return x - (band_x + amp * wave(y, h, 4.0))
        return (w - band_x - amp * wave(y, h, 6.0)) - x

    if kind == "side":
        return lambda x, y: side(arg, x, y)
    if kind == "outer":
        a, b = arg
        return lambda x, y: min(side(a, x, y), side(b, x, y))
    r = CORNER * w
    corner = arg

    def inner(x, y):
        dx = (w - x) if corner in ("NE", "SE") else x
        dy = (h - y) if corner in ("SE", "SW") else y
        return dx + dy * w / h - r + amp * 0.6 * wave(x + y, w + h, 1.0)
    return inner


def edge_tile(terrain, grass, base, kind, arg, w, h, scale):
    pred = inside(kind, arg, w, h)
    soft = 2.5 * scale
    out = bytearray(w * h * 4)
    rim = (255, 255, 255) if terrain == "water" else (24, 30, 20)
    rim_w = 3.0 * scale
    for y in range(h):
        for x in range(w):
            d = pred(x, y)
            t = min(1.0, max(0.0, (d + soft) / (2 * soft)))
            t = t * t * (3 - 2 * t)
            o = (y * w + x) * 4
            col = [grass[o + c] * (1 - t) + base[o + c] * t for c in range(3)]
            # A foam line on water, a shaded rim on land, along the shore.
            near = max(0.0, 1 - abs(d) / rim_w)
            k = 0.35 * near if terrain == "water" else 0.25 * near
            col = [col[c] * (1 - k) + rim[c] * k for c in range(3)]
            out[o:o + 4] = bytes((round(col[0]), round(col[1]), round(col[2]), 255))
    return bytes(out)


def cmd_compose(cli):
    tw, th, scale = tile_size()
    tiles = os.path.join(PACK, "art", "tiles")
    written = []
    bases = {name: base_tile(name, tw, th) for name in BASES}
    for name, px in bases.items():
        write_png(os.path.join(tiles, name + ".png"), tw, th, px)
        written.append(f"art/tiles/{name}.png")
    for terrain, sides in (("forest", SIDES), ("desert", SIDES),
                           ("mountain", SIDES), ("water", WATER_SIDES)):
        for num, (kind, arg) in sides.items():
            px = edge_tile(terrain, bases["grass"], bases[terrain], kind, arg, tw, th, scale)
            rel = f"art/tiles/{terrain}_edge_{num}.png"
            write_png(os.path.join(PACK, rel), tw, th, px)
            written.append(rel)
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    for rel in (pack["sprites"]["combat"][0], pack["ending"]["grass_tile"]):
        write_png(os.path.join(PACK, rel), tw, th, bases["grass"])
        written.append(rel)
    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} terrain tiles at {tw}x{th}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=3)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="", help="comma list of base names")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
