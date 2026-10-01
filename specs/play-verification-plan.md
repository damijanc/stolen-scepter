# Play Verification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Test the parity target itself. Settle the seven ranked suspects in
`specs/play-verification.md` §14 against the manual. Then run a human
playthrough of §1-§12 and turn every mismatch into a triaged finding.

**Architecture:** Three tasks with a hard split between what an agent can check
and what only a person at the keyboard can. Task 1 reads the manual OCR and
the pack, with no window. Task 2 is a guided playthrough: the user plays and
the session records and triages. The shell's window is macroquad-native, so no
session tool can see or drive it. Task 3 turns findings into proposed slices
and refreshes the guide. **This plan changes no Rust code.** A confirmed
defect becomes a separate slice the user approves.

**Tech Stack:** The built shell, `game-assets/.manual_ocr/page_NNN.txt`
(67 OCR'd pages of the scanned DOS manual), `specs/manual-reference-notes.md`,
and the pack's `game.json` in `reference/assets/kings-bounty/`.

## When to run this

**Run it after the sidebar HUD and the cheat menu have landed**
(`specs/sidebar-hud-plan.md`, `specs/cheat-menu-plan.md`). Before that, gold is
visible only in the Character view, which makes every economy scenario in §5,
§7 and §10 tedious. Without F10, setting up a late-game scenario means playing
to it. Task 1 has no such dependency and can run at any time.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a task, report it, ask. Do not chain commits.
- **Commit messages are a single subject line.** No body, no bullets, no
  trailers. Example: `docs: record the manual cross-check`.
- **Never commit `game-assets/` or `reference/`.** Quote at most a short phrase
  from the manual per finding and cite it by page; do not transcribe passages.
- **Do not invent gameplay fixes from the manual.** `AGENTS.md`: *"Use the
  manual only to validate data and user-facing facts."* A manual/C
  disagreement is RECORDED, not resolved in code. Only the user decides whether
  one becomes a deliberate divergence.
- **Treat the manual with caution.** `specs/manual-reference-notes.md` notes the
  "DOS" PDF describes Sega Mega Drive controls in places. An edition-specific
  statement is weak evidence.
- **No DOSBox is installed** on this machine (checked 2026-09-30). The DOS
  binary cannot be run from a session. Where the manual is silent and only the
  binary can settle a question, say so and leave it for the user.

---

### Task 1: Cross-check the §14 suspects against the manual

**Files:**
- Create: `specs/play-verification-findings.md`

**Interfaces:**
- Produces: a findings document with one section per §14 suspect, plus a
  running "Playthrough findings" table that Task 2 appends to.

For each of the seven §14 suspects, find what the manual says, find what the
pack declares, and find what the port does (the Rust `path:line` of the rule).
Grep the OCR with, for example,
`grep -il "fog\|explore\|sight" game-assets/.manual_ocr/*.txt`. OCR is noisy,
so try synonyms and partial words.

| # | Suspect (from §14) | What to look for in the manual | Where the port's rule is |
|---|---|---|---|
| 1 | Fixed 5x5 fog reveal, pack sight radius unread | Any statement of how far the hero sees, or a screenshot showing the revealed block | `engine/src/fog.rs` `reveal` |
| 2 | Only astrology-matching dwellings repopulate | Weekly astrology text: "week of the X" and what it says about population | `apply_week_end` in `engine/src` (grep `astrology`) |
| 3 | `gold <= cost` refuses an exact-cost purchase | Any purchase example where gold equals the price | town transaction cores (grep `gold <=` in `engine/src`) |
| 4 | Bridge and gate spells cost no charge at cast time | Spell descriptions for Bridge, Castle Gate, Town Gate | `engine/src/spells_adventure.rs` |
| 5 | Bridge caps at 2 tiles; C's comment says 5 | Bridge spell description | `try_build_bridge` in `engine/src/spells_adventure.rs` |
| 6 | Sorceress commission drop, Paladin spell regression | Appendix C, the character advancement chart | pack `classes` ranks in `game.json` |
| 7 | 14 of 32 combat-log strings parsed and never used | Combat section: the messages shown while picking a target or retaliating | `shell/src/combat.rs` string use |

- [ ] **Step 1: Write the findings skeleton**

```markdown
# Play Verification Findings

Manual cross-check run against `<git rev-parse --short HEAD>` on <date>.
Evidence: `game-assets/.manual_ocr/` (page numbers are PDF sheets; see
`tools/manual_word.py` for the sheet-to-printed-page mapping).

## §14 suspects

### 1. Fixed 5x5 fog reveal
- Manual: <page, and a paraphrase; at most one short quote>
- Pack: <field and value>
- Port: <path:line and what it does>
- Verdict: AGREES | DISAGREES | MANUAL SILENT | NEEDS BINARY

(... one section per suspect ...)

## Playthrough findings

| # | § / scenario | Expected (per guide) | Actual | Tag | Verdict |
|---|---|---|---|---|---|
```

- [ ] **Step 2: Fill all seven sections**

A verdict of `DISAGREES` needs both sides cited. `MANUAL SILENT` needs the
search terms you tried. `NEEDS BINARY` means the manual is silent and a DOSBox
run is the only way to settle it; state the exact observation to make in-game.

- [ ] **Step 3: Check the manual facts list too**

`specs/manual-reference-notes.md` "High-Value Manual Facts To Cross-Check"
lists starting class stats, the Archmage's location and price, siege cost, boat
rental, puzzle composition and the morale chart. Compare each against the pack
`game.json` and add one row per fact to a `## Manual facts` table in the
findings document, marking each AGREES or DISAGREES.

- [ ] **Step 4: Report, ask for approval, then commit**

```bash
git add specs/play-verification-findings.md
git commit -m "docs: cross-check the parity suspects against the manual"
```

---

### Task 2: Guided playthrough

**Files:**
- Modify: `specs/play-verification-findings.md` ("Playthrough findings" table)

The user plays; the session drives the checklist and records. The session
cannot see the game window, so ask the user for every observation. Never
assume one.

- [ ] **Step 1: Confirm the build and the gaps**

Run: `cargo build -p openbounty-shell --release`
Expected: builds cleanly.

Re-read `specs/play-verification.md` §13 and check each GAP row against the
code, since the guide warns rows go stale. Tell the user which sections are
not playable yet, so they skip them rather than report them.

- [ ] **Step 2: Give the user the launch command**

```bash
cargo run -p openbounty-shell --release -- --seed 1 --pack reference/assets/kings-bounty
```

A fixed `--seed` makes any finding reproducible. Record the seed in every row.

- [ ] **Step 3: Walk §1 through §12, one section at a time**

For each section, show the user that section's numbered scenarios and the
tagged claims they exercise. Wait for results, and record each mismatch as a
table row with its source tag. Record passes too, in a short line under the
table ("§2: all three scenarios agree"), so the next refresh knows what was
covered.

Use F10 cheats (once `specs/cheat-menu-plan.md` has landed) to set up late
scenarios: `G` for gold to test purchases at exact cost, `S` for siege
weapons, `N` to reveal a continent, `W`/`L` for the end screens. Say in the
row when a cheat set up the scenario.

- [ ] **Step 4: Classify each mismatch before recording a verdict**

| Tag on the claim | Verdict to record | Next step |
|---|---|---|
| [PORT] | PORT BUG if the port breaks its own documented divergence | a Rust fix slice |
| [PACK] | PACK DATA | a `game.json` issue (pack is upstream; note, do not edit `reference/`) |
| [C], and the port differs from C | PORT BUG | a Rust fix slice, with the C `file:line` |
| [C], the port matches C, but play contradicts the manual | C SUSPECT | record only; the user decides |
| no tag / guide is wrong | GUIDE ERROR | fix the guide in Task 3 |

To tell "port differs from C" from "C itself is off", read the C function
the claim cites and compare it to the Rust. This is the only code reading
Task 2 needs.

- [ ] **Step 5: Report, ask for approval, then commit**

Commit after each two or three sections rather than at the end, so a long
session does not lose its record.

```bash
git add specs/play-verification-findings.md
git commit -m "docs: record playthrough findings"
```

---

### Task 3: Triage and refresh the guide

**Files:**
- Modify: `specs/play-verification-findings.md` (add "Proposed slices")
- Modify: `specs/play-verification.md`

- [ ] **Step 1: Write the proposed slices**

Under `## Proposed slices`, one line per PORT BUG: slice name (for example
`fix: refuse gate spells without a visited destination`), the finding
number, and the C `file:line` it must match. List C SUSPECT rows separately,
as `## Decisions for the user`, each with the evidence on both sides.
Do not implement any of them.

- [ ] **Step 2: Refresh the guide**

In `specs/play-verification.md`: fix every GUIDE ERROR in place; change the
status of any §13 GAP you confirmed closed; add a one-line pointer at the top
of §14 to `specs/play-verification-findings.md`; and move the
"Last refreshed" line to today's date and `git rev-parse --short HEAD`.

- [ ] **Step 3: Report, ask for approval, then commit**

```bash
git add specs/play-verification.md specs/play-verification-findings.md
git commit -m "docs: triage the playthrough and refresh the guide"
```
