"""Programmatically draw the original map-object tiles for the open art pack.

Covers the placeable objects: the six castle pieces (drawn as one 3x2 canvas
and sliced, so they align), town, chest, the four dwellings, sign and the two
bridges. Everything is original pixel art composed from primitives over the
same grass underlay the terrain edges use; nothing reads the original game art.

Writes `packs/stolen-scepter/art/tiles/*.png` and registers provenance.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_terrain import W, H, grass_tile, water_tile  # noqa: E402
from packart import ROOT, out_dir, register  # noqa: E402
from pngio import write_png  # noqa: E402

OUT = out_dir("tiles")
AUTHOR = "procedural (tools/gen_objects.py)"

STONE_L = (178, 176, 168)
STONE = (148, 145, 138)
STONE_D = (108, 105, 100)
STONE_XD = (74, 72, 68)
WOOD = (140, 96, 58)
WOOD_L = (176, 134, 88)
WOOD_D = (98, 64, 38)
ROOF = (158, 64, 54)
ROOF_D = (112, 42, 36)
PLASTER = (206, 188, 158)
GATE = (36, 27, 22)
IRON = (86, 86, 92)
GOLD = (232, 196, 72)
WINDOW = (58, 66, 84)
THATCH = (196, 156, 84)
THATCH_D = (150, 112, 56)
LEAF = (52, 116, 56)
LEAF_D = (32, 82, 40)
ROCK = (120, 118, 112)
ROCK_D = (84, 82, 78)
DIRT = (156, 126, 84)


class Canvas:
    def __init__(self, w, h, fill=(0, 0, 0, 0)):
        self.w, self.h = w, h
        self.px = bytearray(bytes(fill) * (w * h)) if len(fill) == 4 else bytearray(fill * (w * h))

    def set(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            o = (y * self.w + x) * 4
            self.px[o:o + 4] = bytes(c) if len(c) == 4 else bytes((c[0], c[1], c[2], 255))

    def rect(self, x, y, w, h, c):
        for yy in range(y, y + h):
            for xx in range(x, x + w):
                self.set(xx, yy, c)

    def frame(self, x, y, w, h, c):
        for xx in range(x, x + w):
            self.set(xx, y, c)
            self.set(xx, y + h - 1, c)
        for yy in range(y, y + h):
            self.set(x, yy, c)
            self.set(x + w - 1, yy, c)

    def hline(self, x, y, w, c):
        for xx in range(x, x + w):
            self.set(xx, y, c)

    def vline(self, x, y, h, c):
        for yy in range(y, y + h):
            self.set(x, yy, c)

    def disc(self, cx, cy, r, c):
        for yy in range(cy - r, cy + r + 1):
            for xx in range(cx - r, cx + r + 1):
                if (xx - cx) ** 2 + (yy - cy) ** 2 <= r * r:
                    self.set(xx, yy, c)

    def tri(self, ax, ay, bx, by, cx, cy, c):
        xs = [ax, bx, cx]
        ys = [ay, by, cy]
        for y in range(min(ys), max(ys) + 1):
            for x in range(min(xs), max(xs) + 1):
                def sign(p, q, r):
                    return (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])
                d1 = sign((x, y), (ax, ay), (bx, by))
                d2 = sign((x, y), (bx, by), (cx, cy))
                d3 = sign((x, y), (cx, cy), (ax, ay))
                neg = d1 < 0 or d2 < 0 or d3 < 0
                pos = d1 > 0 or d2 > 0 or d3 > 0
                if not (neg and pos):
                    self.set(x, y, c)

    def tile(self, src, cx, cy):
        for y in range(34):
            for x in range(48):
                o = (y * 48 + x) * 4
                self.set(cx * 48 + x, cy * 34 + y, tuple(src[o:o + 4]))

    def crop(self, cx, cy):
        out = bytearray(48 * 34 * 4)
        for y in range(34):
            for x in range(48):
                o = ((cy * 34 + y) * self.w + (cx * 48 + x)) * 4
                d = (y * 48 + x) * 4
                out[d:d + 4] = self.px[o:o + 4]
        return bytes(out)


def grass_canvas():
    c = Canvas(W, H)
    c.tile(grass_tile(), 0, 0)
    return c


def castle_pieces():
    c = Canvas(144, 68)
    for cy in range(2):
        for cx in range(3):
            c.tile(grass_tile(), cx, cy)

    def stone_block(x, y, w, h):
        c.rect(x, y, w, h, STONE)
        c.hline(x, y, w, STONE_L)
        c.vline(x, y, h, STONE_L)
        c.hline(x, y + h - 1, w, STONE_D)
        c.vline(x + w - 1, y, h, STONE_D)
        for yy in range(y + 5, y + h - 2, 5):
            c.hline(x + 1, yy, w - 2, STONE_XD)

    def merlons(x0, x1, y):
        for x in range(x0, x1, 8):
            c.rect(x, y, 5, 6, STONE)
            c.hline(x, y, 5, STONE_L)

    stone_block(3, 8, 20, 50)
    stone_block(121, 8, 20, 50)
    merlons(3, 23, 3)
    merlons(121, 141, 3)
    stone_block(23, 18, 98, 40)
    merlons(23, 121, 13)
    c.rect(23, 13, 98, 5, STONE_D)

    c.disc(72, 40, 9, GATE)
    c.rect(63, 40, 18, 18, GATE)
    for yy in range(40, 58, 4):
        c.hline(64, yy, 16, IRON)
    c.vline(70, 40, 18, IRON)
    c.vline(74, 40, 18, IRON)

    for (wx, wy) in ((9, 20), (16, 20), (128, 20), (135, 20), (40, 26), (100, 26), (54, 26), (86, 26)):
        c.rect(wx, wy, 4, 6, WINDOW)
        c.hline(wx, wy, 4, STONE_D)

    c.vline(72, 0, 20, WOOD_D)
    c.tri(73, 1, 73, 7, 82, 4, ROOF)

    return {
        "castle_tl": c.crop(0, 0),
        "castle_br": c.crop(1, 0),
        "castle_tr": c.crop(2, 0),
        "castle_ml": c.crop(0, 1),
        "castle_gate": c.crop(1, 1),
        "castle_mr": c.crop(2, 1),
    }


def house(c, x, y, w, h, wall, roof):
    c.rect(x, y, w, h, wall)
    c.hline(x, y, w, STONE_L)
    c.vline(x, y, h, STONE_L)
    c.hline(x, y + h - 1, w, STONE_XD)
    c.vline(x + w - 1, y, h, STONE_XD)
    c.tri(x - 2, y, x + w // 2, y - 8, x + w + 2, y, roof)
    c.vline(x + w // 2, y - 8, 8, roof)
    c.rect(x + w // 2 - 3, y + h - 8, 6, 8, WOOD_D)
    c.rect(x + 3, y + 3, 4, 4, WINDOW)


def town():
    c = grass_canvas()
    c.rect(8, 28, 34, 4, DIRT)
    house(c, 8, 16, 18, 13, PLASTER, ROOF)
    house(c, 28, 20, 14, 9, WOOD_L, ROOF_D)
    return c.px


def chest():
    c = grass_canvas()
    c.rect(11, 30, 26, 3, STONE_XD)
    c.rect(12, 15, 24, 15, WOOD)
    c.hline(12, 15, 24, WOOD_L)
    c.hline(12, 29, 24, WOOD_D)
    c.vline(12, 15, 15, WOOD_L)
    c.vline(35, 15, 15, WOOD_D)
    c.rect(12, 13, 24, 4, WOOD_D)
    c.hline(12, 13, 24, WOOD_L)
    c.rect(22, 16, 4, 14, IRON)
    c.rect(23, 20, 2, 3, GOLD)
    c.rect(12, 18, 24, 1, IRON)
    return c.px


def dwelling_plains():
    c = grass_canvas()
    c.disc(24, 22, 12, THATCH)
    c.disc(24, 20, 10, (THATCH[0] + 20, THATCH[1] + 16, THATCH[2] + 10))
    c.tri(12, 22, 24, 6, 36, 22, THATCH_D)
    c.rect(20, 22, 8, 10, WOOD_D)
    c.rect(23, 27, 2, 5, GATE)
    return c.px


def dwelling_forest():
    c = grass_canvas()
    c.rect(13, 16, 22, 16, WOOD)
    c.hline(13, 16, 22, WOOD_L)
    c.vline(13, 16, 16, WOOD_L)
    c.vline(34, 16, 16, WOOD_D)
    c.disc(17, 12, 8, LEAF)
    c.disc(30, 12, 8, LEAF)
    c.disc(24, 9, 9, LEAF_D)
    c.rect(20, 23, 8, 9, GATE)
    c.rect(16, 20, 3, 3, WINDOW)
    return c.px


def dwelling_hills():
    c = grass_canvas()
    c.tri(4, 32, 24, 8, 44, 32, ROCK)
    c.tri(6, 32, 22, 14, 30, 32, (ROCK[0] + 18, ROCK[1] + 18, ROCK[2] + 18))
    c.disc(24, 30, 9, GATE)
    c.rect(17, 30, 14, 2, GATE)
    c.vline(24, 22, 10, ROCK_D)
    return c.px


def dwelling_dungeon():
    c = grass_canvas()
    c.rect(10, 12, 28, 20, STONE_D)
    c.rect(10, 12, 28, 3, STONE)
    c.vline(10, 12, 20, STONE)
    c.vline(37, 12, 20, STONE_XD)
    c.disc(24, 22, 8, GATE)
    c.rect(16, 22, 16, 10, GATE)
    c.vline(18, 14, 18, STONE_XD)
    c.vline(30, 14, 18, STONE_XD)
    c.rect(12, 9, 5, 4, ROCK_D)
    c.rect(31, 9, 5, 4, ROCK_D)
    return c.px


def artifact_chest():
    c = grass_canvas()
    c.rect(11, 30, 26, 3, STONE_XD)
    c.rect(12, 15, 24, 15, (92, 62, 44))
    c.hline(12, 15, 24, (128, 92, 62))
    c.hline(12, 29, 24, (64, 42, 30))
    c.vline(12, 15, 15, (128, 92, 62))
    c.vline(35, 15, 15, (64, 42, 30))
    c.rect(12, 13, 24, 4, (64, 42, 30))
    c.hline(12, 13, 24, (128, 92, 62))
    c.rect(22, 16, 4, 14, IRON)
    c.rect(20, 20, 8, 5, GOLD)
    c.set(23, 22, (90, 190, 220))
    c.set(24, 22, (90, 190, 220))
    return c.px


def artifact_ring():
    c = grass_canvas()
    for y in range(10, 35):
        for x in range(12, 37):
            d2 = (x - 24) ** 2 + (y - 22) ** 2
            if 16 <= d2 <= 100:
                lit = y < 22
                c.set(x, y, (246, 220, 120) if lit else GOLD)
    c.set(24, 12, (90, 190, 220))
    c.set(25, 12, (90, 190, 220))
    c.set(24, 13, (150, 220, 240))
    return c.px


def wandering_army():
    c = grass_canvas()
    c.frame(22, 4, 4, 22, WOOD_D)
    c.rect(23, 4, 2, 22, WOOD)
    c.tri(24, 4, 24, 16, 40, 10, ROOF)
    c.tri(24, 4, 24, 16, 8, 10, ROOF_D)
    c.rect(18, 22, 12, 9, (72, 74, 84))
    c.rect(19, 20, 10, 4, (110, 112, 122))
    c.rect(20, 24, 2, 2, (30, 32, 40))
    c.rect(26, 24, 2, 2, (30, 32, 40))
    c.rect(19, 30, 3, 2, (40, 42, 50))
    c.rect(26, 30, 3, 2, (40, 42, 50))
    return c.px


def sign():
    c = grass_canvas()
    c.rect(22, 12, 4, 18, WOOD)
    c.vline(22, 12, 18, WOOD_L)
    c.vline(25, 12, 18, WOOD_D)
    c.rect(10, 8, 28, 12, WOOD_L)
    c.frame(10, 8, 28, 12, WOOD_D)
    c.hline(14, 12, 20, WOOD_D)
    c.hline(14, 15, 14, WOOD_D)
    return c.px


def bridge_h():
    c = Canvas(W, H)
    c.tile(water_tile(), 0, 0)
    c.rect(0, 11, W, 12, WOOD)
    c.hline(0, 11, W, WOOD_L)
    c.hline(0, 22, W, WOOD_D)
    for x in range(2, W, 4):
        c.vline(x, 12, 10, WOOD_D)
    c.hline(0, 13, W, WOOD_L)
    c.hline(0, 20, W, WOOD_D)
    return c.px


def bridge_v():
    c = Canvas(W, H)
    c.tile(water_tile(), 0, 0)
    c.rect(17, 0, 14, H, WOOD)
    c.vline(17, 0, H, WOOD_L)
    c.vline(30, 0, H, WOOD_D)
    for y in range(2, H, 4):
        c.hline(18, y, 12, WOOD_D)
    c.vline(20, 0, H, WOOD_L)
    c.vline(27, 0, H, WOOD_D)
    return c.px


def main():
    os.makedirs(OUT, exist_ok=True)
    tiles = {
        "town": town(),
        "chest": chest(),
        "artifact_chest": artifact_chest(),
        "artifact_ring": artifact_ring(),
        "wandering_army": wandering_army(),
        "dwelling_plains": dwelling_plains(),
        "dwelling_forest": dwelling_forest(),
        "dwelling_hills": dwelling_hills(),
        "dwelling_dungeon": dwelling_dungeon(),
        "sign": sign(),
        "bridge_h": bridge_h(),
        "bridge_v": bridge_v(),
    }
    tiles.update(castle_pieces())

    written = []
    for name, px in tiles.items():
        assert len(px) == W * H * 4, (name, len(px))
        write_png(os.path.join(OUT, name + ".png"), W, H, px)
        written.append("art/tiles/" + name + ".png")

    register(written, AUTHOR)

    print(f"wrote {len(written)} object tiles to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
