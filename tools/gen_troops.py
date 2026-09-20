"""Programmatically draw the original troop sprites for the open art pack.

25 troops x 4 idle frames = 100 files under `packs/open/art/troops/`. Each
troop is an archetype (armoured knight, green orc, mounted rider, wolf,
dragon, ...) drawn from primitives; no original game art is read.

Writes the PNGs and registers provenance.
"""
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas, GOLD, LEAF, LEAF_D, ROCK, STONE, STONE_L, DIRT  # noqa: E402
from upscale import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "packs", "open", "art", "troops")
PROVENANCE = os.path.join(ROOT, "packs", "open", "provenance.json")
AUTHOR = "procedural (tools/gen_troops.py)"

OUTLINE = (26, 22, 30)
SKIN = (232, 190, 152)
STEEL = (146, 152, 166)
STEEL_L = (198, 202, 212)
STEEL_D = (98, 104, 118)
GOLD = (232, 196, 72)
BONE = (232, 228, 214)
WHITE = (240, 240, 240)


def shade(c, d):
    return tuple(max(0, min(255, v + d)) for v in c)


def sprite():
    return Canvas(48, 34, (0, 0, 0, 0))


def weapon(c, kind, cx, top, gy, s, metal):
    if kind == "sword":
        c.vline(38, top - 2, max(8, int(12 * s)), metal)
        c.hline(35, top + max(8, int(12 * s)) - 4, 6, GOLD)
    elif kind == "axe":
        c.vline(38, top, max(8, int(11 * s)), shade(DIRT, -40))
        c.rect(34, top, 6, 5, metal)
        c.set(33, top + 1, metal)
    elif kind == "pike":
        c.vline(12, top - 8, gy - top, shade(DIRT, -30))
        c.rect(10, top - 12, 4, 5, metal)
    elif kind == "bow":
        for i in range(-7, 8):
            x = 10 + abs(i) // 2
            c.set(x, 18 + i, shade(DIRT, -30))
        c.vline(12, 11, 14, WHITE)
    elif kind == "scimitar":
        for i in range(0, 12):
            c.set(34 + i, top + 6 - i // 2, metal)
        c.vline(34, top + 6, 5, GOLD)
    elif kind == "staff":
        c.vline(37, top - 4, gy - top - 4, shade(DIRT, -50))
        c.disc(37, top - 5, 3, GOLD)
    elif kind == "club":
        c.rect(36, top - 2, 4, 12, shade(DIRT, -20))
        c.disc(38, top - 3, 4, shade(DIRT, -10))


def headgear(c, cfg, cx, cy, r):
    if cfg.get("helm"):
        c.rect(cx - r - 1, cy - r, r * 2 + 2, r, STEEL)
        c.hline(cx - r - 1, cy - 1, r * 2 + 2, OUTLINE)
        if cfg.get("plume"):
            c.rect(cx - 1, cy - r - 4, 2, 4, cfg["plume"])
    if cfg.get("hat"):
        c.tri(cx - r - 2, cy - r + 1, cx, cy - r - 9, cx + r + 2, cy - r + 1, cfg["hat"])
    if cfg.get("hood"):
        c.disc(cx, cy - 1, r + 2, cfg["hood"])
        c.disc(cx, cy + 1, r - 1, cfg.get("skin", SKIN))
    if cfg.get("horns"):
        c.tri(cx - r, cy - r + 1, cx - r - 4, cy - r - 5, cx - r + 2, cy - r, BONE)
        c.tri(cx + r, cy - r + 1, cx + r + 4, cy - r - 5, cx + r - 2, cy - r, BONE)
    if cfg.get("ears"):
        c.tri(cx - r, cy, cx - r - 4, cy - 4, cx - r + 1, cy - 2, cfg.get("skin", SKIN))
        c.tri(cx + r, cy, cx + r + 4, cy - 4, cx + r - 1, cy - 2, cfg.get("skin", SKIN))
    if cfg.get("hair"):
        c.disc(cx, cy - 2, r + 1, cfg["hair"])
        c.disc(cx, cy + 1, r, cfg.get("skin", SKIN))
    if cfg.get("beard"):
        c.disc(cx, cy + r, r - 1, cfg["beard"])
        c.disc(cx, cy + r - 2, r - 3, cfg.get("skin", SKIN))


def humanoid(frame, cfg):
    c = sprite()
    s = cfg.get("scale", 1.0)
    dy = [0, -1, 0, -1][frame]
    gy = 32 + dy
    leg_h = max(4, int(6 * s))
    body_h = max(5, int(9 * s))
    body_w = max(7, int(9 * s))
    hr = max(3, int(4 * s))
    cx = 24
    top = gy - leg_h - body_h
    hcy = top - hr
    cloth = cfg.get("cloth", (70, 90, 150))
    cloth_d = shade(cloth, -34)
    skin = cfg.get("skin", SKIN)
    metal = cfg.get("metal", STEEL)
    legc = cfg.get("leg", cloth_d)
    armor = cfg.get("armor", cloth)

    if cfg.get("wings"):
        wc = cfg["wings"]
        c.tri(cx - 2, top + 1, cx - 15, hcy - 4, cx - 5, top + body_h, wc)
        c.tri(cx + 2, top + 1, cx + 15, hcy - 4, cx + 5, top + body_h, wc)
    if cfg.get("cape"):
        cap = cfg["cape"]
        for yy in range(top, gy - 1):
            hw = int(4 + (yy - top) * 0.45)
            c.rect(cx - hw, yy, hw * 2, 1, cap)

    lw = max(2, int(3 * s))
    c.rect(cx - lw - 1, gy - leg_h, lw, leg_h, legc)
    c.rect(cx + 1, gy - leg_h, lw, leg_h, legc)
    c.rect(cx - lw - 1, gy - 2, lw, 2, shade(legc, -40))
    c.rect(cx + 1, gy - 2, lw, 2, shade(legc, -40))

    if cfg.get("robe"):
        for yy in range(top, gy):
            t = (yy - top) / max(1, gy - top)
            hw = int(body_w / 2 + t * 5)
            c.rect(cx - hw, yy, hw * 2, 1, cloth)
        draw_torso = False
    else:
        draw_torso = True

    if draw_torso:
        c.rect(cx - body_w // 2, top, body_w, body_h, armor)
        c.hline(cx - body_w // 2, top, body_w, shade(armor, 24))
        c.hline(cx - body_w // 2, gy - leg_h - 1, body_w, shade(armor, -50))

    aw = max(2, int(3 * s))
    armc = cfg.get("arm_skin", skin)
    c.rect(cx - body_w // 2 - aw, top + 1, aw, body_h - 2, armc)
    c.rect(cx + body_w // 2, top + 1, aw, body_h - 2, armc)

    if cfg.get("shield"):
        c.rect(cx - body_w // 2 - aw - 1, top + 1, 6, 8, metal)
        c.frame(cx - body_w // 2 - aw - 1, top + 1, 6, 8, shade(metal, -40))
        c.vline(cx - body_w // 2 - aw + 2, top + 2, 6, GOLD)

    weapon(c, cfg.get("weapon"), cx, top, gy, s, metal)

    c.disc(cx, hcy, hr, skin)
    c.rect(cx - hr, hcy + hr - 1, hr * 2, 2, shade(skin, -24))
    eye = cfg.get("eye", OUTLINE)
    c.set(cx - 1, hcy, eye)
    c.set(cx + 1, hcy, eye)
    headgear(c, cfg, cx, hcy, hr)
    return c.px


def beast(frame, cfg):
    c = sprite()
    s = cfg.get("scale", 1.0)
    dy = [0, -1, 0, -1][frame]
    gy = 31 + dy
    body = cfg.get("cloth", (120, 118, 112))
    body_d = shade(body, -30)
    cx, cy = 24, gy - max(5, int(7 * s))
    bw = max(10, int(16 * s))
    bh = max(5, int(8 * s))
    # body
    c.rect(cx - bw // 2, cy - bh // 2, bw, bh, body)
    c.disc(cx - bw // 2, cy, bh // 2 + 1, body)
    c.disc(cx + bw // 2, cy, bh // 2 + 1, body)
    # legs
    lh = max(4, int(6 * s))
    for lx in (cx - bw // 2 + 2, cx - 2, cx + 2, cx + bw // 2 - 3):
        c.rect(lx, gy - lh, 3, lh, body_d)
    # tail
    c.vline(cx - bw // 2 - 2, cy - 1, 5, body)
    c.set(cx - bw // 2 - 3, cy - 2, body)
    # neck + head
    hx = cx + bw // 2
    c.rect(hx, cy - bh // 2 - 3, 3, 4, body)
    c.disc(hx + 2, cy - bh // 2 - 4, max(3, int(4 * s)), body)
    c.rect(hx + 4, cy - bh // 2 - 4, 4, 3, body_d)
    c.set(hx + 3, cy - bh // 2 - 6, OUTLINE)
    c.tri(hx + 1, cy - bh // 2 - 8, hx - 1, cy - bh // 2 - 11, hx + 3, cy - bh // 2 - 8, body)
    c.tri(hx + 4, cy - bh // 2 - 8, hx + 7, cy - bh // 2 - 11, hx + 5, cy - bh // 2 - 8, body)
    return c.px


def dragon(frame, cfg):
    c = sprite()
    body = cfg.get("cloth", (170, 70, 54))
    body_d = shade(body, -34)
    belly = cfg.get("belly", (222, 168, 90))
    dy = [0, -1, 0, -1][frame]
    cx, gy = 22, 31 + dy
    c.tri(cx - 2, gy - 18, cx - 20, gy - 22, cx - 8, gy - 6, body_d)
    c.rect(cx - 12, gy - 12, 22, 8, body)
    c.rect(cx - 10, gy - 8, 18, 3, belly)
    c.rect(cx - 6, gy - 5, 3, 4, body_d)
    c.rect(cx + 4, gy - 5, 3, 4, body_d)
    c.tri(cx + 8, gy - 14, cx + 10, gy - 26, cx + 15, gy - 18, body)
    c.disc(cx + 15, gy - 18, 4, body)
    c.rect(cx + 18, gy - 18, 4, 3, body_d)
    c.tri(cx + 14, gy - 21, cx + 12, gy - 27, cx + 17, gy - 20, body)
    c.tri(cx + 18, gy - 21, cx + 21, gy - 27, cx + 20, gy - 20, body)
    c.set(cx + 17, gy - 19, GOLD)
    c.tri(cx - 8, gy - 12, cx - 24, gy - 30, cx + 2, gy - 22, body)
    c.tri(cx - 8, gy - 12, cx - 22, gy - 8, cx - 4, gy - 2, body_d)
    return c.px


def ghost(frame, cfg):
    c = sprite()
    body = cfg.get("cloth", (150, 190, 205))
    dy = [0, -2, 0, -2][frame]
    cx = 24
    base = 30 + dy
    c.disc(cx, base - 14, 10, body)
    c.rect(cx - 10, base - 16, 20, 18, body)
    for x in range(cx - 10, cx + 10, 4):
        c.rect(x, base + 2, 2, 3, body)
    c.disc(cx - 4, base - 16, 2, OUTLINE)
    c.disc(cx + 4, base - 16, 2, OUTLINE)
    c.hline(cx - 3, base - 9, 6, shade(body, -40))
    return c.px


def fairy(frame, cfg):
    c = sprite()
    body = cfg.get("cloth", (120, 200, 150))
    skin = cfg.get("skin", (250, 226, 190))
    dy = [0, -1, 0, -1][frame]
    gy = 31 + dy
    c.tri(20, gy - 16, 12, gy - 22, 20, gy - 10, (210, 230, 255))
    c.tri(28, gy - 16, 36, gy - 22, 28, gy - 10, (210, 230, 255))
    c.rect(22, gy - 12, 4, 8, body)
    c.disc(24, gy - 15, 3, skin)
    c.set(23, gy - 15, OUTLINE)
    c.set(25, gy - 15, OUTLINE)
    c.rect(22, gy - 4, 4, 4, body)
    return c.px


CONFIGS = {
    "peasants": dict(scale=0.9, cloth=(120, 96, 66), weapon="pike"),
    "militia": dict(scale=0.95, cloth=(96, 84, 70), weapon="sword", shield=True),
    "pikemen": dict(scale=1.0, cloth=(70, 84, 120), weapon="pike"),
    "archers": dict(scale=0.95, cloth=(66, 120, 78), weapon="bow"),
    "knights": dict(scale=1.05, cloth=(60, 80, 150), armor=STEEL, metal=STEEL, weapon="sword", shield=True, helm=True, plume=(190, 60, 52)),
    "barbarians": dict(scale=1.0, cloth=(150, 110, 70), armor=(170, 130, 90), weapon="axe", hair=(120, 84, 40)),
    "nomads": dict(scale=1.0, cloth=(210, 190, 140), weapon="scimitar", hood=(180, 160, 110)),
    "orcs": dict(scale=1.0, cloth=(80, 70, 50), armor=(110, 80, 60), skin=(122, 168, 92), weapon="axe"),
    "ogres": dict(scale=1.35, cloth=(140, 110, 80), armor=(150, 120, 90), skin=(176, 150, 110), weapon="club"),
    "giants": dict(scale=1.6, cloth=(120, 130, 150), armor=(140, 150, 168), skin=(200, 176, 150), weapon="club"),
    "trolls": dict(scale=1.3, cloth=(70, 100, 60), armor=(90, 130, 74), skin=(110, 150, 90), weapon="club"),
    "zombies": dict(scale=1.0, cloth=(90, 110, 80), armor=(110, 120, 96), skin=(150, 170, 130), weapon=None),
    "skeletons": dict(scale=1.0, cloth=(210, 206, 190), armor=BONE, skin=BONE, weapon="sword", eye=(60, 60, 70)),
    "vampires": dict(scale=1.05, cloth=(60, 40, 70), armor=(80, 56, 92), skin=(226, 214, 210), weapon=None, cape=(120, 30, 50)),
    "elves": dict(scale=0.98, cloth=(80, 150, 100), armor=(120, 180, 130), weapon="bow", ears=True, hair=(230, 220, 150)),
    "dwarves": dict(scale=0.8, cloth=(120, 70, 60), armor=(150, 100, 80), weapon="axe", beard=(160, 120, 70), helm=True),
    "gnomes": dict(scale=0.68, cloth=(90, 130, 80), armor=(110, 150, 96), weapon=None, hat=(180, 60, 60), beard=(210, 210, 210)),
    "druids": dict(scale=1.0, cloth=(60, 110, 70), weapon="staff", robe=True, hood=(70, 130, 84), beard=(220, 220, 220)),
    "archmages": dict(scale=1.0, cloth=(60, 70, 160), weapon="staff", robe=True, hat=(70, 82, 180), beard=(235, 235, 235)),
    "demons": dict(scale=1.2, cloth=(170, 60, 48), armor=(190, 80, 60), skin=(196, 70, 58), weapon="sword", horns=True, wings=(90, 40, 60)),
    "wolves": ("beast", dict(cloth=(120, 118, 112), scale=0.95)),
    "cavalry": ("mounted", dict(cloth=(70, 86, 150), armor=STEEL, skin=SKIN, weapon="sword", helm=True)),
    "dragons": ("dragon", dict(cloth=(172, 72, 56), belly=(226, 176, 96))),
    "ghosts": ("ghost", dict(cloth=(150, 190, 205))),
    "sprites": ("fairy", dict(cloth=(120, 200, 150), skin=(250, 226, 190))),
}


def mounted(frame, cfg):
    c = Canvas(48, 34, (0, 0, 0, 0))
    horse = beast(frame, {"cloth": (110, 84, 56), "scale": 0.95})
    # paste horse
    for i in range(0, len(horse), 4):
        if horse[i + 3] >= 128:
            o = i
            x = (o // 4) % 48
            y = (o // 4) // 48
            c.set(x, y, tuple(horse[o:o + 4]))
    rider = humanoid(frame, dict(cfg, scale=0.72))
    for i in range(0, len(rider), 4):
        if rider[i + 3] >= 128:
            o = i
            x = (o // 4) % 48
            y = (o // 4) // 48 - 4
            if 0 <= y < 34:
                c.set(x, y, tuple(rider[o:o + 4]))
    return c.px


def draw(name, frame):
    spec = CONFIGS[name]
    if isinstance(spec, tuple):
        kind, cfg = spec
        return {"beast": beast, "dragon": dragon, "ghost": ghost,
                "fairy": fairy, "mounted": mounted}[kind](frame, cfg)
    return humanoid(frame, spec)


def main():
    os.makedirs(OUT, exist_ok=True)
    with open(PROVENANCE) as fh:
        doc = json.load(fh)
    today = time.strftime("%Y-%m-%d")
    written = 0
    for name in sorted(CONFIGS):
        for frame in range(4):
            px = draw(name, frame)
            assert len(px) == 48 * 34 * 4, (name, frame, len(px))
            rel = f"art/troops/{name}_{frame:02d}.png"
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
    print(f"wrote {written} troop files to {os.path.relpath(OUT, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
