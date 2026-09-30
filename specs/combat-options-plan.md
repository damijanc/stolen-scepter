# Combat Options and Controls Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the player open the Options view with `O` and the Controls view
with `C` during a fight, swap between them in place, and cycle Controls rows
with `1`..`9` while the fight is paused, porting `reference/src/combat_loop.c`'s
openers and its paused-view branch. This closes the `Combat O` GAP in
`specs/play-verification.md` §9 and §13.

**Architecture:** Two shell-only changes. `CombatLoop::step`
(`shell/src/combat.rs`) learns the two missing openers and reports them through
the `CombatStatus::OpenView` it already returns for Army and Character. A new
free function, `combat::combat_view_input`, ports C's paused-view branch so
`main.rs` stops dismissing on any key and calls it instead. `main.rs` also
starts drawing the open view over the battlefield, which it does not do today
(see "Why this slice is bigger than one key"). No engine change.

**Tech Stack:** Rust, the workspace's existing `shell` crate. No new
dependencies. `cargo test -p openbounty-shell`.

## The C rules this ports

Every rule below is executable C, not a comment. Cite these in test names or
doc comments where the plan says so.

1. **`O` is a top-level opener and works on the AI's turn.**
   `combat_loop.c:549-553`: inside
   `if (c.cast_phase == COMBAT_CAST_NONE && !c.picker_active)`, `KEY_O` sets
   `VIEW_OPTIONS` and `continue`s. The block runs before the `player_turn`
   dispatch (`:538-540`, `:612`), so it does not depend on whose turn it is.
   The comment at `:542-544` says so too, and the code agrees.
2. **Opener order is `O`, `A`, `V`** (`combat_loop.c:550`, `:554`, `:558`).
   Rust has `A`, `V` (`shell/src/combat.rs:480-487`). `O` goes first.
3. **`C` is a player-turn key, not a top-level opener.** `combat_loop.c:306-312`
   sits at the END of `combat_player_action_full` (`:220`), which is called only
   from the `else if (player_turn)` arm (`:612-613`). That arm is behind the
   cast-phase branch (`:568`) and the shoot/fly picker branch (`:575`). So `C`
   does nothing on the AI's turn, nothing while a spell is being picked (it is
   spell key C, Fireball), and nothing while a picker is armed (it is a picker
   confirm key -- Rust's `pick_step`, `shell/src/combat.rs:828`). It returns 0,
   so opening Controls spends no action.
4. **Guard order inside the player action:** direction, Space/W, G, S, F, U,
   then C (`combat_loop.c:224-312`). Rust's `player_action` has direction,
   Space/W, S, F, U, G (`shell/src/combat.rs:604-686`). `C` goes last. Do not
   reorder the existing branches; see Out of scope.
5. **While a view is open, combat is paused and ESC is the ONLY dismiss key.**
   `combat_loop.c:489-493` closes the view on ESC and `continue`s. The paused
   branch at `:502-519` then reads only `C`, `O` and digits and `continue`s
   unconditionally. There is no `ui_any_key_pressed` anywhere in it. So Army and
   Character, which Rust closes on any key (`shell/src/main.rs:847-852`), close
   only on ESC in C combat.
6. **The swap:** Options + `C` sets Controls (`combat_loop.c:504-505`); Controls
   + `O` sets Options (`:506-508`). `C` on Controls does NOT close it here --
   unlike the overworld, where `main.c:1132-1135` closes Controls on `C`.
7. **Digits index the CATALOG row, not the visible row.**
   `combat_loop.c:511-513` loops `i` over 0..9 and calls
   `views_controls_advance(g, i)` directly. The overworld branch maps the digit
   through `vis_map` first and moves the cursor (`main.c:1138-1142`). The combat
   branch does neither, has no Up/Down/Enter/Space handling, and never reaches
   the Scale row (`views_controls_advance` bounds `row` by `controls.count`,
   `views.c:945`). With the stock pack's hidden `cga` row at catalog index 4
   (`reference/assets/kings-bounty/game.json`, `controls.settings`), digit `5`
   in combat cycles the hidden CGA value and digit `6` cycles Music, which the
   panel labels `5`. That is C's behaviour; port it and name the test after it.
8. **The open view draws over the still-visible field.** `combat_present`,
   `combat_loop.c:321-327`: `if (views_active() != VIEW_NONE) overlay_draw(g,
   NULL, NULL, sprites);`, after `combat_render_frame` and before the prompt and
   dialog.

## Why this slice is bigger than one key

Investigating the gap turned up two existing problems the `O` key would expose
immediately. Both are in scope because the feature does not work without them.

- **No view draws during combat.** `main.rs`'s render pass
  (`shell/src/main.rs:1473-1490`) draws `Chrome` and `combat_render` and
  nothing else; `views.draw` is only called in the adventure `else` branch
  (`:1525`). So `A` and `V` today pause the fight behind an invisible view, and
  `specs/play-verification.md` §9's "over the still-visible battlefield" is not
  what a player sees. Adding Options and Controls without fixing this would add
  two more invisible views.
- **The combat view branch dismisses on any key** (`main.rs:850`). That would
  make the `C` swap impossible: pressing `C` on Options would close it.

## Decided: clicks on a view over the battlefield

C has no pointer; this is port-side behaviour. Once views draw in combat they
publish `HitList` regions, because `Views::draw` pushes them
(`shell/src/views.rs:1246-1262` for the read-only views, `:1586-1590` and
`:1630-1634` for the Controls rows). The combat branch of `main.rs` ignores
`clicked` today, so those regions would be dead.

**Decision: honour them with the meaning they already have on the overworld.**
A click on a read-only view (`HitTarget::ViewDismiss`) closes it; a click on a
Controls row (`HitTarget::ControlsRow(k)`) selects and cycles the row drawn
under the pointer (`controls_visible[k]`, or the Scale row on a modern pack),
exactly as `main.rs:967-976` does. Rationale:

- `specs/mouse-support-design.md` makes the combat FIELD a non-goal ("It sits on
  the turn machinery and the SYN-tick AI pacing"). A view over the field is a
  view; a click on it touches neither the turn machinery nor the AI.
- Its modal-precedence rule is "regions come from whatever actually drew". A
  drawn region that does nothing is the "misbehaves only under the pointer"
  risk that doc warns about.
- A click advances the row it is ON, not the combat digit's catalog row. The
  pointer names a drawn thing; mapping it through rule 7's quirk would make a
  click on "Music" cycle CGA.

The `Mouse: overworld & combat | BY DESIGN` row in §13 stays; its wording is
narrowed in Task 2.

## Global Constraints

Copied from `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** The user wants approval before every commit.
  Complete a tested task, report it, and ask. Do not chain commits.
- **Commit messages are a single subject line.** No body paragraphs, no bullet
  lists, no `Co-Authored-By` or other attribution trailers. Example:
  `feat: open options and controls from combat`.
- **Never commit `reference/`, `game-assets/`, or build output.** They are
  gitignored; keep it that way.
- `shell/src/backend.rs` is the ONLY module that may know macroquad exists.
  `combat.rs` must not import it.
- Do not touch `engine/`. `engine/tests/render_free.rs` must still pass.
- Transliterate `combat_loop.c` first; do not improve it. Where a C comment and
  the C code disagree, port the code and say so in the test name.
- Preserve unrelated working-tree changes. Do not reset, checkout, or clean.
  At the time of writing several other `specs/*-plan.md` files and
  `specs/remaining-work.md` are untracked, written by parallel work; leave
  them out of every `git add`. Every `git add` below names its files
  explicitly for that reason -- never `git add -A` or `git add specs/`.
- Use `apply_patch` for hand edits.
- Run `cargo fmt --all --check` before every commit.

## File Structure

- **Modify `shell/src/combat.rs`**: one new `CombatLoop` field, the `O` opener,
  the `C` branch in `player_action`, the new `combat_view_input` function, and
  their tests in the existing `#[cfg(test)] mod tests`. It already owns the
  port of `RunCombat`'s per-frame dispatch; the paused-view branch is the same
  loop's next `if`, so it belongs beside it rather than in `views.rs`.
- **Modify `shell/src/main.rs`**: the combat input branch calls
  `combat_view_input`; the combat render branch calls `views.draw`.
- **Modify `specs/play-verification.md`** (§9, §13, "Last refreshed") and
  **`AGENTS.md`** (test counts).

## Out of scope -- do not build these

- **Reordering `player_action`'s existing branches.** Rust checks `G` after `U`
  where C checks it before `S` (rule 4). Every branch keys off a distinct key
  and returns, so the order is only observable with two keys pressed in one
  frame. Not this slice; add `C` last and leave the rest alone.
- **An Options/Controls swap on the overworld.** C has none: overworld Options
  falls to the generic any-key dismiss (`main.c:1238-1262`) and overworld
  Controls closes on `C` (`main.c:1132-1135`). Rust already matches both
  (`shell/src/main.rs:990`, `:1138`).
- **Up/Down/Enter/Space on Controls during combat.** C's combat branch reads
  none of them (rule 7). Do not reuse the overworld handler.
- **Gamepad cancel.** The overworld Controls branch accepts
  `gamepad_pressed_cancel()`; the combat branch does not, and this shell has no
  gamepad.
- Audio, the in-game menu, the pack picker. Separate slices; see
  `specs/play-verification.md` §13.

---

### Task 1: The `O` and `C` openers

**Files:**
- Modify: `shell/src/combat.rs:286-308` (the `CombatLoop` struct),
  `:335-351` (`CombatLoop::new`), `:478-487` (the openers), `:508-517` (after the
  banner update), `:670-686` (end of `player_action`)
- Test: `shell/src/combat.rs` (the existing `mod tests`, and its `loop_fixture`
  struct literal at `:1107-1123`)

**Interfaces:**
- Consumes: `CombatLoop::step(&mut self, &dyn KeySource, &mut Game, &Resources,
  f64, &mut Prompt) -> CombatStatus`; `CombatStatus::OpenView(ViewKind)`;
  `ViewKind::Options`, `ViewKind::Controls`; `Keys::pressed(&[Key])`;
  `Prompt::default()`; `COMBAT_SIDE_AI`, `COMBAT_SIDE_PLAYER`; test helper
  `loop_fixture() -> (CombatLoop, Resources, Game)`.
- Produces: private field `CombatLoop::requested_view: Option<ViewKind>`.
  No new public API.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `shell/src/combat.rs`, after
`a_view_key_opens_a_view_over_the_battlefield`:

```rust
    #[test]
    fn o_opens_the_options_view_on_either_sides_turn() {
        // combat_loop.c:549-553: the top-level opener runs before the
        // player_turn dispatch, so the AI's turn does not gate it.
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Options));

        // Reuse the one fixture: a second `loop_fixture()` in the same test
        // frame overflows the default test-thread stack in a debug build.
        loop_.combat.side = COMBAT_SIDE_AI;
        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Options));
    }

    #[test]
    fn c_opens_the_controls_view_on_the_players_turn_without_spending_it() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Controls));
        // combat_loop.c:310-311 returns 0: no action, the same unit still acts.
        assert!(!loop_.combat.units[COMBAT_SIDE_PLAYER][0].acted);
        assert_eq!(loop_.combat.side, COMBAT_SIDE_PLAYER);
        assert_eq!(loop_.combat.unit_id, 0);
    }

    #[test]
    fn c_is_a_player_action_so_the_ai_turn_ignores_it() {
        // combat_loop.c:306-312 lives in combat_player_action_full, reached
        // only from the player_turn arm (:612-613).
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.side = COMBAT_SIDE_AI;
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::Running);
    }

    #[test]
    fn an_armed_picker_keeps_o_and_c_from_opening_a_view() {
        // O is gated on !picker_active (combat_loop.c:549); C is the picker's
        // confirm key (combat_pick_step), and the cursor sits on our own
        // stack, which the Enemy filter refuses.
        let (mut loop_, resources, mut game) = loop_fixture();
        let mut prompt = Prompt::default();
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert!(loop_.picker_active);

        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::Running);
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::Running);
        assert!(
            loop_.picker_active,
            "a refused confirm leaves the picker armed"
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell combat::tests 2>&1 | tail -30`
Expected: FAIL, 2 tests:
`o_opens_the_options_view_on_either_sides_turn` and
`c_opens_the_controls_view_on_the_players_turn_without_spending_it`, each
with `left: Running` / `right: OpenView(...)`.
`c_is_a_player_action_so_the_ai_turn_ignores_it` and
`an_armed_picker_keeps_o_and_c_from_opening_a_view` already PASS: they pin
guards that hold today and must keep holding after Step 3.

- [ ] **Step 3: Write the implementation**

In `shell/src/combat.rs`, add the field to `CombatLoop`, after
`pending_banner_clear`:

```rust
    /// A view the player action asked for this step. C's `views_set` from
    /// inside `combat_player_action_full` (`combat_loop.c:306-312`); the
    /// action returns "not acted" and `step` reports the view.
    requested_view: Option<ViewKind>,
```

In `CombatLoop::new`, add to the struct literal after
`pending_banner_clear: false,`:

```rust
            requested_view: None,
```

In `mod tests`, `loop_fixture`'s struct literal, add the same line after
`pending_banner_clear: false,`:

```rust
            requested_view: None,
```

Replace the opener block in `step` (currently `:478-487`) with:

```rust
        // View openers work even on the AI's turn, but not while a cast or a
        // target picker is mid-flight. C's order is O, A, V
        // (combat_loop.c:549-561).
        if self.cast_phase == CastPhase::None && !self.picker_active {
            if keys.pressed(Key::O) {
                return CombatStatus::OpenView(ViewKind::Options);
            }
            if keys.pressed(Key::A) {
                return CombatStatus::OpenView(ViewKind::Army);
            }
            if keys.pressed(Key::V) {
                return CombatStatus::OpenView(ViewKind::Character);
            }
        }
```

In `step`, directly after the `// The engine recorded this action's line`
block and before `if !acted {`, add:

```rust
        // The player action opened a view (C's KEY_C). It spent no action, so
        // nothing below would run anyway.
        if let Some(view) = self.requested_view.take() {
            return CombatStatus::OpenView(view);
        }
```

At the end of `player_action`, after the `if keys.pressed(Key::G) { ... }`
block and before the final `false`, add:

```rust
        if keys.pressed(Key::C) {
            // Opening a view pauses combat in the outer loop; the C<->O swap
            // lives there too (combat_loop.c:306-312). Not an action.
            self.requested_view = Some(ViewKind::Controls);
            return false;
        }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell combat::tests`
Expected: PASS, including the 4 new tests.

- [ ] **Step 5: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass. The shell suite goes from 299 to
303.

At this point `O` and `C` open their views, but they are still invisible and
the next key closes them. Task 2 fixes both; do not verify on screen yet.

- [ ] **Step 6: Ask for approval, then commit**

Report the task and ask before running this:

```bash
git add shell/src/combat.rs
git commit -m "feat: open options and controls from combat"
```

---

### Task 2: The paused-view branch, and drawing views over the battlefield

**Files:**
- Modify: `shell/src/combat.rs:31-39` (imports), plus a new free function after
  `combat_read_dir` (`:950-976`)
- Modify: `shell/src/main.rs:36` (import), `:846-853` (combat input branch),
  `:1484-1489` (combat render branch)
- Modify: `specs/play-verification.md` §9 (`:316-324`, `:338-352`), §13
  (`:420`, `:426`), "Last refreshed" (`:45-48`)
- Modify: `AGENTS.md:207` (test counts)
- Test: `shell/src/combat.rs` (the existing `mod tests`)

**Interfaces:**
- Consumes: `Views::active() -> Option<ViewKind>`, `Views::set(ViewKind)`,
  `Views::dismiss()`, `Views::controls_cursor() -> i32`,
  `Views::controls_set_cursor(i32)`,
  `Views::controls_click(HitTarget) -> Option<usize>`,
  `Views::controls_advance(&self, &mut Game, &Resources, usize)`,
  `Views::draw(...)` (`shell/src/views.rs:1169`);
  `views::controls_visible(&Resources) -> Vec<usize>`;
  `pointer::HitTarget::{ViewDismiss, ControlsRow}`;
  `present::Present::{new, advance_scale}`; `layout::Layout::from_resources`,
  field `Layout::is_modern: bool`; `input::DIGITS: [Key; 10]`;
  `Game::default()` (whose `stats.options` is `[1; GAME_OPTIONS]`,
  `engine/src/game.rs:368`).
- Produces: `pub fn combat_view_input(views: &mut Views, keys: &dyn KeySource,
  clicked: Option<HitTarget>, game: &mut Game, resources: &Resources,
  layout: &Layout, present: &mut Present)` in `shell/src/combat.rs`.

- [ ] **Step 1: Write the failing tests**

Append to `mod tests` in `shell/src/combat.rs`:

```rust
    /// A legacy pack with a hidden control AHEAD of a visible one, so the
    /// visible row a digit labels and the catalog row C's combat branch
    /// cycles are different rows. Visible rows: Delay (0), Animation (2).
    fn view_fixture() -> (Resources, Layout, Game) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "controls": {"settings": [
                  {"id":"delay","label":"Delay","type":"numeric","range":10,"default":5},
                  {"id":"cga","label":"CGA","type":"numeric","range":8,"hidden":true},
                  {"id":"animation","label":"Animation","type":"bool"}
                ]}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        (resources, layout, Game::default())
    }

    fn press(
        views: &mut Views,
        keys: &[Key],
        game: &mut Game,
        resources: &Resources,
        layout: &Layout,
    ) {
        combat_view_input(
            views,
            &Keys::pressed(keys),
            None,
            game,
            resources,
            layout,
            &mut Present::new(),
        );
    }

    #[test]
    fn only_escape_closes_a_view_over_the_battlefield() {
        // combat_loop.c:489-493 is the only dismiss; the paused branch
        // (:502-519) reads no other key for it.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Army);
        for key in [Key::Space, Key::A, Key::Enter, Key::Digit1] {
            press(&mut views, &[key], &mut game, &resources, &layout);
            assert_eq!(
                views.active(),
                Some(ViewKind::Army),
                "{key:?} must not close it"
            );
        }
        press(&mut views, &[Key::Escape], &mut game, &resources, &layout);
        assert_eq!(views.active(), None);
    }

    #[test]
    fn c_and_o_swap_options_and_controls_in_place() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Options);

        press(&mut views, &[Key::C], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Controls));
        // Unlike the overworld (main.c:1132-1135), C does not close Controls.
        press(&mut views, &[Key::C], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Controls));

        press(&mut views, &[Key::O], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Options));
        press(&mut views, &[Key::O], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Options));

        // Digits cycle only on Controls.
        let before = game.stats.options;
        press(&mut views, &[Key::Digit1], &mut game, &resources, &layout);
        assert_eq!(game.stats.options, before);
    }

    #[test]
    fn a_combat_digit_cycles_the_catalog_row_not_the_labelled_row() {
        // The panel labels Animation "2" because CGA is hidden, but
        // combat_loop.c:511-513 hands the raw loop index to
        // views_controls_advance, skipping the overworld's vis_map.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);

        press(&mut views, &[Key::Digit2], &mut game, &resources, &layout);
        assert_eq!(game.stats.options[1], 2, "hidden CGA 1 -> 2");
        assert_eq!(game.stats.options[2], 1, "Animation untouched");
        assert_eq!(
            views.controls_cursor(),
            0,
            "the combat branch moves no cursor"
        );

        press(&mut views, &[Key::Digit1], &mut game, &resources, &layout);
        assert_eq!(game.stats.options[0], 2, "Delay 1 -> 2");
    }

    #[test]
    fn the_overworld_controls_keys_are_inert_in_combat() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        let before = game.stats.options;

        // combat_loop.c:509-517 reads digits only: no Up/Down, no
        // Enter/Space, and a digit past the catalog is views_controls_advance's
        // own out-of-range no-op (views.c:945).
        for key in [Key::Down, Key::Up, Key::Enter, Key::Space, Key::Digit9] {
            press(&mut views, &[key], &mut game, &resources, &layout);
        }
        assert_eq!(game.stats.options, before);
        assert_eq!(views.controls_cursor(), 0);
        assert_eq!(views.active(), Some(ViewKind::Controls));
    }

    #[test]
    fn a_click_on_a_controls_row_cycles_the_row_drawn_under_it() {
        // Shell-side: C has no pointer. A click names the drawn row, so the
        // second VISIBLE row is Animation, not the hidden CGA a digit reaches.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        combat_view_input(
            &mut views,
            &Keys::pressed(&[]),
            Some(HitTarget::ControlsRow(1)),
            &mut game,
            &resources,
            &layout,
            &mut Present::new(),
        );
        assert_eq!(game.stats.options[2], 0, "Animation 1 -> 0");
        assert_eq!(game.stats.options[1], 1, "CGA untouched");
        assert_eq!(views.controls_cursor(), 1);
    }

    #[test]
    fn a_click_closes_a_read_only_view_over_the_battlefield() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Character);
        combat_view_input(
            &mut views,
            &Keys::pressed(&[]),
            Some(HitTarget::ViewDismiss),
            &mut game,
            &resources,
            &layout,
            &mut Present::new(),
        );
        assert_eq!(views.active(), None);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell combat::tests 2>&1 | head -30`
Expected: FAIL to compile, with `cannot find function 'combat_view_input'`
among the errors. `Views`, `Layout`, `Present` and `HitTarget` are also
unresolved through `use super::*` until Step 3 adds the imports, so expect
several.

- [ ] **Step 3: Write `combat_view_input`**

In `shell/src/combat.rs`, change these imports:

```rust
use crate::input::{Key, KeySource};
use crate::layout::{MAP_X, MAP_Y};
```

```rust
use crate::views::{troop_sprite, ViewKind};
```

to:

```rust
use crate::input::{Key, KeySource, DIGITS};
use crate::layout::{Layout, MAP_X, MAP_Y};
```

```rust
use crate::views::{controls_visible, troop_sprite, ViewKind, Views};
```

and add these two directly after `use crate::palette::{Pal, Palette};`, so
the list stays in the order `rustfmt` sorts it:

```rust
use crate::pointer::HitTarget;
use crate::present::Present;
```

Add after `combat_read_dir`, before `#[cfg(test)]`:

```rust
/// One frame of input while a view is open over the battlefield. Combat is
/// paused: nothing here steps the fight. Ports `RunCombat`'s ESC branch
/// (`combat_loop.c:489-493`) and its paused-view branch (`:502-519`).
///
/// ESC is the ONLY key that closes a view here. C's paused branch reads no
/// other dismiss key, so Army and Character do not close on any key as they do
/// on the overworld. `C` on Options swaps to Controls and `O` on Controls swaps
/// back. A digit on Controls cycles the control at CATALOG row `digit - 1`, not
/// the visible row the panel labels with that digit: C passes its loop index
/// straight to `views_controls_advance`, skipping the `vis_map` the overworld
/// branch goes through (`main.c:1138-1142`), and moves no cursor. With a hidden
/// row ahead of it the digit and the label disagree -- ported as written.
///
/// A click is shell-side (C has no pointer) and keeps the meaning its region
/// has on the overworld: a click on a read-only view closes it, and a click on
/// a Controls row selects and cycles the row drawn under the pointer, the
/// modern Scale row included.
pub fn combat_view_input(
    views: &mut Views,
    keys: &dyn KeySource,
    clicked: Option<HitTarget>,
    game: &mut Game,
    resources: &Resources,
    layout: &Layout,
    present: &mut Present,
) {
    if let Some(target) = clicked {
        if target == HitTarget::ViewDismiss {
            views.dismiss();
        } else if let Some(row) = views.controls_click(target) {
            let vis = controls_visible(resources);
            if layout.is_modern && row == vis.len() {
                views.controls_set_cursor(row as i32);
                present.advance_scale();
            } else if let Some(&index) = vis.get(row) {
                views.controls_set_cursor(row as i32);
                views.controls_advance(game, resources, index);
            }
        }
        return;
    }

    if keys.pressed(Key::Escape) {
        views.dismiss();
        return;
    }

    let active = views.active();
    if active == Some(ViewKind::Options) && keys.pressed(Key::C) {
        views.set(ViewKind::Controls);
    } else if active == Some(ViewKind::Controls) && keys.pressed(Key::O) {
        views.set(ViewKind::Options);
    } else if active == Some(ViewKind::Controls) {
        for i in 0..9 {
            if keys.pressed(DIGITS[1 + i]) {
                views.controls_advance(game, resources, i);
                break;
            }
        }
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell combat::tests`
Expected: PASS, including the 6 new tests.

- [ ] **Step 5: Wire it into `main.rs`**

Change the import at `shell/src/main.rs:36` from:

```rust
use combat::{combat_render, CombatLoop, CombatStatus};
```

to:

```rust
use combat::{combat_render, combat_view_input, CombatLoop, CombatStatus};
```

Replace the combat input branch's view arm (currently `:847-852`):

```rust
            if views.is_active() {
                // A view is open over the battlefield; the read-only Army and
                // Character views dismiss on any key, pausing the fight.
                if keys.any_pressed() {
                    views.dismiss();
                }
            } else {
```

with:

```rust
            if views.is_active() {
                // A view is open over the battlefield and the fight is paused:
                // no step, so no AI and no animation tick. ESC closes it, C/O
                // swap Options and Controls, digits cycle Controls rows.
                combat_view_input(
                    &mut views,
                    &keys,
                    clicked,
                    &mut game,
                    &resources,
                    &layout,
                    &mut present,
                );
            } else {
```

In the combat render branch, directly after the `combat_render(...);` call
(currently ending at `:1489`) and before the `} else {` of the adventure
branch, add:

```rust
            // An open view draws over the still-visible field, porting
            // combat_present (combat_loop.c:321-327). C passes NULL map and
            // fog because no view combat can open reads them; the overworld's
            // are passed only to satisfy the signature.
            views.draw(
                &mut list,
                &mut hits,
                &layout,
                backend.palette(),
                &resources,
                &catalog,
                &map,
                &fog,
                &game,
                &present,
                get_time(),
            );
```

The prompt and dialog still draw after it (`:1543-1548`), matching
`combat_present`'s order: field, view, prompt/dialog.

- [ ] **Step 6: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass. The shell suite goes from 303 to
309.

Then the focused engine checks `AGENTS.md` asks for, which this slice must not
move:

```sh
cargo test -p openbounty-engine --lib
cargo test -p openbounty-engine --test combat_digests
cargo test -p openbounty-engine --test render_free
```

Expected: 350 engine unit tests, 25 digests, 1 render-free test, all passing.

- [ ] **Step 7: Verify it on screen**

Run:

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

Walk into a hostile foe and accept the fight. Then confirm:

1. `A` shows the Army view OVER the battlefield (the field stays visible around
   it). Before this task nothing was drawn. Space, `A` and a digit leave it up;
   ESC closes it.
2. `O` on your turn shows the Options reference. Press `C`: it becomes Controls
   without closing. Press `O`: back to Options. ESC closes it.
3. Press Space to end your turn and, while the enemy is walking, press `O`.
   Options opens and the enemy stops moving until you press ESC.
4. On the enemy's turn press `C`: nothing happens.
5. On your turn press `C`, then `1`: the Delay row cycles. Press `5`: no
   visible row changes (it cycles the hidden CGA value, rule 7). Press `6`:
   the row labelled `5 Music` would cycle, but it is an audio row and is
   grayed out, so nothing visible changes either.
6. Click the `1 Delay` row: it cycles and highlights. Open Army and click
   anywhere: it closes.
7. Arm a shot with `S`, then press `O`: no Options view. ESC, then continue.

- [ ] **Step 8: Close the gap in the docs**

In `specs/play-verification.md` §9, replace the bullet at `:316-320`
("`A` opens the Army view ...") and the GAP bullet at `:321-324` ("C also opens
an Options menu ...") with:

```markdown
- `A` opens the Army view, `V` the Character view and `O` the Options reference,
  over the still-visible battlefield, on either side's turn. `C` opens the
  Controls panel, but only on your own turn, because it is a player action
  [C]. **Opening a view pauses the fight** -- no AI advancement and no
  animation ticks while it is up [C]. All four openers are suppressed while a
  spell cast or a target picker is mid-flight, because `A` and `C` are also the
  picker's confirm keys [C].
- **Only ESC closes a view during combat** [C]. Unlike the overworld, other keys
  do not dismiss Army or Character. While a view is up, `C` on Options switches
  to Controls and `O` on Controls switches back.
- In combat, digit `N` on Controls cycles the pack's `N`th control counting
  HIDDEN rows, not the row the panel labels `N`, and moves no cursor [C]. With
  the stock pack's hidden CGA row, `5` cycles CGA and `6` cycles Music, which
  is labelled `5`. The overworld maps digits to visible rows. A click cycles the
  row under the pointer in both places [PORT].
```

In the same section's "Verify in play" list, add after item 5 (the Army-view
item):

```markdown
6. Open Options mid-fight with `O` on the enemy's turn and confirm the enemy
   stops moving. Press `C` and `O` to swap between Options and Controls, and
   confirm neither swap closes the view. ESC closes it.
```

and renumber the existing second `5.` (the title-bar item, which is misnumbered
today) to `7.`.

In §13, delete the `| Combat \`O\` | GAP | ... |` row, and change the mouse row
to:

```markdown
| Mouse: overworld & combat | BY DESIGN | Click-to-move and clicks on the combat field are design non-goals. A view opened over the battlefield takes clicks as it does on the overworld |
```

Update the "Last refreshed" line under the status legend to today's date and
the tip this commit sits on, as the Maintenance section asks.

In `AGENTS.md`, the test-count sentence (`:207`) is already stale -- it says
348 engine and 265 shell tests where the tree at the time of writing has 350
and 299. Re-derive both from the `test result:` lines of Step 6's runs rather
than from this plan, and write them in. At the time of writing that is:

```markdown
Test coverage is 350 engine unit tests, 309 shell unit tests, 25 combat digest
goldens, and one render-free boundary test.
```

- [ ] **Step 9: Ask for approval, then commit**

Report the task and ask before running this:

```bash
git add shell/src/combat.rs shell/src/main.rs specs/play-verification.md AGENTS.md
git commit -m "feat: pause combat under options and controls views"
```
