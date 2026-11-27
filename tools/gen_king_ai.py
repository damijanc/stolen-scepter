"""AI portrait of King Aldric for the mobile audience screen.

    python3 tools/gen_king_ai.py render --run king1 [--seeds 4]
    python3 tools/gen_king_ai.py compose --run king1 --seed 103

`render` draws head-and-shoulders portraits on ComfyUI (through the tunnel,
see tools/README.md) into local, ignored `game-assets/ai-out/<run>/king/`,
with a contact sheet, in the villain portraits' painterly style.

`compose` needs no tunnel: it writes the picked render as the four breathing
frames `art/ui/king_00..03.png` at the villain portraits' size (the 48x34
slot times the pack's scale), records provenance, and `build_pack.py` then
names `sprites.ui.king_portrait`. The look is the user's call (2026-11-23):
a kindly old king.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
import json  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from gen_villains_ai import BREATH, FRAMES, NEG, SIZE, STYLE, frame  # noqa: E402
from packart import PACK, ROOT, register  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

LOOK = ("a kindly old king with a long silver beard and warm tired eyes, a gold crown "
        "set with rubies, a red velvet robe with an ermine collar, a gentle wise "
        "expression, a throne hall with candles and banners behind him")
AUTHOR = "AI-generated (tools/gen_king_ai.py: SDXL DreamShaper XL), composed by " \
         "tools/gen_king_ai.py"


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "king")


def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, None)
    gen_ai.clear_queue(args.host)
    for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
        print("king", seed, flush=True)
        render(args,
               f"head and shoulders portrait of {LOOK}, looking at the viewer, face in "
               f"the upper middle of the picture, {STYLE}",
               NEG, SIZE, seed, False, os.path.join(out, f"king_{seed}.png"))
    contact_sheet(out)
    return 0


def cmd_compose(cli):
    with open(os.path.join(PACK, "game.json")) as fh:
        scale = json.load(fh)["render"].get("scale", 1)
    tw, th = 48 * scale, 34 * scale
    w, h, px = read_png(os.path.join(out_dir(cli.run), f"king_{cli.seed}.png"))
    written = []
    for i in range(FRAMES):
        rel = f"art/ui/king_{i:02d}.png"
        write_png(os.path.join(PACK, rel), tw, th, frame(w, h, px, tw, th, BREATH[i]))
        written.append(rel)
    register(written, AUTHOR, method="ai-generated")
    print(f"wrote {len(written)} King frames at {tw}x{th}; now run tools/build_pack.py")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    r = sub.add_parser("render")
    r.add_argument("--run", default="king1")
    r.add_argument("--seeds", type=int, default=4)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    c = sub.add_parser("compose")
    c.add_argument("--run", default="king1")
    c.add_argument("--seed", type=int, required=True)
    c.set_defaults(func=cmd_compose)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
