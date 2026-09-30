# Engine Gap Sweep Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Re-derive, mechanically and from the C tree, whether any engine core
is still unported -- and commit the sweep as a repeatable tool so the next
"nothing left" claim can be checked in one command instead of from memory.

**Architecture:** Two audits and one report. A committed shell script
(`tools/engine_sweep.sh`) does the name-matching half. A hand walk classifies
every miss. A field audit catches the gap shape the name sweep cannot see:
state the engine stores and saves but never reads or writes. Findings go to
`specs/engine-sweep-findings.md`. **This plan fixes nothing.** Each real gap it
finds is reported to the user as a proposed slice of its own.

**Tech Stack:** POSIX `sh`, `grep`, `sed`. No Rust changes unless a finding is
approved as a separate slice.

## Why this exists

`AGENTS.md` records three wrong "the engine is complete" claims: the v11 save
format, `spells_adventure.c` plus `GameSwitchZone`, and the win condition
(`flow_apply_search`, `GameSpendDays`, `GameComputeScore`). Each was caught by
the mechanical sweep or the stored-but-unread tell, never by recall. The
sweep has only ever been run by hand and never committed.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a task, report it, ask. Do not chain commits.
- **Commit messages are a single subject line.** No body, no bullets, no
  `Co-Authored-By` or other trailers. Example: `feat: commit the engine gap sweep`.
- **Never commit `reference/`, `game-assets/`, or build output.** The script
  READS `reference/`; its output is not committed, only the findings document.
- **Do not fix gaps inside this plan.** A confirmed gap becomes a proposal to
  the user: the C function, its callers, the state it touches, and a one-line
  slice name. Porting it is a separate session.
- **C code beats C comments.** Where a header comment and the code disagree,
  classify by the code (`AGENTS.md` "Parity Rules").

## Known state before you start

A prototype of the script was run at `f081b2c` on 2026-09-30:
283 declared functions, 203 HIT, 80 MISS. Filtering out the host prefixes left
**36 misses to walk by hand**:

```
adventure.h      adventure_handle_interact, adventure_walkable_in_flight
assets_bytes.h   LoadAssetBytes, UnloadAssetBytes
combat.h         combat_log_strings, combat_log_template, combat_run_headless_ex
fatal.h          fatal_user_error
flow_resolve.h   flow_apply_accept_friendly
flows.h          start_foe_hostile_flow
game.h           GameApplyTileMutations, GameFindCastleConst, GameFindFoeConst,
                 GameIsOver, GameTouchTown, furnish_map, player_accept_rank
map.h            MapGetTile, MapInBounds
resources.h      resources_art_manifest, resources_castle_at,
                 resources_count_bucket_label, resources_republish,
                 resources_town_at, resources_zone_town
savegame.h       SaveResultText
spells_adventure.h spells_adventure_reset_ui
tables.h         spell_index_by_id, troop_morale_groups
tile.h           ArtBlocksFoot, ArtIsBridge, InteractToString,
                 TerrainMoveCost, TerrainName, TerrainWalkable
```

Most of these will turn out to be ported under another name (`AGENTS.md`
already notes `flow_apply_navigate` is `resolve_navmap`), host-side, or
`*Const` twins that Rust's borrow rules make unnecessary. The point is to
**prove** that per row, not assume it. Re-run the script: the tip may have moved.

---

### Task 1: Commit the sweep script

**Files:**
- Create: `tools/engine_sweep.sh`
- Modify: `tools/README.md` (add one entry)

**Interfaces:**
- Produces: `sh tools/engine_sweep.sh` prints one tab-separated line per
  declared C function: `HIT|MISS <TAB> header <TAB> CName <TAB> short_name`,
  sorted and unique. Exit status 0. Task 2 consumes this output.

- [ ] **Step 1: Write the script**

```sh
#!/bin/sh
# Header-vs-Rust name sweep for the OpenBounty port.
#
# For every function declared in reference/engine/include/*.h, try three
# spellings against engine/src and shell/src as whole words:
#   the C name            GameBuyTroop
#   its snake_case form   game_buy_troop
#   the prefix-stripped   buy_troop   (Game/Map/Fog/resources_/... removed)
# HIT means one spelling appears somewhere. That is a HEURISTIC: a HIT can be a
# comment or an unrelated same-named function, and a MISS can be a core ported
# under a different name. Every MISS needs a hand check (AGENTS.md "Good Next
# Work"). Run from the repository root; needs the reference/ clone.
set -eu

if [ ! -d reference/engine/include ]; then
  echo "engine_sweep: reference/engine/include not found; run from the repo root with the OpenBounty clone present" >&2
  exit 1
fi

for h in reference/engine/include/*.h; do
  grep -hoE '^[A-Za-z_][A-Za-z0-9_ \*]*[ \*]([A-Za-z_][A-Za-z0-9_]*)\(' "$h" \
    | sed -E 's/.*[ \*]([A-Za-z_][A-Za-z0-9_]*)\($/\1/' \
    | while read -r fn; do
        snake=$(printf '%s' "$fn" | sed -E 's/([a-z0-9])([A-Z])/\1_\2/g' | tr 'A-Z' 'a-z')
        short=$(printf '%s' "$snake" | sed -E 's/^(game|map|fog|resources|adventure|flow_apply|spells_adventure|combat|pending|state|save)_//')
        if grep -rqw -e "$fn" -e "$snake" -e "$short" engine/src shell/src; then
          s=HIT
        else
          s=MISS
        fi
        printf '%s\t%s\t%s\t%s\n' "$s" "$(basename "$h")" "$fn" "$short"
      done
done | sort -u
```

- [ ] **Step 2: Run it and check the totals**

Run: `sh tools/engine_sweep.sh | cut -f1 | sort | uniq -c`
Expected: roughly `203 HIT` / `80 MISS` at `f081b2c`. Different numbers are
fine if the tip has moved; a total far from 283 means the declaration regex
broke on a header -- inspect before continuing.

Run: `sh tools/engine_sweep.sh | grep ^MISS | grep -vE 'player_io_|dialog_|prompt_|screen_|views_|pack_|SavePath|ui_host' | wc -l`
Expected: about 36.

- [ ] **Step 3: Document it**

Add to `tools/README.md`, matching the file's existing entry style:

```markdown
## engine_sweep.sh

`sh tools/engine_sweep.sh` lists every function declared in
`reference/engine/include/*.h` as HIT or MISS against `engine/src` and
`shell/src`. It is a name-matching heuristic: every MISS needs a hand check,
and a HIT can still be a core that does less than C. See
`specs/engine-sweep-findings.md` for the last classified run.
```

- [ ] **Step 4: Ask for approval, then commit**

```bash
git add tools/engine_sweep.sh tools/README.md
git commit -m "feat: commit the engine gap sweep"
```

---

### Task 2: Classify every miss by hand

**Files:**
- Create: `specs/engine-sweep-findings.md`

**Interfaces:**
- Consumes: `tools/engine_sweep.sh` output from Task 1.
- Produces: a findings table Task 3 appends to and the user reads.

For each non-host MISS, open its definition in `reference/engine/src/` (grep
`reference/engine` for `^[a-zA-Z].*\bNAME(`), read it and its callers, then
search the Rust tree for the BEHAVIOUR, not the name. Classify it as exactly one of:

| Class | Meaning | Evidence required |
|---|---|---|
| `PORTED-AS` | Ported under another name | the Rust `path:line` and one sentence showing it does the same thing |
| `INLINED` | Its body lives inside a ported caller | the caller's Rust `path:line` |
| `CONST-TWIN` | A `*Const` duplicate that Rust's `&`/`&mut` make redundant | the non-const twin's Rust location |
| `HOST` | Host/presentation code (logging, text formatting, asset I/O) | why it is presentation, citing what it returns or writes |
| `DIVERGENCE` | Deliberately not ported, already recorded | the `AGENTS.md` or module doc line that records it |
| `DEAD` | Declared or defined but never called in C | the grep showing zero callers in `reference/` |
| `GAP` | An engine rule with no Rust counterpart | the C behaviour, its callers, and the state it touches |

Specific traps already known, so you do not have to rediscover them:

- `fatal_user_error` -- `AGENTS.md` records the `GAME_CONTINENTS` capacity
  contract as a known, deliberately unresolved DIVERGENCE. Classify it as that;
  do not propose a fix unless you find a second caller that matters.
- `combat_log_template` / `combat_log_strings` -- `AGENTS.md` says the engine
  records DATA and carries `CombatEvent`s instead of formatted lines. Likely
  `HOST`; confirm the Rust shell formats the same strings (grep `shell/src/combat.rs`).
- `adventure_walkable_in_flight`, `TerrainWalkable`, `TerrainMoveCost`,
  `MapWalkable` -- movement is ported; find where each rule lives.
- `GameApplyTileMutations`, `furnish_map` -- tile mutation persistence and
  placements are ported; find them in `engine/src/game.rs` or `map.rs`.
- `player_accept_rank`, `GameTouchTown` -- check carefully. A rank promotion or
  a "town visited" mark that nothing performs would be a real GAP of exactly
  the stored-but-never-written shape.

- [ ] **Step 1: Write the findings skeleton**

```markdown
# Engine Sweep Findings

Run against `<git rev-parse --short HEAD>` on <date>. Script:
`tools/engine_sweep.sh`. Totals: <N> declared, <H> HIT, <M> MISS, <K> after the
host-prefix filter.

## Name sweep: classified misses

| Header | C function | Class | Evidence |
|---|---|---|---|

## Field audit

(Task 3)

## Proposed slices

(Only `GAP` rows. One line each: slice name, C function, state touched.)
```

- [ ] **Step 2: Walk every row**

Fill one table row per non-host MISS. No row may say "probably" -- if you
cannot find the evidence, the class is `GAP` until proven otherwise.

- [ ] **Step 3: Spot-check five HITs**

The sweep cannot see a core "whose Rust counterpart is named similarly but does
less" (`AGENTS.md`). Pick the five HITs with the largest C bodies in
`reference/engine/src/game.c` and `step.c` (count lines between the opening and
closing brace) and compare branch by branch. Record any branch the Rust
version lacks as a `GAP` row, with the branch quoted.

- [ ] **Step 4: Report, ask for approval, then commit**

```bash
git add specs/engine-sweep-findings.md
git commit -m "docs: classify the engine sweep misses"
```

---

### Task 3: Audit stored-but-never-read state

**Files:**
- Modify: `specs/engine-sweep-findings.md` (fill the "Field audit" section)

This is the tell that found the last three gaps: `game.scepter` placed and
saved with nothing to dig it up; `Mount::Fly` honoured by movement with no way
to enter it; `stats.siege_weapons` buyable but never revoked.

- [ ] **Step 1: List every state field**

List the `pub` fields of every struct reachable from `Game` in
`engine/src/game.rs` (and the structs it embeds: stats, character, world,
spells, army, castles, foes, fog). Save the list to your scratchpad directory, not the repo.

- [ ] **Step 2: Count readers and writers per field, excluding persistence**

For each field `f`, run:

```sh
grep -rnw "$f" engine/src shell/src \
  | grep -v -e 'engine/src/savegame.rs' -e 'engine/src/snapshot' -e '#\[cfg(test)\]' -e '/tests'
```

Then classify each hit by eye as a READ or a WRITE (`= `, `+= `, `-= `,
`.push(`, `&mut` passed on). Unit tests do not count. `savegame.rs` and the
snapshot builder do not count -- persisting a field is not using it.

- [ ] **Step 3: Record the suspicious fields**

A field is suspicious when, outside persistence and tests, it has:

- writers but no readers (state accumulated for nothing), or
- readers but no writer other than init/load (a mode that cannot be entered), or
- a writer in only one direction (set, never cleared; bought, never revoked).

For each suspicious field, check C: grep `reference/engine` and `reference/src`
for the C field name. If C reads/writes it where Rust does not, it is a `GAP`
row. If C has the same hole, record it as `C-SAME` (for example
`game.hud_visible`, which `specs/play-verification.md` already lists as DEAD --
do not wire it up).

- [ ] **Step 4: Report, ask for approval, then commit**

Report to the user: the `GAP` rows from Tasks 2 and 3, each with a proposed
one-line slice name. Do not start porting any of them.

```bash
git add specs/engine-sweep-findings.md
git commit -m "docs: audit engine state for unread fields"
```

- [ ] **Step 5: Update AGENTS.md**

In `AGENTS.md` "Good Next Work", replace the paragraph beginning "The check
that actually works is mechanical" so it names `sh tools/engine_sweep.sh` and
`specs/engine-sweep-findings.md` instead of describing the sweep in prose.
Leave the "three wrong claims" history intact. Ask, then commit:

```bash
git add AGENTS.md
git commit -m "docs: point the gap check at the committed sweep"
```
