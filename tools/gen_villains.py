"""Programmatically draw the original villain wanted-poster art.

17 villains x 4 animation frames = 68 files under `packs/stolen-scepter/art/villains/`.
Each villain is a bust drawn from the look its bio gives (witch, orc chief,
glass queen, fiend, ...) with a blink on frame 2; no original game art is read.

Writes the PNGs and registers provenance.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas  # noqa: E402
from packart import ROOT, out_dir, register  # noqa: E402
from pngio import write_png  # noqa: E402

OUT = out_dir("villains")
AUTHOR = "procedural (tools/gen_villains.py)"

OUTLINE = (26, 22, 30)
SKIN = (226, 194, 164)
BONE = (232, 228, 214)
WHITE = (240, 240, 240)


def shade(c, d):
    return tuple(max(0, min(255, v + d)) for v in c)


def face(frame, cfg):
    c = Canvas(48, 34, (0, 0, 0, 0))
    dy = [0, 0, -1, 0][frame]
    cx, hy, hr = 24, 14 + dy, 10
    skin = cfg.get("skin", SKIN)
    skin_d = shade(skin, -34)
    cloth = cfg.get("cloth", (74, 70, 92))

    for y in range(25, 34):
        hw = int(7 + (y - 25) * 1.4)
        c.rect(cx - hw, y, hw * 2, 1, cloth)
    c.rect(cx - 4, 20 + dy, 8, 7, skin_d)
    c.disc(cx, hy, hr, skin)
    c.rect(cx - hr + 1, hy - 1, hr * 2 - 2, 6, skin)

    if cfg.get("hair"):
        c.disc(cx, hy - 3, hr + 1, cfg["hair"])
        c.rect(cx - hr - 1, hy - 4, hr * 2 + 2, 4, cfg["hair"])
        c.disc(cx, hy + 1, hr, skin)
        c.rect(cx - hr + 2, hy - 1, hr * 2 - 4, 7, skin)
    if cfg.get("hood"):
        c.disc(cx, hy - 2, hr + 2, cfg["hood"])
        c.disc(cx, hy + 2, hr - 1, skin)
    if cfg.get("helm"):
        c.disc(cx, hy - 2, hr + 1, cfg["helm"])
        c.rect(cx - hr - 1, hy - 2, hr * 2 + 2, 5, cfg["helm"])
        c.disc(cx, hy + 2, hr - 1, skin)
        c.hline(cx - hr, hy - 1, hr * 2, shade(cfg["helm"], -40))
    if cfg.get("crown"):
        c.rect(cx - hr, hy - hr - 1, hr * 2, 3, cfg["crown"])
        for x in range(cx - hr, cx + hr, 4):
            c.rect(x, hy - hr - 4, 2, 4, cfg["crown"])
    if cfg.get("hat"):
        c.rect(cx - hr - 2, hy - hr - 1, hr * 2 + 4, 4, cfg["hat"])
        c.tri(cx - hr - 2, hy - hr, cx, hy - hr - 13, cx + hr + 2, hy - hr, cfg["hat"])
    if cfg.get("horns"):
        c.tri(cx - hr + 1, hy - hr + 2, cx - hr - 4, hy - hr - 6, cx - hr + 4, hy - hr + 2, BONE)
        c.tri(cx + hr - 1, hy - hr + 2, cx + hr + 4, hy - hr - 6, cx + hr - 4, hy - hr + 2, BONE)

    if cfg.get("angry"):
        c.hline(cx - 7, hy - 1, 5, OUTLINE)
        c.hline(cx + 2, hy - 2, 5, OUTLINE)
    else:
        c.hline(cx - 7, hy - 2, 5, OUTLINE)
        c.hline(cx + 2, hy - 2, 5, OUTLINE)

    blink = frame == 2
    ec = cfg.get("eye", OUTLINE)
    if cfg.get("eyepatch"):
        if not blink:
            c.rect(cx + 2, hy - 1, 3, 3, WHITE)
            c.rect(cx + 3, hy, 2, 2, ec)
        c.rect(cx - 8, hy - 2, 7, 6, OUTLINE)
        c.hline(cx - 9, hy - 3, 9, OUTLINE)
    else:
        if blink:
            c.hline(cx - 6, hy, 4, ec)
            c.hline(cx + 2, hy, 4, ec)
        else:
            c.rect(cx - 6, hy - 1, 3, 3, WHITE)
            c.rect(cx - 5, hy, 2, 2, ec)
            c.rect(cx + 3, hy - 1, 3, 3, WHITE)
            c.rect(cx + 4, hy, 2, 2, ec)

    c.vline(cx, hy + 2, 3, skin_d)
    c.hline(cx - 3, hy + 6, 6, shade(skin, -60))
    if cfg.get("tusks"):
        c.vline(cx - 3, hy + 5, 4, BONE)
        c.vline(cx + 3, hy + 5, 4, BONE)
    if cfg.get("beard"):
        c.disc(cx, hy + 8, hr - 1, cfg["beard"])
        c.hline(cx - 3, hy + 6, 6, shade(skin, -50))
    if cfg.get("scar"):
        for i in range(5):
            c.set(cx - 7 + i, hy - 1 + i, shade(skin, -70))
    return c.px


CONFIGS = {
    "ottoline": dict(skin=(226, 198, 172), hair=(120, 120, 128), cloth=(92, 74, 52)),
    "jessa": dict(skin=(206, 170, 134), hood=(96, 70, 48), scar=True, angry=True),
    "morwen": dict(skin=(224, 210, 204), hair=(40, 34, 58), hat=(52, 40, 86), eye=(240, 200, 90)),
    "harrek": dict(skin=(196, 156, 118), hair=(92, 60, 36), beard=(92, 60, 36), angry=True),
    "edric": dict(skin=(222, 192, 160), helm=(150, 156, 170), cloth=(120, 40, 46)),
    "isolde": dict(skin=(236, 226, 224), hair=(210, 206, 196), crown=(190, 190, 204), eye=(150, 40, 50)),
    "torvald": dict(skin=(212, 176, 140), helm=(84, 88, 100), beard=(60, 56, 54), angry=True),
    "elowen": dict(skin=(214, 196, 158), hood=(52, 116, 56), eye=(90, 170, 80)),
    "grubnak": dict(skin=(128, 162, 88), hair=(44, 40, 38), tusks=True, angry=True),
    "mordecai": dict(skin=(190, 196, 184), hood=(44, 44, 52), eye=(160, 220, 170)),
    "nhalia": dict(skin=(232, 228, 214), hood=(214, 210, 196), eye=(70, 60, 70)),
    "thrumm": dict(skin=(150, 140, 128), hair=(70, 66, 62), beard=(120, 116, 110), angry=True),
    "vash": dict(skin=(200, 220, 230), crown=(170, 220, 236), hair=(60, 90, 120)),
    "cassian": dict(skin=(220, 190, 160), hair=(30, 28, 34), eyepatch=True, cloth=(40, 40, 52)),
    "valdris": dict(skin=(176, 70, 92), horns=True, eye=(250, 220, 90), cloth=(110, 30, 70)),
    "oren": dict(skin=(200, 176, 150), hat=(36, 34, 46), eye=(120, 150, 200), angry=True),
    "sorvane": dict(skin=(168, 160, 156), horns=True, hood=(70, 64, 66), eye=(250, 140, 60), crown=(60, 56, 58), angry=True),
}


def main():
    os.makedirs(OUT, exist_ok=True)
    written = 0
    written_rels = []
    for name in sorted(CONFIGS):
        for frame in range(4):
            px = face(frame, CONFIGS[name])
            assert len(px) == 48 * 34 * 4
            rel = f"art/villains/{name}_{frame:02d}.png"
            write_png(os.path.join(OUT, f"{name}_{frame:02d}.png"), 48, 34, px)
            written_rels.append(rel)
            written += 1
    register(written_rels, AUTHOR)
    print(f"wrote {written} villain files to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
