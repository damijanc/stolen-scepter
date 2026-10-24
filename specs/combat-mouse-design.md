# Combat Mouse Design

Status: APPROVED 2026-10-02 (all four decisions as recommended). Slice 4 of the P11 mouse work.
Read `specs/mouse-support-design.md` first; its rules all apply here.

**DIVERGENCE, deliberate.** C's combat (`combat_loop.c`) is keyboard-only.
This adds pointer input to the shell's `CombatLoop` and nothing else. Every
click produces exactly what a key already produces, by calling the same
`CombatLoop` method the key calls. So RNG order, the once-per-round spell
latch, refusal banners and the turn machinery stay those of the keyboard path.

## 0. Prerequisite: two port gaps (not mouse work)

Found while reading `combat.rs` for this note. Both break keyboard play too:

| Gap | C | Rust |
| --- | --- | --- |
| Target ring | `combat_render.c:191-200` draws `combat_tile[11 + (cursor_frame & 3)]` at the cursor while `picker_active` | `combat_render` omits it ("minus the cursor"). Shoot, fly, spell and teleport aiming show no cursor |
| Spell menu | `combat_loop.c:328-349` draws the 240x130 "Spells / Combat" panel with counts and A-G while `cast_phase == PICK_SPELL` | not drawn; the player picks a letter blind |

Proposed as their own slice, BEFORE the mouse slice:
`fix: draw the combat target ring and spell menu`. It is a parity port with C
as the reference, and it gives the mouse something to point at.

## 1. What a click means

All coordinates are logical (the `DrawList` space). Cell rects come from the
same `cell_origin` the field is drawn with, so regions cannot drift.

| State | Click | Same as |
| --- | --- | --- |
| Player's turn | one of the 8 cells around the acting unit | that direction key: move, attack, or a silent refusal (obstacle, own stack) |
| Player's turn | an enemy farther away | `S`, then the cursor on that cell, then Enter. Refusals log the `S` banners ("No ammo", "Can't Shoot") |
| Player's turn | the acting unit itself | opens the **action menu** (below) |
| Player's turn | any other cell | nothing. There is no pathfinding in combat: walking more than one step is new behaviour, and out of scope |
| Picker armed (shoot / fly / spell target / teleport destination) | a cell | move the cursor there, then Enter. A cell that fails the filter does nothing, as Enter does |
| Pick-spell menu | a spell row | its letter A-G (including the "No spells of that type" refusal) |
| Give-up prompt | Yes / No | `Y` / `N` (the prompt's existing regions; combat does not route clicks to it today) |
| AI's turn | anything | nothing |
| A view open over the field | as today | `combat_view_input`, unchanged; the fight stays paused |

**Right-click is ESC in combat:** it cancels a picker, the spell menu, or the
action menu. This is the first binding for `Button::Right`. It mirrors the
Amiga's right-button-backs-out habit, and in combat it is the only way to
cancel without ESC.

*Changed 2026-10-24 (playtest feedback: the action menu was hard to find):*
with nothing open to back out of, a right-click OPENS the action menu. It
still backs out of everything it backed out of before.

**The pointer names the click** (added 2026-10-24): over a field cell on the
player's turn it is a move, attack, shoot or spell-target shape, or a red
"no" sign where a click does nothing, decided by the same rules the click
uses (`CombatLoop::pointer_shape`, shapes in `cursor.rs`). The arrow
elsewhere. **The title bar alternates** the unit line with the key tips
every 3 seconds on the player's turn, and says how to pick while a picker is
armed (`strings.combat_log.key_tips` / `pick_hint`).

**Hover moves the target ring** while a picker is armed: the "one highlight"
rule from the Game Menu. The ring is where Enter or a click would land.

### The action menu

`S`, `F`, `U`, `W`/Space, `G` and the view keys have no on-screen affordance,
and the field fills the whole inner area. Clicking the acting unit opens a
small panel beside it, clamped to the field: **Wait, Shoot, Fly, Cast, Give
up, Army, Character, Options, Controls**. An entry is exactly its key, refusal
banners included, so it shows every action rather than guessing which are
legal. It closes on a pick, on a right-click, or on a click off the panel.
It is shell UI that C does not have.

Fly gets no smart click (a click on a far empty cell does nothing). Spending a
flight on a misclick is too easy, and the menu reaches it in two clicks.

## 2. Pacing and pauses

- A click is consumed only where `step` reads keys, behind the same gates: not
  on the AI's turn, not while the give-up prompt owns input (it gets its own
  click there), and never while a view is open. In that case `main.rs` calls
  `combat_view_input` instead of `step`, as it does now.
- The AI keeps its SYN-tick pacing; a click cannot advance a tick or act for
  the AI.
- The action menu is not a view, so opening it does not pause anything. But
  it only exists on the player's turn, when nothing else moves anyway.

## 3. Shape

- `HitTarget::CombatCell(x, y)`, `CombatSpell(i)`, `CombatMenu(entry)`,
  `CombatMenuClose`. Published by `combat_render` (cells), the spell menu draw,
  and the action menu draw.
- `CombatLoop::step` gains `click: Option<HitTarget>` and `cancel: bool`
  (right-click). The key handlers that a click shares are split out
  (`arm_shoot`, `arm_fly`, `open_cast`, `wait`, `give_up`, `pick_confirm`).
  The key path and the click path call them, so the two cannot diverge.
- Tests (headless, through `PointerSource`/`HitList`): an anti-drift test per
  surface; for every row of the table above, a click equivalence test against
  the key path, comparing the whole `Combat` and the loop state afterwards (so
  RNG drift would show); a click on the AI's turn changes nothing; a
  right-click cancels each picker kind.

## Decisions (approved 2026-10-02)

1. The two port gaps land first, as their own commit.
2. Clicking the acting unit opens the action menu.
3. A far enemy shoots; a far empty cell does not fly.
4. Right-click is ESC in combat.

## As built (2026-10-02)

Shipped as planned, with these details settled in code:

- The pointer reaches the fight as one value, `PointerInput { click, cancel }`,
  through `CombatLoop::step_input`. `step` is the keys-only wrapper the
  existing tests use. The shared handlers are `wait`, `arm_shoot`, `arm_fly`,
  `open_cast`, `give_up`, `cursor_passes` (the confirm half of
  `combat_pick_step`) and `resolve_pick`; the key path and the click path both
  call them.
- The spell menu publishes the same field-wide `CombatBack` backdrop as the
  action menu, so a click off either panel is ESC. The row under the pointer
  draws yellow (C's rows are all white).
- Hover steers the target ring only on ENTERING a cell. A pointer already
  resting on a cell when `S` arms the picker leaves the ring on the acting
  unit, where `S` puts it, as in C.
- The action menu's labels come from `strings.combat_menu` (`wait`, `shoot`,
  `fly`, `cast`, `give_up`, `army`, `character`, `options`, `controls`), with
  shell defaults that lead with the key ("S Shoot").
- Test harness note: comparing two whole fights (`Combat` + `Game`, both large
  fixed-size structs) overflows a debug test thread's 2 MB stack. The
  equivalence tests run on a 64 MB thread (`big_stack`).
