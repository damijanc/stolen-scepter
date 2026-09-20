"""Procedurally generate original terrain tiles for the open art pack.

Writes the 54 terrain slots (base tiles plus the 12-piece autotile edges) to
`packs/open/art/tiles/` and registers their provenance. The art is synthesized
from noise and hand-chosen palettes -- it never reads the original game art.

The edge scheme is functional, derived from how the maps use each piece:

  forest/desert/mountain  01=N+W 02=S+W 03=N+E 04=S+E (outer corners),
                          05=SE 06=NE 07=SW 08=NW (inner corners),
                          09=E 10=W 11=N 12=S (sides)
  water                   00=N+E 01=N+W 02=S+W 03=S+E,
                          04=SE 05=NE 06=SW 07=NW,
                          08=E 09=W 10=N 11=S

Every edge paints the base terrain shape over a grass underlay, which is the
model the maps are authored against.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from upscale import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "packs", "open", "art", "tiles")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
W, H = 48, 34
AUTHOR = "procedural (tools/gen_terrain.py)"

GRASS = [(34, 120, 52), (56, 150, 62), (92, 178, 80)]
GRASS_SPECK = (30, 100, 45)
FOREST = [(20, 80, 40), (34, 110, 50), (60, 140, 66)]
FOREST_HOLE = (14, 58, 34)
WATER = [(20, 70, 150), (36, 100, 190), (70, 140, 225)]
WATER_CREST = (150, 205, 245)
DESERT = [(196, 150, 96), (224, 184, 120), (246, 214, 150)]
MOUNTAIN = [(86, 88, 98), (122, 124, 134), (168, 170, 180)]
SNOW = (232, 236, 240)
FLOWER = [(240, 220, 90), (245, 245, 245), (235, 120, 150)]


def hash2(x, y, seed):
    h = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h >> 16) & 0xFF) / 255.0


def smooth(t):
    return t * t * (3 - 2 * t)


def bilinear(x, y, cell, seed):
    gx, gy = x / cell, y / cell
    x0, y0 = int(gx), int(gy)
    fx, fy = smooth(gx - x0), smooth(gy - y0)
    a = hash2(x0, y0, seed)
    b = hash2(x0 + 1, y0, seed)
    c = hash2(x0, y0 + 1, seed)
    d = hash2(x0 + 1, y0 + 1, seed)
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def ramp(cols, t):
    if t < 0.5:
        a, b, u = cols[0], cols[1], t * 2
    else:
        a, b, u = cols[1], cols[2], (t - 0.5) * 2
    return tuple(int(a[i] + (b[i] - a[i]) * u) for i in range(3))


def put(px, x, y, c):
    if 0 <= x < W and 0 <= y < H:
        o = (y * W + x) * 4
        px[o], px[o + 1], px[o + 2], px[o + 3] = c[0], c[1], c[2], 255


def new_tile():
    return bytearray([0, 0, 0, 0] * (W * H))


def grass_tile(seed=11):
    px = new_tile()
    for y in range(H):
        for x in range(W):
            t = 0.5 * bilinear(x, y, 5, seed) + 0.5 * hash2(x, y, seed + 1)
            c = ramp(GRASS, t)
            if hash2(x, y, seed + 7) > 0.94:
                c = GRASS_SPECK
            put(px, x, y, c)
    return px


def grass_variant(seed=17):
    px = grass_tile(seed)
    for i in range(12):
        gx = int(hash2(i, 1, seed) * (W - 4)) + 2
        gy = int(hash2(i, 2, seed) * (H - 6)) + 4
        for dy in range(3):
            put(px, gx, gy - dy, GRASS[2])
            put(px, gx + 1, gy - dy, GRASS[1])
        if hash2(i, 3, seed) > 0.6:
            f = FLOWER[int(hash2(i, 4, seed) * len(FLOWER))]
            put(px, gx + 1, gy - 3, f)
    return px


def water_tile(seed=23):
    px = new_tile()
    for y in range(H):
        band = bilinear(0, y, 2.5, seed)
        for x in range(W):
            t = 0.65 * band + 0.35 * bilinear(x, y, 4, seed + 1)
            c = ramp(WATER, t)
            if hash2(x, y, seed + 9) > 0.965:
                c = WATER_CREST
            put(px, x, y, c)
    return px


def desert_tile(seed=31):
    px = new_tile()
    for y in range(H):
        for x in range(W):
            t = 0.6 * bilinear(x, y, 7, seed) + 0.4 * hash2(x // 2, y, seed + 1)
            c = ramp(DESERT, t)
            if hash2(x, y, seed + 5) > 0.96:
                c = DESERT[2]
            put(px, x, y, c)
    return px


def forest_tile(seed=41):
    px = new_tile()
    for y in range(H):
        for x in range(W):
            t = 0.55 * bilinear(x, y, 6, seed) + 0.45 * hash2(x, y, seed + 1)
            c = ramp(FOREST, t)
            if hash2(x, y, seed + 11) > 0.95:
                c = FOREST_HOLE
            put(px, x, y, c)
    for i in range(9):
        cx = int(hash2(i, 1, seed) * W)
        cy = int(hash2(i, 2, seed) * H)
        r = 2 + int(hash2(i, 3, seed) * 2)
        for y in range(cy - r, cy + r + 1):
            for x in range(cx - r, cx + r + 1):
                if (x - cx) ** 2 + (y - cy) ** 2 <= r * r:
                    put(px, x, y, FOREST[2] if hash2(x, y, seed + 5) > 0.4 else FOREST[1])
    return px


def mountain_tile(seed=53):
    px = new_tile()
    for y in range(H):
        for x in range(W):
            t = 0.55 * bilinear(x, y, 6, seed) + 0.45 * hash2(x, y, seed + 1)
            c = ramp(MOUNTAIN, t)
            put(px, x, y, c)
    for i in range(3):
        apex_x = 8 + int(hash2(i, 1, seed) * (W - 16))
        apex_y = 3 + int(hash2(i, 2, seed) * 5)
        half = 8 + int(hash2(i, 3, seed) * 6)
        height = H - apex_y
        for dy in range(height):
            y = apex_y + dy
            hw = int(half * (dy / height))
            for x in range(apex_x - hw, apex_x + hw + 1):
                shade = (x - (apex_x - max(1, hw))) / (2 * max(1, hw))
                base = ramp(MOUNTAIN, 0.25 + 0.5 * shade)
                if (hw and abs(x - apex_x) < hw * 0.5 and dy < height * 0.28):
                    base = SNOW
                put(px, x, y, base)
    return px


# name -> (mask kind, sides/diagonal)
SIDES = {
    "01": ("outer", ("N", "W")),
    "02": ("outer", ("S", "W")),
    "03": ("outer", ("N", "E")),
    "04": ("outer", ("S", "E")),
    "05": ("inner", "SE"),
    "06": ("inner", "NE"),
    "07": ("inner", "SW"),
    "08": ("inner", "NW"),
    "09": ("side", "E"),
    "10": ("side", "W"),
    "11": ("side", "N"),
    "12": ("side", "S"),
}
WATER_SIDES = {
    "00": ("outer", ("N", "E")),
    "01": ("outer", ("N", "W")),
    "02": ("outer", ("S", "W")),
    "03": ("outer", ("S", "E")),
    "04": ("inner", "SE"),
    "05": ("inner", "NE"),
    "06": ("inner", "SW"),
    "07": ("inner", "NW"),
    "08": ("side", "E"),
    "09": ("side", "W"),
    "10": ("side", "N"),
    "11": ("side", "S"),
}


def outside_pred(kind, arg, seed):
    if kind == "side":
        return lambda x, y: _side(arg, x, y, seed)
    if kind == "outer":
        return lambda x, y: _side(arg[0], x, y, seed) or _side(arg[1], x, y, seed)
    corner = arg
    r = 9 + int(hash2(0, 0, seed) * 3)
    return lambda x, y: _inner(corner, x, y, r, seed)


def _side(side, x, y, seed):
    if side == "N":
        return y < 7 + int(bilinear(x, 0, 3, seed) * 6)
    if side == "S":
        return y >= H - 7 - int(bilinear(x, 0, 3, seed) * 6)
    if side == "W":
        return x < 6 + int(bilinear(0, y, 3, seed) * 6)
    return x >= W - 6 - int(bilinear(0, y, 3, seed) * 6)


def _inner(corner, x, y, r, seed):
    dx = (W - 1 - x) if corner in ("NE", "SE") else x
    dy = (H - 1 - y) if corner in ("SW", "SE") else y
    limit = r + int(hash2(x, y, seed) * 3)
    return dx + dy < limit


def edge_tile(base, underlay, kind, arg, seed):
    px = bytearray(base)
    pred = outside_pred(kind, arg, seed)
    for y in range(H):
        for x in range(W):
            if pred(x, y):
                o = (y * W + x) * 4
                px[o:o + 4] = underlay[o:o + 4]
    return px


def main():
    os.makedirs(OUT, exist_ok=True)
    grass = grass_tile()
    bases = {
        "grass": grass,
        "grass_variant": grass_variant(),
        "water": water_tile(),
        "desert": desert_tile(),
        "forest": forest_tile(),
        "mountain": mountain_tile(),
    }

    written = []

    def emit(name, px):
        path = os.path.join(OUT, name + ".png")
        write_png(path, W, H, bytes(px))
        written.append("art/tiles/" + name + ".png")

    for name, px in bases.items():
        emit(name, px)

    for terrain, sides in (("forest", SIDES), ("desert", SIDES),
                           ("mountain", SIDES), ("water", WATER_SIDES)):
        base = bases[terrain]
        for num, (kind, arg) in sides.items():
            seed = 100 + sum(ord(c) for c in terrain) + int(num)
            emit(f"{terrain}_edge_{num}", edge_tile(base, grass, kind, arg, seed))

    with open(PROVENANCE) as fh:
        doc = json.load(fh)
    today = time.strftime("%Y-%m-%d")
    for rel in written:
        doc["assets"][rel] = {
            "author": AUTHOR,
            "method": "procedural",
            "source": "original",
            "license": "CC0-1.0",
            "date": today,
        }
    with open(PROVENANCE, "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")

    print(f"wrote {len(written)} terrain tiles to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
