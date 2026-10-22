"""Programmatically draw the original villain wanted-poster art.

17 villains x 4 animation frames = 68 files under `packs/open/art/villains/`.
Each villain is an original archetype bust (sorceress, orc warlord, czar,
wizard, demon, ...) with a blink on frame 2; no original game art is read.

Writes the PNGs and registers provenance.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas  # noqa: E402
from pngio import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "packs", "open", "art", "villains")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
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
    "aimola": dict(skin=(224, 198, 196), hair=(66, 44, 86), hood=(96, 56, 128)),
    "arech": dict(skin=(186, 198, 184), hood=(72, 92, 74), eye=(196, 60, 60)),
    "auric": dict(skin=(212, 172, 118), hair=(184, 146, 62), helm=(214, 178, 84)),
    "bargash": dict(skin=(128, 162, 88), hair=(44, 40, 38), tusks=True, angry=True),
    "baron_makahl": dict(skin=(226, 198, 172), hair=(86, 64, 44), beard=(96, 74, 52)),
    "barrowpine": dict(skin=(202, 172, 142), hood=(74, 110, 70), beard=(116, 96, 64)),
    "caneghor": dict(skin=(178, 170, 174), hair=(34, 32, 44), helm=(74, 78, 96), angry=True),
    "czar_nickolai": dict(skin=(220, 190, 160), beard=(232, 232, 232), crown=(232, 196, 72)),
    "dread_rob": dict(skin=(208, 178, 150), eyepatch=True, cloth=(120, 40, 46)),
    "hack": dict(skin=(198, 170, 138), hood=(120, 92, 56), eye=(70, 90, 140)),
    "magus": dict(skin=(220, 192, 162), beard=(226, 226, 226), hat=(66, 60, 150)),
    "mahk": dict(skin=(160, 132, 100), helm=(90, 90, 96), angry=True),
    "moradon": dict(skin=(190, 72, 62), horns=True, eye=(250, 220, 90), angry=True),
    "murray": dict(skin=(122, 168, 120), hood=(70, 104, 72), eye=(220, 200, 80)),
    "ragface": dict(skin=(200, 172, 142), hair=(150, 150, 150), scar=True, angry=True),
    "rinaldus": dict(skin=(224, 196, 166), helm=(150, 156, 170), cloth=(60, 80, 150)),
    "urthrax": dict(skin=(168, 60, 70), horns=True, eye=(250, 210, 80), crown=(70, 40, 50)),
}


def main():
    os.makedirs(OUT, exist_ok=True)
    with open(PROVENANCE) as fh:
        doc = json.load(fh)
    today = time.strftime("%Y-%m-%d")
    written = 0
    for name in sorted(CONFIGS):
        for frame in range(4):
            px = face(frame, CONFIGS[name])
            assert len(px) == 48 * 34 * 4
            rel = f"art/villains/{name}_{frame:02d}.png"
            write_png(os.path.join(OUT, f"{name}_{frame:02d}.png"), 48, 34, px)
            doc["assets"][rel] = {
                "author": AUTHOR,
                "method": "procedural",
                "source": "original",
                "license": "CC0-1.0",
                "date": today,
            }
            written += 1
    with open(PROVENANCE, "w") as fh:
        json.dump(doc, fh, indent=2)
        fh.write("\n")
    print(f"wrote {written} villain files to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
