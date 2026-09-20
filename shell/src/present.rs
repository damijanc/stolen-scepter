//! Turning the logical design space into window pixels, porting
//! `src/present.c`.
//!
//! The target is blitted at an INTEGER scale and centred, with black around
//! it, so pixels stay square. All of that is arithmetic, so it lives here and
//! is tested without a window; a backend only has to clear the frame and draw
//! one rectangle where [`Present::blit_rect`] says.
//!
//! The chosen scale is a property of the machine looking at the game, not of
//! the pack and not of the save. C keeps it in a static that nothing
//! serialises, and this keeps it in a value the shell owns, for the same
//! reason: `stats.options[]` goes into saves, and a display scale must not.

use crate::draw::Rect;
use crate::layout::{Layout, SCALE_MAX, SCALE_MAX_WEB, SCALE_MIN};

/// Which ceiling applies. A browser tab is one window among many, so it gets
/// the tighter one.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Target {
    #[default]
    Desktop,
    Web,
}

impl Target {
    pub fn scale_ceiling(self) -> i32 {
        match self {
            Self::Desktop => SCALE_MAX,
            Self::Web => SCALE_MAX_WEB,
        }
    }
}

/// The display-scale preference and the arithmetic that applies it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Present {
    /// 0 means auto-fit, which is the startup state.
    scale_override: i32,
}

impl Present {
    pub fn new() -> Self {
        Self::default()
    }

    /// A modern pack opens at 1:1 rather than auto, so the Scale row reads
    /// "1x" and the player scales UP from the pack's native size. Legacy never
    /// touches the override at all.
    pub fn for_layout(layout: &Layout) -> Self {
        let mut present = Self::new();
        if layout.is_modern {
            present.set_scale(1);
        }
        present
    }

    /// Choose a scale, or restore auto-fit with anything at or below zero.
    ///
    /// C also grows the window to match here. That is the backend's job;
    /// [`Present::requested_window_size`] reports the size to apply. Growing
    /// it is the point of the control: clamping the choice to whatever the
    /// window already fits made every option collapse to the current scale, so
    /// the setting looked like it did nothing.
    pub fn set_scale(&mut self, scale: i32) {
        self.scale_override = if scale > 0 { scale } else { 0 };
    }

    /// 0 when auto-fitting.
    pub fn scale_override(&self) -> i32 {
        self.scale_override
    }

    /// Cycle Auto -> 1x -> ... -> 5x -> Auto, porting
    /// `views_controls_advance_scale`.
    ///
    /// The cycle always runs to [`SCALE_MAX`], even on web where the ceiling
    /// is lower: the extra choices are offered and then clamped when the scale
    /// is applied, rather than being hidden from the menu.
    pub fn advance_scale(&mut self) {
        self.set_scale((self.scale_override + 1) % (SCALE_MAX + 1));
    }

    /// Window size this override asks for, or `None` while auto-fitting.
    pub fn requested_window_size(&self, layout: &Layout) -> Option<(i32, i32)> {
        if self.scale_override <= 0 {
            return None;
        }
        Some((
            layout.screen_w * self.scale_override,
            layout.screen_h * self.scale_override,
        ))
    }

    /// The integer scale for a window this size, porting `present_scale`.
    ///
    /// Legacy and modern do NOT share a path. Legacy is left exactly as it was
    /// before render modes existed: auto-fit with a floor of [`SCALE_MIN`],
    /// with no override consulted at all, because the scale control is a
    /// modern-only feature and must not change legacy behaviour.
    pub fn scale(&self, layout: &Layout, win_w: i32, win_h: i32, target: Target) -> i32 {
        let fit_x = win_w / layout.screen_w;
        let fit_y = win_h / layout.screen_h;
        let fit = fit_x.min(fit_y);
        // A pack shipping art at `pixel_scale` times the logical size must not
        // be presented below that, or the art is downsampled and aliases. The
        // ceiling rides along so a large window can still scale an HD pack up.
        let pixel = layout.pixel_scale.max(1);
        let ceiling = target.scale_ceiling() * pixel;

        if !layout.is_modern {
            // Floor then ceiling, in that order and separately, as C does.
            // A single clamp would read the same today but panics the moment
            // a ceiling is ever set below the floor, where C just returns the
            // ceiling.
            //
            // An HD pack snaps to a whole multiple of its art scale. Without
            // that, a window with room for 5x presents 4x art at 5/4, so some
            // source pixels land on one screen pixel and some on two; the
            // multiple keeps every source pixel square. A plain pack (pixel 1)
            // skips this and is byte-for-byte what it was before.
            let mut scale = if pixel > 1 {
                pixel * (fit / pixel).max(1)
            } else {
                fit
            };
            if scale < SCALE_MIN {
                scale = SCALE_MIN;
            }
            if scale < pixel {
                scale = pixel;
            }
            if scale > ceiling {
                scale = ceiling;
            }
            return scale;
        }

        let scale = if self.scale_override > 0 {
            // An explicit choice is honoured as given. The clamp below is a
            // default-sizing rule, not a veto on what the player asked for,
            // and `set_scale` has already grown the window to match, so the
            // fit clamp normally agrees anyway.
            self.scale_override.min(fit.max(1))
        } else {
            // Auto-fit: floor first, then ceiling. The floor belongs to the
            // geometry -- 320x200 needs 2x to be usable, while a modern pack
            // is already large and sits at 1:1. A pack that declares an art
            // pixel multiple floors there instead.
            fit.max(layout.default_scale.max(pixel))
        };
        scale.min(ceiling)
    }

    /// Where the design-space target lands in the window: integer-scaled and
    /// centred, porting `present_scaled`'s destination rectangle. Whatever is
    /// left over around it is the black letterbox.
    ///
    /// The rectangle can be WIDER than the window. Legacy floors the scale at
    /// 2x, so a window narrower than 640 gets a target that overhangs it, and
    /// the origin comes out negative -- the view stays centred and the edges
    /// are cropped rather than the game being squeezed.
    pub fn blit_rect(&self, layout: &Layout, win_w: i32, win_h: i32, target: Target) -> Rect {
        let scale = self.scale(layout, win_w, win_h, target);
        let w = layout.screen_w * scale;
        let h = layout.screen_h * scale;
        Rect::new((win_w - w) / 2, (win_h - h) / 2, w, h)
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::resources::Resources;

    fn layout_for(render: &str) -> Layout {
        let resources =
            Resources::from_json_str(&format!(r#"{{"render": {render}, "troops": []}}"#))
                .expect("fixture parses");
        Layout::from_resources(&resources).expect("lays out")
    }

    /// 320x200, the DOS geometry.
    fn legacy_layout() -> Layout {
        layout_for(r#"{"mode": "legacy"}"#)
    }

    /// 800x702: seven 96px tiles each way, plus the chrome bands.
    fn modern_layout() -> Layout {
        layout_for(r#"{"mode": "modern"}"#)
    }

    #[test]
    fn legacy_auto_fits_between_its_floor_and_ceiling() {
        let layout = legacy_layout();
        let present = Present::new();

        // 320x200 at 3x is 960x600, and the window has room for exactly that.
        assert_eq!(present.scale(&layout, 960, 600, Target::Desktop), 3);
        // The smaller axis decides.
        assert_eq!(present.scale(&layout, 1600, 600, Target::Desktop), 3);
        // Too small for the floor still gets the floor: 320x200 is unusable
        // at 1x, so the view overhangs the window instead.
        assert_eq!(present.scale(&layout, 400, 300, Target::Desktop), SCALE_MIN);
        // A 4K display would otherwise reach 10x, which blows an 8x8 glyph up
        // to 80 pixels.
        assert_eq!(
            present.scale(&layout, 3840, 2160, Target::Desktop),
            SCALE_MAX
        );
        assert_eq!(
            present.scale(&layout, 3840, 2160, Target::Web),
            SCALE_MAX_WEB
        );
    }

    #[test]
    fn legacy_ignores_the_scale_override_entirely() {
        // The scale control is a modern-only feature. C returns from the
        // legacy branch before it ever looks at the override, so a legacy pack
        // behaves exactly as it did before render modes existed.
        let layout = legacy_layout();
        let mut present = Present::new();
        present.set_scale(5);

        assert_eq!(present.scale(&layout, 960, 600, Target::Desktop), 3);
        assert_eq!(present.scale_override(), 5, "the choice is still recorded");
    }

    #[test]
    fn an_hd_pack_is_never_presented_below_its_art_scale() {
        // A legacy HD pack: DOS geometry, art authored 4x.
        let layout = layout_for(r#"{"mode": "legacy", "scale": 4}"#);
        let present = Present::new();

        // A window with room only for 2x still gets 4x, so the 4x art is not
        // downsampled; it overhangs the window exactly as a plain legacy pack
        // does at its own floor.
        assert_eq!(present.scale(&layout, 640, 400, Target::Desktop), 4);
        // A 1920x1080 window fits 5x, but 4x art snaps down to the nearest
        // whole multiple (4x) rather than showing uneven 5/4 pixels.
        assert_eq!(present.scale(&layout, 1920, 1080, Target::Desktop), 4);
        // A window that fits exactly 8x takes 2x the art scale.
        assert_eq!(present.scale(&layout, 2560, 1600, Target::Desktop), 8);
        // Legacy still ignores an explicit scale choice.
        let mut chosen = Present::new();
        chosen.set_scale(1);
        assert_eq!(chosen.scale(&layout, 640, 400, Target::Desktop), 4);
    }

    #[test]
    fn a_modern_pack_opens_at_one_to_one_and_legacy_opens_on_auto() {
        // 800x702 at 2x exceeds 1080p, so a modern pack starts at its native
        // size and the player scales up from there.
        assert_eq!(Present::for_layout(&modern_layout()).scale_override(), 1);
        assert_eq!(Present::for_layout(&legacy_layout()).scale_override(), 0);
    }

    #[test]
    fn modern_auto_fit_floors_at_the_packs_own_scale() {
        let layout = modern_layout();
        let present = Present::new();

        // 800x702: a window with room for 2x gets 2x.
        assert_eq!(present.scale(&layout, 1600, 1404, Target::Desktop), 2);
        // A window too small for even 1x still gets the pack's default, which
        // for modern is 1 -- it is already large, so it does not need a floor
        // above its native size.
        assert_eq!(
            present.scale(&layout, 400, 300, Target::Desktop),
            layout.default_scale
        );
        assert_eq!(layout.default_scale, 1);
    }

    #[test]
    fn a_modern_override_is_honoured_but_never_beyond_what_fits() {
        let layout = modern_layout();
        let mut present = Present::new();
        present.set_scale(3);

        // Room for 3x, so the choice stands.
        assert_eq!(present.scale(&layout, 2400, 2106, Target::Desktop), 3);
        // Room for only 2x: the choice is clamped down to what the window can
        // actually show.
        assert_eq!(present.scale(&layout, 1600, 1404, Target::Desktop), 2);
        // A window too small for even one whole copy still renders at 1x
        // rather than collapsing to zero.
        assert_eq!(present.scale(&layout, 100, 100, Target::Desktop), 1);
        // The ceiling still applies on top of an explicit choice.
        present.set_scale(5);
        assert_eq!(
            present.scale(&layout, 8000, 8000, Target::Web),
            SCALE_MAX_WEB
        );
    }

    #[test]
    fn the_scale_control_cycles_through_auto() {
        let mut present = Present::new();
        assert_eq!(present.scale_override(), 0);

        let mut seen = vec![];
        for _ in 0..=SCALE_MAX {
            present.advance_scale();
            seen.push(present.scale_override());
        }
        // Auto -> 1x .. 5x -> back to Auto.
        assert_eq!(seen, vec![1, 2, 3, 4, 5, 0]);
    }

    #[test]
    fn setting_a_scale_asks_for_a_window_that_fits_it() {
        let layout = legacy_layout();
        let mut present = Present::new();
        assert_eq!(present.requested_window_size(&layout), None);

        present.set_scale(3);
        assert_eq!(present.requested_window_size(&layout), Some((960, 600)));

        // Anything at or below zero means auto, not a scale of zero.
        present.set_scale(0);
        assert_eq!(present.scale_override(), 0);
        present.set_scale(-2);
        assert_eq!(present.scale_override(), 0);
        assert_eq!(present.requested_window_size(&layout), None);
    }

    #[test]
    fn the_target_is_centred_with_the_leftovers_as_letterbox() {
        let layout = legacy_layout();
        let present = Present::new();

        // 1000x700 fits 3x (960x600), leaving 40 and 100 to split.
        let rect = present.blit_rect(&layout, 1000, 700, Target::Desktop);
        assert_eq!(rect, Rect::new(20, 50, 960, 600));
        assert_eq!(rect.right(), 980);
        assert_eq!(rect.bottom(), 650);
    }

    #[test]
    fn a_window_below_the_legacy_floor_overhangs_rather_than_squeezing() {
        // The floor is 2x, so 320x200 becomes 640x400 in a 500x300 window.
        // The origin goes negative: the view stays centred and the edges are
        // cropped, rather than the pixels being scaled non-uniformly.
        let present = Present::new();
        let rect = present.blit_rect(&legacy_layout(), 500, 300, Target::Desktop);

        assert_eq!(rect, Rect::new(-70, -50, 640, 400));
    }

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
        // 1920x800 fits 4x (1280x800) with 320 to each side: a real horizontal
        // letterbox. (1280x800 is exactly 4x, so it would not.)
        let (win_w, win_h) = (1920, 800);
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
}
