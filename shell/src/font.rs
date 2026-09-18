//! The bitmap font's metrics, porting `src/bfont.c`.
//!
//! One 1024x8 strip holds 128 glyphs of 8x8, indexed by byte value, drawn as
//! white shapes on transparent so a draw tints them. Everything a view needs
//! in order to place text -- the cell size, the measurement, the centring --
//! is arithmetic over that fixed grid, so none of it needs the asset loaded.
//! Only the backend does.
//!
//! Text is measured in BYTES, as C's `bfont_measure` counts them. A byte of
//! 128 or more draws as a space, so a multi-byte UTF-8 character occupies its
//! byte count in blanks. That is the original's behaviour, and the pack's
//! strings are ASCII.

/// Glyph cell, which is also the advance: the font is fixed-pitch.
pub const GLYPH_W: i32 = 8;
pub const GLYPH_H: i32 = 8;

/// The highest byte the strip has a glyph for. Anything above draws blank.
pub const GLYPH_COUNT: usize = 128;

/// Line advance for `\n`, C's `bfont_line_height`.
pub const fn line_height() -> i32 {
    GLYPH_H
}

/// Which glyph a byte draws. Out-of-strip bytes fall back to the space glyph
/// rather than to nothing, so a stray byte leaves a gap of the right width.
///
/// The four spinner codes are redirected to the printable glyphs that carry
/// their shapes. C instead patches those glyphs INTO the strip's control
/// slots after loading, which needs pixel surgery on the image; redirecting
/// the index draws exactly the same thing with none.
pub const fn glyph_index(byte: u8) -> usize {
    let mut i = 0;
    while i < TWIRL_GLYPHS.len() {
        if TWIRL_GLYPHS[i].0 == byte {
            return TWIRL_GLYPHS[i].1 as usize;
        }
        i += 1;
    }
    if (byte as usize) < GLYPH_COUNT {
        byte as usize
    } else {
        b' ' as usize
    }
}

/// Glyphs the pack's font PNG does not export, which the original stores at
/// control-code positions. Each pair is `(control code, the printable glyph
/// carrying its shape)`.
///
/// These are the four spinner frames: `|`, `/`, `-`, `\`. [`glyph_index`]
/// applies the redirection, so nothing has to edit the loaded strip.
pub const TWIRL_GLYPHS: [(u8, u8); 4] = [(0x1D, b'|'), (0x05, b'/'), (0x1F, b'-'), (0x1C, b'\\')];

/// Width and height of `text` in pixels, porting `bfont_measure`.
///
/// The height is always at least one line, so empty text measures 0x8 -- C
/// seeds its result the same way.
pub fn measure(text: &str) -> (i32, i32) {
    let mut widest = 0;
    let mut line_w = 0;
    let mut height = GLYPH_H;
    for byte in text.as_bytes() {
        if *byte == b'\n' {
            widest = widest.max(line_w);
            line_w = 0;
            height += GLYPH_H;
        } else {
            line_w += GLYPH_W;
        }
    }
    (widest.max(line_w), height)
}

/// Width of `text` in pixels.
pub fn measure_width(text: &str) -> i32 {
    measure(text).0
}

/// Left edge that centres `text` on `cx`, porting `bfont_draw_centered`.
///
/// C halves the measured width with integer division, so an odd-width string
/// sits half a pixel left of true centre. Kept, because the original's text
/// lines up against art that was positioned against it.
pub fn centered_x(text: &str, cx: i32) -> i32 {
    cx - measure_width(text) / 2
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_line_measures_one_cell_per_byte() {
        assert_eq!(measure("Days Left:400"), (13 * GLYPH_W, GLYPH_H));
        assert_eq!(measure("A"), (8, 8));
    }

    #[test]
    fn empty_text_is_still_one_line_tall() {
        // C seeds the measurement at one line height and only ever adds, so
        // an empty string is 0x8 rather than 0x0. Layout code that stacks
        // lines depends on that.
        assert_eq!(measure(""), (0, GLYPH_H));
    }

    #[test]
    fn newlines_add_a_line_and_the_widest_one_wins() {
        assert_eq!(measure("ab\ncdef\ng"), (4 * GLYPH_W, 3 * GLYPH_H));
        // A trailing newline opens a line that stays empty, which still
        // counts towards the height.
        assert_eq!(measure("ab\n"), (2 * GLYPH_W, 2 * GLYPH_H));
    }

    #[test]
    fn centring_rounds_towards_the_left() {
        // 3 glyphs is 24 wide, so an exact 12 either side.
        assert_eq!(centered_x("abc", 100), 88);
        // 1 glyph is 8 wide: also exact.
        assert_eq!(centered_x("a", 100), 96);
        // The measured width is always a multiple of 8, so the truncation
        // only ever bites on an odd centre.
        assert_eq!(centered_x("abc", 101), 89);
    }

    #[test]
    fn a_byte_past_the_strip_draws_as_a_space() {
        assert_eq!(glyph_index(b'A'), 65);
        assert_eq!(glyph_index(127), 127);
        assert_eq!(glyph_index(128), b' ' as usize);
        assert_eq!(glyph_index(255), b' ' as usize);
    }

    #[test]
    fn a_multibyte_character_measures_by_its_bytes() {
        // C walks bytes, and every byte of a UTF-8 sequence is >= 128, so it
        // draws that many spaces. Matching it keeps a pack's column
        // arithmetic right even when a string is not ASCII.
        let text = "\u{e9}";
        assert_eq!(text.len(), 2);
        assert_eq!(measure(text), (2 * GLYPH_W, GLYPH_H));
    }

    #[test]
    fn a_spinner_code_draws_the_printable_glyph_that_has_its_shape() {
        // The font PNG leaves these control slots blank, so drawing the code
        // directly would show nothing where the spinner should turn.
        assert_eq!(glyph_index(0x1D), b'|' as usize);
        assert_eq!(glyph_index(0x05), b'/' as usize);
        assert_eq!(glyph_index(0x1F), b'-' as usize);
        assert_eq!(glyph_index(0x1C), b'\\' as usize);
        // A control code that is not one of the four is left alone.
        assert_eq!(glyph_index(0x01), 1);
    }

    #[test]
    fn the_twirl_codes_map_onto_printable_glyphs() {
        // The spinner is drawn with control codes the font PNG leaves empty,
        // so the loader has to copy these four glyphs into those slots.
        assert_eq!(TWIRL_GLYPHS.len(), 4);
        for (dst, src) in TWIRL_GLYPHS {
            assert!(dst < 0x20, "the destination is a control code");
            assert!(src.is_ascii_graphic(), "the source is a printable glyph");
        }
    }
}
