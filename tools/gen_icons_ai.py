"""AI icons: the eight artifacts and the four continent maps.

    python3 tools/gen_icons_ai.py render --run icons1 [--seeds 2] [--only sword,map_dunemark]
    python3 tools/gen_icons_ai.py compose

`render` draws each object on ComfyUI (through the tunnel), matted with rembg,
into local, ignored `game-assets/ai-out/<run>/icons/`, with a contact sheet.

`compose` needs no tunnel. An icon fills a 48x34 cell (a puzzle cell, an
inventory slot on the character screen) over whatever the view paints behind
it, so it stays transparent: the picked object is scaled into the cell,
centred, at the pack's art size. Artifacts go to their `icon` path; the maps
to `art/ui/inventory_zone_<zone>.png`, the names `build_pack.py` declares.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from gen_hero_ai import over_transparent  # noqa: E402
from gen_objects_ai import subject  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "icons1"
PICKS = {"sword": 202, "map_highmoor": 301, "shield": 102, "crown": 102, "map_greenreach": 102, "map_isleward": 102,
         "map_dunemark": 102}
DEFAULT_SEED = 101
AUTHOR = "AI-generated (tools/gen_icons_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_icons_ai.py"

STYLE = ("single isolated object, game inventory icon, painterly classic high fantasy "
         "art, rich saturated colours, crisp detail, soft light, plain flat light grey "
         "background")
NEG = ("text, letters, words, watermark, signature, border, frame, hand, person, table, "
       "scenery, multiple objects, cropped, blurry, low quality, deformed")

OBJECTS = {
    "sword": "one single ornate longsword with a long straight steel blade, a gold "
             "crossguard and a glowing blue gem in the pommel, lying diagonally, nothing "
             "else",
    "shield": "an ancient round shield of bronze and steel with a golden lion emblem",
    "crown": "a slender golden circlet set with a single red ruby",
    "articles": "a royal charter, a rolled parchment scroll with a red wax seal and a "
                "red ribbon",
    "amulet": "a silver amulet on a fine chain with a glowing star-shaped sapphire",
    "ring": "a gold ring with a large glowing violet gem and runes engraved on the band",
    "book": "an old closed leather-bound tome with a silver clasp and faintly glowing "
            "runes on the cover",
    "anchor": "an ornate iron ship's anchor with a coil of rope",
    "map_highmoor": "an unrolled old parchment map drawn with green rolling hills, grey "
                    "mountains and lakes, a red wax seal in the corner",
    "map_greenreach": "an unrolled old parchment map drawn with dense green forests and "
                      "rivers, a green wax seal in the corner",
    "map_isleward": "an unrolled old parchment map drawn with scattered islands in a blue "
                    "sea, a blue wax seal in the corner",
    "map_dunemark": "an unrolled old parchment map drawn with golden desert dunes and an "
                    "oasis, a gold wax seal in the corner",
}
BOX = (0.9, 0.9)


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "icons")


def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, gen_ai.resolve_rembg("rembg"))
    if args.rembg is None:
        sys.exit("rembg not found; see gen_ai.py --rembg")
    gen_ai.clear_queue(args.host)
    only = set(filter(None, cli.only.split(",")))
    for name, prompt in OBJECTS.items():
        if only and name not in only:
            continue
        print(name, flush=True)
        for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
            render(args, f"{prompt}, {STYLE}", NEG, (1024, 1024), seed, True,
                   os.path.join(out, f"{name}_{seed}.png"))
    contact_sheet(out)
    return 0


def icon(name, tw, th):
    seed = PICKS.get(name, DEFAULT_SEED)
    w, h, px = read_png(os.path.join(out_dir(RUN), f"{name}_{seed}.png"))
    sw, sh, spx = subject(w, h, px)
    k = min(tw * BOX[0] / sw, th * BOX[1] / sh)
    nw, nh = max(1, round(sw * k)), max(1, round(sh * k))
    _, _, scaled = gen_ai.box_downscale(sw, sh, spx, nw, nh)
    canvas = bytearray(tw * th * 4)
    over_transparent(tw, canvas, nw, nh, scaled, (tw - nw) // 2, (th - nh) // 2)
    return bytes(canvas)


def cmd_compose(cli):
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    scale = pack["render"].get("scale", 1)
    tw, th = 48 * scale, 34 * scale
    targets = {a["id"]: a["icon"] for a in pack["artifacts"]}
    targets.update({f"map_{z['id']}": f"art/ui/inventory_zone_{z['id']}.png"
                    for z in pack["zones"]})
    for name, rel in targets.items():
        write_png(os.path.join(PACK, rel), tw, th, icon(name, tw, th))
    register(sorted(targets.values()), AUTHOR, method="ai-generated")
    print(f"wrote {len(targets)} icons at {tw}x{th}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=2)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="", help="comma list of icon names")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
