# Sidebar HUD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Paint the five sidebar panels -- contract, siege weapons, magic star,
puzzle grid and gold purse -- into the 48px column `layout.rs` already reserves
and nothing currently draws, porting `reference/src/hud.c`.

**Architecture:** A new `shell/src/hud.rs` holding one `Hud` struct of borrowed
references and a `draw(&self, list: &mut DrawList)`, shaped exactly like the
existing `shell/src/chrome.rs`. It emits draw commands only -- no engine logic,
no graphics library -- so every panel is testable against a `DrawList` with no
window open. `main.rs` calls it once per frame between the map and the views,
matching `reference/src/shell_frame.c`'s `chrome -> map -> hud -> overlay`.

**Tech Stack:** Rust, the workspace's existing `shell` crate. No new
dependencies. `cargo test -p openbounty-shell`.

## Why this slice is small

Everything below the draw layer already exists and is tested. Do not rebuild it:

- `Sprite::HudContractSilhouette`, `HudSiegeSilhouette`, `HudMagicSilhouette`,
  `HudPuzzleGrid`, `HudGoldPurse`, `HudSiegeAnim { frame }`,
  `HudMagicAnim { frame }` and `PuzzleCover` are all in the `Sprite` enum
  (`shell/src/draw.rs`).
- `ResSprites` parses every `sprites.hud.*` path and `sprites.ui.puzzle_cover`
  (`engine/src/resources.rs:1839`).
- `SpriteCatalog::path` resolves all of them (`shell/src/sprites.rs:277-292`),
  and `all_paths()` already lists them, so the **backend loads this art today**
  -- a run reports `loaded 310 textures`.
- `Game::puzzle_cell_revealed(row, col)` (`engine/src/game.rs:1493`) is the
  ported reveal rule.
- `Layout::sidebar_x()`, `sidebar_y()`, `sidebar_h()` and `Layout::sidebar_w`
  exist (`shell/src/layout.rs:205-215`).

The only missing piece is a renderer that emits the commands, plus one call site.

## Global Constraints

Copied from `AGENTS.md`; every task's requirements implicitly include these.

- **Ask before committing.** The user wants approval before every commit.
  Complete a tested task, report it, and ask. Do not chain commits.
- **Commit messages are a single subject line.** No body paragraphs, no bullet
  lists, no `Co-Authored-By` or other attribution trailers. Example:
  `feat: draw the sidebar contract panel`.
- **Never commit `reference/`, `game-assets/`, or build output.** They are
  gitignored; keep it that way.
- `shell/src/backend.rs` is the ONLY module that may know macroquad exists.
  `hud.rs` must not import it.
- Do not add anything to `engine/Cargo.toml`. `engine/tests/render_free.rs`
  enforces this.
- Transliterate `hud.c` first; do not improve it. Where a C comment and the C
  code disagree, port the code and say so in the test name.
- Preserve unrelated working-tree changes. Do not reset, checkout, or clean.
  Note `tools/gen_ai.py` is already modified at the time of writing.
- Run `cargo fmt --all --check` before every commit.

## File Structure

- **Create `shell/src/hud.rs`** (~200 lines incl. tests). Owns the sidebar
  column and nothing else: panel geometry, the five panels, and their unit
  tests. One responsibility, mirroring how `chrome.rs` owns the frame and
  status bar.
- **Modify `shell/src/main.rs`**: add `pub mod hud;`, a `use`, and one draw
  call in the adventure branch.
- **Modify `shell/src/sprites.rs`**: add two frame-count accessors (Task 2).
- **Modify `shell/src/views.rs`**: widen `villain_sprite` from private to
  `pub(crate)` (Task 1) so both the contract view and the HUD use the one
  helper. This is C's own sharing -- both renderers read `s->villain_anim`
  through `sprites_frame` -- not a tidy-up of duplicated C.

## Out of scope -- do not build these

- **`game.hud_visible`.** C writes it in `main.c:974-977` when an overlay opens
  and closes, and `draw_frame` never reads it. It is DEAD in C. Do not wire it
  up, and do not add a Rust field for it.
- **The floating HUD bar.** `Sprite::HudBarStrip` is already drawn by
  `Chrome::draw_shell` (`shell/src/chrome.rs:90`). Leave it there.
- **A pointer/click target for any panel.** `specs/mouse-support-plan.md` does
  not list the sidebar, and C's HUD has no hit regions. Emit no `HitList`
  entries.
- Audio, the in-game menu, the pack picker. Separate slices; see
  `specs/play-verification.md` §13.

---

### Task 1: The module, the panel geometry, and the contract panel

**Files:**
- Create: `shell/src/hud.rs`
- Modify: `shell/src/views.rs:2935` (make `villain_sprite` `pub(crate)`)
- Modify: `shell/src/main.rs` (add `pub mod hud;` to the module list)
- Test: `shell/src/hud.rs` (inline `#[cfg(test)] mod tests`, the crate's convention)

**Interfaces:**
- Consumes: `Layout::sidebar_x() -> i32`, `Layout::sidebar_y() -> i32`,
  `Layout::sidebar_w: i32`, `Layout::tile_h: i32`;
  `DrawList::blit(Sprite, Dst)`; `Dst::rect(x, y, w, h) -> Dst`;
  `SpriteCatalog::villain_anim_len(usize) -> usize`;
  `Resources::villain_by_id(&str) -> Option<&VillainDef>` (field `.index: i32`);
  `views::villain_sprite(&SpriteCatalog, i32, i32) -> Option<Sprite>`.
- Produces: `hud::Hud<'a>` with public fields `layout`, `palette`, `catalog`,
  `resources`, `game`, `time: f64`; `Hud::draw(&self, list: &mut DrawList)`;
  `Hud::panel(&self, index: i32) -> Rect` (private, but Tasks 2-4 call it);
  `hud::PANEL_COUNT: i32 = 5`.

- [ ] **Step 1: Widen `villain_sprite` and register the module**

In `shell/src/views.rs`, change the signature at line 2935 from `fn` to
`pub(crate) fn`:

```rust
pub(crate) fn villain_sprite(catalog: &SpriteCatalog, index: i32, tick: i32) -> Option<Sprite> {
```

In `shell/src/main.rs`, add to the module list (keep it alphabetical -- it sits
between `font` and `input`):

```rust
pub mod hud;
```

- [ ] **Step 2: Write the failing tests**

Create `shell/src/hud.rs` containing ONLY the test module below. It pulls
everything it needs through `use super::*`, and `super` is empty until Step 4 --
so the crate will not compile after this step. That is the point: Step 3
records the failure before Step 4 fixes it.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::resources::Resources;

    /// A pack declaring every HUD slot, so `SpriteCatalog::path` resolves and
    /// the blits actually land in the list.
    fn fixtures() -> (Resources, Layout, SpriteCatalog) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "villains": [
                  {"index":0,"id":"varn","name":"Varn","castle":"castle0",
                   "portrait":"murray.png"}
                ],
                "sprites": {
                  "ui": {"puzzle_cover": "cover.png"},
                  "hud": {
                    "contract_silhouette": "contract.png",
                    "siege_silhouette": "siege_off.png",
                    "siege_animation": ["siege0.png", "siege1.png"],
                    "magic_silhouette": "magic_off.png",
                    "magic_animation": ["magic0.png", "magic1.png"],
                    "puzzle_grid": "grid.png",
                    "gold_purse": "purse.png"
                  }
                }}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        (resources, layout, catalog)
    }

    fn render(resources: &Resources, layout: &Layout, catalog: &SpriteCatalog,
              game: &Game, time: f64) -> DrawList {
        let palette = Palette::fallback();
        let mut list = DrawList::new();
        Hud { layout, palette: &palette, catalog, resources, game, time }
            .draw(&mut list);
        list
    }

    #[test]
    fn the_five_panels_stack_one_tile_apart_down_the_sidebar() {
        let (_, layout, _) = fixtures();
        let hud_x = layout.sidebar_x();
        let hud_y = layout.sidebar_y();
        for index in 0..PANEL_COUNT {
            let rect = Hud::panel_rect(&layout, index);
            assert_eq!(rect.x, hud_x);
            assert_eq!(rect.y, hud_y + index * layout.tile_h);
            assert_eq!(rect.w, layout.sidebar_w);
            assert_eq!(rect.h, layout.tile_h);
        }
        // Legacy: five 34px panels exactly fill the 170px column.
        assert_eq!(PANEL_COUNT * layout.tile_h, layout.sidebar_h());
    }

    #[test]
    fn the_contract_panel_draws_the_silhouette_with_no_contract() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudContractSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 0)));
        // No contract, so no face on top of it.
        assert!(list.blits_of(Sprite::VillainPortrait(0)).is_empty());
    }

    #[test]
    fn the_contract_panel_overlays_the_villain_face_on_the_silhouette() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.active_id.copy_from_str("varn");
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        // C blits the silhouette FIRST, then the face over it.
        let silhouette = list.blits_of(Sprite::HudContractSilhouette);
        let face = list.blits_of(Sprite::VillainPortrait(0));
        assert_eq!(silhouette.len(), 1);
        assert_eq!(face.len(), 1);
        assert_eq!(face[0], Dst::Stretch(Hud::panel_rect(&layout, 0)));
    }

    #[test]
    fn an_unknown_contract_id_leaves_the_silhouette_bare() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.active_id.copy_from_str("nobody");
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::HudContractSilhouette).len(), 1);
        assert!(list.blits_of(Sprite::VillainPortrait(0)).is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell hud:: 2>&1 | head -30`
Expected: FAIL to compile, with `cannot find struct, variant or union type
'Hud'` among the errors (every name the tests reach for through `super::*` is
unresolved at this point, so expect several).

- [ ] **Step 4: Write the implementation**

Prepend to `shell/src/hud.rs`, above the test module:

```rust
//! The sidebar panels, porting `src/hud.c`.
//!
//! Five 48x34 panels stack down the column `layout.rs` reserves to the right
//! of the map: contract, siege weapons, magic star, puzzle grid, gold purse.
//! C reads `GetTime()` inside `hud_draw`; this takes the clock as a field so
//! every panel is testable against a `DrawList` with no window open.
//!
//! NOT ported: `game.hud_visible`. C writes it when an overlay opens and
//! closes (`main.c:974`) and `draw_frame` never reads it. It is dead there,
//! so it is absent here.

use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;

use crate::draw::{Dst, DrawList, Rect, Sprite};
use crate::layout::Layout;
use crate::palette::Palette;
use crate::sprites::SpriteCatalog;
use crate::views::villain_sprite;

/// Panels stacked down the sidebar, from C's five `blit_panel` calls.
pub const PANEL_COUNT: i32 = 5;

/// The sidebar animations tick at ~2Hz, C's `(int)(GetTime() * 2.0)`.
const ANIM_HZ: f64 = 2.0;

pub struct Hud<'a> {
    pub layout: &'a Layout,
    pub palette: &'a Palette,
    pub catalog: &'a SpriteCatalog,
    pub resources: &'a Resources,
    pub game: &'a Game,
    /// Wall-clock seconds, as `get_time()` reports them.
    pub time: f64,
}

impl Hud<'_> {
    /// Panel `index`'s rectangle. C walks `y += CL_TILE_H` after each blit;
    /// this indexes the same stack so a caller can name one panel.
    ///
    /// An associated function rather than a method so tests can ask for a
    /// rectangle without standing up a whole `Hud`.
    pub(crate) fn panel_rect(layout: &Layout, index: i32) -> Rect {
        Rect::new(
            layout.sidebar_x(),
            layout.sidebar_y() + index * layout.tile_h,
            layout.sidebar_w,
            layout.tile_h,
        )
    }

    fn panel(&self, index: i32) -> Rect {
        Self::panel_rect(self.layout, index)
    }

    /// The shared animation counter. C recomputes it per panel from the same
    /// clock, so one value here is the same answer.
    fn tick(&self) -> i32 {
        (self.time * ANIM_HZ) as i32
    }

    pub fn draw(&self, list: &mut DrawList) {
        self.draw_contract(list);
    }

    /// Panel 0: the silhouette always, with the active contract's animated
    /// villain face blitted over the top of it.
    fn draw_contract(&self, list: &mut DrawList) {
        let rect = self.panel(0);
        list.blit(Sprite::HudContractSilhouette, Dst::Stretch(rect));

        if self.game.contract.active_id.is_empty() {
            return;
        }
        let Some(villain) = self
            .resources
            .villain_by_id(self.game.contract.active_id.as_str())
        else {
            return;
        };
        // C bounds the index at 17 before indexing its fixed arrays;
        // `villain_sprite` answers None out of range, which is the same guard.
        if let Some(sprite) = villain_sprite(self.catalog, villain.index, self.tick()) {
            list.blit(sprite, Dst::Stretch(rect));
        }
    }
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell hud::`
Expected: PASS, 4 tests.

- [ ] **Step 6: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

Two warnings are expected here and both clear later in the plan, so do not
"fix" them by deleting the fields:

- `field 'palette' is never read` -- `draw_gold` reads it in Task 4.
- `field 'time' is never read` will NOT appear, because `draw_contract` uses
  `tick()` already.

The test fixture in Step 2 has been verified to parse against the real
`Resources` loader, including `villain_by_id("varn").index == 0` and all
seven `sprites.hud.*` slots. If it fails to parse, suspect a change to the
loader rather than the JSON.

- [ ] **Step 7: Ask for approval, then commit**

Report the task and ask before running this:

```bash
git add shell/src/hud.rs shell/src/main.rs shell/src/views.rs
git commit -m "feat: draw the sidebar contract panel"
```

---

### Task 2: The siege and magic panels

**Files:**
- Modify: `shell/src/sprites.rs` (add two accessors near `villain_anim_len`, line 223)
- Modify: `shell/src/hud.rs`
- Test: `shell/src/hud.rs`

**Interfaces:**
- Consumes: `Hud::panel(&self, i32) -> Rect`, `Hud::tick(&self) -> i32` from
  Task 1; `sprites::frame_index(tick: i32, count: usize) -> usize`;
  `Game.stats.siege_weapons: i32`, `Game.stats.knows_magic: bool`.
- Produces: `SpriteCatalog::hud_siege_anim_len(&self) -> usize` and
  `SpriteCatalog::hud_magic_anim_len(&self) -> usize`;
  `Hud::draw_siege` and `Hud::draw_magic` (private).

- [ ] **Step 1: Write the failing tests**

Add to `shell/src/hud.rs`'s `mod tests`:

```rust
    #[test]
    fn the_siege_panel_shows_the_silhouette_until_a_weapon_is_owned() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudSiegeSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 1)));
        assert!(list.blits_of(Sprite::HudSiegeAnim { frame: 0 }).is_empty());
    }

    #[test]
    fn the_siege_panel_animates_once_a_weapon_is_owned() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.siege_weapons = 1;

        // Two frames declared, ~2Hz: t=0.0 is frame 0, t=0.5 is frame 1.
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        assert_eq!(
            list.blits_of(Sprite::HudSiegeAnim { frame: 0 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 1))]
        );
        assert!(list.blits_of(Sprite::HudSiegeSilhouette).is_empty());

        let list = render(&resources, &layout, &catalog, &game, 0.5);
        assert_eq!(
            list.blits_of(Sprite::HudSiegeAnim { frame: 1 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 1))]
        );
    }

    #[test]
    fn the_magic_panel_shows_the_silhouette_until_the_hero_knows_magic() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let blits = list.blits_of(Sprite::HudMagicSilhouette);
        assert_eq!(blits.len(), 1);
        assert_eq!(blits[0], Dst::Stretch(Hud::panel_rect(&layout, 2)));
    }

    #[test]
    fn the_magic_panel_animates_once_the_hero_knows_magic() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.knows_magic = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(
            list.blits_of(Sprite::HudMagicAnim { frame: 0 }),
            vec![Dst::Stretch(Hud::panel_rect(&layout, 2))]
        );
        assert!(list.blits_of(Sprite::HudMagicSilhouette).is_empty());
    }

    /// C tests `if (g->stats.siege_weapons)`, an int, so any non-zero count
    /// lights the panel -- not just 1.
    #[test]
    fn several_siege_weapons_still_light_the_panel_once() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.siege_weapons = 3;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::HudSiegeAnim { frame: 0 }).len(), 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell hud::`
Expected: FAIL. The silhouette tests fail on `assert_eq!(blits.len(), 1)`
getting 0; the animation tests fail the same way.

- [ ] **Step 3: Add the frame-count accessors**

In `shell/src/sprites.rs`, immediately after `villain_anim_len` (which ends at
line 225):

```rust
    /// Frames in the sidebar siege-weapon cycle.
    pub fn hud_siege_anim_len(&self) -> usize {
        self.sprites.hud_siege_animation_count
    }

    /// Frames in the sidebar magic-star cycle.
    pub fn hud_magic_anim_len(&self) -> usize {
        self.sprites.hud_magic_animation_count
    }
```

- [ ] **Step 4: Write the panels**

In `shell/src/hud.rs`, extend `draw`:

```rust
    pub fn draw(&self, list: &mut DrawList) {
        self.draw_contract(list);
        self.draw_siege(list);
        self.draw_magic(list);
    }
```

and add the two methods after `draw_contract`:

```rust
    /// Panel 1: silhouette when no siege weapon is owned, the animated cart
    /// when one is. C tests the int count, so any non-zero lights it.
    fn draw_siege(&self, list: &mut DrawList) {
        let rect = self.panel(1);
        if self.game.stats.siege_weapons != 0 {
            let frame = frame_index(self.tick(), self.catalog.hud_siege_anim_len()) as i32;
            list.blit(Sprite::HudSiegeAnim { frame }, Dst::Stretch(rect));
        } else {
            list.blit(Sprite::HudSiegeSilhouette, Dst::Stretch(rect));
        }
    }

    /// Panel 2: silhouette until the hero knows magic, then the animated star.
    fn draw_magic(&self, list: &mut DrawList) {
        let rect = self.panel(2);
        if self.game.stats.knows_magic {
            let frame = frame_index(self.tick(), self.catalog.hud_magic_anim_len()) as i32;
            list.blit(Sprite::HudMagicAnim { frame }, Dst::Stretch(rect));
        } else {
            list.blit(Sprite::HudMagicSilhouette, Dst::Stretch(rect));
        }
    }
```

Extend the `sprites` import at the top of the file:

```rust
use crate::sprites::{frame_index, SpriteCatalog};
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell hud::`
Expected: PASS, 9 tests.

- [ ] **Step 6: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 7: Ask for approval, then commit**

```bash
git add shell/src/hud.rs shell/src/sprites.rs
git commit -m "feat: draw the sidebar siege and magic panels"
```

---

### Task 3: The puzzle grid panel

**Files:**
- Modify: `shell/src/hud.rs`
- Test: `shell/src/hud.rs`

**Interfaces:**
- Consumes: `Hud::panel(&self, i32) -> Rect` from Task 1;
  `Game::puzzle_cell_revealed(&self, row: i32, col: i32) -> bool`
  (`engine/src/game.rs:1493`).
- Produces: `Hud::draw_puzzle` (private); the module constants
  `COVER_W: i32 = 9`, `COVER_H: i32 = 6`, `COVER_INSET: i32 = 2`.

**Do not port `hud.c`'s `PUZZLE_MAP`.** That 5x5 table is a verbatim copy of
`tables.c`'s `PUZZLE_GRID`, and `hud_draw` re-implements the caught/found test
over it inline. Both are already ported once, as `tables::puzzle_grid_entity`
and `Game::puzzle_cell_revealed` (`engine/src/game.rs:1493`), with tests
asserting the grid's structure. Copying the table into the shell would put
engine data in the wrong crate. This is the one place in `hud.c` where the
transliterate-first rule does NOT mean copying what you see.

**Note on the cover size.** C reads `s->puzzle_cover.width/height` off the
loaded texture. The draw vocabulary is deliberately headless -- a view never
learns a texture's pixel size (`draw.rs` module docs) -- so this port fixes the
legacy native 9x6 as constants. Same trade already made for `FACE_W`/`FACE_H`
in `views.rs:40`. Record it in a comment; do not add a source-rect to `Dst`.

- [ ] **Step 1: Write the failing tests**

Add to `shell/src/hud.rs`'s `mod tests`:

```rust
    #[test]
    fn the_puzzle_panel_covers_all_twenty_five_cells_at_the_start() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let grid = list.blits_of(Sprite::HudPuzzleGrid);
        assert_eq!(grid.len(), 1);
        assert_eq!(grid[0], Dst::Stretch(Hud::panel_rect(&layout, 3)));
        // Nothing caught or found yet, so every cell is still covered.
        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 25);
    }

    #[test]
    fn catching_a_villain_lifts_exactly_one_cover() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.contract.villains_caught[0] = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 24);
    }

    #[test]
    fn finding_an_artifact_lifts_exactly_one_cover() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.artifacts.found[0] = true;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert_eq!(list.blits_of(Sprite::PuzzleCover).len(), 24);
    }

    /// The covers tile the panel from a 2px inset, 9x6 each, row-major.
    #[test]
    fn the_covers_tile_the_panel_from_the_inset() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let panel = Hud::panel_rect(&layout, 3);

        let covers = list.blits_of(Sprite::PuzzleCover);
        // First cell: top-left, inset by 2 on both axes.
        assert_eq!(
            covers[0],
            Dst::Stretch(Rect::new(
                panel.x + COVER_INSET,
                panel.y + COVER_INSET,
                COVER_W,
                COVER_H
            ))
        );
        // Last cell: row 4, col 4.
        assert_eq!(
            covers[24],
            Dst::Stretch(Rect::new(
                panel.x + COVER_INSET + 4 * COVER_W,
                panel.y + COVER_INSET + 4 * COVER_H,
                COVER_W,
                COVER_H
            ))
        );
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell hud::`
Expected: FAIL. `cannot find value 'COVER_INSET' in this scope`, and the grid
assertions get 0 blits.

- [ ] **Step 3: Write the panel**

In `shell/src/hud.rs`, add the constants below `ANIM_HZ`:

```rust
/// The puzzle cover's native pixel size. C reads it off the loaded texture;
/// the draw vocabulary is headless and never learns a texture's size, so the
/// legacy native 9x6 is fixed here -- the same trade `views.rs` makes for
/// `FACE_W`/`FACE_H`.
const COVER_W: i32 = 9;
const COVER_H: i32 = 6;
/// C insets the 5x5 cover block 2px inside the 48x34 panel.
const COVER_INSET: i32 = 2;
```

Extend `draw`:

```rust
    pub fn draw(&self, list: &mut DrawList) {
        self.draw_contract(list);
        self.draw_siege(list);
        self.draw_magic(list);
        self.draw_puzzle(list);
    }
```

and add the method after `draw_magic`:

```rust
    /// Panel 3: the empty grid, with a cover stamped over every cell whose
    /// villain is still free or whose artifact is still lost. Covers vanish
    /// as the player progresses, so the grid fills in toward the scepter.
    fn draw_puzzle(&self, list: &mut DrawList) {
        let rect = self.panel(3);
        list.blit(Sprite::HudPuzzleGrid, Dst::Stretch(rect));

        for row in 0..5 {
            for col in 0..5 {
                if self.game.puzzle_cell_revealed(row, col) {
                    continue;
                }
                list.blit(
                    Sprite::PuzzleCover,
                    Dst::rect(
                        rect.x + COVER_INSET + col * COVER_W,
                        rect.y + COVER_INSET + row * COVER_H,
                        COVER_W,
                        COVER_H,
                    ),
                );
            }
        }
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell hud::`
Expected: PASS, 13 tests.

- [ ] **Step 5: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/hud.rs
git commit -m "feat: draw the sidebar puzzle grid panel"
```

---

### Task 4: The gold purse panel

**Files:**
- Modify: `shell/src/hud.rs`
- Test: `shell/src/hud.rs`

**Interfaces:**
- Consumes: `Hud::panel(&self, i32) -> Rect` from Task 1;
  `font::measure_width(&str) -> i32`, `font::GLYPH_H: i32`;
  `Palette::clr(Pal) -> Color`; `DrawList::text(&str, i32, i32, Color)`;
  `DrawList::texts(&self) -> Vec<(i32, i32, &str)>`; `Game.stats.gold: i32`.
- Produces: `Hud::draw_gold` (private). This is the last panel; after it the
  module is feature-complete and Task 5 only wires it in.

- [ ] **Step 1: Write the failing tests**

Add to `shell/src/hud.rs`'s `mod tests`:

```rust
    #[test]
    fn the_gold_panel_draws_the_purse_and_the_figure() {
        let (resources, layout, catalog) = fixtures();
        let mut game = Game::default();
        game.stats.gold = 7500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        let purse = list.blits_of(Sprite::HudGoldPurse);
        assert_eq!(purse.len(), 1);
        assert_eq!(purse[0], Dst::Stretch(Hud::panel_rect(&layout, 4)));
        assert!(list.has_text_containing("7500"));
    }

    /// C right-aligns the figure 2px off the panel's right edge and sits it
    /// 2px up from the bottom, so a wider number grows leftward.
    #[test]
    fn the_gold_figure_is_right_aligned_in_the_panel() {
        let (resources, layout, catalog) = fixtures();
        let panel = Hud::panel_rect(&layout, 4);

        let mut game = Game::default();
        game.stats.gold = 7500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let (x, y, text) = list.texts().pop().expect("a gold figure");
        assert_eq!(text, "7500");
        assert_eq!(x, panel.x + layout.sidebar_w - font::measure_width("7500") - 2);
        assert_eq!(y, panel.y + layout.tile_h - font::GLYPH_H - 2);

        // One more digit starts one glyph further left, same baseline.
        game.stats.gold = 17500;
        let list = render(&resources, &layout, &catalog, &game, 0.0);
        let (wide_x, wide_y, _) = list.texts().pop().expect("a gold figure");
        assert_eq!(wide_x, x - font::GLYPH_W);
        assert_eq!(wide_y, y);
    }

    /// Zero gold still draws; C formats the int unconditionally.
    #[test]
    fn zero_gold_still_draws_the_figure() {
        let (resources, layout, catalog) = fixtures();
        let game = Game::default();
        let list = render(&resources, &layout, &catalog, &game, 0.0);

        assert!(list.has_text_containing("0"));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell hud::`
Expected: FAIL. `assert_eq!(purse.len(), 1)` gets 0, and
`list.texts().pop()` returns `None`.

- [ ] **Step 3: Write the panel**

In `shell/src/hud.rs`, extend `draw`:

```rust
    pub fn draw(&self, list: &mut DrawList) {
        self.draw_contract(list);
        self.draw_siege(list);
        self.draw_magic(list);
        self.draw_puzzle(list);
        self.draw_gold(list);
    }
```

and add the method after `draw_puzzle`:

```rust
    /// Panel 4: the purse, with the gold figure right-aligned 2px off the
    /// panel's right edge and 2px up from its bottom.
    fn draw_gold(&self, list: &mut DrawList) {
        let rect = self.panel(4);
        list.blit(Sprite::HudGoldPurse, Dst::Stretch(rect));

        let gold = self.game.stats.gold.to_string();
        list.text(
            &gold,
            rect.x + self.layout.sidebar_w - font::measure_width(&gold) - 2,
            rect.y + self.layout.tile_h - font::GLYPH_H - 2,
            self.palette.clr(Pal::Yellow),
        );
    }
```

Add the two imports at the top of the file:

```rust
use crate::font;
use crate::palette::{Pal, Palette};
```

(replacing the bare `use crate::palette::Palette;` from Task 1).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell hud::`
Expected: PASS, 16 tests.

- [ ] **Step 5: Check formatting and the whole suite**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/hud.rs
git commit -m "feat: draw the sidebar gold purse panel"
```

---

### Task 5: Wire the HUD into the frame

**Files:**
- Modify: `shell/src/main.rs:1508-1519` (the adventure branch, between the
  `MapScene { ... }.draw(&mut list);` call and `views.draw(`)

**Interfaces:**
- Consumes: `hud::Hud` and its `draw` from Tasks 1-4. In `main`'s scope the
  bindings are `layout: Layout`, `backend.palette() -> &Palette`,
  `catalog: SpriteCatalog`, `resources: Resources`, `game: Game`, and
  `get_time() -> f64` (already imported from macroquad at `main.rs:31`).
- Produces: nothing further; this is the last task.

**Ordering.** `reference/src/shell_frame.c:17-20` is
`chrome_draw -> map_render_draw -> hud_draw -> overlay_draw`. `overlay_draw`
is what runs the views, so the HUD goes after `MapScene` and before
`views.draw`. That order matters: the character view covers the sidebar as well
as the map (`views.rs:1756` sizes it `map_w + sidebar_w`), and it must paint
over the panels, not under them.

The combat branch gets no HUD. C's `combat_render` owns the full inner area and
calls only `chrome_draw_with_status` (`combat_render.c:208`) -- no `hud_draw`.

- [ ] **Step 1: Add the import**

In `shell/src/main.rs`, beside the other `use` lines for shell modules (after
`use draw::DrawList;`):

```rust
use hud::Hud;
```

- [ ] **Step 2: Add the draw call**

In the `else` branch of the render block, immediately after the `MapScene`
statement ends with `.draw(&mut list);` and before the
`// A view covers the map area ...` comment, insert:

```rust
            // The sidebar panels, drawn over the map and under the views --
            // `shell_frame.c`'s chrome -> map -> hud -> overlay order. A view
            // that covers the sidebar must paint over these.
            Hud {
                layout: &layout,
                palette: backend.palette(),
                catalog: &catalog,
                resources: &resources,
                game: &game,
                time: get_time(),
            }
            .draw(&mut list);
```

- [ ] **Step 3: Verify it builds and the suite is green**

Run: `cargo fmt --all --check && cargo test --workspace`
Expected: no formatting diff; all tests pass.

- [ ] **Step 4: Verify it on screen**

Run:

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```

Walk through the startup wizard to the overworld and confirm, in the 48px
column right of the map:

1. Five panels fill the column top to bottom with no gap and no overflow.
2. The gold figure matches the Character view's gold, and updates when it
   changes (open a chest, or rent a boat in a town).
3. The siege and magic panels show silhouettes for a fresh Knight. Buy siege
   weapons in a town (3000 gold) and confirm that panel starts animating.
4. The puzzle panel shows 25 covers on a fresh game.
5. Open the Character view and confirm it paints OVER the sidebar, not under.
6. Start a combat and confirm the sidebar is gone for the duration.

If the panels are misaligned rather than absent, the layout is the suspect, not
this module -- `Hud::panel_rect` is the single source of the geometry.

- [ ] **Step 5: Close the gap in the verification doc**

In `specs/play-verification.md` §13, delete the `| Sidebar HUD | GAP | ... |`
row, and update the "Last refreshed" line under the status legend to today's
date and the new tip. The doc's Maintenance section asks for exactly this, and
that row is the one this plan closes.

- [ ] **Step 6: Ask for approval, then commit**

```bash
git add shell/src/main.rs specs/play-verification.md
git commit -m "feat: draw the sidebar hud each frame"
```

---

## Known limitation: the five panels assume a 5-tall viewport

**Decided: port C's behaviour unchanged. Do not add a clamp.** This section is
here so that whoever first authors a modern pack recognises the symptom instead
of filing it as a HUD bug.

The sidebar column is `sidebar_h() = map_h = tile_h * tiles_h`, and the HUD
always occupies `PANEL_COUNT * tile_h`. Those agree only at `tiles_h == 5`.

- **Legacy always fits.** `parse_render` hard-codes 5x5 and ignores anything the
  pack declares (`engine/src/resources.rs:1872`, asserted by
  `legacy_render_mode_forces_the_dos_geometry`). Five 34px panels fill 170px
  exactly.
- **Modern is pack-declared and defaults to 7**, not 5
  (`engine/src/resources.rs:1884`). `Layout` requires an odd viewport of at
  least 3, so the legal values are 3, 5, 7, 9...

At the modern defaults (`tile_h` 96, `tiles_h` 7) the column is 672px and the
panels use 480px, leaving 192px of dead column under the purse. At `tiles_h: 3`
the column is 288px and the panels need 480px, so the purse and part of the
puzzle panel draw past the map area, over the bottom frame and the dialog
panel.

Why this is still the right port:

- C has exactly this property. `hud_draw` walks `y += CL_TILE_H` five times
  whatever `CL_SIDEBAR_H` is, and does not guard it. Parity target is
  OpenBounty's C.
- Nothing in the tree hits it. All three packs -- `reference/assets/
  kings-bounty`, `game-assets/hd-pack`, `game-assets/open-pack` -- declare
  `legacy`. There is no modern pack in this repository, so no run can reach
  either case today.
- A clamp would be a divergence invented against a hypothetical pack, which is
  what `AGENTS.md`'s transliterate-first rule exists to prevent. Every
  behaviour change needs a test or golden artifact first, and there is nothing
  here to write one against.

If a modern pack is ever authored, that is the moment to decide between three
options, and the pack's own art will make the choice obvious in a way it is not
now: leave the gap, skip panels that would overflow `sidebar_h()`, or size the
panels at `sidebar_h() / PANEL_COUNT` and accept that they are no longer one
tile tall. Do not pre-empt it.
