"""Measure continent maps as numbers: how much is water, what the land is
made of, how wide the open paths are, how big the walls are.

    python3 tools/measure_maps.py --reference      # the original maps, local only
    python3 tools/measure_maps.py                  # our pack's maps

The originals live in the local, ignored `reference/` clone and are never
copied. Only these statistics are kept (`--write` saves the reference numbers
to `tools/map_targets.json`), the same way `asset_layout.json` keeps
measurements of the original art and nothing else. `gen_maps.py` tunes its
own maps toward them; no tile layout crosses over.

Stdlib only.
"""
import argparse
import json
import os
import sys
from collections import Counter, deque

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REF = os.path.join(ROOT, "reference", "assets", "kings-bounty")
PACK = os.path.join(ROOT, "packs", "stolen-scepter")
TARGETS = os.path.join(ROOT, "tools", "map_targets.json")

WALKABLE = {"grass", "desert"}
OBSTACLE = {"forest", "mountain"}
N8 = [(dx, dy) for dx in (-1, 0, 1) for dy in (-1, 0, 1) if dx or dy]


def load(pack_dir):
    with open(os.path.join(pack_dir, "game.json")) as fh:
        pack = json.load(fh)
    codes = pack["tile_codes"]
    out = []
    for zone in pack["zones"]:
        with open(os.path.join(pack_dir, zone["map"])) as fh:
            rows = [line.rstrip("\n") for line in fh if not line.startswith("#")]
        grid = [[codes[ch]["terrain"] for ch in row] for row in rows]
        out.append((zone["id"], grid))
    return out


def comps(grid, kinds):
    h, w = len(grid), len(grid[0])
    seen, sizes = set(), []
    for y in range(h):
        for x in range(w):
            if (x, y) in seen or grid[y][x] not in kinds:
                continue
            n, q = 0, deque([(x, y)])
            seen.add((x, y))
            while q:
                cx, cy = q.popleft()
                n += 1
                for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                    nx, ny = cx + dx, cy + dy
                    if 0 <= nx < w and 0 <= ny < h and (nx, ny) not in seen \
                            and grid[ny][nx] in kinds:
                        seen.add((nx, ny))
                        q.append((nx, ny))
            sizes.append(n)
    return sorted(sizes, reverse=True)


def clearance(grid):
    """For each walkable tile, the chessboard distance to the nearest tile that
    is not walkable: 1 is a one-tile path, 2 a three-wide road, and so on."""
    h, w = len(grid), len(grid[0])
    dist = {}
    q = deque()
    for y in range(h):
        for x in range(w):
            if grid[y][x] not in WALKABLE:
                dist[x, y] = 0
                q.append((x, y))
    while q:
        x, y = q.popleft()
        for dx, dy in N8:
            n = (x + dx, y + dy)
            if 0 <= n[0] < w and 0 <= n[1] < h and n not in dist:
                dist[n] = dist[x, y] + 1
                q.append(n)
    return [dist[x, y] for y in range(h) for x in range(w) if grid[y][x] in WALKABLE]


def measure(grid):
    cells = [t for row in grid for t in row]
    count = Counter(cells)
    land = len(cells) - count["water"]
    clear = clearance(grid)
    cc = Counter(clear)
    walls = comps(grid, OBSTACLE)
    # Obstacle tiles touching a different non-grass terrain: how much the
    # original relies on terrains meeting directly.
    h, w = len(grid), len(grid[0])
    direct = 0
    for y in range(h):
        for x in range(w):
            t = grid[y][x]
            if t in ("grass",):
                continue
            if any(0 <= x + dx < w and 0 <= y + dy < h
                   and grid[y + dy][x + dx] not in (t, "grass") for dx, dy in N8):
                direct += 1
    nongrass = len(cells) - count["grass"]
    return {
        "water": round(count["water"] / len(cells), 3),
        "land": {t: round(count[t] / land, 3) for t in ("grass", "forest", "mountain", "desert")},
        "clearance": {str(k): round(cc[k] / len(clear), 3) for k in sorted(cc)},
        "mean_clearance": round(sum(clear) / len(clear), 2),
        "walls": len([s for s in walls if s >= 3]),
        "wall_mean": round(sum(walls) / len(walls), 1) if walls else 0,
        "wall_largest": walls[0] if walls else 0,
        "landmasses": len([s for s in comps(grid, WALKABLE | OBSTACLE) if s >= 4]),
        "direct_contact": round(direct / nongrass, 3) if nongrass else 0,
    }


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--reference", action="store_true", help="measure the originals")
    ap.add_argument("--write", action="store_true", help="save to tools/map_targets.json")
    args = ap.parse_args()
    if args.reference and not os.path.isdir(REF):
        sys.exit(f"no reference maps at {REF}")
    maps = load(REF if args.reference else PACK)
    result = {}
    for i, (zid, grid) in enumerate(maps):
        # The originals are kept anonymous: continent 1..4 in pack order.
        key = f"continent_{i + 1}" if args.reference else zid
        result[key] = measure(grid)
        print(key, json.dumps(result[key]))
    if args.write:
        if not args.reference:
            sys.exit("--write saves the reference measurements only")
        with open(TARGETS, "w") as fh:
            json.dump(result, fh, indent=1)
            fh.write("\n")
        print(f"wrote {os.path.relpath(TARGETS, ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
