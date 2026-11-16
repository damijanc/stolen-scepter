"""Compose the master presentation run. Writes master.txt: a plain playtest
script with `#SHOT name` markers (comments to playtest) where stills belong,
and `#HOLD n` markers where the video should linger on a screen."""
from driver import *

L = []
def a(*c): L.extend(c)

# --- pre-game wizard -------------------------------------------------------
a('wait 1s', '#SHOT title', 'key Enter', 'wait 20', '#SHOT logo', 'wait 45', 'key Enter', 'wait 20',
  '#SHOT credits', 'wait 45', 'key Enter', 'wait 20', '#SHOT class-select', 'wait 60',
  'key A', 'wait 10')
for ch in 'Rowan':
    a(f'type {ch}', 'wait 4')
a('wait 20', 'key Enter', 'wait 10', '#SHOT difficulty', 'key Enter', 'wait 10',
  'key Enter', 'wait 10', '#SHOT intro-banner', 'wait 60', 'key Enter', 'wait 20')

d = Driver([l for l in L if not l.startswith('#')])
d.lines = list(L)  # keep markers; playtest ignores comments

a2 = d.add
a2('#SHOT opening', 'wait 90')
# --- the King's castle: recruit ---------------------------------------------
a2('key Up', 'wait 20', '#SHOT gate', 'key Up', 'wait 20', '#SHOT royal-hall', 'wait 60',
   'key A', 'wait 10', '#SHOT recruit-menu', 'wait 45', 'key C', 'wait 10', '#SHOT recruit-howmany', 'wait 45')
for ch in '10':
    a2(f'type {ch}', 'wait 6')
a2('key Enter', 'wait 10', '#SHOT recruit-pikemen', 'wait 45', 'key B', 'wait 10')
a2('type 8', 'wait 6', 'key Enter', 'wait 10', '#SHOT recruit-done', 'wait 60', 'key Escape', 'wait 10')
a2('key A', 'wait 10', '#SHOT army', 'wait 75', 'key Escape', 'wait 10')
# --- first battle: the hostile band waiting outside ---------------------------
a2('key Down', 'wait 20', '#SHOT battle1-start', 'wait 45')
r = 0
while True:
    (_, _, _, _), text, _ = d.look()
    if not ('Shoot' in text or 'Options /' in text or ' vs ' in text or 'Choose' in text):
        break
    for k in COMBAT_CYCLE:
        a2(f'key {k}', 'wait 30')
    r += 1
    a2(f'#SHOT battle1-r{r:02d}')
a2('#SHOT battle1-victory', 'wait 90')
print('battle1 rounds', r, d.look()[2])
a2('key Enter', 'wait 10')
# --- treasure: a choice -----------------------------------------------------
d.goto(35, 38)
a2('#SHOT before-chest', 'key Down', 'wait 20', '#SHOT chest-choice', 'wait 150', 'key A', 'wait 20', '#SHOT chest-after')
print('chest', d.look()[2])
# --- walk to Stillwell, a shot per step ----------------------------------------
n = [0]
orig_add = d.add
def add_with_shots(*cmds):
    orig_add(*cmds)
    if cmds and cmds[-1] == 'wait 14':
        n[0] += 1
        orig_add(f'#SHOT walk1-{n[0]:03d}')
d.add = add_with_shots
d.goto(18, 29)
d.add = orig_add
print('town', d.look()[2])
a2('key Up', 'wait 20', '#SHOT town-menu', 'wait 60', 'key A', 'wait 10', '#SHOT town-contract', 'wait 120',
   'key Enter', 'wait 10', 'key E', 'wait 10', '#SHOT town-siege-bought', 'wait 60', 'key Enter', 'wait 10',
   'key C', 'wait 10', '#SHOT town-news', 'wait 90', 'key Enter', 'wait 10', 'key Escape', 'wait 10')
a2('key I', 'wait 10', '#SHOT contract-view', 'wait 90', 'key Escape', 'wait 10')
# --- march on Ironwatch --------------------------------------------------------
n[0] = 0
def add_with_shots2(*cmds):
    orig_add(*cmds)
    if cmds and cmds[-1] == 'wait 14':
        n[0] += 1
        orig_add(f'#SHOT walk2-{n[0]:03d}')
d.add = add_with_shots2
d.goto(23, 37)
d.add = orig_add
print('ironwatch', d.look()[2])
a2('key Up', 'wait 20', '#SHOT siege-prompt', 'wait 90', 'key Y', 'wait 30', '#SHOT siege-start', 'wait 60')
r = 0
while r < 60:
    (_, _, _, _), text, _ = d.look()
    if not ('Shoot' in text or 'Options /' in text or ' vs ' in text or 'Choose' in text):
        break
    for k in COMBAT_CYCLE:
        a2(f'key {k}', 'wait 30')
        if r < 4 or r >= 9:
            a2(f'#SHOT siege-r{r:02d}-{k}')
    r += 1
    a2(f'#SHOT siege-r{r:02d}')
a2('#SHOT siege-end', 'wait 120')
print('siege rounds', r, d.look()[1], d.look()[2])
open('master.txt', 'w').write('\n'.join(d.lines) + '\n')
# --- after the siege -------------------------------------------------------------
a2('key Enter', 'wait 20', '#SHOT home-again', 'wait 60')
print('after', d.look()[1])
a2('key V', 'wait 10', '#SHOT hero-view', 'wait 60', 'key Escape', 'wait 10',
   'key P', 'wait 10', '#SHOT puzzle', 'wait 60', 'key Escape', 'wait 10',
   'key M', 'wait 10', '#SHOT worldmap-explored', 'wait 90', 'key Escape', 'wait 10',
   'key O', 'wait 10', '#SHOT options', 'key Escape', 'wait 10')
open('master.txt', 'w').write('\n'.join(d.lines) + '\n')
# Debug-menu extras, kept out of the main story (and the video).
dbg = list(d.lines) + ['#ENDSTORY', 'key F10', 'wait 10', 'key O', 'wait 10', 'key Escape', 'wait 10',
    'key M', 'wait 10', 'shot dbg-worldmap-revealed']
open('debug.txt', 'w').write('\n'.join(dbg) + '\n')
