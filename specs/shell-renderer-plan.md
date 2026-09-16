# Shell And Renderer Plan

Status: agreed 2026-09-16. The engine port is complete (303 unit tests, 25
combat digest goldens, one render-free boundary test); this document covers
the shell, which is currently a three-line stub.

Read `AGENTS.md` first. Everything there still applies -- parity-first
translation, one-line commit messages, approval before every commit.

## What the engine already gives you

Verified by running the real pack (`reference/assets/kings-bounty/game.json`,
loaded with `Resources::from_json_file` so `resource_root` resolves the map
paths) through 20,000 steps on the real 64x64 continentia map:

```
hero="Sir Rust" class="knight" zone="continentia" pos=(11,58) gold=7500 days=900
army: [("militia", 20), ("archers", 2)]
scepter buried at (22,55)
moved=20000  days_left=400  gold=98253
interactions: CastleGate 99, Town 48, Dwelling 28, Foe 9480,
              Sign 361, Telecave 46, TreasureChest 6
```

So the rules run. Note that this smoke test is NOT committed: it depends on
`reference/`, which is gitignored, so CI could not run it. Every parity claim
in the repo rests on synthetic fixtures. Worth fixing if a redistributable
pack ever exists.

## Decisions

### 1. Architecture: a draw-command layer

Split the shell in two, and keep the seam strict:

- **View/layout logic** takes engine state and emits a list of draw commands
  in LOGICAL coordinates (`Blit { sprite, src, dst }`, `Rect`, `Text`). Pure
  Rust, no graphics dependency, headlessly testable -- assert on the command
  list the way `engine/tests/combat_digests.rs` asserts on damage.
- **Backend** executes that list. A few hundred lines. Swappable.

About 9,500 of the C shell's ~10,000 lines are view/layout logic. This is
where a resolution change or an art overhaul lands, so the seam is what makes
those cheap later. It is the engine/shell discipline one layer down.

### 2. Backend: macroquad, with wgpu as the escape hatch

Pure Rust (no C toolchain), GPU-accelerated, arbitrary resolution, render
targets and shaders available for later effects, exports to web.

Rejected, with reasons, so this is not relitigated from scratch:

- **CPU framebuffer** (`pixels`/`softbuffer` + `winit`). Tempting: the legacy
  design IS a 320x200 buffer blitted at integer scale, and it would make
  frames golden-testable. Rejected because it is the one option that blocks
  the stated goal of higher resolution and a graphics overhaul -- at 1080p
  with alpha, rotation or effects you are writing a software rasteriser. The
  draw-command layer recovers the testability without the ceiling.
- **raylib-rs**. The reference uses raylib, so `views_render.c` would
  transcribe almost literally. Rejected: a C dependency, and the API-affinity
  argument turned out to be worth little. The whole C shell uses about six
  drawing primitives across ~90 call sites: `DrawRectangle` (43),
  `DrawTexturePro` (30), `DrawRectangleLines` (12), three rounded-rect calls,
  three `DrawText`. The bulk of the port is layout arithmetic, not API calls.
- **SDL2**. Mature, but a C dependency without even the reference's API
  affinity to show for it.
- **wgpu + winit** directly. The most future-proof and the most actively
  maintained, but meaningfully more work before anything is on screen. With
  the draw-command layer in place this is a backend swap, not a shell
  rewrite, so it is deferred rather than ruled out.

Do NOT add a graphics dependency to `engine/Cargo.toml`.
`engine/tests/render_free.rs` enforces that.

### 3. Fidelity: legacy first, but never hardcode its geometry

Reproduce the legacy layout faithfully first, because it is the only mode
with a reference build to check against. But drive all geometry from the
pack's `render` block, exactly as C's `layout_init` does for its ~340 `CL_`
macro call sites. No literal `320` may reach view code.

Then `"mode": "modern"` -- square tiles, larger viewport, already a supported
pack value -- is a config flip plus an art swap, not a port.

## Reference map

| Concern | C source | Rust target |
| --- | --- | --- |
| Geometry constants + derivation | `src/layout.h`, `src/layout.c` | `shell/src/layout.rs` |
| `render` block parsing | `engine/resources.c` ~2473 | `engine/src/resources.rs` |
| Main loop | `src/main.c` (1421) | `shell/src/main.rs` |
| Town/castle/dwelling/alcove screens | `src/views.c` (950), `src/views_render.c` (887) | `shell/src/views/` |
| Combat loop + render | `src/combat_loop.c` (714), `src/combat_render.c` (212) | `shell/src/combat/` |
| Map render | `src/map_render.c` (206) | `shell/src/map_render.rs` |
| Prompts / dialog | `src/prompt.c` (309), `src/encode_dialog.c` (213) | `shell/src/prompt.rs` |
| Chrome / overlay / sprites | `src/chrome.c`, `src/overlay.c`, `src/sprites.c` | `shell/src/chrome.rs` etc. |
| Integer-scale present | `src/present.c` | backend |

## Legacy geometry, for checking the port

`layout_init` derives everything from the render block:

```
map_w     = tile_w * tiles_w          48 * 5 = 240
map_h     = tile_h * tiles_h          34 * 5 = 170
sidebar_w = tile_w                    48
screen_w  = 16 + map_w + sidebar_w + 16       = 320
screen_h  = 8 + 9 + 5 + map_h + 8             = 200
```

Fixed chrome bands, in pixels, which do NOT scale with tile size (they are
text and border furniture):

```
FRAME_TOP_H 8   FRAME_BOTTOM_H 8   FRAME_LEFT_W 16   FRAME_RIGHT_W 16
BAR_H 5   BAR_Y 17
STATUS_X 16   STATUS_Y 8   STATUS_W screen_w-32   STATUS_H 9
MAP_X 16   MAP_Y 22 (= 8 + 9 + 5)
SIDEBAR_X MAP_X+MAP_W   SIDEBAR_Y MAP_Y   SIDEBAR_H MAP_H
BOTTOM_Y MAP_Y+MAP_H (192)
PANEL_X MAP_X-5   PANEL_W MAP_W+5   PANEL_H 68   PANEL_Y MAP_Y+MAP_H-PANEL_H
PANEL_PAD_X 4   PANEL_COLS 30
```

The panel is 5px wider than the map, borrowed from the decorative left frame,
so a 30-character line fits with a margin. `DrawRectangleLines` takes 1px per
edge, so the map's own 240px left only 238px of interior against 30*8 = 240px
of cells. The margin is one-sided by construction: the right edge is pinned
to the sidebar.

Presentation scale: `default_scale` is 2 for legacy (320x200 is tiny) and 1
for modern (already large; 800x702 at 2x exceeds 1080p). Clamps are
`SCALE_MIN 2`, `SCALE_MAX 5` desktop, `SCALE_MAX_WEB 3`.

## Parity traps in the render block

- `render.mode` is REQUIRED. A pack declaring neither `legacy` nor `modern`
  is REJECTED at load, not guessed at, because legacy art is drawn for a
  48x34 tile and modern art for a square one. This is a load-time contract
  like `tuning.temp_death`.
- Legacy IGNORES any declared `tile_w`/`tile_h`/`tiles_w`/`tiles_h` and
  forces 48/34/5/5. Only modern reads them, defaulting to 96/96/7/7. A pack
  that sets `tile_w` in legacy mode gets no effect from it.
- The viewport must be ODD on both axes and at least 3x3, with tiles at least
  8px. `map_render` centres the hero with `RADIUS = tiles / 2`; an even count
  leaves him half a tile off.

## Engine state the renderer needs

Starting the renderer surfaced engine fields the port had dropped because
nothing in a render-free engine read them. They are not presentation-only:
C sets them in engine code and writes them to saves, so they belong here.

- `Position.facing` / `Position.facing_left` -- DONE. Set by `step`, carried
  in saves. A save written before four-way facing existed carries only the
  mirror flag, so the loader derives the facing from it rather than bumping
  the save version. The Rust port was writing neither field, which made its
  v11 saves narrower than C's v11.
- `Tile.art` -- DONE. The sprite filename base ("water", "castle_gate"),
  which `map_render` draws with via `tile_cache_get(t->art)`. The Rust `Tile`
  kept only `terrain`, and the legend is not retained, so the name was
  unrecoverable. It is not derivable either, in two separate ways: an
  artifact and a chest on grass share a terrain and not a sprite, and the
  shipped pack's legend maps 5 terrains onto 60 art names -- every coastline
  and forest/desert/mountain edge variant. A terrain-only renderer would have
  drawn flat blocks of colour with no edges at all.

  Verified against the shipped pack: loading every zone yields 72 distinct
  art names, all 72 have a PNG in `art/tiles/`, and none is empty. That check
  is not committed, for the same reason as the smoke test above.

Both were invisible to the sweep methods in `AGENTS.md`, which compare
function names and body sizes. Neither catches a missing struct FIELD. When
checking a struct, compare it field by field against its C original.

`Game.anim_frame` is genuinely the shell's: C declares it in `game.h` but
nothing in the engine reads or serializes it.

## Asset inventory

- 311 RGBA PNGs under `art/`: `tiles`, `troops`, `villains`, `classes`,
  `combat`, `sprites`, `ui`, `font`.
- Font is `art/font/kb-font.png`, a single 1024x8 RGBA strip -- 128 glyphs of
  8x8. Drawn via `DrawTexturePro`, not raylib's `DrawText` (only 3 of those
  in the whole shell).
- `palettes/palette.bin`.
- Audio under `audio/`.

## Pack sections the engine ignores

These are parsed by nobody today and are the shell's to claim:

`strings` (20 groups; C's `resources.h` declares 143 banner strings),
`render`, `colors`, `controls`, `audio`, `sprites`, `title`, `credits`,
`ending`, `pack_name`, `pack_kind`.

The engine deliberately returns outcome VALUES where C queues TEXT --
`AdventureCast`, `SearchOutcome`, `SpellCast`, `VillainSiegeOutcome` and the
rest. So the shell owns a banner/template layer that does not exist yet,
including `resources_format_template` and its `%VAR%` substitution.

## Work order

1. **Layout model.** Parse `render` in `engine/src/resources.rs`; port
   `layout_init` and the `CL_*` geometry to `shell/src/layout.rs`. No
   graphics dependency yet. DONE.
2. **Draw-command vocabulary.** DONE: `shell/src/draw.rs` (`DrawCmd`,
   `DrawList`, `Sprite`, `Color`, `Rect`, `Dst`), `shell/src/font.rs` and
   `shell/src/palette.rs`. `DrawList` is itself the headless recorder; there
   is nothing to execute it until step 3. Findings from reading every draw
   site in the C shell, which settled the design:
   - Six primitives across ~90 call sites: `DrawRectangle` (43),
     `DrawTexturePro` (30), `DrawRectangleLines` (12), three
     `DrawRectangleRounded`, three `DrawRectangleRoundedLines`, and three
     `DrawText` (all three in the pack picker, none in the game).
   - EVERY src rect is the whole texture. The only exception is the bitmap
     font's glyph sub-rect, which a `Text` command absorbs. So a blit
     command needs NO src rect, which is what keeps view code testable
     headlessly -- it never has to know a texture's dimensions.
   - Mirroring is real but is expressed in C as a NEGATIVE src width, a
     raylib convention. Make it an explicit `flip_x`. Three sites decide it:
     hero facing, combat side, end cartoon.
   - Rotation is 0 and origin is (0,0) at every single call site.
   - Dst size is a layout constant everywhere EXCEPT `combat_render`, which
     sizes dst from the texture's own pixels. So the dst needs a "native
     size" case alongside the explicit rect.
   - `BeginScissorMode` is used exactly once, for the map viewport.
   - Keep `Text` a command rather than expanding it to glyph blits at emit
     time: a test then asserts on the string, not on 20 blits. Metrics are
     fixed 8x8 and pure, so measuring needs no asset.
   - DEFER the sprite catalog (`sprites_anim_tex`'s directional/mirror
     resolution, `sprites_frame` folding, and the "animated else still"
     fallback). All three need pack data, and their shape is decided by
     `map_render`, their only caller, in step 4. Same reasoning as
     `CombatTurnRecord`.
   - `palette.bin` is 768 raw RGB bytes; the first 16 entries have a
     canonical fallback in `palette.c` so chrome renders without the file.
     Parsing it is pure and belongs in this step.
3. **Backend.** macroquad window, integer-scale present, sprite atlas load,
   bitmap font, input mapping. The art manifest comes first, because the
   backend cannot load what the pack has not been asked about:
   - DONE: per-catalog art. `TroopDef.sprite` / `.anim[]` and
     `VillainDef.anim[]` were unparsed; `ClassDef.portrait`,
     `VillainDef.portrait` and `ArtifactDef.icon` already were.
     `parse_path_array` ports C's, including its fix: a non-string entry is
     SKIPPED rather than burning a slot, so one bad frame does not leave a
     hole mid-cycle. The stored COUNT is the cycle length, not the array's
     16 slots. Verified on the shipped pack: 25 troops x 4 frames, 17
     villains, every path on disk.
     Note the shipped pack declares NO villain animations at all, so the
     `<portrait-stem>_NN` sibling derivation is the live path for wanted
     posters, not a fallback. The shell owes that.
     Boxing `Resources.troops` and `.villains` was forced, not stylistic:
     the two catalogs grew to ~140KB and overflowed the test stack.
   - TODO: the role-fixed `sprites` manifest block (`resources.h` ~890):
     hero walk/idle/boat as `ResAnimSet`, the 15 combat tiles, the font and
     palette paths, six backdrops, two endings, the HUD panels and their
     animations, the bar strip, chrome, splashes, class picker, orb.
     `ResAnimSet` accepts both authoring forms -- a flat array lands in
     `south` with `directional` false, and an object names up to four facings.
   - TODO: `present_scale` from `src/present.c`. Pure arithmetic, so it can
     be ported and tested before any window exists. Legacy and modern take
     completely separate paths: legacy auto-fits with the `SCALE_MIN` floor
     and has NO override, because the scale control is a modern-only feature.
4. **Chrome + map viewport.** First thing actually on screen.
5. **Banner/template layer.** `strings` parsing and `%VAR%` substitution.
6. **Views.** Town, castle, dwelling, alcove, contract, army, spells, puzzle.
7. **Prompts and the decision loop**, wiring to the engine's outcome enums.
8. **Combat loop and rendering**, then `CombatTurnRecord` once its consumer
   exists and its shape is known.

## Still deliberately unported in the engine

- `CombatTurnRecord`: write-only replay data for animation. Its shape depends
  on what the renderer wants to replay, so it waits for step 8.
- `player_io.c`: the FIFO carries header/body TEXT, which is presentation,
  and its `player_io_answer` routing exists to pick a `flow_apply_*` core
  from a mirrored global `pending_flow`. All twelve of those cores are
  directly callable engine methods here, so a shell calls the one it means.
