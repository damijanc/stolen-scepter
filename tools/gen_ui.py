"""Draw the pack's interface art: frame, sidebar icons, backdrops, splash,
class picker, inventory icons, endings, and the palette.

Placeholder art composed from primitives and the project's own glyphs
(`glyphs.py`); nothing reads the original game. Every path comes from the
built `game.json`, so a slot renamed in `data/rules.json` is followed here.
Sizes are the legacy cells the shell places them in (`shell/src/layout.rs`,
`startup.rs`).

Writes `packs/stolen-scepter/art/ui/*.png` and `palettes/palette.bin`, and
registers provenance. Run after `build_pack.py` has written `game.json` and
after `gen_sprites.py` (it reuses the hero and class portraits).
"""
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas  # noqa: E402
import gen_sprites  # noqa: E402
from gen_terrain import grass_tile  # noqa: E402
from glyphs import draw_text, text_width  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import write_png  # noqa: E402

AUTHOR = "procedural (tools/gen_ui.py)"
TITLE_TOP = "THE QUEST FOR A"
TITLE_MAIN = "STOLEN SCEPTER"

CLEAR = (0, 0, 0, 0)
INK = (20, 16, 24)
NIGHT = (16, 18, 40)
STONE_D = (52, 48, 58)
STONE = (88, 82, 94)
STONE_L = (128, 122, 132)
GOLD = (232, 196, 72)
GOLD_D = (164, 124, 40)
PARCH = (222, 200, 150)
PARCH_D = (176, 148, 98)
RED = (168, 44, 40)
RED_D = (110, 26, 30)
BLUE = (56, 84, 168)
GREEN = (62, 128, 60)
GREEN_D = (36, 84, 40)
SKY_TOP = (66, 104, 176)
SKY_LOW = (176, 200, 226)
WHITE = (240, 240, 240)
GLOW = (250, 236, 150)


def h2(x, y, seed):
    v = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    v = ((v ^ (v >> 13)) * 1274126177) & 0xFFFFFFFF
    return (v ^ (v >> 16)) / 0xFFFFFFFF


def mix(a, b, t):
    return tuple(round(a[i] + (b[i] - a[i]) * t) for i in range(3))


def jitter(c, x, y, seed, amount=10):
    d = int((h2(x, y, seed) - 0.5) * 2 * amount)
    return tuple(max(0, min(255, v + d)) for v in c)


def vgrad(c, x, y, w, h, top, low):
    for yy in range(h):
        c.rect(x, y + yy, w, 1, mix(top, low, yy / max(1, h - 1)))


def paste(c, px, w, h, x, y, scale=1.0):
    """Blit RGBA `px` (w x h) at (x, y), nearest-sampled to `scale`."""
    ow, oh = int(w * scale), int(h * scale)
    for yy in range(oh):
        for xx in range(ow):
            o = (int(yy / scale) * w + int(xx / scale)) * 4
            if px[o + 3] >= 128:
                c.set(x + xx, y + yy, tuple(px[o:o + 3]))


def centered(c, text, y, color, scale=1, shadow=INK):
    draw_text(c, text, (c.w - text_width(text, scale)) // 2, y, color, scale, shadow)


def bevel(c, x, y, w, h, light, dark):
    c.hline(x, y, w, light)
    c.vline(x, y, h, light)
    c.hline(x, y + h - 1, w, dark)
    c.vline(x + w - 1, y, h, dark)


# ---- landscape pieces ---------------------------------------------------

def hills(c, y0, color, seed, amp=8, period=40):
    for x in range(c.w):
        top = y0 - int(amp * (0.5 + 0.5 * math.sin(x / period * 6.283 + seed)))
        for y in range(top, c.h):
            c.set(x, y, jitter(color, x, y, seed, 6))


def tree(c, x, base, size, leaf=GREEN, dark=GREEN_D):
    c.rect(x - 1, base - size // 2, 3, size // 2, (92, 64, 40))
    for i in range(3):
        r = size // 2 - i * 2
        c.tri(x - r, base - size // 2 - i * size // 4, x, base - size - i * size // 4,
              x + r, base - size // 2 - i * size // 4, dark if i == 0 else leaf)


def house(c, x, base, w, h, wall=PARCH, roof=RED):
    c.rect(x, base - h, w, h, wall)
    c.tri(x - 2, base - h, x + w // 2, base - h - w // 2, x + w + 1, base - h, roof)
    c.rect(x + w // 2 - 2, base - 6, 4, 6, (70, 50, 36))
    c.rect(x + 2, base - h + 3, 3, 3, (60, 70, 96))


def tower(c, x, base, w, h, stone=STONE, roof=BLUE):
    c.rect(x, base - h, w, h, stone)
    for i in range(0, w, 4):
        c.rect(x + i, base - h - 3, 2, 3, stone)
    c.vline(x, base - h, h, STONE_D)
    c.rect(x + w // 2 - 1, base - h + 6, 2, 5, INK)
    if roof:
        c.tri(x - 1, base - h - 3, x + w // 2, base - h - 3 - w, x + w, base - h - 3, roof)


# ---- backdrops (240x102) ------------------------------------------------

def backdrop(kind):
    c = Canvas(240, 102, (0, 0, 0, 255))
    if kind == "dungeon":
        for y in range(102):
            for x in range(240):
                row = y // 8
                mortar = y % 8 == 0 or (x + (row % 2) * 8) % 16 == 0
                c.set(x, y, (24, 22, 28) if mortar else jitter((60, 56, 64), x // 16, row, 3, 8))
        for tx in (40, 200):
            c.rect(tx - 1, 30, 3, 12, (92, 64, 40))
            c.disc(tx, 26, 4, (240, 150, 50))
            c.disc(tx, 27, 2, GLOW)
        c.rect(96, 40, 48, 62, INK)
        c.disc(120, 40, 24, INK)
        return c.px
    if kind == "hillcave":
        vgrad(c, 0, 0, 240, 60, (110, 120, 150), (190, 180, 170))
        hills(c, 50, (120, 110, 96), 2, amp=20, period=60)
        hills(c, 80, (96, 88, 76), 5, amp=8, period=30)
        c.disc(120, 92, 22, INK)
        c.rect(98, 92, 45, 10, INK)
        return c.px
    vgrad(c, 0, 0, 240, 70, SKY_TOP, SKY_LOW)
    c.disc(200, 18, 8, (250, 240, 200))
    if kind == "plains":
        hills(c, 66, (98, 150, 80), 1, amp=6, period=50)
        hills(c, 84, (80, 136, 64), 4, amp=4, period=24)
        for x in (30, 180, 214):
            tree(c, x, 74, 16)
    elif kind == "forest":
        hills(c, 64, GREEN_D, 1, amp=4, period=30)
        for i in range(18):
            x = int(h2(i, 0, 9) * 240)
            tree(c, x, 70 + int(h2(i, 1, 9) * 30), 24 + int(h2(i, 2, 9) * 16))
    elif kind == "town":
        hills(c, 72, (98, 150, 80), 3, amp=3, period=40)
        for i, x in enumerate(range(12, 228, 30)):
            house(c, x, 92 - (i % 2) * 6, 20, 14 + (i % 3) * 4,
                  roof=(RED, (150, 96, 50), BLUE)[i % 3])
    elif kind == "castle":
        hills(c, 80, (98, 150, 80), 3, amp=3, period=40)
        c.rect(50, 46, 140, 40, STONE)
        for i in range(50, 190, 6):
            c.rect(i, 42, 3, 4, STONE)
        tower(c, 40, 86, 20, 56)
        tower(c, 180, 86, 20, 56)
        tower(c, 108, 60, 24, 40, roof=RED)
        c.rect(112, 66, 16, 20, INK)
        c.disc(120, 66, 8, INK)
    return c.px


# ---- frame and strips ---------------------------------------------------

def chrome(w=320, h=200, left=16, right=16, top=8, bottom=8):
    c = Canvas(w, h, CLEAR)
    for y in range(h):
        for x in range(w):
            if x < left or x >= w - right or y < top or y >= h - bottom:
                c.set(x, y, jitter(STONE, x // 4, y // 4, 7, 12))
    bevel(c, 0, 0, w, h, STONE_L, STONE_D)
    c.frame(left - 1, top - 1, w - left - right + 2, h - top - bottom + 2, GOLD_D)
    for x, y in ((left // 2, top // 2), (w - right // 2, top // 2),
                 (left // 2, h - bottom // 2), (w - right // 2, h - bottom // 2)):
        c.disc(x, y, 3, GOLD)
        c.set(x - 1, y - 1, GLOW)
    for y in range(top + 12, h - bottom - 8, 24):
        c.rect(left // 2 - 1, y, 3, 6, GOLD_D)
        c.rect(w - right // 2 - 1, y, 3, 6, GOLD_D)
    return c.px


def bar_strip(w=320, h=5):
    c = Canvas(w, h, (0, 0, 0, 255))
    c.hline(0, 0, w, GOLD_D)
    c.rect(0, 1, w, 3, RED_D)
    c.hline(0, 4, w, GOLD_D)
    for x in range(2, w, 8):
        c.set(x, 2, GOLD)
    return c.px


# ---- sidebar and inventory icons (48x34) -------------------------------

def icon():
    return Canvas(48, 34, CLEAR)


def contract_silhouette():
    c = icon()
    c.rect(14, 4, 20, 26, (60, 56, 64))
    c.disc(24, 13, 5, (40, 36, 44))
    c.rect(18, 19, 12, 7, (40, 36, 44))
    return c.px


def catapult(frame, lit=True):
    c = icon()
    wood, wood_d = ((150, 104, 60), (96, 64, 36)) if lit else ((60, 56, 64), (40, 36, 44))
    c.rect(8, 24, 32, 4, wood)
    for x in (12, 34):
        c.disc(x, 29, 3, wood_d)
    c.tri(18, 24, 24, 12, 30, 24, wood_d)
    angle = [0.9, 0.6, 0.25, 0.6][frame] if lit else 0.9
    for i in range(16):
        c.rect(24 - int(math.cos(angle) * i), 14 - int(math.sin(angle) * i), 2, 2, wood)
    if lit:
        bx, by = 24 - int(math.cos(angle) * 16), 14 - int(math.sin(angle) * 16)
        c.disc(bx, by, 2, (110, 106, 100))
    return c.px


def star(frame, lit=True):
    c = icon()
    col = [GOLD, GLOW, WHITE, GLOW][frame] if lit else (60, 56, 64)
    r = [9, 10, 11, 10][frame] if lit else 10
    pts = []
    for i in range(10):
        a = -math.pi / 2 + i * math.pi / 5
        rr = r if i % 2 == 0 else r * 0.45
        pts.append((24 + rr * math.cos(a), 17 + rr * math.sin(a)))
    for i in range(0, 10, 2):
        a, b, m = pts[i], pts[(i + 1) % 10], pts[(i - 1) % 10]
        c.tri(int(a[0]), int(a[1]), int(b[0]), int(b[1]), int(m[0]), int(m[1]), col)
    c.disc(24, 17, int(r * 0.5), col)
    if lit and frame == 2:
        for d in (-14, 14):
            c.set(24 + d, 17, WHITE)
            c.set(24, 17 + d // 2, WHITE)
    return c.px


def puzzle_grid():
    c = icon()
    c.rect(9, 2, 30, 30, PARCH)
    for i in range(6):
        c.vline(9 + i * 6, 2, 30, PARCH_D)
        c.hline(9, 2 + i * 6, 30, PARCH_D)
    c.rect(22, 15, 3, 3, RED)
    return c.px


def gold_purse():
    c = icon()
    c.disc(24, 21, 9, (150, 104, 60))
    c.rect(20, 9, 8, 5, (150, 104, 60))
    c.hline(19, 13, 10, GOLD_D)
    for x, y in ((10, 29), (16, 30), (34, 29), (39, 30)):
        c.disc(x, y, 2, GOLD)
    draw_text(c, "$", 20, 17, GOLD, 1)
    return c.px


def artifact_icon(name):
    c = icon()
    if name == "sword":
        c.rect(23, 3, 3, 20, STONE_L)
        c.rect(17, 22, 15, 3, GOLD)
        c.rect(23, 25, 3, 6, (110, 70, 40))
    elif name == "shield":
        c.rect(14, 4, 20, 14, BLUE)
        c.tri(14, 18, 24, 31, 34, 18, BLUE)
        c.rect(22, 6, 4, 20, GOLD)
        c.rect(16, 11, 16, 4, GOLD)
    elif name == "crown":
        c.rect(12, 18, 24, 8, GOLD)
        for x in (12, 21, 30):
            c.tri(x, 18, x + 3, 8, x + 6, 18, GOLD)
        c.disc(24, 22, 2, RED)
    elif name == "articles":
        c.rect(12, 6, 22, 24, PARCH)
        c.rect(32, 6, 4, 24, PARCH_D)
        for y in range(10, 27, 4):
            c.hline(15, y, 15, PARCH_D)
        c.disc(28, 26, 3, RED)
    elif name == "amulet":
        for i in range(24):
            a = math.pi * i / 23
            c.set(24 + int(12 * math.cos(a)), 6 + int(10 * math.sin(a)), GOLD_D)
        c.disc(24, 22, 6, GOLD)
        c.disc(24, 22, 3, (60, 160, 120))
    elif name == "ring":
        c.disc(24, 19, 9, GOLD)
        c.disc(24, 19, 6, CLEAR)
        c.disc(24, 9, 4, (120, 200, 240))
    elif name == "book":
        c.rect(12, 6, 24, 24, RED_D)
        c.rect(14, 8, 20, 20, RED)
        c.rect(34, 7, 2, 22, PARCH)
        draw_text(c, "*", 19, 13, GOLD, 1)
    elif name == "anchor":
        c.rect(23, 6, 3, 22, STONE_L)
        c.disc(24, 6, 3, STONE_L)
        c.disc(24, 6, 1, CLEAR)
        c.rect(17, 11, 15, 2, STONE_L)
        for i in range(10):
            c.set(14 + i, 22 + i // 2, STONE_L)
            c.set(34 - i, 22 + i // 2, STONE_L)
            c.set(14 + i, 23 + i // 2, STONE_L)
            c.set(34 - i, 23 + i // 2, STONE_L)
    else:
        c.disc(24, 17, 10, GOLD)
    return c.px


def zone_icon(index, seed):
    """A small scroll map with a continent blob, one per zone."""
    c = icon()
    c.rect(6, 4, 36, 26, (74, 110, 190))
    c.frame(6, 4, 36, 26, PARCH_D)
    c.rect(4, 3, 3, 28, PARCH)
    c.rect(41, 3, 3, 28, PARCH)
    for y in range(6, 28):
        for x in range(9, 40):
            d = math.hypot((x - 24) / 13, (y - 17) / 9)
            if d + (h2(x // 3, y // 3, seed) - 0.5) * 0.6 < 0.85:
                c.set(x, y, [(98, 160, 80), (52, 116, 56), (98, 160, 80), (214, 190, 120)][index % 4])
    draw_text(c, str(index + 1), 20, 13, WHITE, 1, INK)
    return c.px


# ---- startup, picker, endings ------------------------------------------

def scepter(c, x, y, size):
    c.rect(x - 1, y, 3, size, GOLD_D)
    c.rect(x, y, 1, size, GOLD)
    c.disc(x, y, size // 6, GOLD)
    c.disc(x, y, size // 10, RED)
    c.rect(x - size // 8, y + size // 5, size // 4, 2, GOLD)


def splash_logo():
    c = Canvas(320, 84, (0, 0, 0, 255))
    scepter(c, 160, 10, 40)
    centered(c, "AN OPENBOUNTY TALE", 58, GOLD, 1)
    return c.px


def splash_title():
    c = Canvas(320, 200, (0, 0, 0, 255))
    vgrad(c, 0, 0, 320, 140, NIGHT, (70, 60, 110))
    for i in range(40):
        c.set(int(h2(i, 0, 5) * 320), int(h2(i, 1, 5) * 100), WHITE)
    hills(c, 150, (36, 44, 40), 2, amp=12, period=70)
    tower(c, 136, 150, 18, 46, stone=(50, 48, 62), roof=(40, 30, 50))
    tower(c, 170, 150, 18, 38, stone=(50, 48, 62), roof=(40, 30, 50))
    c.rect(150, 122, 24, 28, (50, 48, 62))
    scepter(c, 160, 156, 30)
    centered(c, TITLE_TOP, 28, PARCH, 1)
    centered(c, TITLE_MAIN, 44, GOLD, 2)
    return c.px


def class_picker(portraits):
    """288x184: the four class portraits where `CLASS_FIGURES` expects them,
    each with its letter A-D."""
    c = Canvas(288, 184, (0, 0, 0, 255))
    vgrad(c, 0, 0, 288, 184, (30, 30, 60), (12, 12, 24))
    c.frame(0, 0, 288, 184, GOLD_D)
    spots = [(8, 74, 1.0), (120, 30, 1.0), (227, 40, 0.6), (166, 112, 0.6)]
    for i in (0, 1, 2, 3):
        x, y, s = spots[i]
        paste(c, portraits[i], 96, 102, x, y, s)
        letter = "ABCD"[i]
        draw_text(c, letter, x + 2, y - 12, GOLD, 1, INK)
    return c.px


def class_highlight(portrait):
    c = Canvas(42, 44, (0, 0, 0, 255))
    paste(c, portrait, 96, 102, -3, -1, 0.48)
    c.frame(0, 0, 42, 44, GOLD)
    return c.px


def end_screen(win):
    c = Canvas(144, 170, (0, 0, 0, 255))
    vgrad(c, 0, 0, 144, 170, (60, 30, 40) if win else (24, 22, 32), (20, 12, 16))
    for x in (16, 128):
        c.rect(x - 6, 20, 12, 150, (80, 70, 86) if win else (44, 40, 50))
    c.rect(56, 120, 32, 50, RED if win else RED_D)
    c.rect(48, 60, 48, 70, GOLD_D)
    c.rect(52, 66, 40, 60, (110, 30, 40))
    c.tri(48, 60, 72, 36, 96, 60, GOLD_D)
    if win:
        c.disc(72, 84, 8, (230, 190, 150))
        c.rect(62, 92, 20, 26, (70, 70, 160))
        c.rect(64, 72, 16, 5, GOLD)
        scepter(c, 88, 80, 26)
    return c.px


def end_tile(kind):
    if kind == "grass":
        return bytes(grass_tile())
    c = Canvas(48, 34, (0, 0, 0, 255))
    if kind == "carpet":
        c.rect(0, 0, 48, 34, RED)
        c.rect(4, 0, 2, 34, GOLD_D)
        c.rect(42, 0, 2, 34, GOLD_D)
        return c.px
    c = Canvas(48, 34, CLEAR)
    paste(c, gen_sprites.hero_walk(0), 48, 34, 0, 0)
    return c.px


def puzzle_cover():
    c = Canvas(9, 6, (0, 0, 0, 255))
    for y in range(6):
        for x in range(9):
            c.set(x, y, jitter(PARCH, x, y, 13, 14))
    c.frame(0, 0, 9, 6, PARCH_D)
    return c.px


# ---- palette ------------------------------------------------------------

# 0..15: the canonical sixteen text and chrome colours (the standard VGA
# set, which `shell/src/palette.rs` names). 16..255: ramps for art.
VGA16 = [(0, 0, 0), (0, 0, 170), (0, 170, 0), (0, 170, 170), (170, 0, 0),
         (170, 0, 170), (170, 85, 0), (170, 170, 170), (85, 85, 85),
         (85, 85, 255), (85, 255, 85), (85, 255, 255), (255, 85, 85),
         (255, 85, 255), (255, 255, 85), (255, 255, 255)]


def palette():
    out = list(VGA16)
    for i in range(16):
        v = round(i * 255 / 15)
        out.append((v, v, v))
    hues = [GOLD, RED, BLUE, GREEN, PARCH, STONE, (150, 104, 60), (120, 200, 240),
            (176, 70, 92), (98, 160, 80), (214, 190, 120), (74, 110, 190), gen_sprites.SKIN]
    for base in hues:
        for i in range(16):
            t = i / 15
            out.append(mix((0, 0, 0), base, t * 2) if t < 0.5 else mix(base, (255, 255, 255), (t - 0.5) * 1.4))
    while len(out) < 256:
        out.append((0, 0, 0))
    return bytes(v for rgb in out[:256] for v in rgb)



def main():
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    ui, hud, ending = pack["sprites"]["ui"], pack["sprites"]["hud"], pack["ending"]
    portraits = [gen_sprites.class_knight(), gen_sprites.class_paladin(),
                 gen_sprites.class_sorceress(), gen_sprites.class_barbarian()]

    art = {
        ui["chrome_overworld"]: (320, 200, chrome()),
        hud["bar_strip"]: (320, 5, bar_strip()),
        ui["splash_logo"]: (320, 84, splash_logo()),
        ui["splash_title"]: (320, 200, splash_title()),
        ui["class_picker"]: (288, 184, class_picker(portraits)),
        ui["class_highlight"]: (42, 44, class_highlight(portraits[0])),
        ui["puzzle_cover"]: (9, 6, puzzle_cover()),
        ui["ending_win"]: (144, 170, end_screen(True)),
        ui["ending_lose"]: (144, 170, end_screen(False)),
        ending["grass_tile"]: (48, 34, end_tile("grass")),
        ending["carpet_tile"]: (48, 34, end_tile("carpet")),
        ending["hero_tile"]: (48, 34, end_tile("hero")),
        hud["contract_silhouette"]: (48, 34, contract_silhouette()),
        hud["siege_silhouette"]: (48, 34, catapult(0, lit=False)),
        hud["magic_silhouette"]: (48, 34, star(0, lit=False)),
        hud["puzzle_grid"]: (48, 34, puzzle_grid()),
        hud["gold_purse"]: (48, 34, gold_purse()),
    }
    for kind in ("town", "castle", "plains", "forest", "hillcave", "dungeon"):
        art[ui[kind + "_backdrop"]] = (240, 102, backdrop(kind))
    for frame, rel in enumerate(hud["siege_animation"]):
        art[rel] = (48, 34, catapult(frame))
    for frame, rel in enumerate(hud["magic_animation"]):
        art[rel] = (48, 34, star(frame))
    for artifact in pack["artifacts"]:
        name = os.path.splitext(os.path.basename(artifact["icon"]))[0].rsplit("_", 1)[-1]
        art[artifact["icon"]] = (48, 34, artifact_icon(name))
    for index, rel in enumerate(ui["view_icons_extra"]):
        art[rel] = (48, 34, zone_icon(index, 31 + index))

    for rel, (w, h, px) in art.items():
        assert len(px) == w * h * 4, (rel, len(px), w * h * 4)
        path = os.path.join(PACK, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        write_png(path, w, h, bytes(px))

    pal_rel = pack["sprites"].get("palette", "palettes/palette.bin")
    os.makedirs(os.path.join(PACK, os.path.dirname(pal_rel)), exist_ok=True)
    with open(os.path.join(PACK, pal_rel), "wb") as fh:
        fh.write(palette())

    register(sorted(art) + [pal_rel], AUTHOR)
    print(f"wrote {len(art)} interface images and the palette to "
          f"{os.path.relpath(PACK, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
