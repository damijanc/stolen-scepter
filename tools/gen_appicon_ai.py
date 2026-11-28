"""AI app icon for the mobile build: the Stolen Scepter as a launcher picture.

    python3 tools/gen_appicon_ai.py render --run appicon1 [--seeds 4] [--only shield]
    python3 tools/gen_appicon_ai.py compose --run appicon1 --pick scepter_103

`render` draws square candidates on ComfyUI (through the tunnel, see
tools/README.md) into local, ignored `game-assets/ai-out/<run>/appicon/`,
with a contact sheet, in the villain portraits' painterly style. A launcher
icon is seen at 48 px, so every look is one bold object, centred, on a plain
dark ground: no text, no scene. The look is the user's call; the candidates
are shown before anything is composed.

`compose` needs no tunnel: it keys the picked render's flat grey surround to
transparent and writes the Android adaptive icon (`shell/android/res/`: a
foreground mipmap per density over a navy colour layer), which
`build_apk.sh` compiles in, plus `shell/android/play_icon_512.png` for the
Play listing. The user picked scepter_103 on 2026-11-28.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_ai  # noqa: E402
from gen_startup import Args, contact_sheet, render  # noqa: E402
from packart import ROOT  # noqa: E402
from pngio import read_png, write_png  # noqa: E402

STYLE = ("app icon, centred single object, bold simple silhouette readable at small "
         "size, painterly digital painting, classic high fantasy strategy game art, "
         "rich saturated colours, dramatic rim light, crisp detail")
NEG = ("text, letters, words, watermark, signature, frame, border, people, hands, "
       "busy background, clutter, multiple objects, blurry, low quality, deformed, "
       "cropped")
SIZE = (1024, 1024)

LOOKS = {
    "shield": ("a golden royal scepter topped with a large red ruby, standing upright "
               "in front of a deep blue heraldic shield with a gold rim, on a dark "
               "navy background"),
    "scepter": ("a single ornate golden royal scepter topped with a glowing red ruby "
                "and a small crown, standing upright, glowing softly, on a plain dark "
                "royal blue background with a faint golden halo"),
    "crown": ("a golden crown resting on a red velvet cushion with a golden royal "
              "scepter laid across it, ruby gems, on a plain dark background"),
}


def out_dir(run):
    return os.path.join(ROOT, "game-assets", "ai-out", run, "appicon")


def cmd_render(cli):
    out = out_dir(cli.run)
    os.makedirs(out, exist_ok=True)
    gen_ai.OUT = out
    args = Args(cli.host, cli.ckpt, None)
    gen_ai.clear_queue(args.host)
    looks = cli.only.split(",") if cli.only else list(LOOKS)
    for look in looks:
        for seed in range(cli.first_seed, cli.first_seed + cli.seeds):
            print(look, seed, flush=True)
            render(args, f"{LOOKS[look]}, {STYLE}", NEG, SIZE, seed, False,
                   os.path.join(out, f"{look}_{seed}.png"))
    contact_sheet(out)
    return 0


# The ground the medallion sits on, sampled from its own navy field, so the
# adaptive icon's background layer continues the picture.
GROUND = (14, 22, 40)
# Android adaptive icon: a 108 dp layer, of which a launcher shows about the
# middle 72 dp. The picture is scaled so the medallion ring (0.885 of the
# render's width) is just inside that circle.
LAYER_DP, PICTURE_DP = 108, 79
DENSITIES = {"mdpi": 1.0, "hdpi": 1.5, "xhdpi": 2.0, "xxhdpi": 3.0, "xxxhdpi": 4.0}
RES = os.path.join(ROOT, "shell", "android", "res")
# The Play listing's 512 px icon: full bleed (the staff runs off the bottom
# edge, as in the render), Play rounds the corners itself.
PLAY_SIZE, PLAY_PICTURE = 512, 512


def key_out_ground(w, h, px):
    """Make the render's flat grey surround transparent: flood from the
    border through grey-ish pixels, alpha ramping up with the distance from
    the grey. Keyed pixels take GROUND's colour, so downscaling leaves no
    grey halo."""
    corners = [px[i * 4:i * 4 + 3] for i in (0, w - 1, (h - 1) * w, h * w - 1)]
    grey = [sum(c[k] for c in corners) / 4 for k in range(3)]

    def dist(i):
        return int(max(abs(px[i * 4 + k] - grey[k]) for k in range(3)))

    out = bytearray(px)
    seen = bytearray(w * h)
    stack = [i for i in range(w)] + [(h - 1) * w + i for i in range(w)] + \
        [y * w for y in range(h)] + [y * w + w - 1 for y in range(h)]
    lo, hi = 18, 60
    while stack:
        i = stack.pop()
        if seen[i]:
            continue
        seen[i] = 1
        d = dist(i)
        if d >= hi:
            continue
        a = max(0, min(255, (d - lo) * 255 // (hi - lo)))
        out[i * 4:i * 4 + 4] = bytes((*GROUND, a)) if a == 0 else \
            bytes((*px[i * 4:i * 4 + 3], a))
        x, y = i % w, i // w
        if x > 0:
            stack.append(i - 1)
        if x < w - 1:
            stack.append(i + 1)
        if y > 0:
            stack.append(i - w)
        if y < h - 1:
            stack.append(i + w)
    return out


def place(src_w, src_px, size, picture, ground):
    """`src` scaled to `picture` px, centred on a `size` px square that is
    transparent, or `ground` when given (alpha-composited)."""
    _, _, pic = gen_ai.box_downscale(src_w, src_w, src_px, picture, picture)
    fill = bytes((*ground, 255)) if ground else bytes((*GROUND, 0))
    out = bytearray(fill * (size * size))
    off = (size - picture) // 2
    for y in range(picture):
        for x in range(picture):
            s = (y * picture + x) * 4
            d = ((y + off) * size + x + off) * 4
            r, g, b, a = pic[s:s + 4]
            if ground:
                out[d:d + 3] = bytes(
                    (c * a + ground[k] * (255 - a)) // 255 for k, c in enumerate((r, g, b)))
            else:
                out[d:d + 4] = pic[s:s + 4]
    return out


ADAPTIVE_XML = """<?xml version="1.0" encoding="utf-8"?>
<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">
    <background android:drawable="@color/ic_launcher_background" />
    <foreground android:drawable="@mipmap/ic_launcher_foreground" />
</adaptive-icon>
"""
COLORS_XML = """<?xml version="1.0" encoding="utf-8"?>
<resources>
    <color name="ic_launcher_background">#{:02X}{:02X}{:02X}</color>
</resources>
"""


def cmd_compose(cli):
    w, h, px = read_png(os.path.join(out_dir(cli.run), f"{cli.pick}.png"))
    assert w == h, "the render must be square"
    keyed = key_out_ground(w, h, px)
    written = []
    for name, scale in DENSITIES.items():
        size, pic = round(LAYER_DP * scale), round(PICTURE_DP * scale)
        rel = f"mipmap-{name}/ic_launcher_foreground.png"
        os.makedirs(os.path.join(RES, f"mipmap-{name}"), exist_ok=True)
        write_png(os.path.join(RES, rel), size, size, place(w, keyed, size, pic, None))
        written.append(rel)
    os.makedirs(os.path.join(RES, "mipmap-anydpi-v26"), exist_ok=True)
    os.makedirs(os.path.join(RES, "values"), exist_ok=True)
    with open(os.path.join(RES, "mipmap-anydpi-v26", "ic_launcher.xml"), "w") as fh:
        fh.write(ADAPTIVE_XML)
    with open(os.path.join(RES, "values", "ic_launcher_colors.xml"), "w") as fh:
        fh.write(COLORS_XML.format(*GROUND))
    play = os.path.join(os.path.dirname(RES), "play_icon_512.png")
    write_png(play, PLAY_SIZE, PLAY_SIZE, place(w, keyed, PLAY_SIZE, PLAY_PICTURE, GROUND))
    print("wrote", ", ".join(written), "+ adaptive xml +", os.path.relpath(play, ROOT))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    sub = ap.add_subparsers(dest="command", required=True)
    c = sub.add_parser("compose")
    c.add_argument("--run", default="appicon1")
    c.add_argument("--pick", default="scepter_103")
    c.set_defaults(func=cmd_compose)
    r = sub.add_parser("render")
    r.add_argument("--run", default="appicon1")
    r.add_argument("--seeds", type=int, default=4)
    r.add_argument("--first-seed", type=int, default=101)
    r.add_argument("--only", default="")
    r.add_argument("--host", default="http://127.0.0.1:8188")
    r.add_argument("--ckpt", default=gen_ai.CKPT)
    r.set_defaults(func=cmd_render)
    cli = ap.parse_args()
    return cli.func(cli)


if __name__ == "__main__":
    sys.exit(main())
