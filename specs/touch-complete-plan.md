# Touch-Complete Play Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On the Android build, every action in normal play can be done with a left click (a tap). No keyboard and no right button is needed anywhere after the hero's name is typed, and the name itself can be typed with an on-screen letter grid.

**Architecture:** Each keyboard-only surface gets hit regions that route into the SAME code its keys run, so click and key cannot drift apart. Shared guard logic moves out of the key branches into small helpers that both paths call. One new module, `numpad.rs`, draws the digit strip that the dwelling prompt and the castle recruit box share. Everything stays in `shell/`; the engine is untouched.

**Tech Stack:** Rust 2021, `openbounty-shell`, headless tests over `Keys` / `FrameInput` / `HitList`, and the `playtest` binary for on-screen checks.

**Spec:** `specs/click-everything-design.md`. This plan carries out its slices 3, 4 and 6, which are still open. The gap list comes from the 2026-11-21 re-audit (recorded in this file's "Audit" section). Where this plan departs from the design, the task says so.

## Audit (2026-11-21, every row confirmed in code)

| # | Gap | Where | Task |
|---|---|---|---|
| 1 | Chest gold/leadership A/B prompt has no tap targets | `prompt.rs:166` `click` is yes/no only | 2 |
| 2 | Dismiss-stack and sail-to-continent 1–5 pickers | same | 2 |
| 3 | Home castle rows numbered by body LINE, so the shipped pack's "A) Recruit" / "B) Speak with the King" are not the hit rows | `views.rs:2121-2129`, `views.rs:964-965` | 1 |
| 4 | Dwelling recruit count is typed only | `prompt.rs:227-254` | 3 |
| 5 | Castle recruit count is typed only | `views.rs:1142-1183`; `recruit_click` returns early at `views.rs:1217` | 4 |
| 6 | Combat: an armed shoot/fly/spell-target picker cancels only on Esc or right-click | `combat.rs:1273,1330` | 5 |
| 7 | Combat: Controls opened in a fight ignores the `ViewBack` tap | `combat.rs:1524-1537` | 5 |
| 8 | Bridge direction is arrow keys only | `session.rs:447-469` | 6 |
| 9 | Game Menu → Quit confirm is Y/N/Esc only | `session.rs:852-859` | 6 |
| 10 | World map orb toggle is Space only | `session.rs:843` | 6 |
| 11 | Win cartoon skip is any-key only | `session.rs:835` | 6 |
| 12 | Game over "any key quits" has no tap | `session.rs:862` | 6 |
| 13 | Name confirm is Enter only, and there is no way to type on a phone | `startup.rs:449` | 7 |

Deliberately NOT done, with the reason:
- **Combat Options↔Controls swap (`combat.rs:1546-1549`).** Once task 5 makes Controls close on a tap, a touch player closes one view and opens the other from the action menu. A swap button adds a surface for a two-tap saving.
- **Opening O/A/V on the AI's turn.** A touch player opens them on their own turn. This is not a lost action, just a lost moment.
- **Back-out from name entry and difficulty to class select.** Android Back is ESC (`backend.rs`), and desktop has ESC. A phone player who picked the wrong class can press Back.
- **Ctrl+Q quit from a dialog.** Game Menu → Quit (task 6 makes it tappable) is the tap path, and Android closes apps itself.
- **Android's native soft keyboard.** The design's letter grid (D5) works on every target and keeps the 10-character cap and capitalisation in one place. miniquad's `show_keyboard` would also need the char path proven on real devices.

## Global Constraints

- The engine crate must not change. All work is in `shell/src`.
- `backend.rs` stays the only module that names macroquad. No task here needs it.
- A right click may only DUPLICATE a left path (design §Right button). No new surface binds `Button::Right`.
- Click == key by construction: the click handler calls the same helper the key branch calls. Never re-implement a guard on the click side.
- Hit regions are the rectangles the surface just drew, pushed in draw order. `HitList::hit` returns the LAST pushed region that contains the point (`pointer.rs:125-133`).
- Commit messages are one subject line in conventional form, with no body and no trailers. **Ask the user before every commit** (AGENTS.md).
- Before each commit, run `cargo fmt --all --check`, `cargo test -p openbounty-shell` and `python3 tools/check_independent.py`.
- New user-facing text uses our own words, and any pack-overridable label gets a `DEFAULT_*` constant next to the existing ones.

## Review Focus

1. **A tap on the panel's text, not the strip, must not answer a prompt.** A chest body line containing "A)" is not a hit region. Only the strip answers. (Task 2 test `body_click_does_not_answer`.)
2. **The shipped chest body must still fit once the A/B strip takes a row.** If the body is clipped, the player loses the text that says what A and B are. (Task 2 test `shipped_chest_body_fits_above_the_strip`.)
3. **Max must never exceed what can be bought.** The dwelling prompt's `max_value` is the session's cap, and the castle Max is `recruit_max`. A Max tap followed by OK must buy exactly what Enter with that number typed buys. (Tasks 3 and 4.)
4. **Cancelling a combat picker by tapping a non-target must not spend the unit's turn or a spell charge.** (Task 5 test `invalid_target_tap_cancels_without_acting`.)
5. **A bridge tap on a non-adjacent or diagonal tile must do what the keyboard can't**, so it is ignored, not "built toward" the tile. Only the 4 orthogonal neighbours count, matching the arrow directions the bridge branch passes to the engine's `try_build_bridge` (`engine/src/spells_adventure.rs:164`). (Task 6 test `bridge_tap_far_tile_is_ignored`.)

---

### Task 1: Home castle rows hit the "A)" and "B)" lines

**Files:**
- Modify: `shell/src/views.rs` (`draw_home_castle`, around `:2110-2132`; `home_castle_click` `:961-968` is unchanged)
- Test: `shell/src/views.rs` tests module

**Interfaces:**
- Produces: `fn home_castle_choice(line: &str) -> Option<usize>`, private in `views.rs`. It returns `Some(0)` for a line whose first non-space characters are `A)`, `A.`, `1)` or `1.` (case-insensitive), `Some(1)` for the same with `B` or `2`, and `None` otherwise.

The body is pack text. The shipped pack writes `"The Royal Hall\n\nA) Recruit soldiers\nB) Speak with the King"`, and the compiled default is `"1. Recruit Soldiers\n2. Audience with the King"` (`interact.rs:687`). Push `HitTarget::HomeCastleRow(choice)` only for lines where `home_castle_choice` is `Some`, so row 0 is Recruit and row 1 is Audience whatever line they sit on.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn home_castle_choice_reads_letter_and_number_prefixes() {
    assert_eq!(home_castle_choice("A) Recruit soldiers"), Some(0));
    assert_eq!(home_castle_choice("  b) Speak with the King"), Some(1));
    assert_eq!(home_castle_choice("1. Recruit Soldiers"), Some(0));
    assert_eq!(home_castle_choice("2. Audience"), Some(1));
    assert_eq!(home_castle_choice("The Royal Hall"), None);
    assert_eq!(home_castle_choice(""), None);
}

#[test]
fn home_castle_rows_sit_on_the_choice_lines_of_the_shipped_body() {
    // Draw the home castle with body_home_castle =
    // "The Royal Hall\n\nA) Recruit soldiers\nB) Speak with the King".
    // Exactly two HomeCastleRow regions are published. Row 0's rect has the
    // y of the third drawn line ("A) ..."), row 1's the fourth.
    // home_castle_click(HomeCastleRow(0)) == HomeCastleAction::Recruit.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib home_castle`. Expected: FAIL (`home_castle_choice` not defined).
- [ ] **Step 3:** Implement `home_castle_choice` and change the draw loop to push regions only for choice lines.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell --lib home_castle`. Expected: PASS, along with the existing home-castle anti-drift test, which may need its row indices updated to choice indices.
- [ ] **Step 5:** Ask the user, then commit `fix: tap the recruit and audience lines at the home castle`.

---

### Task 2: Tap strips for the 1–5 picker and the A/B choice

**Files:**
- Modify: `shell/src/pointer.rs` (`HitTarget`), `shell/src/prompt.rs` (`click`, `draw`, `bottom_rows`)
- Test: `shell/src/prompt.rs` tests

**Interfaces:**
- Produces: `HitTarget::PromptPick(usize)`. For a numeric prompt, `PromptPick(i)` is choice `i + 1` (`i` in `0..max_choice`). For an A/B prompt, `PromptPick(0)` is A and `PromptPick(1)` is B. Both resolve through the existing `index_result(i)` (`prompt.rs:367`) and `self.dismiss()`, exactly as the key branches do.
- DIVERGENCE from the design's naming: the design wrote `PromptPick(1/2)` for A/B. This plan uses 0-based `PromptPick(0/1)` so that A/B and numeric share `index_result(i)` with no offset.

Draw rules:
- **Numeric:** the hint row (`y + h - row_h - 2`) becomes a strip of `max_choice` equal cells across the panel's inner width (`x + pad .. x + w - pad`). Each cell is labelled with its digit, centred, in yellow.
- **A/B:** `bottom_rows` becomes 1. The same row splits into two halves labelled `A` and `B`, mirroring the yes/no halves at `prompt.rs:357-363`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn numeric_pick_click_equals_digit_key() {
    for max in 1..=5 {
        for i in 0..max as usize {
            let mut by_key = Prompt::default();
            by_key.numeric("h", "b", max);
            let keyed = by_key.update(&Keys::pressed(&[DIGITS[i + 1]]));
            let mut by_click = Prompt::default();
            by_click.numeric("h", "b", max);
            assert_eq!(by_click.click(HitTarget::PromptPick(i)), keyed);
            assert!(!by_click.is_active());
        }
    }
}

#[test]
fn ab_pick_click_equals_a_and_b_keys() {
    // PromptPick(0) == Key::A (PromptResult::One); PromptPick(1) == Key::B (Two).
}

#[test]
fn pick_strip_regions_match_their_choices() {
    // Draw a numeric(max=3) prompt. Exactly 3 PromptPick regions, left to
    // right, inside the panel rect, each containing the x of its digit glyph.
    // Draw an A/B prompt: 2 PromptPick regions, the halves of one row.
}

#[test]
fn body_click_does_not_answer() {
    // An A/B prompt whose body is "A) Take the gold\nB) Take the leadership":
    // no region is published over the body rows, and click(PromptPick(5))
    // (out of range) returns PromptResult::None and leaves it active.
}

#[test]
fn shipped_chest_body_fits_above_the_strip() {
    // Load the chest body from packs/stolen-scepter/game.json (the string
    // session.rs:1106 passes to `Prompt::ab`), draw it, and assert every word of
    // it appears in the DrawList's text.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib prompt`. Expected: FAIL (`PromptPick` not defined).
- [ ] **Step 3:** Add `PromptPick(usize)` to `HitTarget` with a doc comment. Widen `Prompt::click` to match on the kind: YesNo as now, Numeric `PromptPick(i) if i < max_choice`, AbChoice `PromptPick(0|1)`. Draw and publish the strips.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell --lib prompt`. Expected: PASS. Then run `cargo test -p openbounty-shell`. Expected: all pass; the existing yes/no tests are untouched.
- [ ] **Step 5:** Playtest. Find a chest with the playtest map's seed, `click` the strip's A cell, and `expect` the gold message. Then open Dismiss from the hero menu, `click` cell 1, and `expect` the same text `key 1` shows.
- [ ] **Step 6:** Ask the user, then commit `feat: tap the numbered and A/B prompt choices`.

---

### Task 3: Shared digit strip, used by the dwelling count prompt

**Files:**
- Create: `shell/src/numpad.rs` (add `pub mod numpad;` in `shell/src/lib.rs`)
- Modify: `shell/src/pointer.rs`, `shell/src/prompt.rs`
- Test: `shell/src/numpad.rs`, `shell/src/prompt.rs` tests

**Interfaces:**
- Produces in `numpad.rs`:
  - `#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)] pub enum PadKey { Digit(u8), Del, Max, Ok }`
  - `pub const PAD_KEYS: [PadKey; 13]`, in order `Digit(0)..=Digit(9), Del, Max, Ok`
  - `pub fn cells(strip: Rect) -> [(Rect, PadKey); 13]`, which splits `strip` into 13 cells left to right. The ten digits each get one width unit; `Del`, `Max` and `Ok` get two units each (16 units in total), and the last cell takes the rounding remainder.
  - `pub fn draw(list: &mut DrawList, hits: &mut HitList, strip: Rect, palette: &Palette, hover: Option<PadKey>)`, which draws each cell's label centred (`0`–`9`, `<`, `MAX`, `OK`), a highlight under `hover`, and pushes `HitTarget::Pad(key)` per cell.
- Produces `HitTarget::Pad(PadKey)`. One variant serves both count surfaces, resolved by whichever surface is active, the same way `ViewBack` already works. DIVERGENCE from the design's separate `PromptDigit`/`RecruitDigit` families: one pad, one target.
- Produces in `prompt.rs` (private), all called by BOTH `update`'s key branch and `click`:
  - `fn type_digit(&mut self, d: u8)`: the length and value guards moved from `prompt.rs:229-241`
  - `fn backspace(&mut self)`
  - `fn fill_max(&mut self)`: `self.buffer = self.max_value.to_string()`, truncated to `max_digits`
  - `fn commit(&mut self) -> PromptResult`: `self.kind = Kind::None; PromptResult::Yes`

Draw rule: for `Kind::TextInput`, the hint row becomes the pad strip, `Rect::new(x + pad, y + h - row_h - 2, w - 2 * pad, row_h)`. The typed-value row above it stays.

- [ ] **Step 1: Write the failing tests**

```rust
// numpad.rs
#[test]
fn cells_tile_the_strip_without_gaps() {
    let strip = Rect::new(10, 100, 237, 9);
    let c = cells(strip);
    assert_eq!(c[0].0.x, 10);
    assert_eq!(c[12].0.x + c[12].0.w, 10 + 237);
    for k in 1..13 { assert_eq!(c[k].0.x, c[k - 1].0.x + c[k - 1].0.w); }
    assert_eq!(c.map(|(_, key)| key), PAD_KEYS);
}

// prompt.rs
#[test]
fn pad_clicks_type_the_same_count_as_keys() {
    // text_input("h","b",3,50): click Pad(Digit(4)), Pad(Digit(2)) -> buffer "42";
    // Pad(Digit(9)) rejected (429 > 50, same as the key path); Pad(Del) -> "4";
    // Pad(Ok) -> PromptResult::Yes, text_input_value() == 4.
}

#[test]
fn pad_max_fills_the_value_cap() {
    // text_input("h","b",4,137): Pad(Max) then Pad(Ok) -> text_input_value() == 137.
}

#[test]
fn pad_regions_are_published_only_for_text_input() {
    // A text input publishes 13 Pad regions inside the panel; yes/no, numeric
    // and A/B publish none.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib numpad prompt`. Expected: FAIL.
- [ ] **Step 3:** Create `numpad.rs`. Add `HitTarget::Pad`. Extract the four helpers, route `update` through them, add `Pad` handling to `click`, and draw the strip.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS, including the existing `text_input` tests at `prompt.rs:561-622`.
- [ ] **Step 5:** Playtest. Walk to a dwelling, `click` Max then OK, and `expect` the same army text that typing the max and pressing Enter gives.
- [ ] **Step 6:** Ask the user, then commit `feat: tap a recruit count at a dwelling`.

---

### Task 4: The castle recruit count uses the same strip

**Files:**
- Modify: `shell/src/views.rs` (`recruit_soldiers_update` `:1142-1183`, `recruit_click` `:1195-1234`, `draw_recruit_soldiers` `:2300-2337`), `shell/src/session.rs` only if the recruit click call site filters targets
- Test: `shell/src/views.rs` tests

**Interfaces:**
- Consumes: `numpad::{PadKey, cells, draw}` and `HitTarget::Pad` from task 3.
- Produces (private on `Views`), each called by both the key branch and `recruit_click`:
  - `fn recruit_type_digit(&mut self, d: u8)`: pushes while `recruit_input.len() < 6` (the existing guard at `:1179`)
  - `fn recruit_backspace(&mut self)`
  - `fn recruit_fill_max(&mut self)`: `self.recruit_input = self.recruit_max.to_string()`
  - `fn recruit_commit(&mut self, game: &mut Game, resources: &Resources)`: the body of the Enter branch at `:1146-1170`, moved unchanged (clamp to `recruit_recompute_max`, `buy_troop`, refusal message, reset `recruit_whom` and `recruit_input`)

`recruit_click` currently returns `false` early when `recruit_whom != 0` (`:1217`). In that phase it must now accept `HitTarget::Pad(key)` and dispatch to the helpers. The pad is drawn only while `recruit_whom != 0`, on the row below `"{input}_"` (`rby + 4 * row_h`), spanning from `tx` to the panel's inner right edge.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn recruit_pad_commit_buys_what_enter_buys() {
    // Two identical games at the home castle. In game A, pick row 0 and press
    // digits "1","2", then Enter. In game B, click RecruitRow(0), Pad(Digit(1)),
    // Pad(Digit(2)), Pad(Ok). Assert game_a.army == game_b.army and
    // game_a.stats.gold == game_b.stats.gold.
}

#[test]
fn recruit_pad_max_then_ok_buys_recruit_max() {
    // Pick a row, click Pad(Max), Pad(Ok): the bought count == recruit_max
    // read before the commit.
}

#[test]
fn recruit_pad_is_inside_the_panel_and_only_after_a_pick() {
    // Before a row is picked, no Pad region. After, 13 Pad regions, all inside
    // the recruit panel's rect.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib recruit`. Expected: FAIL.
- [ ] **Step 3:** Extract the helpers, widen `recruit_click`, and draw the pad.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS, including the existing `RecruitRow` and `ViewBack` recruit tests.
- [ ] **Step 5:** Playtest. At the home castle, click the recruit row (task 1), a troop row, then `MAX` and `OK`, and `expect` the army view to list the troop.
- [ ] **Step 6:** Ask the user, then commit `feat: tap a recruit count at the castle`.

---

### Task 5: Combat picker cancel and the Controls close

**Files:**
- Modify: `shell/src/combat.rs` (`pick_step` `:1263-1333`, `combat_view_input` `:1515-1556`)
- Test: `shell/src/combat.rs` tests

**Interfaces:**
- Consumes: the existing `HitTarget::CombatCell`, `HitTarget::ViewBack` and `cursor_passes`.

Rules:
- **Picker cancel.** In `pick_step`, a `CombatCell(x, y)` click on a cell where `cursor_passes` is false sets `*out_cancelled = true` and returns false. That is the same result Esc gives at `:1330`. DIVERGENCE: on the keyboard a failed target leaves the picker armed. On a touch screen every cell is tappable, so "tap something that isn't a target" is the only cancel a one-button player has, and it closes the soft-lock where no valid target exists (Turn Undead with no undead).
- **Controls close.** In `combat_view_input`, `HitTarget::ViewBack` calls `views.dismiss()`, the same as Esc at `:1540`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn invalid_target_tap_cancels_without_acting() {
    // Arm Shoot on an archer (CombatAction::Shoot via the action menu), then
    // click a CombatCell holding no enemy. The picker is no longer armed, the
    // same unit is still to act, and the field state equals a run that
    // pressed Esc instead.
}

#[test]
fn spell_target_with_no_valid_target_can_be_cancelled_by_tap() {
    // Cast a spell whose filter matches nothing on the field, tap any cell:
    // cast_phase returns to None and the spell count is unchanged.
}

#[test]
fn combat_controls_close_on_view_back() {
    // Open Controls through CombatAction::Controls, then
    // combat_view_input(.., Some(HitTarget::ViewBack), ..) leaves no view active.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib combat`. Expected: FAIL.
- [ ] **Step 3:** Make the two edits.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS. The existing combat click-equivalence tests (`assert_same`, `combat.rs:~2510-2810`) must stay green. If one taps a non-target expecting the picker to stay armed, it asserts the old rule: update it and name the divergence in its comment.
- [ ] **Step 5:** Ask the user, then commit `feat: tap off a target to cancel a combat pick`.

---

### Task 6: One-off overworld screens

**Files:**
- Modify: `shell/src/pointer.rs`, `shell/src/session.rs` (bridge `:447-469`, win cartoon `:832-837`, worldmap `:838-850`, fast quit `:852-859`, game over `:862`, plus the draw pass where `StatusMode` is chosen, `~:1344`), `shell/src/views.rs` (hit publishing for `WinCartoon` and `Worldmap`, `~:1356-1375`)
- Test: `shell/src/session.rs` tests (use `drive`, `to_overworld`, `region_center` at `:2412-2452`)

**Interfaces:**
- Produces: `HitTarget::{WinCartoonSkip, FastQuitYes, FastQuitNo, WorldmapToggle, GameOverQuit}`.
- Status-bar rect, used by fast quit and the worldmap toggle: `Rect::new(STATUS_X, STATUS_Y, layout.status_w(), STATUS_H)` (`layout.rs:28-30,201`).

Rules (each resolves to the code its key runs):
- **Bridge.** While `bridge_waiting`, a click on `MapTile(mx, my)` with `(mx - hero.x, my - hero.y)` one of `(0,-1),(0,1),(-1,0),(1,0)` sets `dx, dy` and runs the existing arrow branch. Any other tile is ignored. This needs the dialog's `DialogDismiss` region NOT to cover the map. If it does, publish the map tiles after the dialog while `bridge_waiting`.
- **Fast quit.** While `*fast_quit`, publish `FastQuitYes` over the left half of the status bar and `FastQuitNo` over the right half, matching the "(Y/N)" order. Yes returns `FrameStatus::Quit`; No clears `fast_quit`.
- **World map.** When the worldmap is active and `worldmap_has_orb`, publish `WorldmapToggle` over the status bar AFTER `ViewDismiss`, so it wins there and a tap anywhere else still dismisses. It calls `views.toggle_whole_map()`.
- **Win cartoon.** While it is active, publish `WinCartoonSkip` full-screen. It calls `views.finish_win_cartoon()`.
- **Game over.** When `game.is_over()` and nothing modal is up, publish `GameOverQuit` full-screen. It returns what `game_over_quits` returns for a key press.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn bridge_tap_on_a_neighbour_builds_like_the_arrow() {
    // Two sessions with the bridge spell and water east of the hero. In A,
    // cast Bridge and press Right. In B, cast Bridge and click the east
    // MapTile. Compare the game and map tiles: equal.
}

#[test]
fn bridge_tap_far_tile_is_ignored() {
    // Clicking a tile two east, or the diagonal: bridge_waiting stays true and
    // the map is unchanged.
}

#[test]
fn fast_quit_halves_answer_like_y_and_n() {
    // Open Game Menu -> Quit. Clicking FastQuitNo leaves the game running with
    // fast_quit false. Reopen; clicking FastQuitYes returns FrameStatus::Quit.
}

#[test]
fn worldmap_toggle_needs_the_orb() {
    // With the orb, clicking WorldmapToggle flips whole_map and keeps the view
    // open. Without the orb, no WorldmapToggle region is published.
}

#[test]
fn win_cartoon_skip_equals_any_key() {}

#[test]
fn game_over_tap_quits_like_a_key() {}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib session`. Expected: FAIL.
- [ ] **Step 3:** Add the variants, publish the regions, and route the clicks in each branch.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Playtest. Cast Bridge next to water and `click` the water tile, `expect` the bridge-built text. Then open Game Menu → Quit and `click` the right half of the status bar, `expect` the overworld status line.
- [ ] **Step 6:** Ask the user, then commit `feat: tap the bridge, quit, world map and ending screens`.

---

### Task 7: On-screen letter grid for the hero's name

**Files:**
- Modify: `shell/src/pointer.rs`, `shell/src/startup.rs` (`update_name` `:440-480`, `draw_create_game` `:786-860`, `click` `:240-285`)
- Test: `shell/src/startup.rs` tests

**Interfaces:**
- Produces: `HitTarget::NameKey(char)` (`'A'..='Z'` and `' '`), `HitTarget::NameDel` and `HitTarget::NameDone`.
- Produces (private on `Startup`), called by both `update_name` and `click`:
  - `fn name_type(&mut self, ch: char)`: the `NAME_MAX` cap (`startup.rs:63`) and first-letter-uppercase rule (`:459`) moved out of the char loop
  - `fn name_backspace(&mut self)`
  - `fn name_confirm(&mut self, resources: &Resources)`: the Enter branch at `:449`, including the empty-name default

Grid: 3 rows × 10 cells, each cell `2 * GLYPH_W` wide and `GLYPH_H + 3` tall, centred under the name line, drawn only while the name is being entered (before the difficulty rows appear). Row 1 is `A`–`J`, row 2 `K`–`T`, row 3 `U`–`Z`, then space (drawn as `_`), `<` (Del), and `OK` (Done) spanning the last two cells. The grid's letters type in lower case after the first letter, through `name_type`, exactly as typed keys do.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn clicking_letters_types_like_the_keyboard() {
    // On CreateGame: click NameKey('S'), NameKey('I'), NameKey('R'). The name
    // equals what Keys::default().with_chars(&['S','i','r']) over three frames
    // produces ("Sir").
}

#[test]
fn name_grid_keeps_the_cap_and_del_and_done() {
    // Twelve NameKey clicks stop at NAME_MAX. NameDel removes one. NameDone
    // moves to the difficulty rows exactly as Enter does (screen state and
    // name equal).
}

#[test]
fn empty_done_uses_the_default_name() {}

#[test]
fn grid_regions_cover_their_glyphs_and_vanish_after_done() {}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib startup`. Expected: FAIL.
- [ ] **Step 3:** Extract the helpers, draw the grid, and route the clicks.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Playtest. Click a class, click `A`, `D`, `A`, `OK`, then the Normal row, and `expect` "Ada the Knight".
- [ ] **Step 6:** Ask the user, then commit `feat: type the hero name on an on-screen grid`.

---

### Task 8: Touch-only acceptance on the Android emulator

**Files:**
- Create: `shell/tests/touch_only.txt`, a playtest script using only `click` (no `key`, `type` or `rclick`)
- Modify: `specs/click-everything-design.md` (mark slices 3, 4 and 6 done, and point to this plan), and `AGENTS.md` (one line under the shell layout naming `numpad.rs`)

- [ ] **Step 1:** Write the script: start a game by tapping (class, name grid, difficulty), walk by tapping, open a chest and take gold, recruit at a dwelling with Max/OK, recruit at the castle, cast Bridge, open and close the world map, and quit through the Game Menu. Each step `expect`s its result text.
- [ ] **Step 2:** Run `cargo run -p openbounty-shell --bin playtest -- --script shell/tests/touch_only.txt --out target/touch-only --seed 7`. Expected: exit 0, every `expect` met.
- [ ] **Step 3:** Run `sh tools/build_apk.sh`, install the APK on the emulator, and repeat the same path with `adb shell input tap` only. Screenshot each step. Check that Quit closes the app cleanly. If it leaves a frozen surface, note it for the Android follow-up list; don't fix it here.
- [ ] **Step 4:** Update the design doc status and AGENTS.md.
- [ ] **Step 5:** Ask the user, then commit `test: play a whole game by touch alone`.
