# Overworld Click-to-Move Design

Status: APPROVED 2026-10-02. Slice 5 of the P11 mouse work. Read
`specs/mouse-support-design.md` first; its rules all apply.

**DIVERGENCE, deliberate.** C and the DOS game move one tile per key. Walking a
route to a clicked tile is new behaviour, so this note says what it may and may
not do. The rule that keeps it honest: **every tile of a route is one ordinary
step through `game_step_basic`**, fed through the same per-step path a key
press uses. Interactions, foe following, fog, days, week ends and audio all
fire exactly as they do for keys. The route only chooses which direction keys
to press.

## Decisions (approved 2026-10-02)

| Question | Decision |
| --- | --- |
| What a route may pass through and end on | Mid-route tiles must be plain: no interaction, no travel-mode change. The goal may interact (town, castle, chest, dwelling, sign, alcove, orb, navmap, telecave) or change mode (boarding the boat, landing). A foe is never the goal of a multi-step walk. |
| Fog | Route and goal must be revealed tiles. Planning through fog would leak hidden terrain through the route it picks. |
| Route cost | Fewest days: each step weighs the engine's move cost (desert 40, else 1). |
| Pacing | Animated, one step per 0.12 s. Any key or click cancels the rest of the walk. |

Clicking a tile next to the hero is not a route: it is exactly that direction
key, foes and everything else included, as the keyboard does today.

## Part 1: engine query (its own slice)

`feat: expose the step preview query`. No behaviour change.

`game_step_basic`'s inline can-move logic (flying, boarding the owned boat,
`walkable_on_foot`, `walkable_in_boat`) moves into a public function, which
`game_step_basic` itself then calls, so the shell's answer and the real step
cannot drift:

```rust
pub enum StepPreview {
    BlockedEdge,
    BlockedTerrain { terrain: Terrain },
    Open { cost: i32, interacts: bool, changes_mode: bool },
}
pub fn step_preview(game: &Game, map: &Map, x: i32, y: i32) -> StepPreview;
```

- It is evaluated as if the hero stood next to `(x, y)` in their current
  travel mode. Walkability depends on the mode and the boat, not on the tile
  being left.
- `cost` is the existing `step_move_cost`.
- `interacts` is exactly the condition under which `game_step_basic` calls
  `resolve_interaction`. Today that is `tile.interactive != None` even while
  flying (engine-sweep GAP 1); when GAP 1 is fixed the preview changes with
  it, and flying routes open up with no shell change. Foes are stamped into
  the map (`Map::stamp_foe`), so a foe's tile reports `interacts`.
- `changes_mode` is boarding the owned boat, or a boat step onto land.

Tests: for each travel mode (walk, boat, fly) and each tile kind, the preview
agrees with what `game_step_basic` then does on a cloned game: blocked or
moved, and with or without an interaction or mode change.

## Part 2: the route, `shell/src/walk.rs`

`plan(game, map, fog, goal) -> Option<Vec<(dx, dy)>>`, an A* search:

- 8-way moves, as the keyboard has, weighted by `cost`.
- A tile is usable only if `fog.seen` is true and `step_preview` is `Open`.
- Mid-route tiles need `!interacts && !changes_mode`. The goal may have
  either, unless the goal holds a foe (a placement in `game.foes`), which is
  refused: GAP 3 means stepping onto a hostile foe starts combat with no
  prompt.
- No route means nothing happens. Zones are at most 64x64, so the search is
  bounded.
- It plans in the current travel mode only. A mode change can only be the
  last step, so a route never continues past boarding or landing.

## Part 3: walking, in `main.rs`

- `map_render` publishes `HitTarget::MapTile(x, y)` for each visible map tile,
  on the rect it draws, with an anti-drift test.
- A click on a non-adjacent `MapTile` plans a route and stores it as a
  `Walk`. A click on an adjacent tile is that direction, through the same move
  input a key produces.
- Each 0.12 s, while the input chain reaches the overworld branch (nothing
  modal open), the walk feeds its next `(dx, dy)` into the same
  `input.dx / input.dy` path the arrow keys use. It is not a synthesised key:
  it is the move value the key handler would have produced.
- Before each step it re-checks the next tile with `step_preview`. If the tile
  no longer matches what was planned (a foe stepped onto it, say), the walk
  stops rather than act on a changed world.

The walk stops:

- on any key press or click (a click starts a new walk instead),
- on a step result that is not `Moved`,
- on any interaction,
- on a foe collision -- even though GAP 4 means the shell shows nothing for
  one, so this stop must read `StepResult::Moved.collision`, not "a dialog
  opened",
- on a week end,
- on game over,
- when any view, dialog, prompt or combat is up,
- when the re-check above fails.

No route preview is drawn on hover (YAGNI).

## Engine GAPs that shape this design (not fixed here)

| GAP | Effect on click-to-move |
| --- | --- |
| 1, flying fires interactions | Flying routes must avoid interactive tiles mid-route. Handled by reading `interacts` from the engine, so fixing GAP 1 later needs no shell change. |
| 2, no bounce-back | Only where the hero stands after the goal's interaction (on the town tile, rather than bounced back). The walk has stopped by then either way. |
| 3, no attack prompt | The reason a foe can never be a route's goal. |
| 4, foe collision dropped | The reason the collision stop reads the step result directly. |

## Testing

- Engine: the preview against `game_step_basic` for every travel mode.
- Planner, on small hand-built maps: the desert detour, the fog limit, an
  interactive tile as goal and as obstacle, a foe refused as goal, flying
  avoiding interactions today, boarding or landing only as the last step, and
  no route.
- Walk, headless through `PointerSource`: each stop condition. And
  equivalence: a walk leaves the whole `Game`, `Map` and `Fog` exactly as the
  same arrow presses do.
