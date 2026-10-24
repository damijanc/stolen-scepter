//! The in-game mouse pointer.
//!
//! **DIVERGENCE, deliberate.** The DOS game has no mouse, so neither C nor the
//! pack has pointer art (the pack's `combat/cursor_*` frames are the combat
//! target ring, not a pointer). The Amiga edition drew its own sprite pointer
//! that scaled with the screen; this does the same with a shell-authored arrow,
//! because the macOS system arrow is black and all but vanishes over the
//! game's mostly black frame. See `specs/mouse-support-design.md`.
//!
//! The arrow is drawn in LOGICAL pixels as ordinary `DrawCmd::Rect` runs, last
//! in the frame, so the present layer's integer scale enlarges it with
//! everything else. No window point or framebuffer pixel appears here.

use crate::draw::{Color, DrawList, Rect};

/// `X` is outline, `.` is fill, a space is transparent. The hotspot is the
/// top-left cell, the arrow's tip.
pub const ARROW: [&str; 16] = [
    "X",
    "XX",
    "X.X",
    "X..X",
    "X...X",
    "X....X",
    "X.....X",
    "X......X",
    "X.......X",
    "X....XXXXX",
    "X..X..X",
    "X.X X..X",
    "XX  X..X",
    "     X..X",
    "     X..X",
    "      XX",
];

pub const OUTLINE: Color = Color::BLACK;
pub const FILL: Color = Color::WHITE;
const GOLD: Color = Color::rgb(232, 196, 72);
const RED: Color = Color::rgb(214, 40, 40);

/// Which pointer to draw. Combat swaps the arrow for one that says what a
/// click on the hovered cell would do; everywhere else it is the arrow.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Shape {
    #[default]
    Arrow,
    /// Step onto (or fly to) this cell.
    Move,
    /// Melee the stack on this cell.
    Attack,
    /// Shoot the stack on this cell.
    Shoot,
    /// Cast the armed spell on this cell.
    Target,
    /// A click here does nothing.
    No,
}

/// The combat shapes as FILL masks: `#` white, `y` gold, `r` red, a space
/// transparent. The black outline is added around the fill when drawn, so
/// these stay readable over grass and over a dark troop alike. The hotspot is
/// the centre cell.
const MOVE: [&str; 11] = [
    "     #     ",
    "    ###    ",
    "   #####   ",
    "  #  #  #  ",
    " #   #   # ",
    "###########",
    " #   #   # ",
    "  #  #  #  ",
    "   #####   ",
    "    ###    ",
    "     #     ",
];
const ATTACK: [&str; 11] = [
    "         ##",
    "        ###",
    "       ### ",
    "      ###  ",
    " y   ###   ",
    "  y ###    ",
    "   y##     ",
    "   yy      ",
    "  y  y     ",
    " y         ",
    "y          ",
];
const SHOOT: [&str; 11] = [
    "     #     ",
    "   #####   ",
    "  #  #  #  ",
    " #       # ",
    " #       # ",
    "###  r  ###",
    " #       # ",
    " #       # ",
    "  #  #  #  ",
    "   #####   ",
    "     #     ",
];
const TARGET: [&str; 11] = [
    "     y     ",
    "     y     ",
    "    yyy    ",
    "y   yyy   y",
    " yyyyyyyyy ",
    "  yyyyyyy  ",
    "   yyyyy   ",
    "  yyy yyy  ",
    "  yy   yy  ",
    " yy     yy ",
    " y       y ",
];
const NO: [&str; 11] = [
    "   rrrrr   ",
    "  rr   rr  ",
    " rr    rrr ",
    "rr    rr rr",
    "r    rr   r",
    "r   rr    r",
    "r  rr     r",
    "rr rr    rr",
    " rrr    rr ",
    "  rr   rr  ",
    "   rrrrr   ",
];

/// Push `shape` with its hotspot at logical `(x, y)`.
pub fn push_shape(list: &mut DrawList, at: Option<(i32, i32)>, shape: Shape) {
    let mask = match shape {
        Shape::Arrow => return push_pointer(list, at),
        Shape::Move => &MOVE,
        Shape::Attack => &ATTACK,
        Shape::Shoot => &SHOOT,
        Shape::Target => &TARGET,
        Shape::No => &NO,
    };
    let Some((x, y)) = at else {
        return;
    };
    let (w, h) = (mask[0].len() as i32, mask.len() as i32);
    let (ox, oy) = (x - w / 2, y - h / 2);
    let lit = |cx: i32, cy: i32| -> Option<u8> {
        if cx < 0 || cy < 0 || cx >= w || cy >= h {
            return None;
        }
        let b = mask[cy as usize].as_bytes()[cx as usize];
        (b != b' ').then_some(b)
    };
    // The outline: every empty cell touching the fill, one pixel past the
    // mask's edge included.
    for cy in -1..=h {
        for cx in -1..=w {
            if lit(cx, cy).is_some() {
                continue;
            }
            let touches = (-1..=1).any(|dy| (-1..=1).any(|dx| lit(cx + dx, cy + dy).is_some()));
            if touches {
                list.rect(Rect::new(ox + cx, oy + cy, 1, 1), OUTLINE);
            }
        }
    }
    for cy in 0..h {
        for cx in 0..w {
            let color = match lit(cx, cy) {
                Some(b'y') => GOLD,
                Some(b'r') => RED,
                Some(_) => FILL,
                None => continue,
            };
            list.rect(Rect::new(ox + cx, oy + cy, 1, 1), color);
        }
    }
}

/// Push the arrow with its tip at logical `(x, y)`. `None` -- the pointer is
/// off the game area -- draws nothing, and the OS arrow shows instead.
pub fn push_pointer(list: &mut DrawList, at: Option<(i32, i32)>) {
    let Some((x, y)) = at else {
        return;
    };
    for (row, line) in ARROW.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut col = 0;
        while col < bytes.len() {
            let cell = bytes[col];
            let start = col;
            while col < bytes.len() && bytes[col] == cell {
                col += 1;
            }
            let color = match cell {
                b'X' => OUTLINE,
                b'.' => FILL,
                _ => continue,
            };
            list.rect(
                Rect::new(x + start as i32, y + row as i32, (col - start) as i32, 1),
                color,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;

    /// Every rect the pointer pushed, as `(rect, color)`.
    fn rects(list: &DrawList) -> Vec<(Rect, Color)> {
        list.as_slice()
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Rect { rect, color } => Some((*rect, *color)),
                _ => None,
            })
            .collect()
    }

    /// The colour the pushed rects paint at one logical pixel, if any.
    fn painted(list: &DrawList, px: i32, py: i32) -> Option<Color> {
        rects(list)
            .into_iter()
            .filter(|(r, _)| px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h)
            .map(|(_, c)| c)
            .last()
    }

    #[test]
    fn off_the_game_area_draws_nothing() {
        let mut list = DrawList::new();
        push_pointer(&mut list, None);
        assert!(list.is_empty());
    }

    #[test]
    fn the_hotspot_is_the_tip_and_is_outlined() {
        let mut list = DrawList::new();
        push_pointer(&mut list, Some((100, 50)));
        assert_eq!(painted(&list, 100, 50), Some(OUTLINE));
        // Nothing above or left of the tip.
        assert_eq!(painted(&list, 99, 50), None);
        assert_eq!(painted(&list, 100, 49), None);
    }

    #[test]
    fn the_pushed_runs_reproduce_the_bitmap_exactly() {
        let mut list = DrawList::new();
        push_pointer(&mut list, Some((7, 3)));
        for (row, line) in ARROW.iter().enumerate() {
            for col in 0..12 {
                let want = match line.as_bytes().get(col) {
                    Some(b'X') => Some(OUTLINE),
                    Some(b'.') => Some(FILL),
                    _ => None,
                };
                assert_eq!(
                    painted(&list, 7 + col as i32, 3 + row as i32),
                    want,
                    "row {row} col {col}"
                );
            }
        }
    }

    #[test]
    fn runs_never_overlap_so_each_pixel_is_painted_once() {
        let mut list = DrawList::new();
        push_pointer(&mut list, Some((0, 0)));
        let area: i32 = rects(&list).iter().map(|(r, _)| r.w * r.h).sum();
        let cells = ARROW
            .iter()
            .flat_map(|l| l.bytes())
            .filter(|b| *b != b' ')
            .count() as i32;
        assert_eq!(area, cells);
    }

    #[test]
    fn the_fill_is_enclosed_by_outline_on_every_row() {
        // Each row starts with outline, so the white body never touches the
        // pointer's left edge, where it would melt into light art.
        for line in ARROW {
            assert!(line.starts_with('X') || line.starts_with(' '), "{line:?}");
            assert!(!line.ends_with('.'), "{line:?} ends in fill");
        }
    }
}
