"""Builds a deterministic playtest script by driving the real game step by step.

Each step re-runs the (fast, screenshot-free) script from the start, reads the
final `state`/`text`, and appends the next input. Nothing here touches the
game's code; it only writes playtest scripts.
"""
import json, re, subprocess, collections, sys, os

ROOT = '/Users/damijancavar/development/kings-bounty'
PT = ROOT + '/target/release/playtest'
PACK = ROOT + '/packs/stolen-scepter'
SCR = os.path.dirname(os.path.abspath(__file__))
G = json.load(open(PACK + '/game.json'))
TC = G['tile_codes']
DIRS = {(0, -1): 'Up', (0, 1): 'Down', (-1, 0): 'Left', (1, 0): 'Right'}
COMBAT_CYCLE = ['Right', 'PageUp', 'PageDown', 'Up', 'Down', 'Left', 'Space']


def run(lines, seed=1):
    script = '\n'.join(lines + ['state', 'text']) + '\n'
    out = subprocess.run([PT, '--pack', PACK, '--seed', str(seed), '--script', '-',
                          '--out', SCR + '/drv-out', '--scale', '1'],
                         input=script, capture_output=True, text=True, cwd=ROOT)
    if out.returncode != 0:
        raise RuntimeError(out.stderr)
    st = [l for l in out.stdout.splitlines() if l.startswith('state:')][-1]
    tx = [l for l in out.stdout.splitlines() if l.startswith('text:')][-1]
    m = re.search(r'at (\w+) \((\d+),(\d+)\) mode (\w+)', st)
    return (m.group(1), int(m.group(2)), int(m.group(3)), m.group(4)), tx[6:], st


class Driver:
    def __init__(self, lines, seed=1):
        self.lines = list(lines)
        self.seed = seed
        self.blocked = set()

    def look(self):
        return run(self.lines, self.seed)

    def add(self, *cmds):
        self.lines.extend(cmds)

    fights = 0
    def fight(self, max_rounds=60):
        Driver.fights += 1
        self.add(f'#SHOT fight{Driver.fights}-start')
        for r in range(max_rounds):
            (_, _, _, _), text, _ = self.look()
            if 'Shoot' not in text and 'Options /' not in text and ' vs ' not in text \
                    and 'Choose a target' not in text:
                return
            for k in COMBAT_CYCLE:
                self.add(f'key {k}', 'wait 30', f'#SHOT fight{Driver.fights}-r{r:02d}-{k}')
        raise RuntimeError('fight did not end')

    def passable(self, zone, x, y, target):
        if (x, y) == target:
            return True
        if not (0 <= x < 64 and 0 <= y < 64) or (zone, x, y) in self.blocked:
            return False
        for c in G['castles']:
            if c.get('zone') == zone and c['x'] - 1 <= x <= c['x'] + 1 and c['y'] - 1 <= y <= c['y']:
                return False
        for t in G['towns']:
            if t.get('zone') == zone and (t['x'], t['y']) == (x, y):
                return False
        d = open(f'{PACK}/maps/{zone}.dat', 'rb').read()
        t = TC.get(chr(d[y * 65 + x]), {})
        return not t.get('blocks_foot') and t.get('terrain') not in ('water', None)

    def route(self, zone, s, t):
        prev = {s: None}
        q = collections.deque([s])
        while q:
            p = q.popleft()
            if p == t:
                break
            for d in DIRS:
                n = (p[0] + d[0], p[1] + d[1])
                if n not in prev and self.passable(zone, *n, t):
                    prev[n] = p
                    q.append(n)
        if t not in prev:
            return None
        path, p = [], t
        while prev[p]:
            path.append(p)
            p = prev[p]
        return path[::-1]

    def goto(self, tx, ty, choice='A', on_unknown=None, max_steps=400):
        for _ in range(max_steps):
            (zone, x, y, mode), text, st = self.look()
            extra = text.split('|')[2:] if text.startswith('Click hero') else None
            if 'Shoot' in text or 'Options /' in text or ' vs ' in text:
                self.fight()
                continue
            if extra and any(e.strip() for e in extra):
                body = ' '.join(e.strip() for e in extra)
                if 'You may' in body or 'A)' in body:
                    self.add(f'key {choice}', 'wait 10')
                    continue
                if on_unknown:
                    r = on_unknown(self, body)
                    if r:
                        continue
                self.add('key Enter', 'wait 10')
                continue
            if not text.startswith('Click hero'):
                raise RuntimeError(f'unexpected screen at ({x},{y}): {text}')
            if (x, y) == (tx, ty):
                return
            path = self.route(zone, (x, y), (tx, ty))
            if not path:
                raise RuntimeError(f'no route from {(x, y)} to {(tx, ty)}')
            n = path[0]
            before = len(self.lines)
            self.add(f'key {DIRS[(n[0] - x, n[1] - y)]}', 'wait 14')
            (z2, x2, y2, _), t2, _ = self.look()
            if (x2, y2) == (x, y) and t2.startswith('Click hero') and \
                    not any(e.strip() for e in t2.split('|')[2:]):
                self.blocked.add((zone, *n))
                self.lines = self.lines[:before]
        raise RuntimeError('too many steps')
