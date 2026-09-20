# Play Verification Guide

## Why this document exists

Every parity claim in this repository bottoms out at OpenBounty's C engine.
That is deliberate -- `AGENTS.md` makes C the authority -- but it leaves one
risk uncovered: **nothing checks whether C itself is complete and faithful to
the original game.** The C tree may be missing behavior, may have simplified
something, or may carry a bug the port has now reproduced perfectly.

This document is the mitigation. It describes the game's behavior in plain
language, in the order a player meets it, with concrete scenarios to check while
playing. When play disagrees with what is written here, that disagreement is the
finding -- and the source tags below say which layer to suspect.

This is NOT a parity spec. It is a play-facing description. Where it states a
number, the number came from the pack or the committed code, not from memory.

## Source tags

Every behavioral claim carries the layer it came from:

- **[C]** -- ported from the OpenBounty C engine. The port reproduces it, so a
  mismatch in play means C is wrong or incomplete. **These are the interesting
  ones.**
- **[PACK]** -- a value declared in `game.json`. A mismatch means the pack data
  is wrong, which is the cheapest kind of fix.
- **[MANUAL]** -- corroborated against the DOS manual in `game-assets/`. Where
  a [C] and a [MANUAL] claim disagree, that disagreement is recorded here rather
  than resolved.
- **[PORT]** -- a deliberate Rust-side divergence from C. Documented in
  `AGENTS.md` or in the module that owns it.

## Status legend

- **OK** -- implemented and playable at the tip this document was written
  against.
- **GAP** -- not built yet. Do not try to verify it; it is listed so the map of
  the game is complete.
- **DEAD** -- present in the pack or C headers but never used by C. Listed so
  nobody "fixes" it into existence.

## How to run

```sh
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

The pack lives in the gitignored `reference/` clone. The shell resolves art and
map paths relative to it.

## Reporting a mismatch

When something in play disagrees with this document, record:

1. What you did (the scenario number, if one applies).
2. What you expected, per this document.
3. What actually happened.
4. Which layer you suspect, using the source tag.

A [C] mismatch is the most valuable: it means the parity target itself is
suspect and needs checking against the manual or the original binaries in
`game-assets/`.

---

# 1. Starting a game

**Status: GAP.** The class picker, name entry, difficulty selection and the save
slot picker are all unported (`reference/src/startup.c`, 856 lines). The shell
hardcodes a Knight named "sir rust" on seed 42, so every run is identical.

What the finished flow owes, once built:

- Four classes [PACK] [MANUAL] -- these four stat lines are confirmed to match
  the manual exactly, which is a good sign for the pack's fidelity:

  | Class | Leadership | Max spells | Spell power | Weekly commission | Starts knowing magic |
  |---|---|---|---|---|---|
  | Knight | 100 | 2 | 1 | 1000 | no |
  | Paladin | 80 | 3 | 1 | 1000 | no |
  | Sorceress | 60 | 5 | 2 | 3000 | **yes** |
  | Barbarian | 100 | 2 | 0 | 2000 | no |

- Starting gold 7500 and a starting army of 20 militia + 2 archers [PACK].
- Four difficulties, which set the day budget [PACK]: easy 900, normal 600,
  hard 400, impossible 200.

**Verify when built:** that the Sorceress alone can cast from turn one, and that
choosing a difficulty actually changes the starting day count.

---

# 2. Time and the calendar

- A day is **40 steps** [PACK]. Walking 40 tiles advances the date by one day.
- A week is **5 days** [PACK]. Not seven.
- The day budget is set by difficulty (above). Running out ends the game.
- Resting in place (numpad 5) consumes a step without moving [C].

**Verify in play:**

1. Count 40 steps and confirm the day counter advances exactly once.
2. Count 5 day-rolls and confirm the week-end transaction fires (see §10).
3. Confirm the status bar's days-left figure starts at the difficulty's number.

---

# 3. Movement, terrain and fog

- The hero walks the overworld a tile at a time; diagonals are allowed, and a
  diagonal key overwrites both axes after the cardinal pass [C].
- Opposing keys pressed in the same frame resolve to the first one checked
  rather than cancelling out [C]. This is load-bearing C behavior, not a bug.
- Water needs a boat; some terrain needs flight (see §8).
- **Fog reveals a fixed 5x5 block around the hero, always.** [C] [PORT]
  `Fog::reveal` accepts a radius argument and ignores it. The pack declares
  `world.fog_sight: 3` -- which would imply a 7x7 reveal -- and **nothing reads
  that field.** **DEAD.**

**Verify in play:**

1. Walk into unexplored territory and count the revealed block. It should be
   5x5 centred on the hero, not 7x7.
2. **This is a high-value check.** If the real game reveals a different area,
   then either C's fixed 5x5 is wrong or the pack's `fog_sight: 3` was meant to
   drive it. The manual's map appendices may settle it.

---

# 4. The overworld and what you can walk into

The world is **4 continents** [PACK]: Continentia (start), Forestria,
Archipelia, Saharia. It holds 26 towns, 27 castles, 17 villains, 8 artifacts and
25 troop types.

Interactions, all **OK** unless marked:

| You step on | What happens |
|---|---|
| Sign | Shows its text [C] |
| Treasure chest | Gold-or-leadership choice, or a message (see §11) [C] |
| Artifact | Picked up, permanent effect |
| Orb | Reveals the continent map |
| Navmap | Discovers a continent for sea travel |
| Telecave | Teleports to its paired cave |
| Magic alcove | Offers to teach magic, 5000 gold [PACK] [MANUAL] |
| Dwelling | Recruit screen (see §7) |
| Town | Town menu (see §5) |
| Castle gate | Home, owned, or hostile castle (see §6) |
| Hostile foe | Attack prompt, then combat (see §9) |
| **Friendly foe** | **GAP** -- the join offer is unported; nothing happens |

**Verify in play:**

1. Walk onto a friendly (non-hostile) foe. Expect nothing to happen today. This
   is the most visible remaining hole.
2. Pick up the Anchor of Admiralty and confirm boat rental drops 500 -> 100 (§5).
3. Confirm a telecave's destination is stable across repeated uses.

---

# 5. Towns

The town menu offers: get a new contract, rent or cancel a boat, gather
information, and buy the town's spell.

- Boat rental **500 gold**, or **100** with the Anchor of Admiralty [PACK]
  [MANUAL].
- The rent check uses C's `gold <= cost` condition, so **paying the exact price
  fails** [C]. With exactly 500 gold you cannot rent a 500-gold boat.
- A town sells one specific spell; spell costs range 100-5000 [PACK].
- Contracts are offered on a **5-slot cycle** [PACK].

**Verify in play:**

1. **The exact-gold boat refusal.** Get to exactly 500 gold and try to rent.
   Expect refusal. If the real game lets you buy at exactly the asking price,
   C's off-by-one is wrong and this is a [C] finding.
2. Confirm the same exact-cost rule on siege weapons (3000) and the alcove
   (5000) -- check whether they behave consistently with the boat.
3. Rent a boat, then cancel it, and confirm the refund behavior matches what the
   manual describes.

---

# 6. Castles

Three kinds, distinguished at the gate:

- **Home castle (King Maximus)** -- OK. Throne room with two options: recruit
  soldiers, or an audience with the King.
- **Owned castle** -- OK. Garrison troops in or out.
- **Hostile castle** -- OK. Prompts for a siege, then runs combat (§9).

**Audience with the King** [C] is the **only** way to gain rank. The hero is
never promoted automatically on capturing a villain. Rank thresholds [PACK]:

| Class | Rank 2 | Rank 3 | Rank 4 |
|---|---|---|---|
| Knight | General (2 villains) | Marshal (8) | Lord (14) |
| Paladin | Crusader (2) | Avenger (7) | Champion (13) |
| Sorceress | Magician (3) | Mage (6) | Archmage (12) |
| Barbarian | Chieftain (1) | Warlord (5) | Overlord (10) |

**Two anomalies worth checking against the manual's Appendix C** -- both are
[PACK] values the port faithfully reproduces:

1. **The Sorceress's commission DROPS on her first promotion**, 3000 -> 1000,
   and never recovers. Every other class's commission rises or holds.
2. **The Paladin's max spells go 3 -> 3 -> 6 -> 5.** The Champion rank is
   *worse* than the Avenger rank it follows.

If the manual's advancement chart disagrees, these are pack-data bugs.

**Verify in play:**

1. Capture a villain and confirm you are **not** promoted until you visit the
   King.
2. At the audience with too few villains, confirm the King tells you how many
   more are needed, with correct singular/plural ("1 more villain" vs "2 more
   villains").
3. Promote a Sorceress and watch the commission figure. Expect it to fall.

---

# 7. Dwellings and recruiting

- A dwelling offers one troop type with a weekly population.
- The recruit cap is the smallest of: what your leadership allows, what your
  gold affords, and the dwelling's remaining population [C].
- Home-castle recruitment draws from a **bottomless** pool [C] -- the castle
  never runs dry, unlike a dwelling.
- Weekly astrology repopulates **only** dwellings matching that week's troop
  [C]. Note this contradicts a C function name (`GameGrowDwellings`) that
  suggests all dwellings grow; the executable behavior is what is ported.

**Verify in play:**

1. Empty a dwelling, then pass a week where astrology names a *different* troop.
   Expect the dwelling to stay empty.
2. Then pass a week where astrology names *that* dwelling's troop and confirm it
   repopulates.
3. **This is a high-value [C] check.** If the real game repopulates all
   dwellings weekly, C's selective growth is wrong.

---

# 8. Spells

Fourteen spells [PACK], seven combat and seven adventure.

Adventure spells: bridge, time stop, find villain, castle gate, town gate,
instant army, raise control. Combat spells: clone, teleport, fireball,
lightning, freeze, resurrect, turn undead.

Charge accounting is **not uniform** [C] -- this is a genuine C quirk:

- Time stop and raise control spend on cast.
- Find villain spends only once a contract exists.
- Instant army spends only on success.
- **Bridge and both gate spells spend nothing at cast time at all.**

Other quirks:

- Bridge builds at most **2 tiles**, despite a C comment claiming up to 5 [C].
- Flight requires **every** non-empty stack to have FLY *and* skill level 2+, so
  a sprite stack grounds the hero even though sprites fly in combat [C].
- Landing requires plain open grass and ignores bridges, so a built bridge can
  be landed on [C].

**Verify in play:**

1. **Cast bridge and count the tiles.** Expect 2, never more.
2. Cast a gate spell and confirm no charge is consumed.
3. Try to fly with a mixed army containing sprites. Expect to be grounded.

---

# 9. Combat

**Status: OK.** Fully playable.

- The field is a fixed **6x5 grid** of 48x34 cells, filling the screen with no
  sidebar [C].
- Keys: arrows move, `S` shoot, `F` fly, space/`W` wait, `A`-`G` pick a spell
  while casting, `G` give up (outside spell selection), ESC.
- `A` opens the Army view and `V` the Character view, over the still-visible
  battlefield. **Opening a view pauses the fight** -- no AI advancement and no
  animation ticks while it is up [C]. Both are read-only and close on any key.
  Both openers are suppressed while a spell cast or a target picker is
  mid-flight, because `A` is also the picker's confirm key [C].
- C also opens an Options menu from combat with `O`. **GAP** -- the shell has no
  Options view yet (§13), so that key does nothing here.
- The AI acts **once per completed 4-frame animation cycle** (150ms per frame)
  so its units visibly walk [C].
- Retaliation is melee-only, once per round per defender, and ranged attacks
  never provoke it [C].
- A damage burst persists ~3 animation ticks and shows on **every** connecting
  attack, including one that kills the whole stack [C].
- Victory credits spoils, writes surviving stacks back, and compacts the army so
  the army view has no gaps [C].
- Defeat forfeits army, siege weapons and boat, and returns the hero to the home
  spawn -- **but currently shows no message. GAP** (see §13).
- Morale is a 5x5 chart [PACK]; check it against the manual's morale chart.

**Verify in play:**

1. **Watch the AI's turn.** Units should walk tile by tile, not teleport. If the
   whole enemy turn resolves instantly, the rollover pacing has regressed.
2. Kill a stack outright and confirm the splat still plays over the empty cell.
3. Shoot a stack and confirm it does **not** retaliate; melee the same stack and
   confirm it does, once.
4. Press `G` during spell selection and confirm it picks the 7th spell rather
   than giving up.
5. Open the Army view mid-fight with `A` and confirm the AI stops moving behind
   it. Then arm a shoot picker with `S` and press `A` -- expect it to confirm
   the pick, **not** open the Army view.
5. Check the title bar during combat: before the first kill it should read
   `Options / <troop> M<moves>` (plus `,S<shots>` only when shots remain);
   after the first kill it switches to a "X vs Y killing N" summary.

---

# 10. The economy and the week

At each week's end [C], in this order: bank the commission, pay army upkeep,
then charge the boat fare.

- The fare is judged on the **post-commission** wallet, so a poor hero whose
  commission covers the fare **is** charged [C].
- Astrology names a troop for the week and repopulates matching dwellings (§7).

**Verify in play:**

1. End a week nearly broke with a boat rented, where the commission alone covers
   the fare. Expect to be charged, not to lose the boat.
2. Confirm the order on screen: commission credited before upkeep debited.

---

# 11. Treasure chests

A chest rolls one of several outcomes on a weighted table that varies by
continent [PACK]: gold, a commission increase, spell power, max spells, or a new
spell. The gold outcome offers a choice -- take the gold, or distribute it to
the peasants for leadership [C].

**Verify in play:**

1. Take the peasant option and confirm leadership rises rather than gold.
2. Open chests on different continents and confirm later continents yield
   noticeably larger amounts -- the tables are per-continent.

---

# 12. Contracts, villains and the endgame

- 17 villains, 8 artifacts, 25 puzzle pieces [PACK] [MANUAL] -- the manual
  describes 25 pieces as 16 villains + Arech + 8 artifacts.
- Capturing a villain requires holding its **contract**. Without it, you can
  still beat the Lord, but he goes free, keeps his castle and rebuilds his army
  from the pack roster [C]. The defeat is remembered, so pre-weakening a lord
  before his contract window is a legitimate tactic.
- The scepter is buried; digging on its tile wins. Digging anywhere else costs
  **10 days** [PACK] and can run the calendar out [C].
- Final score: 500/villain + 250/artifact + 100/castle, minus followers killed,
  multiplied by difficulty (0/1/2/4/8), and **halved on easy** [PACK].

**Verify in play:**

1. Siege a villain's castle without his contract. Expect him set free with his
   castle intact and his army rebuilt.
2. Dig on a wrong tile and confirm exactly 10 days are spent.
3. Confirm the victory banner names the **villain**, not the castle. *Known
   issue: it currently names the castle. See §13.*

---

# 13. Known gaps and issues at the time of writing

Do not file these as findings; they are already tracked.

| Area | Status | Effect in play |
|---|---|---|
| Startup / class select / difficulty | GAP | Always a Knight on seed 42 |
| Save / load | GAP | No persistence at all |
| Options menu | GAP | Key does nothing |
| Friendly-foe join offer | GAP | Stepping on a friendly foe does nothing |
| Defeat message | GAP | Army lost and hero teleported with no explanation |
| Victory banner naming | ISSUE | Names the castle, not the villain |
| Sidebar HUD, overlay | GAP | Not drawn |
| Audio | GAP | Silent |
| End cartoon | GAP | No victory sequence |
| Pack picker | GAP | Pack path passed on the command line |

---

# 14. Where C is most likely to be incomplete

The point of this document. Ranked by how likely a gap is and how visible it
would be in play:

1. **The fixed 5x5 fog reveal** (§3). The pack declares a sight radius that
   nothing reads. That looks like an abandoned feature, which is exactly the
   shape of a C simplification.
2. **Selective dwelling repopulation** (§7). A C function name implies all
   dwellings grow; only astrology-matching ones do. One of those two is wrong.
3. **The exact-cost purchase refusal** (§5). `gold <= cost` is the kind of
   off-by-one that survives in a reimplementation precisely because nobody
   tests the boundary.
4. **Non-uniform spell charge accounting** (§8). Bridge and the gates costing
   nothing at cast time is odd enough to be either a faithful quirk or a C bug.
5. **The bridge's 2-tile cap** (§8), where C's own comment says 5.
6. **The Sorceress commission drop and the Paladin spell regression** (§6).
   Check both against the manual's Appendix C.
7. **14 of 32 combat-log strings are parsed and never used by C** -- including
   all six picker instructions and the retaliation line. Either the original
   showed text C does not, or the pack carries strings the original never had.
   The manual's combat section may settle this.

For items 1-6, the DOS binary in `game-assets/Kings-Bounty_DOS_EN.zip` is the
final arbiter, and the manual is the cheaper first check.

---

## Maintenance

Update this document when a GAP closes or a listed issue is fixed. Keep the
source tags accurate -- an untagged claim is not verifiable, because a reader
cannot tell which layer to suspect when play disagrees.
