"""Measure the functional geometry of the original colour-keyed art.

Reads the extracted DOS art (`game-assets/extracted/art/...`, primary Mac only)
and writes `tools/asset_layout.json`: per pack path, where the subject sits in
its cell -- bounding box, ground baseline, horizontal centre, foot centre and
size as a fraction of the cell -- plus, per animated group, the frame-to-frame
motion relative to frame 0.

The output is numbers only (no pixels), so it is safe to commit and lets the
generation Mac register AI sprites without the originals:

    python3 tools/asset_layout.py            # write tools/asset_layout.json
    python3 tools/asset_layout.py --check    # exit 1 if the committed file is stale

Conventions:
  * bbox is [x0, y0, x1, y1] with x1/y1 EXCLUSIVE (PIL getbbox style), in
    native pixels; `baseline` is y1, the row just below the subject's feet.
  * cx/cy are the subject's pixel centroid; foot_cx is the centroid of the
    bottom FOOT_ROWS rows of the bbox -- the anchor to register on, since a
    raised weapon moves cx but not the feet.
  * Blobs smaller than DUST pixels (8-connected) are ignored.
  * A blob SHADOW_GAP+ rows below the largest one is a detached ground shadow
    (the hovering sprites): it is excluded from the subject and recorded as
    `shadow` [x0, y0, x1, y1], so `baseline` is where the subject itself ends.
  * `mask` records how the subject was separated from its background:
      alpha        the PNG's own 0/255 alpha
      ground_diff  pixels differing from the ground tile the art is drawn on
                   (combat obstacles and walls, and the opaque map-object
                   tiles, bake their ground in)
      full         a framed opaque picture (class portraits): the whole cell
"""
import argparse
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pngio import read_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "tools", "hd_assets.json")
EXTRACTED = os.path.join(ROOT, "game-assets", "extracted")
OUT = os.path.join(ROOT, "tools", "asset_layout.json")

DUST = 4
FOOT_ROWS = 3
SHADOW_GAP = 2

# Map tiles that are an object on a ground, and the ground they bake in. The
# bridges are left out: their water differs from water.png on every pixel, so
# a ground diff measures the whole cell.
OBJECT_TILES = {
    "artifact_chest": "grass", "artifact_ring": "grass", "chest": "grass",
    "sign": "grass", "town": "grass", "wandering_army": "grass",
    "dwelling_plains": "grass", "dwelling_forest": "grass",
    "dwelling_hills": "grass", "dwelling_dungeon": "grass",
}
ANIMATED = ("art/troops/", "art/villains/", "art/sprites/")


def load(rel):
    w, h, rgba = read_png(os.path.join(EXTRACTED, rel))
    return w, h, rgba


def mask_for(asset):
    """(mask method, w, h, list of bools) or None if the slot is not measured."""
    path = asset["path"]
    stem = os.path.basename(path)[:-4]
    cat = asset["category"]
    if cat == "classes":
        w, h, _ = load(path)
        return "full", w, h, [True] * (w * h)
    ground = None
    if cat == "combat" and stem != "field_grass":
        w, h, rgba = load(path)
        if any(rgba[i] != 255 for i in range(3, len(rgba), 4)):
            return "alpha", w, h, [rgba[i * 4 + 3] >= 128 for i in range(w * h)]
        ground = "art/combat/field_grass.png"
    elif cat == "tiles" and stem in OBJECT_TILES:
        ground = f"art/tiles/{OBJECT_TILES[stem]}.png"
    elif path.startswith(ANIMATED):
        w, h, rgba = load(path)
        return "alpha", w, h, [rgba[i * 4 + 3] >= 128 for i in range(w * h)]
    if ground is None:
        return None
    w, h, rgba = load(path)
    gw, gh, grgba = load(ground)
    if (gw, gh) != (w, h):
        return None
    return "ground_diff", w, h, [rgba[i * 4:i * 4 + 3] != grgba[i * 4:i * 4 + 3]
                                 for i in range(w * h)]


def blobs(w, h, mask):
    """8-connected blobs of at least DUST pixels, largest first."""
    seen = bytearray(w * h)
    out = []
    for i in range(w * h):
        if seen[i] or not mask[i]:
            continue
        stack, blob = [i], []
        seen[i] = 1
        while stack:
            j = stack.pop()
            blob.append(j)
            x, y = j % w, j // w
            for dy in (-1, 0, 1):
                for dx in (-1, 0, 1):
                    nx, ny = x + dx, y + dy
                    if 0 <= nx < w and 0 <= ny < h:
                        k = ny * w + nx
                        if mask[k] and not seen[k]:
                            seen[k] = 1
                            stack.append(k)
        if len(blob) >= DUST:
            out.append(blob)
    out.sort(key=len, reverse=True)
    return out


def box(pts):
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    return min(xs), min(ys), max(xs) + 1, max(ys) + 1


def measure(method, w, h, mask):
    found = [[(j % w, j // w) for j in b] for b in blobs(w, h, mask)]
    if not found:
        return None
    # A blob at least SHADOW_GAP rows below the largest one is a detached
    # ground shadow (the flying sprites hover over one). It is not the
    # subject: keep it apart so the baseline is the subject's own bottom.
    # demons_03 draws its legs detached one row under the body, which the gap
    # rules out.
    main_bottom = box(found[0])[3]
    below = [b for b in found[1:] if box(b)[1] >= main_bottom + SHADOW_GAP]
    shadow = [p for b in below for p in b]
    pts = [p for b in found if b not in below for p in b]
    xs = [p[0] for p in pts]
    ys = [p[1] for p in pts]
    x0, y0, x1, y1 = box(pts)
    feet = [x for x, y in pts if y >= y1 - FOOT_ROWS]
    rec = {
        "mask": method,
        "cell": [w, h],
        "bbox": [x0, y0, x1, y1],
        "baseline": y1,
        "cx": round(sum(xs) / len(xs), 2),
        "cy": round(sum(ys) / len(ys), 2),
        "foot_cx": round(sum(feet) / len(feet), 2),
        "frac": [round((x1 - x0) / w, 4), round((y1 - y0) / h, 4)],
        "fill": round(len(pts) / (w * h), 4),
    }
    if shadow:
        rec["shadow"] = list(box(shadow))
    return rec


def group_of(path):
    if not path.startswith(ANIMATED):
        return None
    stem = os.path.basename(path)[:-4]
    head, _, tail = stem.rpartition("_")
    if not head or not tail.isdigit():
        return None
    return f"{os.path.dirname(path)}/{head}"


def summarise(frames):
    """Union box and per-frame motion relative to frame 0."""
    f0 = frames[0][1]
    boxes = [f["bbox"] for _, f in frames]
    union = [min(b[0] for b in boxes), min(b[1] for b in boxes),
             max(b[2] for b in boxes), max(b[3] for b in boxes)]
    motion = []
    for _, f in frames:
        motion.append({
            "dcx": round(f["cx"] - f0["cx"], 2),
            "dcy": round(f["cy"] - f0["cy"], 2),
            "dfoot_cx": round(f["foot_cx"] - f0["foot_cx"], 2),
            "dbaseline": f["baseline"] - f0["baseline"],
            "dheight": (f["bbox"][3] - f["bbox"][1]) - (f0["bbox"][3] - f0["bbox"][1]),
        })
    heights = [f["bbox"][3] - f["bbox"][1] for _, f in frames]
    return {
        "frames": [p for p, _ in frames],
        "union_bbox": union,
        "baseline": max(f["baseline"] for _, f in frames),
        "max_height_frac": round(max(heights) / f0["cell"][1], 4),
        "motion": motion,
    }


def build():
    with open(MANIFEST) as fh:
        assets = json.load(fh)["assets"]
    slots, groups = {}, {}
    for asset in assets:
        if not os.path.exists(os.path.join(EXTRACTED, asset["path"])):
            continue
        m = mask_for(asset)
        if m is None:
            continue
        rec = measure(*m)
        if rec is None:
            continue
        slots[asset["path"]] = rec
        g = group_of(asset["path"])
        if g:
            groups.setdefault(g, []).append((asset["path"], rec))
    return {
        "schema": 1,
        "generated_by": "tools/asset_layout.py",
        "description": ("Functional geometry of the original colour-keyed art (numbers "
                        "only). bbox is [x0,y0,x1,y1], x1/y1 exclusive; baseline = y1. "
                        "Used by tools/gen_ai.py --registration spec."),
        "conventions": {"dust_px": DUST, "foot_rows": FOOT_ROWS, "shadow_gap": SHADOW_GAP},
        "slots": dict(sorted(slots.items())),
        "groups": {g: summarise(sorted(fr)) for g, fr in sorted(groups.items())},
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true",
                    help="compare against the committed file instead of writing")
    args = ap.parse_args()
    if not os.path.isdir(os.path.join(EXTRACTED, "art")):
        sys.exit(f"no extracted art at {os.path.relpath(EXTRACTED, ROOT)}/art "
                 "(this runs on the primary Mac only)")
    doc = build()
    text = json.dumps(doc, indent=1) + "\n"
    if args.check:
        with open(OUT) as fh:
            same = fh.read() == text
        print("asset_layout.json up to date" if same else "asset_layout.json is STALE")
        return 0 if same else 1
    with open(OUT, "w") as fh:
        fh.write(text)
    print(f"{os.path.relpath(OUT, ROOT)}: {len(doc['slots'])} slots, "
          f"{len(doc['groups'])} animated groups")
    return 0


if __name__ == "__main__":
    sys.exit(main())
