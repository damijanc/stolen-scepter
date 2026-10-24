"""AI chrome art: the overworld frame, the bar strip, the end-cartoon carpet and hero.

Two steps, like `gen_hud_ai.py`:

    python3 tools/gen_chrome_ai.py render --run chrome1 [--seeds 2] [--only ...]
    python3 tools/gen_chrome_ai.py compose

`render` asks ComfyUI (through the tunnel) for the pieces -- three seamless
textures (castle stone, red velvet, red carpet) and a matted knight seen from
behind -- a few seeds each, into local, ignored
`game-assets/ai-out/<run>/chrome/`, with a contact sheet.

`compose` needs no tunnel. It replaces the four last procedural pieces at the
pack's art scale (4x):

- `chrome_overworld`: the transparent-interior frame the shell blits over the
  map, sidebar, status rect and bar strip. Its new alpha is EXACTLY the old
  file's alpha upscaled 4x nearest-neighbour (asserted); only the colours
  change. The border is the seamless stone texture sampled in screen
  coordinates with a soft bevel (light top-left, dark bottom-right) on both
  edges, the gold corner rivets are redrawn as anti-aliased round studs, the
  small gold bars on the side borders are redrawn as ornate rounded bar studs
  (rounded ends, a dark rim, a vertical gold gradient, a light-side highlight
  and a soft drop shadow), and the 1-logical-pixel trim lines keep their old
  positions and colours.
- `hud_bar_strip`: the deep-red velvet band under the status line, gold edges
  top and bottom, and a row of small gold beads where the old dots sat.
- `end_carpet`: the red carpet runner, tiling seamlessly top-to-bottom so the
  win cartoon's stacked column reads as one path, with code-drawn gold borders.
- `end_hero`: the matted knight walking up the carpet.
"""
import argparse
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
import gen_ui  # noqa: E402
from compose_startup import cover  # noqa: E402
from gen_hero_ai import over_transparent  # noqa: E402
from gen_objects_ai import subject  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "chrome1"
DEFAULT_SEED = 101
PICKS = {"stone": 101, "velvet": 101, "carpet": 101, "hero": 101}
AUTHOR = "AI-generated (tools/gen_chrome_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_chrome_ai.py"

STYLE = ("painterly classic high fantasy strategy game art, rich saturated colours, "
         "crisp detail, single subject, plain flat light grey background")
NEG_FIGURE = ("text, letters, words, watermark, signature, border, frame, multiple people, "
              "several characters, scenery, landscape, cropped, cut off, blurry, low "
              "quality, deformed")

TEXTURE_STYLE = ("seamless tileable texture, flat orthographic view, even lighting, "
                 "painterly fantasy game art")
TEXTURE_NEG = ("text, watermark, perspective, sky, ground, object, border, frame, "
               "blurry, low quality")

GEN_SIZE = (1024, 1024)
HERO_SIZE = (832, 1216)

# name -> base prompt: seamless opaque textures, cropped/downscaled in code.
TEXTURES = {
    "stone": "dark grey castle stone block masonry wall, large flat rectangular blocks "
             "with thin mortar joints",
    "velvet": "deep red crimson velvet fabric, rich dark red cloth with soft subtle folds",
    "carpet": "rich red royal carpet runner, deep crimson woven carpet with a subtle "
              "repeating weave",
}
HERO = ("a knight in polished steel plate armour with a flowing red cape, seen from "
        "behind, back view from slightly above, walking forward away from the viewer, "
        "full body, one character only, centered")

# Colours read from the current art (kept exactly for the trim and gold).
TRIM = (164, 124, 40)
GOLD = (232, 196, 72)
GOLD_HI = (250, 236, 150)
GOLD_D = (120, 78, 20)
BAR_RED = (110, 26, 30)

# Texture tile sizes, in art pixels, after downscale + seam blending.
STONE_TILE = 128
VELVET_TILE = 128
CARPET_TILE = 96

# The chrome frame is the fixed legacy 320x200 screen; the interior it lets
# show through is x [16,304), y [8,192) (shell/src/layout.rs FRAME_*).
FRAME_W, FRAME_H = 320, 200
INNER_L, INNER_R, INNER_T, INNER_B = 16, 304, 8, 192

# The bar strip's geometry, in LOGICAL pixels (shell/src/layout.rs BAR_H).
BAR_DOT_PERIOD = 8     # gold dots every 8 columns, first at x = 2
BAR_DOT_FIRST = 2

# The carpet's gold stripes, in LOGICAL pixels: 2px at x 4..6 and 42..44.
CARPET_STRIPES = [(4, 6), (42, 44)]


def pack_scale():
    with open(os.path.join(PACK, "game.json")) as fh:
        return json.load(fh)["render"].get("scale", 1)


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "chrome")


# ---- render -------------------------------------------------------------

def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, gen_ai.resolve_rembg("rembg"))
    if args.rembg is None:
        sys.exit("rembg not found; see gen_ai.py --rembg")
    gen_ai.clear_queue(args.host)
    only = set(filter(None, cli.only.split(",")))
    seeds = range(cli.first_seed, cli.first_seed + cli.seeds)
    for name, prompt in TEXTURES.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in seeds:
            render(args, f"{prompt}, {TEXTURE_STYLE}", TEXTURE_NEG, GEN_SIZE, seed, False,
                   os.path.join(out, f"{name}_{seed}.png"))
    if not only or "hero" in only:
        print("hero", flush=True)
        for seed in seeds:
            render(args, f"{HERO}, {STYLE}", NEG_FIGURE, HERO_SIZE, seed, True,
                   os.path.join(out, f"hero_{seed}.png"))
    contact_sheet(out)
    return 0


# ---- compose ------------------------------------------------------------

def blank(W, H):
    return bytearray(W * H * 4)


def make_seamless(w, h, px):
    """Blend in a copy rolled by half the tile so it wraps top->bottom and
    left->right, which is what lets a small texture repeat without seams."""
    out = bytearray(px)
    fx, fy = w * 0.22, h * 0.22
    for y in range(h):
        wy = min(1.0, min(y, h - 1 - y) / fy)
        ry = (y + h // 2) % h
        for x in range(w):
            wx = min(1.0, min(x, w - 1 - x) / fx)
            a = wx * wy
            a = a * a * (3 - 2 * a)
            o = (y * w + x) * 4
            r = (ry * w + (x + w // 2) % w) * 4
            for c in range(3):
                out[o + c] = round(px[o + c] * a + px[r + c] * (1 - a))
            out[o + 3] = 255
    return bytes(out)


def texture_tile(name, tw, th):
    """The picked texture, centre-cropped to aspect, box-filtered to tw x th and
    made seamless."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{PICKS[name]}.png"))
    return make_seamless(tw, th, cover(w, h, px, tw, th))


def upscale_alpha(w, h, rgba, k):
    """Nearest-neighbour upscale of the alpha channel alone, by k."""
    nw, nh = w * k, h * k
    out = bytearray(nw * nh)
    for y in range(nh):
        srow = (y // k) * w
        for x in range(nw):
            out[y * nw + x] = rgba[(srow + x // k) * 4 + 3]
    return bytes(out)


def chrome_overworld(scale):
    """The frame at 4x: new colours, alpha locked to the 4x old alpha.

    The 1x source frame is `gen_ui.chrome()` in memory, not
    `art/ui/chrome_overworld.png`: compose overwrites that path with this 4x
    result, so reading the file here would make the script unre-runnable."""
    old_w, old_h = FRAME_W, FRAME_H
    old_px = bytes(gen_ui.chrome(FRAME_W, FRAME_H, INNER_L, FRAME_W - INNER_R,
                                 INNER_T, FRAME_H - INNER_B))
    assert len(old_px) == old_w * old_h * 4
    W, H = old_w * scale, old_h * scale

    # Classify the old opaque pixels: trim (fixed colour), studs (gold), stone.
    trim = bytearray(old_w * old_h)
    stud = bytearray(old_w * old_h)
    for i in range(old_w * old_h):
        o = i * 4
        if old_px[o + 3] < 128:
            continue
        rgb = (old_px[o], old_px[o + 1], old_px[o + 2])
        if rgb == TRIM:
            trim[i] = 1
        elif rgb == GOLD or rgb == GOLD_HI:
            stud[i] = 1

    alpha = upscale_alpha(old_w, old_h, old_px, scale)
    canvas = blank(W, H)
    for i in range(W * H):
        canvas[i * 4 + 3] = alpha[i]
    stone = texture_tile("stone", STONE_TILE, STONE_TILE)

    # Stone border, sampled in SCREEN coordinates so the corners continue.
    for y in range(H):
        for x in range(W):
            i = y * W + x
            if alpha[i] < 128:
                continue
            s = ((y % STONE_TILE) * STONE_TILE + (x % STONE_TILE)) * 4
            canvas[i * 4:i * 4 + 3] = stone[s:s + 3]

    bevel(canvas, W, H, alpha, scale)

    # Split the trim mask: the thin frame outline stays crisp, the compact
    # 3x6 gold bars on the side borders become ornate studs.
    frame, bars = trim_bars(trim, old_w, old_h)

    # Trim frame: the old 1px outline, upscaled crisp (1 logical px = 4 px).
    for y in range(H):
        for x in range(W):
            i = y * W + x
            if alpha[i] < 128 or not frame[(y // scale) * old_w + (x // scale)]:
                continue
            canvas[i * 4:i * 4 + 3] = bytes(TRIM)

    # Gold bar studs, redrawn as ornate rounded bars in place of the old
    # flat rectangles.
    for cx, cy, hw, hh in bars:
        draw_bar_stud(canvas, W, H, alpha, cx * scale, cy * scale,
                      hw * scale, hh * scale, 1.0 * scale)

    # Gold corner rivets, redrawn as round anti-aliased studs.
    for cx, cy, r in stud_rivets(stud, old_w, old_h, scale):
        draw_rivet(canvas, W, H, alpha, cx, cy, r)

    # The rule: the new alpha must be exactly the 4x-upscaled old alpha.
    actual = bytes(canvas[i * 4 + 3] for i in range(W * H))
    assert actual == alpha, "chrome_overworld alpha diverged from the 4x original"
    return W, H, bytes(canvas)


def bevel(canvas, W, H, alpha, scale):
    """Soft bevel: light top-left, dark bottom-right, on the frame's outer
    (screen) edge and its inner (transparent interior) edge. The classic raised
    frame: light top/left outside, dark bottom/right outside, and the reverse on
    the inside."""
    il, it = INNER_L * scale, INNER_T * scale
    ir, ib = INNER_R * scale, INNER_B * scale
    bw = 3.0 * scale
    for y in range(H):
        for x in range(W):
            i = y * W + x
            if alpha[i] < 128:
                continue
            # Outer-edge closeness (1 at the screen edge, 0 at bw inward).
            ot = max(0.0, 1.0 - y / bw)
            ol = max(0.0, 1.0 - x / bw)
            ob = max(0.0, 1.0 - (H - 1 - y) / bw)
            orr = max(0.0, 1.0 - (W - 1 - x) / bw)
            # Inner-edge closeness (1 at the transparent interior edge).
            itc = max(0.0, 1.0 - (it - 1 - y) / bw) if y < it else 0.0
            ilc = max(0.0, 1.0 - (il - 1 - x) / bw) if x < il else 0.0
            ibc = max(0.0, 1.0 - (y - ib) / bw) if y >= ib else 0.0
            irc = max(0.0, 1.0 - (x - ir) / bw) if x >= ir else 0.0
            outer = 0.5 * (ot + ol - ob - orr)
            inner = 0.5 * (ibc + irc - itc - ilc)
            k = 1.0 + 0.16 * (outer + inner)
            o = i * 4
            for c in range(3):
                canvas[o + c] = max(0, min(255, round(canvas[o + c] * k)))


def stud_rivets(stud, w, h, scale):
    """The gold stud components' centres and radii, in art pixels."""
    seen = bytearray(w * h)
    out = []
    for i in range(w * h):
        if not stud[i] or seen[i]:
            continue
        stack = [i]
        seen[i] = 1
        comp = []
        while stack:
            j = stack.pop()
            comp.append(j)
            x, y = j % w, j // w
            for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if 0 <= nx < w and 0 <= ny < h:
                    k = ny * w + nx
                    if not seen[k] and stud[k]:
                        seen[k] = 1
                        stack.append(k)
        xs = [j % w for j in comp]
        ys = [j // w for j in comp]
        cx = sum(xs) / len(xs) * scale
        cy = sum(ys) / len(ys) * scale
        r = (max(xs) - min(xs) + max(ys) - min(ys) + 2) / 4.0 * scale
        out.append((cx, cy, r))
    return out


def draw_rivet(canvas, W, H, alpha, cx, cy, r):
    """An anti-aliased round gold rivet with a dark rim and a bright top-left
    highlight, blended into the stone by coverage (alpha is left alone)."""
    rim = max(1.5, r * 0.28)
    for y in range(max(0, int(cy - r - 1)), min(H, int(cy + r + 2))):
        for x in range(max(0, int(cx - r - 1)), min(W, int(cx + r + 2))):
            i = y * W + x
            if alpha[i] < 128:
                continue
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            cov = max(0.0, min(1.0, r - d + 0.5))
            if cov <= 0.0:
                continue
            hd = math.hypot(x + 0.5 - (cx - r * 0.35), y + 0.5 - (cy - r * 0.4))
            hl = max(0.0, min(1.0, 1.0 - hd / (r * 0.5)))
            rimk = max(0.0, min(1.0, (d - (r - rim)) / rim))
            o = i * 4
            for c in range(3):
                v = GOLD_D[c] * rimk + GOLD[c] * (1.0 - rimk)
                v = v * (1.0 - hl) + GOLD_HI[c] * hl
                canvas[o + c] = round(canvas[o + c] * (1.0 - cov) + v * cov)


def trim_bars(trim, w, h):
    """Split the trim mask into the thin frame outline and the compact gold bar
    studs. Returns (frame, bars) where `frame` is a bytearray marking the 1px
    outline and `bars` is a list of (cx, cy, hw, hh) in LOGICAL pixels."""
    frame = bytearray(w * h)
    bars = []
    seen = bytearray(w * h)
    for i in range(w * h):
        if not trim[i] or seen[i]:
            continue
        stack = [i]
        seen[i] = 1
        comp = []
        while stack:
            j = stack.pop()
            comp.append(j)
            x, y = j % w, j // w
            for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if 0 <= nx < w and 0 <= ny < h:
                    k = ny * w + nx
                    if not seen[k] and trim[k]:
                        seen[k] = 1
                        stack.append(k)
        xs = [j % w for j in comp]
        ys = [j // w for j in comp]
        x0, x1 = min(xs), max(xs)
        y0, y1 = min(ys), max(ys)
        # A compact block (the 3x6 side bars) becomes a stud; the frame outline
        # spans the whole screen and is kept as the crisp trim line.
        if x1 - x0 <= 12 and y1 - y0 <= 12:
            bars.append(((x0 + x1 + 1) / 2.0, (y0 + y1 + 1) / 2.0,
                         (x1 - x0 + 1) / 2.0, (y1 - y0 + 1) / 2.0))
        else:
            for j in comp:
                frame[j] = 1
    return frame, bars


def draw_bar_stud(canvas, W, H, alpha, cx, cy, hw, hh, r):
    """An ornate gold bar stud: rounded ends, a dark rim, a vertical gold
    gradient with a bright highlight on its light side (left for a vertical
    bar, top for a horizontal one), and a soft drop shadow cast onto the stone
    to the lower right. Draws RGB only; alpha is untouched."""
    rim = max(1.5, r * 0.35)
    ox, oy = 2.5, 3.5
    x0 = int(cx - hw - 4)
    x1 = int(cx + hw + 4)
    y0 = int(cy - hh - 4)
    y1 = int(cy + hh + 4)
    for y in range(max(0, y0), min(H, y1 + 1)):
        for x in range(max(0, x0), min(W, x1 + 1)):
            i = y * W + x
            if alpha[i] < 128:
                continue
            px = x + 0.5 - cx
            py = y + 0.5 - cy
            # Signed distance to the rounded rectangle (negative inside).
            qx = abs(px) - (hw - r)
            qy = abs(py) - (hh - r)
            sd = math.hypot(max(qx, 0.0), max(qy, 0.0)) + min(max(qx, qy), 0.0) - r
            cov = max(0.0, min(1.0, 0.5 - sd))
            o = i * 4
            if cov <= 0.0:
                # Drop shadow: a blurred copy of the bar shifted down-right,
                # darkening the stone just beyond the bar's lower-right edge.
                qx2 = abs(px - ox) - (hw - r)
                qy2 = abs(py - oy) - (hh - r)
                sd2 = math.hypot(max(qx2, 0.0), max(qy2, 0.0)) \
                    + min(max(qx2, qy2), 0.0) - r
                sh = max(0.0, min(1.0, (0.5 - sd2) / 2.5))
                if sh > 0.0:
                    k = 1.0 - 0.45 * sh
                    for c in range(3):
                        canvas[o + c] = max(0, min(255, round(canvas[o + c] * k)))
                continue
            # Dark rim along the edge, fading to the gold body.
            rimk = max(0.0, min(1.0, (sd + rim) / rim))
            # Vertical gold gradient, bright at the top.
            t = max(0.0, min(1.0, (py + hh) / (2.0 * hh)))
            # Bright highlight stripe on the light side.
            if hh >= hw:
                hl = max(0.0, min(1.0, 1.0 - (px + hw) / (hw * 0.6)))
            else:
                hl = max(0.0, min(1.0, 1.0 - (py + hh) / (hh * 0.6)))
            for c in range(3):
                base = GOLD[c] + (GOLD_D[c] - GOLD[c]) * (t * 0.7)
                body = base * (1.0 - hl) + GOLD_HI[c] * hl
                v = GOLD_D[c] * rimk + body * (1.0 - rimk)
                canvas[o + c] = round(canvas[o + c] * (1.0 - cov) + v * cov)


def hud_bar_strip(scale):
    """The 5-logical-px band: gold edges, deep-red velvet, a gold bead row."""
    W, H = FRAME_W * scale, 5 * scale
    vel = texture_tile("velvet", VELVET_TILE, VELVET_TILE)
    canvas = blank(W, H)
    for y in range(H):
        for x in range(W):
            o = (y * W + x) * 4
            canvas[o + 3] = 255
            ly = y / scale
            if ly < 1 or ly >= 4:
                canvas[o:o + 3] = bytes(TRIM)
            else:
                s = ((y % VELVET_TILE) * VELVET_TILE + (x % VELVET_TILE)) * 4
                lum = (vel[s] * 299 + vel[s + 1] * 587 + vel[s + 2] * 114) // 1000
                k = 0.55 + 0.7 * (lum / 255.0)
                canvas[o:o + 3] = bytes((min(255, round(BAR_RED[0] * k)),
                                         min(255, round(BAR_RED[1] * k)),
                                         min(255, round(BAR_RED[2] * k))))
    cy = 2.5 * scale
    for k in range(FRAME_W // BAR_DOT_PERIOD):
        cx = (BAR_DOT_FIRST + BAR_DOT_PERIOD * k) * scale + scale / 2.0
        draw_bead(canvas, W, H, cx, cy, 3.0)
    return W, H, bytes(canvas)


def draw_bead(canvas, W, H, cx, cy, r):
    """A small anti-aliased gold bead with a top-left highlight."""
    for y in range(max(0, int(cy - r - 1)), min(H, int(cy + r + 2))):
        for x in range(max(0, int(cx - r - 1)), min(W, int(cx + r + 2))):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            cov = max(0.0, min(1.0, r - d + 0.5))
            if cov <= 0.0:
                continue
            hd = math.hypot(x + 0.5 - (cx - r * 0.3), y + 0.5 - (cy - r * 0.35))
            hl = max(0.0, min(1.0, 1.0 - hd / (r * 0.7)))
            rimk = max(0.0, min(1.0, (d - (r - 1.0)) / 1.0))
            o = (y * W + x) * 4
            for c in range(3):
                v = GOLD_D[c] * rimk + GOLD[c] * (1.0 - rimk)
                v = v * (1.0 - hl) + GOLD_HI[c] * hl
                canvas[o + c] = round(canvas[o + c] * (1.0 - cov) + v * cov)


def end_carpet(scale):
    """The 48x34 carpet tile: seamless red carpet with code-drawn gold borders."""
    W, H = 48 * scale, 34 * scale
    car = texture_tile("carpet", CARPET_TILE, CARPET_TILE)
    canvas = blank(W, H)
    for y in range(H):
        for x in range(W):
            o = (y * W + x) * 4
            s = ((y % CARPET_TILE) * CARPET_TILE + (x % CARPET_TILE)) * 4
            canvas[o:o + 3] = car[s:s + 3]
            canvas[o + 3] = 255
    for (x0, x1) in CARPET_STRIPES:
        gx0, gx1 = x0 * scale, x1 * scale
        for y in range(H):
            for x in range(gx0, gx1):
                o = (y * W + x) * 4
                canvas[o:o + 3] = bytes(TRIM)
        # A gilded shine along each stripe's inner edge.
        hi = gx1 - 1 if x0 == CARPET_STRIPES[0][0] else gx0
        for y in range(H):
            o = (y * W + hi) * 4
            canvas[o:o + 3] = bytes(GOLD)
    return W, H, bytes(canvas)


def end_hero(scale):
    """The matted knight, ~85% of the tile height, centred on a transparent cell."""
    W, H = 48 * scale, 34 * scale
    w, h, px = read_png(os.path.join(out_dir(RUN), f"hero_{PICKS['hero']}.png"))
    sw, sh, spx = subject(w, h, px)
    nh = int(H * 0.85)
    nw = max(1, round(sw * nh / sh))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    canvas = blank(W, H)
    over_transparent(W, canvas, nw, nh, scaled, (W - nw) // 2, (H - nh) // 2)
    return W, H, bytes(canvas)


def cmd_compose(cli):
    scale = pack_scale()
    pieces = {
        "art/ui/chrome_overworld.png": chrome_overworld(scale),
        "art/ui/hud_bar_strip.png": hud_bar_strip(scale),
        "art/ui/end_carpet.png": end_carpet(scale),
        "art/ui/end_hero.png": end_hero(scale),
    }
    written = []
    for rel, (w, h, px) in pieces.items():
        write_png(os.path.join(PACK, rel), w, h, px)
        written.append(rel)
        print(f"wrote {rel} ({w}x{h})")
    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} chrome pieces at scale {scale}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=2)
    r.add_argument("--first-seed", type=int, default=DEFAULT_SEED)
    r.add_argument("--only", default="", help="comma list: stone, velvet, carpet, hero")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
