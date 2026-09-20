//! Screen geometry, porting OpenBounty's `src/layout.h` and `src/layout.c`.
//!
//! Every value here is in LOGICAL pixels -- the internal design space the
//! backend later blits to a window at an integer scale. Nothing in this
//! module draws, and nothing in it depends on a graphics library.
//!
//! The geometry is not fixed. A pack declares `render.mode`: `legacy` keeps
//! the 320x200 / 48x34 / 5x5 layout the DOS original used, and `modern`
//! allows square tiles and a larger viewport. [`Layout::from_resources`]
//! derives everything from that declaration, so no view code should ever
//! contain a literal `320`.
//!
//! The chrome bands stay literal, as in C: they are text and border
//! furniture measured in pixels, and do not scale with tile size.

use openbounty_engine::resources::{RenderMode, Resources};

/// Chrome band thicknesses, from the original's `DOS_frame_ui[]` strips.
pub const FRAME_TOP_H: i32 = 8;
pub const FRAME_BOTTOM_H: i32 = 8;
pub const FRAME_LEFT_W: i32 = 16;
pub const FRAME_RIGHT_W: i32 = 16;
pub const BAR_H: i32 = 5;
pub const BAR_Y: i32 = 17;

/// Status strip, between the top frame and the bar. Its height is the
/// original's `font.h + zoom`, which is 8 + 1.
pub const STATUS_X: i32 = FRAME_LEFT_W;
pub const STATUS_Y: i32 = FRAME_TOP_H;
pub const STATUS_H: i32 = 9;

/// Map viewport origin. `MAP_Y` is the top frame plus the status strip plus
/// the bar.
pub const MAP_X: i32 = FRAME_LEFT_W;
pub const MAP_Y: i32 = FRAME_TOP_H + STATUS_H + BAR_H;

/// The dialog/prompt panel: the blue bottom box, anchored to the bottom of
/// the map area. Its width is the map's plus 5px borrowed from the
/// decorative left frame.
///
/// That 5px is load-bearing. `DrawRectangleLines` takes 1px per edge, so the
/// map's own 240px leaves 238px of interior against 30 * 8 = 240px of cells:
/// a full 30-character line does not fit. Widening by only 2px fits the text
/// but jams it against the border. The resulting margin is one-sided by
/// construction, because the right edge stays pinned to the sidebar.
pub const PANEL_H: i32 = 68;
pub const PANEL_PAD_X: i32 = 4;
pub const PANEL_COLS: i32 = 30;

/// Integer presentation scale bounds. The design-space target is blitted at
/// an INTEGER scale so pixels stay square.
///
/// The floor keeps the game legible in a small window. The ceiling exists
/// because the scale otherwise grows to whatever the display allows: a 4K
/// screen produced 10x, which is not blurry -- integer scaling holds -- but
/// blows an 8x8 glyph up to 80px. The browser ceiling is tighter because a
/// tab is one window among many.
pub const SCALE_MIN: i32 = 2;
pub const SCALE_MAX: i32 = 5;
pub const SCALE_MAX_WEB: i32 = 3;

/// Why a pack's render declaration cannot be laid out.
///
/// C reports these from `resources_load` and refuses the pack there. This
/// port records the declaration in `Resources` and rejects it here instead,
/// so the engine stays free of presentation contracts; a real run refuses
/// the same packs, one layer later.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutError {
    /// The pack declared neither `legacy` nor `modern`. There is no default:
    /// legacy art is drawn for a 48x34 tile and modern art for a square one,
    /// so the two are not interchangeable and a guess would be wrong.
    NoRenderMode,
    /// The viewport must be ODD on both axes and at least 3x3. The map
    /// renderer centres the hero with `RADIUS = tiles / 2`, and an even
    /// count leaves him half a tile off.
    ViewportNotOdd { tiles_w: i32, tiles_h: i32 },
    /// Tiles must be at least 8px on both axes.
    TilesTooSmall { tile_w: i32, tile_h: i32 },
    /// The art pixel multiple must be at least 1. A pack that ships art
    /// upscaled `N` times declares `render.scale = N`.
    PixelScaleInvalid { scale: i32 },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoRenderMode => write!(
                f,
                "pack declares no render.mode (expected \"legacy\" or \"modern\")"
            ),
            Self::ViewportNotOdd { tiles_w, tiles_h } => write!(
                f,
                "render viewport must be odd and at least 3x3 (got {tiles_w}x{tiles_h})"
            ),
            Self::TilesTooSmall { tile_w, tile_h } => write!(
                f,
                "render tiles must be at least 8px (got {tile_w}x{tile_h})"
            ),
            Self::PixelScaleInvalid { scale } => {
                write!(f, "render.scale must be at least 1 (got {scale})")
            }
        }
    }
}

impl std::error::Error for LayoutError {}

/// Derived screen geometry. Ports `ClLayout` and the `CL_*` macro family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Layout {
    pub tile_w: i32,
    pub tile_h: i32,
    /// Viewport size in tiles, odd on both axes so the hero centres.
    pub tiles_w: i32,
    pub tiles_h: i32,
    pub map_w: i32,
    pub map_h: i32,
    /// One tile wide, as in the original.
    pub sidebar_w: i32,
    pub screen_w: i32,
    pub screen_h: i32,
    /// Window scale at startup.
    pub default_scale: i32,
    /// True only for a pack that declared `modern`. Legacy must stay
    /// indistinguishable from the build before render modes existed, so
    /// anything new is gated on this.
    pub is_modern: bool,
    /// The pack's art pixel multiple, at least 1. The shell presents at a
    /// window scale no lower than this, so art authored `N` times the logical
    /// size is never sampled below 1:1. `1` is the DOS original and leaves the
    /// old presentation exactly as it was.
    pub pixel_scale: i32,
}

impl Layout {
    /// Derive the geometry from a pack's render declaration. Ports
    /// `layout_init`, plus the validation C performs at resource-load time.
    pub fn from_resources(resources: &Resources) -> Result<Self, LayoutError> {
        let render = &resources.render;
        let mode = match render.mode {
            RenderMode::None => return Err(LayoutError::NoRenderMode),
            mode => mode,
        };

        if render.tiles_w % 2 == 0
            || render.tiles_h % 2 == 0
            || render.tiles_w < 3
            || render.tiles_h < 3
        {
            return Err(LayoutError::ViewportNotOdd {
                tiles_w: render.tiles_w,
                tiles_h: render.tiles_h,
            });
        }
        if render.tile_w < 8 || render.tile_h < 8 {
            return Err(LayoutError::TilesTooSmall {
                tile_w: render.tile_w,
                tile_h: render.tile_h,
            });
        }
        if render.scale < 1 {
            return Err(LayoutError::PixelScaleInvalid {
                scale: render.scale,
            });
        }

        let map_w = render.tile_w * render.tiles_w;
        let map_h = render.tile_h * render.tiles_h;
        let sidebar_w = render.tile_w;
        let is_modern = mode == RenderMode::Modern;

        Ok(Self {
            tile_w: render.tile_w,
            tile_h: render.tile_h,
            tiles_w: render.tiles_w,
            tiles_h: render.tiles_h,
            map_w,
            map_h,
            sidebar_w,
            screen_w: FRAME_LEFT_W + map_w + sidebar_w + FRAME_RIGHT_W,
            screen_h: FRAME_TOP_H + STATUS_H + BAR_H + map_h + FRAME_BOTTOM_H,
            // Legacy opens at 2x because 320x200 is tiny on a modern display;
            // an HD pack opens at its art scale instead, or the initial window
            // is too small to show it. A modern pack is already large -- at 2x
            // it would exceed 1080p -- so it opens 1:1 and the user scales up.
            default_scale: if is_modern { 1 } else { render.scale.max(2) },
            is_modern,
            pixel_scale: render.scale,
        })
    }

    pub fn window_w(&self) -> i32 {
        self.screen_w * self.default_scale
    }

    pub fn window_h(&self) -> i32 {
        self.screen_h * self.default_scale
    }

    pub fn status_w(&self) -> i32 {
        self.screen_w - FRAME_LEFT_W - FRAME_RIGHT_W
    }

    pub fn sidebar_x(&self) -> i32 {
        MAP_X + self.map_w
    }

    pub fn sidebar_y(&self) -> i32 {
        MAP_Y
    }

    pub fn sidebar_h(&self) -> i32 {
        self.map_h
    }

    /// The decorative 8px strip at the very bottom of the screen. Distinct
    /// from the dialog panel above it.
    pub fn bottom_y(&self) -> i32 {
        MAP_Y + self.map_h
    }

    pub fn panel_x(&self) -> i32 {
        MAP_X - 5
    }

    pub fn panel_w(&self) -> i32 {
        self.map_w + 5
    }

    pub fn panel_y(&self) -> i32 {
        MAP_Y + self.map_h - PANEL_H
    }

    /// How far the hero's tile sits from the viewport edge, in tiles. The
    /// odd-axis rule is what makes this exact.
    pub fn viewport_radius_x(&self) -> i32 {
        self.tiles_w / 2
    }

    pub fn viewport_radius_y(&self) -> i32 {
        self.tiles_h / 2
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Layout, LayoutError, BAR_H, FRAME_BOTTOM_H, FRAME_LEFT_W, FRAME_RIGHT_W, FRAME_TOP_H,
        MAP_X, MAP_Y, PANEL_COLS, PANEL_H, STATUS_H,
    };
    use openbounty_engine::resources::Resources;

    fn pack(render: &str) -> Resources {
        Resources::from_json_str(&format!(r#"{{"render": {render}, "troops": []}}"#))
            .expect("fixture parses")
    }

    #[test]
    fn legacy_derives_the_dos_three_twenty_by_two_hundred_screen() {
        let layout = Layout::from_resources(&pack(r#"{"mode": "legacy"}"#)).expect("lays out");

        assert_eq!((layout.tile_w, layout.tile_h), (48, 34));
        assert_eq!((layout.tiles_w, layout.tiles_h), (5, 5));
        assert_eq!((layout.map_w, layout.map_h), (240, 170));
        assert_eq!(layout.sidebar_w, 48);
        // 16 + 240 + 48 + 16 and 8 + 9 + 5 + 170 + 8.
        assert_eq!((layout.screen_w, layout.screen_h), (320, 200));
        assert!(!layout.is_modern);
        assert_eq!(layout.default_scale, 2);
        assert_eq!((layout.window_w(), layout.window_h()), (640, 400));
    }

    #[test]
    fn the_legacy_bands_add_up_to_the_screen_on_both_axes() {
        let layout = Layout::from_resources(&pack(r#"{"mode": "legacy"}"#)).expect("lays out");

        assert_eq!(
            FRAME_LEFT_W + layout.map_w + layout.sidebar_w + FRAME_RIGHT_W,
            layout.screen_w
        );
        assert_eq!(
            FRAME_TOP_H + STATUS_H + BAR_H + layout.map_h + FRAME_BOTTOM_H,
            layout.screen_h
        );
        // The known absolute positions from the original.
        assert_eq!(MAP_Y, 22);
        assert_eq!(layout.sidebar_x(), 256);
        assert_eq!(layout.bottom_y(), 192);
        assert_eq!(layout.status_w(), 288);
    }

    #[test]
    fn the_legacy_panel_is_five_pixels_wider_than_the_map() {
        let layout = Layout::from_resources(&pack(r#"{"mode": "legacy"}"#)).expect("lays out");

        assert_eq!(layout.panel_x(), MAP_X - 5);
        assert_eq!(layout.panel_w(), 245);
        // Its right edge stays pinned to the sidebar.
        assert_eq!(layout.panel_x() + layout.panel_w(), layout.sidebar_x());
        // A full 30-character line fits inside the 1px borders, which is the
        // whole reason for the 5px: the map's own 240 would leave 238.
        assert!(layout.panel_w() - 2 >= PANEL_COLS * 8);
        assert_eq!(layout.panel_y() + PANEL_H, MAP_Y + layout.map_h);
    }

    #[test]
    fn the_art_pixel_scale_defaults_to_one_and_legacy_keeps_its_geometry() {
        let layout = Layout::from_resources(&pack(r#"{"mode": "legacy"}"#)).expect("lays out");
        assert_eq!(layout.pixel_scale, 1);

        // An HD pack keeps the DOS logical geometry and only moves the
        // presentation floor, so nothing else about the layout changes.
        let hd =
            Layout::from_resources(&pack(r#"{"mode": "legacy", "scale": 4}"#)).expect("lays out");
        assert_eq!(hd.pixel_scale, 4);
        assert_eq!((hd.screen_w, hd.screen_h), (320, 200));
        assert_eq!((hd.tile_w, hd.tile_h), (48, 34));
        assert_eq!((hd.tiles_w, hd.tiles_h), (5, 5));
        // It opens at the art scale, so the window is big enough to show it.
        assert_eq!(hd.default_scale, 4);
        assert_eq!((hd.window_w(), hd.window_h()), (1280, 800));
    }

    #[test]
    fn a_pixel_scale_below_one_is_refused() {
        assert_eq!(
            Layout::from_resources(&pack(r#"{"mode": "legacy", "scale": 0}"#)),
            Err(LayoutError::PixelScaleInvalid { scale: 0 })
        );
    }

    #[test]
    fn modern_derives_a_larger_screen_from_the_pack() {
        let layout = Layout::from_resources(&pack(
            r#"{"mode": "modern", "tile_w": 96, "tile_h": 96, "tiles_w": 7, "tiles_h": 7}"#,
        ))
        .expect("lays out");

        assert_eq!((layout.map_w, layout.map_h), (672, 672));
        // 16 + 672 + 96 + 16 and 8 + 9 + 5 + 672 + 8.
        assert_eq!((layout.screen_w, layout.screen_h), (800, 702));
        assert!(layout.is_modern);
        // Already large, so it opens 1:1 rather than doubling past 1080p.
        assert_eq!(layout.default_scale, 1);
        assert_eq!((layout.window_w(), layout.window_h()), (800, 702));
    }

    #[test]
    fn the_viewport_radius_centres_the_hero() {
        let legacy = Layout::from_resources(&pack(r#"{"mode": "legacy"}"#)).expect("lays out");
        assert_eq!(
            (legacy.viewport_radius_x(), legacy.viewport_radius_y()),
            (2, 2)
        );
        // Radius on each side plus the hero's own tile is the whole viewport,
        // which only holds because the axis count is odd.
        assert_eq!(legacy.viewport_radius_x() * 2 + 1, legacy.tiles_w);

        let modern =
            Layout::from_resources(&pack(r#"{"mode": "modern", "tiles_w": 9, "tiles_h": 11}"#))
                .expect("lays out");
        assert_eq!(
            (modern.viewport_radius_x(), modern.viewport_radius_y()),
            (4, 5)
        );
        assert_eq!(modern.viewport_radius_y() * 2 + 1, modern.tiles_h);
    }

    #[test]
    fn a_pack_declaring_no_render_mode_is_refused_here() {
        // The engine loads such a pack; laying it out is what fails, which
        // is where C's layout_init consumes the declaration.
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("engine loads it");
        assert_eq!(
            Layout::from_resources(&resources),
            Err(LayoutError::NoRenderMode)
        );
        assert_eq!(
            Layout::from_resources(&pack(r#"{"mode": "vga"}"#)),
            Err(LayoutError::NoRenderMode)
        );
    }

    #[test]
    fn an_even_or_tiny_viewport_is_refused() {
        assert_eq!(
            Layout::from_resources(&pack(r#"{"mode": "modern", "tiles_w": 4}"#)),
            Err(LayoutError::ViewportNotOdd {
                tiles_w: 4,
                tiles_h: 7
            })
        );
        assert_eq!(
            Layout::from_resources(&pack(r#"{"mode": "modern", "tiles_h": 1}"#)),
            Err(LayoutError::ViewportNotOdd {
                tiles_w: 7,
                tiles_h: 1
            })
        );
        assert_eq!(
            Layout::from_resources(&pack(r#"{"mode": "modern", "tile_w": 4}"#)),
            Err(LayoutError::TilesTooSmall {
                tile_w: 4,
                tile_h: 96
            })
        );
    }

    #[test]
    fn the_real_pack_lays_out_as_legacy() {
        // Skipped unless the ignored reference clone is present: the pack
        // lives there and CI has no copy.
        let path = "../reference/assets/kings-bounty/game.json";
        let Ok(resources) = Resources::from_json_file(path) else {
            eprintln!("SKIPPED: no reference clone at {path}");
            return;
        };
        let layout = Layout::from_resources(&resources).expect("the shipped pack lays out");
        assert_eq!((layout.screen_w, layout.screen_h), (320, 200));
        assert!(!layout.is_modern);
    }

    #[test]
    fn the_hd_pack_lays_out_at_its_art_scale() {
        // Built by `tools/make_hd_pack.py`; skipped when it has not been run.
        let path = "../game-assets/hd-pack/game.json";
        let Ok(resources) = Resources::from_json_file(path) else {
            eprintln!("SKIPPED: no HD pack at {path}");
            return;
        };
        let layout = Layout::from_resources(&resources).expect("the HD pack lays out");
        // The logical geometry is the DOS original; only the art scale moves.
        assert_eq!((layout.screen_w, layout.screen_h), (320, 200));
        assert_eq!((layout.tile_w, layout.tile_h), (48, 34));
        assert_eq!(layout.pixel_scale, 4);
    }
}
