"""AI portraits for the seventeen villains.

    python3 tools/gen_villains_ai.py render --run villains1 [--seeds 2] [--only ottoline,vash]
    python3 tools/gen_villains_ai.py compose

`render` draws a head-and-shoulders portrait per villain on ComfyUI (through
the tunnel) into local, ignored `game-assets/ai-out/<run>/villains/`, with a
contact sheet. Each look follows the villain's `features` in
`packs/stolen-scepter/data/text.json`.

`compose` needs no tunnel. A face fills a 48x34 slot (the contract panel,
the sidebar's contract panel and a puzzle cell), so each pick is cropped to
that aspect and scaled to the pack's art size. The four animation frames
(`<id>_00..03.png`, the names the catalog derives from `portrait`) are that
one picture with a slow breathing zoom; four separate renders would not match.
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from compose_startup import crop  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

RUN = "villains1"
# The kept seed per villain; missing ones use DEFAULT_SEED.
PICKS = {"isolde": 102, "elowen": 102, "nhalia": 102, "thrumm": 202, "sorvane": 202}
DEFAULT_SEED = 101
AUTHOR = "AI-generated (tools/gen_villains_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_villains_ai.py"

STYLE = ("painterly digital painting, classic high fantasy strategy game art, rich "
         "saturated colours, dramatic light, crisp detail, dark moody background")
NEG = ("text, letters, words, watermark, signature, frame, border, multiple people, "
       "full body, cropped head, blurry, low quality, deformed")
SIZE = (1216, 832)  # near the 48x34 slot's aspect

LOOKS = {
    "ottoline": "a sly middle-aged woman tax collector with spectacles, a fur-trimmed coat, "
                "ink-stained fingers and a heavy ledger chained to her belt",
    "jessa": "a grinning female bandit with short tousled hair under a hood, a leather "
             "jerkin, one gold tooth and a knife on each hip",
    "morwen": "a pale witch in dark green and black robes holding up an eerie glowing "
              "lantern, marsh mist around her",
    "harrek": "a rugged scarred huntsman wearing a wolfskin cloak with the wolf's head as a "
              "hood, a yellowed bone whistle on a cord",
    "edric": "a smug treacherous knight in polished plate armour, a crest scratched off his "
             "breastplate, short blond hair",
    "isolde": "a ghostly pale countess with chalk-white skin, white hair in an elegant "
              "updo and an old-fashioned black gown, faintly translucent",
    "torvald": "a stern old general with a grey beard and a scar, a battered officer's "
               "breastplate and a red sash",
    "elowen": "a wild druid woman with moss and leaves in her long hair, bright green eyes "
              "and a robe of woven vines",
    "grubnak": "an enormous fat ogre chieftain with tusks and a crude iron crown, a belt "
               "hung with stolen bronze church bells",
    "mordecai": "a gaunt necromancer in a wide-brimmed gravedigger's hat holding a shovel, "
                "a sickly green glow",
    "nhalia": "a sinister undead nun in a black and white habit with buttons of finger "
              "bones, hollow glowing eyes",
    "thrumm": "a huge bearded human giant man with a craggy weathered face and wild grey "
               "hair, a great curved dragon's tooth hanging on a cord around his neck, "
               "storm clouds behind him, no dragon, no monster",
    "vash": "a cold beautiful queen wearing a crown of sharp glass shards, an icy glare "
            "and a silver gown",
    "cassian": "a scheming nobleman with slicked dark hair, a black velvet doublet, black "
               "gloves and a gold signet ring",
    "valdris": "a charming demon lord in fine purple velvet with small horns, sharp teeth "
               "and a polite smile",
    "oren": "a dark rider in black armour and a desert cloak beside his horse whose eyes "
            "burn like coals",
    "sorvane": "an old gaunt human sorcerer with a grey beard and a hood, in ash-grey "
               "robes, holding up a golden royal scepter topped with a red ruby, a faint "
               "dragon-shaped shadow on the wall behind him, no horns",
}

# Each frame's zoom: the share of the picture's width it shows.
BREATH = [1.0, 0.985, 0.97, 0.985]
FRAMES = 4


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "villains")


def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, None)
    gen_ai.clear_queue(args.host)
    only = set(filter(None, cli.only.split(",")))
    for vid, look in LOOKS.items():
        if only and vid not in only:
            continue
        print(vid, flush=True)
        for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
            render(args,
                   f"head and shoulders portrait of {look}, a villain, looking at the "
                   f"viewer, face in the upper middle of the picture, {STYLE}",
                   NEG, SIZE, seed, False, os.path.join(out, f"{vid}_{seed}.png"))
    contact_sheet(out)
    return 0


def frame(w, h, px, tw, th, zoom):
    """Crop to the slot's aspect at `zoom` of the widest such crop, centred
    horizontally and anchored a little above centre (where the face is), then
    scale to tw x th."""
    if w * th > h * tw:
        cw, ch = h * tw // th, h
    else:
        cw, ch = w, w * th // tw
    cw, ch = int(cw * zoom), int(ch * zoom)
    x0 = (w - cw) // 2
    y0 = int((h - ch) * 0.35)
    return gen_ai.box_downscale(cw, ch, crop(w, h, px, x0, y0, cw, ch), tw, th)[2]


def cmd_compose(cli):
    with open(os.path.join(PACK, "game.json")) as fh:
        pack = json.load(fh)
    scale = pack["render"].get("scale", 1)
    tw, th = 48 * scale, 34 * scale
    written = []
    for villain in pack["villains"]:
        vid = villain["id"]
        seed = PICKS.get(vid, DEFAULT_SEED)
        w, h, px = read_png(os.path.join(out_dir(RUN), f"{vid}_{seed}.png"))
        stem = villain["portrait"][:-len("_00.png")]
        for i in range(FRAMES):
            rel = f"{stem}_{i:02d}.png"
            write_png(os.path.join(PACK, rel), tw, th, frame(w, h, px, tw, th, BREATH[i]))
            written.append(rel)
    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} villain frames at {tw}x{th}")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default=RUN)
    r.add_argument("--seeds", type=int, default=2)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="", help="comma list of villain ids")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
