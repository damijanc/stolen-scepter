"""AI combat pieces: obstacles, hit burst, castle walls and the target ring.

Two steps, like `gen_objects_ai.py`:

    python3 tools/gen_combat_ai.py render --run combat1 [--seeds 2] [--only ...]
    python3 tools/gen_combat_ai.py compose

`render` asks ComfyUI (through the tunnel) for the matted pieces -- three field
obstacles and the hit burst -- plus ONE seamless castle-stone wall texture,
several seeds each, into local, ignored `game-assets/ai-out/<run>/combat/`,
with a contact sheet.

`compose` needs no tunnel. The obstacles and the burst are matted into the
transparent combat cell (the shell draws the grass under them, so everything
but the subject is alpha 0). The six castle wall pieces are all drawn in code
from the one wall texture, so they share the same masonry and meet seamlessly
at shared cell edges; the four-frame target ring is drawn in code as an
anti-aliased golden ring that shrinks. `field_grass` (slot 0) is written by
`gen_terrain_ai.py` and is left alone.
"""
import argparse
import json
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from gen_hero_ai import over_transparent  # noqa: E402
from gen_objects_ai import subject  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "combat1"
DEFAULT_SEED = 101
PICKS = {
    "obstacle_01": 101, "obstacle_02": 101, "obstacle_03": 101,
    "castle_spike": 101, "wall_texture": 101,
}
AI_AUTHOR = "AI-generated (tools/gen_combat_ai.py: SDXL DreamShaper XL), composed by " \
            "tools/gen_combat_ai.py"
CURSOR_AUTHOR = "procedural (tools/gen_combat_ai.py)"

STYLE = ("three-quarter top-down view, single isolated object, painterly classic high "
         "fantasy strategy game art, rich saturated colours, crisp detail, plain flat "
         "light grey background")
NEG = ("text, letters, words, watermark, signature, border, frame, people, person, "
       "landscape, scenery, ground plane, horizon, sky, blurry, low quality, deformed")

WALL_PROMPT = ("seamless tileable texture of grey castle stone masonry wall, flat "
               "orthographic front view, even lighting, painterly fantasy game art")
WALL_NEG = ("text, watermark, perspective, sky, ground, grass, people, border, frame, "
            "blurry, low quality")

GEN_SIZE = (1024, 1024)

# name -> (prompt, box: fraction of the cell the subject may fill, baseline and
# foot centre as fractions of the cell). The box, baseline and foot centre are
# the measured targets from tools/asset_layout.json for those slots.
OBSTACLES = {
    "obstacle_01": ("a cluster of mossy grey boulders, several large weathered rocks "
                    "piled together with moss and small ferns in the crevices",
                    (0.90, 0.91), 33 / 34, 18.68 / 48),
    "obstacle_02": ("a lone leafy oak tree, a single tall oak with a full crown of "
                    "green leaves and a thick trunk",
                    (0.875, 0.88), 31 / 34, 20.02 / 48),
    "obstacle_03": ("a fallen dead tree trunk with a broken stump, a mossy log lying "
                    "on the ground",
                    (0.917, 0.91), 1.0, 11.62 / 48),
}
# The hit flash the shell draws over a damaged unit (combat tile 4).
SPIKE = ("a bright orange-white impact burst, a magical spark explosion radiating "
         "light and sparks from a single point", (0.60, 0.60))

# Castle geometry, in art pixels (the cell is 48x34 logical x scale).
STONE_TW, STONE_TH = 64, 68     # the wall texture tile; divides the cell 3x2 so
#                                  every piece shares one masonry phase and meets
#                                  its neighbours without a step.
STONE_ZOOM = 1.5                # stones sampled ~1.5x larger than the source
FRONT_TOP = 8                   # front wall top inset (2 logical)
MERLON_W = 20                   # merlon width (5 logical)
MERLON_GAP = 12                 # crenel gap (3 logical)
MERLON_PERIOD = MERLON_W + MERLON_GAP
MERLON_H = 20                   # merlon height (5 logical)
WALKWAY = 16                    # walkway band (4 logical)
SHADOW_H = 14                   # grass strip + cast shadow below a wall's base
SIDE_W = 136                    # side wall strip width (34 logical, ~70%)
SIDE_WALKWAY = 16               # side wall walkway band on the inner edge
SIDE_PROJ = 16                  # side wall merlon projection into the field
SIDE_TOOTH = 20                 # side wall merlon tooth height
SIDE_PERIOD = 34                # side wall crenellation period (4 per cell)
TOWER_RAISE = 8                 # tower raised platform (merlons this much taller)
TOWER_RING = 14                 # tower inner shadow band below the parapet

GOLD = (246, 200, 70)
GOLD_D = (58, 42, 16)
SHADOW = (20, 18, 14)

# Cursor ring: outer radius and stroke thickness in logical pixels, shrinking
# over the four frames to the measured boxes in tools/asset_layout.json. The
# stroke is ~40% of the original and the dark outline is 1 logical pixel.
CURSOR = [(15.5, 1.4), (12.5, 1.4), (8.5, 1.2), (2.5, 0.8)]


def tile_size():
    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    return 48 * scale, 34 * scale, scale


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "combat")


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
    for name, (prompt, *_rest) in OBSTACLES.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in seeds:
            render(args, f"{prompt}, {STYLE}", NEG, GEN_SIZE, seed, True,
                   os.path.join(out, f"{name}_{seed}.png"))
    if not only or "castle_spike" in only:
        print("castle_spike", flush=True)
        for seed in seeds:
            render(args, f"{SPIKE[0]}, {STYLE}", NEG, GEN_SIZE, seed, True,
                   os.path.join(out, f"castle_spike_{seed}.png"))
    if not only or "wall_texture" in only:
        print("wall_texture", flush=True)
        for seed in seeds:
            render(args, WALL_PROMPT, WALL_NEG, GEN_SIZE, seed, False,
                   os.path.join(out, f"wall_texture_{seed}.png"))
    contact_sheet(out)
    return 0


# ---- compose ------------------------------------------------------------

def blank(W, H):
    return bytearray(W * H * 4)


def crop(w, h, px, x0, y0, cw, ch):
    out = bytearray(cw * ch * 4)
    for y in range(ch):
        s = ((y0 + y) * w + x0) * 4
        out[y * cw * 4:(y + 1) * cw * 4] = px[s:s + cw * 4]
    return bytes(out)


def make_seamless(w, h, px):
    """Blend in a copy rolled by half the tile so it wraps (top->bottom and
    left->right), which is what lets a small stone tile repeat without seams."""
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


def stone_tile():
    """The wall texture, cropped to its aspect, zoomed in so the stones read
    ~1.5x larger, scaled to the tile and made seamless. One shared tile keeps
    every wall piece in the same masonry."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"wall_texture_{PICKS['wall_texture']}.png"))
    if w * STONE_TH > h * STONE_TW:
        cw, ch = h * STONE_TW // STONE_TH, h
    else:
        cw, ch = w, w * STONE_TH // STONE_TW
    zw, zh = max(1, round(cw / STONE_ZOOM)), max(1, round(ch / STONE_ZOOM))
    px = crop(w, h, px, (w - cw) // 2 + (cw - zw) // 2,
              (h - ch) // 2 + (ch - zh) // 2, zw, zh)
    _, _, px = gen_ai.box_downscale(zw, zh, px, STONE_TW, STONE_TH)
    return STONE_TW, STONE_TH, make_seamless(STONE_TW, STONE_TH, px)


def stone_fill(canvas, W, H, tile, gx0, gy0, x0, y0, w, h):
    """Tile the stone over the rect, sampling in FIELD coordinates (the piece's
    cell origin `gx0,gy0`) so adjacent pieces continue the same stones across
    their shared edges."""
    tw, th = tile[0], tile[1]
    px = tile[2]
    for y in range(y0, min(H, y0 + h)):
        ty = (gy0 + y) % th
        for x in range(x0, min(W, x0 + w)):
            tx = (gx0 + x) % tw
            s = (ty * tw + tx) * 4
            d = (y * W + x) * 4
            canvas[d:d + 4] = px[s:s + 4]


def merlons(canvas, W, H, tile, gx0, gy0, x0, x1, y0, y1):
    """Crenel teeth across [x0,x1) at [y0,y1), aligned to the field x phase so
    a neighbour's teeth continue seamlessly."""
    for x in range(x0, min(W, x1)):
        if (gx0 + x) % MERLON_PERIOD < MERLON_W:
            stone_fill(canvas, W, H, tile, gx0, gy0, x, y0, 1, y1 - y0)


def shade(canvas, W, H, x0, y0, w, h, k):
    """Multiply the RGB of every opaque pixel in the rect by k."""
    for y in range(y0, min(H, y0 + h)):
        for x in range(x0, min(W, x0 + w)):
            o = (y * W + x) * 4
            if canvas[o + 3] < 128:
                continue
            for c in range(3):
                canvas[o + c] = max(0, min(255, round(canvas[o + c] * k)))


def light_topleft(canvas, W, H, strength=0.2):
    """Brighten toward the upper left, darken toward the lower right."""
    for y in range(H):
        for x in range(W):
            o = (y * W + x) * 4
            if canvas[o + 3] < 128:
                continue
            t = (x + y) / (W + H)
            k = 1 + strength * (0.5 - t)
            for c in range(3):
                canvas[o + c] = max(0, min(255, round(canvas[o + c] * k)))


def outline(canvas, W, H, borders, k=0.42, t=2):
    """Darken the stone's silhouette: pixels that border transparent grass, or a
    grass-facing cell edge named in `borders` (N/S/E/W). Edges that continue into
    another wall piece, or run along the field's outer border, are left clean."""
    opaque = bytearray(W * H)
    for i in range(W * H):
        opaque[i] = 1 if canvas[i * 4 + 3] >= 128 else 0
    mark = bytearray(W * H)
    for y in range(H):
        for x in range(W):
            i = y * W + x
            if not opaque[i]:
                continue
            n = (y == 0 and 'N' in borders) or (y > 0 and not opaque[i - W])
            s = (y == H - 1 and 'S' in borders) or (y < H - 1 and not opaque[i + W])
            w = (x == 0 and 'W' in borders) or (x > 0 and not opaque[i - 1])
            e = (x == W - 1 and 'E' in borders) or (x < W - 1 and not opaque[i + 1])
            if n or s or w or e:
                mark[i] = 1
    for _ in range(t - 1):
        grow = bytearray(W * H)
        for y in range(H):
            for x in range(W):
                i = y * W + x
                if not opaque[i] or mark[i]:
                    continue
                if ((x > 0 and mark[i - 1]) or (x < W - 1 and mark[i + 1])
                        or (y > 0 and mark[i - W]) or (y < H - 1 and mark[i + W])):
                    grow[i] = 1
        for i in range(W * H):
            mark[i] = mark[i] or grow[i]
    for i in range(W * H):
        if mark[i]:
            o = i * 4
            for c in range(3):
                canvas[o + c] = round(canvas[o + c] * k)


def _shadow_px(canvas, W, H, x, y, a):
    """Set a soft shadow on a grass (transparent) pixel, never on the stone."""
    o = (y * W + x) * 4
    if canvas[o + 3] >= 128:
        return
    na = round(a * 255)
    if na <= canvas[o + 3]:
        return
    canvas[o:o + 3] = bytes(SHADOW)
    canvas[o + 3] = na


def cast_shadow(canvas, W, H, edge, start, alpha=0.35, fade=12):
    """A soft semi-transparent shadow cast onto the grass, fading from the
    field-facing `edge` at `start` (the first grass pixel)."""
    for d in range(fade):
        a = alpha * (1 - d / fade)
        if edge == 'E':
            x = start + d
            if x >= W:
                break
            for y in range(H):
                _shadow_px(canvas, W, H, x, y, a)
        elif edge == 'W':
            x = start - d
            if x < 0:
                break
            for y in range(H):
                _shadow_px(canvas, W, H, x, y, a)
        else:  # 'S'
            y = start + d
            if y >= H:
                break
            for x in range(W):
                _shadow_px(canvas, W, H, x, y, a)


def front_wall(W, H, tile, gx0, gy0, jamb):
    """A full-width curtain wall: merlons on top, a lighter walkway band, a
    darker face below lit from the upper left, and a soft shadow on the grass at
    the base. `jamb` ('E' or 'W') is the gate-side edge the outline runs along."""
    canvas = blank(W, H)
    wall_top = FRONT_TOP + MERLON_H
    base = H - SHADOW_H
    stone_fill(canvas, W, H, tile, gx0, gy0, 0, wall_top + WALKWAY, W,
               base - wall_top - WALKWAY)                             # face
    stone_fill(canvas, W, H, tile, gx0, gy0, 0, wall_top, W, WALKWAY)  # walkway
    merlons(canvas, W, H, tile, gx0, gy0, 0, W, FRONT_TOP, wall_top)
    shade(canvas, W, H, 0, wall_top + WALKWAY, W, base - wall_top - WALKWAY, 0.84)
    shade(canvas, W, H, 0, wall_top, W, WALKWAY, 1.16)
    light_topleft(canvas, W, H, 0.14)
    outline(canvas, W, H, {jamb})
    cast_shadow(canvas, W, H, 'S', base)
    return bytes(canvas)


def side_wall(W, H, tile, gx0, gy0, flip):
    """A vertical strip with a lighter walkway on its inner edge and crenel
    teeth projecting into the courtyard, lit from the upper left. `flip` mirrors
    it for the right column."""
    canvas = blank(W, H)
    face_w = SIDE_W - SIDE_WALKWAY
    if flip:
        face_x = W - SIDE_W + SIDE_WALKWAY      # [72, 192)
        walk_x = W - SIDE_W                     # [56, 72)
        teeth_x = W - SIDE_W - SIDE_PROJ        # [40, 56)
    else:
        face_x = 0                              # [0, 120)
        walk_x = SIDE_W - SIDE_WALKWAY          # [120, 136)
        teeth_x = SIDE_W                        # [136, 152)
    stone_fill(canvas, W, H, tile, gx0, gy0, face_x, 0, face_w, H)        # face
    stone_fill(canvas, W, H, tile, gx0, gy0, walk_x, 0, SIDE_WALKWAY, H)  # walkway
    for y in range(0, H, SIDE_PERIOD):
        stone_fill(canvas, W, H, tile, gx0, gy0, teeth_x, y, SIDE_PROJ, SIDE_TOOTH)
    shade(canvas, W, H, face_x, 0, face_w, H, 0.84)
    shade(canvas, W, H, walk_x, 0, SIDE_WALKWAY, H, 1.14)
    light_topleft(canvas, W, H, 0.14)
    outline(canvas, W, H, set())
    if flip:
        cast_shadow(canvas, W, H, 'W', teeth_x - 1)
    else:
        cast_shadow(canvas, W, H, 'E', teeth_x + SIDE_PROJ)
    return bytes(canvas)


def tower(W, H, tile, gx0, gy0):
    """A square corner tower: a raised crenellated platform, a darker inner
    shadow band below it, a lighter tone, and a soft shadow at the base."""
    canvas = blank(W, H)
    mh = MERLON_H + TOWER_RAISE
    plat_bot = mh + WALKWAY
    ring_bot = plat_bot + TOWER_RING
    base = H - SHADOW_H
    stone_fill(canvas, W, H, tile, gx0, gy0, 0, ring_bot, W, base - ring_bot)  # face
    stone_fill(canvas, W, H, tile, gx0, gy0, 0, mh, W, WALKWAY)                # platform
    stone_fill(canvas, W, H, tile, gx0, gy0, 0, plat_bot, W, TOWER_RING)       # shadow band
    merlons(canvas, W, H, tile, gx0, gy0, 0, W, 0, mh)
    shade(canvas, W, H, 0, 0, W, mh, 1.06)                                     # merlons
    shade(canvas, W, H, 0, mh, W, WALKWAY, 1.18)                               # platform
    shade(canvas, W, H, 0, plat_bot, W, TOWER_RING, 0.70)                      # inner shadow
    shade(canvas, W, H, 0, ring_bot, W, base - ring_bot, 0.88)                 # face
    light_topleft(canvas, W, H, 0.12)
    outline(canvas, W, H, set())
    cast_shadow(canvas, W, H, 'S', base)
    return bytes(canvas)


def soft_shadow(canvas, W, H, cx, cy, rx, ry, alpha=90):
    for y in range(max(0, cy - ry), min(H, cy + ry + 1)):
        for x in range(max(0, cx - rx), min(W, cx + rx + 1)):
            d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
            if d >= 1:
                continue
            o = (y * W + x) * 4
            if canvas[o + 3] >= alpha:
                continue
            a = round(alpha * (1 - d))
            canvas[o + 3] = a
            canvas[o:o + 3] = bytes(SHADOW)


def place_subject(W, H, name, box, baseline, foot):
    """Matter a subject into its box, feet on the baseline, foot centre on
    `foot`, with a soft shadow under it."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{PICKS[name]}.png"))
    sw, sh, spx = subject(w, h, px)
    bw, bh = int(W * box[0]), int(H * box[1])
    k = min(bw / sw, bh / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    canvas = blank(W, H)
    oy = int(H * baseline) - nh
    ox = int(W * foot) - nw // 2
    ox, oy = max(0, min(W - nw, ox)), max(0, min(H - nh, oy))
    soft_shadow(canvas, W, H, ox + nw // 2, int(H * baseline), max(4, nw // 2),
                max(3, nh // 8))
    over_transparent(W, canvas, nw, nh, scaled, ox, oy)
    return bytes(canvas)


def place_centered(W, H, name, box):
    """A matted subject scaled into `box` and centred in the cell."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{PICKS[name]}.png"))
    sw, sh, spx = subject(w, h, px)
    bw, bh = int(W * box[0]), int(H * box[1])
    k = min(bw / sw, bh / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    canvas = blank(W, H)
    over_transparent(W, canvas, nw, nh, scaled, (W - nw) // 2, (H - nh) // 2)
    return bytes(canvas)


def aa_band(d, r0, r1):
    if r1 <= r0:
        return 0.0
    return max(0.0, min(1.0, min(d - r0, r1 - d) + 0.5))


def cursor(W, H, scale, outer, thick):
    """An anti-aliased golden ring with a dark outline, centred in the cell."""
    canvas = blank(W, H)
    cx, cy = 24.5 * scale, 16.5 * scale
    r_out, r_in = outer * scale, max(0.0, (outer - thick) * scale)
    ow = 1.0 * scale
    for y in range(H):
        for x in range(W):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            ga = aa_band(d, r_in, r_out)
            da = max(aa_band(d, r_out, r_out + ow), aa_band(d, r_in - ow, r_in))
            a = ga + da * (1 - ga)
            if a <= 0:
                continue
            r = (GOLD[0] * ga + GOLD_D[0] * da * (1 - ga)) / a
            g = (GOLD[1] * ga + GOLD_D[1] * da * (1 - ga)) / a
            b = (GOLD[2] * ga + GOLD_D[2] * da * (1 - ga)) / a
            o = (y * W + x) * 4
            canvas[o:o + 3] = bytes((round(r), round(g), round(b)))
            canvas[o + 3] = round(a * 255)
    return bytes(canvas)


def cmd_compose(cli):
    W, H, scale = tile_size()
    tile = stone_tile()
    written = {}

    for name, (_, box, baseline, foot) in OBSTACLES.items():
        written[name] = place_subject(W, H, name, box, baseline, foot)
    written["castle_spike"] = place_centered(W, H, "castle_spike", SPIKE[1])

    # Each wall piece is drawn at its CASTLE_OMAP cell, so the stone texture is
    # sampled in field coordinates (cell x*W + px, y*H + py) and neighbours
    # continue the same masonry across their shared edges.
    gx = lambda cx: cx * W
    gy = lambda cy: cy * H
    wall_left = side_wall(W, H, tile, gx(0), gy(0), False)
    tower_left = tower(W, H, tile, gx(0), gy(4))
    front_left = front_wall(W, H, tile, gx(1), gy(4), 'E')
    written["castle_wall_01"] = tower_left                        # code 5, bottom-left tower
    written["castle_wall_02"] = tower(W, H, tile, gx(5), gy(4))   # code 6, bottom-right
    written["castle_wall_03"] = front_left                        # code 7, front left of gate
    written["castle_wall_04"] = wall_left                         # code 8, left column
    written["castle_wall_05"] = side_wall(W, H, tile, gx(5), gy(0), True)  # code 9, right
    written["castle_wall_06"] = front_wall(W, H, tile, gx(4), gy(4), 'W')  # code 10, right

    for i, (outer, thick) in enumerate(CURSOR, 1):
        written[f"cursor_{i:02d}"] = cursor(W, H, scale, outer, thick)

    combat = os.path.join(PACK, "art", "combat")
    ai_rels, cur_rels = [], []
    for name, px in written.items():
        rel = f"art/combat/{name}.png"
        write_png(os.path.join(PACK, rel), W, H, px)
        if name.startswith("cursor_"):
            cur_rels.append(rel)
        else:
            ai_rels.append(rel)
    register(ai_rels, AI_AUTHOR, method="ai-generated")
    register(cur_rels, CURSOR_AUTHOR, method="procedural")
    print(f"wrote {len(written)} combat pieces at {W}x{H} into {combat}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=3)
    r.add_argument("--first-seed", type=int, default=DEFAULT_SEED)
    r.add_argument("--only", default="",
                   help="comma list: obstacle_01, obstacle_02, obstacle_03, "
                        "castle_spike, wall_texture")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
