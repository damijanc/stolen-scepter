# Mobile Touch UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On Android, every choice is a big, finger-sized button in plain words, while desktop returns to its pre-2026-11-21 look (with the kept items).

**Architecture:** A `touch` flag on `Layout` tells every draw call which build it is in. One `sheet` module draws all mobile choices as big buttons over the inner screen, and each screen feeds it the options plus the SAME `HitTarget`s its desktop rows already publish, so the click routing is unchanged. Mobile text comes from `mobile_<group>` string groups, falling back to mobile defaults.

**Tech Stack:** Rust 2021, `openbounty-shell`, headless tests over `Keys` / `FrameInput` / `HitList`, the `playtest` binary.

**Spec:** `specs/mobile-ui-design.md` (approved 2026-11-21). Decisions 1-9 there are the user's.

## Global Constraints

- `openbounty-engine` is untouched. `backend.rs` stays the only module naming macroquad.
- Desktop (`layout.touch == false`) draws exactly what the pre-2026-11-21 build drew, except the kept items in spec decision 2.
- Every mobile button publishes an existing `HitTarget` that the screen's `*_click` / `click` already handles, unless a task adds a new one.
- Every mobile tap region is at least 26 logical pixels tall (`sheet::MIN_BUTTON_H`).
- Labels are drawn at 2x glyph size (16 logical px). A label wraps to at most 2 lines, then is cut with "..".
- Mobile text: look up `strings.mobile_<group>.<key>`, then the mobile default. Desktop never reads `mobile_*`.
- New strings use our own words (`tools/check_independent.py`); pack text is edited in `packs/stolen-scepter/data/text.json`, then `python3 tools/build_pack.py` regenerates `game.json`.
- Commit messages: one conventional subject line, no body, no trailers. Ask the user before committing (AGENTS.md) unless they have approved per-task commits for this run.
- Before each commit: `cargo fmt --all --check`, `cargo test -p openbounty-shell`, `python3 tools/check_independent.py`.

## Review Focus

1. **A sheet with more options than fit on a page** (17 hero actions, 10 save slots plus "New game", 14 spells): every option must be reachable through Next/Prev, with Back on every page. (Task 2 test `long_lists_page_and_every_option_is_reachable`.)
2. **A page number left over from an earlier sheet:** opening the hero menu after paging through save slots must start on page 1. (Task 2 test `the_pager_resets_when_the_sheet_changes`.)
3. **A label longer than two lines:** the button's height must stay bounded and the text must be cut with "..", not overflow onto the next button. (Task 2 test `a_label_wraps_to_two_lines_then_is_cut`.)
4. **A town purchase the engine would refuse:** the refusal shows at once, with no confirm, and no gold is spent. (Task 8 test `a_refused_purchase_skips_the_confirm`.)
5. **A pack string that names a key but has no mobile version:** on mobile it must fall back to the mobile default, never the desktop text. (Task 4 test `every_key_naming_string_shown_on_mobile_has_a_mobile_version`.)

---

### Task 1: The touch flag, big text, and `--mobile` in playtest

**Files:**
- Modify: `shell/src/layout.rs` (`Layout`), `shell/src/draw.rs` (`DrawCmd`, `DrawList`), `shell/src/backend.rs` (draw `BigText`), `shell/src/raster.rs` (paint `BigText`, include it in `texts`), `shell/src/main.rs` (set `touch` from `Target::native()`), `shell/src/bin/playtest.rs` (`--mobile`)

**Interfaces:**
- Produces: `Layout.touch: bool` (false from `Layout::from_resources`; main sets it to `present::Target::native() == Target::Mobile`).
- Produces: `DrawCmd::BigText { text: String, x: i32, y: i32, color: Color, scale: i32 }`, `DrawList::big_text(&mut self, text: &str, x: i32, y: i32, color: Color, scale: i32)`, `DrawList::big_text_centered(&mut self, text: &str, cx: i32, y: i32, color: Color, scale: i32)`.
- Produces: `DrawList::texts()` and `raster::texts()` also return `BigText` strings, so playtest `expect` and tests see sheet labels.
- Produces: `playtest --mobile`, which sets `layout.touch = true` and uses `Target::Mobile` placement at the `--scale` window size.

- [ ] **Step 1: Write the failing tests**

```rust
// draw.rs
#[test]
fn big_text_is_listed_with_the_other_text() {
    let mut list = DrawList::new();
    list.big_text_centered("Yes", 100, 50, Color::rgb(255, 255, 0), 2);
    assert_eq!(list.texts(), vec![(100 - 3 * 8, 50, "Yes")]); // 3 glyphs x 16px, centred
}
// raster.rs
#[test]
fn big_text_paints_each_glyph_at_its_scale() {
    // Paint "A" at scale 2 and at scale 1 on two canvases. The scale-2 ink
    // bounding box is exactly twice as wide and tall as the scale-1 one.
}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib big_text`. Expected: FAIL (no `big_text`).
- [ ] **Step 3:** Implement the variant and the helpers. In backend and raster, draw `BigText` exactly like `Text`, with each glyph's destination size multiplied by `scale`. Add the `touch` field, set it in `main.rs`, and add the playtest flag.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add a touch flag and big text for the mobile UI`.

---

### Task 2: The choice sheet

**Files:**
- Create: `shell/src/sheet.rs` (add `pub mod sheet;` to `lib.rs`)
- Modify: `shell/src/pointer.rs` (`HitTarget::SheetPrev`, `HitTarget::SheetNext`)

**Interfaces:**
- Consumes: `DrawList::big_text_centered` (Task 1).
- Produces:
  - `pub const MIN_BUTTON_H: i32 = 26;`
  - `pub const LABEL_SCALE: i32 = 2;`
  - `pub fn area(layout: &Layout) -> Rect`, which is the whole inner screen: `Rect::new(FRAME_LEFT_W, FRAME_TOP_H, layout.screen_w - FRAME_LEFT_W - FRAME_RIGHT_W, layout.screen_h - FRAME_TOP_H - FRAME_BOTTOM_H)` (288x184 legacy).
  - `pub struct SheetButton { pub label: String, pub target: HitTarget }`
  - `pub enum Columns { One, Two }`
  - `pub struct Sheet { pub key: &'static str, pub title: String, pub body: String, pub buttons: Vec<SheetButton>, pub columns: Columns, pub back: Option<HitTarget> }`. `key` names the screen, for pager resets.
  - `#[derive(Default)] pub struct Pager { page: usize, key: &'static str }` with `pub fn page_for(&mut self, sheet: &Sheet) -> usize` (resets to 0 when `sheet.key` changes, and clamps to the last page), `pub fn next(&mut self)` and `pub fn prev(&mut self)`.
  - `pub fn draw(list: &mut DrawList, hits: &mut HitList, layout: &Layout, palette: &Palette, sheet: &Sheet, page: usize)`.
  - `pub fn keypad(list: &mut DrawList, hits: &mut HitList, layout: &Layout, palette: &Palette, title: &str, value: &str, max: Option<i32>)`. It draws a 4x4 keypad (`7 8 9 <`, `4 5 6 MAX`, `1 2 3 Back`, `0` spanning 2 columns, `OK` spanning 2 columns), publishing `HitTarget::Pad(PadKey::…)` with `Back` as `Pad(PadKey::Esc)`.
- Layout rules:
  - The area is filled `Pal::DBlue` with a `Pal::Yellow` frame.
  - The title is in big text on the top line.
  - The body is in normal 8px text, wrapped to the area width.
  - Buttons stack below the body and use `DrawList::button`, with big text and a 2px gap.
  - A label wraps to 2 lines, which makes the button `2 * 16 + 10` px tall.
  - The bottom row holds `Back` (when `back` is set) and `Prev` / `Next` (only when there is more than one page).
  - A page holds as many buttons as fit above that row.

- [ ] **Step 1: Write the failing tests**

```rust
#[test] fn every_button_is_at_least_min_height_and_inside_the_area() {}
#[test] fn regions_are_exactly_the_drawn_buttons() {}   // anti-drift, as other surfaces
#[test] fn a_label_wraps_to_two_lines_then_is_cut() {
    // A 60-char label: its button holds two text lines, the second ends in ".."
    // and the next button starts below it.
}
#[test] fn long_lists_page_and_every_option_is_reachable() {
    // 17 buttons: walking Next through pages visits each target once; Back is
    // published on every page; Prev is absent on page 0, Next absent on the last.
}
#[test] fn the_pager_resets_when_the_sheet_changes() {}
#[test] fn the_keypad_publishes_every_pad_key_at_min_height() {}
```

- [ ] **Step 2:** Run `cargo test -p openbounty-shell --lib sheet`. Expected: FAIL.
- [ ] **Step 3:** Implement `sheet.rs` and the two new `HitTarget` variants.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: add the mobile choice sheet`.

---

### Task 3: Restore the desktop yes/no line and name screen

**Files:**
- Modify: `shell/src/prompt.rs` (YesNo draw), `shell/src/startup.rs` (name grid only when `layout.touch`)

**Interfaces:**
- Produces, on desktop: the yes/no row draws the pack's `prompts.yes_no_hint` (default `"(y/n)?"`) centred, publishing `PromptYes` / `PromptNo` over the left and right halves of that row, exactly as at `ad77d4e`. Restore `DEFAULT_YES_NO_HINT`.
- Mobile keeps the 2026-11-21 buttons until Task 5 replaces them with a sheet.

- [ ] **Step 1: Write the failing tests**: `desktop_yes_no_shows_the_hint_with_halves` (the hint text is drawn, there is no `DrawCmd::Rect` button block in the row, and the halves hit `PromptYes` / `PromptNo`), and `desktop_name_screen_has_no_letter_grid` (no `NameKey` regions when `touch` is false; 29 regions when it is true).
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Branch the two draws on `layout.touch`.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5: Desktop pixel check.**
  - Build `ad77d4e` in a worktree.
  - Run the keyboard script from this session (`target/desk/keys.txt`) on both builds, plus a mouse variant that clicks the yes/no halves and the hero menu.
  - Expected: only these screens differ: the dismiss strip, the count strips and the quit buttons (kept items).
- [ ] **Step 6:** Commit `fix: restore the classic yes/no line and name screen on desktop`.

---

### Task 4: Mobile text

**Files:**
- Modify: `shell/src/banner.rs`, the call sites of the key-naming defaults in `session.rs:1576`, `combat.rs:47,77`, `startup.rs:31,33`, `interact.rs:393,688-689`, `chrome.rs:19-24`, `menu.rs:31`, `packs/stolen-scepter/data/text.json`, the generated `game.json`

**Interfaces:**
- Produces: `banner::text_on(resources: &Resources, touch: bool, group: &str, key: &str, default: &str, mobile_default: &str) -> &str`, which looks up `mobile_<group>.<key>`, then `mobile_default` when `touch`, and otherwise behaves like `banner::text`. Also `banner::formatted_on`, with the same `touch` and `mobile_default` parameters as `formatted`.
- Produces: in `text.json`, `strings.mobile_banners`, `mobile_ui`, `mobile_combat_log` and `mobile_prompts` entries for every key-naming string found by the audit:
  - `banners.spell_bridge_prompt` → "Raise a bridge which way?\nTap a tile next to you."
  - `banners.astrology_body` → the same text with " (space)" replaced by " (tap)"
  - `banners.combat_give_up_header`, `combat_log.exit_hint`, `ui.press_esc_to_exit` → "Back to leave"
  - `banners.body_save_confirm` → "Your game is saved.\nTap to return to the game."
  - `ui.worldmap_hint_your_map` / `ui.worldmap_hint_whole_map` → "Tap to close"
  - `ui.own_castle_mode_garrison` / `ui.own_castle_mode_remove` → "Leave troops" / "Take troops"
  - the `prompts.*` hints → empty (sheets replace them)

  Each shell default gets a matching `MOBILE_DEFAULT_*`.

- [ ] **Step 1: Write the failing tests.**
  - `text_on_prefers_the_mobile_mirror_only_on_touch`
  - `every_key_naming_string_shown_on_mobile_has_a_mobile_version`: load the shipped pack; for every `strings.<group>.<key>` matching `(?i)\b(esc|enter|space|arrow|ctrl|key)\b|\(y/n\)`, assert that `mobile_<group>.<key>` exists and doesn't match. `startup.controls_hint`, `startup.new_game_select_hint` and `keybinds` are exempt, because their screens are sheets or hidden on mobile.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement `text_on`, switch the listed call sites to it (passing `layout.touch`), add the mirrors to `text.json`, and run `python3 tools/build_pack.py`.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell && python3 tools/check_independent.py`. Expected: PASS, 0 problems.
- [ ] **Step 5:** Commit `feat: plain mobile wording without key names`.

---

### Task 5: Prompts as sheets

**Files:**
- Modify: `shell/src/prompt.rs`, `shell/src/session.rs` (dismiss `:1008`, continent `:1023`, chest `:1106`, dwelling `:1249`), `shell/src/interact.rs` (option labels)

**Interfaces:**
- Consumes: `sheet::{Sheet, SheetButton, draw, keypad, Pager}`, `banner::text_on`.
- Produces:
  - `Prompt::numeric_labeled(&mut self, header: &str, body: &str, labels: Vec<String>)`, with `max_choice = labels.len()` (1..=5).
  - `Prompt::ab_labeled(&mut self, header: &str, body: &str, labels: [String; 2])`.
  - `Prompt::draw` gains `pager: &mut sheet::Pager`.
  - `interact::chest_option_labels(resources: &Resources, roll: &ChestRoll) -> [String; 2]`, with defaults "Keep %GOLD% gold" and "Share it (+%LEADERSHIP% leadership)", pack keys `mobile_banners.chest_keep` / `chest_share`.
  - `interact::stack_label(resources, game, slot) -> String` ("Militia x20").
- On touch, `Prompt::draw` renders:
  - YesNo → a sheet with buttons "Yes" (`PromptYes`) and "No" (`PromptNo`), stacked, and no back.
  - Numeric → a list of its labels (`PromptPick(i)`), back `PromptCancel`.
  - AbChoice → its two labels (`PromptPick(0/1)`), back `PromptCancel`.
  - TextInput → `sheet::keypad` titled with the header, showing the buffer and `max_value`.
- Desktop keeps the strips (kept items).
- The session passes stack names for dismiss, zone names for continents, and chest labels.

- [ ] **Step 1: Write the failing tests**: `mobile_yes_no_is_a_sheet_pair`, `mobile_dismiss_lists_stacks_by_name` (session-level: open Dismiss on touch; the labels read e.g. "Militia x20"; tapping the first equals pressing `1`), `mobile_chest_labels_name_the_amounts`, and `mobile_dwelling_count_is_a_keypad` (tapping MAX then OK buys what typing the cap does). Each asserts every region is ≥ `MIN_BUTTON_H` tall.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: big-button prompts on mobile`.

---

### Task 6: Startup as sheets

**Files:**
- Modify: `shell/src/startup.rs`, `shell/src/slot_picker.rs`, `shell/src/main.rs` (route `SheetPrev` / `SheetNext` to the startup pager)

**Interfaces:**
- Consumes: Task 2.
- On touch:
  - Difficulty is a one-column list ("Easy - 900 days" …, `Difficulty(i)`), back to class select via a new `HitTarget::StartupBack` handled like ESC.
  - The name grid uses 28x26 keys with big letters plus OK and delete (same `NameKey` / `NameDel` / `NameDone`).
  - The slot picker is a paged list (`SlotRow(i)`, back `SlotCancel`).
- Produces: `Startup.pager: sheet::Pager`, and `slot_picker::SlotPicker::sheet(&self, resources) -> Sheet`.

- [ ] **Step 1: Write the failing tests**: `mobile_difficulty_is_a_list_and_tapping_matches_the_row_click`, `mobile_name_keys_are_finger_sized`, and `mobile_slot_picker_pages_through_all_slots`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: big-button startup screens on mobile`.

---

### Task 7: Overworld menus as sheets

**Files:**
- Modify: `shell/src/hud.rs` (action menu), `shell/src/menu.rs`, `shell/src/session.rs` (pager, quit confirm, world map toggle, routing `SheetPrev` / `SheetNext`)

**Interfaces:**
- On touch:
  - The hero action menu is a two-column paged list of the 17 actions (`OverworldAction(a)`, back `OverworldMenuClose`).
  - The Game Menu is a list (`MenuEntry(i)`, back `MenuClose`). Its save/load picker uses Task 6's slot sheet.
  - The quit confirm is a sheet titled "Quit without saving?" with "Yes" (`FastQuitYes`) and "No" (`FastQuitNo`).
  - The world map with the orb draws a top-strip button `Rect(16, 0, 288, 22)`, "Show whole map" / "Show my map" (`WorldmapToggle`).
- Produces: `Session.pager: sheet::Pager`.

- [ ] **Step 1: Write the failing tests**: `mobile_hero_menu_reaches_all_17_actions_like_their_keys` (page through and compare each with its key, as `the_action_menu_dispatches_each_row_like_its_key` does), `mobile_game_menu_is_a_list`, `mobile_quit_confirm_is_a_sheet`, and `mobile_worldmap_toggle_is_a_big_button`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: big-button overworld menus on mobile`.

---

### Task 8: Town, castles, recruiting, gates, spells and controls as sheets

**Files:**
- Modify: `shell/src/views.rs` (town, home castle, own castle, recruit, gate, spells, controls draws and `town_click`), `shell/src/session.rs` (`run_town_row`), `shell/src/interact.rs` (confirm text, prices)

**Interfaces:**
- Each view below, when `layout.touch`, builds a `Sheet` from the same rows and targets it draws on desktop:
  - Town: `TownRow(r)`, labels with prices, back `ViewBack`.
  - Home castle: `HomeCastleRow(0/1)` "Recruit soldiers" / "Speak with the King".
  - Recruit: `RecruitRow(i)` "Archers - 250", then `sheet::keypad` while counting.
  - Own castle: `OwnCastleSlot(i)` plus `OwnCastleToggle`.
  - Gate: `GateRow(i)`.
  - Spellbook: two-column `Spell(i)`.
  - Controls: `ControlsRow(i)` "Music: On".
- Town confirm (spec decision 6 and "Refused purchases"):
  - Produces: `Views.town_confirm: Option<TownRow>`.
  - On touch, Boat (renting, not returning), Spell and Siege first run the engine call on a copy (`let mut probe = *game;`).
  - If the copy's outcome is a refusal, show the refusal toast exactly as today.
  - Otherwise set `town_confirm` and draw a Yes/No sheet: "Rent a boat for N gold?", "Buy <spell> for N gold?", "Buy siege weapons for N gold?".
  - `PromptYes` runs the real `run_town_row`; `PromptNo` / `ViewBack` clears `town_confirm`.
  - Produces: `interact::town_confirm_text(resources: &Resources, game: &Game, row: TownRow, town_key: &str) -> String`.

- [ ] **Step 1: Write the failing tests**: `mobile_town_rows_are_sheet_buttons_with_prices`, `a_town_purchase_asks_first_on_mobile` (gold unchanged until Yes; Yes equals the desktop row click), `a_refused_purchase_skips_the_confirm` (with too little gold, the refusal toast shows at once, there is no confirm, and gold is unchanged), `mobile_recruit_goes_list_then_keypad`, and `mobile_spellbook_and_gates_are_lists`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: big-button towns, castles and spells on mobile`.

---

### Task 9: Combat menus as sheets

**Files:**
- Modify: `shell/src/combat.rs` (`draw_menus` gains `layout: &Layout`; action menu, spell menu, picker cancel), `shell/src/session.rs` (call site)

**Interfaces:**
- On touch:
  - The action menu is a list of its 9 entries (`CombatAction(a)`, back `CombatBack`).
  - The spell menu is a list of 7 (`CombatSpell(i)`, back `CombatBack`).
  - While a target picker is armed, a top-strip button `Rect(16, 0, 288, 22)` "Cancel" publishes `CombatBack`, which `pick_step` treats as cancel, the same as Esc.
- Tapping a non-target still cancels (kept).

- [ ] **Step 1: Write the failing tests**: `mobile_combat_action_menu_is_a_list_like_its_keys` (`assert_same` against each key), `mobile_spell_menu_is_a_list`, and `mobile_picker_cancel_button_cancels_like_escape`.
- [ ] **Step 2:** Run them. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p openbounty-shell`. Expected: PASS.
- [ ] **Step 5:** Commit `feat: big-button combat menus on mobile`.

---

### Task 10: Acceptance on mobile and desktop

**Files:**
- Modify: `shell/tests/touch_only.txt` (rewritten for mobile sheets; run with `--mobile`), `shell/tests/touch_only.rs` (pass `--mobile`; add a min-size audit), `AGENTS.md`, `specs/mobile-ui-design.md` (status)

- [ ] **Step 1:** Add `every_mobile_tap_region_is_finger_sized` to `touch_only.rs`. The playtest records, for each frame, the smallest published region height while running `--mobile`. Assert that every region outside the map tiles, combat cells and sidebar panels is ≥ 26 px tall, and that full-screen backdrops count as large.
- [ ] **Step 2:** Rewrite `touch_only.txt` for mobile: start, name, difficulty, walk, a chest (gold), dismiss by name, the town (contract, confirm a boat), the home castle (recruit with the keypad, speak with the King), combat (menu, cancel a pick, surrender), the Game Menu, and quit No. Only `click` commands.
- [ ] **Step 3:** Run `cargo test -p openbounty-shell --test touch_only`. Expected: PASS.
- [ ] **Step 4: Desktop pixel check** against `ad77d4e`, as in Task 3 Step 5. Expected: only the kept items differ.
- [ ] **Step 5: Devices.** On the Pixel 9a (`adb -s 63051JEBF02543`) and the tablet emulator in both orientations, play the Goals path by finger. Screenshot each sheet and check that no button is under about 7.6 mm.
- [ ] **Step 6:** Update AGENTS.md (`sheet.rs`, the touch flag, the mobile text groups) and the design status.
- [ ] **Step 7:** Commit `test: play the mobile build by finger alone`.
