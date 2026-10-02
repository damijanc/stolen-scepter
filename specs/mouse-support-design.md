# Mouse Support Design

Status: agreed 2026-09-20 for menus, views, dialogs and prompts; WIDENED
2026-10-02 by the user's decision P11 (`specs/play-verification-findings.md`) to
full mouse support, Amiga-style: the drawn pointer, the Game Menu and slot
picker, the startup wizard, combat (`specs/combat-mouse-design.md`) and
overworld click-to-move (`specs/overworld-click-to-move-design.md`). All of it
has shipped. The 2026-09-20 non-goals for combat, click-to-move and the wizard
are reversed; the rest stand (see Non-goals).

Read `AGENTS.md` and `specs/shell-renderer-plan.md` first. Everything there
still applies -- parity-first translation, one-line commit messages, approval
before every commit.

## Why this is a deliberate divergence

The DOS release this port targets is keyboard-only, and OpenBounty's C shell has
no pointer code. The Amiga edition of King's Bounty did have mouse support, so
pointer input is not foreign to the game, but nothing in the C tree specifies
its behaviour and this design does not attempt to reconstruct it.

**DIVERGENCE, deliberate.** Mouse support is a presentation-layer addition, made
in service of the eventual modernisation the shell plan already anticipates
(modern render mode, higher resolution, an art overhaul). It is confined to the
shell by construction:

- The engine gains no presentation code. `engine/tests/render_free.rs` must
  still pass. The one engine addition for the mouse is the read-only
  `step::step_preview` query, which `game_step_basic` itself uses for its
  can-move decision, so click-to-move reads walkability from the engine
  instead of re-deriving it.
- The pointer cannot reach a transaction the keyboard cannot. Every click
  resolves to a value the keyboard path already produces, so all existing
  engine guards, ordering and tests remain the only authority on what is legal.

That second rule is the one to defend in review. It is what keeps this from
becoming the kind of invented behaviour `AGENTS.md` warns against.

## Decisions

### 1. Hit regions are emitted by the code that draws

A view's draw function fills a `HitList` alongside the `DrawList`, pushing the
**same rect** it just drew. One rect, two consumers, one line apart, so the two
cannot drift.

Rejected, with reasons, so this is not relitigated:

- **Tag `DrawCmd` with a hit id.** Most integrated -- hover could be derived
  automatically -- but it widens the draw-command vocabulary that every view and
  `backend.rs` share. The shell plan calls that seam the thing that makes later
  changes cheap; widening it for input trades away the wrong thing.
- **A separate `hit_regions()` per view.** Reads well, but recomputes the layout
  arithmetic `draw()` already does. The two drift the moment a row height
  changes in one and not the other, and a drifted region still renders
  perfectly -- it only misbehaves under the pointer. That is the failure this
  codebase is least equipped to notice.

### 2. A click produces the view's existing outcome value

`Views::town_update` already returns `TownInput::RunRow(TownRow)`, and `main.rs`
already handles that variant. The pointer produces the same value. It does NOT
synthesise a keypress: that would smuggle events through C's load-bearing
else-if ordering and make the input path lie about where they came from.

### 3. Hover highlights, nothing more

The pointer highlights the actionable row under it, reusing the styling the
keyboard cursor already draws. This holds for the original views (town,
castles, recruit, spells, gate, controls). The surfaces added since follow 3b
instead -- the Game Menu, the slot picker, the difficulty table and the combat
target ring move their single highlight when the pointer ENTERS a row or cell.

### 3b. In the Game Menu and slot picker, hover MOVES the cursor (P10)

Revised in play: there, a hover highlight beside the keyboard cursor showed two
yellow rows and read as "the mouse does nothing". The menu and picker therefore
keep ONE highlight -- the pointer ENTERING a row moves the cursor to it, so the
yellow row is always what Enter or a click picks. A pointer resting on the same
row moves nothing, so the arrow keys keep working under a still mouse. The
earlier views (town, castles, recruit, spells, gate, controls) still follow
decision 3 and highlight without moving.

### 3a. A click off a menu panel is ESC (P10)

The Game Menu and the slot picker publish a full-screen backdrop region before
their rows (`MenuClose`, `SlotCancel`), so a click off the panel gives the value
ESC gives. That is always a step BACK: the menu closes, and the picker returns
to the Game Menu in-game or to class select at startup. Only ESC on class
select itself quits, and class select publishes no backdrop, so no stray click
can quit the game.

### 3c. The startup wizard (reverses a 2026-09-20 non-goal, per P11)

A click is the key it stands for: anywhere on a splash, the credits or the
intro is "any key"; a figure on class select is its letter; the class-select
hint bar, which names `L`, is `L`; a picker row is Enter on it; a difficulty row
is "select, then Enter" and, like the menu, the pointer entering a row moves the
`>` marker. Name entry stays keyboard, and the difficulty rows are inert until
the name is confirmed, exactly as the arrows are.

The figure rectangles (`startup::CLASS_FIGURES`) are authored by eye from the
stock art -- nothing in C or the pack locates them. They are relative to the
picker's 288x184 placement, which the HD pack's 4x art shares. A pack with a
different composition would need its own geometry; that waits for the asset
update. Class select publishes no backdrop, so no stray click can quit.

### 4. The shell draws its own pointer (revised 2026-10-02, P3)

The first cut kept the OS arrow. In play (P3) the macOS arrow, black with a thin
white edge, all but vanished over the game's mostly black frame. Diagnosis
ruled out a hidden cursor: in-process, `CGCursorIsVisible` was 1, the window was
key and `NSCursor.currentCursor` was the arrow. It was a contrast problem.

**DIVERGENCE, deliberate.** The DOS game and C have no pointer, and the pack's
`combat/cursor_*` frames are the combat target ring, not a pointer. Like the
Amiga edition's sprite pointer, `shell/src/cursor.rs` draws a shell-authored
10x16 arrow (white fill, black outline, hotspot at the tip) as `DrawCmd::Rect`
runs in LOGICAL pixels, last in every frame, so the integer-scale blit enlarges
it with the game. The OS arrow is hidden only while the shell's arrow is
drawn -- over the game's blit rect -- and shows over the letterbox and outside
the window (`Backend::set_os_cursor_visible`).

Coordinate spaces, since P1 was a points/pixels mix-up: `mouse_position` and
`Backend::window_size` are both window POINTS (macroquad divides out the DPI
scale); `Present::window_to_logical` turns points into logical pixels in
`main.rs::logical_pointer`, the one conversion both hit-testing and the drawn
pointer share. Only the scissor rect is in framebuffer pixels.

## Components

| Concern | Location |
| --- | --- |
| `PointerSource` trait, `Button`, `HitTarget`, `HitRegion`, `HitList`, hit-test scan | `shell/src/pointer.rs` |
| `MacroquadPointer`, `set_os_cursor_visible` -- the only mouse-aware code | `shell/src/backend.rs` |
| `window_to_logical`, the inverse of `blit_rect` | `shell/src/present.rs` |
| The drawn pointer | `shell/src/cursor.rs` |
| Views, dialogs, prompts | `shell/src/views.rs`, `dialog.rs`, `prompt.rs` |
| Game Menu, slot picker | `shell/src/menu.rs`, `slot_picker.rs` |
| Startup wizard | `shell/src/startup.rs` |
| Combat cells, action menu, spell menu, `PointerInput` | `shell/src/combat.rs` |
| Map tile regions | `shell/src/map_render.rs` (`MapScene::publish_hits`) |
| Route planning and the walk | `shell/src/walk.rs`, over `engine/src/step.rs::step_preview` |
| `logical_pointer`, poll, hit-test, merge with the keyboard result | `shell/src/main.rs` |

`PointerSource` mirrors `KeySource`'s shape and exists for the same reason --
every hit-test and dispatch test runs headlessly, with no window:

```rust
pub trait PointerSource {
    fn position(&self) -> Option<(i32, i32)>;  // window POINTS (see decision 4)
    fn pressed(&self, button: Button) -> bool; // went down this frame
    fn down(&self, button: Button) -> bool;    // held
    fn released(&self, button: Button) -> bool;
}
```

`HitTarget` wraps each view's existing row vocabulary rather than inventing one:

```rust
pub enum HitTarget {
    TownRow(TownRow),
    GateRow(usize),
    Spell(i32),
    RecruitRow(usize),
    DialogDismiss,
    PromptYes,
    PromptNo,
    // ... one variant per actionable region, all reusing existing types
}
```

## Coordinate model

Hit regions live in **logical** coordinates -- the same space `DrawList` uses.
Nothing downstream of `present.rs` ever sees a window pixel.

`present.rs` already owns the forward mapping, `blit_rect(layout, win_w, win_h,
target)`. This design adds its inverse. Putting it there is the point: scale
selection, letterboxing and the overhang case are already reasoned about in that
one module.

Watch the overhang case. In legacy mode the scale floor (`SCALE_MIN` = 2) can
make the render target larger than the window, so `blit_rect` returns a
**negative origin** and the edges crop. That is deliberate. The inverse must
therefore accept logical coordinates that are off-screen, and the caller decides
they are out of bounds -- the inversion itself must not clamp.

## Data flow

Per frame, in `main.rs`:

1. `MacroquadPointer` reads window points and button edges.
2. `present.window_to_logical(...)` -> `Option<(i32, i32)>`; `None` when the
   pointer is in the letterbox outside the blit rect.
3. Hit-test the **previous** frame's `HitList` -> `Option<HitTarget>`.
4. Store as `views.hover` for this frame's draw.
5. On press, resolve the target into the view's existing outcome value and hand
   it to the caller exactly where the keyboard result goes today.

**One-frame latency, accepted.** The loop runs input -> update -> draw, so
regions produced during draw are consumed by input on the next frame.
Imperceptible at 60fps, and how immediate-mode UIs normally work. The real edge:
on the frame a view first opens its `HitList` is empty, so a click landing
exactly then does nothing. Accepted rather than reordering the loop, which would
disturb recently stabilised code.

## Hover rules

- In the original views hover is **purely visual**: it does not move the
  keyboard cursor. Where the two-highlight form confused play (P10), hover
  moves the single highlight instead, but only on ENTERING a row, so a still
  mouse never pins the cursor against the arrow keys (decision 3b).
- Hover clears when the pointer leaves the window or sits in the letterbox, so
  nothing stays lit.

## Modal precedence

The existing chain is combat, then dialog, then prompt, then views, then the
overworld. The pointer obeys it without a new rule: while a dialog is up, only
the dialog's regions are in the `HitList`, because only the dialog drew any.
Precedence falls out of regions coming from whatever actually drew.

## Failure modes

All degrade to "nothing happens":

- Pointer outside the blit rect -> `window_to_logical` yields `None`.
- Click on empty space -> hit-test yields `None`.
- A stale region from the previous frame can only name a target that view still
  owns, because the list is rebuilt wholesale each frame.

## Testing

Five categories, all headless via a `Pointer` test builder mirroring `Keys`:

1. **Coordinate inversion.** Window -> logical across scales, with letterboxing,
   and specifically the legacy overhang case where `blit_rect` returns a
   negative origin.
2. **Anti-drift.** For each view, the hit region for row N contains the centre
   of the rect that view drew for row N. This makes decision 1's guarantee real
   rather than aspirational, and it is the pattern every new view copies.
3. **Dispatch equivalence.** Clicking the boat row yields the same
   `TownInput::RunRow(TownRow::Boat)` that pressing `B` yields. The keystone
   test: it pins the two input paths together permanently.
4. **Hover.** Sets `views.hover`, does not move `town_cursor`, clears outside
   the blit rect.
5. **Modal precedence.** With a dialog open, a click at a town row's coordinates
   does not run the town row.

## Non-goals

Reversed 2026-10-02 (P11), and shipped: overworld click-to-move, combat, the
startup wizard, and the right button (ESC in combat only).

Still out of scope:

- Drag and the scroll wheel.
- Recruit/dwelling "how many" count entry, and hero name entry: keyboard-only.
- Pack-supplied pointer art. The shell draws its own arrow (decision 4); a
  pack-declared pointer sprite waits for the asset update.
- Touch input.
- Presentation code in the engine. `render_free` must still pass.

## Risks

- **A new view forgetting its `HitList` pushes.** Invisible -- it renders fine
  and only misbehaves under the pointer. Mitigation: make the anti-drift test
  the documented house style, and note it in the shell plan's work order.
- **The one-frame-empty `HitList`** when a view first opens (above).
- **Modern render mode.** Regions derive from `Layout`, so they *should* follow
  a resolution change for free. That is unverified until modern mode is actually
  exercised, and this document does not claim otherwise.

## Work order

Done, 2026-09-20 to 2026-09-22. The P11 widening then shipped as:
`70d82de` the drawn pointer (P3), `693ad41` the Game Menu and slot picker
(P10), `f7da7b7` the startup wizard, `5c77b59` the combat target ring and spell
menu (a parity fix, P12), `479a43b` combat clicks, `d2cf4c4` the engine's
`step_preview`, and `0d27e65` overworld click-to-move.

The original order, kept for reference:

1. `PointerSource`, `Button`, `HitRegion`, `HitList` and the hit-test scan in
   `shell/src/pointer.rs`, with the test builder. No callers yet.
2. `present::window_to_logical` plus its inversion tests, including the overhang
   case.
3. `MacroquadPointer` in `backend.rs`. Keeps macroquad confined there.
4. `HitList` population for ONE view (town), plus the anti-drift and dispatch
   equivalence tests. This establishes the pattern.
5. `main.rs` wiring: poll, hit-test, merge with the keyboard result.
6. Hover state and highlight rendering for town.
7. Roll out to the remaining views, dialogs and prompts, one slice per surface,
   each carrying its own anti-drift and dispatch-equivalence tests.
