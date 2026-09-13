# OpenBounty Rust Parity Port Plan

## Purpose

Build a faithful, parity-first Rust port of OpenBounty's engine before adding a
modern shell. The engine stays render-free from day one, and each step should
add tests that lock in OpenBounty behavior before any idiomatic Rust refactor.

## Current Baseline

The repository now has:

- A Cargo workspace with `engine` and `shell` crates.
- A render-free `openbounty-engine` crate.
- Bit-exact RNG tests for `GameSeedFromIndex` and the OpenBounty LCG.
- A C-shaped combat damage/digest port.
- All 25 OpenBounty combat digest goldens passing.
- A simple dependency boundary test that rejects renderer/UI crates in
  `engine/Cargo.toml`.
- `NOTICES.md` crediting OpenBounty and documenting the asset policy.

## Guiding Rules

- Parity target is OpenBounty, not the DOS binary.
- Transliterate first, refactor later.
- Keep structs and arrays fixed-size where OpenBounty does.
- Preserve biased modulo RNG behavior.
- Use `wrapping_*` for parity-sensitive multiplication.
- Keep the engine crate free of rendering, input-windowing, and UI dependencies.
- Every behavior change needs a test or golden artifact before cleanup.

## Milestone 1: Resources And Catalog Tables

Goal: replace the temporary static troop table path with an OpenBounty-style
resource/catalog loader while keeping combat digest tests green.

Planned work:

- Add `engine/src/resources.rs`.
- Represent the needed subset of `game.json` in C-shaped Rust structs.
- Load troop definitions from JSON into fixed-capacity arrays.
- Move `troop_by_id` and `troop_by_index` behind a catalog/resource state.
- Keep the existing static table only as a test fixture or remove it once the
  loader is proven.
- Add tests that loaded troop records match the current combat fixture values.
- Keep all 25 combat digest goldens passing through the loaded catalog.

Acceptance criteria:

- `cargo test --workspace` passes.
- Combat digest tests still use public engine APIs.
- Engine has no graphics or shell dependency.
- Missing or malformed troop data fails clearly.

Open review questions:

- Should the repository include a minimal OpenBounty-derived `game.json` fixture
  containing catalog data only, or should tests load a copy under
  `tests/fixtures/` created specifically for parity tests?
- Should resource loading be explicit via a `Resources` handle passed to lookups,
  or temporarily global like OpenBounty's `resources_current()` while
  transliterating?

Recommended decision:

- Use an explicit `Resources` handle for new Rust APIs, but keep a narrow
  compatibility layer for transliterated code if it substantially reduces churn.

## Milestone 2: Game State Skeleton And World RNG

Goal: introduce the main `Game` state shape and port enough initialization to
preserve OpenBounty RNG call order.

Planned work:

- Add `engine/src/game.rs`.
- Port the relevant `Game` sub-structs as fixed-size Rust structs.
- Add world RNG snapshot/restore equivalents.
- Port `GameInitSeeded` in phases, starting with seed resolution and scepter RNG
  calls.
- Add tests for seed index resolution and early RNG call order.

Acceptance criteria:

- `seed_from_index` remains the sole catalog seed expansion.
- `min >= max` RNG paths do not advance state.
- Scepter key and continent rolls match OpenBounty for selected seed indices.

## Milestone 3: State Serialization Snapshot Harness

Goal: compare Rust initialization output against OpenBounty-style JSON snapshots.

Planned work:

- Port the subset of `state_serialize` needed for initialization snapshots.
- Define deterministic snapshot tests at fixed seed indices.
- Normalize fields that are intentionally absent until later milestones.
- Add fixtures documenting the exact expected snapshot shape.

Acceptance criteria:

- Snapshot tests are deterministic and stable.
- Snapshot fields are tied to OpenBounty names where possible.
- New initialization fields are added with tests as they are ported.

## Milestone 4: Map, Fog, Step, And Adventure Flow

Goal: port overworld mechanics in dependency order.

Planned work:

- Port map tile structures and map loading.
- Port fog state and visibility updates.
- Port `step_try` movement bookkeeping.
- Port adventure tile interaction dispatch.
- Add focused tests for each mechanic before end-to-end flows.

Acceptance criteria:

- Engine remains render-free.
- Movement/fog tests cover edge coordinates and blocked moves.
- RNG call order is not changed by map/fog refactors.

## Milestone 5: Full Headless Combat

Goal: expand from damage/digest parity to full headless combat parity.

Planned work:

- Port combat setup, battlefield reset, obstacles, unit turn selection, AI, and
  headless resolution.
- Add deterministic tests for obstacle placement, AI target choice, no-progress
  cutoff, and win/loss outcomes.
- Keep digest tests as the damage-formula ratchet.

Acceptance criteria:

- Headless combat can run without shell code.
- Known fixed encounters produce stable outcomes.
- Combat state mutations are covered before renderer work begins.

## Milestone 6: Save/Load And JSON Version 8

Goal: port OpenBounty save compatibility after core state exists.

Planned work:

- Port save structs and JSON read/write.
- Add round-trip tests.
- Add golden fixture tests for OpenBounty save format version 8.

Acceptance criteria:

- Rust can read known OpenBounty saves for supported fields.
- Rust write/read round trips preserve engine state.
- Unknown or future versions fail clearly.

## Milestone 7: Shell Selection And Integration

Goal: choose the modern shell only after engine parity is protected.

Planned work:

- Evaluate macroquad, Bevy, and Tauri only against the engine API after the
  headless harness exists.
- Keep renderer dependencies in `shell`, never `engine`.
- Build shell from engine snapshots and commands, not duplicated logic.

Acceptance criteria:

- Engine tests pass without shell dependencies installed.
- Shell can be replaced without changing engine behavior.
- The render-free boundary check remains in CI.

## Near-Term Commit Plan

1. `feat: load troop catalog resources`
2. `test: lock loaded troop catalog parity`
3. `feat: add game state seed initialization`
4. `test: add world rng initialization vectors`
5. `feat: add initial state snapshot serialization`

## Review Checklist

- Does the milestone order match the parity risks?
- Are the acceptance criteria specific enough to reject accidental divergence?
- Should OpenBounty reference files be kept as a submodule, copied fixtures, or
  temporary developer-only clones?
- Is the explicit `Resources` handle approach acceptable, or should early code
  stay closer to OpenBounty's global lookup style?
