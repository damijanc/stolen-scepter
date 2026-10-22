"""Draw the app icon: the Scepter on a royal shield, as 64x64 pixel art.

Writes a macOS iconset directory (every size from 16 to 1024 px: whole-pixel
upscales above 64, box-averaged below), which `make_dmg.sh` turns into the
bundle's `.icns` with `iconutil`.

Usage: python3 tools/gen_icon.py OUT.iconset
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_objects import Canvas  # noqa: E402
from pngio import write_png  # noqa: E402

N = 64

INK = (24, 18, 30)
NAVY = (34, 40, 96)
NAVY_L = (58, 70, 150)
GOLD = (232, 196, 72)
GOLD_D = (164, 124, 40)
GLOW = (250, 236, 150)
RED = (176, 40, 44)
RED_L = (226, 84, 76)


def shield(c):
    """A rounded shield, top flat, tapering to a point."""
    for y in range(4, 61):
        if y < 36:
            half = 26
        else:
            half = int(26 * math.cos((y - 36) / 25 * math.pi / 2))
        for x in range(32 - half, 32 + half):
            edge = x in (32 - half, 32 + half - 1) or y == 4 or y == 60
            inner = abs(x - 31.5) < half - 3 and 7 <= y
            c.set(x, y, GOLD_D if edge else (NAVY_L if not inner else NAVY))


def scepter(c):
    for y in range(18, 56):
        c.set(31, y, GOLD)
        c.set(32, y, GOLD_D)
    c.rect(27, 24, 10, 2, GOLD)
    c.rect(28, 50, 8, 2, GOLD_D)
    c.disc(32, 15, 6, GOLD)
    c.disc(32, 15, 4, RED)
    c.set(30, 13, RED_L)
    c.set(31, 12, GLOW)
    for dx in (-6, 0, 6):
        c.tri(32 + dx - 2, 10, 32 + dx, 5, 32 + dx + 2, 10, GOLD)


def resize(px, size):
    """The 64x64 art at `size`: nearest-neighbour up, box-averaged down."""
    out = bytearray(size * size * 4)
    if size >= N:
        k = size // N
        for y in range(size):
            for x in range(size):
                o = ((y // k) * N + x // k) * 4
                d = (y * size + x) * 4
                out[d:d + 4] = px[o:o + 4]
        return bytes(out)
    k = N // size
    for y in range(size):
        for x in range(size):
            acc = [0, 0, 0, 0]
            for yy in range(y * k, y * k + k):
                for xx in range(x * k, x * k + k):
                    o = (yy * N + xx) * 4
                    a = px[o + 3]
                    for ch in range(3):
                        acc[ch] += px[o + ch] * a
                    acc[3] += a
            d = (y * size + x) * 4
            if acc[3]:
                out[d:d + 3] = bytes(round(acc[ch] / acc[3]) for ch in range(3))
            out[d + 3] = round(acc[3] / (k * k))
    return bytes(out)


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    c = Canvas(N, N, (0, 0, 0, 0))
    shield(c)
    scepter(c)
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for size in (16, 32, 128, 256, 512):
        write_png(os.path.join(out, f"icon_{size}x{size}.png"), size, size,
                  resize(c.px, size))
        write_png(os.path.join(out, f"icon_{size}x{size}@2x.png"), size * 2, size * 2,
                  resize(c.px, size * 2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
