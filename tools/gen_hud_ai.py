"""AI sidebar art: the five HUD panels and their animations.

Two steps, like `gen_combat_ai.py`:

    python3 tools/gen_hud_ai.py render --run hud1 [--seeds 2] [--only contract,catapult]
    python3 tools/gen_hud_ai.py compose

`render` asks ComfyUI (through the tunnel) for the pieces -- the hooded
contract figure, the wooden catapult, the arcane star and the coin purse
(matted with rembg) -- plus TWO textures, aged parchment and dark wood, a few
seeds each, into local, ignored `game-assets/ai-out/<run>/hud/`, with a
contact sheet.

`compose` needs no tunnel. The five 48x34 panels sit on the black sidebar, so
each subject is a matted piece centred on a transparent cell (the black shows
through), except the contract, whose grey silhouette sits on a dark backdrop
like an empty wanted poster. The siege and magic loops are ONE picture
animated in code: the catapult bobs one logical pixel, the star breathes with
a glow pulse. Each silhouette is the same picture flattened to a dark grey so
the on/off pair clearly matches. The puzzle grid is the parchment texture with
dark grid lines on the 9x6 cell boundaries the shell stamps its covers on;
the cover is the wood texture with a one-logical-pixel darker edge.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from compose_startup import cover, over  # noqa: E402
from gen_hero_ai import over_transparent  # noqa: E402
from gen_objects_ai import subject  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "hud1"
DEFAULT_SEED = 101
PICKS = {"contract": 101, "catapult": 101, "magic": 101, "purse": 101,
         "parchment": 101, "wood": 101}
AUTHOR = "AI-generated (tools/gen_hud_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_hud_ai.py"

STYLE = ("single isolated subject, painterly classic high fantasy strategy game art, "
         "rich saturated colours, crisp detail, soft light, plain flat light grey "
         "background")
NEG_OBJECT = ("text, letters, words, watermark, signature, border, frame, person, people, "
              "hand, table, scenery, landscape, multiple objects, cropped, blurry, low "
              "quality, deformed")
NEG_FIGURE = ("text, letters, words, watermark, signature, border, frame, multiple people, "
              "full body, scenery, landscape, cropped, blurry, low quality, deformed")

TEXTURE_STYLE = ("seamless tileable texture, flat orthographic view, even lighting, "
                 "painterly fantasy game art")
TEXTURE_NEG = ("text, watermark, perspective, sky, ground, object, border, frame, "
               "blurry, low quality")

GEN_SIZE = (1024, 1024)

# name -> (prompt, negative): matted subjects.
SUBJECTS = {
    "contract": ("a hooded figure bust, a mysterious hooded assassin seen from the front, "
                 "head and shoulders only, the face hidden in deep shadow under a dark "
                 "hood, a dark cloak around the shoulders", NEG_FIGURE),
    "catapult": ("a single wooden medieval siege catapult, a wooden catapult with a "
                 "throwing arm and a scoop, seen in three-quarter view", NEG_OBJECT),
    "magic": ("a single glowing arcane magic star sigil, a radiant golden and blue magical "
              "star emblem radiating soft light and sparkles, isolated symbol", NEG_OBJECT),
    "purse": ("a leather coin purse, a small brown leather pouch tied at the neck, with a "
              "few gold coins spilling out beside it", NEG_OBJECT),
}
# name -> prompt: opaque textures, cropped/downscaled in code.
TEXTURES = {
    "parchment": "seamless texture of aged parchment paper, yellowed old map paper with "
                 "stains and creases",
    "wood": "seamless texture of dark carved wooden boards, old oak planks",
}

# Puzzle geometry, in LOGICAL pixels, mirroring the shell's `hud.rs` constants.
COVER_INSET = 2
COVER_W = 9
COVER_H = 6
GRID_COLS = 5
GRID_ROWS = 5

SUBJECT_BOX = 0.85   # subjects fill ~85% of a panel
PURSE_BAND = 23      # logical rows above the gold text (y 24..32): 92 px at scale 4
INK = (52, 36, 22)   # the puzzle grid's dark ink
EDGE_K = 0.45        # how dark the cover's 1-logical-px edge is


def tile_size():
    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    return 48 * scale, 34 * scale, scale


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "hud")


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
    for name, (prompt, neg) in SUBJECTS.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in seeds:
            render(args, f"{prompt}, {STYLE}", neg, GEN_SIZE, seed, True,
                   os.path.join(out, f"{name}_{seed}.png"))
    for name, prompt in TEXTURES.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in seeds:
            render(args, f"{prompt}, {TEXTURE_STYLE}", TEXTURE_NEG, GEN_SIZE, seed, False,
                   os.path.join(out, f"{name}_{seed}.png"))
    contact_sheet(out)
    return 0


# ---- compose ------------------------------------------------------------

def blank(W, H):
    return bytearray(W * H * 4)


def subject_pixels(name, W, H, box):
    """The picked subject, matted, cropped to its blob and scaled into `box`
    (fractions of W x H), returned with its centred origin."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{PICKS[name]}.png"))
    sw, sh, spx = subject(w, h, px)
    bw, bh = int(W * box), int(H * box)
    k = min(bw / sw, bh / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    return nw, nh, scaled, (W - nw) // 2, (H - nh) // 2


def purse_panel(W, H, band):
    """The purse fitted into the top `band` pixel rows of the panel -- above the
    gold figure the shell draws at logical y 24..32 -- centred horizontally and
    within that band, as large as fits there."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"purse_{PICKS['purse']}.png"))
    sw, sh, spx = subject(w, h, px)
    k = min(W / sw, band / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    return onto(W, H, nw, nh, scaled, (W - nw) // 2, (band - nh) // 2)


def grey_silhouette(w, h, px):
    """The subject flattened to a near-uniform dark grey: desaturated, then the
    luminance compressed into a small dark range, alpha kept."""
    out = bytearray(px)
    for i in range(w * h):
        o = i * 4
        if px[o + 3] < 16:
            out[o + 3] = 0
            continue
        lum = (px[o] * 299 + px[o + 1] * 587 + px[o + 2] * 114) // 1000
        v = 46 + lum // 20
        out[o] = out[o + 1] = out[o + 2] = v
        out[o + 3] = 255
    return bytes(out)


def brighten(w, h, px, k):
    out = bytearray(px)
    for i in range(w * h):
        o = i * 4
        if px[o + 3] < 16:
            continue
        for c in range(3):
            out[o + c] = min(255, round(px[o + c] * k))
    return bytes(out)


def onto(W, H, nw, nh, px, ox, oy):
    """The subject on a transparent panel (the black sidebar shows through)."""
    canvas = blank(W, H)
    over_transparent(W, canvas, nw, nh, px, ox, oy)
    return bytes(canvas)


def dark_backdrop(W, H):
    canvas = blank(W, H)
    top, low = (28, 26, 34), (10, 10, 14)
    for y in range(H):
        t = y / max(1, H - 1)
        col = tuple(round(top[c] + (low[c] - top[c]) * t) for c in range(3))
        for x in range(W):
            o = (y * W + x) * 4
            canvas[o:o + 3] = bytes(col)
            canvas[o + 3] = 255
    return canvas  # mutable, so `over` can composite onto it


def contract_panel(W, H):
    nw, nh, px, ox, oy = subject_pixels("contract", W, H, SUBJECT_BOX)
    canvas = dark_backdrop(W, H)
    over(W, canvas, nw, nh, grey_silhouette(nw, nh, px), ox, oy)
    return bytes(canvas)


def animated(name, W, H, frames, offset=None, pulse=None):
    """One subject as a 4-frame loop: a vertical `offset` bob, or a `pulse`
    brightness cycle, one of the two. Also returns the matching silhouette (the
    base picture, flattened to grey at the frame-0 position)."""
    nw, nh, px, ox, oy = subject_pixels(name, W, H, SUBJECT_BOX)
    out = []
    for frame in range(frames):
        dy = offset[frame] if offset else 0
        im = px if pulse is None else brighten(nw, nh, px, pulse[frame])
        out.append(onto(W, H, nw, nh, im, ox, oy + dy))
    silhouette = onto(W, H, nw, nh, grey_silhouette(nw, nh, px), ox, oy)
    return out, silhouette


def grid_ink(canvas, W, x, y):
    o = (y * W + x) * 4
    canvas[o:o + 3] = bytes(INK)


def puzzle_grid(W, H, scale):
    w, h, px = read_png(os.path.join(out_dir(RUN), f"parchment_{PICKS['parchment']}.png"))
    canvas = bytearray(cover(w, h, px, W, H))
    inset = COVER_INSET * scale
    cw = COVER_W * scale
    ch = COVER_H * scale
    thick = scale
    top = left = inset
    right = inset + GRID_COLS * cw
    bottom = inset + GRID_ROWS * ch
    for col in range(GRID_COLS + 1):
        x0 = inset + col * cw
        for x in range(x0, min(W, x0 + thick)):
            for y in range(top, bottom + thick):
                grid_ink(canvas, W, x, y)
    for row in range(GRID_ROWS + 1):
        y0 = inset + row * ch
        for y in range(y0, min(H, y0 + thick)):
            for x in range(left, right + thick):
                grid_ink(canvas, W, x, y)
    return bytes(canvas)


def puzzle_cover(scale):
    cw, ch = COVER_W * scale, COVER_H * scale
    w, h, px = read_png(os.path.join(out_dir(RUN), f"wood_{PICKS['wood']}.png"))
    canvas = bytearray(cover(w, h, px, cw, ch))
    edge = scale  # one logical pixel
    for y in range(ch):
        for x in range(cw):
            if x < edge or x >= cw - edge or y < edge or y >= ch - edge:
                o = (y * cw + x) * 4
                for c in range(3):
                    canvas[o + c] = round(canvas[o + c] * EDGE_K)
    return bytes(canvas)


def cmd_compose(cli):
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    scale = pack["render"].get("scale", 1)
    W, H = 48 * scale, 34 * scale
    hud = pack["sprites"]["hud"]
    ui = pack["sprites"]["ui"]
    written = []

    def put(rel, w, h, px):
        write_png(os.path.join(PACK, rel), w, h, px)
        written.append(rel)

    put(hud["contract_silhouette"], W, H, contract_panel(W, H))

    siege, siege_sil = animated("catapult", W, H, len(hud["siege_animation"]),
                                offset=[0, -scale, 0, scale])
    for frame, rel in enumerate(hud["siege_animation"]):
        put(rel, W, H, siege[frame])
    put(hud["siege_silhouette"], W, H, siege_sil)

    magic, magic_sil = animated("magic", W, H, len(hud["magic_animation"]),
                                pulse=[1.0, 1.18, 1.0, 0.88])
    for frame, rel in enumerate(hud["magic_animation"]):
        put(rel, W, H, magic[frame])
    put(hud["magic_silhouette"], W, H, magic_sil)

    put(hud["gold_purse"], W, H, purse_panel(W, H, PURSE_BAND * scale))

    put(hud["puzzle_grid"], W, H, puzzle_grid(W, H, scale))
    put(ui["puzzle_cover"], COVER_W * scale, COVER_H * scale, puzzle_cover(scale))

    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} HUD panels at {W}x{H}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=2)
    r.add_argument("--first-seed", type=int, default=DEFAULT_SEED)
    r.add_argument("--only", default="", help="comma list: contract, catapult, magic, "
                                              "purse, parchment, wood")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
