//! The 256-colour VGA palette, porting `src/palette.c`.
//!
//! The pack ships `palettes/palette.bin`: 256 RGB triples, 768 bytes, already
//! scaled to 8 bits. Entries 0..15 are the canonical VGA sixteen that chrome
//! and text draw with; 16..255 belong to sprites, tiles and backdrops.
//!
//! Parsing is pure, so it lives here rather than in a backend: a view can be
//! tested against real colours with nothing loaded.
//!
//! The file is authoritative over the fallback, including for the named
//! sixteen, and the two really do differ: the shipped pack's entry 7 is white
//! rather than the canonical light grey, so [`Pal::Grey`] draws white in a
//! real run. Never assume a named entry equals its fallback value.

use crate::draw::Color;

pub const PALETTE_SIZE: usize = 256;

/// Bytes a palette file must have. A shorter file is rejected outright.
pub const PALETTE_BIN_LEN: usize = PALETTE_SIZE * 3;

/// The named first sixteen, C's `PalIndex`. Anything beyond these is referred
/// to by raw index.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Pal {
    Black = 0,
    DBlue = 1,
    DGreen = 2,
    DCyan = 3,
    DRed = 4,
    Magenta = 5,
    Brown = 6,
    Grey = 7,
    DGrey = 8,
    Blue = 9,
    Green = 10,
    Cyan = 11,
    Red = 12,
    Violet = 13,
    Yellow = 14,
    White = 15,
}

/// The canonical sixteen, used when no palette file loads. Chrome and text
/// still render correctly with only these, which is the point: a missing
/// palette degrades the art, not the interface.
const FALLBACK_16: [Color; 16] = [
    Color::rgb(0, 0, 0),
    Color::rgb(0, 0, 170),
    Color::rgb(0, 170, 0),
    Color::rgb(0, 170, 170),
    Color::rgb(170, 0, 0),
    Color::rgb(170, 0, 170),
    Color::rgb(170, 85, 0),
    Color::rgb(170, 170, 170),
    Color::rgb(85, 85, 85),
    Color::rgb(85, 85, 255),
    Color::rgb(85, 255, 85),
    Color::rgb(85, 255, 255),
    Color::rgb(255, 85, 85),
    Color::rgb(255, 85, 255),
    Color::rgb(255, 255, 85),
    Color::rgb(255, 255, 255),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    colors: [Color; PALETTE_SIZE],
}

impl Default for Palette {
    fn default() -> Self {
        Self::fallback()
    }
}

impl Palette {
    /// The sixteen canonical colours, with everything above them opaque black.
    pub const fn fallback() -> Self {
        let mut colors = [Color::BLACK; PALETTE_SIZE];
        let mut i = 0;
        while i < FALLBACK_16.len() {
            colors[i] = FALLBACK_16[i];
            i += 1;
        }
        Self { colors }
    }

    /// Read `palette.bin`. A file shorter than 768 bytes is refused; C keeps
    /// the fallback in that case, so a caller should too. Trailing bytes are
    /// ignored, and every entry is opaque -- the file carries no alpha.
    pub fn from_bin(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < PALETTE_BIN_LEN {
            return None;
        }
        let mut colors = [Color::BLACK; PALETTE_SIZE];
        for (index, slot) in colors.iter_mut().enumerate() {
            *slot = Color::rgb(bytes[index * 3], bytes[index * 3 + 1], bytes[index * 3 + 2]);
        }
        Some(Self { colors })
    }

    /// Read `palette.bin`, falling back to the canonical sixteen when it is
    /// missing or too short. This is what `palette_init` does as a whole.
    pub fn from_bin_or_fallback(bytes: Option<&[u8]>) -> Self {
        bytes
            .and_then(Self::from_bin)
            .unwrap_or_else(Self::fallback)
    }

    /// Colour at a raw index. Out of range is opaque black, never a panic: a
    /// bad index in pack data must not take the game down mid-frame.
    pub fn at(&self, index: usize) -> Color {
        self.colors.get(index).copied().unwrap_or(Color::BLACK)
    }

    /// Colour of a named entry, C's `PAL_CLR`.
    pub fn clr(&self, name: Pal) -> Color {
        self.colors[name as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fallback_carries_the_canonical_sixteen_and_black_above_them() {
        let palette = Palette::fallback();
        assert_eq!(palette.clr(Pal::Black), Color::rgb(0, 0, 0));
        assert_eq!(palette.clr(Pal::White), Color::rgb(255, 255, 255));
        assert_eq!(palette.clr(Pal::Yellow), Color::rgb(255, 255, 85));
        assert_eq!(palette.clr(Pal::DBlue), Color::rgb(0, 0, 170));
        // Everything past the sixteen is opaque black, not transparent, so a
        // sprite drawn without a palette shows as silhouette rather than
        // vanishing.
        assert_eq!(palette.at(16), Color::rgb(0, 0, 0));
        assert_eq!(palette.at(255).a, 255);
    }

    #[test]
    fn a_palette_file_is_768_rgb_bytes() {
        let mut bytes = vec![0u8; PALETTE_BIN_LEN];
        bytes[0..3].copy_from_slice(&[1, 2, 3]);
        bytes[765..768].copy_from_slice(&[250, 251, 252]);

        let palette = Palette::from_bin(&bytes).expect("768 bytes is a whole palette");
        assert_eq!(palette.at(0), Color::rgb(1, 2, 3));
        assert_eq!(palette.at(255), Color::rgb(250, 251, 252));
        // The file has no alpha column, so every entry is opaque.
        assert!(palette.colors.iter().all(|c| c.a == 255));
    }

    #[test]
    fn a_short_file_is_refused_and_leaves_the_fallback_standing() {
        assert_eq!(Palette::from_bin(&[0u8; PALETTE_BIN_LEN - 1]), None);
        assert_eq!(
            Palette::from_bin_or_fallback(Some(&[0u8; 8])),
            Palette::fallback()
        );
        assert_eq!(Palette::from_bin_or_fallback(None), Palette::fallback());
    }

    #[test]
    fn a_loaded_palette_overrides_the_canonical_sixteen_too() {
        // The file owns all 256 entries, including the named ones, so a pack
        // can restyle the chrome without touching any code.
        let mut bytes = vec![0u8; PALETTE_BIN_LEN];
        bytes[45..48].copy_from_slice(&[10, 20, 30]);
        let palette = Palette::from_bin(&bytes).expect("whole palette");
        assert_eq!(palette.clr(Pal::White), Color::rgb(10, 20, 30));
    }

    #[test]
    fn an_index_past_the_table_is_black_rather_than_a_panic() {
        assert_eq!(Palette::fallback().at(999), Color::BLACK);
    }
}
