"""The world map pictures: each continent drawn from its own tiles.

    python3 tools/gen_worldmap.py

The world map view (M) used to paint one flat colour per tile, which at the
pack's 4x art scale is a grid of blocks. This bakes a picture per continent
instead: every tile's art, box-filtered down to the size the view gives a
tile, so the coasts, forest edges and shore tiles keep their shapes. The
view draws it under the fog and the town, castle and hero markers, and
falls back to the colour blocks when a pack ships no picture.

The size follows `draw_worldmap` in `shell/src/views.rs`: the largest whole
number of logical pixels per tile that fits the padded map panel, times the
pack's art scale. Run after `gen_maps.py` and the terrain art; the maps are
pure terrain, so objects (castles, towns, chests) are the view's markers.
Writes `art/worldmap/<zone>.png`, the path the shell derives.
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_ai import box_downscale  # noqa: E402
from gen_terrain import SIDES, WATER_SIDES  # noqa: E402
from gen_terrain_ai import inside  # noqa: E402
from packart import PACK, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

AUTHOR = "composed by tools/gen_worldmap.py from the pack's own terrain tiles"

# Shrunk to a few pixels, grass and forest average to the same green. Each
# base is graded apart (an RGB multiplier) so a wall reads at a glance.
GRADE = {"grass": (1.18, 1.15, 0.95), "grass_variant": (1.18, 1.15, 0.95),
         "forest": (0.52, 0.6, 0.5), "mountain": (1.2, 1.17, 1.12),
         "desert": (1.05, 1.0, 0.95), "water": (1.0, 1.0, 1.0)}
SUPER = 4          # mask samples per pixel, per axis
SOFT = 0.35        # the edge's blend half-width, in output pixels
RIM = 0.9          # the shoreline rim's width, in output pixels
EDGE = re.compile(r"^(forest|mountain|desert|water)_(edge|shore)_(\d+)$")

# Legacy geometry (shell/src/layout.rs) and the view's padding (views.rs).
LEGACY_TILE = (48, 34)
LEGACY_TILES = (5, 5)
VIEW_PAD = 4


def tile_px(render, width, height):
    tw, th = LEGACY_TILE
    cols, rows = LEGACY_TILES
    if render.get("mode") != "legacy":
        tw, th = render["tile_w"], render["tile_h"]
        cols, rows = render["tiles_w"], render["tiles_h"]
    avail_w = tw * cols - 2 * VIEW_PAD
    avail_h = th * rows - 2 * VIEW_PAD
    pix = max(1, min(avail_w // width, avail_h // height))
    return pix * render.get("scale", 1)


def base_cell(name, cell):
    sw, sh, px = read_png(os.path.join(PACK, "art", "tiles", f"{name}.png"))
    px = box_downscale(sw, sh, px, cell, cell)[2]
    k = GRADE[name]
    return [tuple(min(255.0, px[i * 4 + c] * k[c]) for c in range(3))
            for i in range(cell * cell)]


def art_cell(art, cell, bases):
    """One tile's art at `cell` pixels: a base, or an edge rebuilt at this
    size from the same shape the terrain tool cut the full tile with (a
    shrunk full tile would blur its edge away)."""
    if art in GRADE:
        rgb = bases[art]
        return bytes(v for p in rgb for v in (round(p[0]), round(p[1]), round(p[2]), 255))
    m = EDGE.match(art)
    if not m:
        sys.exit(f"no world map rule for tile art {art!r}")
    terrain, kind, num = m.groups()
    shapes = WATER_SIDES if terrain == "water" else SIDES
    shape, arg = shapes[num]
    outside = bases["water"] if kind == "shore" else bases["grass"]
    foam = terrain == "water" or kind == "shore"
    rim = (255, 255, 255) if foam else (24, 30, 20)
    big = cell * SUPER
    pred = inside(shape, arg, big, big)
    out = bytearray(cell * cell * 4)
    for y in range(cell):
        for x in range(cell):
            t = near = 0.0
            for sy in range(SUPER):
                for sx in range(SUPER):
                    d = pred(x * SUPER + sx + 0.5, y * SUPER + sy + 0.5) / SUPER
                    t += min(1.0, max(0.0, (d + SOFT) / (2 * SOFT)))
                    near += max(0.0, 1 - abs(d) / RIM)
            t /= SUPER * SUPER
            near /= SUPER * SUPER
            i = y * cell + x
            col = [outside[i][c] * (1 - t) + bases[terrain][i][c] * t for c in range(3)]
            k = (0.45 if foam else 0.35) * near
            col = [col[c] * (1 - k) + rim[c] * k for c in range(3)]
            out[i * 4:i * 4 + 4] = bytes((round(col[0]), round(col[1]), round(col[2]), 255))
    return bytes(out)


def main():
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    codes = pack["tile_codes"]
    cache = {}
    bases = {}
    written = []
    for zone in pack["zones"]:
        with open(os.path.join(PACK, zone["map"])) as fh:
            rows = [line.rstrip("\n") for line in fh if not line.startswith("#")]
        h, w = len(rows), len(rows[0])
        cell = tile_px(pack["render"], w, h)
        out = bytearray(w * cell * h * cell * 4)
        stride = w * cell * 4
        for y, row in enumerate(rows):
            for x, ch in enumerate(row):
                art = codes[ch]["art"]
                if cell not in bases:
                    bases[cell] = {name: base_cell(name, cell) for name in GRADE}
                if (art, cell) not in cache:
                    cache[art, cell] = art_cell(art, cell, bases[cell])
                small = cache[art, cell]
                for yy in range(cell):
                    o = (y * cell + yy) * stride + x * cell * 4
                    out[o:o + cell * 4] = small[yy * cell * 4:(yy + 1) * cell * 4]
        rel = f"art/worldmap/{zone['id']}.png"
        os.makedirs(os.path.dirname(os.path.join(PACK, rel)), exist_ok=True)
        write_png(os.path.join(PACK, rel), w * cell, h * cell, bytes(out))
        written.append(rel)
        print(f"{rel}: {w * cell}x{h * cell}")
    register(written, AUTHOR, method="ai-generated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
