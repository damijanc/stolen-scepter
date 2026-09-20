"""Programmatically draw the original combat tileset for the open art pack.

Covers `art/combat/`: the grass field (opaque), three obstacles, the hit-burst
splat, six castle wall pieces and the four-frame cursor ring. Slot order and
meaning follow the renderer: index 0 field, 1..3 obstacles, 4 burst, 5..10
walls, 11..14 cursor.

Writes `packs/open/art/combat/*.png` and registers provenance.
"""
import json
import math
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import (  # noqa: E402
    Canvas, DIRT, GOLD, LEAF, LEAF_D, ROCK, ROCK_D,
    STONE, STONE_D, STONE_L, STONE_XD,
)

BROWN = (122, 86, 50)
BROWN_D = (84, 58, 32)
RED = (186, 62, 52)
from gen_terrain import W, H, grass_tile  # noqa: E402
from upscale import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "packs", "open", "art", "combat")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
AUTHOR = "procedural (tools/gen_combat.py)"

ORANGE = (222, 132, 48)
YELLOW = (246, 216, 96)
WHITE = (250, 250, 250)


def transparent():
    return Canvas(W, H, (0, 0, 0, 0))


def wall_face(c, x, y, w, h):
    c.rect(x, y, w, h, STONE)
    for px in range(x, x + w, 8):
        c.vline(px, y, h, STONE_D)
        c.vline(px + 1, y, h, STONE_L)
    c.hline(x, y, w, STONE_L)
    c.hline(x, y + h - 1, w, STONE_XD)


def merlons(c, y, x0, x1, h=6):
    for mx in range(x0, x1, 8):
        c.rect(mx, y, 5, h, STONE)
        c.hline(mx, y, 5, STONE_L)
        c.vline(mx + 4, y, h, STONE_D)


def obstacle_boulder():
    c = transparent()
    c.disc(19, 25, 9, ROCK)
    c.disc(30, 26, 8, ROCK_D)
    c.disc(24, 30, 7, ROCK)
    c.disc(18, 22, 5, STONE_L)
    c.disc(29, 23, 4, (146, 144, 138))
    c.disc(24, 29, 3, (150, 148, 142))
    return c.px


def obstacle_tree():
    c = transparent()
    c.rect(22, 20, 5, 12, BROWN_D)
    c.vline(22, 20, 12, BROWN)
    c.disc(24, 13, 11, LEAF_D)
    c.disc(19, 11, 7, LEAF)
    c.disc(29, 12, 7, LEAF)
    c.disc(24, 8, 8, LEAF)
    c.disc(20, 9, 3, (96, 168, 92))
    c.disc(27, 13, 2, (96, 168, 92))
    return c.px


def obstacle_mound():
    c = transparent()
    for y in range(16, 34):
        t = (y - 16) / 17.0
        hw = int(4 + t * 18)
        c.rect(24 - hw, y, hw * 2, 1, DIRT)
        c.rect(min(47, 24 + hw - 5), y, 5, 1, (120, 94, 60))
    for x in range(8, 40):
        c.set(x, 16 + abs(x - 24) // 3, LEAF_D)
    for x in range(10, 38):
        c.set(x, 17 + abs(x - 24) // 3, LEAF)
    c.disc(18, 28, 2, ROCK_D)
    c.disc(30, 30, 2, ROCK_D)
    return c.px


def castle_spike():
    c = transparent()
    cx, cy = 24, 17
    for i in range(8):
        a = i * math.pi / 4 + 0.2
        for r in range(4, 17):
            x = int(cx + math.cos(a) * r)
            y = int(cy + math.sin(a) * r)
            c.set(x, y, RED if r > 9 else ORANGE)
            if r > 12:
                c.set(x + 1, y, RED)
    c.disc(cx, cy, 6, ORANGE)
    c.disc(cx, cy, 4, YELLOW)
    c.disc(cx, cy, 2, WHITE)
    return c.px


def wall_piece(kind):
    c = transparent()
    if kind in ("04", "05"):
        wall_face(c, 0, 0, W, H)
        if kind == "04":
            c.vline(46, 0, H, STONE_D)
            c.vline(47, 0, H, STONE_XD)
        else:
            c.vline(1, 0, H, STONE_D)
            c.vline(0, 0, H, STONE_XD)
    else:
        wall_face(c, 0, 6, W, H - 6)
        merlons(c, 0, 0, W)
        if kind == "01":
            c.rect(0, 0, 6, H, STONE_D)
            c.vline(0, 0, H, STONE_XD)
        elif kind == "02":
            c.rect(W - 6, 0, 6, H, STONE_D)
            c.vline(W - 1, 0, H, STONE_XD)
    return c.px


def cursor(frame):
    c = transparent()
    cx, cy = 24, 17
    for y in range(H):
        for x in range(W):
            dx, dy = x - cx, y - cy
            d2 = dx * dx + dy * dy
            if 121 <= d2 <= 196:
                ang = (math.atan2(dy, dx) + math.pi) % (2 * math.pi)
                gap = (frame * math.pi / 2)
                diff = abs(((ang - gap + math.pi) % (2 * math.pi)) - math.pi)
                if diff > 0.5:
                    lit = (x + y + frame) % 3 == 0
                    c.set(x, y, WHITE if lit else GOLD)
    return c.px


def main():
    os.makedirs(OUT, exist_ok=True)
    tiles = {
        "field_grass": grass_tile(),
        "obstacle_01": obstacle_boulder(),
        "obstacle_02": obstacle_tree(),
        "obstacle_03": obstacle_mound(),
        "castle_spike": castle_spike(),
        "castle_wall_01": wall_piece("01"),
        "castle_wall_02": wall_piece("02"),
        "castle_wall_03": wall_piece("03"),
        "castle_wall_04": wall_piece("04"),
        "castle_wall_05": wall_piece("05"),
        "castle_wall_06": wall_piece("06"),
        "cursor_01": cursor(0),
        "cursor_02": cursor(1),
        "cursor_03": cursor(2),
        "cursor_04": cursor(3),
    }

    with open(PROVENANCE) as fh:
        doc = json.load(fh)
    today = time.strftime("%Y-%m-%d")
    for name, px in tiles.items():
        assert len(px) == W * H * 4, (name, len(px))
        write_png(os.path.join(OUT, name + ".png"), W, H, px)
        doc["assets"]["art/combat/" + name + ".png"] = {
            "author": AUTHOR,
            "method": "procedural",
            "source": "original",
            "license": "CC0-1.0",
            "date": today,
        }
    with open(PROVENANCE, "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")

    print(f"wrote {len(tiles)} combat tiles to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
