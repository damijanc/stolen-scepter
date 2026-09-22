# In-Game Menu Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Game Menu reachable during play, offering Save, Load and Quit, so
save-and-continue and mid-session load become possible at all.

**Architecture:** The 10-slot save picker already in `startup.rs` moves to its
own module and both callers share it. The four-step load recipe moves with it.
The menu itself is a `ViewKind`, so it inherits dismissal, the paused
background and click handling from the existing view stack. Everything is a
shell change; the engine is untouched.

**Tech Stack:** Rust, the existing `engine` and `shell` crates. No new
dependencies.

**READ FIRST:** `specs/ingame-menu-design.md`. It carries the decisions and,
critically, **why this is not a port** -- C's `VIEW_MENU` is unreachable dead
code, so there is no C behaviour to match and its shape is not an authority.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a tested task, report it, ask.
- **Commit messages are a single subject line.** No body, no bullets, no
  trailers. Example: `feat: share the save slot picker`.
- **Never add a dependency to `engine/Cargo.toml`.**
- **Never commit `reference/`, `game-assets/`, or build output.**
- Preserve unrelated working-tree changes.
- Run `cargo fmt --all --check` before every commit.

## Verified before this plan was written

- `startup.rs:599` `draw_save_picker` reads only `self.cursor` and
  `self.slots`. `update_save_picker` mutates `self.cursor`, `self.slot` and
  `self.screen`, and returns `Option<StartupChoice>`. So the widget's own state
  is exactly `cursor` + `slots`; everything else belongs to the caller.
- `savepath::scan_slots(&SavePaths, pack_id) -> SlotSet`, with
  `SAVE_SLOT_COUNT = 10` and `SlotSet.headers: [Option<SaveHeader>; 10]`.
- `save_read(path, &mut Game, &Resources, &Map, &mut Fog) -> Result<_, SaveError>`
  and `save_write(path, &Game, &Resources, &Map, &Fog) -> Result<_, _>`.
  **Neither carries combat state** -- hence Decision 2 in the design.
- The load recipe at `main.rs:712-736` is four steps: `save_read` with a dummy
  `Map::new(1, 1)`, read `game.position.zone`,
  `Map::load_zone_with_placements`, then `map.apply_consumed(&game, &zone)`.
- `fast_quit` is a `bool` in `main.rs:768`; its branch at `main.rs:1141` is
  modal and **consumes ESC as "no"**. `StatusMode::FastQuit` draws the prompt.
- `input.rs:244` special-cases `Q` / `Ctrl+Q` AFTER the `BINDINGS` loop, with a
  comment explaining the ordering. ESC follows that shape.
- `Views::{set, dismiss, active}` at `views.rs:302-317`;
  `wants_exit_hint` at `:508`. `Prompt::yes_no(header, body)` at
  `prompt.rs:81`; `Dialog::open(header, body)` at `dialog.rs:47`.

## Out of scope -- do not build these

- **A Views submenu.** All six of C's entries already have overworld keys
  (`A`, `U`, `V`, `I`, `P`, `M`).
- **New Game.** C's `menu_new` keeps gold, army, villains and days while
  teleporting the hero home -- it is not a new game. A restart is the startup
  wizard.
- **Mid-combat access.** The save format has no combat state; see the design.
- **Autosave, save naming, save deletion, more slots.**
- **Changing `Q` or `Ctrl+Q`.** Both work and are ported.

---

### Task 1: Extract the save slot picker

**Files:**
- Create: `shell/src/slot_picker.rs`
- Modify: `shell/src/startup.rs`
- Modify: `shell/src/main.rs` (add `pub mod slot_picker;`)
- Test: `shell/src/slot_picker.rs`

**Interfaces:**
- Consumes: `savepath::{SlotSet, SAVE_SLOT_COUNT}`, `input::{Key, KeySource}`,
  `draw::DrawList`, `layout::Layout`, `palette::{Pal, Palette}`, `banner`.
- Produces: `slot_picker::SlotPicker` with `new(SlotSet, extra_row: bool)`,
  `update(&mut self, &dyn KeySource) -> Option<SlotChoice>`,
  `draw(&self, ...)`, and `slots(&self) -> &SlotSet`;
  `slot_picker::SlotChoice { Slot(usize), Extra, Cancel }`.

**This is a pure refactor. Behaviour must not change.** The acceptance check is
that the existing startup tests pass unmodified except for where they now call
the shared widget.

- [ ] **Step 1: Move the existing tests first**

Find the tests in `shell/src/startup.rs` that exercise the save picker:

```bash
grep -n "save_picker\|SavePicker" shell/src/startup.rs | grep -i test
```

Move them into `shell/src/slot_picker.rs`'s `mod tests`, rewritten to drive
`SlotPicker` directly rather than `Startup`. Add these, which pin the split
between the two callers:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Keys;
    use openbounty_engine::savegame::SaveHeader;

    fn slots_with(filled: &[usize]) -> SlotSet {
        let mut set = SlotSet::default();
        for &slot in filled {
            let mut header = SaveHeader::default();
            header.exists = true;
            header.name.copy_from_str("Sir Rust");
            header.rank_title.copy_from_str("General");
            header.days_left = 400;
            set.headers[slot] = Some(header);
            set.existing += 1;
        }
        set
    }

    #[test]
    fn the_cursor_wraps_over_the_slots_alone_without_an_extra_row() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(picker.cursor, 0);
        picker.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(picker.cursor, SAVE_SLOT_COUNT - 1, "wraps to the last slot");
        picker.update(&Keys::pressed(&[Key::Down]));
        assert_eq!(picker.cursor, 0);
    }

    /// Startup adds a trailing "New game" row, so its cursor runs one further.
    #[test]
    fn an_extra_row_adds_one_position_past_the_slots() {
        let mut picker = SlotPicker::new(slots_with(&[]), true);
        picker.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(picker.cursor, SAVE_SLOT_COUNT, "the extra row is last");
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Enter])),
            Some(SlotChoice::Extra)
        );
    }

    #[test]
    fn confirming_a_slot_reports_that_slot_whether_or_not_it_is_filled() {
        let mut picker = SlotPicker::new(slots_with(&[3]), false);
        picker.cursor = 3;
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Enter])),
            Some(SlotChoice::Slot(3))
        );
        // An empty slot reports too; the CALLER decides what that means.
        let mut picker = SlotPicker::new(slots_with(&[3]), false);
        picker.cursor = 5;
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Space])),
            Some(SlotChoice::Slot(5))
        );
    }

    #[test]
    fn escape_cancels() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(
            picker.update(&Keys::pressed(&[Key::Escape])),
            Some(SlotChoice::Cancel)
        );
    }

    #[test]
    fn nothing_pressed_reports_nothing() {
        let mut picker = SlotPicker::new(slots_with(&[]), false);
        assert_eq!(picker.update(&Keys::pressed(&[])), None);
    }
}
```

If `SaveHeader` does not implement `Default`, build it field by field rather
than deriving `Default` on an engine type -- the engine's shape is not this
task's to change.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell slot_picker:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find struct 'SlotPicker'`.

- [ ] **Step 3: Write the widget**

Create `shell/src/slot_picker.rs`. Move the bodies of `draw_save_picker` and
the navigation half of `update_save_picker` across **verbatim** -- do not
improve the layout arithmetic or the key handling while moving it. The only
change is that `self.cursor` and `self.slots` now belong to `SlotPicker`, and
the row count comes from `extra_row`.

```rust
//! The save slot picker, shared by the startup wizard and the in-game menu.
//!
//! Extracted from `startup.rs` so both callers show the same widget. It is
//! deliberately dumb: it owns the cursor and the scanned headers and reports
//! which row was confirmed. What a row MEANS is the caller's business --
//! startup treats an empty slot as "start a new game here", the menu treats it
//! as an overwrite target or as nothing to load.

use crate::savepath::{SlotSet, SAVE_SLOT_COUNT};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotChoice {
    /// A slot row was confirmed. It may be empty; the caller decides.
    Slot(usize),
    /// The trailing extra row was confirmed (startup's "New game").
    Extra,
    /// ESC.
    Cancel,
}

pub struct SlotPicker {
    pub cursor: usize,
    slots: SlotSet,
    extra_row: bool,
}

impl SlotPicker {
    pub fn new(slots: SlotSet, extra_row: bool) -> Self {
        Self { cursor: 0, slots, extra_row }
    }

    pub fn slots(&self) -> &SlotSet {
        &self.slots
    }

    fn row_count(&self) -> usize {
        SAVE_SLOT_COUNT + usize::from(self.extra_row)
    }

    // update(): the navigation half of startup's update_save_picker, verbatim,
    // returning a SlotChoice instead of driving Startup's state machine.

    // draw(): startup's draw_save_picker, verbatim, with the trailing extra
    // row drawn only when `extra_row` is set.
}
```

Fill `update` and `draw` from the originals. `draw` keeps its
`banner::text(..)` lookups so the pack can still reword the title, the empty
label and the extra row; pass the extra row's label in rather than hard-coding
startup's.

- [ ] **Step 4: Point startup at the shared widget**

In `startup.rs`, replace the `cursor`/`slots` fields' picker use with a
`SlotPicker`, and map the outcome onto the existing behaviour exactly:

| `SlotChoice` | Startup does (unchanged) |
|---|---|
| `Cancel` | back to `Screen::ClassSelect` |
| `Extra` | `slot` = first empty, `Screen::CreateGame` |
| `Slot(n)`, header present | `StartupAction::Load` on slot `n` |
| `Slot(n)`, header absent | `slot` = `n`, `Screen::CreateGame` |

Delete `draw_save_picker` and `update_save_picker` once nothing calls them.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell`
Expected: PASS. Every pre-existing startup test must still pass; if one fails,
the move changed behaviour and that is a bug in the move, not in the test.

- [ ] **Step 6: Verify the wizard by hand**

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

Press `L` on class select. Confirm the picker looks and behaves exactly as
before: 10 rows plus "New game", wrapping, ESC back, and that loading a save
still works.

- [ ] **Step 7: Check formatting and the suite, then ask and commit**

Run: `cargo fmt --all --check && cargo test --workspace`

```bash
git add shell/src/slot_picker.rs shell/src/startup.rs shell/src/main.rs
git commit -m "feat: share the save slot picker"
```

---

### Task 2: Extract the load recipe

**Files:**
- Modify: `shell/src/slot_picker.rs`
- Modify: `shell/src/main.rs`
- Test: `shell/src/slot_picker.rs`

**Interfaces:**
- Produces: `slot_picker::load_slot(&SavePaths, &Resources, pack_id: &str,
  slot: usize) -> Result<(Game, Map, Fog), LoadError>` and
  `slot_picker::LoadError { NoPath, Read(SaveError), Zone(String) }`.

The four steps currently inline at `main.rs:712-736` become one function with
two callers. `load_slot` builds everything into fresh values and returns them,
which is what makes the menu's load transactional -- the caller swaps only on
`Ok`.

- [ ] **Step 1: Write the failing test**

Add to `slot_picker.rs`'s `mod tests`:

```rust
    /// The transactional guarantee: a load that fails returns Err and produces
    /// no half-built state for a caller to accidentally install.
    #[test]
    fn loading_a_missing_slot_fails_without_producing_state() {
        let paths = SavePaths::with_override(std::env::temp_dir().join("ob_no_such_dir"));
        let resources = test_resources();
        assert!(matches!(
            load_slot(&paths, &resources, "kb", 0),
            Err(LoadError::NoPath) | Err(LoadError::Read(_))
        ));
    }

    /// A corrupt save is a Read error, not a panic.
    #[test]
    fn loading_a_corrupt_slot_reports_a_read_error() {
        let dir = std::env::temp_dir().join(format!("ob_corrupt_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        // `with_override` takes `impl Into<PathBuf>`; `&PathBuf` does not
        // satisfy that, so pass the `&Path` (or clone).
        let paths = SavePaths::with_override(dir.as_path());
        let path = paths.slot("kb", 0).expect("slot path");
        std::fs::write(&path, b"{ not valid json").expect("write");

        let resources = test_resources();
        assert!(matches!(load_slot(&paths, &resources, "kb", 0), Err(LoadError::Read(_))));
    }
```

Add a `test_resources()` helper returning a minimal `Resources` via
`Resources::from_json_str`, modelled on the fixture in `views.rs`'s tests.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p openbounty-shell slot_picker:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find function 'load_slot'`.

- [ ] **Step 3: Write it**

```rust
/// Why a load could not complete. Every variant leaves the caller's live
/// session untouched, because nothing is returned to install.
#[derive(Debug)]
pub enum LoadError {
    /// No usable save directory for this slot.
    NoPath,
    Read(openbounty_engine::savegame::SaveError),
    /// The save named a zone the pack could not load.
    Zone(String),
}

/// Load a slot into fresh state, porting the four coupled steps `main.rs`
/// performs at startup.
///
/// Returns everything or nothing. The in-game menu relies on that: a failed
/// load must not disturb a live session, so there is deliberately no variant
/// that hands back a partially built game.
pub fn load_slot(
    paths: &SavePaths,
    resources: &Resources,
    pack_id: &str,
    slot: usize,
) -> Result<(Game, Map, Fog), LoadError> {
    let path = paths.slot(pack_id, slot).ok_or(LoadError::NoPath)?;
    let mut game = Game::default();
    let mut fog = Fog::new();
    save_read(&path, &mut game, resources, &Map::new(1, 1), &mut fog)
        .map_err(LoadError::Read)?;

    let zone = game.position.zone.as_str().to_owned();
    let mut map = Map::load_zone_with_placements(resources, &zone, &game)
        .map_err(|err| LoadError::Zone(format!("{zone}: {err}")))?;
    map.apply_consumed(&game, &zone);
    Ok((game, map, fog))
}
```

- [ ] **Step 4: Point startup at it**

Replace `main.rs:712-736`'s inline recipe with `load_slot`. Keep startup's
existing forgiving behaviour -- *"C pre-initializes a default game so a failed
load still leaves something playable"* -- by handling the `Err` the way it does
today: report, then fall through to a seeded default game rather than exiting.
That difference in ERROR HANDLING is the point; the recipe itself is shared.

- [ ] **Step 5: Run the tests, verify by hand, commit**

Run: `cargo fmt --all --check && cargo test --workspace`

Then load a save through the startup wizard and confirm the hero, zone, fog and
gold are all as before.

```bash
git add shell/src/slot_picker.rs shell/src/main.rs
git commit -m "feat: share the save load recipe"
```

---

### Task 3: The menu itself

**Files:**
- Create: `shell/src/menu.rs`
- Modify: `shell/src/main.rs` (add `pub mod menu;`)
- Test: `shell/src/menu.rs`

**Interfaces:**
- Produces: `menu::Menu` with `new()`, `update(&mut self, &dyn KeySource) ->
  Option<MenuAction>`, `draw(&self, ...)`; `menu::MenuAction { Save, Load,
  Quit, Close }`.

Not reachable yet -- Task 4 wires the opener. Keep it pure and tested first.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Keys;

    #[test]
    fn the_cursor_wraps_in_both_directions() {
        let mut menu = Menu::new();
        assert_eq!(menu.cursor, 0);
        menu.update(&Keys::pressed(&[Key::Up]));
        assert_eq!(menu.cursor, ENTRY_COUNT - 1);
        menu.update(&Keys::pressed(&[Key::Down]));
        assert_eq!(menu.cursor, 0);
    }

    #[test]
    fn each_entry_confirms_to_its_own_action() {
        for (index, expected) in [
            (0, MenuAction::Save),
            (1, MenuAction::Load),
            (2, MenuAction::Quit),
        ] {
            let mut menu = Menu::new();
            menu.cursor = index;
            assert_eq!(
                menu.update(&Keys::pressed(&[Key::Enter])),
                Some(expected)
            );
        }
    }

    #[test]
    fn escape_closes_without_acting() {
        let mut menu = Menu::new();
        menu.cursor = 2;
        assert_eq!(
            menu.update(&Keys::pressed(&[Key::Escape])),
            Some(MenuAction::Close)
        );
    }

    /// The digit shortcuts match the drawn numbering, 1-based. `input.rs`
    /// names these `Key::Digit0..Digit9` and exposes them as `DIGITS`.
    #[test]
    fn digits_select_and_confirm_their_row() {
        let mut menu = Menu::new();
        assert_eq!(
            menu.update(&Keys::pressed(&[Key::Digit2])),
            Some(MenuAction::Load)
        );
    }

    #[test]
    fn nothing_pressed_reports_nothing() {
        let mut menu = Menu::new();
        assert_eq!(menu.update(&Keys::pressed(&[])), None);
    }
}
```

`Menu::update` should reuse `input::DIGITS` for the digit row rather than
naming variants inline, the way the existing prompt code does.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell menu:: 2>&1 | head -20`
Expected: FAIL to compile, `cannot find struct 'Menu'`.

- [ ] **Step 3: Write it**

```rust
//! The in-game Game Menu.
//!
//! NOT a port. C's `VIEW_MENU` exists but is unreachable -- nothing calls
//! `views_set(VIEW_MENU)` -- so there is no C behaviour to match. See
//! `specs/ingame-menu-design.md` for what was taken from C's shape and what
//! was deliberately dropped: the Views submenu duplicates existing single
//! keys, and C's "New Game" keeps gold, army and days while only teleporting
//! the hero home, which its label does not admit.

use crate::input::{Key, KeySource};

pub const ENTRY_COUNT: usize = 3;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    Save,
    Load,
    Quit,
    /// ESC: close without acting.
    Close,
}

pub struct Menu {
    pub cursor: usize,
}
```

Give it `new`, `update` (up/down with wrap, digits 1..3, Enter/Space confirm,
ESC closes) and `draw` (a rounded panel with a title and three rows, the
cursor row in yellow). Model the panel on `draw_options` in `views.rs`, which
is the closest existing shape.

- [ ] **Step 4: Run the tests to verify they pass, then commit**

Run: `cargo test -p openbounty-shell menu::`
Expected: PASS, 5 tests.

Run: `cargo fmt --all --check && cargo test --workspace`

```bash
git add shell/src/menu.rs shell/src/main.rs
git commit -m "feat: add the in-game menu view"
```

---

### Task 4: Open it with ESC

**Files:**
- Modify: `shell/src/views.rs`
- Modify: `shell/src/input.rs`
- Modify: `shell/src/main.rs`
- Test: `shell/src/input.rs`, `shell/src/views.rs`

**Interfaces:**
- Produces: `ViewKind::Menu`, `Action::GameMenu`, and the routing that opens it.

**The ordering is the whole task.** ESC already has five consumers and the new
one must be checked LAST. In `main.rs` the chain is: views open -> prompt or
dialog up -> `fast_quit` (modal, and it consumes ESC as "no") -> the general
input branch. The menu opener belongs in the last one.

- [ ] **Step 1: Write the failing tests**

In `shell/src/input.rs`'s tests:

```rust
    #[test]
    fn escape_asks_for_the_game_menu() {
        assert_eq!(poll_keys(&[Key::Escape]).action, Action::GameMenu);
    }

    /// ESC is not a C binding, so it must not disturb the parity-ordered
    /// letter table: a letter pressed in the same frame still wins.
    #[test]
    fn a_letter_still_beats_escape_in_the_same_frame() {
        assert_eq!(poll_keys(&[Key::Escape, Key::A]).action, Action::ViewArmy);
    }
```

In `shell/src/views.rs`'s tests:

```rust
    /// C returns false for VIEW_MENU at views.c:372 -- the menu draws its own
    /// hints, so the generic "Press ESC to exit" line would be noise.
    #[test]
    fn the_menu_view_does_not_want_the_exit_hint() {
        let game = knight_game();
        let mut views = Views::new();
        views.set(ViewKind::Menu);
        assert!(!views.wants_exit_hint(&game));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell escape_asks 2>&1 | head -20`
Expected: FAIL, `no variant named 'GameMenu'`.

- [ ] **Step 3: Add the action and the view kind**

In `input.rs`, add `GameMenu` to `Action`, and special-case ESC **after** the
`BINDINGS` loop and after the `Q` block, mirroring how `Q` is handled:

```rust
    // ESC last of all. It is NOT a C binding -- C's ESC only ever dismisses --
    // so it stays out of the parity-ordered BINDINGS table above, and any
    // letter pressed in the same frame wins. main.rs only reaches this when
    // no view, prompt, dialog or quit confirm is up.
    if state.action == Action::None && keys.pressed(Key::Escape) {
        state.action = Action::GameMenu;
    }
```

In `views.rs`, add `Menu` to `ViewKind` and return `false` from
`wants_exit_hint` for it.

- [ ] **Step 4: Wire the opener and the draw**

In `main.rs`'s general input branch:

```rust
                Action::GameMenu => {
                    menu = Menu::new();
                    views.set(ViewKind::Menu);
                }
```

Drive `menu.update(..)` where the other views are updated, and draw it where
the other views draw. Because it is a `ViewKind`, the combat branch never
reaches it -- combat renders its own path and does not run this chain, which is
what keeps Decision 2 true without a special case.

- [ ] **Step 5: Run the tests and verify the routing by hand**

Run: `cargo fmt --all --check && cargo test --workspace`

Then run the game and confirm, in order:

1. Bare ESC on the overworld opens the menu; ESC again closes it.
2. ESC with the Army view open still dismisses the view and does NOT open the
   menu.
3. ESC during a yes/no prompt still cancels the prompt.
4. Ctrl+Q then ESC still cancels the quit confirm rather than opening the menu.
5. ESC during combat does what it did before, and never opens the menu.

Item 5 is the one to be most careful about; it is the design's hard constraint.

- [ ] **Step 6: Commit**

```bash
git add shell/src/input.rs shell/src/views.rs shell/src/main.rs
git commit -m "feat: open the game menu with escape"
```

---

### Task 5: Wire Save, Load and Quit

**Files:**
- Modify: `shell/src/main.rs`

**Interfaces:**
- Consumes: `SlotPicker`, `SlotChoice`, `load_slot`, `Menu`, `MenuAction`,
  `Prompt::yes_no`, `Dialog::open`, `save_write`, `StatusMode::FastQuit`.

- [ ] **Step 1: Wire Quit**

`MenuAction::Quit` dismisses the menu and sets `fast_quit = true`, reusing the
existing modal confirm and its status-bar prompt. Add no second confirmation.

- [ ] **Step 2: Wire Save**

`MenuAction::Save` opens a `SlotPicker::new(scan_slots(&save_paths,
resources.pack_id.as_str()), false)`.

On `SlotChoice::Slot(n)`:

- If `picker.slots().headers[n].is_some()`, confirm first with
  `Prompt::yes_no`, and only write on `PromptResult::Yes`.
- Otherwise write immediately.

The write is the existing call:

```rust
    match save_paths.slot(resources.pack_id.as_str(), n) {
        Some(path) => match save_write(&path, &game, &resources, &map, &fog) {
            Ok(()) => dialog.open(saved_header, saved_body),
            Err(err) => dialog.open(failed_header, &format!("{err:?}")),
        },
        None => dialog.open(failed_header, no_save_dir_body),
    }
```

Take the four strings through `banner::text` with compiled-in defaults, as
every other shell message does.

On `SlotChoice::Cancel`, return to the menu rather than closing it outright.

- [ ] **Step 3: Wire Load, transactionally**

`MenuAction::Load` opens the same picker. On `SlotChoice::Slot(n)`:

```rust
    if picker.slots().headers[n].is_none() {
        dialog.open(load_failed_header, empty_slot_body);
    } else {
        match load_slot(&save_paths, &resources, resources.pack_id.as_str(), n) {
            Ok((new_game, new_map, new_fog)) => {
                // Swap only on success: a failed load must not disturb play.
                game = new_game;
                map = new_map;
                fog = new_fog;
                views.dismiss();
            }
            Err(err) => dialog.open(load_failed_header, &format!("{err:?}")),
        }
    }
```

After a successful swap, reset anything derived from the old session that would
otherwise be stale: the hero animator, any pending week phase, the foe
collision state and the active dialog. Grep for what `main.rs` holds outside
`game`/`map`/`fog` and reset each deliberately -- a stale `pending_week` after
a load would fire someone else's week-end screen.

- [ ] **Step 4: Verify by hand**

1. Save into an empty slot, quit, relaunch, load it from startup. State matches.
2. Save over an occupied slot: the confirm appears, No leaves it untouched, Yes
   overwrites.
3. Load mid-session: hero, zone, gold, army and fog all change to the save.
4. Load a slot, then walk 40 steps and confirm the day counter behaves -- this
   is where a stale animator or week phase would show.
5. Point a slot at a corrupt file and load it: a message appears and the live
   session keeps playing unchanged.
6. Save, then Load the same slot, and confirm nothing shifts.

- [ ] **Step 5: Update the docs and commit**

`specs/play-verification.md`: the §13 `| In-game menu | GAP |` row closes.
Reword it rather than deleting if anything remains, and move the "Last
refreshed" line. Add ESC to the Controls reference if the pack's `keybinds`
block drives it -- check whether `strings.keybinds` needs a row so the options
screen lists the new key.

```bash
git add shell/src/main.rs specs/play-verification.md
git commit -m "feat: save load and quit from the game menu"
```

---

## Open item for the reviewer

**Task 5, Step 3's "reset what is derived" list is the soft spot.** Everything
else in this plan is mechanical, but a mid-session load replaces three values
that a dozen other pieces of shell state were derived from, and nothing in the
type system says which. The listed suspects -- animator, `pending_week`, foe
collision, open dialog -- came from reading `main.rs`, not from a compiler
error, so treat that list as a starting point and walk `main`'s locals before
declaring it done.

If in doubt, prefer resetting too much: a needlessly reset animator costs one
frame, a stale week phase costs the player a day.
