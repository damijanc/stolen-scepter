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
- The town transaction cores: boat rental/cancellation, siege-weapon purchase,
  and spell purchase, with their town/location, already-owned, spell-cap, and
  exact-gold guards.
- Home-castle recruitment from the bottomless castle pool, and garrison
  transfers in both directions at an owned castle.
- The one-step location markers `in_town`, `home_castle`, `own_castle`, and
  the dwelling pair: cleared at the top of every step and re-set by the gate
  and dwelling paths, so a transaction is legal only in the step that arrived.
- Combat damage plus all 25 digest goldens.
- Headless combat end to end: seeding, battlefield setup (open-field bands,
  random obstacles, the fixed castle layout), movement and flight, the turn
  and round machinery, the AI policy, and `combat_run_headless` with its
  stall cutoff, spoils, and player-army writeback.
- Hostile foe encounters wired to that resolver: `Game::attack_hostile_foe`
  builds the target from the foe's garrison, keyed by its placement id, and
  routes the outcome through `resolve_hostile_foe`.
- Castle sieges, both kinds. `Game::siege_monster_castle` flips ownership,
  empties the garrison, and sets `own_castle` on a win.
  `Game::siege_villain_castle` gates capture on the active contract: without
  it the Lord is set free, keeps his castle, rebuilds from the pack roster,
  and is recorded in `villains_prefought`.
- `Game.oracle_mode`, the transient autoplay flag that keeps the legacy
  promote-on-capture behavior and the legacy reachable-flying-hero behavior.
  Real play promotes only at an audience and cannot be caught while flying.
- Foe following (`Game::foes_follow`): live in-zone foes chase the hero's
  PREVIOUS tile, refuse anything but plain grass (desert and bridges are
  barriers, which is what makes desert a refuge), keep clear of the ring
  around a castle gate, never stack, and report the collision that triggers
  the attack flow.

Test coverage is 177 engine unit tests, 25 combat digest goldens, and one
render-free boundary test.

The most recent slice is foe following. Run `git log --oneline -5`
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

The foe and combat systems are closed: foes wander, collide, and every
encounter resolves. The state model has been stable for several slices, so
JSON v8 save/load is the next target -- note it must cover the fields added
recently (`home_castle`, `own_castle`, castle `flow`, `economy.siege_cost`)
and must NOT serialize `oracle_mode`, which C keeps transient.

`Game::foes_follow` has no caller yet: C drives it from the step loop, so
wiring it into `game_step_basic` is a small follow-up worth doing first.

Two standing omissions:

- A villain siege's "Capture" dialog is built inline in C and pushed through
  `player_io_message`. The engine returns `VillainSiegeOutcome` instead so a
  shell can write the text; the message queue itself is unported.
- `combat.rs` deliberately omits what only a rendered fight needs: combat
  spells (`combat_cast_spell` and the `spell_*` helpers, reachable only
  through a player policy -- the headless default policy never casts) and
  `CombatTurnRecord` (write-only replay data for the shell's animation).
  Port those when a shell exists, not before.

Three habits these cores rewarded:

- Check the guard ORDER, not just the guard set. Each core reuses one result
  as a catch-all for illegal calls, so an out-of-town owner and a broke owner
  return different values purely because of the sequence C tests them in.
- Where a header comment and the C code disagree, port the code and say so in
  the test name. `GameKnownSpells` is documented as counting distinct spells
  but sums `counts[]`. `calc_distance` is documented as "squared-distance
  x 256" but actually scales by 2^24, because `isqrt32` already returns 16.16
  fixed point and `calc_distance` pre-shifts on top of it.
- When a test you just wrote fails, check the C before changing the code. Two
  combat tests failed on first run and both times the port was right and the
  expectation was wrong -- the second was an out-of-control stack targeting
  ITSELF, which C permits because it drops the self-skip along with the
  own-side skip.
- Resist tidying duplicated C into shared Rust helpers. The garrison and
  ungarrison slot scans look identical but compact differently afterwards, and
  a trait over both hid that. Translate the index scans as written.

Note that the town-flow callers live in `reference/src/views.c` (for example
`town_do_boat`), not in `flow_resolve.c` or `player_io.c`. Those callers hold
only dialogs; the state mutation belongs in the engine.

Other major gaps, in rough dependency order:

- Town/castle action flows, including home-castle recruitment.
- Full headless combat: setup, turn loop, AI, player-army writeback, and spoils.
- Foe following and collision-triggered flows.
- Fog and zone navigation flows.
- JSON v8 save/load compatibility and OpenBounty snapshot differentials.
