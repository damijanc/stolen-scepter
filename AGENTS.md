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
- v11 save read/write (`engine/src/savegame.rs`), guarded on an exact
  `version` match and on `pack_id`. A whole-struct round-trip test is the
  acceptance check -- keep it, it has already caught two real bugs.
- `save_read_header`, the slot-picker summary. It applies the version guard
  but deliberately NOT the `pack_id` guard: C lists a foreign-pack save and
  refuses it later, at load. `SaveHeader.pack_hash` exists because a C-written
  save carries one; the Rust engine never writes it.
- The save-shaped snapshot: `build_snapshot_full` emits `pack_id`, `boat`
  and `map_state` (hex fog rows per discovered continent, the ACTIVE zone's
  live fog winning over its stale per-continent snapshot). `pack_hash` is
  not emitted -- it hashes the pack zip, which the Rust engine does not load,
  and nothing gates on it.
- Fog of war (`engine/src/fog.rs`): a fixed 64x64 seen grid per continent,
  revealed by `game_step_basic` right after each move. `reveal` takes a
  radius and IGNORES it -- the original always stamps 5x5.
- Foe following (`Game::foes_follow`): live in-zone foes chase the hero's
  PREVIOUS tile, refuse anything but plain grass (desert and bridges are
  barriers, which is what makes desert a refuge), keep clear of the ring
  around a castle gate, never stack, and report the collision that triggers
  the attack flow. `game_step_basic` drives the pass on every step and
  surfaces a `FoeCollision` for the shell to prompt on; timestop freezes it.

Test coverage is 208 engine unit tests, 25 combat digest goldens, and one
render-free boundary test.

The most recent slice is the save-slot header reader. Run `git log --oneline -5`
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

The foe, combat, and save/load systems are all closed. Everything ported so
far is reachable from `game_step_basic`; there are no dangling engine cores
left to wire up, and no headless gap left that the C engine fills.

What remains is shell-facing, which makes the next decision a scoping one
rather than a porting one: the `player_io` message/prompt queue (the engine
returns outcome enums where C queues a dialog), and the two combat omissions
noted below. All three only pay off once a shell exists to consume them, so
consider selecting the shell before porting more of its scaffolding.

Traps the save slice turned up, worth knowing before touching that format:

- Castle garrisons name the troop `"troop"` (C's shared build/parse_unit_array
  pair); foe garrisons name it `"id"`, written inline. Both are C's shape.
- `Interact::as_str` is not injective: `None` and `Alcove` both encode as
  "none", so an alcove placement decodes as `None`. C has the same hole.
- A load upgrades `continent_fog[active]` to the live fog rather than
  preserving whatever the source game held there, so whole-struct round-trip
  equality needs that one slot accounted for.

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

The gap list that used to close this file -- town/castle flows, headless
combat, foe following, fog, and save/load -- is now entirely ported. Do not
re-add items here without checking `Current Engine State` first.
