# Play Verification Findings

Manual cross-check run against `33a5a50` on 2026-09-30.

Evidence, weakest caveat first:

- **DOS** = the scanned DOS manual, OCR'd to `game-assets/.manual_ocr/`.
  Each `page_NNN.txt` is keyed by the PRINTED page number, not the PDF sheet
  (`tools/manual_word.py` writes `page_{printed:03d}.txt`; the sheet is
  `printed // 2 + 2`). The table of contents on p.3 confirms it: p.41 and p.46
  open the sections it lists at those numbers. This manual has no advancement
  chart, morale chart or troop chart. It points to a Quick-Reference card
  (p.23, p.46), and that card is not in `game-assets/`. Its Appendix C is the
  artifacts (p.57), not character advancement.
- **MD** = `game-assets/Kings-Bounty_Manual_DOS_EN_Web-Manual.pdf`, read with
  `pdftotext`. Despite the filename it is the **Sega Mega Drive** manual
  (p.4 gives cartridge setup), re-typeset in 2000. It is the only local
  source for the advancement, morale and troop charts. Treat it as a
  different edition: it agrees with DOS wherever both speak, but a number
  found only here is weak evidence. Page numbers are its `--- N ---` folios.
- **Pack** = `reference/assets/kings-bounty/game.json`.
- **Coordinates**: the manuals count Y from the bottom. `manual_y = 63 - pack_y`
  holds for all 52 castle and town entries in Appendix A (MD p.I, DOS p.67).

## §14 suspects

### 1. Fixed 5x5 fog reveal

- Manual: no sight radius is stated. The only size the manual gives for what
  the hero sees is the "5x5 Adventure Window" (DOS p.28). The auto-map records
  terrain you have crossed (DOS p.31), with orbs revealing the rest (MD p.47).
- Pack: `world.fog_sight: 3`.
- Port: `engine/src/fog.rs:32` `Fog::reveal` stamps -2..=+2 and discards
  `radius` (line 33), as C's `FogReveal` does (`reference/engine/fog.c:8`).
  The legacy viewport is also radius 2 (`shell/src/layout.rs:350` test), so
  the fog clears exactly the window the hero is shown.
- Verdict: **AGREES** (indirect). A 5x5 reveal matches the only 5x5 the
  manual names. `fog_sight` really is dead. It is not a lost 7x7 feature.
- Guide correction: §3 says "nothing reads that field". Code does read it:
  `engine/src/step.rs:197` and `engine/src/game.rs:1272` pass it to `reveal`,
  which ignores it. "Read and discarded" is accurate.

### 2. Only astrology-matching dwellings repopulate

- Manual: the creature named for the week gets its dwelling replenished
  (DOS p.34, MD p.51). A depleted dwelling "will not be replenished until"
  that creature's week (DOS p.61). MD p.82 gives the same warning as a hint.
- Pack: per-troop `max_population` and `growth_per_week`. Refill goes to
  `max_population`.
- Port: `engine/src/game.rs:3372-3376` (`apply_astrology`) refills only
  dwellings whose troop matches, called from `apply_week_end`
  (`game.rs:3421`). This matches C `GameApplyAstrology`
  (`reference/engine/game.c:2123`). `grow_dwellings` (`game.rs:3353`) and C's
  `GameGrowDwellings` (`game.c:2098`) have no caller in either tree. Only a
  Rust unit test calls it.
- Verdict: **AGREES**. §14 asked which of the two is wrong. The function name
  is the wrong one: `GameGrowDwellings` is dead code, and the executable rule
  is the manual's rule.

### 3. `gold <= cost` refuses an exact-cost purchase

- Manual: silent on the equal case. Search terms tried in both manuals:
  `enough`, `afford`, `exact`, `price`, `cost`. The only hits are "enough
  gold" for recruiting (MD p.38) and unpaid upkeep (DOS p.30). Neither covers
  a purse equal to the price.
- Pack: `economy.boat_cost_normal: 500`, `boat_cost_cheap: 100`,
  `siege_cost: 3000`, `alcove_cost: 5000`, spell costs 100-5000.
- Port: the guards are NOT uniform, and they match C exactly:
  - `gold <= cost` (exact fails): `rent_boat` `game.rs:3227`, `buy_siege`
    `game.rs:3268`, `buy_spell` `game.rs:3316`. C: `game.c:1275`, `1315`,
    `1334`, each commented "KB: <= fails". The C author credits the rule to
    the original game, not to their own choice.
  - `gold < cost` (exact succeeds): `accept_alcove` `game.rs:899` (C
    `flow_resolve.c:278`), and recruitment `game.rs:2677`.
- Verdict: **NEEDS BINARY**. In DOS, with exactly 500 gold and no Anchor, try
  to rent a boat. Then try exactly 3000 for siege weapons, and exactly 5000
  at Aurange. Per C, the first two refuse and the alcove accepts. Getting an
  exact purse in the original is fiddly; that is the user's call.
- Guide correction: §5 scenario 2 expects the alcove to "behave consistently
  with the boat". It does not, by design of C. Exactly 5000 buys magic.

### 4. Bridge and gate spells cost no charge at cast time

- Manual: spell entries describe effects only (DOS p.42-43). MD p.77 says
  spells "can only be used once". Neither says when the charge goes, or what
  a cancelled cast costs.
- Pack: `bridge`, `castle_gate`, `town_gate` in `spells`.
- Port: the charge is **deferred, not waived**. `cast_bridge`
  (`engine/src/spells_adventure.rs:358`) and `cast_castle_gate` /
  `cast_town_gate` (`:368`, `:404`) only check that a charge exists. The
  bridge charge is spent in the shell once `try_build_bridge` returns a
  positive count (`shell/src/main.rs:899`). C spends it in its host too
  (`reference/src/main.c:1286`). A gate spends on arrival, in
  `gate_teleport` (`engine/src/game.rs:1474-1475`). The net effect: one charge
  per spell that takes effect, and nothing for an ESC or a bridge with no
  water.
- Verdict: **AGREES** with "used once" in outcome. The manual is silent on
  cancellation, so the cancel case **NEEDS BINARY**. Cast Bridge in DOS,
  choose a direction with no water (or back out), and see whether the spell
  count drops.
- Guide correction: §8 scenario 2 says "confirm no charge is consumed" by a
  gate spell. A gate you complete DOES consume one. Only the arming step
  is free.
- Aside, not a finding: the bridge decrement is a game rule living in host
  code in both C and this port. It could move into the engine once a test
  protects it.

### 5. Bridge caps at 2 tiles; C's comment says 5

- Manual: the bridge "spans two squares" in a cardinal direction, and bridges
  may be chained (DOS p.42). MD p.77 gives no length.
- Pack: no bridge-length field.
- Port: `try_build_bridge` `engine/src/spells_adventure.rs:164`, which breaks
  at `built >= 2` (`:195`).
- Verdict: **AGREES**. The code and the manual both say 2. Only C's header
  comment says 5.

### 6. Sorceress commission drop, Paladin spell regression

- Manual: MD Appendix C (p.IV) prints every rank after the first as `+N`, an
  increment. DOS has no chart, but DOS p.25 gives two qualitative checks: the
  Sorceress commission "reaches a lower plateau" than other classes at high
  rank, and the Barbarian commission "just might have the best scale".
- Pack: `classes[].ranks[]` holds exactly the chart's numbers without the
  `+` sign (for example the Sorceress Magician `commission: 1000`).
- Port: the values are summed, not assigned. `class_stats_at_rank`
  (`engine/src/tables.rs:384`) accumulates ranks `0..=r`, mirroring C
  `reference/engine/tables.c:11`. Promotion adds the cumulative delta
  (`engine/src/game.rs:2530-2535`, C `game.c:1482`). The real per-rank totals
  are:

  | Class | Leadership | Max spells | Spell power | Commission |
  |---|---|---|---|---|
  | Knight | 100 / 200 / 500 / 1000 | 2 / 5 / 9 / 14 | 1 / 2 / 3 / 5 | 1000 / 2000 / 4000 / 8000 |
  | Paladin | 80 / 160 / 400 / 800 | 3 / 6 / 12 / 17 | 1 / 2 / 4 / 6 | 1000 / 2000 / 4000 / 8000 |
  | Sorceress | 60 / 120 / 300 / 600 | 5 / 13 / 23 / 35 | 2 / 5 / 10 / 15 | 3000 / 4000 / 5000 / 6000 |
  | Barbarian | 100 / 200 / 500 / 1000 | 2 / 4 / 7 / 10 | 0 / 1 / 2 / 3 | 2000 / 4000 / 6000 / 8000 |

- Verdict, Sorceress: **AGREES**. There is no drop. Her commission rises
  3000 -> 6000 and tops out below everyone else's 8000, which is exactly the
  "lower plateau". The Barbarian's +2000 at every rank is the "best scale".
  The guide's absolute reading would freeze him at 2000 forever, which
  contradicts DOS p.25.
- Verdict, Paladin: **DISAGREES** (minor, weak evidence). The raw 3 -> 3 ->
  6 -> 5 was never a regression, because those are increments. But the pack
  swaps two of them: Avenger `max_spells: 6` and Champion `5`, where MD p.IV
  prints `+5` and `+6`. The totals are 3 / 6 / 12 / 17 per pack and
  3 / 6 / 11 / 17 per chart, so only Avenger differs. The other 78 chart cells
  match, apart from two MD typos (base leadership "10" for Knight and
  Barbarian, where the class table on p.11 says 100). NEEDS BINARY to settle:
  promote a DOS Paladin to Avenger and read Max Spells, which is 12 per pack
  and 11 per chart. Tag: PACK DATA suspect.
- Guide correction: §6's two "anomalies" and §6 scenario 3 ("Expect it to
  fall") come from reading increments as totals. That is a **GUIDE ERROR**,
  for Task 3 to fix.

### 7. 14 of 32 combat-log strings parsed and never used

- Manual: silent on on-screen text. DOS p.37 confirms the menu bar reads
  "Options/(Troop Name) Movement Remaining", which matches the pre-kill title
  in guide §9. For shooting, DOS p.39 describes "a pulsating sight" and no
  instruction line. DOS p.38 describes retaliation and DOS p.33 out-of-control
  troops only as rules. MD p.26 and p.30 describe coloured target rings, not
  text. Search terms tried: `retaliat`, `out of control`, `message`, `sight`.
- Pack: `strings.combat_log` has 32 keys. These 14 are unused: `retaliate`,
  `ranged_no_effect`, `move`, `wait`, `pass`, `ooc`, `select_clone`,
  `select_freeze`, `select_resurrect`, `select_damage`, `select_teleport`,
  `select_dest`, `give_up_prompt`, `exit_hint`.
- Port: the shell carries defaults for exactly the other 18
  (`shell/src/combat.rs:47-64`). The give-up confirm uses its own
  `combat_give_up_*` banners (`:68-70`). C loads all 32
  (`reference/engine/resources.c:1495-1588`), and no C file outside
  `resources.c` reads the 14.
- Verdict: **NEEDS BINARY**. In DOS combat, watch the message line in five
  cases: (a) `S` with archers, before choosing a target; (b) casting Clone,
  Freeze, Teleport or Fireball, before choosing a target; (c) a melee hit on a
  stack that strikes back: is there a "retaliate, killing N" line?; (d) an
  out-of-control stack's turn; (e) a wait, a pass and a plain move.

## Manual facts

From `specs/manual-reference-notes.md` "High-Value Manual Facts To
Cross-Check", plus the troop chart and Appendix A, which that list also names.

| Fact | Manual | Pack | Verdict |
|---|---|---|---|
| Knight start: leadership 100, 2 spells, power 1, 1000/wk | MD p.11 | `knight.ranks[0]` 100 / 2 / 1 / 1000 | AGREES |
| Paladin start: 80, 3, 1, 1000 | MD p.11 | 80 / 3 / 1 / 1000 | AGREES |
| Sorceress start: 60, 5, 2, 3000, knows magic | MD p.11; magic from the start DOS p.25 | 60 / 5 / 2 / 3000, `knows_magic: true` | AGREES |
| Barbarian start: 100, 2, 0, 2000 | MD p.11 | 100 / 2 / 0 / 2000 | AGREES |
| Starting gold order | DOS p.25: Sorceress most, tied with Paladin; Barbarian lowest. DOS p.23: Knight's among the best | Paladin 10000, Sorceress 10000, Knight 7500, Barbarian 7500 | DISAGREES (weak, qualitative): Knight ties Barbarian for lowest |
| Archmage Aurange: Continentia (11,19), 5000 gold | MD p.17 | `continentia.magic_alcove` (11,44), `economy.alcove_cost` 5000 | AGREES (63 - 44 = 19) |
| Siege weapons 3000 | MD p.16, p.76 (DOS p.60 gives no price) | `economy.siege_cost` 3000 | AGREES |
| Boat 500, Anchor lowers it to 100 | MD p.19, p.74; DOS p.57 says only "significantly discounted" | `boat_cost_normal` 500, `boat_cost_cheap` 100 | AGREES |
| Puzzle: 25 pieces = 16 villains + Arech + 8 artifacts, Arech in the centre | DOS p.21, p.41; MD p.7 | 17 villains, 8 artifacts; `PUZZLE_GRID[2][2]` = 16 = `arech` (`engine/src/tables.rs:446`) | AGREES |
| Morale chart `NNNNN / NNNNN / NNHNN / LNLHN / LLLNN` | MD p.III (DOS defers to the card) | `combat.morale_chart`, identical | AGREES |
| Troop chart: skill, HP, move, damage, cost, morale group for 25 troops | MD p.II; shot counts DOS p.48, p.50, p.52, p.56 | all 25 rows match | AGREES |
| Orc shots per battle | MD p.70: six (DOS p.56 silent) | `orcs.ranged[2]` 10 | DISAGREES (weak, MD only) |
| Castle and town coordinates, 52 entries | MD p.I, DOS p.67 | all 52 match after the Y flip | AGREES |
| Place-name spelling | DOS p.67 and MD p.I: "Lorshe", "Wood's End" | "Lorsche", "Woods End" | DISAGREES (cosmetic) |
| Flight excludes Sprites | DOS p.31, MD p.22 | sprites skill 1; port needs skill 2+ with FLY | AGREES |
| Week is 5 days | DOS p.34, MD p.50 | `time.week_days` 5 | AGREES |
| Five contracts on offer | DOS p.60, MD p.13 | `contract.cycle_length` 5 | AGREES |

## Other manual/C disagreements found in passing

These came up while reading, and they are outside the seven suspects. Per
`AGENTS.md` the manual does not decide gameplay, so each is recorded for the
user and none is resolved. In each case C and the port agree with each other.

| # | Topic | Manual | C / port | Tag |
|---|---|---|---|---|
| M1 | Scepter search cost | A failed search costs ten days, a successful one a day. With Arech captured every search costs one day (DOS p.32, p.41; MD p.52, p.56) | Success costs 0 days. Failure always costs `tuning.search_cost_days` (10), Arech or not (`engine/src/step.rs:517-528`) | C SUSPECT |
| M2 | Week of the Peasant | Ghosts in your army OR garrisoning your castle turn into Peasants (DOS p.54) | Only army stacks convert (`engine/src/game.rs:3378-3387`; C `game.c:2143`) | C SUSPECT |
| M3 | Unpaid upkeep | A troop you cannot pay leaves (DOS p.30, MD p.43) | Upkeep is subtracted, gold is clamped at 0, and every troop stays (`engine/src/game.rs:3402-3419`; C `game.c:1049-1064`) | C SUSPECT |

M1 is the one most visible in play. It changes the endgame's cost of guessing
once Arech is caught.

## Guide errors for Task 3

1. §3: `fog_sight` is read and discarded, not unread (suspect 1).
2. §5 scenario 2: the alcove accepts an exact purse; it is not meant to
   match the boat (suspect 3).
3. §8 scenario 2: a completed gate spends a charge (suspect 4).
4. §6 anomalies and scenario 3: the rank values are increments, so the
   Sorceress's commission does not fall (suspect 6).
5. §1: "Starting gold 7500 and a starting army of 20 militia + 2 archers" is
   the Knight only. Paladin: 10000 gold, 20 peasants + 20 militia. Sorceress:
   10000, 30 peasants + 10 sprites. Barbarian: 7500, 20 wolves.
6. §14 item 2's "one of those two is wrong" is settled: the dead function
   name is the wrong one.

## Playthrough findings

| # | § / scenario | Expected (per guide) | Actual | Tag | Verdict |
|---|---|---|---|---|---|
