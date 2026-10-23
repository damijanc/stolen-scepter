"""AI map objects: castle, town, chests, dwellings, sign, camp and bridges.

Two steps, like `gen_terrain_ai.py`:

    python3 tools/gen_objects_ai.py render --run objects1 [--seeds 3] [--only town,chest]
    python3 tools/gen_objects_ai.py compose

`render` asks ComfyUI (through the tunnel) for each object, several seeds
each, into local, ignored `game-assets/ai-out/<run>/objects/`, with a contact
sheet. Objects are matted with rembg; the bridge deck is a plain texture.

`compose` needs no tunnel. The engine draws an object tile INSTEAD of the
terrain under it, so every tile is opaque: the picked object is scaled into
its box and set on the pack's own grass tile (`art/tiles/grass.png`, written
by `gen_terrain_ai.py`) with a soft shadow under it. The castle is one wide
picture over a 3x2 grass canvas, sliced into its six pieces so they align
(top row tl, br, tr; bottom row ml, gate, mr -- the names are the engine's).
A bridge lays the deck texture across the water tile from edge to edge, so
neighbouring bridge tiles join.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from compose_startup import crop, over  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "objects1"
PICKS = {
    "castle": 301, "town": 101, "chest": 103, "artifact_chest": 103, "artifact_ring": 102,
    "sign": 101, "wandering_army": 103, "dwelling_plains": 103, "dwelling_forest": 101,
    "dwelling_hills": 202, "dwelling_dungeon": 103, "deck": 101,
}
AUTHOR = "AI-generated (tools/gen_objects_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_objects_ai.py"

STYLE = ("three-quarter top-down view, single isolated object, game map icon, painterly "
         "classic high fantasy strategy game art, rich saturated colours, crisp detail, "
         "plain flat light grey background")
NEG = ("text, letters, words, watermark, signature, border, frame, people, person, "
       "landscape, scenery, ground plane, horizon, sky, blurry, low quality, deformed")

# name -> (prompt, generation size, box: fraction of the tile's width and height
# the object may fill). The castle's box is of the whole 3x2 canvas.
OBJECTS = {
    "castle": ("a grand fantasy stone castle with round towers and battlements, a long "
               "front wall facing the viewer with a large arched wooden gate in its centre, "
               "symmetrical, the whole castle in frame with empty space above its "
               "tallest tower", (1024, 1024), (0.98, 0.9)),
    "town": ("a small medieval village of four timber-framed houses with red tiled roofs "
             "clustered around a stone well", (1024, 1024), (0.92, 0.92)),
    "chest": ("a closed wooden treasure chest with iron bands and gold trim",
              (1024, 1024), (0.55, 0.62)),
    "artifact_chest": ("an ornate golden treasure chest studded with gems, glowing with a "
                       "magical blue light", (1024, 1024), (0.6, 0.68)),
    "artifact_ring": ("a single large golden ring with a glowing blue gem, lying flat",
                      (1024, 1024), (0.5, 0.6)),
    "sign": ("a wooden signpost, one blank wooden board nailed to a post",
             (1024, 1024), (0.5, 0.86)),
    "wandering_army": ("a small military camp, a red canvas tent and a war banner on a pole",
                       (1024, 1024), (0.8, 0.86)),
    "dwelling_plains": ("a small thatched farm cottage with a wooden fence",
                        (1024, 1024), (0.86, 0.9)),
    "dwelling_forest": ("a house built into a huge hollow tree stump with a round door and "
                        "small windows", (1024, 1024), (0.8, 0.92)),
    "dwelling_hills": ("a grassy rounded hill with brown rock outcrops and a large dark "
                       "cave entrance in its front, no water",
                       (1024, 1024), (0.9, 0.9)),
    "dwelling_dungeon": ("a dark stone dungeon entrance, a stone archway over stairs leading "
                         "down into the ground, two torches", (1024, 1024), (0.82, 0.9)),
}
DECK = ("seamless texture of weathered wooden bridge planks running side to side, flat "
        "orthographic top-down view, even lighting, painterly fantasy game art")
DECK_NEG = "text, watermark, nails pattern, water, grass, frame, border, blurry, low quality"

FOOT = 0.94        # where an object stands: the fraction of the tile's height
SHADOW = 0.35      # the shadow's darkness under an object
DECK_SPAN = 0.56   # the deck's width across the bridge, as a fraction of the tile
RAIL = (52, 34, 20)


def tile_size():
    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    return 48 * scale, 34 * scale


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "objects")


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
    for name, (prompt, size, _) in OBJECTS.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in seeds:
            render(args, f"{prompt}, {STYLE}", NEG, size, seed, True,
                   os.path.join(out, f"{name}_{seed}.png"))
    if not only or "deck" in only:
        print("deck", flush=True)
        for seed in seeds:
            render(args, DECK, DECK_NEG, (1024, 1024), seed, False,
                   os.path.join(out, f"deck_{seed}.png"))
    contact_sheet(out)
    return 0


def subject(w, h, px):
    """The bounding box of every sizeable matted blob, so a village of separate
    houses stays whole while specks of matte noise are dropped."""
    comps = gen_ai.alpha_components(w, h, px)
    keep = [c for c in comps if len(c) >= len(comps[0]) // 50]
    xs = [j % w for c in keep for j in c]
    ys = [j // w for c in keep for j in c]
    x0, y0 = min(xs), min(ys)
    cw, ch = max(xs) - x0 + 1, max(ys) - y0 + 1
    out = bytearray(crop(w, h, px, x0, y0, cw, ch))
    kept = set(j for c in keep for j in c)
    for y in range(ch):
        for x in range(cw):
            if (y0 + y) * w + x0 + x not in kept:
                out[(y * cw + x) * 4 + 3] = 0
    return cw, ch, bytes(out)


def tiled(w, h, px, cols, rows):
    """`px` repeated cols x rows."""
    W = w * cols
    out = bytearray(W * h * rows * 4)
    for ty in range(rows):
        for y in range(h):
            row = px[y * w * 4:(y + 1) * w * 4]
            for tx in range(cols):
                d = ((ty * h + y) * W + tx * w) * 4
                out[d:d + w * 4] = row
    return bytes(out)


def shadow(W, H, canvas, cx, cy, rx, ry):
    """Darken a soft ellipse on the opaque canvas."""
    for y in range(max(0, cy - ry), min(H, cy + ry + 1)):
        for x in range(max(0, cx - rx), min(W, cx + rx + 1)):
            d = ((x - cx) / rx) ** 2 + ((y - cy) / ry) ** 2
            if d < 1:
                k = 1 - SHADOW * (1 - d)
                o = (y * W + x) * 4
                for c in range(3):
                    canvas[o + c] = round(canvas[o + c] * k)


def place(name, W, H, canvas, box):
    """Fit the picked object into `box` (fractions of W x H), centred, its feet on
    FOOT, with a shadow under it."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{PICKS[name]}.png"))
    sw, sh, spx = subject(w, h, px)
    bw, bh = int(W * box[0]), int(H * box[1])
    k = min(bw / sw, bh / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    foot = int(H * FOOT)
    ox, oy = (W - nw) // 2, foot - nh
    assert oy >= 0, f"{name}: box taller than FOOT, the top would be cut"
    shadow(W, H, canvas, W // 2, foot - nh // 12, nw * 9 // 20, max(2, nh // 9))
    over(W, canvas, nw, nh, scaled, ox, oy)


def deck_strip(length, span, vertical):
    """The deck texture scaled to a `length` x `span` strip, planks running
    across it, with dark rails along both long edges."""
    w, h, px = read_png(os.path.join(out_dir(RUN), f"deck_{PICKS['deck']}.png"))
    side = min(w, h)
    sq = crop(w, h, px, (w - side) // 2, (h - side) // 2, side, side)
    # Planks run side to side in the render: a horizontal bridge wants them
    # across its width, i.e. vertical, so it takes the texture rotated.
    if not vertical:
        rot = bytearray(len(sq))
        for y in range(side):
            for x in range(side):
                s = (y * side + x) * 4
                d = (x * side + (side - 1 - y)) * 4
                rot[d:d + 4] = sq[s:s + 4]
        sq = bytes(rot)
    sw, sh = (span, length) if vertical else (length, span)
    _, _, strip = gen_ai.box_downscale(side, side, sq, sw, sh)
    strip = bytearray(strip)
    rail = max(2, span // 14)
    for y in range(sh):
        for x in range(sw):
            edge = x if vertical else y
            far = (sw if vertical else sh) - 1 - edge
            if min(edge, far) < rail:
                o = (y * sw + x) * 4
                strip[o:o + 3] = bytes(RAIL)
    return sw, sh, bytes(strip)


def bridge(tw, th, water, vertical):
    canvas = bytearray(water)
    if vertical:
        span = int(tw * DECK_SPAN)
        sw, sh, strip = deck_strip(th, span, True)
        over(tw, canvas, sw, sh, strip, (tw - sw) // 2, 0)
    else:
        span = int(th * DECK_SPAN)
        sw, sh, strip = deck_strip(tw, span, False)
        over(tw, canvas, sw, sh, strip, 0, (th - sh) // 2)
    return bytes(canvas)


def cmd_compose(cli):
    tw, th = tile_size()
    tiles = os.path.join(PACK, "art", "tiles")
    gw, gh, grass = read_png(os.path.join(tiles, "grass.png"))
    _, _, water = read_png(os.path.join(tiles, "water.png"))
    assert (gw, gh) == (tw, th), "regenerate the terrain first (gen_terrain_ai.py compose)"
    written = {}

    W, H = tw * 3, th * 2
    canvas = bytearray(tiled(tw, th, grass, 3, 2))
    place("castle", W, H, canvas, OBJECTS["castle"][2])
    for name, (cx, cy) in {"castle_tl": (0, 0), "castle_br": (1, 0), "castle_tr": (2, 0),
                           "castle_ml": (0, 1), "castle_gate": (1, 1),
                           "castle_mr": (2, 1)}.items():
        written[name] = crop(W, H, bytes(canvas), cx * tw, cy * th, tw, th)

    for name, (_, _, box) in OBJECTS.items():
        if name == "castle":
            continue
        canvas = bytearray(grass)
        place(name, tw, th, canvas, box)
        written[name] = bytes(canvas)

    written["bridge_h"] = bridge(tw, th, water, False)
    written["bridge_v"] = bridge(tw, th, water, True)

    for name, px in written.items():
        write_png(os.path.join(tiles, name + ".png"), tw, th, px)
    rels = sorted(f"art/tiles/{name}.png" for name in written)
    register(rels, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} object tiles at {tw}x{th}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=3)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="", help="comma list of object names, or deck")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
