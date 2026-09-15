# Continuation Prompt: Town Boat Flow

Continue work in `/Users/damijancavar/development/kings-bounty` on the
parity-first Rust port of OpenBounty. Read `AGENTS.md` and
`specs/parity-port-plan.md` before editing.

The Rust engine is render-free by design. OpenBounty's current C engine is the
sole gameplay author ity; the original-game manuals and binaries under
`game-assets/` are local-only secondary references. The local ignored C clone
is at `reference/`.

The most recent committed slice is:

```text
bb10f64 feat: apply weekly economy and astrology
```

Current Rust coverage includes 73 engine unit tests, 25 combat digest goldens,
and a render-free boundary test. The working tree should be clean before you
begin; preserve any user changes you encounter.

## Task

Port the town boat rental/cancellation state logic from OpenBounty into the
Rust engine. Start by reading the exact implementations and callers of these
functions:

```text
reference/engine/game.c: GameBoatCost, GameRentBoat, GameCancelBoat
reference/engine/include/game.h: BoatActionResult declarations/comments
reference/engine/flow_resolve.c and reference/engine/player_io.c: town-flow callers
```

The weekly slice has already added resource-backed normal/cheap boat costs and
`Game::boat_cost`. Reuse those conventions rather than introducing a duplicate
price path.

## Required Parity Details To Verify Against C

- Rental is legal only when the hero is in a town (`position.in_town` is set).
- The rent attempt uses the C condition `gold <= cost`: exact-cost rental must
  fail without mutating state.
- On success, deduct the exact cost and initialize boat ownership, dock
  coordinates, and zone exactly like C.
- Cancellation must refuse while the hero is traveling by boat; otherwise it
  clears the boat state as the C implementation does.
- Preserve the effect of the `cheaper_boats` artifact through the existing
  `Game::boat_cost` helper.
- Keep prompt text and UI decisions out of the engine. Return a compact Rust
  outcome enum analogous to C's `BoatActionResult`.

## Implementation Constraints

- Keep fixed-size state and explicit `Resources` arguments.
- Use wrapping arithmetic wherever C can overflow.
- Do not add rendering, windowing, or UI dependencies to `engine/Cargo.toml`.
- Do not commit or modify `reference/` or `game-assets/`.
- Use `apply_patch` for edits and do not overwrite unrelated user changes.

## Tests And Handoff

Add focused tests covering successful rent, exact-cost failure, out-of-town
failure, artifact-discount pricing, cancellation while sailing, and ordinary
cancellation. Run:

```sh
cargo fmt --all --check
cargo test -p openbounty-engine --lib
cargo test -p openbounty-engine --test combat_digests
cargo test -p openbounty-engine --test render_free
```

When the slice is implemented and tested, report the changed files and results,
then ask the user for approval before committing. Suggested commit message:

```text
feat: support town boat rental
```
