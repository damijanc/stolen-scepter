"""Generate the four continent maps and their placements for the pack.

Every map is our own: land from seeded noise is filled with forest and
mountain walls, grass is carved through it as clearings and winding paths
(`raw_terrain`), then it is cleaned until every tile can be drawn with the 12
edge tiles each terrain has, and the
placements (castles, towns, chests, wandering armies, signs, spawns, the magic
alcove) are put where the terrain allows and the hero can reach them.

    python3 tools/gen_maps.py                 # all continents, default seeds
    python3 tools/gen_maps.py --only highmoor --seed 7
    python3 tools/gen_maps.py --preview DIR   # also write PNG previews to DIR

Writes `packs/stolen-scepter/maps/<id>.dat` (64 rows of 64 tile codes),
`packs/stolen-scepter/data/zones/<id>.json` (placement coordinates; names and
ids are given by the pack builder), and `packs/stolen-scepter/data/tile_codes.json`.
Curate by trying seeds (`--seed`) and keeping the best; the chosen seeds live
in CONTINENTS below so the committed maps can be regenerated exactly.

Engine facts this follows (engine/src/map.rs, game.rs):
  * A map is text: one row per line, one byte per tile, looked up in the
    pack's `tile_codes`. A '#' line before the first row is a comment, so '#'
    is never used as a code.
  * Edge tiles exist only for a terrain bordering GRASS, in 12 shapes: 4 outer
    corners, 4 inner corners, 4 sides. So different terrains (water included)
    never touch directly, and any tile whose neighbours match none of the 12
    shapes is reshaped until all do.
  * A castle is 3x2, centred on its gate at the bottom middle; only the gate
    is walkable. A town is one tile with a boat dock on adjacent water.
  * Each zone's chest list is also the salt pool the engine turns into
    artifacts, orbs, maps, teleport caves, dwellings and friendly armies, so
    every chest slot is a reachable grass tile.

Stdlib only.
"""
import argparse
import heapq
import json
import math
import os
import random
import sys
from collections import deque

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pngio import write_png  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PACK = os.path.join(ROOT, "packs", "stolen-scepter")
SIZE = 64

WATER, GRASS, FOREST, MOUNTAIN, DESERT = "W", "G", "F", "M", "D"
WALKABLE = {GRASS, DESERT}
# Which terrain keeps its tile where two meet (higher wins). Forest and
# mountain may meet water directly: their shore tiles draw that edge.
RANK = {WATER: 4, MOUNTAIN: 3, FOREST: 2, DESERT: 1}
WALLS = {FOREST, MOUNTAIN}

# Totals match what the engine's salting is built around: 26 towns (spells
# are pinned to towns) and 27 castles (17 villains plus the King and monster
# holds). Chests also feed the salt pool, which needs at least 21 slots.
CONTINENTS = [
    dict(id="highmoor", home=True, seed=11, land=0.58, falloff=1.6, freq=3.0,
         patch=4.0, forest=0.8, mountain=1.0, desert=0.0, clearings=14, loops=0.3,
         towns=11, castles=12, signs=12, chests=70, armies=30),
    dict(id="greenreach", home=False, seed=23, land=0.66, falloff=1.6, freq=3.0,
         patch=4.0, forest=1.0, mountain=0.9, desert=0.0, clearings=14, loops=0.25,
         towns=6, castles=6, signs=10, chests=60, armies=25),
    dict(id="isleward", home=False, seed=37, land=0.42, falloff=0.4, freq=6.0,
         islands=True, patch=5.0, forest=1.0, mountain=0.6, desert=0.0, clearings=14,
         loops=0.25, towns=6, castles=6, signs=8, chests=45, armies=12),
    dict(id="dunemark", home=False, seed=41, land=0.70, falloff=1.6, freq=3.0,
         patch=4.5, forest=0.2, mountain=1.05, desert=1.0, clearings=10, loops=0.3,
         towns=3, castles=3, signs=8, chests=50, armies=15),
]

# --- tile codes ------------------------------------------------------------

OFFSETS = {"N": (0, -1), "S": (0, 1), "E": (1, 0), "W": (-1, 0),
           "NE": (1, -1), "NW": (-1, -1), "SE": (1, 1), "SW": (-1, 1)}

# Edge shape -> edge number, as the 12 edge tiles are numbered (measured from
# which neighbours each edge tile faces: outer corners, inner corners, sides).
LAND_EDGE = {("outer", "NW"): 1, ("outer", "SW"): 2, ("outer", "NE"): 3,
             ("outer", "SE"): 4, ("inner", "SE"): 5, ("inner", "NE"): 6,
             ("inner", "SW"): 7, ("inner", "NW"): 8, ("side", "E"): 9,
             ("side", "W"): 10, ("side", "N"): 11, ("side", "S"): 12}
WATER_EDGE = {("outer", "NE"): 0, ("outer", "NW"): 1, ("outer", "SW"): 2,
              ("outer", "SE"): 3, ("inner", "SE"): 4, ("inner", "NE"): 5,
              ("inner", "SW"): 6, ("inner", "NW"): 7, ("side", "E"): 8,
              ("side", "W"): 9, ("side", "N"): 10, ("side", "S"): 11}

BASE_CODES = {
    ".": dict(art="grass", terrain="grass"),
    ",": dict(art="grass_variant", terrain="grass"),
    "~": dict(art="water", terrain="water"),
    "%": dict(art="forest", terrain="forest", blocks_foot=True),
    "^": dict(art="mountain", terrain="mountain", blocks_foot=True),
    ":": dict(art="desert", terrain="desert"),
}
BASE_OF = {GRASS: ".", WATER: "~", FOREST: "%", MOUNTAIN: "^", DESERT: ":"}
# Edge codes: a run of letters per terrain, edge number order.
EDGE_CODES = {FOREST: "ABCDEFGHIJKL", MOUNTAIN: "MNOPQRSTUVWX",
              DESERT: "abcdefghijkl", WATER: "mnopqrstuvwx"}
# Shore codes: a wall edge drawn over water instead of grass, same numbering.
SHORE_CODES = {FOREST: "0123456789YZ", MOUNTAIN: "yz!$&*+-=?@<"}
EDGE_ART = {FOREST: "forest", MOUNTAIN: "mountain", DESERT: "desert", WATER: "water"}
TERRAIN_NAME = {FOREST: "forest", MOUNTAIN: "mountain", DESERT: "desert", WATER: "water"}


def tile_codes():
    codes = dict(BASE_CODES)
    for t, letters in EDGE_CODES.items():
        for i, code in enumerate(letters):
            num = i if t == WATER else i + 1
            entry = dict(art=f"{EDGE_ART[t]}_edge_{num:02d}", terrain=TERRAIN_NAME[t])
            if t in (FOREST, MOUNTAIN):
                entry["blocks_foot"] = True
            codes[code] = entry
    for t, letters in SHORE_CODES.items():
        for i, code in enumerate(letters):
            codes[code] = dict(art=f"{EDGE_ART[t]}_shore_{i + 1:02d}",
                               terrain=TERRAIN_NAME[t], blocks_foot=True)
    return codes


# --- noise -----------------------------------------------------------------

def value_noise(rng, cells):
    grid = [[rng.random() for _ in range(cells + 2)] for _ in range(cells + 2)]

    def sample(u, v):
        x, y = u * cells, v * cells
        x0, y0 = int(x), int(y)
        tx, ty = x - x0, y - y0
        sx, sy = tx * tx * (3 - 2 * tx), ty * ty * (3 - 2 * ty)
        a = grid[y0][x0] * (1 - sx) + grid[y0][x0 + 1] * sx
        b = grid[y0 + 1][x0] * (1 - sx) + grid[y0 + 1][x0 + 1] * sx
        return a * (1 - sy) + b * sy
    return sample


def fbm(rng, freq, octaves=4):
    layers = [value_noise(rng, max(2, int(freq * 2 ** o))) for o in range(octaves)]

    def sample(u, v):
        total, amp, norm = 0.0, 1.0, 0.0
        for layer in layers:
            total += layer(u, v) * amp
            norm += amp
            amp *= 0.5
        return total / norm
    return sample


def quantile(values, frac):
    s = sorted(values)
    if not s:
        return 0.0
    return s[min(len(s) - 1, max(0, int(len(s) * frac)))]


# --- terrain ---------------------------------------------------------------

def raw_terrain(cfg, rng):
    """Land from noise, filled with walls, then grass carved through it.

    The originals (measured as numbers by `measure_maps.py`, kept in
    `map_targets.json`) are solid forest and mountain cut by narrow paths and
    small clearings, so this builds terrain that way round: every land cell
    starts as a wall chosen by biome noise, then clearings (castle-sized ones
    first) and coves are opened and joined by winding paths, with a few extra
    joins for loops. Desert is a floor biome: it stays open, as a desert
    continent's basins are.

    It works on a half-resolution grid of 2x2 tile blocks. Any shape made of
    aligned 2x2 blocks can be drawn with the 12 edge tiles, so the walls come
    out whole instead of being eroded by `clean`; paths are two tiles wide.
    """
    m = SIZE // 2
    land_n = fbm(rng, cfg["freq"])
    biome = {t: fbm(rng, cfg.get("patch", 5.0)) for t in (FOREST, MOUNTAIN, DESERT)}
    weight = {FOREST: cfg["forest"], MOUNTAIN: cfg["mountain"], DESERT: cfg["desert"]}
    height = {}
    for y in range(m):
        for x in range(m):
            u, v = (x + 0.5) / m, (y + 0.5) / m
            d = math.hypot(u - 0.5, v - 0.5) * 2
            height[x, y] = land_n(u, v) - cfg["falloff"] * 0.25 * d * d
    cut = quantile(list(height.values()), 1 - cfg["land"])
    cell = [[WATER] * m for _ in range(m)]
    land = [(x, y) for (x, y), h in height.items() if h > cut
            and 1 <= x < m - 1 and 1 <= y < m - 1]
    land_set = set(land)
    for x, y in land:
        u, v = (x + 0.5) / m, (y + 0.5) / m
        cell[y][x] = max((t for t in weight if weight[t] > 0),
                         key=lambda t: biome[t](u, v) * weight[t])

    def get(x, y):
        return cell[y][x] if 0 <= x < m and 0 <= y < m else None

    def opened(x, y):
        if (x, y) in land_set and cell[y][x] != DESERT:
            cell[y][x] = GRASS

    # Castle clearings first (2x2 blocks: 4x4 tiles hold the 3x2 castle and
    # the tile before its gate), then small clearings, then coves.
    roomy = [(x, y) for x, y in land
             if all((x + dx, y + dy) in land_set for dx in (-1, 0, 1, 2) for dy in (-1, 0, 1, 2))]
    rng.shuffle(roomy)
    anchors = []
    for x, y in roomy:
        if len(anchors) >= cfg["castles"] + 1:
            break
        if all(abs(x - ax) + abs(y - ay) >= 5 for ax, ay in anchors):
            anchors.append((x, y))
            for dx in (0, 1):
                for dy in (0, 1):
                    opened(x + dx, y + dy)
    rest = [c for c in land if c not in anchors]
    rng.shuffle(rest)
    for x, y in rest:
        if len(anchors) >= cfg["castles"] + cfg["clearings"]:
            break
        if all(abs(x - ax) + abs(y - ay) >= cfg.get("spacing", 4) for ax, ay in anchors):
            anchors.append((x, y))
            opened(x, y)
            if rng.random() < 0.5:
                opened(x + rng.choice((-1, 1)), y)
    shore = [(x, y) for x, y in land
             if any(get(x + dx, y + dy) == WATER for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
    rng.shuffle(shore)
    coves = []
    for x, y in shore:
        if len(coves) >= cfg["towns"] + cfg.get("coves", 4):
            break
        if all(abs(x - cx) + abs(y - cy) >= 3 for cx, cy in coves):
            coves.append((x, y))
            opened(x, y)
    anchors += coves

    # Paths: a minimum spanning tree over each landmass's clearings, walked on
    # a noisy cost so they wind, plus extra joins for loops.
    wiggle = fbm(rng, 6.0)

    def cost(x, y):
        if cell[y][x] in (GRASS, DESERT):
            return 1.0
        return 2.0 + 6.0 * wiggle((x + 0.5) / m, (y + 0.5) / m)

    def carve(a, b):
        dist, prev = {a: 0.0}, {}
        heap = [(0.0, a)]
        while heap:
            d, cur = heapq.heappop(heap)
            if cur == b:
                break
            if d > dist[cur]:
                continue
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                n = (cur[0] + dx, cur[1] + dy)
                if n in land_set:
                    nd = d + cost(*n)
                    if nd < dist.get(n, 1e18):
                        dist[n] = nd
                        prev[n] = cur
                        heapq.heappush(heap, (nd, n))
        cur = b
        while cur in prev:
            opened(*cur)
            cur = prev[cur]

    seen = set()
    for x, y in land:
        if (x, y) in seen:
            continue
        mass, q = set(), deque([(x, y)])
        seen.add((x, y))
        while q:
            cx, cy = q.popleft()
            mass.add((cx, cy))
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                n = (cx + dx, cy + dy)
                if n in land_set and n not in seen:
                    seen.add(n)
                    q.append(n)
        pts = [a for a in anchors if a in mass]
        if len(pts) < 2:
            continue
        tree, todo, edges = [pts[0]], pts[1:], []
        while todo:
            a, b = min(((a, b) for a in tree for b in todo),
                       key=lambda e: abs(e[0][0] - e[1][0]) + abs(e[0][1] - e[1][1]))
            edges.append((a, b))
            tree.append(b)
            todo.remove(b)
        for _ in range(int(len(pts) * cfg.get("loops", 0.25))):
            a = rng.choice(pts)
            near = sorted((p for p in pts if p != a),
                          key=lambda p: abs(p[0] - a[0]) + abs(p[1] - a[1]))[:3]
            edges.append((a, rng.choice(near)))
        for a, b in edges:
            carve(a, b)

    # Block-level rules, so the tiles need no erosion: two different walls (or
    # a wall and desert) never touch -- the lower-ranked one opens to grass --
    # and desert never meets water.
    for _ in range(50):
        changed = False
        for y in range(m):
            for x in range(m):
                t = cell[y][x]
                if t in (GRASS, WATER):
                    continue
                around = [get(x + dx, y + dy) for dx, dy in OFFSETS.values()]
                clash = any(RANK.get(n, -1) > RANK[t] and not (t in WALLS and n == WATER)
                            for n in around)
                if clash:
                    cell[y][x] = GRASS
                    changed = True
        if not changed:
            break

    grid = [[WATER] * SIZE for _ in range(SIZE)]
    for y in range(SIZE):
        for x in range(SIZE):
            grid[y][x] = cell[y // 2][x // 2]
    return grid


def at(grid, x, y):
    if 0 <= x < SIZE and 0 <= y < SIZE:
        return grid[y][x]
    return None


def pure_shore(grid, x, y):
    """A wall that meets the sea and nothing else: drawn with its shore tile,
    which shows the water's edge itself."""
    around = {at(grid, x + dx, y + dy) for dx, dy in OFFSETS.values()}
    return grid[y][x] in WALLS and WATER in around and not (around & {GRASS, DESERT})


def same_as_at(grid, t, x, y):
    """Whether the tile at (x, y) counts as more of `t` when shaping `t`.
    Water takes a pure-shore wall as more water, since the wall's shore tile
    draws that edge; a wall that also touches grass is drawn with its grass
    edge, so the water draws its own shoreline against it."""
    n = at(grid, x, y)
    if n is None or n == t:
        return True
    return t == WATER and n in WALLS and pure_shore(grid, x, y)


def shape(grid, x, y):
    """The edge shape of a non-grass tile: (kind, where), "interior", or None
    if no edge tile can draw it. Out-of-map neighbours count as the same. A
    wall bordering both grass and water is drawn with its grass edge (see
    `encode`), so its shape counts both."""
    t = grid[y][x]
    diff = {k for k, (dx, dy) in OFFSETS.items() if not same_as_at(grid, t, x + dx, y + dy)}
    orth = {k for k in diff if len(k) == 1}
    diag = diff - orth
    if not orth:
        if not diag:
            return "interior"
        if len(diag) == 1:
            return ("inner", next(iter(diag)))
        return None
    if len(orth) == 1:
        side = next(iter(orth))
        if all(side in d for d in diag):
            return ("side", side)
        return None
    if len(orth) == 2:
        a, b = sorted(orth, key="NSEW".index)
        if {a, b} in ({"N", "S"}, {"E", "W"}):
            return None
        corner = a + b  # "NE", "NW", "SE", "SW"
        if all(a in d or b in d for d in diag):
            return ("outer", corner)
        return None
    return None


def components(grid, kinds, cells=None):
    """4-connected components of tiles whose type is in `kinds`."""
    seen, out = set(), []
    pool = cells if cells is not None else [(x, y) for y in range(SIZE) for x in range(SIZE)]
    for start in pool:
        if start in seen or grid[start[1]][start[0]] not in kinds:
            continue
        comp, q = [], deque([start])
        seen.add(start)
        while q:
            x, y = q.popleft()
            comp.append((x, y))
            for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
                n = (x + dx, y + dy)
                if 0 <= n[0] < SIZE and 0 <= n[1] < SIZE and n not in seen \
                        and grid[n[1]][n[0]] in kinds:
                    seen.add(n)
                    q.append(n)
        out.append(comp)
    out.sort(key=len, reverse=True)
    return out


def clean(grid):
    """Reshape until every tile is drawable and terrains meet only via grass."""
    for _ in range(200):
        changed = False
        # Small blobs and lakes read as noise; fold them into grass.
        for t in (FOREST, MOUNTAIN, DESERT):
            for comp in components(grid, {t})[0:]:
                if len(comp) < 6:
                    for x, y in comp:
                        grid[y][x] = GRASS
                        changed = True
        for comp in components(grid, {WATER}):
            touches_edge = any(x in (0, SIZE - 1) or y in (0, SIZE - 1) for x, y in comp)
            if not touches_edge and len(comp) < 12:
                for x, y in comp:
                    grid[y][x] = GRASS
                    changed = True
        for y in range(SIZE):
            for x in range(SIZE):
                t = grid[y][x]
                if t == GRASS:
                    continue
                # Two different non-grass terrains may not touch. Only the
                # lower-ranked one gives way, so the seam between a forest and
                # a mountain range is one grass tile wide, not two.
                clash = any(RANK.get(n, -1) > RANK[t] and not (t in WALLS and n == WATER)
                            for n in (at(grid, x + dx, y + dy) for dx, dy in OFFSETS.values()))
                if clash or shape(grid, x, y) is None:
                    # A wall on the coast that cannot be drawn sinks into the
                    # sea; turning it to grass would leave its neighbours
                    # bordering grass and water at once, and unzip the coast.
                    wet = t in WALLS and any(at(grid, x + dx, y + dy) == WATER
                                             for dx, dy in OFFSETS.values())
                    grid[y][x] = WATER if wet else GRASS
                    changed = True
        if not changed:
            return grid
    raise RuntimeError("terrain did not settle")


def encode(grid, rng):
    rows = []
    for y in range(SIZE):
        row = []
        for x in range(SIZE):
            t = grid[y][x]
            if t == GRASS:
                row.append("," if rng.random() < 0.12 else ".")
                continue
            s = shape(grid, x, y)
            if s == "interior":
                row.append(BASE_OF[t])
            else:
                table = WATER_EDGE if t == WATER else LAND_EDGE
                num = table[s]
                # The shore set only where the wall meets water alone; where it
                # meets grass too, its grass edge and the water's own shoreline
                # read as a strip of beach.
                shore = pure_shore(grid, x, y)
                codes = SHORE_CODES[t] if shore else EDGE_CODES[t]
                row.append(codes[num if t == WATER else num - 1])
        rows.append("".join(row))
    return rows


# --- placements ------------------------------------------------------------

def walk_reach(grid, start):
    """Tiles reachable on foot from `start` (grass and desert)."""
    seen = {start}
    q = deque([start])
    while q:
        x, y = q.popleft()
        for dx, dy in OFFSETS.values():
            n = (x + dx, y + dy)
            if n not in seen and at(grid, *n) in WALKABLE:
                seen.add(n)
                q.append(n)
    return seen


def sea(grid):
    """Water connected to the map border: where boats sail."""
    comps = components(grid, {WATER})
    return set(next((c for c in comps if any(x in (0, SIZE - 1) or y in (0, SIZE - 1)
                                             for x, y in c)), []))


def spread(rng, candidates, count, min_dist, taken, keep_off=1):
    """Pick up to `count` candidates by farthest-point sampling, never within
    `keep_off` of anything in `taken` (a set of tiles)."""
    pool = [c for c in candidates
            if all((c[0] + dx, c[1] + dy) not in taken
                   for dx in range(-keep_off, keep_off + 1)
                   for dy in range(-keep_off, keep_off + 1))]
    rng.shuffle(pool)
    chosen = []
    while pool and len(chosen) < count:
        if not chosen:
            pick = pool[0]
        else:
            pick = max(pool, key=lambda c: min(abs(c[0] - p[0]) + abs(c[1] - p[1])
                                               for p in chosen))
            if min(abs(pick[0] - p[0]) + abs(pick[1] - p[1]) for p in chosen) < min_dist:
                break
        chosen.append(pick)
        pool = [c for c in pool if abs(c[0] - pick[0]) + abs(c[1] - pick[1]) >= min_dist]
    return chosen


def castle_walls(castles):
    """The six tiles a castle stamps, gate included: stepping onto the gate
    bounces the hero back, so none of them can be walked through."""
    return {(x + dx, y + dy) for x, y in castles
            for dx, dy in ((-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0))}


def walk_components(grid, blocked):
    """8-connected walkable regions, matching how the hero moves."""
    seen, out = set(), []
    for y in range(SIZE):
        for x in range(SIZE):
            if (x, y) in seen or (x, y) in blocked or grid[y][x] not in WALKABLE:
                continue
            comp, q = [], deque([(x, y)])
            seen.add((x, y))
            while q:
                px, py = q.popleft()
                comp.append((px, py))
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        if dx == dy == 0:
                            continue
                        n = (px + dx, py + dy)
                        if 0 <= n[0] < SIZE and 0 <= n[1] < SIZE and n not in seen \
                                and n not in blocked and grid[n[1]][n[0]] in WALKABLE:
                            seen.add(n)
                            q.append(n)
            out.append(comp)
    return out


def severs_walkable(grid, castles):
    """Whether these castles' walls split any walkable region in two."""
    blocked = castle_walls(castles)
    if not blocked:
        return False
    owner = {t: i for i, comp in enumerate(walk_components(grid, blocked)) for t in comp}
    for comp in walk_components(grid, set()):
        if len({owner[t] for t in comp if t not in blocked}) > 1:
            return True
    return False


def spread_castles(rng, candidates, count, min_dist, grid):
    """`spread`, but a candidate is skipped when its walls would sever the
    walkable land. Skipping is what lets a castle stand against a wall: the
    originals do, and only the corridor-pinching sites are refused."""
    pool = list(candidates)
    rng.shuffle(pool)
    chosen = []
    while pool and len(chosen) < count:
        if not chosen:
            order = pool
        else:
            def to_chosen(c):
                return min(abs(c[0] - p[0]) + abs(c[1] - p[1]) for p in chosen)

            order = sorted(pool, key=lambda c: -to_chosen(c))
            if to_chosen(order[0]) < min_dist:
                break
        pick = None
        for cand in order:
            if chosen and min(abs(cand[0] - p[0]) + abs(cand[1] - p[1])
                              for p in chosen) < min_dist:
                continue
            if not severs_walkable(grid, chosen + [cand]):
                pick = cand
                break
        if pick is None:
            break
        chosen.append(pick)
        pool = [c for c in pool if abs(c[0] - pick[0]) + abs(c[1] - pick[1]) >= min_dist]
    return chosen


def place(cfg, grid, rng):
    water = sea(grid)
    land = components(grid, WALKABLE)
    if not land:
        raise RuntimeError("no land")
    main = set(land[0])
    reachable_land = set(main)
    for comp in land[1:]:
        if len(comp) >= 4 and any((x + dx, y + dy) in water for x, y in comp
                                  for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
            reachable_land |= set(comp)
    # Islands: every landing-reachable landmass hosts castles, towns and the
    # rest; elsewhere they stay on the main landmass the hero walks.
    home_land = reachable_land if cfg.get("islands") else main
    coast = [(x, y) for x, y in home_land if grid[y][x] == GRASS
             and any((x + dx, y + dy) in water for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)))]
    if not coast:
        raise RuntimeError("the main landmass has no coast")
    taken = set()

    def take(*tiles):
        for t in tiles:
            taken.add(t)

    # Castles: a 3x2 grass block (gate at the bottom middle) with the tile
    # below the gate reachable. The castle's own tiles carry their grass, so
    # it may stand against a wall, as the originals' castles do -- but a
    # castle dropped into a two-wide path would seal the land on both sides,
    # so each site is taken only if its footprint leaves every walkable
    # region whole.
    sites = []
    for x, y in home_land:
        block = [(x + dx, y + dy) for dx in (-1, 0, 1) for dy in (-1, 0)]
        below = (x, y + 1)
        if all(at(grid, *c) == GRASS for c in block) and below in home_land \
                and at(grid, *below) == GRASS:
            sites.append((x, y))
    castles = spread_castles(rng, sites, cfg["castles"], 7, grid)
    if len(castles) < cfg["castles"]:
        raise RuntimeError(f"room for {len(castles)} of {cfg['castles']} castles")
    if cfg["home"]:
        # The King's castle first: the one nearest the middle of the land.
        cx = sum(x for x, _ in main) / len(main)
        cy = sum(y for _, y in main) / len(main)
        castles.sort(key=lambda c: (c[0] - cx) ** 2 + (c[1] - cy) ** 2)
    for x, y in castles:
        take(*[(x + dx, y + dy) for dx in (-2, -1, 0, 1, 2) for dy in (-2, -1, 0, 1, 2)])

    # Towns: coast grass with a dock on the open sea and a walkable gate.
    town_sites = []
    for x, y in coast:
        dock = next(((x + dx, y + dy) for dx, dy in ((0, 1), (1, 0), (-1, 0), (0, -1))
                     if (x + dx, y + dy) in water), None)
        gate = next(((x + dx, y + dy) for dx, dy in ((0, 1), (1, 0), (-1, 0), (0, -1))
                     if (x + dx, y + dy) in home_land and (x + dx, y + dy) != dock), None)
        if dock and gate:
            town_sites.append((x, y))
    towns_xy = spread(rng, town_sites, cfg["towns"], 6, taken, keep_off=1)
    if len(towns_xy) < cfg["towns"]:
        raise RuntimeError(f"room for {len(towns_xy)} of {cfg['towns']} towns")
    towns = []
    for x, y in towns_xy:
        dock = next((x + dx, y + dy) for dx, dy in ((0, 1), (1, 0), (-1, 0), (0, -1))
                    if (x + dx, y + dy) in water)
        gate = next((x + dx, y + dy) for dx, dy in ((0, 1), (1, 0), (-1, 0), (0, -1))
                    if (x + dx, y + dy) in home_land and (x + dx, y + dy) != dock)
        towns.append(dict(x=x, y=y, boat=dict(x=dock[0], y=dock[1]),
                          gate=dict(x=gate[0], y=gate[1])))
        take((x, y), gate)

    out = dict(id=cfg["id"], width=SIZE, height=SIZE,
               castles=[dict(x=x, y=y) for x, y in castles], towns=towns)

    # Spawns: arrival on the coast; at home also the King's gate front.
    free_coast = [c for c in coast if c not in taken]
    hero = spread(rng, free_coast, 1, 1, taken, keep_off=1)[0]
    out["hero_spawn"] = dict(x=hero[0], y=hero[1])
    take(hero)
    if cfg["home"]:
        kx, ky = castles[0]
        home = (kx, ky + 2) if (kx, ky + 2) in main else (kx, ky + 1)
        out["home_spawn"] = dict(x=home[0], y=home[1])
        take(home)
        alcove = spread(rng, [c for c in main if grid[c[1]][c[0]] == GRASS], 1, 1,
                        taken, keep_off=2)[0]
        out["magic_alcove"] = dict(x=alcove[0], y=alcove[1])
        take(alcove)

    # Chests (also the salt pool): grass on any landmass the hero can reach on
    # foot or land on from the sea; most on the main landmass.
    grass = [c for c in reachable_land if grid[c[1]][c[0]] == GRASS]
    chests = spread(rng, grass, cfg["chests"], 3, taken, keep_off=1)
    out["chests"] = [dict(x=x, y=y) for x, y in chests]
    take(*chests)
    if len(chests) < 21:
        raise RuntimeError("fewer than 21 chests: the salt pool would be too small")

    # Wandering armies: main-landmass grass, clear of castle gates.
    armies = spread(rng, [c for c in home_land if grid[c[1]][c[0]] == GRASS],
                    cfg["armies"], 4, taken, keep_off=1)
    out["wandering_armies"] = [dict(x=x, y=y) for x, y in armies]
    take(*armies)

    signs = spread(rng, [c for c in home_land if grid[c[1]][c[0]] == GRASS],
                   cfg["signs"], 6, taken, keep_off=1)
    out["signs"] = [dict(x=x, y=y) for x, y in signs]
    return out


# --- preview ---------------------------------------------------------------

COLORS = {GRASS: (110, 170, 80), WATER: (40, 90, 170), FOREST: (30, 100, 45),
          MOUNTAIN: (130, 120, 110), DESERT: (220, 195, 120)}
MARKS = {"castles": (210, 40, 40), "towns": (250, 220, 40), "chests": (240, 140, 30),
         "wandering_armies": (150, 50, 180), "signs": (250, 250, 250)}


def preview(grid, zone, path, scale=6):
    w = h = SIZE * scale
    px = bytearray(w * h * 4)

    def fill(x, y, col, inset=0):
        for yy in range(y * scale + inset, (y + 1) * scale - inset):
            for xx in range(x * scale + inset, (x + 1) * scale - inset):
                o = (yy * w + xx) * 4
                px[o:o + 4] = bytes((*col, 255))
    for y in range(SIZE):
        for x in range(SIZE):
            fill(x, y, COLORS[grid[y][x]])
    for key, col in MARKS.items():
        for p in zone.get(key, []):
            fill(p["x"], p["y"], col, inset=1)
    for key in ("hero_spawn", "home_spawn", "magic_alcove"):
        if key in zone:
            fill(zone[key]["x"], zone[key]["y"], (0, 230, 230), inset=1)
    write_png(path, w, h, bytes(px))


# --- main ------------------------------------------------------------------

def generate(cfg, seed):
    rng = random.Random(seed)
    for attempt in range(40):
        grid = clean(raw_terrain(cfg, rng))
        try:
            zone = place(cfg, grid, rng)
        except RuntimeError as exc:
            print(f"  {cfg['id']} seed {seed} try {attempt}: {exc}; regenerating", flush=True)
            continue
        return grid, zone, encode(grid, random.Random(seed + 1))
    raise RuntimeError(f"{cfg['id']}: no valid map in 40 tries")


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--only", default="", help="one continent id")
    ap.add_argument("--seed", type=int, default=None, help="override the seed")
    ap.add_argument("--preview", default="", help="also write PNG previews here")
    args = ap.parse_args()
    os.makedirs(os.path.join(PACK, "maps"), exist_ok=True)
    os.makedirs(os.path.join(PACK, "data", "zones"), exist_ok=True)
    for cfg in CONTINENTS:
        if args.only and cfg["id"] != args.only:
            continue
        seed = cfg["seed"] if args.seed is None else args.seed
        grid, zone, rows = generate(cfg, seed)
        zone["seed"] = seed
        with open(os.path.join(PACK, "maps", f"{cfg['id']}.dat"), "w") as fh:
            fh.write("\n".join(rows) + "\n")
        with open(os.path.join(PACK, "data", "zones", f"{cfg['id']}.json"), "w") as fh:
            json.dump(zone, fh, indent=1)
            fh.write("\n")
        if args.preview:
            os.makedirs(args.preview, exist_ok=True)
            preview(grid, zone, os.path.join(args.preview, f"{cfg['id']}.png"))
        counts = {k: len(v) for k, v in zone.items() if isinstance(v, list)}
        print(f"{cfg['id']}: seed {seed}, {counts}", flush=True)
    with open(os.path.join(PACK, "data", "tile_codes.json"), "w") as fh:
        json.dump(tile_codes(), fh, indent=1, sort_keys=True)
        fh.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
