# OpenBounty Rust Port Guide

## Mission

This repository is a parity-first Rust port of the MIT-licensed
[OpenBounty](https://github.com/dannyheskett/openbounty) engine, not a port of
the original DOS executable. OpenBounty's current C behavior and published
outputs are authoritative. Translate first; refactor only after a parity test
protects the behavior.

The engine crate must remain completely render-free. The shell is a stub and
will be selected only after headless engine parity is solid.

## Layout

- `engine/`: `openbounty-engine`, the only place for game rules and state.
- `shell/`: `openbounty-shell` stub; do not add engine logic here.
- `engine/tests/combat_digests.rs`: 25 OpenBounty combat digest goldens.
- `engine/tests/render_free.rs`: rejects renderer and UI dependencies in the
  engine manifest.
- `specs/parity-port-plan.md`: original plan; useful background, but its early
  milestones have largely been completed.
- `specs/manual-reference-notes.md`: policy and useful facts from local game
  materials.
- `reference/`: local, ignored clone of OpenBounty. Read it for source parity;
  never commit it.
- `game-assets/`: local, ignored manuals and game binaries. Use as secondary
  evidence only; do not commit assets or binaries.
- `NOTICES.md`: licensing and asset policy. Preserve OpenBounty attribution.

## Current Engine State

Implemented and tested:

- SplitMix64 catalog seed expansion and the full-width Java-constant LCG.
- JSON resources in fixed-capacity structures; world initialization, salted
  placements, scepter placement, contracts, castles, spells, and dwellings.
- Compact state snapshots with OpenBounty-style field names.
- Map loading, terrain and boat/flying movement, interactions, and tile
  mutation persistence.
- Chests, artifacts, orbs, navmaps, telecaves, magic alcoves, dwellings, and
  dwelling recruitment.
- Friendly foe offers/acceptance and hostile foe post-combat resolution.
- Weekly commission/upkeep/boat handling and astrology effects.
- Town boat rental/cancellation and siege-weapon purchase, with the
  town/location, already-owned, and exact-gold guards.
- Combat damage plus all 25 digest goldens. Full battle setup, turns, AI, and
  headless encounter resolution are still unported.

Test coverage is 86 engine unit tests, 25 combat digest goldens, and one
render-free boundary test.

The most recent slice is the town siege purchase. Run `git log --oneline -5`
for the exact tip rather than trusting a hash written here; the working tree
should be clean before you start.

## Parity Rules

- Preserve fixed-size arrays, index scans, and explicit state threading where
  the C source has them.
- Keep RNG call order exact. `min >= max` must return without advancing state.
- Use `wrapping_*` for parity-sensitive arithmetic, especially multiplication.
- Preserve OpenBounty's modulo bias. Never replace it with a uniform sampler.
- Do not invent gameplay fixes based on the original game or manual. Use the
  manual only to validate data and user-facing facts.
- Treat a comment as secondary to executable C behavior when they disagree.
  Example: `GameGrowDwellings` exists, but the current week-end transaction
  replenishes only dwellings matching the astrology troop.
- Keep presentation, prompts, graphics, and input routing outside `engine`.
  Engine methods should accept resolved decisions/results and return state or
  small outcome values for a future shell to present.

## Reference Workflow

Before implementing a slice, inspect the relevant C functions and adjacent
callers in `reference/engine/`. Prefer the flow-apply cores for game state
effects and separate host/prompt code from engine behavior. Add focused Rust
tests that state the observed C rule, then run:

```sh
cargo fmt --all --check
cargo test -p openbounty-engine --lib
cargo test -p openbounty-engine --test combat_digests
cargo test -p openbounty-engine --test render_free
```

`cargo test --workspace` should also pass, but the focused commands above make
failures easier to attribute. Do not add renderer/UI crates to
`engine/Cargo.toml`.

## Git And Editing

- Preserve unrelated changes. Do not reset, checkout, or clean the worktree.
- Use `apply_patch` for hand edits.
- Keep commits focused and use conventional messages such as `feat:` or
  `test:`.
- Commit messages are a single subject line only. No body paragraphs, no
  bullet lists, and no `Co-Authored-By` or other attribution trailers.
  Example: `feat: support town boat rental`.
- The user wants approval before every commit. Complete a tested slice, report
  it, and ask before committing.
- Never commit `reference/`, `game-assets/`, or generated build output.

## Good Next Work

The most coherent next slice is `GameBuySpell`, the last of the town action
cores beside the boat and siege functions in `reference/engine/game.c`. Then
home-castle recruitment.

When porting a town core, check the guard ORDER, not just the guard set. The
boat and siege cores both reuse their "no gold" result as a catch-all for
illegal calls, so an out-of-town owner and a broke owner return different
values purely because of the sequence the C tests them in.

Note that the town-flow callers live in `reference/src/views.c` (for example
`town_do_boat`), not in `flow_resolve.c` or `player_io.c`. Those callers hold
only dialogs; the state mutation belongs in the engine.

Other major gaps, in rough dependency order:

- Town/castle action flows, including home-castle recruitment.
- Full headless combat: setup, turn loop, AI, player-army writeback, and spoils.
- Foe following and collision-triggered flows.
- Fog and zone navigation flows.
- JSON v8 save/load compatibility and OpenBounty snapshot differentials.
