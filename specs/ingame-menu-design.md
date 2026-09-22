# In-Game Menu Design

Status: agreed 2026-09-22. Covers a Game Menu reachable during play, offering
Save, Load and Quit. Its purpose is to make save-and-continue and mid-session
load possible at all; today `Q` saves slot 0 and quits in the same breath, and
loading is only reachable from the startup wizard.

Read `AGENTS.md` and `specs/shell-renderer-plan.md` first. Everything there
still applies -- parity-first translation, one-line commit messages, approval
before every commit.

## Why this is a deliberate divergence

**C's Game Menu is unreachable.** `view_kind.h:13` describes `VIEW_MENU` as
*"Esc / O - unified Game Menu (view switch + save/load/quit)"*, and the whole
apparatus exists -- `views_menu_update`, `ROOT_PAGE` / `VIEWS_PAGE` /
`SYSTEM_PAGE`, `draw_menu`, `shell_menu.c`'s four callbacks, the `MenuCtx`
wiring in `main.c:845` -- but **nothing in the C tree ever calls
`views_set(VIEW_MENU)` or `views_push(VIEW_MENU)`.** `O` opens `VIEW_OPTIONS`
(the controls reference, already ported) and ESC dismisses views.
`main.c:1098` calls `views_menu_update` only inside
`else if (views_active() == VIEW_MENU)`, a condition that can never be true.

That comment is the exact trap `AGENTS.md` warns about: *"Treat a comment as
secondary to executable C behavior when they disagree."* There is no observable
C behaviour here to port.

**DIVERGENCE, deliberate.** This is a shell feature designed on purpose, as
mouse support was. C's structures are a useful starting shape, not an
authority -- where C's dead code and a sane menu disagree, this document wins,
and each such point is recorded below. The feature is confined to the shell:

- The engine gains nothing. `engine/tests/render_free.rs` must still pass.
- The menu reaches no transaction the keyboard cannot. Save and load already
  exist as engine calls; the menu only chooses when to make them.

## Decisions

### 1. ESC opens it, and only on the overworld

ESC has no top-level binding today. Every use of it -- `main.rs:906`, `:990`,
`:1147`, `startup.rs` -- sits inside a view, prompt or picker branch, so a bare
ESC with nothing open currently does nothing. That makes it free, it matches
C's own comment, and it is what a player expects.

ESC keeps every existing meaning. Only the bare case is new:

| Context | ESC does |
|---|---|
| A view is open | dismiss it (unchanged) |
| A prompt or dialog is up | cancel it (unchanged) |
| The menu is open | close the menu |
| Combat | unchanged -- see Decision 2 |
| Overworld, nothing open | **open the Game Menu** |

Rejected: **a free letter** (`G`, `E`, `K`, `R`, `T`, `X`, `Y`, `Z` are
unused). Cleaner in that ESC stays purely a cancel key, but it spends one of
the few remaining letters on something ESC already implies, and is less
discoverable. Rejected: **both**, which doubles the suppression rules and the
Controls-reference rows for one action.

### 2. The menu is not available during combat

This is forced, not stylistic. `save_write` and `save_read`
(`engine/src/savegame.rs:45`) take `(path, game, resources, map, fog)` and
carry **no combat state** -- the only occurrence of "combat" in `savegame.rs`
is a spell in a test fixture. Saving mid-battle would silently discard the
fight and reload the hero standing on an empty field.

So ESC in combat keeps its current meaning and the menu never opens there. A
player who wants to save finishes or flees the battle first.

Recorded because it is invisible from the UI and someone will eventually try to
"fix" it: the fix is not opening the menu in combat, it is serializing combat,
which is a much larger slice nobody has asked for.

### 3. Three entries: Save, Load, Quit

C's root menu is Views / Options / Back / Exit, with Views holding Army,
Spells, Character, Contract, Puzzle and View Map, and Options holding Save,
Load, New Game and Back.

**The Views submenu is dropped.** Every one of those six already has a single
key on the overworld (`A`, `U`, `V`, `I`, `P`, `M`, from `input.rs:191`).
A menu path to a key that already works is pure duplication, and it is the
kind of surface that rots when a key changes.

**"New Game" is dropped.** C's `menu_new` does not start a new game: it
teleports the hero to the spawn point, clears the boat, re-inits fog and
toasts -- while keeping gold, army, villains caught and days elapsed. Shipping
that under the label "New Game" would lie to the player. A genuine restart is
the startup wizard, which already exists and does it correctly.

**Quit reuses the ported fast-quit confirm** (`StatusMode::FastQuit`) rather
than inventing a second confirmation. With Save sitting directly above it in
the same menu, a quit-without-saving path is the honest one.

So:

```
Game Menu
  Save
  Load
  Quit
```

ESC closes. The menu is a view, so it inherits dismissal, the paused
background and click handling from the existing stack.

### 4. Save and Load share one extracted slot picker

`startup.rs:599` (`draw_save_picker` / `update_save_picker`) already implements
a complete 10-slot picker: it reads headers through `savepath::scan_slots`,
shows name, rank title and days-left per slot, wraps the cursor, and confirms
on Enter/Space. It is coupled to `Startup`'s state machine only by accident of
where it was written.

It moves to its own module and both callers use it. Two implementations of a
save picker would drift, and the drift would be invisible until someone lost a
save to it.

The widget stays **dumb**: it owns the cursor and the scanned headers, and
reports which row was chosen. The caller decides what that row means.

- Startup passes an extra trailing "New game" row and treats an empty slot as
  "start a new game here". Unchanged behaviour.
- The menu passes no extra row. For Save every slot is selectable, and choosing
  an occupied one confirms through the existing `Prompt::yes_no` before
  overwriting. For Load an empty slot is a no-op with a message.

Rejected: **slot 0 only, as C's `menu_save`/`menu_load` hardcode.** One
keystroke and no refactor, but the shell already offers 10 slots at startup, so
the menu would silently ignore nine of them and overwrite slot 0 without
warning. Rejected: **quick-save to slot 0 with a picker only for Load**, which
optimises the common action but is asymmetric in a way that surprises.

### 5. Loading in-game is transactional

Startup pre-initialises a default game before loading *"so a failed load still
leaves something playable"* (`main.rs:714`). In-game that reasoning inverts: a
failed load must not destroy a live session.

The load recipe is four coupled steps, currently inline at `main.rs:712-736`:

1. `save_read(path, &mut game, &resources, &Map::new(1, 1), &mut fog)`
2. read `game.position.zone`
3. `Map::load_zone_with_placements(&resources, &zone, &game)`
4. `map.apply_consumed(&game, &zone)`

The menu runs all four into **temporaries** and swaps them into the live
`game` / `map` / `fog` only if every step succeeds. Any failure reports and
leaves play untouched.

That recipe also gets extracted, for the same reason as the picker: two
callers, one function, or they drift. Startup keeps its own pre-initialise
behaviour by calling the shared loader and handling the error differently.

## Components

- `shell/src/slot_picker.rs`: the extracted widget (`SlotPicker`, its draw and
  update, and the row-choice outcome), plus `load_slot`, the shared four-step
  recipe returning `Result<(Game, Map, Fog), _>`.
- `shell/src/menu.rs`: the menu's entries, cursor, navigation and draw. Pure
  enough to unit-test with no window, as `pack_select`'s logic is.
- `shell/src/views.rs`: `ViewKind::Menu` added; `wants_exit_hint` returns false
  for it, as C does at `views.c:372` -- the menu draws its own hints.
- `shell/src/startup.rs`: its picker is replaced by the shared one.
- `shell/src/main.rs`: the bare-ESC opener, the menu's action dispatch, and the
  save/load call sites.

## Data flow

```
bare ESC on the overworld
   -> views.set(ViewKind::Menu)
   -> menu.update(keys) -> MenuAction::{Save, Load, Quit, Close}
        Save -> SlotPicker(save mode) -> slot
                  occupied? Prompt::yes_no -> save_write
        Load -> SlotPicker(load mode) -> slot
                  load_slot(..) -> Ok  -> swap game/map/fog
                                 Err -> report, keep playing
        Quit -> StatusMode::FastQuit (the existing confirm)
```

## Failure modes

- **Save fails** (no writable directory, disk error): report, stay in the
  menu, keep playing. `save_write` already returns a `Result`.
- **Load fails** at any of the four steps: report, keep the live session
  exactly as it was. This is the behaviour worth testing.
- **A foreign-pack save** is refused by `save_read`'s `pack_id` guard, which
  already exists. The picker still lists it, because `save_read_header`
  deliberately omits that guard -- `AGENTS.md` records that C lists a foreign
  save and refuses it at load, and this keeps that.
- **An empty slot chosen for Load**: no-op with a message, not an error.
- **No writable save directory at all**: `SavePaths::slot` returns `None`.
  Save and Load both report and the menu stays usable.

## Testing

Unit tests, no window, matching how the view and `pack_select` tests already
work:

- Menu navigation: wrap in both directions, each entry returns its action, ESC
  closes without acting.
- The extracted picker still behaves as `startup.rs`'s did -- move the existing
  startup picker tests onto the shared widget rather than writing new ones, so
  the extraction is proven not to have changed anything.
- The picker with and without the trailing extra row, since that is the one
  thing that differs between the two callers.
- **A failed load leaves the game unchanged.** Point `load_slot` at a missing
  or corrupt file and assert the live `Game` is untouched. This is the test
  that earns the transactional design.
- ESC routing: with a view open ESC dismisses; with nothing open it opens the
  menu; in combat it does neither.

## Non-goals

- **A Views submenu** (Decision 3).
- **New Game** (Decision 3). A restart is the startup wizard's job.
- **Mid-combat access** (Decision 2). Reaching it means serializing combat.
- **Autosave**, save naming, save deletion, or more than the 10 slots
  `SAVE_SLOT_COUNT` already defines.
- **Porting `shell_menu.c` faithfully.** It is dead code with a submenu stack
  (`MENU_STACK_MAX 4`) that three entries do not need.
- **Changing `Q` or `Ctrl+Q`.** Both already work and are ported; the menu is
  an addition, not a replacement.

## Risks

- **The extraction touches working startup code.** `startup.rs`'s picker is
  live and tested; moving it risks regressing the one flow every run passes
  through. Mitigation: move the existing tests with it and change no behaviour
  in the same step as the move.
- **ESC is overloaded.** Five contexts already use it. The bare-overworld case
  must be checked last, after every other ESC consumer, or the menu will
  swallow a dismissal. The ordering is the thing to review.
- **Save/load mid-session is newly reachable**, so bugs in `save_write` /
  `save_read` that only startup exercised will now surface during play. That is
  a feature of the change, not a regression, but expect it.

## Work order

1. Extract the slot picker into its own module, behaviour unchanged, with the
   startup tests moved onto it. Startup keeps working.
2. Extract `load_slot`, the shared four-step recipe, and point startup at it.
3. `menu.rs`: entries, navigation, draw, tests. Not yet reachable.
4. `ViewKind::Menu` and the bare-ESC opener, with the routing rules.
5. Wire Save, Load and Quit, including the overwrite confirm and the
   transactional swap.

Steps 1 and 2 are pure refactors and must leave the suite green with no
behaviour change; that is what makes the rest safe.
