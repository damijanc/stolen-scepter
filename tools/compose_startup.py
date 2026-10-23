"""Compose the startup slots from the AI pieces `gen_startup.py` rendered.

Writes, at the pack's art scale (`render.scale`):

- `art/ui/class_select_picker.png` (288x184 logical): the hall, with each
  class's cut-out figure standing in its own column, where the shell's click
  regions expect it (`CLASS_FIGURES`, `shell/src/startup.rs`), and its letter
  A-D above it.
- `art/classes/<class>.png` (96x102): the portrait busts.
- `art/ui/class_select_highlight.png` (42x44): the credits image, cut from
  the knight's portrait.
- `art/ui/splash_title.png` (320x200): the castle at dusk with the game's
  title lettered over its sky.
- `art/ui/splash_logo.png` (320x84): the Scepter, with a byline.

The picks below name which seed of each piece was kept; the sources are in
local, ignored `game-assets/ai-out/<run>/startup/`, and the PNGs this writes
are what the repo keeps.

Usage: python3 tools/compose_startup.py
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_ai import alpha_components, box_downscale  # noqa: E402
from gen_objects import Canvas  # noqa: E402
from glyphs import draw_text, lit, text_width  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "startup1"
PICKS = {
    "hall": 103,
    "figure": {"knight": 101, "paladin": 101, "sorceress": 101, "barbarian": 103},
    "portrait": {"knight": 102, "paladin": 102, "sorceress": 102, "barbarian": 103},
    "title": 101,
    "logo": 103,
}
AUTHOR = "AI-generated (tools/gen_startup.py: SDXL DreamShaper XL), composed by " \
         "tools/compose_startup.py"

# Class index -> (letter, column in logical picker pixels: x, y, w, h). The
# same four columns are `CLASS_FIGURES` in `shell/src/startup.rs`, which is
# where a click on the picker lands; the two must change together. Each hero
# is scaled to FIGURE_H and stands on FLOOR_Y, centred in its column, with
# its letter centred above.
FIGURES = [
    ("knight", "A", (0, 4, 72, 178)),
    ("paladin", "B", (72, 4, 72, 178)),
    ("sorceress", "C", (144, 4, 72, 178)),
    ("barbarian", "D", (216, 4, 72, 178)),
]
FIGURE_H = 158
FLOOR_Y = 180
LETTER_Y = 8
PICKER_W, PICKER_H = 288, 184
PORTRAIT_W, PORTRAIT_H = 96, 102
HIGHLIGHT_W, HIGHLIGHT_H = 42, 44

GOLD = (232, 196, 72)
GOLD_L = (255, 236, 150)
GOLD_D = (190, 128, 36)
PARCH = (240, 226, 196)
INK = (20, 16, 24)

TITLE_TOP = "THE QUEST FOR A"
TITLE_MAIN = "STOLEN SCEPTER"
BYLINE = "AN OPENBOUNTY TALE"


def src(kind, name=None):
    seed = PICKS[kind] if name is None else PICKS[kind][name]
    stem = f"{kind}_{seed}" if name is None else f"{kind}_{name}_{seed}"
    return os.path.join(ROOT, "game-assets", "ai-out", RUN, "startup", stem + ".png")


def crop(w, h, px, x0, y0, cw, ch):
    out = bytearray(cw * ch * 4)
    for y in range(ch):
        s = ((y0 + y) * w + x0) * 4
        out[y * cw * 4:(y + 1) * cw * 4] = px[s:s + cw * 4]
    return bytes(out)


def cover(w, h, px, tw, th):
    """Centre-crop to the target aspect, then box-filter down to tw x th."""
    if w * th > h * tw:
        cw, ch = h * tw // th, h
    else:
        cw, ch = w, w * th // tw
    px = crop(w, h, px, (w - cw) // 2, (h - ch) // 2, cw, ch)
    return box_downscale(cw, ch, px, tw, th)[2]


def subject(w, h, px):
    """The largest matted blob, cropped to its box."""
    comps = alpha_components(w, h, px)
    blob = comps[0]
    xs = [j % w for j in blob]
    ys = [j // w for j in blob]
    x0, y0 = min(xs), min(ys)
    cw, ch = max(xs) - x0 + 1, max(ys) - y0 + 1
    return cw, ch, crop(w, h, px, x0, y0, cw, ch)


def over(dst_w, dst, w, h, px, ox, oy):
    """Alpha-composite RGBA `px` onto opaque RGBA `dst` at (ox, oy)."""
    dst_h = len(dst) // (dst_w * 4)
    for y in range(h):
        dy = oy + y
        if not 0 <= dy < dst_h:
            continue
        for x in range(w):
            dx = ox + x
            if not 0 <= dx < dst_w:
                continue
            o = (y * w + x) * 4
            a = px[o + 3]
            if a == 0:
                continue
            d = (dy * dst_w + dx) * 4
            t = a / 255
            for c in range(3):
                dst[d + c] = round(px[o + c] * t + dst[d + c] * (1 - t))


def picker(scale):
    tw, th = PICKER_W * scale, PICKER_H * scale
    hw, hh, hpx = read_png(src("hall"))
    canvas = bytearray(cover(hw, hh, hpx, tw, th))
    letters = Canvas(tw, th, (0, 0, 0, 0))
    for cls, letter, (bx, by, bw, bh) in FIGURES:
        fw, fh, fpx = read_png(src("figure", cls))
        sw, sh, spx = subject(fw, fh, fpx)
        nh = FIGURE_H * scale
        nw = max(1, sw * nh // sh)
        _, _, scaled = box_downscale(sw, sh, spx, nw, nh)
        ox = (bx + bw // 2) * scale - nw // 2
        oy = FLOOR_Y * scale - nh
        over(tw, canvas, nw, nh, scaled, ox, oy)
        draw_text(letters, letter, (bx + bw // 2 - 4) * scale, LETTER_Y * scale, GOLD, scale, INK)
    over(tw, canvas, tw, th, bytes(letters.px), 0, 0)
    return tw, th, bytes(canvas)


def lettering(w, h, px, text, y, dot, top, bottom, outline=2):
    """Centre `text` at row `y` in glyph dots of `dot` pixels: an `outline`-dot
    dark border, then the letters shaded from `top` to `bottom` colour."""
    px = bytearray(px)
    x0 = (w - text_width(text, dot)) // 2
    dots = [(x0 + (i * 8 + gx) * dot, y + gy * dot)
            for i, ch in enumerate(text) for gx, gy in lit(ch)]
    ink = outline * dot // 2
    for dx, dy in dots:
        for yy in range(dy - ink, dy + dot + ink):
            for xx in range(dx - ink, dx + dot + ink):
                if 0 <= xx < w and 0 <= yy < h:
                    o = (yy * w + xx) * 4
                    px[o:o + 3] = bytes(INK)
    span = 7 * dot
    for dx, dy in dots:
        for yy in range(dy, dy + dot):
            t = min(1.0, max(0.0, (yy - y) / span))
            col = tuple(round(top[c] + (bottom[c] - top[c]) * t) for c in range(3))
            for xx in range(dx, dx + dot):
                if 0 <= xx < w and 0 <= yy < h:
                    o = (yy * w + xx) * 4
                    px[o:o + 3] = bytes(col)
    return bytes(px)


def darken_top(w, h, px, depth, strength):
    """Fade the top `depth` rows toward black, strongest at the top edge."""
    px = bytearray(px)
    for y in range(min(depth, h)):
        k = 1 - strength * (1 - y / depth)
        for x in range(w):
            o = (y * w + x) * 4
            for c in range(3):
                px[o + c] = round(px[o + c] * k)
    return bytes(px)


def title(scale):
    tw, th = 320 * scale, 200 * scale
    w, h, px = read_png(src("title"))
    out = cover(w, h, px, tw, th)
    out = darken_top(tw, th, out, 90 * scale, 0.55)
    out = lettering(tw, th, out, TITLE_TOP, 18 * scale, scale, PARCH, GOLD)
    out = lettering(tw, th, out, TITLE_MAIN, 34 * scale, 2 * scale, GOLD_L, GOLD_D)
    return tw, th, out


def logo(scale):
    tw, th = 320 * scale, 84 * scale
    w, h, px = read_png(src("logo"))
    out = cover(w, h, px, tw, th)
    out = lettering(tw, th, out, BYLINE, 72 * scale, scale, PARCH, GOLD)
    return tw, th, out


def main():
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    scale = pack["render"].get("scale", 1)
    written = {}

    ui = pack["sprites"]["ui"]
    written[ui["class_picker"]] = picker(scale)
    written[ui["splash_title"]] = title(scale)
    written[ui["splash_logo"]] = logo(scale)
    for cls in pack["classes"]:
        w, h, px = read_png(src("portrait", cls["id"]))
        tw, th = PORTRAIT_W * scale, PORTRAIT_H * scale
        written[cls["portrait"]] = (tw, th, cover(w, h, px, tw, th))
        if cls["id"] == "knight":
            hw, hh = HIGHLIGHT_W * scale, HIGHLIGHT_H * scale
            written[pack["sprites"]["ui"]["class_highlight"]] = (hw, hh, cover(w, h, px, hw, hh))

    for rel, (w, h, px) in written.items():
        write_png(os.path.join(PACK, rel), w, h, px)
        print(f"wrote {rel} ({w}x{h})")
    register(sorted(written), AUTHOR, method="ai-generated")
    return 0


if __name__ == "__main__":
    sys.exit(main())
