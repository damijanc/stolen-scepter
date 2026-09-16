//! The draw-command vocabulary: what view code emits, and the list a backend
//! executes.
//!
//! View code never touches a graphics library. It appends commands in LOGICAL
//! coordinates -- the design space [`crate::layout`] measures -- and a backend
//! walks the list. That seam is what makes a resolution change or a backend
//! swap cheap, and it is what lets a view be tested with no window open: the
//! list IS the recording.
//!
//! The vocabulary is small because the C shell's is. Every drawing call in
//! `src/*.c` is one of six raylib primitives, and this module covers all of
//! them.
//!
//! Two shapes here are deliberately NOT raylib's:
//!
//! - There is no source rectangle. Every `DrawTexturePro` in the C shell
//!   passes the whole texture; the one exception is the bitmap font's glyph
//!   sub-rect, which [`DrawCmd::Text`] absorbs. Dropping it is what keeps
//!   view code headless -- it never has to know a texture's pixel size.
//! - Mirroring is [`Blit::flip_x`], not raylib's negative source width. The
//!   negative width is an API convention, and three call sites decide the
//!   mirror from game state (hero facing, combat side, the end cartoon), so
//!   an explicit flag is what they actually mean.

use openbounty_engine::map::TILE_ART_LEN;
use openbounty_engine::tables::{Facing, FixedStr};

/// A straight RGBA colour. The pack's `colors` block stores packed ARGB, and
/// the palette stores RGB triples; both land here.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// Draw a sprite unchanged. C passes raylib's `WHITE` as the tint for
    /// exactly this reason.
    pub const NO_TINT: Self = Self::WHITE;

    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Unpack a pack-declared `0xAARRGGBB`, porting `chrome.c`'s
    /// `color_from_packed`. A colour written without an alpha byte therefore
    /// comes out fully transparent, which is C's behaviour too.
    pub const fn from_argb(value: u32) -> Self {
        Self {
            r: ((value >> 16) & 0xFF) as u8,
            g: ((value >> 8) & 0xFF) as u8,
            b: (value & 0xFF) as u8,
            a: ((value >> 24) & 0xFF) as u8,
        }
    }
}

/// A rectangle in logical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }

    pub const fn right(&self) -> i32 {
        self.x + self.w
    }

    pub const fn bottom(&self) -> i32 {
        self.y + self.h
    }
}

/// Where a blit lands.
///
/// `Stretch` is the usual case: the destination comes from the layout and the
/// texture is scaled into it, so a 48x34 tile sprite fills a 48x34 cell
/// whatever its own pixel size. `Native` covers `combat_render`, the one place
/// C sizes the destination from the texture's own pixels instead -- a backend
/// resolves it, because a headless view does not know that size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Dst {
    Stretch(Rect),
    Native { x: i32, y: i32 },
}

impl Dst {
    pub const fn at(x: i32, y: i32) -> Self {
        Self::Native { x, y }
    }

    pub const fn rect(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self::Stretch(Rect::new(x, y, w, h))
    }

    /// Top-left corner, which both forms carry.
    pub const fn origin(&self) -> (i32, i32) {
        match self {
            Self::Stretch(rect) => (rect.x, rect.y),
            Self::Native { x, y } => (*x, *y),
        }
    }
}

/// Which hero animation a [`Sprite::Hero`] means. Ports the three `SpriteAnim`
/// fields of C's `Sprites`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HeroAnim {
    Walk,
    Idle,
    Boat,
}

/// The six location backdrops, `GR_LOCATION` sub-ids 0..5. Each is 240x102 in
/// the legacy pack and fills the upper half of a location screen.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Backdrop {
    Town,
    Castle,
    Plains,
    Forest,
    HillCave,
    Dungeon,
}

/// One texture slot, naming what to draw without knowing how it is stored.
///
/// This ports C's `Sprites` bundle field for field, including its array
/// indices, which are the same numeric indices the engine's tables use:
/// classes 0..3, villains 0..16, troops 0..24, view icons 0..13.
///
/// Every animated variant takes an ALREADY-FOLDED frame index, not the
/// free-running tick. Folding it (`sprites_frame`), choosing between an
/// authored facing and a mirrored single strip (`sprites_anim_tex`), and
/// falling back from an animation to a still image all need to know what the
/// pack authored. That catalog is deferred until `map_render` exists to
/// define its shape -- the same reasoning that defers `CombatTurnRecord`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Sprite {
    /// Terrain or object art, named by the pack. This is `Tile::art`, which
    /// C looks up through `tile_cache_get`.
    Tile(FixedStr<TILE_ART_LEN>),

    Hero {
        anim: HeroAnim,
        facing: Facing,
        frame: i32,
    },

    ClassPortrait(usize),
    VillainPortrait(usize),
    VillainAnim {
        villain: usize,
        frame: i32,
    },
    /// 0-7 artifacts, 8-11 maps, 12 empty, 13 empty map.
    ViewIcon(usize),
    TroopStill(usize),
    TroopAnim {
        troop: usize,
        frame: i32,
    },

    PuzzleCover,
    Backdrop(Backdrop),
    EndingWin,
    EndingLose,

    HudContractSilhouette,
    HudSiegeSilhouette,
    HudMagicSilhouette,
    HudPuzzleGrid,
    HudGoldPurse,
    HudSiegeAnim {
        frame: i32,
    },
    HudMagicAnim {
        frame: i32,
    },
    /// The middle bar, `screen_w` by 5.
    HudBarStrip,
    /// The outer frame, full screen, with a transparent interior so whatever
    /// was painted underneath still shows.
    ChromeOverworld,

    SplashLogo,
    SplashTitle,
    ClassPicker,
    ClassHighlight,
    Orb,

    EndGrass,
    EndCarpet,
    EndHero,
    EndThrone,

    /// 0 grass field, 1..3 obstacles, 4 castle decoration, 5..10 castle wall
    /// pieces, 11..14 cursors.
    CombatTile(usize),
}

impl Sprite {
    /// Name a tile sprite by its art string, the form `Tile::art` carries.
    pub fn tile(art: &str) -> Self {
        let mut name = FixedStr::empty();
        name.copy_from_str(art);
        Self::Tile(name)
    }
}

/// The rounded-box style. All three of the C shell's rounded-rectangle call
/// sites pass exactly this pair, so it is one visual style rather than a
/// parameter anybody varies -- see [`DrawList::rounded_panel`].
pub const ROUNDNESS: f32 = 0.05;
pub const ROUND_SEGMENTS: i32 = 6;

/// One thing to draw. A backend walks these in order.
///
/// Not `Eq`: the rounded-rectangle commands carry a float, as raylib's do.
#[derive(Clone, Debug, PartialEq)]
pub enum DrawCmd {
    /// Filled rectangle. C's `DrawRectangle`, the most common call by far.
    Rect {
        rect: Rect,
        color: Color,
    },
    /// One-pixel outline drawn INSIDE the rectangle, so a 240-wide box leaves
    /// 238 pixels of interior. C's `DrawRectangleLines`. That pixel per edge
    /// is why the dialog panel is wider than the map it sits over.
    RectLines {
        rect: Rect,
        color: Color,
    },
    RectRounded {
        rect: Rect,
        roundness: f32,
        segments: i32,
        color: Color,
    },
    RectRoundedLines {
        rect: Rect,
        roundness: f32,
        segments: i32,
        color: Color,
    },
    /// Draw a whole texture. `tint` multiplies it; pass [`Color::NO_TINT`] to
    /// leave it alone.
    Blit {
        sprite: Sprite,
        dst: Dst,
        flip_x: bool,
        tint: Color,
    },
    /// Bitmap-font text, kept whole rather than expanded into one blit per
    /// glyph: a test then asserts on the string. `\n` starts a new line at
    /// `x`. See [`crate::font`] for the metrics, which are fixed and need no
    /// asset.
    Text {
        text: String,
        x: i32,
        y: i32,
        color: Color,
    },
    /// Clip everything after this to `rect`, until [`DrawCmd::ClipEnd`].
    /// C's `BeginScissorMode`, which does not nest and is used exactly once,
    /// for the map viewport.
    ClipBegin(Rect),
    ClipEnd,
}

/// An ordered recording of draw commands.
///
/// This is both what view code writes into and the headless backend: a test
/// builds a view, then asserts on the list. There is nothing to execute it
/// until a real backend exists.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DrawList {
    cmds: Vec<DrawCmd>,
}

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, cmd: DrawCmd) {
        self.cmds.push(cmd);
    }

    pub fn as_slice(&self) -> &[DrawCmd] {
        &self.cmds
    }

    pub fn len(&self) -> usize {
        self.cmds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cmds.is_empty()
    }

    pub fn clear(&mut self) {
        self.cmds.clear();
    }

    // Emit helpers. These exist so a view reads like the C it came from
    // rather than like a struct literal.

    pub fn rect(&mut self, rect: Rect, color: Color) {
        self.push(DrawCmd::Rect { rect, color });
    }

    pub fn rect_lines(&mut self, rect: Rect, color: Color) {
        self.push(DrawCmd::RectLines { rect, color });
    }

    pub fn rect_rounded(&mut self, rect: Rect, roundness: f32, segments: i32, color: Color) {
        self.push(DrawCmd::RectRounded {
            rect,
            roundness,
            segments,
            color,
        });
    }

    pub fn rect_rounded_lines(&mut self, rect: Rect, roundness: f32, segments: i32, color: Color) {
        self.push(DrawCmd::RectRoundedLines {
            rect,
            roundness,
            segments,
            color,
        });
    }

    /// The rounded box the dialog panel and the two view popups all draw: a
    /// fill with an outline over it, in the one style C ever uses.
    pub fn rounded_panel(&mut self, rect: Rect, fill: Color, border: Color) {
        self.rect_rounded(rect, ROUNDNESS, ROUND_SEGMENTS, fill);
        self.rect_rounded_lines(rect, ROUNDNESS, ROUND_SEGMENTS, border);
    }

    /// The common blit: whole texture, no mirror, no tint.
    pub fn blit(&mut self, sprite: Sprite, dst: Dst) {
        self.push(DrawCmd::Blit {
            sprite,
            dst,
            flip_x: false,
            tint: Color::NO_TINT,
        });
    }

    pub fn blit_flipped(&mut self, sprite: Sprite, dst: Dst, flip_x: bool) {
        self.push(DrawCmd::Blit {
            sprite,
            dst,
            flip_x,
            tint: Color::NO_TINT,
        });
    }

    pub fn blit_tinted(&mut self, sprite: Sprite, dst: Dst, flip_x: bool, tint: Color) {
        self.push(DrawCmd::Blit {
            sprite,
            dst,
            flip_x,
            tint,
        });
    }

    pub fn text(&mut self, text: &str, x: i32, y: i32, color: Color) {
        self.push(DrawCmd::Text {
            text: text.to_owned(),
            x,
            y,
            color,
        });
    }

    /// Text centred on `cx`, porting `bfont_draw_centered`.
    pub fn text_centered(&mut self, text: &str, cx: i32, y: i32, color: Color) {
        self.text(text, crate::font::centered_x(text, cx), y, color);
    }

    pub fn clip_begin(&mut self, rect: Rect) {
        self.push(DrawCmd::ClipBegin(rect));
    }

    pub fn clip_end(&mut self) {
        self.push(DrawCmd::ClipEnd);
    }

    // Queries, for tests and for debugging a view.

    /// Every text command, in order, as `(x, y, text)`.
    pub fn texts(&self) -> Vec<(i32, i32, &str)> {
        self.cmds
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Text { text, x, y, .. } => Some((*x, *y, text.as_str())),
                _ => None,
            })
            .collect()
    }

    /// True when some text command contains `needle`.
    pub fn has_text_containing(&self, needle: &str) -> bool {
        self.cmds.iter().any(|cmd| match cmd {
            DrawCmd::Text { text, .. } => text.contains(needle),
            _ => false,
        })
    }

    /// Where `sprite` was drawn, in order. Empty when it was never drawn.
    pub fn blits_of(&self, sprite: Sprite) -> Vec<Dst> {
        self.cmds
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::Blit { sprite: s, dst, .. } if *s == sprite => Some(*dst),
                _ => None,
            })
            .collect()
    }

    /// How deep the clip nesting is at the end of the list. Zero means every
    /// [`DrawCmd::ClipBegin`] was closed; negative means one closed too many.
    pub fn clip_balance(&self) -> i32 {
        self.cmds.iter().fold(0, |depth, cmd| match cmd {
            DrawCmd::ClipBegin(_) => depth + 1,
            DrawCmd::ClipEnd => depth - 1,
            _ => depth,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_packed_pack_colour_unpacks_the_way_chrome_reads_it() {
        // The canonical Normal-difficulty bar, 0xFFAA0000.
        assert_eq!(Color::from_argb(0xFFAA_0000), Color::rgb(0xAA, 0, 0));
        // No alpha byte means fully transparent, not opaque. C does the same,
        // so a pack that writes 0xAA0000 gets an invisible status bar.
        assert_eq!(Color::from_argb(0x00AA_0000).a, 0);
    }

    #[test]
    fn the_list_keeps_what_it_was_given_in_order() {
        let mut list = DrawList::new();
        list.rect(Rect::new(0, 0, 320, 200), Color::BLACK);
        list.blit(Sprite::HudBarStrip, Dst::rect(0, 17, 320, 5));
        list.text("Days Left:400", 17, 9, Color::WHITE);

        assert_eq!(list.len(), 3);
        assert!(matches!(list.as_slice()[0], DrawCmd::Rect { .. }));
        assert!(matches!(list.as_slice()[1], DrawCmd::Blit { .. }));
        assert_eq!(list.texts(), vec![(17, 9, "Days Left:400")]);
        assert!(list.has_text_containing("400"));
        assert!(!list.has_text_containing("Time Stop"));
    }

    #[test]
    fn a_blit_can_be_found_by_the_sprite_it_names() {
        let mut list = DrawList::new();
        list.blit(Sprite::tile("grass"), Dst::rect(16, 22, 48, 34));
        list.blit(Sprite::tile("water"), Dst::rect(64, 22, 48, 34));
        list.blit(Sprite::tile("grass"), Dst::rect(112, 22, 48, 34));

        assert_eq!(
            list.blits_of(Sprite::tile("grass")),
            vec![Dst::rect(16, 22, 48, 34), Dst::rect(112, 22, 48, 34)]
        );
        assert!(list.blits_of(Sprite::tile("mountain")).is_empty());
    }

    #[test]
    fn a_native_destination_carries_a_corner_and_no_size() {
        // combat_render sizes the destination from the texture's own pixels,
        // which a headless view cannot know. It says where, not how big.
        let dst = Dst::at(40, 60);
        assert_eq!(dst.origin(), (40, 60));
        assert_eq!(Dst::rect(40, 60, 48, 34).origin(), (40, 60));
        assert_ne!(dst, Dst::rect(40, 60, 48, 34));
    }

    #[test]
    fn a_rounded_panel_is_a_fill_with_its_outline_over_it() {
        let mut list = DrawList::new();
        list.rounded_panel(
            Rect::new(11, 124, 245, 68),
            Color::rgb(0, 0, 170),
            Color::rgb(255, 255, 85),
        );

        assert_eq!(list.len(), 2);
        match &list.as_slice()[0] {
            DrawCmd::RectRounded {
                roundness,
                segments,
                ..
            } => {
                assert_eq!((*roundness, *segments), (ROUNDNESS, ROUND_SEGMENTS));
            }
            other => panic!("expected a filled rounded rect, got {other:?}"),
        }
        assert!(matches!(
            list.as_slice()[1],
            DrawCmd::RectRoundedLines { .. }
        ));
    }

    #[test]
    fn clipping_is_balanced_only_when_every_region_is_closed() {
        let mut list = DrawList::new();
        assert_eq!(list.clip_balance(), 0);

        list.clip_begin(Rect::new(16, 22, 240, 170));
        list.rect(Rect::new(16, 22, 240, 170), Color::BLACK);
        assert_eq!(list.clip_balance(), 1);

        list.clip_end();
        assert_eq!(list.clip_balance(), 0);
    }

    #[test]
    fn centred_text_is_placed_by_the_font_metrics() {
        let mut list = DrawList::new();
        // 19 glyphs at 8px is 152 wide, so it starts 76 left of centre.
        list.text_centered("Press 'ESC' to exit", 160, 9, Color::WHITE);
        assert_eq!(list.texts(), vec![(84, 9, "Press 'ESC' to exit")]);
    }
}
