"""Programmatically draw the original hero and class-portrait sprites.

Covers `art/sprites/hero_walk_00..03`, `boat_00..03` (48x34, alpha-keyed) and
`art/classes/{knight,paladin,sorceress,barbarian}.png` (96x102). Original pixel
art composed from primitives; nothing reads the original game art.

Writes `packs/open/art/**` and registers provenance.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas  # noqa: E402
from upscale import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "packs", "open", "art")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
AUTHOR = "procedural (tools/gen_sprites.py)"

OUTLINE = (26, 22, 30)
SKIN = (232, 190, 152)
SKIN_D = (196, 150, 116)
STEEL_L = (198, 202, 212)
STEEL = (146, 152, 166)
STEEL_D = (98, 104, 118)
TUNIC = (58, 92, 168)
TUNIC_D = (38, 62, 120)
GOLD = (232, 196, 72)
GOLD_D = (176, 140, 40)
WHITE = (240, 240, 240)
RED = (186, 62, 52)
RED_D = (132, 42, 36)
BROWN = (122, 86, 50)
BROWN_D = (84, 58, 32)
WOOD_L = (176, 134, 88)
PURPLE = (122, 62, 150)
PURPLE_D = (84, 42, 106)
LEATHER = (170, 122, 76)


def transparent(w, h):
    return Canvas(w, h, (0, 0, 0, 0))


def outline_rect(c, x, y, w, h):
    c.frame(x, y, w, h, OUTLINE)


def hero_walk(frame):
    c = transparent(48, 34)
    bob = 0 if frame % 2 == 0 else -1
    swing = [0, 2, 0, -2][frame]
    top = 15 + bob

    # legs and boots
    c.rect(20 + swing, 24 + bob, 4, 6, TUNIC_D)
    c.rect(24 - swing, 24 + bob, 4, 6, TUNIC_D)
    c.rect(19 + swing, 29 + bob, 7, 2, BROWN_D)
    c.rect(23 - swing, 29 + bob, 7, 2, BROWN_D)

    # torso, belt, pauldrons
    c.rect(18, top, 12, 10, TUNIC)
    c.hline(18, top, 12, (86, 120, 196))
    c.hline(18, top + 8, 12, TUNIC_D)
    c.rect(18, top + 8, 12, 2, BROWN_D)
    c.rect(16, top, 4, 4, STEEL)
    c.rect(28, top, 4, 4, STEEL)

    # arms
    c.rect(15, top + 3, 3, 7, STEEL_D)
    c.rect(30, top + 3, 3, 7, STEEL_D)

    # shield (left) with a gold cross
    c.rect(11, top + 2, 6, 9, STEEL)
    outline_rect(c, 11, top + 2, 6, 9)
    c.vline(13, top + 3, 7, GOLD)
    c.hline(12, top + 6, 4, GOLD)

    # sword (right)
    c.vline(35, top - 2, 12, STEEL_L)
    c.hline(33, top + 8, 5, GOLD)

    # head, helm, plume
    c.rect(20, top - 6, 8, 7, SKIN)
    c.rect(19, top - 8, 10, 4, STEEL)
    c.hline(20, top - 5, 8, OUTLINE)
    c.set(22, top - 4, OUTLINE)
    c.set(25, top - 4, OUTLINE)
    c.rect(23, top - 11, 2, 3, RED)

    return c.px


def hero_boat(frame):
    c = transparent(48, 34)
    bob = frame % 2
    # hull
    for y in range(22 + bob, 31 + bob):
        t = (y - (22 + bob)) / 8.0
        hw = int(15 - t * 6)
        c.rect(24 - hw, y, hw * 2, 1, BROWN)
        c.rect(24 + hw - 4, y, 4, 1, BROWN_D)
    c.hline(11, 22 + bob, 26, WOOD_L)
    # mast and sail
    c.vline(24, 6 + bob, 16, BROWN_D)
    c.tri(25, 7 + bob, 25, 20 + bob, 39, 13 + bob, WHITE)
    c.tri(25, 9 + bob, 25, 18 + bob, 35, 13 + bob, RED)
    # oars alternate
    if frame % 2 == 0:
        c.vline(14, 18 + bob, 8, WOOD_L)
        c.vline(33, 18 + bob, 8, WOOD_L)
    else:
        c.vline(16, 20 + bob, 8, WOOD_L)
        c.vline(31, 20 + bob, 8, WOOD_L)
    # wake
    for x in range(6, 12):
        c.set(x, 32, (200, 220, 235))
    for x in range(36, 42):
        c.set(x, 32, (200, 220, 235))
    return c.px


def base_bust(bg, panel, accent):
    c = transparent(96, 102)
    c.rect(0, 0, 96, 102, bg)
    c.rect(8, 8, 80, 86, panel)
    c.frame(5, 5, 86, 92, accent)
    c.frame(7, 7, 82, 88, accent)
    return c


def bust_head(c, skin=SKIN, dark=SKIN_D):
    c.rect(43, 54, 10, 12, dark)
    c.disc(48, 40, 16, skin)
    c.rect(32, 36, 3, 10, dark)
    c.rect(61, 36, 3, 10, dark)


def bust_shoulders(c, main, dark):
    for y in range(66, 102):
        t = (y - 66) / 36.0
        hw = int(17 + t * 29)
        c.rect(48 - hw, y, hw * 2, 1, main)
        c.rect(min(95, 48 + hw - 8), y, 8, 1, dark)


def eyes(c):
    c.rect(41, 38, 3, 3, OUTLINE)
    c.rect(52, 38, 3, 3, OUTLINE)
    c.hline(45, 48, 6, SKIN_D)


def class_knight():
    c = base_bust((36, 44, 86), (52, 64, 116), STEEL)
    bust_shoulders(c, STEEL, STEEL_D)
    bust_head(c)
    c.rect(30, 26, 36, 18, STEEL)
    c.disc(48, 28, 17, STEEL)
    c.hline(31, 27, 34, STEEL_L)
    c.rect(36, 38, 24, 4, STEEL_D)
    c.rect(36, 38, 24, 1, OUTLINE)
    c.rect(42, 48, 12, 7, SKIN)
    c.rect(46, 12, 4, 12, RED)
    c.disc(48, 12, 4, RED)
    return c.px


def class_paladin():
    c = base_bust((120, 104, 56), (176, 156, 92), GOLD)
    bust_shoulders(c, (206, 182, 110), (150, 126, 66))
    bust_head(c)
    c.tri(30, 34, 16, 12, 34, 26, GOLD)
    c.tri(66, 34, 80, 12, 62, 26, GOLD)
    c.disc(48, 30, 16, STEEL)
    c.hline(33, 30, 30, STEEL_L)
    c.hline(34, 40, 28, GOLD)
    c.set(48, 30, (90, 190, 220))
    c.rect(42, 48, 12, 7, SKIN)
    eyes(c)
    return c.px


def class_sorceress():
    c = base_bust((62, 36, 92), (86, 52, 120), PURPLE)
    bust_shoulders(c, PURPLE, PURPLE_D)
    bust_head(c)
    c.disc(48, 34, 19, PURPLE_D)
    c.disc(48, 40, 16, PURPLE)
    c.disc(48, 44, 13, SKIN)
    c.disc(48, 26, 9, PURPLE_D)
    c.set(48, 26, GOLD)
    c.set(47, 25, GOLD)
    c.set(49, 25, GOLD)
    eyes(c)
    return c.px


def class_barbarian():
    c = base_bust((104, 56, 42), (142, 84, 56), BROWN_D)
    bust_shoulders(c, LEATHER, BROWN_D)
    bust_head(c, skin=(226, 178, 132), dark=(184, 138, 96))
    c.disc(48, 32, 19, BROWN)
    c.disc(48, 42, 14, (226, 178, 132))
    c.disc(48, 52, 13, BROWN_D)
    c.disc(48, 48, 9, (226, 178, 132))
    c.tri(30, 22, 24, 6, 38, 14, BROWN)
    c.tri(66, 22, 72, 6, 58, 14, BROWN)
    eyes(c)
    return c.px


def main():
    sprites = {}
    for i in range(4):
        sprites[f"art/sprites/hero_walk_{i:02d}.png"] = (48, 34, hero_walk(i))
        sprites[f"art/sprites/boat_{i:02d}.png"] = (48, 34, hero_boat(i))
    sprites["art/classes/knight.png"] = (96, 102, class_knight())
    sprites["art/classes/paladin.png"] = (96, 102, class_paladin())
    sprites["art/classes/sorceress.png"] = (96, 102, class_sorceress())
    sprites["art/classes/barbarian.png"] = (96, 102, class_barbarian())

    with open(PROVENANCE) as fh:
        doc = json.load(fh)
    today = time.strftime("%Y-%m-%d")
    for rel, (w, h, px) in sprites.items():
        assert len(px) == w * h * 4, (rel, len(px), w * h * 4)
        path = os.path.join(OUT, rel[len("art/"):])
        write_png(path, w, h, px)
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

    print(f"wrote {len(sprites)} sprite/portrait files to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
