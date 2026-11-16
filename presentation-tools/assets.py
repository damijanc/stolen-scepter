"""Asset sheets built only from the pack's own PNGs (current and git history).
Scaling is whole-number nearest-neighbour only; nothing is redrawn."""
import json, os, sys
from PIL import Image, ImageDraw, ImageFont

ROOT = '/Users/damijancavar/development/kings-bounty'
ART = ROOT + '/packs/stolen-scepter/art'
SCR = os.path.dirname(os.path.abspath(__file__))
OLD = SCR + '/pack-724fec2/packs/stolen-scepter/art'
OUT = sys.argv[1]
BG = (24, 26, 32)
FG = (235, 235, 235)
DIM = (160, 165, 175)
GOLD = (232, 190, 80)
F = lambda s, b=True: ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial%s.ttf' % (' Bold' if b else ''), s)


def img(path, k=1):
    im = Image.open(path).convert('RGBA')
    return im.resize((im.width * k, im.height * k), Image.NEAREST) if k != 1 else im


def checker(w, h):
    """Neutral backdrop so transparent sprites read clearly."""
    return Image.new('RGBA', (w, h), (58, 62, 72, 255))


def grid(title, subtitle, items, cols, cell, k, out, label_size=26, pad=24, bg_tile=True):
    cw, ch = cell
    head = 150
    rows = (len(items) + cols - 1) // cols
    W = cols * (cw + pad) + pad
    H = head + rows * (ch + pad + label_size + 12) + pad
    sheet = Image.new('RGBA', (W, H), BG + (255,))
    d = ImageDraw.Draw(sheet)
    d.text((pad, 28), title, font=F(56), fill=FG)
    d.text((pad, 96), subtitle, font=F(28, False), fill=DIM)
    for i, (label, path) in enumerate(items):
        x = pad + (i % cols) * (cw + pad)
        y = head + (i // cols) * (ch + pad + label_size + 12)
        im = img(path, k)
        if bg_tile:
            sheet.alpha_composite(checker(cw, ch), (x, y))
        sheet.alpha_composite(im, (x + (cw - im.width) // 2, y + (ch - im.height) // 2))
        d.text((x, y + ch + 6), label, font=F(label_size, False), fill=FG)
    sheet.convert('RGB').save(out)
    print(out, sheet.size)


def nice(n):
    return n.replace('.png', '').replace('_', ' ')


# 02 terrain tiles --------------------------------------------------------------
base = ['grass', 'grass_variant', 'forest', 'mountain', 'water', 'desert']
grid('Terrain tiles', 'The six base terrains every continent is painted with (one tile = 48x34 game pixels, shown at 3x)',
     [(nice(n), f'{ART}/tiles/{n}.png') for n in base], 3, (576, 408), 3, f'{OUT}/02-terrain-tiles.png', 30)

edges = []
for fam in ['water_edge', 'forest_edge', 'mountain_edge', 'forest_shore', 'mountain_shore', 'desert_edge']:
    names = sorted(f for f in os.listdir(f'{ART}/tiles') if f.startswith(fam + '_'))
    edges += [(nice(f), f'{ART}/tiles/{f}') for f in names]
grid('Transition tiles', 'Edge and shore pieces: the game picks one per cell so terrain blends instead of meeting in hard squares',
     edges, 12, (192, 136), 1, f'{OUT}/02c-transition-tiles.png', 18)

objs = ['castle_tl', 'castle_tr', 'castle_ml', 'castle_gate', 'castle_mr', 'castle_br', 'town', 'dwelling_plains',
        'dwelling_forest', 'dwelling_hills', 'dwelling_dungeon', 'chest', 'artifact_chest', 'artifact_ring',
        'sign', 'bridge_h', 'bridge_v', 'wandering_army']
grid('Map objects', 'Castles (built from six pieces), towns, troop dwellings, treasure, signposts, bridges and roaming armies',
     [(nice(n), f'{ART}/tiles/{n}.png') for n in objs], 6, (384, 272), 2, f'{OUT}/02d-map-objects.png', 26)

# 02b the same tiles assembled: terrain layer of a real continent, straight from its map file
g = json.load(open(ROOT + '/packs/stolen-scepter/game.json'))
tc = g['tile_codes']
m = open(ROOT + '/packs/stolen-scepter/maps/highmoor.dat', 'rb').read()
x0, y0, x1, y1 = 10, 26, 34, 48  # Highmoor's south-west: lake, ridges, forest, coast
TW, TH = 96, 68  # half the art size, an exact 2:1 reduction
cache = {}
canvas = Image.new('RGBA', ((x1 - x0) * TW, (y1 - y0) * TH))
for y in range(y0, y1):
    for x in range(x0, x1):
        art = tc[chr(m[y * 65 + x])]['art']
        if art not in cache:
            cache[art] = Image.open(f'{ART}/tiles/{art}.png').convert('RGBA').resize((TW, TH), Image.BOX)
        canvas.alpha_composite(cache[art], ((x - x0) * TW, (y - y0) * TH))
head = 150
sheet = Image.new('RGBA', (canvas.width + 48, canvas.height + head + 24), BG + (255,))
d = ImageDraw.Draw(sheet)
d.text((24, 28), 'The same tiles, assembled', font=F(56), fill=FG)
d.text((24, 96), f'Terrain layer of Highmoor, tiles ({x0},{y0})-({x1 - 1},{y1 - 1}), laid out from the map file: '
       f'{(x1 - x0) * (y1 - y0)} cells, about 70 distinct images', font=F(28, False), fill=DIM)
sheet.alpha_composite(canvas, (24, head))
sheet.convert('RGB').save(f'{OUT}/02b-terrain-assembled.png')
print('assembled', sheet.size)

# 03 characters --------------------------------------------------------------------
troops = sorted({f.rsplit('_', 1)[0] for f in os.listdir(f'{ART}/troops')})
grid('The armies', f'All {len(troops)} troop types a hero can recruit or meet in battle (first animation frame, 2x)',
     [(t.capitalize(), f'{ART}/troops/{t}_00.png') for t in troops], 5, (384, 272), 2, f'{OUT}/03-characters.png', 30)

frames = [('Hero on horseback', 'sprites/hero_walk'), ('Hero by boat', 'sprites/boat'), ('Dragons', 'troops/dragons'),
          ('Knights', 'troops/knights'), ('Ghosts', 'troops/ghosts'), ('Wolves', 'troops/wolves')]
items = []
for label, stem in frames:
    for i in range(4):
        items.append((f'{label} - frame {i + 1}', f'{ART}/{stem}_{i:02d}.png'))
grid('Animation frames', 'Each moving figure is four hand-picked frames played in a loop', items, 4, (384, 272), 2,
     f'{OUT}/03b-animation-frames.png', 24)

vill = sorted({f.rsplit('_', 1)[0] for f in os.listdir(f'{ART}/villains')})
grid('The villains', f'{len(vill)} wanted villains: each contract names one, and catching them all reveals the puzzle map',
     [(v.capitalize(), f'{ART}/villains/{v}_00.png') for v in vill], 6, (384, 272), 2, f'{OUT}/03c-villains.png', 28)

grid('The four heroes', 'Playable classes: Knight, Paladin, Sorceress, Barbarian', [
    (n.capitalize(), f'{ART}/classes/{n}.png') for n in ['knight', 'paladin', 'sorceress', 'barbarian']],
    4, (768, 816), 2, f'{OUT}/03d-hero-classes.png', 30)

# 10 polish: tile-level before/after (placeholder pixel art at 724fec2 vs today) ---------
pairs = [('Grass', 'tiles/grass.png'), ('Forest', 'tiles/forest.png'), ('Mountain', 'tiles/mountain.png'),
         ('Water', 'tiles/water.png'), ('Town', 'tiles/town.png'), ('Treasure chest', 'tiles/chest.png'),
         ('Hero', 'sprites/hero_walk_00.png'), ('Boat', 'sprites/boat_00.png')]
cw, ch, pad, head = 384, 272, 24, 210
W = pad + 4 * (2 * cw + 3 * pad)
rows = 2
H = head + rows * (ch + 70) + pad
sheet = Image.new('RGBA', (W, H), BG + (255,))
d = ImageDraw.Draw(sheet)
d.text((pad, 28), 'Before and after: the same game objects', font=F(56), fill=FG)
d.text((pad, 96), 'Left of each pair: placeholder pixel art (git 724fec2, 22 Oct 2026).  Right: current art.',
       font=F(28, False), fill=DIM)
d.text((pad, 136), 'Both are the files the game actually loads, shown at the same size.', font=F(28, False), fill=DIM)
for i, (label, rel) in enumerate(pairs):
    x = pad + (i % 4) * (2 * cw + 3 * pad)
    y = head + (i // 4) * (ch + 70)
    for j, root in enumerate([OLD, ART]):
        im = Image.open(f'{root}/{rel}').convert('RGBA')
        k = max(1, min(cw // im.width, ch // im.height))
        im = im.resize((im.width * k, im.height * k), Image.NEAREST)
        sheet.alpha_composite(checker(cw, ch), (x + j * (cw + pad), y))
        sheet.alpha_composite(im, (x + j * (cw + pad) + (cw - im.width) // 2, y + (ch - im.height) // 2))
    d.text((x, y + ch + 8), f'{label}:  before  |  after', font=F(28, False), fill=FG)
sheet.convert('RGB').save(f'{OUT}/10b-polish-assets.png')
print('pairs', sheet.size)
