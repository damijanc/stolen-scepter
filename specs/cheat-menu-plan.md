# Cheat Menu (F10) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port `reference/src/shell_cheats.c` (131 lines), the F10 debug menu:
`G` gold +50000, `V` leadership +100 (current and base), `M` spell power and
max spells +1, `U` +1 of every spell in the first `min(count, 14)` slots, `S`
siege weapons, `F` flight mount, `N` discover the first undiscovered zone, `O`
clear the live fog over the current map, `W` win (cartoon + win screen), `L`
lose screen, ESC/F10 close, any other letter "Unknown cheat: X". Each
non-terminal cheat closes the menu and shows its result under a "Debug" header.

**Architecture:** A new `shell/src/cheats.rs` with two layers, the way
`input.rs` separates the binding table from the backend: a pure
`apply_cheat(letter, &mut Game, &Resources, &Map, &mut Fog) -> CheatOutcome`
that is the whole `switch (letter)`, and a `CheatMenu { active }` whose
`tick(&dyn KeySource, ..., &mut Dialog) -> CheatResult` is the key routing
around it. The endings stay in `main.rs`, which already owns `show_win_game` /
`show_lose_game` and the `Views`. `main.rs` calls `tick` first thing in every
frame, as `main.c:966` does, and skips its input chain on any frame the menu
owns. No engine change: cheats are C host code (`src/`, not `engine/`) that
write `Game` fields directly, and every field they touch is already `pub`.

**Tech Stack:** Rust, the workspace's existing `shell` crate. No new
dependencies. `cargo test -p openbounty-shell`.

## What C does, settled

Every claim below was checked against `reference/` at the time of writing.

### 1. Gating: none. No `--cheats` flag.

- `main.c:964-969` calls `cheat_menu_tick` unconditionally at the top of every
  main-loop frame, before the overlay bookkeeping, the week-end pump, the
  player-IO pumps, fast-quit, demo/autoplay and the whole input chain. There
  is no `#ifdef`, no CLI flag, and the `Makefile` compiles `shell_cheats.c`
  into both debug and release (`CFLAGS_release := -O2 -DNDEBUG` does not touch
  it). `reference/docs/OPENBOUNTY-SPEC.md` REQ-450 says it "is gated behind the
  F10 toggle" -- the toggle IS the gate.
- It is therefore live over the overworld, over any view (castle, town, army,
  worldmap...), over a prompt, over the fast-quit status prompt, and after
  `game_over`.
- The only places it cannot fire are the ones C runs in a SEPARATE blocking
  loop: the startup wizard (`startup.c`), combat (`RunCombat`,
  `combat_loop.c:463`) and the win cartoon (`run_end_cartoon`,
  `end_cartoon.c:138`). This shell runs combat and the cartoon inside its one
  `main.rs` loop (`combat: Option<CombatSession>`, `ViewKind::WinCartoon`), so
  `main.rs` must exclude those two explicitly to match. The startup wizard is
  already a separate loop here too.

**Decision: port it ungated, and do NOT add a `--cheats` flag.** A flag would be
a divergence with no parity benefit: C ships the menu to every player, the
spec calls it a development aid gated only by the key, and this shell is
itself a development build of a parity port. F10 is not bound to anything
else in C or here, so an ungated menu cannot collide with play. If the user
later wants it hidden from players, a flag is a one-line gate at the call
site in Task 4 -- see "Open question" at the end.

### 2. The module boundary: shell, writing public engine fields.

`shell_cheats.c` lives in `src/` and writes `game->stats.*`,
`game->spells.counts[]`, `game->character.mount`,
`game->world.zones_discovered[]` and `fog->seen[][]` directly. It calls no
engine core. The Rust counterparts, all verified `pub`:

| C | Rust | Where |
|---|---|---|
| `stats.gold` | `game.stats.gold: i32` | `engine/src/game.rs:328` |
| `stats.leadership_base` / `_current` | `game.stats.leadership_base` / `leadership_current: i32` | `game.rs:330-331` |
| `stats.spell_power` / `max_spells` | `game.stats.spell_power` / `max_spells: i32` | `game.rs:334-335` |
| `stats.siege_weapons` | `game.stats.siege_weapons: i32` | `game.rs:337` |
| `spells.counts[si]` | `game.spells.counts: [i32; CAT_SPELLS_MAX]` (32) | `game.rs:388` |
| `spells_count()` | `Resources::spells_count() -> usize` | `engine/src/resources.rs:1250` |
| `character.mount = MOUNT_FLY` | `game.character.mount = Mount::Fly` | `game.rs:312`, `game.rs:56` |
| `world.zones_discovered[zi]` | `game.world.zones_discovered: [bool; RES_MAX_ZONES]` | `game.rs:620` |
| `res->zone_count`, `res->zones[i].id` | `Resources::zone_count()`, `zone_by_index(i)` -> `.id` | `resources.rs:1197-1203` |
| `fog->seen[y][x]` | `Fog.seen: [[bool; MAP_MAX_W]; MAP_MAX_H]` | `engine/src/fog.rs:9` |
| `map->width` / `height` | `Map.width` / `height: usize` | `engine/src/map.rs:80-81` |

So no engine method is needed, and adding one would put a debug tool in the
render-free rules crate. `cheats.rs` is shell code, like `interact.rs`.

### 3. Messages and the endings.

- C opens the menu with `player_io_message(game, "Debug menu", CHEAT_MENU_BODY)`
  and each result with `player_io_message(game, "Debug", body)`. This shell has
  no player-IO queue by design (see `AGENTS.md`); a message is
  `Dialog::open(header, body)` (`shell/src/dialog.rs:47`). The menu body is 7
  lines, which is exactly one page (`BODY_LINES = 7`, `dialog.rs:28`, matching
  `DLG_BOTTOM_BODY_LINES` in `overlay.c:28`).
- `W`: `s_cheat_menu_active = false; dialog_dismiss(); run_end_cartoon(...);
  show_win_game(...)`. `show_win_game` (`reference/engine/flows.c:73`) composes
  the end text, opens `VIEW_WIN`, and sets `g->stats.game_over = true`. It does
  **not** set `stats.won` -- only `flow_apply_search` does.
  The Rust `show_win_game` (`shell/src/main.rs:305`) already ports it: it sets
  `game_over`, builds `interact::end_message(.., true)` and calls
  `Views::win_cartoon_open`, which plays `ViewKind::WinCartoon` (or opens
  `ViewKind::Win` directly when the pack ships no ending tiles). Reuse it.
- `L`: `dialog_dismiss(); show_lose_game(...)`. `show_lose_game`
  (`flows.c:44`) opens `VIEW_LOSE` and writes **no state** -- not even
  `game_over`. So after the lose screen is dismissed the game is still live.
  The Rust `show_lose_game` (`main.rs:313`) also writes nothing. Reuse it; do
  not "fix" it to set `game_over`.
- Both terminal cheats return `CHEAT_DISPATCHED_TERMINAL` and the caller
  `continue`s the loop. Here that is: skip the input chain for the frame.

### 4. Keys: F10 is missing.

`shell/src/input.rs`'s `Key` enum has no F10. Task 1 adds `Key::F10` and its
`KeyCode::F10` mapping in `shell/src/backend.rs` (the only macroquad-aware
module). `input::poll` must keep ignoring it.

### 5. The `O` cheat and the fog shapes.

C walks `map->height x map->width` over `fog->seen`, a fixed
`[MAP_MAX_H][MAP_MAX_W]` (64x64, `reference/engine/include/fog.h:13`), and
counts the tiles it flips. The Rust `Fog` is the same fixed 64x64 grid. The
fog C passes is the LIVE one (`main.c`'s `fog`), which is `main.rs`'s
`let mut fog` -- not `game.world.continent_fog[]`, the per-continent snapshot
that `switch_zone` stashes into. A loaded map never exceeds 64x64
(`MapLoadZone` refuses, `map.c:72`), but `Map::new` in a test can, so the
loops clamp to `MAP_MAX_W` / `MAP_MAX_H`.

## Divergences, deliberate

Each is written into `cheats.rs`'s module doc in Task 2.

1. **The menu owns the whole frame.** C drains raylib's `GetKeyPressed`
   queue. `ui_any_key_pressed` (the dialog) reads the same queue, so a key the
   menu takes never reaches the dialog -- but the `IsKeyPressed` handlers
   further down (`input_key_pressed`, e.g. the home-castle ESC at `main.c`'s
   `VIEW_HOME_CASTLE && !dialog_is_active()` branch) still see it. So in C an
   ESC that closes the menu over the home castle also leaves the castle that
   frame. This shell's `KeySource` has no queue, so the only way to "consume"
   a key is to not run the chain: `main.rs` skips it on any frame the menu is
   open or has just handled a key. This also makes the menu immune to a mouse
   click, which in C could dismiss the menu's dialog while
   `s_cheat_menu_active` stayed set.
2. **Two letters in one frame.** C pops one key per frame, in press order, and
   the second key is handled next frame by whatever is then active. With no
   queue, `tick` takes the first pressed letter in A..Z order.
3. **F10 over an open dialog.** C queues the menu text behind it; the next
   menu key's `dialog_dismiss()` then tears down the COVERED dialog, and the
   stale menu text surfaces afterwards. With no message queue the menu simply
   replaces the open dialog.
4. **`wrapping_add`** for the additive cheats, per `AGENTS.md`'s parity rule.
   C's `int` overflow is undefined; a debug Rust build would panic instead.

## Global Constraints

Copied from `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** The user wants approval before every commit.
  Complete a tested task, report it, and ask. Do not chain commits.
- **Commit messages are a single subject line.** No body paragraphs, no bullet
  lists, no `Co-Authored-By` or other attribution trailers. Example:
  `feat: port the cheat menu effects`.
- **Never commit `reference/`, `game-assets/`, or build output.** They are
  gitignored; keep it that way.
- `shell/src/backend.rs` is the ONLY module that may know macroquad exists.
  `cheats.rs` must not import it.
- Do not add anything to `engine/Cargo.toml`, and do not touch `engine/` at
  all in this slice. `engine/tests/render_free.rs` enforces the manifest half.
- Transliterate `shell_cheats.c` first; do not improve it. `F` does not ask
  `player_can_fly`, `M` does not set `knows_magic`, `S` assigns 1 rather than
  adding, `N` reports the zone's `id` not its `name`, `L` does not end the
  game. All of that is C.
- Preserve unrelated working-tree changes. Do not reset, checkout, or clean.
- Run `cargo fmt --all --check` before every commit.

## File Structure

- **Create `shell/src/cheats.rs`** (~600 lines incl. tests). Owns the menu and
  nothing else: the menu text, the letter effects, the key routing, and their
  unit tests.
- **Modify `shell/src/input.rs`**: add `Key::F10` and one test (Task 1).
- **Modify `shell/src/backend.rs`**: map `Key::F10` to `KeyCode::F10` (Task 1).
- **Modify `shell/src/main.rs`**: `pub mod cheats;` (Task 2), then the per-frame
  call and the input-chain gate (Task 4).
- **Modify `specs/play-verification.md`** and **`AGENTS.md`** (Task 5).

## Out of scope -- do not build these

- **A `--cheats` flag.** See settled point 1.
- **A player-IO message queue** to reproduce divergence 3. The shell returns
  outcome values and writes text by design.
- **The post-game "any key quits" branch.** C's main loop has
  `else if (GameIsOver(&game)) { if (ui_any_key_pressed()) quit_requested = true; }`
  (`main.c:1336`); this shell's overworld branch is `else if
  !game.stats.game_over` and simply freezes. That gap predates this slice and
  is reachable without cheats (dig up the scepter). Report it; do not fix it
  here.
- Demo/autoplay: C's cheat tick also runs during `--demo` / `--autoplay`, which
  this shell does not have (`specs/play-verification.md` §13, OUT OF SCOPE).

---

### Task 1: F10 in the key vocabulary

**Files:**
- Modify: `shell/src/input.rs` (the `Key` enum, lines 21-87; the test module)
- Modify: `shell/src/backend.rs:431-432` (`MacroquadKeys::code`)
- Test: `shell/src/input.rs` (inline `#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `input::poll(&dyn KeySource) -> InputState`, `Keys::pressed(&[Key])`,
  `InputState: Default + PartialEq`.
- Produces: `Key::F10`.

- [ ] **Step 1: Write the failing test**

In `shell/src/input.rs`'s test module, insert immediately before
`fn any_pressed_skips_the_control_modifiers`:

```rust
    #[test]
    fn f10_is_not_an_overworld_binding() {
        // The cheat menu reads F10 before the overworld chain runs; `poll`
        // must not turn it into an action or a step.
        assert_eq!(poll_keys(&[Key::F10]), InputState::default());
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p openbounty-shell input:: 2>&1 | head -20`
Expected: FAIL to compile, `no variant or associated item named 'F10' found for
enum 'Key'`.

- [ ] **Step 3: Add the variant and the backend mapping**

In `shell/src/input.rs`, the `Key` enum currently ends:

```rust
    LeftControl,
    RightControl,
}
```

Change it to:

```rust
    LeftControl,
    RightControl,
    /// Opens and closes the debug cheat menu (`crate::cheats`). Not an
    /// overworld binding, so [`poll`] ignores it.
    F10,
}
```

In `shell/src/backend.rs`, `MacroquadKeys::code` is an exhaustive `match`, so
the crate will not build until the new arm exists. After
`Key::RightControl => KeyCode::RightControl,` add:

```rust
            Key::F10 => KeyCode::F10,
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell input::`
Expected: PASS, including `f10_is_not_an_overworld_binding`.

- [ ] **Step 5: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass (shell: 300 unit tests).

- [ ] **Step 6: Ask for approval, then commit**

Report the task and ask before running this:

```bash
git add shell/src/input.rs shell/src/backend.rs
git commit -m "feat: add F10 to the shell key vocabulary"
```

---

### Task 2: The cheat effects (`apply_cheat`)

**Files:**
- Create: `shell/src/cheats.rs`
- Modify: `shell/src/main.rs` (add `pub mod cheats;` to the module list)
- Test: `shell/src/cheats.rs` (inline `#[cfg(test)] mod tests`, the crate's convention)

**Interfaces:**
- Consumes: the fields in settled point 2's table;
  `Resources::from_json_str(&str) -> Result<Resources, _>`;
  `Map::new(usize, usize)`; `Fog::new()`, `Fog::reveal(&Map, i32, i32, i32)`,
  `Fog::seen(i32, i32) -> bool`; `openbounty_engine::fog::{MAP_MAX_W, MAP_MAX_H}`;
  `Dialog::default()`, `Dialog::open(&str, &str)`, `Dialog::page_count() -> usize`;
  `Game: Default + PartialEq`.
- Produces: `cheats::MENU_HEADER`, `cheats::MENU_BODY`, `cheats::RESULT_HEADER`
  (all `&str`); `cheats::CheatOutcome { Message(String), Win, Lose }`;
  `cheats::apply_cheat(char, &mut Game, &Resources, &Map, &mut Fog) -> CheatOutcome`.

- [ ] **Step 1: Register the module**

In `shell/src/main.rs`, add to the module list (alphabetical -- between
`banner` and `chrome`):

```rust
pub mod cheats;
```

- [ ] **Step 2: Write the failing tests**

Create `shell/src/cheats.rs` containing ONLY the test module below. `super` is
empty until Step 4, so the crate will not compile after this step. That is the
point: Step 3 records the failure before Step 4 fixes it.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialog::Dialog;

    /// Three zones and sixteen spells: enough to see `N` walk the zone list
    /// and `U` stop at slot 14 with slots left over.
    fn resources() -> Resources {
        let spells: Vec<String> = (0..16)
            .map(|i| {
                format!(r#"{{"index":{i},"id":"s{i}","name":"S{i}","kind":"combat","cost":1}}"#)
            })
            .collect();
        Resources::from_json_str(&format!(
            r#"{{"troops": [],
                "zones": [{{"id":"home"}}, {{"id":"greenreach"}}, {{"id":"isleward"}}],
                "spells": [{}]}}"#,
            spells.join(",")
        ))
        .expect("fixture parses")
    }

    fn apply(letter: char, game: &mut Game) -> CheatOutcome {
        let mut fog = Fog::new();
        apply_cheat(letter, game, &resources(), &Map::new(4, 4), &mut fog)
    }

    fn message(body: &str) -> CheatOutcome {
        CheatOutcome::Message(body.to_owned())
    }

    #[test]
    fn the_menu_body_is_c_verbatim_and_fits_one_dialog_page() {
        assert_eq!(MENU_BODY.lines().count(), 7);
        assert_eq!(MENU_BODY.lines().next(), Some("G  Gold        N  Zone"));
        assert_eq!(MENU_BODY.lines().last(), Some("F10/ESC  close"));
        let mut dialog = Dialog::default();
        dialog.open(MENU_HEADER, MENU_BODY);
        assert_eq!(dialog.page_count(), 1);
    }

    #[test]
    fn g_adds_fifty_thousand_gold() {
        let mut game = Game::default();
        game.stats.gold = 1000;
        assert_eq!(apply('G', &mut game), message("Gold +50000"));
        assert_eq!(game.stats.gold, 51000);
    }

    #[test]
    fn v_raises_current_and_base_leadership_together() {
        let mut game = Game::default();
        game.stats.leadership_base = 100;
        game.stats.leadership_current = 60;
        assert_eq!(apply('V', &mut game), message("Leadership +100"));
        assert_eq!(game.stats.leadership_base, 200);
        assert_eq!(game.stats.leadership_current, 160);
    }

    #[test]
    fn m_raises_spell_power_and_max_spells_but_not_knows_magic() {
        let mut game = Game::default();
        game.stats.spell_power = 1;
        game.stats.max_spells = 2;
        assert_eq!(apply('M', &mut game), message("Magic boosted"));
        assert_eq!(game.stats.spell_power, 2);
        assert_eq!(game.stats.max_spells, 3);
        assert!(!game.stats.knows_magic, "C leaves knows_magic alone");
    }

    #[test]
    fn u_adds_one_of_every_spell_in_the_first_fourteen_slots_only() {
        let mut game = Game::default();
        game.spells.counts[0] = 2;
        assert_eq!(apply('U', &mut game), message("+1 of every spell"));
        assert_eq!(game.spells.counts[0], 3);
        for si in 1..14 {
            assert_eq!(game.spells.counts[si], 1, "slot {si}");
        }
        // The pack has 16 spells; C's `si < 14` leaves the last two alone.
        assert_eq!(game.spells.counts[14], 0);
        assert_eq!(game.spells.counts[15], 0);
    }

    #[test]
    fn u_stops_at_the_catalog_when_it_has_fewer_than_fourteen_spells() {
        let resources = Resources::from_json_str(
            r#"{"troops": [], "spells": [
                {"index":0,"id":"a","name":"A","kind":"combat","cost":1},
                {"index":1,"id":"b","name":"B","kind":"adventure","cost":1}
            ]}"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        let mut fog = Fog::new();
        apply_cheat('U', &mut game, &resources, &Map::new(1, 1), &mut fog);
        assert_eq!(&game.spells.counts[..3], &[1, 1, 0]);
    }

    #[test]
    fn s_sets_siege_weapons_to_one_rather_than_adding() {
        let mut game = Game::default();
        assert_eq!(apply('S', &mut game), message("Siege weapons granted"));
        assert_eq!(game.stats.siege_weapons, 1);
        apply('S', &mut game);
        assert_eq!(game.stats.siege_weapons, 1);
    }

    #[test]
    fn f_sets_the_fly_mount_without_asking_whether_the_army_can_fly() {
        let mut game = Game::default();
        assert_eq!(game.character.mount, Mount::Ride);
        assert_eq!(apply('F', &mut game), message("Flight granted"));
        assert_eq!(game.character.mount, Mount::Fly);
    }

    #[test]
    fn n_discovers_the_first_undiscovered_zone_and_names_it_by_id() {
        let mut game = Game::default();
        game.world.zones_discovered[0] = true;
        assert_eq!(apply('N', &mut game), message("Revealed: greenreach"));
        assert!(game.world.zones_discovered[1]);
        assert!(!game.world.zones_discovered[2], "one zone per press");

        assert_eq!(apply('N', &mut game), message("Revealed: isleward"));
        assert_eq!(apply('N', &mut game), message("All zones already known"));
        // Bounded by the pack's zone count, not the array's capacity.
        assert!(!game.world.zones_discovered[3]);
    }

    #[test]
    fn o_clears_the_live_fog_over_the_map_and_counts_what_was_hidden() {
        let resources = resources();
        let mut game = Game::default();
        let map = Map::new(10, 6);
        let mut fog = Fog::new();
        fog.reveal(&map, 2, 2, 2); // a 5x5 square already seen

        let outcome = apply_cheat('O', &mut game, &resources, &map, &mut fog);
        assert_eq!(outcome, message("Map fog cleared (35 tiles)"));
        for y in 0..6 {
            for x in 0..10 {
                assert!(fog.seen(x, y), "({x},{y})");
            }
        }
        // Only the map's own rectangle, not the rest of the 64x64 grid.
        assert!(!fog.seen(10, 0));
        assert!(!fog.seen(0, 6));
        // The per-continent snapshot is not the fog C passes.
        assert_eq!(game.world.continent_fog[0], Fog::new());

        let again = apply_cheat('O', &mut game, &resources, &map, &mut fog);
        assert_eq!(again, message("Map fog cleared (0 tiles)"));
    }

    #[test]
    fn w_and_l_are_terminal_and_write_no_state_themselves() {
        let mut game = Game::default();
        assert_eq!(apply('W', &mut game), CheatOutcome::Win);
        assert_eq!(apply('L', &mut game), CheatOutcome::Lose);
        assert_eq!(game, Game::default());
    }

    #[test]
    fn an_unbound_letter_reports_itself_and_changes_nothing() {
        let mut game = Game::default();
        assert_eq!(apply('X', &mut game), message("Unknown cheat: X"));
        // A menu letter is not an overworld binding: A is not "view army".
        assert_eq!(apply('A', &mut game), message("Unknown cheat: A"));
        assert_eq!(game, Game::default());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell cheats:: 2>&1 | head -30`
Expected: FAIL to compile, with `cannot find function 'apply_cheat'` and
`cannot find type 'CheatOutcome'` among the errors (every name the tests reach
for through `super::*` is unresolved, so expect several).

- [ ] **Step 4: Write the implementation**

Prepend to `shell/src/cheats.rs`, above the test module:

```rust
//! The F10 debug cheat menu, porting `src/shell_cheats.c`.
//!
//! C host code, not an engine core: every cheat writes a `Game` (or live
//! `Fog`) field directly, exactly as `cheat_menu_tick` does, so there is no
//! engine method to call and none is added. [`apply_cheat`] is the pure
//! letter-to-effect core; [`CheatMenu::tick`] is the key routing around it.
//! Neither touches a graphics library.
//!
//! Gating, ported as C has it: none. `main.c` calls `cheat_menu_tick` at the
//! top of every main-loop frame, in release and debug builds alike, with no
//! flag. The only places it cannot fire are the ones C runs in a SEPARATE
//! loop -- the startup wizard, combat (`RunCombat`) and the blocking win
//! cartoon (`run_end_cartoon`) -- and `main.rs` excludes the last two
//! explicitly, because this shell runs them inside its one loop.
//!
//! DIVERGENCE, deliberate: C drains raylib's `GetKeyPressed` queue, which
//! `ui_any_key_pressed` (the dialog) shares but which the `IsKeyPressed`
//! handlers further down the frame do not, so an ESC that closes the menu
//! over, say, the home castle also leaves the castle that same frame. This
//! shell's `KeySource` has no queue, so `main.rs` skips the whole input
//! chain on any frame the menu is open or has just handled a key. C pops one
//! key per frame in press order; with no queue, a frame holding two letters
//! resolves to the first in A..Z order.
//!
//! DIVERGENCE, deliberate: C queues the menu text with `player_io_message`,
//! so F10 over an open dialog waits behind it (and the next menu key's
//! `dialog_dismiss` tears down the covered dialog, not the menu). This shell
//! has no message queue by design, so the menu replaces the open dialog.

use openbounty_engine::fog::{Fog, MAP_MAX_H, MAP_MAX_W};
use openbounty_engine::game::{Game, Mount};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;

/// The menu dialog's header, C's `player_io_message(game, "Debug menu", ..)`.
pub const MENU_HEADER: &str = "Debug menu";

/// C's `CHEAT_MENU_BODY`, verbatim. Seven lines: exactly one dialog page.
pub const MENU_BODY: &str = "G  Gold        N  Zone\n\
                             V  Leadership  O  Fog\n\
                             M  Magic       W  Win\n\
                             U  Spells      L  Lose\n\
                             S  Siege\n\
                             F  Flight\n\
                             F10/ESC  close";

/// The header every non-terminal cheat's result is shown under.
pub const RESULT_HEADER: &str = "Debug";

/// `U` touches at most this many spell slots: C's hard-coded `si < 14`, the
/// stock 7 combat + 7 adventure. The same bound `Game::discard_spell` uses.
const SPELL_SLOTS: usize = 14;

/// What one letter did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheatOutcome {
    /// State was changed (or the letter was unknown); show this body under
    /// [`RESULT_HEADER`].
    Message(String),
    /// `W`: the caller plays the win ending. Nothing is written here --
    /// `show_win_game` sets `game_over`, as C's does.
    Win,
    /// `L`: the caller opens the lose screen. C's `show_lose_game` writes no
    /// state at all, so the game is still live once the screen is dismissed.
    Lose,
}

/// Apply one cheat letter, porting the `switch (letter)` in
/// `cheat_menu_tick`. `fog` is the LIVE fog, the one `main.c` passes; the
/// per-continent snapshots in `game.world.continent_fog` are not touched.
pub fn apply_cheat(
    letter: char,
    game: &mut Game,
    resources: &Resources,
    map: &Map,
    fog: &mut Fog,
) -> CheatOutcome {
    let body = match letter {
        'G' => {
            game.stats.gold = game.stats.gold.wrapping_add(50000);
            "Gold +50000".to_owned()
        }
        'V' => {
            game.stats.leadership_current = game.stats.leadership_current.wrapping_add(100);
            game.stats.leadership_base = game.stats.leadership_base.wrapping_add(100);
            "Leadership +100".to_owned()
        }
        'M' => {
            game.stats.spell_power = game.stats.spell_power.wrapping_add(1);
            game.stats.max_spells = game.stats.max_spells.wrapping_add(1);
            "Magic boosted".to_owned()
        }
        'U' => {
            let n = resources.spells_count();
            let mut si = 0;
            while si < n && si < SPELL_SLOTS {
                game.spells.counts[si] = game.spells.counts[si].wrapping_add(1);
                si += 1;
            }
            "+1 of every spell".to_owned()
        }
        'S' => {
            game.stats.siege_weapons = 1;
            "Siege weapons granted".to_owned()
        }
        'F' => {
            // Straight to the field, as C: no `player_can_fly` check, and the
            // travel mode is left alone.
            game.character.mount = Mount::Fly;
            "Flight granted".to_owned()
        }
        'N' => {
            let mut found = None;
            for zi in 0..resources.zone_count() {
                if !game.world.zones_discovered[zi] {
                    game.world.zones_discovered[zi] = true;
                    found = Some(zi);
                    break;
                }
            }
            match found.and_then(|zi| resources.zone_by_index(zi)) {
                Some(zone) => format!("Revealed: {}", zone.id.as_str()),
                None => "All zones already known".to_owned(),
            }
        }
        'O' => {
            // C walks map->height x map->width over the fixed 64x64 grid; a
            // loaded map never exceeds it (`MapLoadZone` refuses), so the
            // clamp only guards a hand-built test map.
            let mut revealed = 0;
            for y in 0..map.height.min(MAP_MAX_H) {
                for x in 0..map.width.min(MAP_MAX_W) {
                    if !fog.seen[y][x] {
                        fog.seen[y][x] = true;
                        revealed += 1;
                    }
                }
            }
            format!("Map fog cleared ({revealed} tiles)")
        }
        'W' => return CheatOutcome::Win,
        'L' => return CheatOutcome::Lose,
        other => format!("Unknown cheat: {other}"),
    };
    CheatOutcome::Message(body)
}
```

(The `[`CheatMenu::tick`]` doc link resolves once Task 3 lands; `cargo test`
does not run rustdoc lints, so it does not warn in between.)

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell cheats::`
Expected: PASS, 12 tests.

- [ ] **Step 6: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass (shell: 312 unit tests). A plain
`cargo build -p openbounty-shell` is warning-free at this point: the unused
items are `pub` in a `pub mod`, which dead-code analysis does not flag.

The fixtures in Step 2 have been verified to parse against the real
`Resources` loader and every test in this task has been run green against this
exact implementation. If a fixture fails to parse, suspect a loader change
rather than the JSON.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/src/cheats.rs shell/src/main.rs
git commit -m "feat: port the cheat menu effects"
```

---

### Task 3: The menu's key routing (`CheatMenu::tick`)

**Files:**
- Modify: `shell/src/cheats.rs`
- Test: `shell/src/cheats.rs`

**Interfaces:**
- Consumes: `apply_cheat`, `CheatOutcome`, `MENU_HEADER`, `MENU_BODY`,
  `RESULT_HEADER` from Task 2; `Key::F10` from Task 1; `KeySource::pressed`;
  `input::Keys::pressed(&[Key])`; `Dialog::{open, dismiss, is_active,
  header_text, body_text}`; `views::{Views, ViewKind}`, `Views::new()`,
  `Views::active() -> Option<ViewKind>`; the crate-root private fns
  `crate::show_win_game(&mut Views, &Resources, &mut Game)` and
  `crate::show_lose_game(&mut Views, &Resources, &Game)` (`main.rs:305`,
  `main.rs:313`) -- a child module may call its ancestors' private items, so no
  visibility change is needed.
- Produces: `cheats::CheatResult { Idle, Opened, Dismissed, Dispatched,
  Terminal(CheatOutcome) }`; `cheats::CheatMenu` (`Default`) with
  `is_active(&self) -> bool` and
  `tick(&mut self, &dyn KeySource, &mut Game, &Resources, &Map, &mut Fog, &mut Dialog) -> CheatResult`.

- [ ] **Step 1: Write the failing tests**

In the test module of `shell/src/cheats.rs`, replace the line

```rust
    use crate::dialog::Dialog;
```

with

```rust
    use crate::input::Keys;
    use crate::views::{ViewKind, Views};
```

(`Dialog` now arrives through `use super::*`, from the import Step 3 adds.)
Then append, inside the test module after
`an_unbound_letter_reports_itself_and_changes_nothing`:

```rust
    struct Frame {
        menu: CheatMenu,
        game: Game,
        resources: Resources,
        map: Map,
        fog: Fog,
        dialog: Dialog,
    }

    impl Frame {
        fn new() -> Self {
            Self {
                menu: CheatMenu::default(),
                game: Game::default(),
                resources: resources(),
                map: Map::new(4, 4),
                fog: Fog::new(),
                dialog: Dialog::default(),
            }
        }

        fn press(&mut self, keys: &[Key]) -> CheatResult {
            self.menu.tick(
                &Keys::pressed(keys),
                &mut self.game,
                &self.resources,
                &self.map,
                &mut self.fog,
                &mut self.dialog,
            )
        }
    }

    #[test]
    fn the_menu_is_closed_until_f10_and_letters_do_nothing_meanwhile() {
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::G]), CheatResult::Idle);
        assert_eq!(frame.game.stats.gold, 0);
        assert!(!frame.menu.is_active());
        assert!(!frame.dialog.is_active());
    }

    #[test]
    fn f10_opens_the_menu_dialog() {
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::F10]), CheatResult::Opened);
        assert!(frame.menu.is_active());
        assert!(frame.dialog.is_active());
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
        assert_eq!(frame.dialog.body_text(), MENU_BODY);
    }

    #[test]
    fn the_opening_press_does_not_also_close_the_menu() {
        // C drains the key queue after opening so the same F10 is not read
        // again as "close"; here the open path returns before the close check.
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::F10, Key::G]), CheatResult::Opened);
        assert!(frame.menu.is_active());
        assert_eq!(
            frame.game.stats.gold, 0,
            "the G in the opening frame is drained"
        );
    }

    #[test]
    fn escape_or_a_second_f10_closes_the_menu() {
        for close in [Key::Escape, Key::F10] {
            let mut frame = Frame::new();
            frame.press(&[Key::F10]);
            assert_eq!(frame.press(&[close]), CheatResult::Dismissed, "{close:?}");
            assert!(!frame.menu.is_active());
            assert!(!frame.dialog.is_active());
        }
    }

    #[test]
    fn a_non_letter_leaves_the_menu_open() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::Up]), CheatResult::Idle);
        assert_eq!(frame.press(&[Key::Digit1]), CheatResult::Idle);
        assert_eq!(frame.press(&[]), CheatResult::Idle);
        assert!(frame.menu.is_active());
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
    }

    #[test]
    fn a_letter_applies_closes_the_menu_and_shows_the_result() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::G]), CheatResult::Dispatched);
        assert_eq!(frame.game.stats.gold, 50000);
        assert!(!frame.menu.is_active(), "one cheat per F10");
        assert!(frame.dialog.is_active());
        assert_eq!(frame.dialog.header_text(), RESULT_HEADER);
        assert_eq!(frame.dialog.body_text(), "Gold +50000");

        // Closed again, so a second G is idle.
        assert_eq!(frame.press(&[Key::G]), CheatResult::Idle);
        assert_eq!(frame.game.stats.gold, 50000);
    }

    #[test]
    fn an_unknown_letter_still_closes_the_menu() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::Q]), CheatResult::Dispatched);
        assert!(!frame.menu.is_active());
        assert_eq!(frame.dialog.body_text(), "Unknown cheat: Q");
    }

    #[test]
    fn two_letters_in_one_frame_resolve_in_alphabetical_order() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        frame.press(&[Key::V, Key::G]);
        assert_eq!(frame.game.stats.gold, 50000);
        assert_eq!(frame.game.stats.leadership_base, 0);
    }

    #[test]
    fn f10_over_an_open_dialog_replaces_it_with_the_menu() {
        let mut frame = Frame::new();
        frame.dialog.open("Sign", "Welcome");
        frame.press(&[Key::F10]);
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
    }

    #[test]
    fn w_closes_the_menu_and_hands_the_win_to_the_caller() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(
            frame.press(&[Key::W]),
            CheatResult::Terminal(CheatOutcome::Win)
        );
        assert!(!frame.menu.is_active());
        assert!(
            !frame.dialog.is_active(),
            "no Debug message after an ending"
        );
        assert!(
            !frame.game.stats.game_over,
            "the caller's show_win_game sets it"
        );

        // What main.rs then does with it. The fixture ships no ending tiles,
        // so the cartoon is skipped and the win screen opens directly.
        let mut views = Views::new();
        crate::show_win_game(&mut views, &frame.resources, &mut frame.game);
        assert!(frame.game.stats.game_over);
        assert!(!frame.game.stats.won, "C's show_win_game never sets won");
        assert_eq!(views.active(), Some(ViewKind::Win));
    }

    #[test]
    fn l_opens_the_lose_screen_without_ending_the_game() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(
            frame.press(&[Key::L]),
            CheatResult::Terminal(CheatOutcome::Lose)
        );
        assert!(!frame.dialog.is_active());

        let mut views = Views::new();
        crate::show_lose_game(&mut views, &frame.resources, &frame.game);
        assert_eq!(views.active(), Some(ViewKind::Lose));
        // C's show_lose_game writes nothing, so dismissing the screen returns
        // to a live board.
        assert!(!frame.game.stats.game_over);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell cheats:: 2>&1 | head -30`
Expected: FAIL to compile, with `cannot find type 'CheatMenu'`,
`cannot find type 'CheatResult'` and `cannot find type 'Dialog'` among the
errors.

- [ ] **Step 3: Write the implementation**

In `shell/src/cheats.rs`, below the four `use openbounty_engine::...` lines,
add:

```rust

use crate::dialog::Dialog;
use crate::input::{Key, KeySource};
```

Below `const SPELL_SLOTS: usize = 14;`, add:

```rust

/// The letter keys in A..Z order, which is also the tie-break when two land
/// in one frame.
const LETTERS: [(Key, char); 26] = [
    (Key::A, 'A'),
    (Key::B, 'B'),
    (Key::C, 'C'),
    (Key::D, 'D'),
    (Key::E, 'E'),
    (Key::F, 'F'),
    (Key::G, 'G'),
    (Key::H, 'H'),
    (Key::I, 'I'),
    (Key::J, 'J'),
    (Key::K, 'K'),
    (Key::L, 'L'),
    (Key::M, 'M'),
    (Key::N, 'N'),
    (Key::O, 'O'),
    (Key::P, 'P'),
    (Key::Q, 'Q'),
    (Key::R, 'R'),
    (Key::S, 'S'),
    (Key::T, 'T'),
    (Key::U, 'U'),
    (Key::V, 'V'),
    (Key::W, 'W'),
    (Key::X, 'X'),
    (Key::Y, 'Y'),
    (Key::Z, 'Z'),
];
```

After the closing `}` of `apply_cheat`, before `#[cfg(test)]`, add:

```rust

/// What a frame of the menu did, porting `CheatResult`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheatResult {
    /// Nothing consumed: the menu is closed and F10 was not pressed, or the
    /// menu is open and no key it reads went down.
    Idle,
    /// F10 opened the menu.
    Opened,
    /// ESC or F10 closed it.
    Dismissed,
    /// A non-terminal letter was applied and its result is in the dialog.
    Dispatched,
    /// `W` or `L`: the caller opens the ending. C's `CHEAT_DISPATCHED_TERMINAL`.
    Terminal(CheatOutcome),
}

/// The menu's one piece of state, C's `s_cheat_menu_active`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CheatMenu {
    active: bool,
}

impl CheatMenu {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// One frame, porting `cheat_menu_tick`. The menu and its results are
    /// shown in `dialog`; the endings are left to the caller, which owns the
    /// views.
    pub fn tick(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
        map: &Map,
        fog: &mut Fog,
        dialog: &mut Dialog,
    ) -> CheatResult {
        if !self.active {
            if keys.pressed(Key::F10) {
                self.active = true;
                dialog.open(MENU_HEADER, MENU_BODY);
                return CheatResult::Opened;
            }
            return CheatResult::Idle;
        }

        if keys.pressed(Key::Escape) || keys.pressed(Key::F10) {
            self.active = false;
            dialog.dismiss();
            return CheatResult::Dismissed;
        }
        let Some(letter) = LETTERS
            .iter()
            .find(|(key, _)| keys.pressed(*key))
            .map(|(_, letter)| *letter)
        else {
            return CheatResult::Idle;
        };

        self.active = false;
        dialog.dismiss();
        match apply_cheat(letter, game, resources, map, fog) {
            CheatOutcome::Message(body) => {
                dialog.open(RESULT_HEADER, &body);
                CheatResult::Dispatched
            }
            ending => CheatResult::Terminal(ending),
        }
    }
}
```

Order note, from C: the open branch is tested first and returns, so a frame
that opens the menu never also reads a letter or the close key (C gets the same
effect by draining the queue after opening). ESC/F10 are tested before letters,
as C tests `KEY_ESCAPE || KEY_F10` before `KEY_A..KEY_Z`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell cheats::`
Expected: PASS, 23 tests.

- [ ] **Step 5: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass (shell: 323 unit tests).

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/cheats.rs
git commit -m "feat: route the F10 cheat menu keys"
```

---

### Task 4: Run the menu every frame

**Files:**
- Modify: `shell/src/main.rs` (imports near line 35; the `let mut combat`
  binding at line 803; the top of `loop {` at line 805; the input chain's
  first `if let Some(session) = &mut combat {` at line 846)

**Interfaces:**
- Consumes: `CheatMenu`, `CheatResult`, `CheatOutcome` from Tasks 2-3. In
  `main`'s scope: `keys: MacroquadKeys` (line 683), `game`, `map`, `fog`
  (line 712), `resources`, `dialog` (line 780), `views` (line 800),
  `combat: Option<CombatSession>` (line 803), and the free fns
  `show_win_game` / `show_lose_game`.
- Produces: nothing further.

**Placement.** `main.c:966` runs the cheat tick FIRST in the frame, before
`pump_week_end_dialog`. In `main.rs` the week-end pump is the first statement
inside `loop {`, so the tick goes above it. The input chain is one
`if / else if` ladder starting at `if let Some(session) = &mut combat {`;
prefixing it with an empty `if cheat_owns_frame {}` arm skips every input
branch -- dialog, prompt, views, fast-quit and overworld -- while leaving the
pumps, the cartoon clock and the render untouched.

`ViewKind` is already imported in `main.rs` (it is used at line 818).

- [ ] **Step 1: Add the import**

In `shell/src/main.rs`, directly above `use chrome::{Chrome, StatusMode};`:

```rust
use cheats::{CheatMenu, CheatOutcome, CheatResult};
```

- [ ] **Step 2: Add the state and the per-frame call**

Replace:

```rust
    let mut combat: Option<CombatSession> = None;

    loop {
```

with:

```rust
    let mut combat: Option<CombatSession> = None;
    // The F10 debug menu, C's `s_cheat_menu_active`.
    let mut cheat_menu = CheatMenu::default();

    loop {
        // F10 -> debug cheat menu, first thing in the frame as in `main.c`.
        // C cannot reach it during combat or the win cartoon because both run
        // their own loops there; this shell runs them in this one, so they are
        // excluded here instead.
        let cheat = if combat.is_none() && views.active() != Some(ViewKind::WinCartoon) {
            cheat_menu.tick(&keys, &mut game, &resources, &map, &mut fog, &mut dialog)
        } else {
            CheatResult::Idle
        };
        match &cheat {
            CheatResult::Terminal(CheatOutcome::Win) => {
                show_win_game(&mut views, &resources, &mut game)
            }
            CheatResult::Terminal(CheatOutcome::Lose) => {
                show_lose_game(&mut views, &resources, &game)
            }
            _ => {}
        }
        // The menu owns the keys while it is open and on the frame it handled
        // one; see the DIVERGENCE note in `cheats.rs`.
        let cheat_owns_frame = cheat != CheatResult::Idle || cheat_menu.is_active();

```

- [ ] **Step 3: Gate the input chain**

Replace:

```rust
        if let Some(session) = &mut combat {
            if views.is_active() {
```

with:

```rust
        if cheat_owns_frame {
            // Nothing else reads input this frame.
        } else if let Some(session) = &mut combat {
            if views.is_active() {
```

- [ ] **Step 4: Verify it builds and the suite is green**

Run: `cargo fmt --all --check && cargo build -p openbounty-shell && cargo test --workspace`
Expected: no formatting diff; the build has no warnings; all tests pass
(shell 323, engine 350, combat digests 25, render-free 1). This exact edit has
been built and run green; `cargo fmt` leaves it as written above.

- [ ] **Step 5: Verify it in play**

Run:

```bash
cargo run -p openbounty-shell -- --seed 1 reference/assets/kings-bounty/game.json
```

Walk through the startup wizard to the overworld, then:

1. Press F10. The bottom panel shows "Debug menu" and the seven-line body.
   Arrow keys do nothing and the hero does not move. ESC closes it; F10 again
   reopens it, and a second F10 closes it.
2. F10, `G`: a "Debug" dialog says "Gold +50000". Dismiss it, press `V`: the
   Character view's gold is 50000 higher than before.
3. F10, `V`, then `V` (Character view): leadership has risen by 100.
4. F10, `U`, then `U` (spellbook): every listed spell shows at least 1.
5. F10, `S`: "Siege weapons granted". F10, `F`: "Flight granted" -- the hero
   is now flying even with a ground army (C's behaviour; `L` lands normally
   on grass).
6. F10, `N`: "Revealed: <zone id>" (the zone's `id`, not its display name).
   Repeat until "All zones already known". To see the effect, board a boat
   (rent one in a town, then sail) and press `N` (New Continent): every
   revealed continent that neighbours the current one is now listed --
   `interact::navigate_targets` filters on both `zones_discovered` and the
   zone's `neighbors`.
7. F10, `O`: "Map fog cleared (<n> tiles)". Press `M` (view map): the whole
   continent is visible. A second F10, `O` reports 0 tiles.
8. F10, `X`: "Unknown cheat: X", and the menu is closed.
9. F10, `L`: the lose screen opens. Dismiss it: the overworld is live and the
   hero can still walk (C's `show_lose_game` does not end the game).
10. F10, `W`: the win cartoon plays (any key skips it), then the win screen.
    After dismissing it the board is frozen -- that freeze is the
    pre-existing gap listed under "Out of scope", not this slice.
11. Start a fight with a wandering army and press F10 during combat: nothing
    happens. That matches C, where `RunCombat` owns its own loop.
12. Enter the home castle, press F10 then ESC: the menu closes and the castle
    view stays up (divergence 1; in C that ESC also leaves the castle).

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/main.rs
git commit -m "feat: open the cheat menu from the main loop"
```

---

### Task 5: Record it in the docs

**Files:**
- Modify: `specs/play-verification.md` (the "Last refreshed" line; "How to
  run"; the §13 table)
- Modify: `AGENTS.md` (the `shell/` layout bullet; the "Current Engine State"
  shell bullets; the test-count sentence)

- [ ] **Step 1: Close the §13 row**

In `specs/play-verification.md` §13, delete this row outright (the doc's
Maintenance section and `specs/sidebar-hud-plan.md` both close a GAP by
deleting its row):

```markdown
| Cheat menu (F10) | GAP | `reference/src/shell_cheats.c` unported. Only affects how fast the scenarios in this document can be set up |
```

Then grep the file for any other mention, as its Maintenance section asks:

Run: `grep -n -i "cheat\|F10" specs/play-verification.md`
Expected: only the lines Step 2 adds.

- [ ] **Step 2: Mention F10 under "How to run"**

In `specs/play-verification.md`, directly after the paragraph that ends
"The shell resolves art and map paths relative to it.", add:

```markdown
To set a scenario up quickly, press **F10** on the overworld for the debug
menu (`shell/src/cheats.rs`, porting `shell_cheats.c` [C]): `G` +50000 gold,
`V` +100 leadership, `M` +1 spell power and max spells, `U` +1 of every spell,
`S` siege weapons, `F` flight, `N` discover the next continent, `O` clear this
map's fog, `W` win, `L` lose screen; ESC or F10 closes it. It is ungated, as in
C, and unavailable during combat and the win cartoon, as in C. Note that `L`
only shows the lose screen -- C's `show_lose_game` does not end the game -- and
`W` sets `game_over` but not `won`. [PORT]: while the menu is open it owns
every key, where C lets the key also reach a view's own `IsKeyPressed` handler.
```

- [ ] **Step 3: Move the "Last refreshed" line**

In `specs/play-verification.md`, the line currently reads
"Last refreshed 2026-09-22, against `06db5cb`." Replace the date with the
commit date of this task and the hash with the output of
`git rev-parse --short HEAD` taken just before this task's commit (the tip
that carries Task 4). Leave the rest of that paragraph as is.

- [ ] **Step 4: Update `AGENTS.md`**

In the `shell/` bullet of the Layout section, replace

```markdown
  `savepath.rs` the save-slot locations, and `input.rs` the key bindings.
```

with

```markdown
  `savepath.rs` the save-slot locations, `cheats.rs` the F10 debug menu, and
  `input.rs` the key bindings.
```

In "Current Engine State", after the bullet that begins
"- The end-of-week screens (`shell/src/main.rs` + ...", add:

```markdown
- The F10 debug menu (`shell/src/cheats.rs`, porting `shell_cheats.c`):
  `apply_cheat` is the letter switch and `CheatMenu::tick` the key routing,
  called first in every `main.rs` frame. It writes public `Game` fields and
  the LIVE `Fog` directly, as C does; no engine method exists for it and none
  should. Ungated, as in C -- there is no `--cheats` flag -- but excluded
  during combat and the win cartoon, which C runs in their own loops. `W`
  reuses `show_win_game` (sets `game_over`, never `won`); `L` reuses
  `show_lose_game`, which writes nothing, so the game continues. DIVERGENCE:
  with no key queue, the menu skips the whole input chain on any frame it
  owns, where C's `GetKeyPressed` drain leaves `IsKeyPressed` handlers able to
  see the same key.
```

Replace the test-count sentence

```markdown
Test coverage is 348 engine unit tests, 265 shell unit tests, 25 combat digest
goldens, and one render-free boundary test.
```

with the real numbers from `cargo test --workspace`. At the time of writing
they are 350 engine (the 348 in `AGENTS.md` was already stale) and 323 shell
(299 before this slice, +1 in Task 1, +12 in Task 2, +11 in Task 3):

```markdown
Test coverage is 350 engine unit tests, 323 shell unit tests, 25 combat digest
goldens, and one render-free boundary test.
```

If the counts differ when you run it, write the numbers the run reports, not
these.

- [ ] **Step 5: Check the suite once more**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add specs/play-verification.md AGENTS.md
git commit -m "docs: record the ported cheat menu"
```

---

## Open question for the user

**Should the menu be hidden behind a flag?** This plan ports C exactly: F10
works for anyone, always. That is the right parity answer and costs nothing
today. If you would rather a release build not expose it, the change is a
`--cheats` CLI flag parsed beside `--seed` in `main.rs` and one extra
`&& cheats_enabled` in Task 4's call-site condition, recorded as a
DIVERGENCE. Recommendation: do not add it now; revisit when this shell is
packaged for players.
