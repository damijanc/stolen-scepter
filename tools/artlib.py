"""A small anti-aliased, gradient-shaded raster toolkit for modern 2D art.

Float RGBA canvas with signed-distance-field shapes, one-pixel coverage
anti-aliasing, soft shadows and value noise. Renders at whatever pixel size a
caller asks for (the modern pack authors at 4x: 192x136 per 48x34 cell).

No dependencies; `write_png` comes from `upscale`.
"""
import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pngio import write_png  # noqa: E402


def clamp(x, lo=0.0, hi=1.0):
    return lo if x < lo else hi if x > hi else x


def mix(a, b, t):
    return (a[0] + (b[0] - a[0]) * t,
            a[1] + (b[1] - a[1]) * t,
            a[2] + (b[2] - a[2]) * t)


def rgb(hexcode):
    return (int(hexcode[1:3], 16) / 255.0,
            int(hexcode[3:5], 16) / 255.0,
            int(hexcode[5:7], 16) / 255.0)


def smoothstep(e0, e1, x):
    t = clamp((x - e0) / (e1 - e0) if e1 != e0 else 0.0)
    return t * t * (3 - 2 * t)


def _hash(x, y, seed):
    h = (x * 374761393 + y * 668265263 + seed * 2246822519) & 0xFFFFFFFF
    h = ((h ^ (h >> 13)) * 1274126177) & 0xFFFFFFFF
    return ((h >> 16) & 0xFFFF) / 65535.0


def noise(x, y, seed):
    gx, gy = math.floor(x), math.floor(y)
    fx, fy = x - gx, y - gy
    fx, fy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = _hash(gx, gy, seed)
    b = _hash(gx + 1, gy, seed)
    c = _hash(gx, gy + 1, seed)
    d = _hash(gx + 1, gy + 1, seed)
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy


def fbm(x, y, seed, octaves=3):
    total, amp, freq, norm = 0.0, 1.0, 1.0, 0.0
    for _ in range(octaves):
        total += noise(x * freq, y * freq, seed) * amp
        norm += amp
        amp *= 0.5
        freq *= 2.0
    return total / norm


class Image:
    def __init__(self, w, h, bg=(0.0, 0.0, 0.0, 0.0)):
        self.w, self.h = w, h
        self.px = list(bg) * (w * h)

    def _over(self, i, color, alpha):
        if alpha <= 0:
            return
        r, g, b = color
        dr, dg, db, da = self.px[i], self.px[i + 1], self.px[i + 2], self.px[i + 3]
        na = alpha + da * (1 - alpha)
        if na <= 0:
            self.px[i:i + 4] = [0, 0, 0, 0]
            return
        self.px[i] = (r * alpha + dr * da * (1 - alpha)) / na
        self.px[i + 1] = (g * alpha + dg * da * (1 - alpha)) / na
        self.px[i + 2] = (b * alpha + db * da * (1 - alpha)) / na
        self.px[i + 3] = na

    def blend(self, x, y, color, alpha):
        if 0 <= x < self.w and 0 <= y < self.h:
            self._over((y * self.w + x) * 4, color, clamp(alpha))

    def get(self, x, y):
        i = (y * self.w + x) * 4
        return self.px[i:i + 4]

    def set_px(self, x, y, rgba):
        i = (y * self.w + x) * 4
        self.px[i:i + 4] = list(rgba)

    def paint_sdf(self, sdf, bbox, color, alpha=1.0, blur=0.0):
        callable_color = callable(color)
        x0, y0, x1, y1 = bbox
        for y in range(max(0, int(y0)), min(self.h, int(y1) + 1)):
            for x in range(max(0, int(x0)), min(self.w, int(x1) + 1)):
                d = sdf(x + 0.5, y + 0.5)
                if d > blur + 2:
                    continue
                cov = 1 / (1 + math.exp(d / max(blur, 1e-3))) if blur > 0 else clamp(0.5 - d)
                if cov > 0:
                    c = color(x + 0.5, y + 0.5) if callable_color else color
                    self._over((y * self.w + x) * 4, c, cov * alpha)

    def vertical_gradient(self, x, y, w, h, top, bottom, alpha=1.0):
        for yy in range(y, y + h):
            if yy < 0 or yy >= self.h:
                continue
            t = (yy - y) / max(1, h - 1)
            c = mix(top, bottom, t)
            for xx in range(x, x + w):
                if 0 <= xx < self.w:
                    self._over((yy * self.w + xx) * 4, c, alpha)

    def fill_poly(self, points, color, alpha=1.0):
        callable_color = callable(color)
        xs = [p[0] for p in points]
        ys = [p[1] for p in points]
        x0, x1 = max(0, int(min(xs))), min(self.w - 1, int(max(xs)) + 1)
        y0, y1 = max(0, int(min(ys))), min(self.h - 1, int(max(ys)) + 1)
        n = len(points)
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                hits = 0
                for s in range(4):
                    for t in range(4):
                        pxx, pyy = x + (s + 0.5) / 4, y + (t + 0.5) / 4
                        inside = False
                        j = n - 1
                        for i in range(n):
                            xi, yi = points[i]
                            xj, yj = points[j]
                            if (yi > pyy) != (yj > pyy) and pxx < (xj - xi) * (pyy - yi) / (yj - yi) + xi:
                                inside = not inside
                            j = i
                        hits += inside
                if hits:
                    c = color(x + 0.5, y + 0.5) if callable_color else color
                    self._over((y * self.w + x) * 4, c, alpha * hits / 16)

    def noise_overlay(self, color, amount, scale=3.0, seed=0, region=None):
        x0, y0, x1, y1 = region or (0, 0, self.w, self.h)
        for y in range(max(0, y0), min(self.h, y1)):
            for x in range(max(0, x0), min(self.w, x1)):
                n = fbm(x / scale, y / scale, seed)
                self._over((y * self.w + x) * 4, color, amount * n)

    def save(self, path):
        out = bytearray(self.w * self.h * 4)
        for i in range(self.w * self.h):
            for c in range(4):
                out[i * 4 + c] = int(clamp(self.px[i * 4 + c]) * 255 + 0.5)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        write_png(path, self.w, self.h, bytes(out))


def circle(cx, cy, r):
    return (lambda x, y: math.hypot(x - cx, y - cy) - r,
            (cx - r - 2, cy - r - 2, cx + r + 2, cy + r + 2))


def capsule(x0, y0, x1, y1, r):
    def sdf(x, y):
        dx, dy = x1 - x0, y1 - y0
        t = clamp(((x - x0) * dx + (y - y0) * dy) / (dx * dx + dy * dy), 0, 1)
        return math.hypot(x - (x0 + dx * t), y - (y0 + dy * t)) - r
    return sdf, (min(x0, x1) - r - 2, min(y0, y1) - r - 2,
                 max(x0, x1) + r + 2, max(y0, y1) + r + 2)


def round_rect(x, y, w, h, r):
    def sdf(px, py):
        dx = abs(px - (x + w / 2)) - (w / 2 - r)
        dy = abs(py - (y + h / 2)) - (h / 2 - r)
        outside = math.hypot(max(dx, 0), max(dy, 0))
        return outside + min(max(dx, dy), 0) - r
    return sdf, (x - 2, y - 2, x + w + 2, y + h + 2)
