# Mouse Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pointer input to the shell's menus, views, dialogs and prompts, so a click does exactly what the equivalent keypress already does.

**Architecture:** A view fills a `HitList` from the same rect it draws, so regions cannot drift from what is on screen. A click hit-tests that list and resolves to the view's existing outcome enum (`TownInput::RunRow`, `SpellsInput::Cast`, ...), joining the keyboard's flow rather than paralleling it. `present.rs` owns the window-to-logical inversion; `backend.rs` stays the only macroquad-aware module.

**Tech Stack:** Rust 2021, macroquad 0.4.16 (backend only), no new dependencies.

Design doc: `specs/mouse-support-design.md` (commit `3d070a6`). Read it first.

## Global Constraints

- **The engine does not change.** No edits under `engine/`. `cargo test -p openbounty-engine --test render_free` must still pass.
- **The pointer must not reach a transaction the keyboard cannot.** Every click resolves to a value the keyboard path already produces.
- **Do not synthesise keypresses.** Resolve to the outcome enum directly.
- **`backend.rs` is the only module that may name macroquad.**
- **Hit regions are in logical coordinates**, the same space `DrawList` uses.
- **Commit messages are a single subject line.** No body, no bullet lists, no `Co-Authored-By` or other trailers. Conventional prefixes (`feat:`, `test:`, `refactor:`).
- **Ask the user for approval before every commit** (`AGENTS.md`). The commit step in each task means "present the tested slice and ask", not "push it yourself".
- Run `cargo fmt --all` before each commit; `cargo fmt --all --check` must be clean.
- Shell tests run with `cargo test -p openbounty-shell`.

---

### Task 1: Pointer vocabulary and hit-testing

**Files:**
- Create: `shell/src/pointer.rs`
- Modify: `shell/src/main.rs` (add `pub mod pointer;` beside the existing `pub mod present;`)

**Interfaces:**
- Consumes: `crate::draw::Rect`, `crate::views::TownRow` (both already exist).
- Produces: `PointerSource` trait, `Button`, `HitTarget`, `HitRegion`, `HitList` with `push`/`clear`/`hit`, and the `Pointer` test builder.

- [ ] **Step 1: Write the failing test**

Create `shell/src/pointer.rs` containing only the test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Rect;
    use crate::views::TownRow;

    #[test]
    fn the_hit_list_returns_the_region_under_a_point() {
        let mut hits = HitList::new();
        hits.push(Rect::new(10, 20, 100, 8), HitTarget::TownRow(TownRow::Contract));
        hits.push(Rect::new(10, 28, 100, 8), HitTarget::TownRow(TownRow::Boat));

        assert_eq!(hits.hit(11, 21), Some(HitTarget::TownRow(TownRow::Contract)));
        assert_eq!(hits.hit(11, 29), Some(HitTarget::TownRow(TownRow::Boat)));
        assert_eq!(hits.hit(5, 21), None, "left of the region");
        assert_eq!(hits.hit(11, 100), None, "below every region");
    }

    #[test]
    fn a_later_region_wins_so_an_overlay_covers_what_it_draws_over() {
        // Regions are pushed in draw order, so whatever drew last is on top.
        let mut hits = HitList::new();
        hits.push(Rect::new(0, 0, 50, 50), HitTarget::TownRow(TownRow::Boat));
        hits.push(Rect::new(0, 0, 50, 50), HitTarget::DialogDismiss);
        assert_eq!(hits.hit(10, 10), Some(HitTarget::DialogDismiss));
    }

    #[test]
    fn clear_empties_the_list() {
        let mut hits = HitList::new();
        hits.push(Rect::new(0, 0, 10, 10), HitTarget::DialogDismiss);
        hits.clear();
        assert_eq!(hits.hit(1, 1), None);
    }

    #[test]
    fn the_test_pointer_reports_position_and_edges() {
        let p = Pointer::at(7, 9).with_pressed(Button::Left);
        assert_eq!(p.position(), Some((7, 9)));
        assert!(p.pressed(Button::Left));
        assert!(!p.pressed(Button::Right));
        assert!(!Pointer::outside().position().is_some());
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p openbounty-shell pointer`
Expected: FAIL to compile — `cannot find type HitList in this scope`.

- [ ] **Step 3: Write the implementation**

Put this above the test module in `shell/src/pointer.rs`:

```rust
//! Pointer input and the hit regions a view publishes for it.
//!
//! The trait mirrors [`crate::input::KeySource`] and exists for the same
//! reason: every hit-test and dispatch test runs headlessly, with no window.
//! `backend.rs` supplies the real implementation.
//!
//! Regions are in LOGICAL coordinates -- the same space `DrawList` uses -- so
//! nothing here ever sees a window pixel. See `specs/mouse-support-design.md`.

use crate::draw::Rect;
use crate::views::TownRow;

/// The buttons the shell reads. Right is carried so a backend can report it;
/// no surface binds it yet.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Button {
    Left,
    Right,
}

/// What clicking a region means, in the vocabulary the keyboard already uses.
/// Variants are added as each surface is wired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HitTarget {
    TownRow(TownRow),
    DialogDismiss,
    PromptYes,
    PromptNo,
}

/// One actionable rectangle, in logical coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HitRegion {
    pub rect: Rect,
    pub target: HitTarget,
}

/// The regions published by one frame's draw, in draw order.
#[derive(Clone, Debug, Default)]
pub struct HitList {
    regions: Vec<HitRegion>,
}

impl HitList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, rect: Rect, target: HitTarget) {
        self.regions.push(HitRegion { rect, target });
    }

    pub fn clear(&mut self) {
        self.regions.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// The target at a logical point, or `None`. Scans in reverse so the
    /// LAST region pushed wins -- draw order is painter's order, so whatever
    /// drew on top is what the pointer hits.
    pub fn hit(&self, x: i32, y: i32) -> Option<HitTarget> {
        self.regions
            .iter()
            .rev()
            .find(|r| {
                x >= r.rect.x
                    && y >= r.rect.y
                    && x < r.rect.x + r.rect.w
                    && y < r.rect.y + r.rect.h
            })
            .map(|r| r.target)
    }
}

/// What the shell reads each frame.
pub trait PointerSource {
    /// Window pixels, or `None` when the pointer is outside the window.
    fn position(&self) -> Option<(i32, i32)>;
    /// Went down this frame.
    fn pressed(&self, button: Button) -> bool;
    /// Held right now.
    fn down(&self, button: Button) -> bool;
    /// Came up this frame.
    fn released(&self, button: Button) -> bool;
}

/// A fixed pointer state, for tests. Mirrors [`crate::input::Keys`].
#[derive(Clone, Debug, Default)]
pub struct Pointer {
    position: Option<(i32, i32)>,
    pressed: Vec<Button>,
    down: Vec<Button>,
    released: Vec<Button>,
}

impl Pointer {
    pub fn at(x: i32, y: i32) -> Self {
        Self {
            position: Some((x, y)),
            ..Self::default()
        }
    }

    pub fn outside() -> Self {
        Self::default()
    }

    pub fn with_pressed(mut self, button: Button) -> Self {
        self.pressed.push(button);
        self.down.push(button);
        self
    }

    pub fn with_released(mut self, button: Button) -> Self {
        self.released.push(button);
        self
    }
}

impl PointerSource for Pointer {
    fn position(&self) -> Option<(i32, i32)> {
        self.position
    }

    fn pressed(&self, button: Button) -> bool {
        self.pressed.contains(&button)
    }

    fn down(&self, button: Button) -> bool {
        self.down.contains(&button)
    }

    fn released(&self, button: Button) -> bool {
        self.released.contains(&button)
    }
}
```

Then add the module declaration in `shell/src/main.rs`, in the existing
alphabetical `pub mod` block:

```rust
pub mod pointer;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell pointer`
Expected: PASS, 4 tests.

- [ ] **Step 5: Format, full suite, then ask before committing**

```bash
cargo fmt --all
cargo fmt --all --check
cargo test --workspace
```
Expected: clean, zero failures, engine counts unchanged.

Suggested message: `feat: add the pointer vocabulary and hit-testing`

---

### Task 2: Window-to-logical inversion

**Files:**
- Modify: `shell/src/present.rs` (add a method to `impl Present`, beside `blit_rect`)

**Interfaces:**
- Consumes: the existing `Present::scale` and `Present::blit_rect`.
- Produces: `Present::window_to_logical(&self, layout: &Layout, win_w: i32, win_h: i32, target: Target, px: i32, py: i32) -> Option<(i32, i32)>`.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `shell/src/present.rs`. It already has
`legacy_layout()` and `modern_layout()` helpers — reuse them.

```rust
#[test]
fn window_to_logical_inverts_blit_rect_at_the_origin() {
    let layout = legacy_layout();
    let present = Present::new();
    let (win_w, win_h) = (1280, 800);
    let rect = present.blit_rect(&layout, win_w, win_h, Target::Desktop);
    // The blit rect's own origin is logical (0, 0).
    assert_eq!(
        present.window_to_logical(&layout, win_w, win_h, Target::Desktop, rect.x, rect.y),
        Some((0, 0))
    );
}

#[test]
fn window_to_logical_rejects_the_letterbox() {
    let layout = legacy_layout();
    let present = Present::new();
    let (win_w, win_h) = (1280, 800);
    let rect = present.blit_rect(&layout, win_w, win_h, Target::Desktop);
    assert!(rect.x > 0, "this window should letterbox horizontally");
    assert_eq!(
        present.window_to_logical(&layout, win_w, win_h, Target::Desktop, rect.x - 1, rect.y),
        None
    );
}

#[test]
fn window_to_logical_maps_the_last_pixel_inside() {
    let layout = legacy_layout();
    let present = Present::new();
    let (win_w, win_h) = (1280, 800);
    let rect = present.blit_rect(&layout, win_w, win_h, Target::Desktop);
    let got = present.window_to_logical(
        &layout,
        win_w,
        win_h,
        Target::Desktop,
        rect.x + rect.w - 1,
        rect.y + rect.h - 1,
    );
    assert_eq!(got, Some((layout.screen_w - 1, layout.screen_h - 1)));
    // One past the edge is outside.
    assert_eq!(
        present.window_to_logical(
            &layout,
            win_w,
            win_h,
            Target::Desktop,
            rect.x + rect.w,
            rect.y + rect.h
        ),
        None
    );
}

/// Legacy floors the scale at 2x, so a small window gets a target that
/// OVERHANGS it and `blit_rect` returns a negative origin. The inversion must
/// cope with that rather than clamping -- a window pixel then maps to a
/// logical coordinate that really is on screen.
#[test]
fn window_to_logical_handles_the_legacy_overhang() {
    let layout = legacy_layout();
    let present = Present::new();
    let (win_w, win_h) = (500, 300);
    let rect = present.blit_rect(&layout, win_w, win_h, Target::Desktop);
    assert!(rect.x < 0 && rect.y < 0, "expected an overhanging target");

    // Window pixel (0, 0) is inside the game, some way in from its left edge.
    let got = present
        .window_to_logical(&layout, win_w, win_h, Target::Desktop, 0, 0)
        .expect("window origin is over the game");
    assert!(got.0 > 0 && got.1 > 0);
    assert!(got.0 < layout.screen_w && got.1 < layout.screen_h);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell present::`
Expected: FAIL to compile — `no method named window_to_logical`.

- [ ] **Step 3: Write the implementation**

Add to `impl Present`, directly after `blit_rect`:

```rust
    /// The inverse of [`Present::blit_rect`]: a window pixel to a logical one.
    ///
    /// `None` when the point is outside the game's rectangle -- the letterbox.
    /// Note the division is `div_euclid`, not `/`: when the target overhangs
    /// the window `blit_rect` returns a NEGATIVE origin, and truncating
    /// division would fold the first `scale - 1` pixels left of the edge onto
    /// logical 0 instead of rejecting them.
    pub fn window_to_logical(
        &self,
        layout: &Layout,
        win_w: i32,
        win_h: i32,
        target: Target,
        px: i32,
        py: i32,
    ) -> Option<(i32, i32)> {
        let scale = self.scale(layout, win_w, win_h, target);
        if scale <= 0 {
            return None;
        }
        let rect = self.blit_rect(layout, win_w, win_h, target);
        let lx = (px - rect.x).div_euclid(scale);
        let ly = (py - rect.y).div_euclid(scale);
        if lx < 0 || ly < 0 || lx >= layout.screen_w || ly >= layout.screen_h {
            return None;
        }
        Some((lx, ly))
    }
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell present::`
Expected: PASS, including the existing present tests.

- [ ] **Step 5: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: invert the present blit rect for pointer input`

---

### Task 3: The macroquad pointer

**Files:**
- Modify: `shell/src/backend.rs` (add `MacroquadPointer` beside the existing `MacroquadKeys`)

**Interfaces:**
- Consumes: `crate::pointer::{Button, PointerSource}` from Task 1.
- Produces: `MacroquadPointer`, a unit struct implementing `PointerSource`.

This task has no unit test: it is the one place that talks to macroquad, and
`MacroquadKeys` beside it is untested for the same reason. Its correctness is
covered by the tested inversion (Task 2) and the tested dispatch (Task 4).

- [ ] **Step 1: Write the implementation**

Add to `shell/src/backend.rs`. Extend the existing macroquad `use` line with
`is_mouse_button_down`, `is_mouse_button_pressed`, `is_mouse_button_released`,
`mouse_position` and `MouseButton`, then:

```rust
/// The real pointer. Like [`MacroquadKeys`], this is the only place our
/// vocabulary meets macroquad's.
pub struct MacroquadPointer;

impl MacroquadPointer {
    fn button(button: Button) -> MouseButton {
        match button {
            Button::Left => MouseButton::Left,
            Button::Right => MouseButton::Right,
        }
    }
}

impl PointerSource for MacroquadPointer {
    fn position(&self) -> Option<(i32, i32)> {
        let (x, y) = mouse_position();
        if !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some((x as i32, y as i32))
    }

    fn pressed(&self, button: Button) -> bool {
        is_mouse_button_pressed(Self::button(button))
    }

    fn down(&self, button: Button) -> bool {
        is_mouse_button_down(Self::button(button))
    }

    fn released(&self, button: Button) -> bool {
        is_mouse_button_released(Self::button(button))
    }
}
```

- [ ] **Step 2: Verify it builds with no warnings**

```bash
cargo build --workspace 2>&1 | grep -E "^(warning|error)" || echo "clean"
```
Expected: `clean`.

- [ ] **Step 3: Verify the macroquad boundary still holds**

```bash
grep -rn "macroquad" shell/src/ --include=*.rs | grep -v "^shell/src/backend.rs"
```
Expected: no output. If anything prints, macroquad has leaked out of the backend.

- [ ] **Step 4: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: read the macroquad pointer`

---

### Task 4: Town hit regions, the pattern for every surface

This is the task that establishes the house pattern. Later surfaces copy it.

**Files:**
- Modify: `shell/src/views.rs` (`Views::draw` signature, `draw_town`, and a new `town_click` method)

**Interfaces:**
- Consumes: `HitList`, `HitTarget` (Task 1).
- Produces: `Views::draw(..., hits: &mut HitList, ...)`, and
  `Views::town_click(&mut self, target: HitTarget) -> TownInput`.

**Watch out:** `draw_town` draws the info overlay OVER the menu rows, in the
same function. Row regions must not be published while
`self.town_info_active` is set, or a click would run a row the player cannot
see. The overlay publishes a single dismiss region instead.

- [ ] **Step 1: Write the failing tests**

Add to the existing `mod tests` in `shell/src/views.rs`. The town fixtures
already exist there — reuse whatever `the_town_view_draws_header_and_rows`
uses to build `resources`, `layout`, `catalog` and `game`.

```rust
/// The anti-drift test. Copy this shape for every new surface: the published
/// region must contain the centre of the rect that was actually drawn.
#[test]
fn the_town_hit_regions_line_up_with_the_drawn_rows() {
    let (resources, layout, catalog) = town_fixtures();
    let game = Game::default();
    let mut views = Views::default();
    views.town_open("Midland", "town0", 0, 0);

    let mut list = DrawList::new();
    let mut hits = HitList::new();
    views.draw_town(&mut list, &mut hits, &layout, &Palette::default(),
                    &resources, &catalog, &game, 0.0);

    // Every row is reachable, and each maps to its own row.
    for (i, row) in [TownRow::Contract, TownRow::Boat, TownRow::Info,
                     TownRow::Spell, TownRow::Leave].iter().enumerate() {
        let region = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::TownRow(*row))
            .unwrap_or_else(|| panic!("row {i} published no region"));
        let cx = region.rect.x + region.rect.w / 2;
        let cy = region.rect.y + region.rect.h / 2;
        assert_eq!(hits.hit(cx, cy), Some(HitTarget::TownRow(*row)));
    }
}

#[test]
fn the_town_info_overlay_publishes_dismiss_instead_of_rows() {
    let (resources, layout, catalog) = town_fixtures();
    let game = Game::default();
    let mut views = Views::default();
    views.town_open("Midland", "town0", 0, 0);
    views.town_show_info("Some info");

    let mut list = DrawList::new();
    let mut hits = HitList::new();
    views.draw_town(&mut list, &mut hits, &layout, &Palette::default(),
                    &resources, &catalog, &game, 0.0);

    assert!(
        hits.regions().iter().all(|r| r.target == HitTarget::DialogDismiss),
        "the overlay covers the rows, so no row may be clickable"
    );
}

/// The keystone test: the pointer and the keyboard must agree, permanently.
#[test]
fn clicking_a_town_row_matches_pressing_its_letter() {
    let mut by_key = Views::default();
    by_key.town_open("Midland", "town0", 0, 0);
    let keyed = by_key.town_update(&Keys::pressed(&[Key::B]));

    let mut by_click = Views::default();
    by_click.town_open("Midland", "town0", 0, 0);
    let clicked = by_click.town_click(HitTarget::TownRow(TownRow::Boat));

    assert_eq!(keyed, TownInput::RunRow(TownRow::Boat));
    assert_eq!(clicked, keyed);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell views::tests::the_town_hit`
Expected: FAIL to compile — `draw_town` takes no `hits`, `town_click` missing,
`HitList::regions` missing.

- [ ] **Step 3: Add the `regions` accessor**

In `shell/src/pointer.rs`, add to `impl HitList`:

```rust
    /// The published regions, in draw order. For tests and hover.
    pub fn regions(&self) -> &[HitRegion] {
        &self.regions
    }
```

- [ ] **Step 4: Thread the list through `draw`**

In `shell/src/views.rs`, add `hits: &mut HitList` to `Views::draw` immediately
after `list: &mut DrawList`, and pass it down to `draw_town`. Give every other
`draw_*` arm the same parameter but leave their bodies alone for now — they
publish nothing until their own task.

Change `draw_town`'s signature to match:

```rust
    fn draw_town(
        &self,
        list: &mut DrawList,
        hits: &mut HitList,
        layout: &Layout,
        palette: &Palette,
        resources: &Resources,
        catalog: &SpriteCatalog,
        game: &Game,
        time: f64,
    ) {
```

- [ ] **Step 5: Publish the row regions from the drawing loop**

In `draw_town`, replace the existing row loop with this. The rect is computed
once and used twice, one line apart — that is the whole anti-drift guarantee:

```rust
        for row in 0..TOWN_ROW_COUNT {
            let label = crate::interact::town_row_text(resources, game, &self.town_record_key, row);
            let selected = row == self.town_cursor;
            let fg = if selected {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
            list.text(&label, tx, ty, fg);
            // Same rect, one line apart: the row's clickable band spans the
            // panel's inner width, not just the glyphs.
            if !self.town_info_active {
                hits.push(
                    Rect::new(tx, ty, w - 2 * pad, row_h),
                    HitTarget::TownRow(TownRow::from_index(row as usize)),
                );
            }
            ty += row_h;
        }
```

Then inside the existing `if self.town_info_active { ... }` block, after the
panel is drawn, publish the dismiss region:

```rust
            hits.push(Rect::new(x, y, w, h), HitTarget::DialogDismiss);
```

- [ ] **Step 6: Add `town_click`**

In `impl Views`, beside `town_update`:

```rust
    /// Resolve a pointer hit to the same outcome `town_update` returns for the
    /// equivalent keypress. The pointer must never reach a transaction the
    /// keyboard cannot, so this maps onto the existing vocabulary only.
    pub fn town_click(&mut self, target: HitTarget) -> TownInput {
        match target {
            HitTarget::DialogDismiss if self.town_info_active => {
                self.town_info_active = false;
                TownInput::Handled
            }
            HitTarget::TownRow(row) if !self.town_info_active => {
                self.town_cursor = row as i32;
                TownInput::RunRow(row)
            }
            _ => TownInput::None,
        }
    }
```

If `TownRow` has no `as i32` cast available, use the inverse of the existing
`TownRow::from_index` — add `TownRow::index(self) -> usize` next to it and use
that.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell views::`
Expected: PASS, including every pre-existing view test.

- [ ] **Step 8: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: publish town hit regions for the pointer`

---

### Task 5: Wire the pointer into the main loop

**Files:**
- Modify: `shell/src/main.rs`

**Interfaces:**
- Consumes: `MacroquadPointer` (Task 3), `Present::window_to_logical` (Task 2),
  `HitList` (Task 1), `Views::town_click` (Task 4).
- Produces: a live `HitList` owned by the loop, rebuilt each frame.

This task has no unit test — `main.rs`'s `async fn main` is not callable from
one. Its logic is covered by Tasks 2 and 4; this is wiring.

- [ ] **Step 1: Own the pointer and the hit list**

Beside the existing `let keys = MacroquadKeys;` and `let mut list = DrawList::new();`:

```rust
    let pointer = MacroquadPointer;
    // Rebuilt every frame during draw, and hit-tested on the NEXT frame's
    // input. One frame of latency; see specs/mouse-support-design.md.
    let mut hits = HitList::new();
```

Add the imports beside the existing ones:

```rust
use backend::{Backend, MacroquadKeys, MacroquadPointer};
use pointer::{Button, HitList, HitTarget, PointerSource};
```

- [ ] **Step 2: Resolve the pointer at the top of the frame**

Immediately before the existing input-priority chain (`if let Some(session) = &mut combat {`):

```rust
        // Where the pointer is, in logical coordinates, and what it is over.
        let (win_w, win_h) = (macroquad::prelude::screen_width() as i32,
                              macroquad::prelude::screen_height() as i32);
        let hovered = pointer.position().and_then(|(px, py)| {
            present.window_to_logical(&layout, win_w, win_h, present::Target::Desktop, px, py)
        }).and_then(|(lx, ly)| hits.hit(lx, ly));
        let clicked = if pointer.pressed(Button::Left) { hovered } else { None };
```

The `screen_width`/`screen_height` calls name macroquad outside `backend.rs`,
which the Global Constraints forbid. Instead add this to `Backend` in
`shell/src/backend.rs` and call `backend.window_size()`:

```rust
    /// The window size in pixels. Here because `backend.rs` is the only
    /// module that may name macroquad.
    pub fn window_size(&self) -> (i32, i32) {
        (screen_width() as i32, screen_height() as i32)
    }
```

so the loop reads:

```rust
        let (win_w, win_h) = backend.window_size();
```

- [ ] **Step 3: Dispatch a click in the views branch**

In the branch that handles an open view — where `views.town_update(&keys)` is
called — take the click first and fall back to the keyboard:

```rust
            let town = match clicked {
                Some(target) => views.town_click(target),
                None => views.town_update(&keys),
            };
```

Leave the existing `match town { ... }` arms untouched. They already handle
every `TownInput` variant, which is the point of resolving to that enum.

- [ ] **Step 4: Rebuild the list during draw**

Where `list.clear();` already runs each frame, clear the hit list too, and pass
it to `views.draw`:

```rust
        list.clear();
        hits.clear();
```

```rust
        views.draw(
            &mut list,
            &mut hits,
            &layout,
            backend.palette(),
            &resources,
            &catalog,
            &map,
            &fog,
            &game,
            &present,
            time,
        );
```

- [ ] **Step 5: Verify it builds clean and the boundary holds**

```bash
cargo build --workspace 2>&1 | grep -E "^(warning|error)" || echo "clean"
grep -rn "macroquad" shell/src/ --include=*.rs | grep -v "^shell/src/backend.rs"
```
Expected: `clean`, and no macroquad outside the backend.

- [ ] **Step 6: Run the app and click a town row**

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```
Walk onto a town and click each menu row. Expected: each row does what its
letter key does. This is the first moment the feature is real; the automated
tests cover the parts, not the integration.

- [ ] **Step 7: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: wire the pointer into the main loop`

---

### Task 6: Hover highlighting

**Files:**
- Modify: `shell/src/views.rs` (a `hover` field, and `draw_town`'s colour choice)
- Modify: `shell/src/main.rs` (feed `hovered` in each frame)

**Interfaces:**
- Consumes: `HitTarget` (Task 1), `hovered` (Task 5).
- Produces: `Views::set_hover(&mut self, target: Option<HitTarget>)`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn hover_highlights_a_row_without_moving_the_keyboard_cursor() {
    let mut views = Views::default();
    views.town_open("Midland", "town0", 0, 0);
    let before = views.town_cursor();
    views.set_hover(Some(HitTarget::TownRow(TownRow::Spell)));
    assert_eq!(
        views.town_cursor(),
        before,
        "hover must not change what Enter does"
    );
}

#[test]
fn leaving_the_window_clears_the_hover() {
    let mut views = Views::default();
    views.town_open("Midland", "town0", 0, 0);
    views.set_hover(Some(HitTarget::TownRow(TownRow::Spell)));
    views.set_hover(None);
    assert_eq!(views.hover(), None);
}
```

If `Views` has no public `town_cursor()` accessor, add one beside the existing
`controls_cursor()` and `gate_cursor()` accessors:

```rust
    pub fn town_cursor(&self) -> i32 {
        self.town_cursor
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell views::tests::hover`
Expected: FAIL to compile — `no method named set_hover`.

- [ ] **Step 3: Add the hover state**

Add the field to `struct Views` beside `town_cursor`:

```rust
    /// What the pointer is over. PURELY VISUAL -- it never moves a cursor,
    /// because moving the mouse must not change what Enter does.
    hover: Option<HitTarget>,
```

and the accessors in `impl Views`:

```rust
    pub fn set_hover(&mut self, target: Option<HitTarget>) {
        self.hover = target;
    }

    pub fn hover(&self) -> Option<HitTarget> {
        self.hover
    }
```

- [ ] **Step 4: Highlight the hovered row**

In `draw_town`, widen the colour choice. Keyboard selection keeps priority so
the two never both read as "the" selection:

```rust
            let selected = row == self.town_cursor;
            let hovered = self.hover == Some(HitTarget::TownRow(TownRow::from_index(row as usize)));
            let fg = if selected || hovered {
                palette.clr(Pal::Yellow)
            } else {
                palette.clr(Pal::White)
            };
```

- [ ] **Step 5: Feed it from the loop**

In `main.rs`, right after `hovered` is computed in Task 5:

```rust
        views.set_hover(hovered);
```

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell views::`
Expected: PASS.

- [ ] **Step 7: Check it by eye**

```bash
cargo run -p openbounty-shell -- reference/assets/kings-bounty/game.json
```
Expected: moving the pointer over town rows lights them; moving off the window
clears the highlight; arrow keys still move the keyboard cursor independently.

- [ ] **Step 8: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: highlight the row under the pointer`

---

### Task 7: Dialogs and prompts

**Files:**
- Modify: `shell/src/dialog.rs`, `shell/src/prompt.rs`, `shell/src/main.rs`

**Interfaces:**
- Consumes: `HitList`, `HitTarget::{DialogDismiss, PromptYes, PromptNo}` (Task 1).
- Produces: `Dialog::draw(..., hits: &mut HitList, ...)`,
  `Prompt::draw(..., hits: &mut HitList, ...)`, `Dialog::click`, `Prompt::click`.

A dialog is one big dismiss region. A yes/no prompt is two. Because regions
come from whatever actually drew, modal precedence needs no new rule: while a
dialog is up, only the dialog contributed regions.

- [ ] **Step 1: Write the failing tests**

In `shell/src/dialog.rs`:

```rust
#[test]
fn a_dialog_publishes_one_dismiss_region_covering_its_panel() {
    let layout = test_layout();
    let mut dialog = Dialog::default();
    dialog.open("Header", "Body");

    let mut list = DrawList::new();
    let mut hits = HitList::new();
    dialog.draw(&mut list, &mut hits, &layout, &Palette::default());

    let region = hits.regions().first().expect("one region");
    assert_eq!(region.target, HitTarget::DialogDismiss);
    let cx = region.rect.x + region.rect.w / 2;
    let cy = region.rect.y + region.rect.h / 2;
    assert_eq!(hits.hit(cx, cy), Some(HitTarget::DialogDismiss));
}

#[test]
fn clicking_a_dialog_advances_it_like_any_key() {
    let mut by_key = Dialog::default();
    by_key.open("Header", "Body");
    let keyed = by_key.advance();

    let mut by_click = Dialog::default();
    by_click.open("Header", "Body");
    let clicked = by_click.click(HitTarget::DialogDismiss);

    assert_eq!(clicked, keyed);
}
```

In `shell/src/prompt.rs`:

```rust
#[test]
fn clicking_yes_matches_pressing_y() {
    let mut by_key = Prompt::default();
    by_key.yes_no("Header", "Body");
    let keyed = by_key.update(&Keys::pressed(&[Key::Y]));

    let mut by_click = Prompt::default();
    by_click.yes_no("Header", "Body");
    let clicked = by_click.click(HitTarget::PromptYes);

    assert_eq!(keyed, PromptResult::Yes);
    assert_eq!(clicked, keyed);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p openbounty-shell dialog:: prompt::`
Expected: FAIL to compile.

- [ ] **Step 3: Publish the regions**

In `Dialog::draw`, after the panel rect is drawn, using that same rect:

```rust
        hits.push(panel, HitTarget::DialogDismiss);
```

In `Prompt::draw`, for a yes/no prompt, publish the two option rects as they
are drawn, exactly as the town rows do — same rect, one line apart:

```rust
        hits.push(yes_rect, HitTarget::PromptYes);
        hits.push(no_rect, HitTarget::PromptNo);
```

If the prompt currently draws its options as one text line rather than
separate rects, split the line into two rects at the same coordinates it
already renders, so the drawn glyphs and the regions still come from one
computation.

- [ ] **Step 4: Add the click resolvers**

```rust
    /// Same outcome `advance` returns for any key.
    pub fn click(&mut self, target: HitTarget) -> bool {
        match target {
            HitTarget::DialogDismiss => self.advance(),
            _ => false,
        }
    }
```

```rust
    /// Same outcome `update` returns for Y / N.
    pub fn click(&mut self, target: HitTarget) -> PromptResult {
        match target {
            HitTarget::PromptYes => self.answer(PromptResult::Yes),
            HitTarget::PromptNo => self.answer(PromptResult::No),
            _ => PromptResult::None,
        }
    }
```

Use whatever internal method `update` already calls to settle an answer rather
than duplicating its bookkeeping. If there is no such method, extract one from
`update` first and have both call it.

- [ ] **Step 5: Dispatch in the loop**

In `main.rs`, in the dialog and prompt branches of the input chain, take the
click first and fall back to the keyboard, mirroring Task 5's town branch.

- [ ] **Step 6: Run the tests to verify they pass**

Run: `cargo test -p openbounty-shell`
Expected: PASS.

- [ ] **Step 7: Verify modal precedence by eye**

Run the game, open a town, then trigger a dialog over it. Clicking where a
town row sits must dismiss the dialog and must NOT run the row.

- [ ] **Step 8: Format, full suite, then ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: click dialogs and prompts`

---

### Task 8: The remaining views

**Files:**
- Modify: `shell/src/views.rs` (one surface at a time)

**Interfaces:**
- Consumes: everything from Tasks 1-7.
- Produces: new `HitTarget` variants and one `*_click` resolver per surface.

Surfaces, each its own commit: **gate picker**, **spells**, **recruit
soldiers**, **home castle**, **own castle**, **dwelling**, **army**,
**character**, **contract**, **controls**.

For each surface, repeat exactly this cycle. Do not batch them — one surface
per commit keeps a regression attributable.

- [ ] **Step 1: Add the `HitTarget` variant**

In `shell/src/pointer.rs`, add one variant wrapping that view's existing row
type, e.g.:

```rust
    GateRow(usize),
```

- [ ] **Step 2: Write the two failing tests**

Copy the shapes from Task 4, substituting the surface. Both are required:

Build the fixture by copying the setup from that surface's EXISTING draw test
— for the gate that is `the_gate_picker_draws_two_columns_and_the_footer`
(`shell/src/views.rs`), which already constructs the destinations and calls
`gate_open`. Worked example for the gate:

```rust
#[test]
fn the_gate_hit_regions_line_up_with_the_drawn_rows() {
    // Same setup as the_gate_picker_draws_two_columns_and_the_footer.
    let (layout, destinations) = gate_fixtures();
    let mut views = Views::default();
    views.gate_open(&destinations, false);

    let mut list = DrawList::new();
    let mut hits = HitList::new();
    views.draw_gate(&mut list, &mut hits, &layout, &Palette::default());

    for index in 0..views.gate_count() {
        let region = hits
            .regions()
            .iter()
            .find(|r| r.target == HitTarget::GateRow(index))
            .unwrap_or_else(|| panic!("row {index} published no region"));
        let cx = region.rect.x + region.rect.w / 2;
        let cy = region.rect.y + region.rect.h / 2;
        assert_eq!(hits.hit(cx, cy), Some(HitTarget::GateRow(index)));
    }
}

#[test]
fn clicking_a_gate_row_matches_selecting_it_by_key() {
    let (_, destinations) = gate_fixtures();

    // Keyboard: move down to row 1, then Enter, and take what was chosen.
    let mut by_key = Views::default();
    by_key.gate_open(&destinations, false);
    by_key.gate_update(&Keys::pressed(&[Key::Down]));
    by_key.gate_update(&Keys::pressed(&[Key::Enter]));
    let keyed = by_key.gate_take_chosen();

    // Pointer: click row 1 directly.
    let mut by_click = Views::default();
    by_click.gate_open(&destinations, false);
    by_click.gate_click(HitTarget::GateRow(1));
    let clicked = by_click.gate_take_chosen();

    assert_eq!(keyed, Some(1));
    assert_eq!(clicked, keyed);
}
```

`gate_update` returns `bool` and the choice is collected with
`gate_take_chosen() -> Option<usize>`, so `gate_click` sets the same internal
chosen slot rather than returning an outcome enum. Match whatever each surface
already does — the rule is that the pointer produces the surface's EXISTING
result, not that every surface has the same shape.

- [ ] **Step 3: Run them to verify they fail**

Run: `cargo test -p openbounty-shell views::tests::the_gate_hit`
Expected: FAIL.

- [ ] **Step 4: Publish regions from the draw loop**

Push the region from the same rect the row draws, immediately after the draw
call, as Task 4 does. If the surface has an overlay that covers its rows,
suppress the row regions while it is up.

- [ ] **Step 5: Add the `*_click` resolver**

Return the surface's existing outcome enum. Do not invent a new one.

- [ ] **Step 6: Dispatch in the loop**

Add the click-first-then-keyboard branch in `main.rs` for that surface.

- [ ] **Step 7: Run tests, format, ask before committing**

```bash
cargo fmt --all && cargo fmt --all --check && cargo test --workspace
```

Suggested message: `feat: click the gate destination picker` (adjust per surface)

- [ ] **Step 8: When every surface is done, update the docs**

In `specs/shell-renderer-plan.md`, mark work order item 9 as done and note
which surfaces are covered. In `specs/play-verification.md`, add a mouse line
to the affected sections so the play pass covers it.

Suggested message: `docs: record the mouse support rollout`

---

## Review checklist

Before declaring the feature done:

- [ ] `cargo test --workspace` passes, engine test count unchanged from `3d070a6`.
- [ ] `cargo test -p openbounty-engine --test render_free` passes.
- [ ] `grep -rn "macroquad" shell/src/ --include=*.rs | grep -v backend.rs` is empty.
- [ ] Every clickable surface has BOTH an anti-drift test and a dispatch-equivalence test.
- [ ] No `HitTarget` reaches a transaction with no keyboard equivalent.
- [ ] Hover never moves a keyboard cursor.
