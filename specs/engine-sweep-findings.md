# Engine Sweep Findings

Run against `7a70641` on 2026-10-01. Script: `tools/engine_sweep.sh`.
Totals: 283 declared, 204 HIT, 79 MISS, 34 after the host-prefix filter.

The plan's prototype (`f081b2c`) reported 203 / 80 and "36" non-host misses,
but its own list held 35. The one row that moved is `GameIsOver`, ported in
`4472380` while this audit was running; it is now a HIT and is not walked
below. The host filter drops 45 rows, all from `pack.h`, `player_io.h`,
`savepath.h` and `ui_host.h` (`ui_host.h` also covers `audio_play_tune`,
`open_dialog`, `recorder_capture` and `main_fast_quit_active`, which the
prefix list does not name but the `ui_host` match catches by header).

Shell line numbers below were read from the working tree; a concurrent session
had uncommitted edits in `shell/src/main.rs` and `AGENTS.md`, none of which
touch the cited lines.

## Name sweep: classified misses

| Header | C function | Class | Evidence |
|---|---|---|---|
| adventure.h | `adventure_handle_interact` | INLINED | Its per-kind dispatch is the `match tile.interactive` in `resolve_interaction`, `engine/src/step.rs:291`. The sign text it formats is HOST (`StepInteraction::Sign` carries title/body out). The flying gate and bounce-back that C's caller wraps around it are NOT ported -- see GAP rows 1 and 2. |
| adventure.h | `adventure_walkable_in_flight` | INLINED | C returns `true` for every in-bounds tile; Rust's `let can_move = if flying { true }`, `engine/src/step.rs:143`, after `tile_i32` already rejected off-map. |
| assets_bytes.h | `LoadAssetBytes` | HOST | Embedded-or-disk byte loader. Rust reads with `std::fs::read` / `read_to_string` at `engine/src/map.rs:112`, `engine/src/resources.rs:1133`. No game rule. |
| assets_bytes.h | `UnloadAssetBytes` | HOST | The free for the above; Rust owns the `Vec`. |
| combat.h | `combat_log_strings` | HOST | Returns `&res->combat_log` for template lookup. The shell resolves the same `strings.combat_log` keys with C's defaults, `shell/src/combat.rs:59`. |
| combat.h | `combat_log_template` | HOST | Formats a line into `c->log_lines` / `c->banner`. The engine records `CombatEvent` data instead (`AGENTS.md`); the shell formats `cloned` / `resurrected` / `immune` / `frozen` / `teleported` from it, `shell/src/combat.rs:102` and `:435`. |
| combat.h | `combat_run_headless_ex` | DIVERGENCE | Adds a `PlayerCombatPolicy` callback. Its only callers outside the `combat_run_headless` wrapper are `reference/demo/demo_brain.c:431`, `reference/autoplay/exec_fight.c:508` and `exec_replay.c:58`; demo/autoplay are OUT OF SCOPE per `specs/play-verification.md:436`. The default policy is `combat_ai_action`, which `combat_run_headless` (`engine/src/combat.rs:965`) uses for both sides. |
| fatal.h | `fatal_user_error` | DIVERGENCE | Engine caller is only the `zone_count > GAME_CONTINENTS` guard, `reference/engine/game.c:99`, recorded in `AGENTS.md` ("One known divergence", line 290). Every other caller is `reference/src/main.c` pack-loading UI. |
| flow_resolve.h | `flow_apply_accept_friendly` | PORTED-AS | `Game::resolve_friendly_offer`, `engine/src/game.rs:2828`: adds the troop on yes, always consumes; `consume_friendly_foe` (`:3489`) clears the stale tile and the foe's current tile, each only if it holds a foe stamp, as C does. |
| flows.h | `start_foe_hostile_flow` | GAP | See GAP row 3. Formats the attack yes/no (garrison listed with `GameNumberName`) and arms `FLOW_ATTACK_FOE`. The shell skips the question and starts combat, `shell/src/main.rs:1396`. |
| game.h | `GameApplyTileMutations` | PORTED-AS | `Map::apply_consumed`, `engine/src/map.rs:569`: same zone filter, same `clear_interactive`. Called from load (`map.rs:211`), zone switch (`game.rs:1231`) and save-load (`shell/src/main.rs:746`). |
| game.h | `GameFindCastleConst` | CONST-TWIN | `Game::find_castle`, `engine/src/game.rs:2480` (non-const C twin `GameFindCastle`, `game.c:2427`). |
| game.h | `GameFindFoeConst` | CONST-TWIN | `Game::find_foe`, `engine/src/game.rs:3464`, including C's current-zone scoping. |
| game.h | `GameTouchTown` | INLINED | Existing-slot path (mark `visited`) is `step.rs:329`. The fresh-slot path is unreachable: `salt_spells` eagerly fills a slot per catalog town (`game.c:443`, Rust `game.rs:2190`), and town tile ids are stamped from that catalog. The return value is ignored at C's only call, `step.c:138`. |
| game.h | `furnish_map` | DEAD | Body is `(void)g;` (`game.c:785`) and it has zero callers in `reference/`. |
| game.h | `player_accept_rank` | DEAD | Body is `(void)g;` (`game.c:859`); its one call, `game.c:241`, is a sequence marker. Rank promotion is `GameMaybeRankUp`, ported as `Game::maybe_rank_up`, `engine/src/game.rs:2505`. Not a stored-but-unwritten gap. |
| map.h | `MapGetTile` | PORTED-AS | `Map::tile_i32`, `engine/src/map.rs:223`: `None` off-map, as C returns `NULL`. |
| map.h | `MapInBounds` | INLINED | Every Rust caller tests bounds through `tile_i32` returning `None` (e.g. `Game::can_land_at`, `game.rs:1665`, porting `game.c:2516`). |
| resources.h | `resources_art_manifest` | DEAD | Callers are only `reference/tests/unit/test_art_manifest.c`. Asset-list tooling. |
| resources.h | `resources_castle_at` | DEAD | Zero callers in `reference/`. |
| resources.h | `resources_count_bucket_label` | DIVERGENCE | Used once, for Instant Army's `%QTY%` (`spells_adventure.c:205`). Recorded in place at `shell/src/interact.rs:576` ("DIVERGENCE: C renders %QTY% through its fuzzy count-bucket labels"). Presentation only. |
| resources.h | `resources_republish` | DIVERGENCE | Re-points the C global `g_resources`; callers are only `reference/autoplay/autoplay.c` (OUT OF SCOPE, `specs/play-verification.md:436`). Rust threads `&Resources` explicitly. |
| resources.h | `resources_town_at` | DEAD | Zero callers in `reference/`. |
| resources.h | `resources_zone_town` | GAP | See GAP row 7. C stamps the towns a zone's own `towns` array names; Rust stamps every catalog town whose `zone` field matches, `engine/src/map.rs:347`. |
| savegame.h | `SaveResultText` | HOST | Maps `SaveResult` to a toast / stdout reason. Rust has `SaveError` (`engine/src/savegame.rs:15`) and the shell prints it, `shell/src/main.rs:733`, `:1282`. |
| spells_adventure.h | `spells_adventure_reset_ui` | HOST | Resets C file-scope prompt globals (`bridge_state`, `gate_state`, `gate_mode`); callers are only autoplay. The Rust shell keeps that state as locals (`bridge_waiting`, `shell/src/main.rs:782`). |
| tables.h | `spell_index_by_id` | INLINED | `resources.spell_by_id(id)` then `.index`, e.g. the gate charge, `engine/src/game.rs:1472`. The bridge charge in `main.c:1285` uses the effect lookup, as the shell does (`spell_index_by_adventure_effect`, `shell/src/main.rs:913`). |
| tables.h | `troop_morale_groups` | DIVERGENCE | Only caller is `reference/autoplay/exec_recruit.c:341` (OUT OF SCOPE). |
| tile.h | `ArtBlocksFoot` | DEAD | Zero callers; `blocks_foot` comes from the pack's tile-code table in both ports (`map.rs` loader). |
| tile.h | `ArtIsBridge` | DEAD | Zero callers; `is_bridge` comes from the tile-code table. |
| tile.h | `InteractToString` | PORTED-AS | `Interact::as_str`, `engine/src/tile.rs:76`; same strings and the same `"none"` default (the `None`/`Alcove` collision `AGENTS.md` records). |
| tile.h | `TerrainMoveCost` | PORTED-AS | `Terrain::move_cost`, `engine/src/tile.rs:41` (1 / 40 / 0). C itself never calls it. |
| tile.h | `TerrainName` | PORTED-AS | `Terrain::as_str`, `engine/src/tile.rs:27`. C's `"?"` default is unreachable for a closed Rust enum. |
| tile.h | `TerrainWalkable` | PORTED-AS | `Terrain::walkable`, `engine/src/tile.rs:37` (grass or desert), used by `walkable_on_foot`, `step.rs:230`. |

## HIT spot-check: the five largest bodies

Ranked by line count across `game.c` and `step.c` among HIT rows:
`GameStep` (609), `GameInitSeeded` (293), `salt_continent` (162),
`GameFoesFollow` (136), `GameRollChest` (92).

### `GameStep` (`reference/engine/step.c:19`) vs `game_step_basic`

Six branches differ; each is a GAP row below.

- `if (!flying && nt->interactive != INTERACT_NONE)` (`step.c:125`) -- Rust
  calls `resolve_interaction` unconditionally (`step.rs:219`). GAP 1.
- `if (ir.bounce_back) { position = prev; travel_mode = prev; boat = prev; bounced = true; }`
  and `if (!bounced) { GameOnStep(...); GameFoesFollow(...); }`
  (`step.c:581`, `:591`) -- Rust never bounces, and runs `consume_step` and
  `foes_follow` BEFORE the interaction (`step.rs:199`-`219`). GAP 2.
- `start_foe_hostile_flow(game, ir.foe_id, nx, ny)` (`step.c:576`) -- GAP 3.
- `start_foe_*_flow` on `collided_idx >= 0` (`step.c:604`) -- the engine
  reports `collision`, but the shell discards it with `..`
  (`shell/src/main.rs:1299`). GAP 4.
- `FogReveal(fog, map, dst->x, dst->y, ...)` after a telecave jump
  (`step.c:487`) -- `Game::resolve_telecave` moves the hero but nothing
  reveals the destination. GAP 5.
- `cr_mut->visited = true;` is the only castle write on a gate step
  (`step.c:184`); Rust also sets `castle.known = true` (`step.rs:311`). GAP 6.

One more, pack-edge only: C clears an artifact tile only
`if (a && GameClaimArtifact(...))` (`step.c:162`); Rust consumes it
unconditionally (`step.rs:351`). This differs only for an artifact id the
catalog lacks, which no local pack has. GAP 14.

Matched: the step-start marker clears, edge/terrain blocking, boarding
(current-zone boat only), facing, boat drift/disembark, fog reveal before the
interaction, the castle audience/own/monster/villain ladder (prompts in the
shell), dwelling markers (`"hills"` normalises to C's `"hill"`,
`game.rs:3947`), alcove `knows_magic` split (shell, `main.rs:1331`), chest,
orb and navmap consumption.

### `GameInitSeeded` (`reference/engine/game.c:87`)

RNG call order matches: scepter key `(0,255)`, continent `(0,3)`,
`bury_scepter`, `salt_spells`, alcoves, `salt_continent` per zone, castles,
`salt_villains`, `repopulate_castle`, then the hash-based dwellings
(`init_seeded_inner`, `engine/src/game.rs:755`). Every step-by-step block
matches except these, none of them reachable with the three local packs:

- The capital-letter fix sits inside `if (name && name[0])` (`game.c:154`),
  so the pack's `world.default_name` is copied as written. Rust capitalises
  on both paths (`capitalize_first_ascii`, `game.rs:3653`). GAP 8.
- `enforce_dwelling` creates the row BEFORE looking up the troop
  (`d = &g->dwellings[g->dwelling_count++]; ... if (t) {...}`,
  `game.c:1822`-`1834`), so a kind with no matching troop leaves an empty
  row. Rust returns `None` and adds nothing (`dwelling_troop_at(...)?`,
  `game.rs:2041`), which changes `dwelling_count`, the save and the
  snapshot. Reached by declared `zones[].dwellings[]`; no local pack declares
  any. GAP 9.
- `if (!troop || count <= 0) continue;` (`game.c:262`) tests a `char[]`, so
  `!troop` is never true and an id-less starting troop with `count > 0` is
  written as `{"", count}`. Rust skips it (`game.rs:3768`). Malformed pack
  only. GAP 10.
- C's trailing placement sweep calling `enforce_dwelling` per dwelling
  placement (`game.c:365`-`376`) has no Rust counterpart. Every such
  placement was already pinned by `salt_continent` (`game.c:760`, Rust
  `game.rs:1895`), so the C loop only ever finds the existing row: DEAD in
  effect, not a gap.

### `salt_continent` (`reference/engine/game.c:621`)

Every RNG draw matches in order and bounds, including `roll_hostile_garrison`
(`game.rs:2131`), the `TAG_N` barrel loop (`game.rs:2166`) and
`salt_pick_dwelling_troop`'s `(hi-lo)*20+20` guard (`game.rs:1929`). One
difference, malformed pack only:

- `if (barrel_len < min_len)` (`game.c:655`) compares signed ints; Rust
  casts the budget sum with `min_len as usize` (`game.rs:1816`). A negative
  salt budget (e.g. `artifacts: 2, navmaps: -5`) makes C tag the artifacts
  (drawing RNG) and Rust place nothing. GAP 11.

C's `artifact_by_index` NULL fallback (`game.c:722`) is dead -- the index
was found by the same lookup -- and the skipped-zone stdout line is a
diagnostic.

### `GameFoesFollow` (`reference/engine/game.c:2557`)

No difference that changes behaviour. Order (index 0..`foe_count`), alive/zone
filters, the 2-tile Chebyshev range around `last_x/last_y`, the dy-outer /
dx-inner 3x3 scan, the flying-hero skip unless `oracle_mode`, the boat
exclusion from `hero_reachable`, `foe_can_stand`'s guard order, strict-less
tie-breaking, the guarded clear on the old tile and the last-collider return
all match (`engine/src/game.rs` `foes_follow`, `foe_can_stand`,
`adjacent_to_castle_gate`). Two differences that cannot be reached:

- C stamps the destination raw (`dst->interactive = INTERACT_FOE;`,
  `game.c:2677`); Rust calls the guarded `Map::stamp_foe`. The destination
  already passed `foe_can_stand` (`interactive == None`) this pass, so the
  guard never refuses.
- Rust's `Map::clear_interactive` (`map.rs:534`) also blanks `sign_title`,
  `sign_body` and `boat_spawn_*`, which C's `MapClearInteractive` leaves.
  Only foe-stamped tiles reach it here, and those fields are empty on them.

### `GameRollChest` (`reference/engine/game.c:1620`)

The six outcomes (gold, commission, spell power, max spells, new spell,
empty), `chest_rand`'s constants, the in-roll writes versus the deferred
`accept_chest_gold` / `accept_chest_leadership`, and `GamePeekChest`'s
read-only clone all match (`engine/src/game.rs:971`-`1112`, `:3867`).
Differences:

- `int zi = (zone_index >= 0 && zone_index < 4) ? zone_index : 0;`
  (`game.c:1627`, `:1594`) -- Rust `zone_index.min(3)` (`game.rs:1000`), so
  zone 4+ reads tier 3 instead of tier 0. Needs a 5-zone pack, which C
  refuses at `GameInit` (the recorded `GAME_CONTINENTS` divergence). GAP 12.
- With a non-positive gold roll C still shows the gold banner as a plain
  message (`step.c:450`); the shell shows nothing (`chest_choice` and
  `chest_message` both return `None`, `shell/src/interact.rs:181`, `:237`).
  Needs `gold_min <= -gold_max`. GAP 13.
- `g->spells.counts[spell_type] += spell_num;` (`game.c:1699`) indexes a
  14-slot array by a catalog index up to 31: undefined behaviour in C for a
  pack with a fifteenth spell. Rust's 32 slots credit a real slot. Nothing
  defined to match; not a gap.

## Field audit

Scope: the `pub` fields of `Game` and every struct it embeds (`ScepterState`,
`Character`, `ClassState`, `Stats`, `Position`, `BoatState`, `ArmyStack`,
`Spellbook`, `Contract`, `ArtifactState`, `TownRecord`, `CastleRecord`,
`TileMutation`, `SaltedPlacement`, `DwellingState`, `FoeState`, `Unit`,
`WorldProgress`) plus `Fog::seen`: 121 struct-field pairs. Reads and writes were counted
over a copy of `engine/src` and `shell/src` with every `#[cfg(test)] mod tests`
cut off and `savegame.rs` / `state_serialize.rs` removed. Writer FUNCTIONS
were then listed per field on both sides (C over `reference/engine` and
`reference/src`, minus `savegame.c` / `state_serialize.c`) and compared by
hand, which catches a writer one side has and the other lacks -- the shape
of GAP 6 (`castles[].known`).

Generic names (`x`, `y`, `id`, `zone`, `count`, `name`, `kind`, `cls`) were
not counted per field; they are read and written by every system.

| Field | Shape | Class | Evidence |
|---|---|---|---|
| `Stats.followers_killed` | read, never written | GAP | C adds the player's losses on every hit, `c->heroes[t_side]->stats.followers_killed += kills;` (`combat.c:486`, "Step 10"). Rust's `combat_deal_damage` goes from the stack-death branch straight to the ABSORB/LEECH effects (`engine/src/combat.rs:1323`-`1332`). `compute_score` subtracts `kill_penalty * followers_killed` (`game.rs:1752`, C `game.c:1536`) and the Character view shows it (`shell/src/views.rs:2558`), so both always read 0. GAP 15. |
| `CastleRecord.known` | extra writer | GAP | Already GAP 6: C's only gameplay writer is `GameCastFindVillain` (`game.c:438`); Rust adds `step.rs:311`. |
| `Contract.villains_prefought` | written, never read | C-SAME | C writes it at `flow_resolve.c:181` and reads it nowhere outside persistence; Rust `game.rs:3120`, same. |
| `Game.seed_from_catalog` | written, never read | C-SAME | C `game.c:122`/`126`/`140`; Rust `game.rs:768`-`776`. Persisted only. |
| `Stats.last_commission` | written, never read | C-SAME | C `game.c:191`, `:1050` (header: "for UI", but no UI reads it); Rust `game.rs:3408`, `:3702`. |
| `Position.facing_left` | written, never read | C-SAME | C `step.c:94`; only its unit test reads it. Both renderers mirror from `facing`. |
| `Stats.score` | never written | C-SAME | Neither port writes it; both compute the score on demand (`GameComputeScore` / `Game::compute_score`) and only persist the stale field. |
| `Game.oracle_mode` | read, never set | DIVERGENCE | C sets it only under `--autoplay` (`reference/src/main.c:933`); autoplay is OUT OF SCOPE (`specs/play-verification.md:436`). Real play leaves it false in both. |
| `ScepterState.continent` | Rust-only | -- | C keeps the scepter continent in a local (`game.c:150`); Rust stores it but never serializes it. No parity effect. |
| `hud_visible` | C-only | C-SAME | DEAD in C, recorded at `specs/play-verification.md:435`. The Rust snapshot also omits it. Do not wire it up. |
| `anim_frame`, `anim_moving` | C-only | HOST | Presentation clocks read only by `reference/src/map_render.c:69`, `:133`; the shell owns them (`shell/src/anim.rs`). |
| `player_io`, `version` | C-only | HOST | The text queue (unported by design, `AGENTS.md`) and the save version constant the v11 guard already checks. |

All other fields had matching writer sets once multi-line `.copy_from_str`
chains and `snprintf` writes were accounted for. Checked by hand because the
counts looked thin: `own_castle` (C also sets it on a siege win,
`flow_resolve.c:113`, `:159`), the weekly astrology garrison growth
(`game.c:1092`-`1104`, Rust `game.rs:3445`, `:3455`), combat spell charges
(`combat.rs:2828`), `spell_for_sale` (C's extra `GameTouchTown` writer is the
unreachable fresh-slot path), and `game_over` (C's extra writers are a
`player_io_answer` local and autoplay's win hand-off, `main.c:1271`).

## Proposed slices

Only `GAP` rows. Ordered by how often normal play reaches them.

| # | Slice | C source | State touched |
|---|---|---|---|
| 1 | `fix: skip tile interactions while flying` | `GameStep`, `step.c:125` | Every interaction: chests, artifacts, orbs, navmaps (consumed), towns/castles (`in_town`, `home_castle`, `own_castle`, `visited`), dwellings, foes |
| 2 | `feat: bounce the hero off towns, castles, dwellings, alcoves and hostile foes` | `GameStep` bounce-back, `step.c:581`-`591`; `bounce_back` set in `adventure.c` | `position`, `travel_mode`, `boat`; whether `consume_step` (days/weeks) and `foes_follow` run, and their order relative to the interaction |
| 3 | `feat: ask before attacking a hostile foe` | `start_foe_hostile_flow`, `flows.c:228` | None in the engine (`attack_hostile_foe` already takes the resolved yes); the shell prompt and a decline path |
| 4 | `feat: start the encounter when a foe walks onto the hero` | `GameStep`, `step.c:604` | None in the engine (`StepResult::Moved.collision` already reported); shell routing to the friendly or hostile flow |
| 5 | `fix: reveal fog at the telecave destination` | `GameStep`, `step.c:487` | `Fog` of the active zone |
| 15 | `fix: count the player's combat losses in followers_killed` | `combat_deal_damage` Step 10, `combat.c:486` | `Stats.followers_killed` (read by `compute_score` and the Character view); needs a per-fight tally in `Combat` written back to the `Game`, since the Rust combat holds no `&mut Game` |
| 6 | `fix: stop marking castles known on a gate visit` | `GameStep`, `step.c:184`; sole C writer `GameCastFindVillain`, `game.c:438` | `castles[].known` (saved, snapshotted, read by the contract view `shell/src/views.rs:3112`) |
| 7 | `fix: stamp zone towns from the zone's town list` | `resources_zone_town`, `map.c:141`; the zone-town index loop in `parse_zones`, `resources.c:353` | `Resources` zone town index list; tile stamping order. Unreachable with the three local packs: their zone lists and catalog `zone` fields agree in set and order |

Rows 8-14 are unreachable with every local pack (stock, open, hd). They are
real parity differences for a hand-made pack, small enough to bundle into one
`fix: match C on pack edge cases` slice if wanted.

| # | Slice | C source | State touched |
|---|---|---|---|
| 8 | `fix: leave the pack default name uncapitalised` | `GameInitSeeded`, `game.c:154` | `character.name` |
| 9 | `fix: keep an empty dwelling row when no troop matches` | `enforce_dwelling`, `game.c:1822` | `dwellings[]`, `dwelling_count` (saved, snapshotted) |
| 10 | `fix: keep an id-less starting troop` | `GameInitSeeded`, `game.c:262` | `army[]` |
| 11 | `fix: compare salt budgets signed` | `salt_continent`, `game.c:655` | `placements[]`, RNG stream |
| 12 | `fix: clamp chest zone tiers to zero` | `GameRollChest` / `GamePeekChest`, `game.c:1594`, `:1627` | Chest outcome for zone 4+ |
| 13 | `fix: show the gold banner for a non-positive chest roll` | caller `step.c:450` | None (shell message) |
| 14 | `fix: leave an unclaimable artifact on the map` | `GameStep`, `step.c:162` | Tile, `consumed[]` |
