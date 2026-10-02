# Overworld Click-to-Move Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clicking a revealed overworld tile walks the hero there, one ordinary `game_step_basic` step at a time, stopping at the first interaction, foe collision, week end or modal.

**Architecture:** The engine gains one read-only query, `step_preview`, which `game_step_basic` itself now uses for its can-move decision. The shell plans an A* route over `step_preview` answers (`shell/src/walk.rs`), the map viewport publishes `HitTarget::MapTile` regions, and `main.rs` feeds one planned `(dx, dy)` per 0.12 s into the same `InputState` the arrow keys produce.

**Tech Stack:** Rust 2021 workspace, `openbounty-engine` (render-free) and `openbounty-shell` (macroquad via `backend.rs` only).

**Spec:** `specs/overworld-click-to-move-design.md` (approved 2026-10-02).

## Global Constraints

- The engine gains no presentation code; `cargo test -p openbounty-engine --test render_free` must pass.
- `backend.rs` stays the only module that knows macroquad.
- Pointer input is tested headlessly through `PointerSource` / `HitList`.
- A click never reaches a transaction the keyboard cannot: every walked tile is one `game_step_basic` call with a `(dx, dy)` an arrow key could produce.
- All coordinates are logical pixels (the `DrawList` space) or map tiles; nothing here touches window points or framebuffer pixels.
- Commit messages are one subject line, no body, no trailers. Each commit needs the user's approval first.
- Before each report: `cargo fmt --all --check` and `cargo test --workspace`.
- Two commits: Task 1 alone (`feat: expose the step preview query`), then Tasks 2-5 (`feat: click to walk on the overworld`). Task 6 is docs (`docs: describe the shipped mouse support`).

## File map

| File | Change |
| --- | --- |
| `engine/src/step.rs` | Add `StepPreview`, `step_preview`, `boards_boat`; `game_step_basic` uses them |
| `shell/src/walk.rs` | NEW: `plan`, `click`, `Walk`, `stops_walk` |
| `shell/src/pointer.rs` | Add `HitTarget::MapTile(i32, i32)` |
| `shell/src/map_render.rs` | Add `MapScene::publish_hits` |
| `shell/src/main.rs` | `mod walk;`, the `walk` session state, wiring |
| `specs/mouse-support-design.md`, `specs/play-verification.md` | Task 6 docs |

---

### Task 1: Engine `step_preview` query

**Files:**
- Modify: `engine/src/step.rs` (the `game_step_basic` can-move block, lines ~138-162, and add the new items after `step_move_cost`)
- Test: `engine/src/step.rs` `mod tests`

**Interfaces:**
- Produces: `pub enum StepPreview { BlockedEdge, BlockedTerrain { terrain: Terrain }, Open { cost: i32, interacts: bool, changes_mode: bool } }` (derive `Clone, Copy, Debug, Eq, PartialEq`) and `pub fn step_preview(game: &Game, map: &Map, x: i32, y: i32) -> StepPreview`.

- [ ] **Step 1: Write the failing tests** (append to `mod tests`; add `step_preview, StepPreview` to the `use super::{...}` list)

```rust
    /// The preview must agree with what `game_step_basic` then does, for
    /// every travel mode, on a strip of every tile kind.
    fn preview_strip() -> Map {
        let mut chest = Tile::interactive(Interact::TreasureChest);
        chest.terrain = Terrain::Grass;
        Map::from_tiles(
            7,
            1,
            vec![
                Tile::plain_grass(),
                Tile::terrain(Terrain::Desert),
                Tile::terrain(Terrain::Water),
                Tile::terrain(Terrain::Mountain),
                Tile::blocked_grass(),
                chest,
                Tile::plain_grass(),
            ],
        )
        .expect("fixture dimensions match")
    }

    fn assert_preview_matches_step(mut game: Game, map: &Map, x: i32) {
        game.position.x = x - 1;
        game.position.y = 0;
        game.stats.steps_left_today = 40;
        let preview = step_preview(&game, map, x, 0);
        let mode_before = game.travel_mode;
        let mut map = map.clone();
        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &time_fixture(), 1, 0);
        match (preview, &result) {
            (StepPreview::BlockedTerrain { terrain }, StepResult::BlockedTerrain { terrain: t }) => {
                assert_eq!(terrain, *t)
            }
            (StepPreview::BlockedEdge, StepResult::BlockedEdge) => {}
            (
                StepPreview::Open { cost, interacts, changes_mode },
                StepResult::Moved { move_cost, interaction, .. },
            ) => {
                assert_eq!(cost, *move_cost, "x={x}");
                if !interacts {
                    assert_eq!(*interaction, StepInteraction::None, "x={x}");
                }
                assert_eq!(changes_mode, game.travel_mode != mode_before, "x={x}");
            }
            other => panic!("x={x}: preview and step disagree: {other:?}"),
        }
    }

    #[test]
    fn the_step_preview_matches_the_step_on_foot() {
        let map = preview_strip();
        for x in 1..7 {
            assert_preview_matches_step(Game::default(), &map, x);
        }
    }

    #[test]
    fn the_step_preview_matches_the_step_in_a_boat() {
        let map = preview_strip();
        for x in 1..7 {
            let mut game = Game::default();
            game.travel_mode = TravelMode::Boat;
            game.boat.has_boat = true;
            assert_preview_matches_step(game, &map, x);
        }
    }

    #[test]
    fn the_step_preview_matches_the_step_while_flying() {
        let map = preview_strip();
        for x in 1..7 {
            let mut game = Game::default();
            game.character.mount = Mount::Fly;
            assert_preview_matches_step(game, &map, x);
        }
    }

    /// Boarding the owned boat is a mode change; the preview says so.
    #[test]
    fn the_step_preview_reports_boarding_the_boat() {
        let map = preview_strip();
        let mut game = Game::default();
        game.boat.has_boat = true;
        game.boat.x = 2;
        game.boat.y = 0;
        assert!(matches!(
            step_preview(&game, &map, 2, 0),
            StepPreview::Open { changes_mode: true, .. }
        ));
        assert_preview_matches_step(game, &map, 2);
    }

    /// GAP 1 today: a flying hero still triggers the chest, so the preview
    /// still reports `interacts` while flying.
    #[test]
    fn the_step_preview_reports_interactions_while_flying_until_gap_1_is_fixed() {
        let map = preview_strip();
        let mut game = Game::default();
        game.character.mount = Mount::Fly;
        assert!(matches!(
            step_preview(&game, &map, 5, 0),
            StepPreview::Open { interacts: true, .. }
        ));
    }

    #[test]
    fn the_step_preview_reports_the_edge() {
        let map = preview_strip();
        assert_eq!(step_preview(&Game::default(), &map, 7, 0), StepPreview::BlockedEdge);
        assert_eq!(step_preview(&Game::default(), &map, 0, -1), StepPreview::BlockedEdge);
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p openbounty-engine --lib step_preview`
Expected: compile error, `step_preview` / `StepPreview` not found.

- [ ] **Step 3: Implement** (in `engine/src/step.rs`)

Add after `step_move_cost`:

```rust
/// What one step onto `(x, y)` would do, read without taking it.
///
/// Evaluated as if the hero stood next to the tile in their current travel
/// mode: walkability depends on the mode and the boat, not on the tile being
/// left. `game_step_basic` makes its own can-move decision through this, so
/// the answer and the real step cannot drift. Shell-facing (click-to-move);
/// C has no counterpart because C has no pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepPreview {
    BlockedEdge,
    BlockedTerrain {
        terrain: Terrain,
    },
    Open {
        /// `step_move_cost`: desert 40 on foot or afloat, else 1.
        cost: i32,
        /// The tile has an interaction `resolve_interaction` will examine.
        /// True while flying too, because `game_step_basic` resolves them
        /// while flying (engine-sweep GAP 1); fixing GAP 1 changes this.
        interacts: bool,
        /// Boarding the owned boat, or a boat step onto land.
        changes_mode: bool,
    },
}

/// True when a step onto `(x, y)` boards the owned boat.
fn boards_boat(game: &Game, x: i32, y: i32) -> bool {
    game.character.mount != Mount::Fly
        && game.travel_mode == TravelMode::Walk
        && game.boat.has_boat
        && x == game.boat.x
        && y == game.boat.y
        && (game.boat.zone.is_empty() || game.boat.zone.as_str() == game.position.zone.as_str())
}

pub fn step_preview(game: &Game, map: &Map, x: i32, y: i32) -> StepPreview {
    let Some(tile) = map.tile_i32(x, y) else {
        return StepPreview::BlockedEdge;
    };
    let flying = game.character.mount == Mount::Fly;
    let walking = game.travel_mode == TravelMode::Walk;
    let in_boat = game.travel_mode == TravelMode::Boat;
    let will_board = boards_boat(game, x, y);
    let can_move = if flying || will_board {
        true
    } else if walking {
        walkable_on_foot(tile)
    } else {
        walkable_in_boat(tile)
    };
    if !can_move {
        return StepPreview::BlockedTerrain {
            terrain: tile.terrain,
        };
    }
    let lands = in_boat && !flying && !(tile.terrain == Terrain::Water || tile.is_bridge);
    StepPreview::Open {
        cost: step_move_cost(tile.terrain, flying),
        interacts: tile.interactive != Interact::None,
        changes_mode: will_board || lands,
    }
}
```

In `game_step_basic`, replace everything from `let nx = game.position.x + dx;` down to the closing `}` of `if !can_move { ... }` with (the marker clearing above it stays first, as now):

```rust
    let nx = game.position.x + dx;
    let ny = game.position.y + dy;
    match step_preview(game, map, nx, ny) {
        StepPreview::BlockedEdge => return StepResult::BlockedEdge,
        StepPreview::BlockedTerrain { terrain } => {
            return StepResult::BlockedTerrain { terrain }
        }
        StepPreview::Open { .. } => {}
    }
    let tile = *map.tile_i32(nx, ny).expect("step_preview checked the edge");

    let prev_x = game.position.x;
    let prev_y = game.position.y;
    let in_boat = game.travel_mode == TravelMode::Boat;
    let flying = game.character.mount == Mount::Fly;
    let will_board = boards_boat(game, nx, ny);
```

(`walking` is no longer read in `game_step_basic`; drop it. `Tile` is `Copy` if `.copied()` compiled before; keep `.copied()` form if `*` does not.)

- [ ] **Step 4: Run the engine tests**

Run: `cargo test -p openbounty-engine --lib && cargo test -p openbounty-engine --test combat_digests && cargo test -p openbounty-engine --test render_free`
Expected: all pass (359 + 6 new lib tests).

- [ ] **Step 5: Report and ask, then commit**

```bash
git add engine/src/step.rs
git commit -m "feat: expose the step preview query"
```

---

### Task 2: The route planner

**Files:**
- Create: `shell/src/walk.rs`
- Modify: `shell/src/main.rs` (add `pub mod walk;` after `pub mod views;`)

**Interfaces:**
- Consumes: `step_preview`, `StepPreview` (Task 1); `Fog::seen(x, y) -> bool`; `Map::tile_i32`, `Map.width/height`; `Interact::Foe`.
- Produces: `pub fn plan(game: &Game, map: &Map, fog: &Fog, goal: (i32, i32)) -> Option<Vec<(i32, i32)>>` returning the direction of each step, hero to goal.

- [ ] **Step 1: Write the failing tests** (in `shell/src/walk.rs`, `#[cfg(test)] mod tests`)

```rust
    use super::*;
    use openbounty_engine::map::Tile;
    use openbounty_engine::tile::{Interact, Terrain};

    /// A map from rows of chars: `.` grass, `d` desert, `~` water, `^`
    /// mountain, `c` chest, `f` foe, `t` town gate. All of it seen.
    fn world(rows: &[&str]) -> (Map, Fog) {
        let h = rows.len();
        let w = rows[0].len();
        let mut tiles = Vec::new();
        for row in rows {
            for ch in row.chars() {
                tiles.push(match ch {
                    'd' => Tile::terrain(Terrain::Desert),
                    '~' => Tile::terrain(Terrain::Water),
                    '^' => Tile::terrain(Terrain::Mountain),
                    'c' => Tile::interactive(Interact::TreasureChest),
                    'f' => Tile::interactive(Interact::Foe),
                    't' => Tile::interactive(Interact::Town),
                    _ => Tile::plain_grass(),
                });
            }
        }
        let map = Map::from_tiles(w, h, tiles).expect("rectangular");
        // `Fog::reveal` ignores its radius and stamps 5x5, so a test that
        // needs exact fog writes the field, as map_render's tests do.
        let mut fog = Fog::new();
        for y in 0..h {
            for x in 0..w {
                fog.seen[y][x] = true;
            }
        }
        (map, fog)
    }

    fn hero_at(x: i32, y: i32) -> Game {
        let mut game = Game::default();
        game.position.x = x;
        game.position.y = y;
        game
    }

    fn walk_to(game: &Game, path: &[(i32, i32)]) -> (i32, i32) {
        path.iter().fold((game.position.x, game.position.y), |(x, y), (dx, dy)| (x + dx, y + dy))
    }

    #[test]
    fn a_straight_route_is_one_step_per_tile() {
        let (map, fog) = world(&["....."]);
        let path = plan(&hero_at(0, 0), &map, &fog, (4, 0)).expect("route");
        assert_eq!(path, vec![(1, 0); 4]);
    }

    #[test]
    fn diagonals_are_used_like_the_keypad() {
        let (map, fog) = world(&["...", "...", "..."]);
        assert_eq!(plan(&hero_at(0, 0), &map, &fog, (2, 2)), Some(vec![(1, 1), (1, 1)]));
    }

    #[test]
    fn the_route_goes_around_mountains() {
        let (map, fog) = world(&[".^.", ".^.", "..."]);
        let game = hero_at(0, 0);
        let path = plan(&game, &map, &fog, (2, 0)).expect("route");
        assert_eq!(walk_to(&game, &path), (2, 0));
        assert_eq!(path.len(), 4);
    }

    /// Fewest days: four grass steps beat one desert step (cost 40).
    #[test]
    fn the_route_detours_around_desert_when_that_is_fewer_days() {
        let (map, fog) = world(&[".d.", "..."]);
        let path = plan(&hero_at(0, 0), &map, &fog, (2, 0)).expect("route");
        assert_eq!(path, vec![(1, 1), (1, -1)]);
    }

    #[test]
    fn unseen_tiles_are_neither_route_nor_goal() {
        let (map, _) = world(&["....."]);
        let mut fog = Fog::new();
        for x in 0..3 {
            fog.seen[0][x] = true;
        }
        assert_eq!(plan(&hero_at(0, 0), &map, &fog, (4, 0)), None);
        assert!(plan(&hero_at(0, 0), &map, &fog, (2, 0)).is_some());
    }

    #[test]
    fn an_interactive_tile_is_a_goal_but_never_a_waypoint() {
        let (map, fog) = world(&[".c.", "..."]);
        let game = hero_at(0, 0);
        let around = plan(&game, &map, &fog, (2, 0)).expect("route");
        assert!(!around.is_empty());
        let mut pos = (0, 0);
        for (dx, dy) in &around[..around.len() - 1] {
            pos = (pos.0 + dx, pos.1 + dy);
            assert_ne!(pos, (1, 0), "walked through the chest");
        }
        let (map, fog) = world(&["..t"]);
        assert_eq!(plan(&game, &map, &fog, (2, 0)), Some(vec![(1, 0), (1, 0)]));
    }

    /// GAP 3: no attack prompt yet, so a foe is never a route's goal.
    #[test]
    fn a_foe_is_never_a_goal() {
        let (map, fog) = world(&["...f"]);
        assert_eq!(plan(&hero_at(0, 0), &map, &fog, (3, 0)), None);
    }

    /// Boarding is allowed as the last step only, and a walker plans on land.
    #[test]
    fn boarding_the_boat_ends_a_route() {
        let (map, fog) = world(&["..~~"]);
        let mut game = hero_at(0, 0);
        game.boat.has_boat = true;
        game.boat.x = 2;
        game.boat.y = 0;
        assert_eq!(plan(&game, &map, &fog, (2, 0)), Some(vec![(1, 0), (1, 0)]));
        assert_eq!(plan(&game, &map, &fog, (3, 0)), None, "no sailing on past boarding");
    }

    /// GAP 1 today: a flyer still avoids interactive tiles mid-route.
    #[test]
    fn a_flyer_crosses_mountains_but_avoids_interactions_today() {
        let (map, fog) = world(&[".^c.", "...."]);
        let mut game = hero_at(0, 0);
        game.character.mount = openbounty_engine::game::Mount::Fly;
        let path = plan(&game, &map, &fog, (3, 0)).expect("route");
        let mut pos = (0, 0);
        for (dx, dy) in &path[..path.len() - 1] {
            pos = (pos.0 + dx, pos.1 + dy);
            assert_ne!(pos, (2, 0));
        }
        assert_eq!(plan(&game, &map, &fog, (2, 1)).map(|p| p.len()), Some(2));
    }

    #[test]
    fn no_route_and_the_hero_tile_plan_nothing() {
        let (map, fog) = world(&[".^."]);
        let game = hero_at(0, 0);
        let (walled, wfog) = world(&[".^.", "^^."]);
        assert_eq!(plan(&game, &walled, &wfog, (2, 0)), None);
        assert_eq!(plan(&game, &map, &fog, (0, 0)), None);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p openbounty-shell walk::`
Expected: compile error (module or `plan` missing).

- [ ] **Step 3: Implement** (top of `shell/src/walk.rs`)

```rust
//! Overworld click-to-move. A DIVERGENCE, deliberate: C moves one tile per
//! key and has no pointer. See `specs/overworld-click-to-move-design.md`.
//!
//! The route only chooses which direction keys to press. Every tile of it is
//! one ordinary `game_step_basic` call, so interactions, foe following, fog,
//! days and week ends fire exactly as they do for keys. Walkability comes
//! from the engine's `step_preview`; nothing here re-derives a terrain rule.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use openbounty_engine::fog::Fog;
use openbounty_engine::game::Game;
use openbounty_engine::map::Map;
use openbounty_engine::step::{step_preview, StepPreview};
use openbounty_engine::tile::Interact;

/// The eight keypad directions.
const DIRS: [(i32, i32); 8] = [
    (0, -1),
    (0, 1),
    (-1, 0),
    (1, 0),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

/// The cost of stepping onto `(x, y)` from next to it, or `None` when the
/// tile may not be on this route. Mid-route tiles must be plain; the goal may
/// interact or change the travel mode, but may not hold a foe (GAP 3).
fn step_cost(game: &Game, map: &Map, fog: &Fog, x: i32, y: i32, is_goal: bool) -> Option<i32> {
    if !fog.seen(x, y) {
        return None;
    }
    let StepPreview::Open {
        cost,
        interacts,
        changes_mode,
    } = step_preview(game, map, x, y)
    else {
        return None;
    };
    if is_goal {
        let foe = map.tile_i32(x, y).is_some_and(|t| t.interactive == Interact::Foe);
        return (!foe).then_some(cost);
    }
    (!interacts && !changes_mode).then_some(cost)
}

/// A* from the hero to `goal`, fewest days first. The heuristic is the
/// keypad (Chebyshev) distance, admissible because every step costs >= 1.
/// Returns each step's direction, or `None` when there is no route (or the
/// goal is the hero's own tile).
pub fn plan(game: &Game, map: &Map, fog: &Fog, goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    let start = (game.position.x, game.position.y);
    if start == goal || map.tile_i32(goal.0, goal.1).is_none() {
        return None;
    }
    let (w, h) = (map.width as i32, map.height as i32);
    let index = |(x, y): (i32, i32)| (y * w + x) as usize;
    let mut best = vec![i32::MAX; (w * h) as usize];
    let mut came_from: Vec<Option<(i32, i32)>> = vec![None; (w * h) as usize];
    let heuristic = |(x, y): (i32, i32)| (x - goal.0).abs().max((y - goal.1).abs());
    let mut open = BinaryHeap::new();
    best[index(start)] = 0;
    open.push(Reverse((heuristic(start), 0, start)));
    while let Some(Reverse((_, cost, pos))) = open.pop() {
        if pos == goal {
            let mut dirs = Vec::new();
            let mut at = goal;
            while let Some(prev) = came_from[index(at)] {
                dirs.push((at.0 - prev.0, at.1 - prev.1));
                at = prev;
            }
            dirs.reverse();
            return Some(dirs);
        }
        if cost > best[index(pos)] {
            continue;
        }
        for (dx, dy) in DIRS {
            let next = (pos.0 + dx, pos.1 + dy);
            if next.0 < 0 || next.1 < 0 || next.0 >= w || next.1 >= h {
                continue;
            }
            let Some(step) = step_cost(game, map, fog, next.0, next.1, next == goal) else {
                continue;
            };
            let total = cost + step;
            if total < best[index(next)] {
                best[index(next)] = total;
                came_from[index(next)] = Some(pos);
                open.push(Reverse((total + heuristic(next), total, next)));
            }
        }
    }
    None
}
```

Note for the implementer: the travel mode is the hero's CURRENT one for the whole search; that is what makes a mode change only legal as the goal step (a later tile would be judged in the wrong mode).

- [ ] **Step 4: Run** `cargo test -p openbounty-shell walk::` -- expected PASS.

---

### Task 3: The walk state and its stop rule

**Files:**
- Modify: `shell/src/walk.rs`

**Interfaces:**
- Consumes: `plan` (Task 2), `StepResult`, `StepInteraction` from `openbounty_engine::step`.
- Produces:
  - `pub const WALK_STEP_SECONDS: f64 = 0.12;`
  - `pub enum ClickMove { Step(i32, i32), Route(Walk), Nothing }` and `pub fn click(game: &Game, map: &Map, fog: &Fog, tile: (i32, i32), now: f64) -> ClickMove`
  - `pub struct Walk` with `pub fn next(&mut self, game: &Game, map: &Map, fog: &Fog, now: f64) -> WalkTick`
  - `pub enum WalkTick { Wait, Step(i32, i32), Done }`
  - `pub fn stops_walk(result: &StepResult) -> bool`

- [ ] **Step 1: Write the failing tests** (append to `walk.rs` tests)

```rust
    use openbounty_engine::step::{game_step_basic, StepAdvance, StepInteraction, StepResult};
    use openbounty_engine::resources::Resources;

    fn resources() -> Resources {
        Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses")
    }

    #[test]
    fn an_adjacent_click_is_that_direction_key_even_onto_a_foe() {
        let (map, fog) = world(&["..f"]);
        let game = hero_at(1, 0);
        assert!(matches!(click(&game, &map, &fog, (2, 0), 0.0), ClickMove::Step(1, 0)));
        assert!(matches!(click(&game, &map, &fog, (0, 0), 0.0), ClickMove::Step(-1, 0)));
        assert!(matches!(click(&game, &map, &fog, (1, 0), 0.0), ClickMove::Nothing));
    }

    #[test]
    fn a_far_click_routes_and_the_walk_paces_its_steps() {
        let (map, fog) = world(&["...."]);
        let game = hero_at(0, 0);
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (3, 0), 10.0) else {
            panic!("expected a route");
        };
        assert!(matches!(walk.next(&game, &map, &fog, 10.0), WalkTick::Step(1, 0)));
        assert!(matches!(walk.next(&game, &map, &fog, 10.05), WalkTick::Wait));
    }

    /// Driving the walk through `game_step_basic` leaves the world exactly as
    /// pressing the same arrows does.
    #[test]
    fn a_walk_equals_the_same_arrow_presses() {
        let (map0, fog0) = world(&["..d..", "....."]);
        let resources = resources();
        let mut game = hero_at(0, 0);
        game.stats.steps_left_today = 40;
        let (mut map, mut fog) = (map0.clone(), fog0.clone());
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (4, 0), 0.0) else {
            panic!("expected a route");
        };
        let mut dirs = Vec::new();
        let mut now = 0.0;
        loop {
            match walk.next(&game, &map, &fog, now) {
                WalkTick::Step(dx, dy) => {
                    dirs.push((dx, dy));
                    let result = game_step_basic(&mut game, &mut map, &mut fog, &resources, dx, dy);
                    if stops_walk(&result) {
                        break;
                    }
                }
                WalkTick::Wait => {}
                WalkTick::Done => break,
            }
            now += WALK_STEP_SECONDS;
        }
        let mut keyed = hero_at(0, 0);
        keyed.stats.steps_left_today = 40;
        let (mut kmap, mut kfog) = (map0, fog0);
        for (dx, dy) in &dirs {
            game_step_basic(&mut keyed, &mut kmap, &mut kfog, &resources, *dx, *dy);
        }
        assert_eq!((game.position.x, game.position.y), (4, 0));
        assert_eq!(game, keyed);
        assert_eq!(map, kmap);
        assert_eq!(fog, kfog);
    }

    /// A tile that changed since planning stops the walk instead of acting.
    #[test]
    fn a_tile_that_changed_since_planning_stops_the_walk() {
        let (mut map, fog) = world(&["...."]);
        let game = hero_at(0, 0);
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (3, 0), 0.0) else {
            panic!("expected a route");
        };
        map.stamp_foe(1, 0, "band0");
        assert!(matches!(walk.next(&game, &map, &fog, 0.0), WalkTick::Done));
    }

    #[test]
    fn the_walk_stops_on_interactions_collisions_week_ends_and_blocks() {
        let moved = |interaction, collision, week_ended| StepResult::Moved {
            terrain: Terrain::Grass,
            move_cost: 1,
            advance: StepAdvance { day_ended: false, week_ended, commission_paid: 0 },
            interaction,
            collision,
        };
        assert!(!stops_walk(&moved(StepInteraction::None, None, false)));
        assert!(stops_walk(&moved(StepInteraction::Alcove, None, false)));
        assert!(stops_walk(&moved(StepInteraction::None, None, true)));
        assert!(stops_walk(&moved(
            StepInteraction::None,
            Some(openbounty_engine::step::FoeCollision {
                id: "band0".into(),
                friendly: false,
                x: 0,
                y: 0
            }),
            false
        )));
        assert!(stops_walk(&StepResult::BlockedEdge));
        assert!(stops_walk(&StepResult::NoDirection));
    }
```

(`Game` and `Fog` derive `Copy, Eq`; `Map` derives `Clone, Eq`. All three compare whole.)

- [ ] **Step 2: Run to verify failure:** `cargo test -p openbounty-shell walk::` -- compile errors.

- [ ] **Step 3: Implement** (append to `walk.rs` above the tests)

```rust
use openbounty_engine::step::{StepInteraction, StepResult};

/// How long each walked step holds before the next, in seconds.
pub const WALK_STEP_SECONDS: f64 = 0.12;

/// What a click on map tile `tile` asks for.
pub enum ClickMove {
    /// An adjacent tile: exactly that direction key, foes included.
    Step(i32, i32),
    /// A route to walk.
    Route(Walk),
    /// The hero's own tile, or no route.
    Nothing,
}

pub fn click(game: &Game, map: &Map, fog: &Fog, tile: (i32, i32), now: f64) -> ClickMove {
    let (dx, dy) = (tile.0 - game.position.x, tile.1 - game.position.y);
    if (dx, dy) == (0, 0) {
        return ClickMove::Nothing;
    }
    if dx.abs() <= 1 && dy.abs() <= 1 {
        return ClickMove::Step(dx, dy);
    }
    match plan(game, map, fog, tile) {
        Some(dirs) => ClickMove::Route(Walk {
            dirs,
            taken: 0,
            next_at: now,
        }),
        None => ClickMove::Nothing,
    }
}

/// A route being walked.
pub struct Walk {
    dirs: Vec<(i32, i32)>,
    taken: usize,
    next_at: f64,
}

pub enum WalkTick {
    /// Not time for the next step yet.
    Wait,
    /// Take this step now, through the ordinary move path.
    Step(i32, i32),
    /// Arrived, or the next tile no longer matches the plan.
    Done,
}

impl Walk {
    /// The next step, re-checked against the world as it is NOW: a tile that
    /// stopped being usable since planning (a foe stepped onto it) ends the
    /// walk rather than acting on a changed world.
    pub fn next(&mut self, game: &Game, map: &Map, fog: &Fog, now: f64) -> WalkTick {
        let Some(&(dx, dy)) = self.dirs.get(self.taken) else {
            return WalkTick::Done;
        };
        if now < self.next_at {
            return WalkTick::Wait;
        }
        let (x, y) = (game.position.x + dx, game.position.y + dy);
        let is_goal = self.taken + 1 == self.dirs.len();
        if step_cost(game, map, fog, x, y, is_goal).is_none() {
            return WalkTick::Done;
        }
        self.taken += 1;
        self.next_at = now + WALK_STEP_SECONDS;
        WalkTick::Step(dx, dy)
    }
}

/// Whether a walked step's result ends the walk: anything but a plain move.
/// The collision stop reads the result directly because the shell shows
/// nothing for a collision yet (engine-sweep GAP 4).
pub fn stops_walk(result: &StepResult) -> bool {
    match result {
        StepResult::Moved {
            advance,
            interaction,
            collision,
            ..
        } => *interaction != StepInteraction::None || collision.is_some() || advance.week_ended,
        _ => true,
    }
}
```

- [ ] **Step 4: Run** `cargo test -p openbounty-shell walk::` -- PASS.

---

### Task 4: Map tiles publish click regions

**Files:**
- Modify: `shell/src/pointer.rs` (add `MapTile(i32, i32)` to `HitTarget`, doc: "An overworld map tile in the viewport, by MAP coordinates.")
- Modify: `shell/src/map_render.rs` (add `publish_hits`; `use crate::pointer::{HitList, HitTarget};`)

**Interfaces:**
- Produces: `pub fn publish_hits(&self, hits: &mut HitList)` on `MapScene`.

- [ ] **Step 1: Failing test** (in `map_render.rs` tests; add `use crate::pointer::{HitList, HitTarget};`)

```rust
    /// The anti-drift test: each viewport cell publishes the rect `cell`
    /// draws it on, labelled with its MAP coordinates.
    #[test]
    fn each_viewport_tile_publishes_its_drawn_cell() {
        let (layout, catalog, palette, resources, game, map, fog) = scene_parts();
        let scene = scene!(layout, catalog, palette, resources, game, map, fog);
        let (cam_x, cam_y) = scene.camera();
        let mut hits = HitList::new();
        scene.publish_hits(&mut hits);
        for ty in 0..scene.layout.tiles_h {
            for tx in 0..scene.layout.tiles_w {
                let (mx, my) = (cam_x + tx, cam_y + ty);
                if scene.map.tile_i32(mx, my).is_none() {
                    continue;
                }
                let Dst::Stretch(drawn) = scene.cell(tx, ty) else {
                    panic!("cells are stretched rects");
                };
                let region = hits
                    .regions()
                    .iter()
                    .find(|r| r.target == HitTarget::MapTile(mx, my))
                    .expect("tile published");
                assert_eq!(region.rect, drawn);
                assert_eq!(
                    hits.hit(drawn.x + drawn.w / 2, drawn.y + drawn.h / 2),
                    Some(HitTarget::MapTile(mx, my))
                );
            }
        }
    }
```

- [ ] **Step 2: Run** `cargo test -p openbounty-shell map_render` -- fails (no `publish_hits`).

- [ ] **Step 3: Implement** in `impl MapScene`:

```rust
    /// The viewport's click regions: one per in-map cell, on the rect `cell`
    /// draws, labelled with MAP coordinates. Unseen tiles are published too:
    /// an adjacent click into fog is a plain step, as its key is; the route
    /// planner refuses unseen goals itself.
    pub fn publish_hits(&self, hits: &mut HitList) {
        let (cam_x, cam_y) = self.camera();
        for ty in 0..self.layout.tiles_h {
            for tx in 0..self.layout.tiles_w {
                let (mx, my) = (cam_x + tx, cam_y + ty);
                if self.map.tile_i32(mx, my).is_none() {
                    continue;
                }
                let rect = Rect::new(
                    MAP_X + tx * self.layout.tile_w,
                    MAP_Y + ty * self.layout.tile_h,
                    self.layout.tile_w,
                    self.layout.tile_h,
                );
                hits.push(rect, HitTarget::MapTile(mx, my));
            }
        }
    }
```

This repeats `cell`'s arithmetic; to keep one source, change `cell` to return `Dst::Stretch(self.cell_rect(tx, ty))` with a new `fn cell_rect(&self, tx, ty) -> Rect`, and push `self.cell_rect(tx, ty)` here. The anti-drift test then guards both.

- [ ] **Step 4: Run** -- PASS.

---

### Task 5: Wire the walk into `main.rs`

**Files:**
- Modify: `shell/src/main.rs`

- [ ] **Step 1: Session state.** Next to `let mut menu_picker ...`, add:

```rust
    // Overworld click-to-move: the route being walked, if any. See walk.rs.
    let mut walk: Option<walk::Walk> = None;
```

In the menu's successful-load reset block (where `menu_picker = None;` is set after a load), add `walk = None;`.

- [ ] **Step 2: Publish the regions.** Where the overworld `MapScene { ... }.draw(&mut list);` is built, bind it and publish:

```rust
            let scene = MapScene { /* fields unchanged */ };
            scene.draw(&mut list);
            scene.publish_hits(&mut hits);
```

- [ ] **Step 3: Feed the step.** In the final overworld `else` branch, replace `let input = input::poll(&keys);` with:

```rust
            let mut input = input::poll(&keys);
            // Click-to-move (walk.rs). Any key or click ends a walk; a map
            // click may start a new one. A walked step is fed in as the
            // same move value an arrow key produces, never as a fake key.
            if keys.any_pressed() || clicked.is_some() {
                walk = None;
            }
            if let Some(HitTarget::MapTile(x, y)) = clicked {
                match walk::click(&game, &map, &fog, (x, y), get_time()) {
                    walk::ClickMove::Step(dx, dy) => {
                        input.dx = dx;
                        input.dy = dy;
                    }
                    walk::ClickMove::Route(route) => walk = Some(route),
                    walk::ClickMove::Nothing => {}
                }
            }
            if input == InputState::default() {
                if let Some(route) = walk.as_mut() {
                    match route.next(&game, &map, &fog, get_time()) {
                        walk::WalkTick::Step(dx, dy) => {
                            input.dx = dx;
                            input.dy = dy;
                        }
                        walk::WalkTick::Wait => {}
                        walk::WalkTick::Done => walk = None,
                    }
                }
            }
```

(`InputState` import: `use input::{..., InputState};`.)

- [ ] **Step 4: Stop on the result.** Directly after `let result = game_step_basic(...)` in the `if input.is_move()` block:

```rust
                if walk::stops_walk(&result) {
                    walk = None;
                }
```

- [ ] **Step 5: Stop on modals.** After the whole input chain (just before `list.clear(); hits.clear();`):

```rust
        // A walk never runs under anything modal, nor past the game's end.
        if dialog.is_active()
            || prompt.is_active()
            || views.is_active()
            || combat.is_some()
            || fast_quit
            || game.is_over()
        {
            walk = None;
        }
```

- [ ] **Step 6: Verify:** `cargo fmt --all && cargo fmt --all --check && cargo build -p openbounty-shell` (no warnings), then `cargo test --workspace` -- all pass.

- [ ] **Step 7: Report and ask, then commit** (Tasks 2-5 together)

```bash
git add shell/src/walk.rs shell/src/pointer.rs shell/src/map_render.rs shell/src/main.rs
git commit -m "feat: click to walk on the overworld"
```

---

### Task 6: Describe what shipped

**Files:**
- Modify: `specs/mouse-support-design.md` -- status line and Non-Goals now describe the shipped scope (menus, views, dialogs, prompts, the Game Menu and slot picker, the startup wizard, combat, overworld click-to-move, the drawn pointer); point to `specs/combat-mouse-design.md` and `specs/overworld-click-to-move-design.md`.
- Modify: `specs/play-verification.md` §13 mouse rows -- one row per shipped surface with how to check it in play.

- [ ] **Step 1:** Read both files' current mouse sections; rewrite them to match the code as committed (no future tense).
- [ ] **Step 2:** `cargo fmt --all --check && cargo test --workspace` (docs-only, sanity).
- [ ] **Step 3: Report and ask, then commit:** `git commit -m "docs: describe the shipped mouse support"`
