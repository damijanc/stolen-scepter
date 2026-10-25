//! Pointer input and the hit regions a view publishes for it.
//!
//! The trait mirrors [`crate::input::KeySource`] and exists for the same
//! reason: every hit-test and dispatch test runs headlessly, with no window.
//! `backend.rs` supplies the real implementation.
//!
//! Regions are in LOGICAL coordinates -- the same space `DrawList` uses -- so
//! nothing here ever sees a window pixel. See `specs/mouse-support-design.md`.

use crate::combat::CombatAction;
use crate::draw::Rect;
use crate::input::Action;
use crate::views::TownRow;

/// The buttons the shell reads. Right is ESC in combat (it backs out of a
/// picker or a combat menu); no other surface binds it.
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
    GateRow(usize),
    Spell(i32),
    RecruitRow(usize),
    HomeCastleRow(usize),
    OwnCastleSlot(usize),
    ControlsRow(usize),
    /// A read-only view: a click anywhere dismisses it, as any key does.
    ViewDismiss,
    DialogDismiss,
    PromptYes,
    PromptNo,
    /// A Game Menu entry, by drawn index.
    MenuEntry(usize),
    /// The screen around the Game Menu panel: a click there is ESC.
    MenuClose,
    /// A save-slot picker row. `SAVE_SLOT_COUNT` is the extra row.
    SlotRow(usize),
    /// The screen around the picker: a click there is ESC, which steps back
    /// (to class select at startup, to the Game Menu in-game).
    SlotCancel,
    /// A splash, the credits or the new-game intro: a click anywhere is the
    /// "any key" that skips it.
    StartupSkip,
    /// A class's figure on the class-select art, by class index (letter A-D).
    StartupClass(usize),
    /// The class-select hint bar, which names `L`: a click there is `L`.
    StartupLoad,
    /// A difficulty row on the create-game screen, in `Difficulty` order.
    Difficulty(usize),
    /// A combat field cell, by grid position.
    CombatCell(i32, i32),
    /// A row of the combat spell menu, in stock order (letter A-G).
    CombatSpell(usize),
    /// An entry of the combat action menu.
    CombatAction(CombatAction),
    /// The field around a combat menu: a click there is ESC.
    CombatBack,
    /// An overworld map tile in the viewport, by MAP coordinates.
    MapTile(i32, i32),
    /// A sidebar panel (`hud.rs`), by panel index. Panel 1 (siege) is
    /// deliberately never published: it has no single-key equivalent, so it
    /// stays inert.
    HudPanel(i32),
    /// A row of the hero's overworld action menu, carrying the exact `Action`
    /// its key would produce. Routed into `session.rs`'s `match input.action`.
    OverworldAction(Action),
    /// The map around the hero action menu: a click there closes the menu and
    /// never commits, mirroring `CombatBack`.
    OverworldMenuClose,
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

    /// The published regions, in draw order. For tests and hover.
    pub fn regions(&self) -> &[HitRegion] {
        &self.regions
    }

    /// The target at a logical point, or `None`. Scans in reverse so the
    /// LAST region pushed wins -- draw order is painter's order, so whatever
    /// drew on top is what the pointer hits.
    pub fn hit(&self, x: i32, y: i32) -> Option<HitTarget> {
        self.regions
            .iter()
            .rev()
            .find(|r| {
                x >= r.rect.x && y >= r.rect.y && x < r.rect.x + r.rect.w && y < r.rect.y + r.rect.h
            })
            .map(|r| r.target)
    }
}

/// What the shell reads each frame.
pub trait PointerSource {
    /// Window POINTS -- the space `Backend::window_size` reports, not
    /// framebuffer pixels (macroquad divides the DPI scale out of both).
    /// `None` only when the position is unknown; a known one may lie outside
    /// the window, because macOS keeps sending moves to the key window after
    /// the pointer leaves it. `Present::window_to_logical` rejects those.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::Rect;
    use crate::views::TownRow;

    #[test]
    fn the_hit_list_returns_the_region_under_a_point() {
        let mut hits = HitList::new();
        hits.push(
            Rect::new(10, 20, 100, 8),
            HitTarget::TownRow(TownRow::Contract),
        );
        hits.push(Rect::new(10, 28, 100, 8), HitTarget::TownRow(TownRow::Boat));

        assert_eq!(
            hits.hit(11, 21),
            Some(HitTarget::TownRow(TownRow::Contract))
        );
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
