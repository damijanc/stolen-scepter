# Remaining Work: Handoff Index

Written 2026-09-30 against `f081b2c`. It lists every open feature slice, one
plan per session. Art and content are out of scope here. That covers the open
pack's `game.json`, the maps, the 43 `ui/` art slots and the bitmap font. They
belong to the asset track (`specs/ai-art-handoff.md`, `packs/open/CHECKLIST.md`).

## How to hand a slice off

Start a fresh session in this repo and give it one line:

> Implement `specs/<name>-plan.md` task by task. Read `AGENTS.md` and the plan's
> READ FIRST document before starting.

Every plan carries the `AGENTS.md` constraints in its own Global Constraints
section: ask before each commit, single-line commit messages with no trailers,
and no dependencies in `engine/Cargo.toml`. A session needs nothing from this
conversation.

## The slices

| # | Plan | Design doc (READ FIRST) | Tasks | New dependency | Touches `engine/` |
|---|---|---|---|---|---|
| 1 | `specs/sidebar-hud-plan.md` | none; plan is self-contained | 5 | none | no |
| 2 | `specs/ingame-menu-plan.md` | `specs/ingame-menu-design.md` | 5 | none | no |
| 3 | `specs/combat-options-plan.md` | none; C behaviour settled inline | 2 | none | no |
| 4 | `specs/cheat-menu-plan.md` | none; C behaviour settled inline | 5 | none | no |
| 4b | `specs/game-over-plan.md` | none | 2 | none | `game.rs`, adding `Game::is_over` |
| 5 | `specs/audio-plan.md` | `specs/audio-design.md` | 6 | `kira` (shell) | `resources.rs`, parsing the pack `audio` block |
| 6 | `specs/pack-picker-plan.md` | none; verified facts inline | 5 | `zip` (shell) | no |
| 7 | `specs/engine-sweep-plan.md` | none | 3 | none | no (audit only) |
| 8 | `specs/play-verification-plan.md` | `specs/play-verification.md` | 3 | none | no (docs only) |

Slices 1, 2, 5 and 6 were written 2026-09-22. Nothing in `shell/src` has changed
since 2026-09-20, so their `file:line` citations should still hold. Treat any
line number that is off as drift and re-find the symbol. It does not mean the
plan is wrong.

## Order

Recommended sequence, with the reason for each position:

1. **Sidebar HUD.** It is the most visible gap in play: gold appears only in
   the Character view. It is small, and everything below the draw layer
   already exists.
2. **Engine sweep.** It is independent of every shell slice and touches no
   Rust. If it finds a real engine gap, that gap outranks everything below it.
   It can run in parallel with anything.
3. **Cheat menu.** It makes the verification scenarios cheap to set up.
4. **Combat options and game-over exit.** Both are tiny. Combat options
   closes the documented §9 gap. The game-over exit ports `GameIsOver` and
   C's any-key-quits branch; without it a finished game freezes on the board.
5. **In-game menu.** Save-and-continue and mid-session load.
6. **Play verification, Task 1 (manual cross-check).** It can run any time. It
   is placed here so it can run while the menu is in review.
7. **Audio.**
8. **Pack picker.** Last, because it rewires startup: `main.rs` stops
   requiring a `game.json` argument. Every other plan's run commands pass that
   path, so landing it earlier would make those commands stale mid-flight.
9. **Play verification, Tasks 2-3 (playthrough and triage).** It goes after
   the HUD and cheat menu at minimum. Ideally it runs once everything else has
   landed, so the playthrough covers the finished game.

## Running slices in parallel

Every shell plan modifies `shell/src/main.rs`, and most also modify
`shell/src/views.rs`. All of them update the test counts in `AGENTS.md`.
Parallel sessions WILL conflict on those three files, though the conflicts are
mechanical: new match arms, new `mod` lines and a number.

- **Safe to run alongside anything:** the engine sweep (#7) and play
  verification Task 1 (#8). They touch only `tools/`, `specs/` and one
  `AGENTS.md` paragraph.
- **Shell slices:** run them one at a time, or in separate worktrees rebased
  onto `main` after each merge. Do not have two sessions editing the same
  checkout.
- **Test counts in `AGENTS.md`:** these are already stale. The file says
  348 engine / 265 shell; the tip actually has 350 / 299. Every slice
  recomputes the counts instead of adding to the old ones. Run `cargo test --workspace 2>&1 | grep "test result"`
  after rebasing.

## Cross-slice interactions to watch

- **Audio × in-game menu:** both touch `views.rs`, where the Controls rows
  live. Audio makes the Sounds/Music/Volume rows do something. The menu adds
  a `ViewKind`. They do not overlap in logic, only in file.
- **Pack picker × in-game menu:** the in-game menu's design explicitly leaves
  out New Game. If a later slice adds it, New Game must go back through the
  pack picker's startup path.
- **Cheat menu × play verification:** the verification plan's Task 2 names
  specific cheats (`G`, `S`, `N`, `W`, `L`). If the cheat plan diverges from
  C's key set, update Task 2's list.
- **Cheat menu × game-over exit:** the cheat plan's `W` sets `game_over` but
  not `stats.won`, as C does. Once the game-over slice lands, the key after
  the win screen quits. That is correct and should not be reported as a bug.
- **Every slice × `specs/play-verification.md` §13:** each slice that closes a
  GAP row must flip that row in the same slice. The guide warns that stale GAP
  rows have already misled a playtester.

## Out of scope, with the reason

| Item | Why it is not a slice here |
|---|---|
| Open `game.json`, maps, `ui/` art, bitmap font | Asset track, by user decision 2026-09-30 |
| `player_io.c` text queue | By design: the engine returns outcome enums and the shell writes the text (`AGENTS.md`) |
| Click-to-move, click-in-combat, mouse count entry | Mouse-support design non-goals |
| `--movie`, `--autoplay`, `--demo`, `--headless`, `--validate-pack`, `--fullscreen` | C dev tooling, not play (`specs/play-verification.md` §13) |
| `game.hud_visible` | DEAD in C; do not wire it up |
| The `GAME_CONTINENTS` capacity contract | Known divergence in `AGENTS.md`. It needs its own decision about how a library crate reports a fatal pack error. Revisit only if the engine sweep finds a second reason |
