"""Proof-of-concept modern renderer: terrain, a castle and a hero at 4x.

Writes to `game-assets/modern-proof/` (gitignored) and an HTML contact sheet.
Nothing here touches the committed pack; it is for judging the art direction.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from artlib import Image, circle, capsule, round_rect, mix, clamp, smoothstep, fbm, rgb, noise  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, "game-assets", "modern-proof")
W, H = 192, 136

G_LIGHT, G_MID, G_DARK = rgb("#8fd07a"), rgb("#5aa84e"), rgb("#3b7a39")
G_HI = rgb("#c8ecb0")
W_TOP, W_BOT, W_HI = rgb("#7cc8ea"), rgb("#1f5f92"), rgb("#e6f7ff")
F_LIGHT, F_MID, F_DARK, F_HI = rgb("#5fae5f"), rgb("#3f8a49"), rgb("#2b6337"), rgb("#8fd07a")
M_LIGHT, M_MID, M_DARK, SNOW = rgb("#c2c0ba"), rgb("#8d8b84"), rgb("#615f59"), rgb("#f4f7f9")
D_LIGHT, D_MID, D_DARK = rgb("#f2dcac"), rgb("#dcb877"), rgb("#b9945a")
S_LIGHT, S_MID, S_DARK, S_EDGE = rgb("#ded9cd"), rgb("#b8b2a3"), rgb("#827c6e"), rgb("#5b564c")
ROOF, ROOF_D = rgb("#c05a45"), rgb("#8a3a2a")
INK = rgb("#2a2530")


def rnd(i, seed):
    return noise(i * 1.37 + 0.5, i * 0.71 + 0.5, seed)


def vgrad(top, bottom, y0, y1):
    return lambda x, y: mix(top, bottom, clamp((y - y0) / max(1, y1 - y0)))


def grass(seed=3):
    img = Image(W, H)
    img.vertical_gradient(0, 0, W, H, G_LIGHT, G_MID)
    for y in range(H):
        for x in range(W):
            n = fbm(x / 30, y / 30, seed, 4)
            if n > 0.55:
                img.blend(x, y, G_DARK, (n - 0.55) * 0.45)
            elif n < 0.44:
                img.blend(x, y, G_HI, (0.44 - n) * 0.5)
    img.noise_overlay(G_DARK, 0.05, scale=1.4, seed=seed + 9)
    for i in range(46):
        x = 6 + int(rnd(i, seed) * (W - 12))
        y = 10 + int(rnd(i + 40, seed) * (H - 18))
        img.paint_sdf(*capsule(x, y, x - 1, y - 6, 0.8),
                      color=mix(G_HI, G_DARK, 0.35), alpha=0.55)
    return img


def water(seed=5):
    img = Image(W, H)
    img.vertical_gradient(0, 0, W, H, W_TOP, W_BOT)
    for y in range(H):
        for x in range(W):
            streak = fbm(x / 55, y / 7, seed, 3)
            band = smoothstep(0.55, 0.75, streak)
            if band > 0:
                img.blend(x, y, W_HI, band * 0.35)
            deep = smoothstep(0.45, 0.2, fbm(x / 60, y / 20, seed + 3, 3))
            if deep > 0:
                img.blend(x, y, W_BOT, deep * 0.3)
    for i in range(18):
        x = 8 + int(rnd(i, seed) * (W - 20))
        y = 8 + int(rnd(i + 20, seed) * (H - 16))
        img.paint_sdf(*capsule(x, y, x + 6, y - 1, 1.1), color=W_HI, alpha=0.5, blur=1.0)
    return img


def forest(seed=7):
    img = grass(seed)
    for i in range(7):
        cx = 26 + int(rnd(i, seed) * (W - 52))
        cy = 20 + int(rnd(i + 11, seed) * (H - 46))
        r = 20 + int(rnd(i + 22, seed) * 12)
        img.paint_sdf(*circle(cx, cy + 8, r * 0.95), color=(0.05, 0.12, 0.06), alpha=0.28, blur=7)
        img.paint_sdf(*circle(cx, cy, r), color=vgrad(F_LIGHT, F_DARK, cy - r, cy + r), blur=1.2)
        img.paint_sdf(*circle(cx - r * 0.3, cy - r * 0.35, r * 0.45), color=F_HI, alpha=0.55, blur=2.5)
    for i in range(5):
        x = 20 + int(rnd(i + 60, seed) * (W - 40))
        img.paint_sdf(*capsule(x, H - 12, x, H - 34, 2.4), color=rgb("#5a3f2b"), alpha=0.9)
    return img


def mountain(seed=11):
    img = Image(W, H)
    img.vertical_gradient(0, 0, W, H, M_LIGHT, M_MID)
    for i in range(4):
        cx = 20 + int(rnd(i, seed) * (W - 40))
        top = 16 + int(rnd(i + 5, seed) * 22)
        half = 34 + int(rnd(i + 9, seed) * 16)
        img.fill_poly([(cx, top), (cx + half, H), (cx - half, H)], M_DARK, 0.95)
        img.fill_poly([(cx, top), (cx + half * 0.5, H), (cx - half * 0.1, H)],
                      vgrad(M_LIGHT, M_MID, top, H), 0.9)
        img.fill_poly([(cx, top), (cx + 10, top + 16), (cx - 10, top + 16)], SNOW, 0.95)
    for i in range(40):
        x = int(rnd(i + 30, seed) * W)
        y = int(rnd(i + 70, seed) * H)
        img.blend(x, y, M_LIGHT, 0.4)
    return img


def desert(seed=13):
    img = Image(W, H)
    img.vertical_gradient(0, 0, W, H, D_LIGHT, D_MID)
    for y in range(H):
        for x in range(W):
            dune = fbm((x + y * 0.6) / 40, y / 26, seed, 3)
            img.blend(x, y, D_DARK, smoothstep(0.5, 0.75, dune) * 0.35)
            img.blend(x, y, D_LIGHT, smoothstep(0.45, 0.2, dune) * 0.3)
    for i in range(60):
        x = int(rnd(i, seed) * W)
        y = int(rnd(i + 50, seed) * H)
        img.blend(x, y, D_DARK, 0.3)
    return img


def outside_pred(kind, arg):
    band = 42
    def und(t):
        return (fbm(t / 40.0, 0.0, 77, 3) - 0.5) * 22
    if kind == "side":
        if arg == "N":
            return lambda x, y: y - (band + und(x))
        if arg == "S":
            return lambda x, y: (H - band - und(x)) - y
        if arg == "W":
            return lambda x, y: x - (band + und(y))
        return lambda x, y: (W - band - und(y)) - x
    if kind == "outer":
        a, b = arg
        return lambda x, y: min(outside_pred("side", a)(x, y), outside_pred("side", b)(x, y))
    corner = arg
    r = 46
    return lambda x, y: ((W - x) + (H - y) if corner == "SE"
                         else (W - x) + y if corner == "NE"
                         else x + (H - y) if corner == "SW"
                         else x + y) - r


def edge(base, grass_img, kind, arg):
    pred = outside_pred(kind, arg)
    img = Image(W, H)
    for y in range(H):
        for x in range(W):
            b = pred(x, y)  # >0 inside terrain
            t = smoothstep(-2.5, 2.5, b)
            g = grass_img.get(x, y)
            te = base.get(x, y)
            col = mix(g[:3], te[:3], t)
            a = max(g[3], te[3])
            if b < 0:  # ambient occlusion on the grass side
                col = mix(col, (0.1, 0.15, 0.1), 0.35 * (1 - smoothstep(-10, 0, b)))
            elif b < 4:  # rim light on the terrain lip
                col = mix(col, (1.0, 1.0, 1.0), 0.25 * (1 - b / 4))
            img.set_px(x, y, (col[0], col[1], col[2], a))
    return img


def castle(seed=17):
    img = Image(3 * W, 2 * H)
    img.vertical_gradient(0, 0, 3 * W, 2 * H, G_LIGHT, G_MID)
    img.noise_overlay(G_DARK, 0.08, scale=3.0, seed=seed)
    W3, H2 = 3 * W, 2 * H

    def block(x, y, w, h, r=6, top=S_LIGHT, bottom=S_DARK):
        img.paint_sdf(*circle(x + w / 2, y + h + 8, w * 0.55), color=(0.1, 0.12, 0.1), alpha=0.22, blur=9)
        img.paint_sdf(*round_rect(x, y, w, h, r), color=vgrad(top, bottom, y, y + h), blur=0.8)

    def merlons(x0, x1, y, w=30, h=34):
        xx = x0
        while xx + w <= x1:
            img.paint_sdf(*round_rect(xx, y, w, h, 6), color=vgrad(S_LIGHT, S_MID, y, y + h), blur=0.8)
            img.paint_sdf(*capsule(xx + 4, y + 5, xx + w - 4, y + 5, 2.2), color=(1, 1, 1), alpha=0.35, blur=1.5)
            xx += int(w * 1.5)

    block(20, 40, 92, 214)
    block(W3 - 112, 40, 92, 214)
    merlons(20, 112, 18, 30, 30)
    merlons(W3 - 112, W3 - 20, 18, 30, 30)
    block(92, 96, W3 - 184, 158)
    merlons(92, W3 - 92, 78, 34, 24)

    gate = round_rect(W3 / 2 - 46, 150, 92, 104, 44)
    img.paint_sdf(*gate, color=(0.08, 0.07, 0.09), blur=0.8)
    img.paint_sdf(*capsule(W3 / 2, 150 + 40, W3 / 2, 254, 5), color=S_EDGE, alpha=0.8)

    for wx in (46, 74, W3 - 76, W3 - 48):
        img.paint_sdf(*round_rect(wx, 118, 20, 30, 6), color=rgb("#ffe9a8"), alpha=0.9, blur=1.0)
        img.paint_sdf(*round_rect(wx, 118, 20, 30, 6), color=(0, 0, 0), alpha=0.15)
    img.paint_sdf(*capsule(W3 / 2, 20, W3 / 2, 96, 3), color=S_EDGE)
    img.fill_poly([(W3 / 2, 22), (W3 / 2, 54), (W3 / 2 + 44, 38)], ROOF)
    img.fill_poly([(W3 / 2, 22), (W3 / 2, 54), (W3 / 2 + 44, 38)], ROOF_D, 0.0)
    return img


def hero(seed=19):
    img = Image(W, H)
    cx, gy = W // 2, 118
    img.paint_sdf(*circle(cx, gy + 6, 26), color=(0.1, 0.12, 0.1), alpha=0.28, blur=8)

    img.fill_poly([(cx - 14, 60), (cx + 14, 60), (cx + 30, gy - 6), (cx - 30, gy - 6)],
                  rgb("#3f5aa8"), 0.9)
    img.paint_sdf(*capsule(cx - 26, gy - 10, cx + 26, gy - 10, 6), color=rgb("#2c3f78"), alpha=0.6)

    img.paint_sdf(*capsule(cx - 10, gy - 8, cx - 12, gy + 2, 7), color=rgb("#4a5f9e"))
    img.paint_sdf(*capsule(cx + 10, gy - 8, cx + 12, gy + 2, 7), color=rgb("#4a5f9e"))

    img.paint_sdf(*round_rect(cx - 20, 52, 40, 44, 12),
                  color=vgrad(rgb("#7d94d6"), rgb("#42548e"), 52, 96), blur=0.8)
    img.paint_sdf(*capsule(cx - 12, 62, cx + 12, 62, 2), color=rgb("#c8d4f2"), alpha=0.5, blur=1.5)

    img.paint_sdf(*capsule(cx - 22, 58, cx - 26, 84, 6), color=rgb("#9aa4b8"))
    img.paint_sdf(*capsule(cx + 22, 58, cx + 26, 84, 6), color=rgb("#9aa4b8"))

    img.paint_sdf(*circle(cx - 30, 80, 13), color=vgrad(rgb("#d9cfa6"), rgb("#a99a63"), 67, 93), blur=0.8)
    img.paint_sdf(*circle(cx - 30, 80, 5), color=rgb("#c05a45"))

    img.paint_sdf(*capsule(cx + 30, 44, cx + 30, 96, 2.5), color=rgb("#cfd6e0"))
    img.paint_sdf(*capsule(cx + 24, 54, cx + 36, 54, 3), color=rgb("#b08a3a"))

    img.paint_sdf(*round_rect(cx - 15, 22, 30, 34, 12),
                  color=vgrad(rgb("#d7dde6"), rgb("#8b93a3"), 22, 56), blur=0.8)
    img.paint_sdf(*round_rect(cx - 15, 34, 30, 9, 4), color=rgb("#2a2f3a"), alpha=0.9)
    img.paint_sdf(*capsule(cx, 22, cx, 6, 2.5), color=rgb("#c05a45"))
    img.paint_sdf(*circle(cx, 6, 5), color=rgb("#e08a6a"))
    return img


def main():
    tiles = {
        "grass": grass(),
        "grass_variant": grass(seed=23),
        "water": water(),
        "forest": forest(),
        "mountain": mountain(),
        "desert": desert(),
    }
    g = tiles["grass"]
    tiles["forest_edge_11"] = edge(tiles["forest"], g, "side", "N")
    tiles["forest_edge_05"] = edge(tiles["forest"], g, "inner", "SE")
    tiles["mountain_edge_02"] = edge(tiles["mountain"], g, "outer", ("S", "W"))
    tiles["water_edge_10"] = edge(tiles["water"], g, "side", "N")

    os.makedirs(os.path.join(OUT, "art", "tiles"), exist_ok=True)
    for name, img in tiles.items():
        img.save(os.path.join(OUT, "art", "tiles", name + ".png"))
    os.makedirs(os.path.join(OUT, "art", "ui"), exist_ok=True)
    castle().save(os.path.join(OUT, "art", "ui", "castle_preview.png"))
    os.makedirs(os.path.join(OUT, "art", "sprites"), exist_ok=True)
    hero().save(os.path.join(OUT, "art", "sprites", "hero_preview.png"))

    figures = []
    for name in list(tiles) + ["../ui/castle_preview", "../sprites/hero_preview"]:
        src = name if name.startswith("..") else f"art/tiles/{name}.png"
        label = os.path.basename(name).replace("_preview", "")
        figures.append(f'<figure><img src="{src}"><figcaption>{label}</figcaption></figure>')
    html = ("<!doctype html><meta charset=utf-8><title>Modern art proof</title>"
            "<style>body{background:#1c1b22;color:#ddd;font:14px system-ui;padding:2rem}"
            ".grid{display:flex;flex-wrap:wrap;gap:1rem}"
            "figure{margin:0;background:#2a2932;padding:.6rem;border-radius:10px}"
            "img{image-rendering:auto;width:288px;display:block}"
            "figcaption{margin-top:.4rem;color:#9aa}</style>"
            "<h1>Modern renderer proof (4x)</h1><div class=grid>"
            + "".join(figures) + "</div>")
    with open(os.path.join(OUT, "index.html"), "w") as fh:
        fh.write(html)
    print(f"proof -> {os.path.relpath(OUT, ROOT)}/index.html")
    return 0


if __name__ == "__main__":
    sys.exit(main())
