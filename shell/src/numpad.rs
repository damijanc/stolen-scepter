//! The tappable digit strip for a typed count: `0`-`9`, delete, max, OK and ESC in
//! one row. A dwelling's "how many" prompt and the castle's recruit box both
//! draw it, so a touch player can enter any count in a few taps.
//!
//! DIVERGENCE: C reads these counts from the keyboard only. The strip is
//! drawn and published here; what a tap DOES is the owning surface's, which
//! routes `HitTarget::Pad(key)` into the same helpers its key branch calls,
//! so a tapped count and a typed one go through one set of guards.

use crate::draw::{DrawList, Rect};
use crate::palette::{Pal, Palette};
use crate::pointer::{HitList, HitTarget};

/// One key of the strip.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PadKey {
    Digit(u8),
    /// Delete the last digit (Backspace).
    Del,
    /// Fill in the most the player can take.
    Max,
    /// Confirm the count (Enter).
    Ok,
    /// Cancel the count (Escape). Its label is the keyboard hint, too.
    Esc,
}

/// The keys left to right.
pub const PAD_KEYS: [PadKey; 14] = [
    PadKey::Digit(0),
    PadKey::Digit(1),
    PadKey::Digit(2),
    PadKey::Digit(3),
    PadKey::Digit(4),
    PadKey::Digit(5),
    PadKey::Digit(6),
    PadKey::Digit(7),
    PadKey::Digit(8),
    PadKey::Digit(9),
    PadKey::Del,
    PadKey::Max,
    PadKey::Ok,
    PadKey::Esc,
];

/// Width units per key: a digit is one, a word key two (18 in all).
fn units(key: PadKey) -> i32 {
    match key {
        PadKey::Digit(_) => 1,
        _ => 2,
    }
}

fn label(key: PadKey) -> String {
    match key {
        PadKey::Digit(d) => d.to_string(),
        PadKey::Del => "<".to_owned(),
        PadKey::Max => "MAX".to_owned(),
        PadKey::Ok => "OK".to_owned(),
        PadKey::Esc => "ESC".to_owned(),
    }
}

/// Split `strip` into the 14 key cells, left to right, with no gaps; the
/// rounding remainder lands on the last cell.
pub fn cells(strip: Rect) -> [(Rect, PadKey); 14] {
    let total: i32 = PAD_KEYS.iter().map(|k| units(*k)).sum();
    let mut before = 0;
    PAD_KEYS.map(|key| {
        let left = strip.x + strip.w * before / total;
        before += units(key);
        let right = strip.x + strip.w * before / total;
        (Rect::new(left, strip.y, right - left, strip.h), key)
    })
}

/// Draw the strip and publish `HitTarget::Pad(key)` for each cell.
pub fn draw(list: &mut DrawList, hits: &mut HitList, strip: Rect, palette: &Palette) {
    for (rect, key) in cells(strip) {
        list.button(
            rect,
            &label(key),
            palette.clr(Pal::Blue),
            palette.clr(Pal::Yellow),
        );
        hits.push(rect, HitTarget::Pad(key));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_tile_the_strip_without_gaps() {
        let strip = Rect::new(10, 100, 237, 9);
        let c = cells(strip);
        assert_eq!(c[0].0.x, 10);
        assert_eq!(c[13].0.x + c[13].0.w, 10 + 237);
        for k in 1..14 {
            assert_eq!(c[k].0.x, c[k - 1].0.x + c[k - 1].0.w);
            assert_eq!((c[k].0.y, c[k].0.h), (100, 9));
        }
        assert_eq!(c.map(|(_, key)| key), PAD_KEYS);
    }

    #[test]
    fn word_keys_are_twice_as_wide_as_digits() {
        let c = cells(Rect::new(0, 0, 180, 9));
        assert_eq!(c[0].0.w, 10);
        assert_eq!(c[10].0.w, 20);
        assert_eq!(c.len(), 14);
    }

    #[test]
    fn draw_publishes_one_region_per_key_with_its_label() {
        let mut list = DrawList::new();
        let mut hits = HitList::new();
        let strip = Rect::new(0, 50, 160, 9);
        draw(&mut list, &mut hits, strip, &Palette::fallback());
        let targets: Vec<_> = hits.regions().iter().map(|r| r.target).collect();
        assert_eq!(targets, PAD_KEYS.map(HitTarget::Pad).to_vec());
        for label in ["0", "9", "<", "MAX", "OK", "ESC"] {
            assert!(list.has_text_containing(label), "{label} drawn");
        }
    }
}
