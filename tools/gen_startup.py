"""Generate the raw AI pieces for the startup screens on ComfyUI.

The class picker is one scene with four heroes standing in it, which no single
prompt draws reliably, so it is built from parts: a full-body figure per class
(matted), a portrait bust per class, and an empty hall to stand them in.
`compose_startup.py` then places the parts into the pack's slots; this script
only renders them, several seeds each, so the best can be picked.

Writes to `game-assets/ai-out/<run>/startup/` (local, ignored) plus a contact
sheet `sheet.png` there. Reuses `gen_ai.py`'s ComfyUI graph, transport and
rembg matte, so it needs the same tunnel (`specs/generation-mac-instructions.md`).

Usage:
    python3 tools/gen_startup.py --run startup1 [--seeds 3] [--only figure,hall]
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

STYLE = ("painterly digital painting, classic high fantasy strategy game art, "
         "rich saturated colours, warm dramatic light, crisp detail")

# One look per class, kept consistent between the figure and the portrait.
CLASSES = {
    "knight": "a noble knight in polished steel plate armour with a red plume on "
              "his helmet, red surcoat, kite shield and longsword",
    "paladin": "a paladin in gleaming gold-trimmed white armour with a winged helmet, "
               "holding a warhammer, a holy symbol on his tabard",
    "sorceress": "a sorceress in flowing deep purple robes with silver embroidery, "
                 "long dark hair, holding a crystal-topped staff",
    "barbarian": "a muscular barbarian warrior with a fur cloak and leather harness, "
                 "braided red beard, holding a great axe",
}

FIGURE_NEG = gen_ai.NEG + (", cropped feet, cut off, close-up, portrait, bust, "
                           "background scenery, landscape, room, floor")
PORTRAIT_NEG = ("text, watermark, signature, frame, border, multiple people, "
                "blurry, low quality, deformed, full body")
HALL_NEG = ("text, watermark, signature, people, person, figure, character, crowd, "
            "blurry, low quality, deformed")

# Generation sizes, multiples of 64 near one megapixel.
FIGURE_SIZE = (832, 1216)     # tall: a whole standing figure
PORTRAIT_SIZE = (960, 1024)   # the 96x102 slot's aspect
HALL_SIZE = (1216, 768)       # the 288x184 picker's aspect


class Args:
    """The attributes gen_ai's helpers read from its argparse namespace."""

    def __init__(self, host, ckpt, rembg):
        self.host = host
        self.ckpt = ckpt
        self.matte = "cli"
        self.rembg = rembg


def render(args, positive, negative, size, seed, matte, out_path):
    w, h = size
    graph = gen_ai.build_graph(args.ckpt, positive, negative, seed, w, h, None)
    rgba = gen_ai.run_graph_matted(args, graph, matte)
    write_png(out_path, w, h, rgba)
    print(f"  wrote {os.path.relpath(out_path, gen_ai.ROOT)}", flush=True)


def contact_sheet(folder):
    """Every PNG in `folder`, scaled to 256 high, on a grey grid."""
    files = sorted(f for f in os.listdir(folder)
                   if f.endswith(".png") and not f.startswith(("_", "sheet")))
    if not files:
        return
    cell_h, cols = 256, 6
    images = []
    for name in files:
        w, h, px = read_png(os.path.join(folder, name))
        nw = max(1, w * cell_h // h)
        images.append(gen_ai.box_downscale(w, h, px, nw, cell_h))
    cell_w = max(im[0] for im in images)
    rows = (len(images) + cols - 1) // cols
    sw, sh = cell_w * cols, cell_h * rows
    sheet = bytearray(bytes((90, 90, 96, 255)) * (sw * sh))
    for i, (w, h, px) in enumerate(images):
        ox, oy = (i % cols) * cell_w, (i // cols) * cell_h
        for y in range(h):
            for x in range(w):
                o = (y * w + x) * 4
                a = px[o + 3] / 255
                d = ((oy + y) * sw + ox + x) * 4
                for c in range(3):
                    sheet[d + c] = round(px[o + c] * a + sheet[d + c] * (1 - a))
    write_png(os.path.join(folder, "sheet.png"), sw, sh, bytes(sheet))
    print("contact sheet: " + os.path.relpath(os.path.join(folder, "sheet.png"), gen_ai.ROOT))
    print("order: " + ", ".join(files))


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--run", required=True, help="output run name under game-assets/ai-out")
    ap.add_argument("--seeds", type=int, default=3, help="variants per piece")
    ap.add_argument("--first-seed", type=int, default=101)
    ap.add_argument("--only", default="figure,portrait,hall",
                    help="comma list of: figure, portrait, hall")
    ap.add_argument("--classes", default=",".join(CLASSES))
    ap.add_argument("--host", default="http://127.0.0.1:8188")
    ap.add_argument("--ckpt", default=gen_ai.CKPT)
    ap.add_argument("--rembg", default="rembg")
    cli = ap.parse_args()

    out = os.path.join(gen_ai.ROOT, "game-assets", "ai-out", cli.run, "startup")
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out  # gen_ai keeps its scratch files (_raw, _cut) here
    rembg = gen_ai.resolve_rembg(cli.rembg)
    if rembg is None and "figure" in cli.only:
        sys.exit("rembg not found; see gen_ai.py --rembg")
    args = Args(cli.host, cli.ckpt, rembg)
    gen_ai.clear_queue(args.host)

    pieces = cli.only.split(",")
    seeds = range(cli.first_seed, cli.first_seed + cli.seeds)
    for cls in cli.classes.split(","):
        look = CLASSES[cls]
        if "figure" in pieces:
            print(f"figure {cls}", flush=True)
            for seed in seeds:
                render(args,
                       f"full body standing pose of {look}, facing the viewer, three-quarter "
                       f"view, heroic stance, whole figure from head to feet, single character, "
                       f"plain flat light grey background, {STYLE}",
                       FIGURE_NEG, FIGURE_SIZE, seed, True,
                       os.path.join(out, f"figure_{cls}_{seed}.png"))
        if "portrait" in pieces:
            print(f"portrait {cls}", flush=True)
            for seed in seeds:
                render(args,
                       f"head and shoulders portrait of {look}, looking at the viewer, "
                       f"dark stone castle wall behind, {STYLE}",
                       PORTRAIT_NEG, PORTRAIT_SIZE, seed, False,
                       os.path.join(out, f"portrait_{cls}_{seed}.png"))
    if "hall" in pieces:
        print("hall", flush=True)
        for seed in seeds:
            render(args,
                   "the empty great hall of a royal castle, stone pillars, red banners with a "
                   "gold scepter emblem, warm torchlight, a long red carpet, an empty floor in "
                   f"the foreground, wide view, {STYLE}",
                   HALL_NEG, HALL_SIZE, seed, False,
                   os.path.join(out, f"hall_{seed}.png"))
    contact_sheet(out)
    return 0


if __name__ == "__main__":
    sys.exit(main())
