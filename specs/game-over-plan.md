# Game Over Exit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Once the game is over and nothing modal is up, any key ends the
session, as C does. Today the shell freezes on a dead board.

**Architecture:** Two small changes. The engine gains `Game::is_over`, a port
of `GameIsOver`. That function was a MISS in the engine sweep and has no Rust
counterpart; the shell tests `stats.game_over` directly, which drops C's
`days_left == 0` half. Then the shell's adventure-input `else if` chain gains
C's game-over branch, in C's position.

**Tech Stack:** Rust, the existing `engine` and `shell` crates. No new dependencies.

## What C does, settled

- `reference/engine/game.c:1152`:
  `GameIsOver` returns `g->stats.game_over || g->stats.days_left == 0`.
- `reference/src/main.c:1336-1338`: in the main loop's input chain, AFTER the
  view, dialog and prompt branches and BEFORE the adventure bindings:
  `else if (GameIsOver(&game)) { if (ui_any_key_pressed()) quit_requested = true; }`.
  So a key that dismisses the Win or Lose view is consumed by the view branch;
  only the NEXT key, on the bare board, quits.
- `quit_requested` ends the main loop, and the process exits. The Rust
  counterpart is `return` from the frame loop, which is what the `fast_quit`
  branch's `Y` already does (`shell/src/main.rs`, search `if keys.pressed(Key::Y)`).
- `main.c:1262-1272` clears `game_over` after the Win view for `won_handoff`.
  That is autoplay tooling, which is OUT OF SCOPE. Do not port it.

## The Rust today

`shell/src/main.rs` (search `} else if !game.stats.game_over {`): the adventure
bindings are guarded on `!game.stats.game_over` and the chain has no `else`,
so a finished game accepts no input at all. `step::end_day` sets `game_over`
when `days_left` reaches 0 (`engine/src/step.rs`, search
`if game.stats.days_left == 0`), so in live play the two halves of
`GameIsOver` coincide. They can come apart on a loaded save or in a test,
which is why the engine port is exact rather than a field read.

## Global Constraints

From `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** Complete a tested task, report it, ask. Do not chain commits.
- **Commit messages are a single subject line.** No body, no bullets, no
  `Co-Authored-By` or other trailers. Example: `feat: port GameIsOver`.
- **Never add a dependency to `engine/Cargo.toml`.**
- **`backend.rs` is the only module that may name macroquad.**
- Run `cargo fmt --all` before each commit; `cargo fmt --all --check` must be clean.
- Preserve unrelated working-tree changes.

---

### Task 1: Port `GameIsOver`

**Files:**
- Modify: `engine/src/game.rs`, adding `is_over` beside `compute_score`
  (search `pub fn compute_score`) and two tests in the `tests` module at the
  bottom of the file.

**Interfaces:**
- Produces: `impl Game { pub fn is_over(&self) -> bool }`. Task 2 calls it.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block at the bottom of `engine/src/game.rs`:

```rust
    #[test]
    fn is_over_on_the_flag_alone() {
        let mut game = Game::default();
        game.stats.days_left = 5;
        assert!(!game.is_over());
        game.stats.game_over = true;
        assert!(game.is_over());
    }

    /// `GameIsOver` ORs in `days_left == 0`, so a game whose calendar has run
    /// out is over even when nothing has set the flag (a loaded save, say).
    #[test]
    fn is_over_when_the_days_run_out_without_the_flag() {
        let mut game = Game::default();
        game.stats.days_left = 0;
        assert!(!game.stats.game_over);
        assert!(game.is_over());
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-engine --lib is_over`
Expected: FAIL to compile, "no method named `is_over` found for struct `Game`".

- [ ] **Step 3: Write the implementation**

Directly above `pub fn compute_score` in `impl Game`:

```rust
    /// Ports `GameIsOver`: the flag, or a calendar that has run out.
    /// `step::end_day` sets the flag at zero days, so in live play the two
    /// agree; they can differ on a loaded save.
    pub fn is_over(&self) -> bool {
        self.stats.game_over || self.stats.days_left == 0
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-engine --lib is_over`
Expected: PASS, 2 tests.

- [ ] **Step 5: Run the focused suites**

```sh
cargo fmt --all --check
cargo test -p openbounty-engine --lib
cargo test -p openbounty-engine --test combat_digests
cargo test -p openbounty-engine --test render_free
```

Expected: all pass.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add engine/src/game.rs
git commit -m "feat: port GameIsOver"
```

---

### Task 2: Any key quits a finished game

**Files:**
- Modify: `shell/src/main.rs` (the adventure-input chain; a new
  `game_over_quits` helper and a `#[cfg(test)] mod tests` at the end of the file)
- Modify: `specs/play-verification.md`, `AGENTS.md`

**Interfaces:**
- Consumes: `Game::is_over(&self) -> bool` from Task 1.
- Produces: `fn game_over_quits(game: &Game, keys: &dyn KeySource) -> bool`
  (private to `main.rs`).

- [ ] **Step 1: Write the failing tests**

`main.rs` has no test module today. Append one at the very end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::game_over_quits;
    use crate::input::{Key, Keys};
    use openbounty_engine::game::Game;

    fn finished() -> Game {
        let mut game = Game::default();
        game.stats.days_left = 3;
        game.stats.game_over = true;
        game
    }

    #[test]
    fn a_finished_game_quits_on_any_key() {
        assert!(game_over_quits(&finished(), &Keys::pressed(&[Key::A])));
        assert!(game_over_quits(&finished(), &Keys::pressed(&[Key::Enter])));
    }

    #[test]
    fn a_finished_game_waits_for_a_key() {
        assert!(!game_over_quits(&finished(), &Keys::default()));
    }

    /// `ui_any_key_pressed` skips modifiers; `any_pressed` already does.
    #[test]
    fn a_modifier_alone_does_not_quit() {
        assert!(!game_over_quits(&finished(), &Keys::pressed(&[Key::LeftControl])));
    }

    #[test]
    fn a_live_game_never_quits_here() {
        let mut game = Game::default();
        game.stats.days_left = 3;
        assert!(!game_over_quits(&game, &Keys::pressed(&[Key::A])));
    }

    /// The `days_left == 0` half of `GameIsOver` quits too.
    #[test]
    fn an_empty_calendar_quits_without_the_flag() {
        let mut game = Game::default();
        game.stats.days_left = 0;
        assert!(game_over_quits(&game, &Keys::pressed(&[Key::A])));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell game_over_quits`
Expected: FAIL to compile, "cannot find function `game_over_quits`".

- [ ] **Step 3: Write the helper**

Directly below `fn show_lose_game` in `shell/src/main.rs`:

```rust
/// Ports `main.c`'s game-over branch: with nothing modal up, a finished game
/// ends the session on any key. It sits after the view branch in the input
/// chain, so the key that dismisses the Win or Lose screen is not also the
/// key that quits.
fn game_over_quits(game: &Game, keys: &dyn KeySource) -> bool {
    game.is_over() && keys.any_pressed()
}
```

If `Game` is not already imported by name in `main.rs`, add it to the
existing `use openbounty_engine::game::{ ... }` list.

- [ ] **Step 4: Wire it into the input chain**

In the frame loop, find:

```rust
        } else if !game.stats.game_over {
            let input = input::poll(&keys);
```

Replace the guard so the chain gains C's branch in C's position:

```rust
        } else if game.is_over() {
            if game_over_quits(&game, &keys) {
                return;
            }
        } else {
            let input = input::poll(&keys);
```

The body of the old `!game.stats.game_over` block is unchanged, and its
closing brace now closes the new `else`. Leave the other `game.stats.game_over`
reads alone (the ones that call `show_lose_game` after a step, a rest or `W`).
Those TRIGGER the lose screen; they are not the input guard.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell game_over_quits`
Expected: PASS, 5 tests.

- [ ] **Step 6: Check it in play**

```bash
cargo run -p openbounty-shell -- --seed 1 reference/assets/kings-bounty/game.json
```

Start an Easy game and press `W` repeatedly until the days run out. Expect:
the lose screen appears; one key dismisses it and leaves the board visible;
the NEXT key closes the window. If the cheat menu has landed, `F10` then `W`
reaches the win screen faster; expect the same two-key sequence after the
cartoon.

- [ ] **Step 7: Update the docs**

In `specs/play-verification.md` §12, add under the endgame scenarios:

```markdown
N. After the win or lose screen is dismissed, confirm the next key closes the
   game [C] (`main.c:1336`, `GameIsOver`).
```

(Use the next free scenario number in that list.) In `AGENTS.md` "Current
Engine State", add one sentence to the endgame bullet:
`Game::is_over` ports `GameIsOver` (the flag OR `days_left == 0`), and the
shell quits on any key once it holds and nothing modal is up.`
Update the test counts in `AGENTS.md` from a fresh
`cargo test --workspace 2>&1 | grep "test result"`, not by adding to the old
numbers, which were already stale at 348/265.

- [ ] **Step 8: Run the whole suite**

```sh
cargo fmt --all --check
cargo test --workspace
```

Expected: all pass.

- [ ] **Step 9: Ask for approval, then commit**

```bash
git add shell/src/main.rs specs/play-verification.md AGENTS.md
git commit -m "feat: quit on any key once the game is over"
```
