//! The macroquad backend: the only module that knows a graphics library
//! exists.
//!
//! It does two things. It loads every file the pack names, once, and it walks
//! a [`DrawList`] turning logical commands into draw calls. Nothing decides
//! WHAT to draw here -- that is view code's job, and stays testable because
//! this seam exists.
//!
//! DIVERGENCE from C, deliberate: C renders into a 320x200 render target and
//! blits that to the window at an integer scale. This applies the scale to
//! each command instead, with no intermediate target. The result is identical
//! -- an integer scale with nearest-neighbour sampling doubles pixels the same
//! way either side of the target -- and it avoids macroquad's y-flipped render
//! textures entirely. It is also strictly sharper when a sprite's own pixel
//! size differs from the layout cell it fills, because there is no
//! intermediate resample.

use std::collections::HashMap;

use macroquad::prelude::{
    clear_background, draw_circle, draw_rectangle, draw_rectangle_lines, draw_texture_ex,
    get_internal_gl, get_keys_pressed, is_key_down, is_key_pressed, load_file, load_texture,
    screen_height, screen_width, Color as MqColor, DrawTextureParams, FilterMode, KeyCode,
    Rect as MqRect, Texture2D, Vec2,
};
use openbounty_engine::resources::Resources;

use crate::draw::{Color, DrawCmd, DrawList, Rect, Sprite};
use crate::font;
use crate::input::{Key, KeySource};
use crate::layout::Layout;
use crate::palette::Palette;
use crate::present::{Present, Target};
use crate::sprites::{tile_art_names, tile_art_path, SpriteCatalog};

fn mq_color(color: Color) -> MqColor {
    MqColor::from_rgba(color.r, color.g, color.b, color.a)
}

/// Everything loaded from the pack, keyed by the pack-relative path that
/// named it.
pub struct Backend {
    textures: HashMap<String, Texture2D>,
    font: Option<Texture2D>,
    palette: Palette,
    /// Paths the pack named that would not load. Not fatal: C installs a zero
    /// texture and every draw site skips it, so a pack missing one file still
    /// runs with a hole rather than refusing to start.
    missing: Vec<String>,
}

impl Backend {
    /// Load the font, the palette, and every art file the pack names.
    pub async fn load(resources: &Resources, catalog: &SpriteCatalog) -> Self {
        let mut backend = Self {
            textures: HashMap::new(),
            font: None,
            palette: Palette::fallback(),
            missing: Vec::new(),
        };

        for relative in catalog.all_paths() {
            backend.load_texture_at(resources, relative).await;
        }
        // Tile art is named by the map rather than the manifest, and loading
        // it lazily would mean an await in the middle of a frame. The set is
        // small and knowable up front, so it is loaded with everything else.
        for art in tile_art_names(resources) {
            backend
                .load_texture_at(resources, &tile_art_path(&art))
                .await;
        }

        backend.font = backend.fetch(resources, catalog.font_path()).await;
        backend.palette = match resources.resolve_path(catalog.palette_path()) {
            Some(path) => {
                let bytes = load_file(&path.to_string_lossy()).await.ok();
                Palette::from_bin_or_fallback(bytes.as_deref())
            }
            None => Palette::fallback(),
        };

        backend
    }

    async fn load_texture_at(&mut self, resources: &Resources, relative: &str) {
        if self.textures.contains_key(relative) {
            return;
        }
        if let Some(texture) = self.fetch(resources, relative).await {
            self.textures.insert(relative.to_owned(), texture);
        }
    }

    async fn fetch(&mut self, resources: &Resources, relative: &str) -> Option<Texture2D> {
        let path = resources.resolve_path(relative)?;
        match load_texture(&path.to_string_lossy()).await {
            Ok(texture) => {
                // Every asset is pixel art. Anything but nearest-neighbour
                // turns an 8x8 glyph into a smear at 3x.
                texture.set_filter(FilterMode::Nearest);
                Some(texture)
            }
            Err(_) => {
                self.missing.push(relative.to_owned());
                None
            }
        }
    }

    /// The pack's palette, or the canonical sixteen when it shipped none.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Paths the pack named that did not load.
    pub fn missing(&self) -> &[String] {
        &self.missing
    }

    /// How many textures are resident.
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    fn texture(&self, catalog: &SpriteCatalog, sprite: &Sprite) -> Option<&Texture2D> {
        match sprite {
            Sprite::Tile(art) => self.textures.get(&tile_art_path(art.as_str())),
            other => self.textures.get(catalog.path(*other)?),
        }
    }

    /// Draw one frame's worth of commands, scaled and centred in the window.
    pub fn execute(
        &self,
        list: &DrawList,
        catalog: &SpriteCatalog,
        layout: &Layout,
        present: &Present,
    ) {
        clear_background(mq_color(Color::BLACK));

        let win_w = screen_width() as i32;
        let win_h = screen_height() as i32;
        let scale = present.scale(layout, win_w, win_h, Target::Desktop);
        let viewport = present.blit_rect(layout, win_w, win_h, Target::Desktop);
        let frame = Frame {
            origin_x: viewport.x as f32,
            origin_y: viewport.y as f32,
            scale: scale as f32,
        };

        for cmd in list.as_slice() {
            self.run(cmd, catalog, &frame);
        }
        // A list that opened a clip and never closed it would leak the
        // scissor into the next frame, so it is always cleared at the end.
        set_clip(None);
    }

    fn run(&self, cmd: &DrawCmd, catalog: &SpriteCatalog, frame: &Frame) {
        match cmd {
            DrawCmd::Rect { rect, color } => {
                let (x, y, w, h) = frame.rect(*rect);
                draw_rectangle(x, y, w, h, mq_color(*color));
            }
            DrawCmd::RectLines { rect, color } => {
                let (x, y, w, h) = frame.rect(*rect);
                // macroquad draws a band of thickness/2 INSIDE the rectangle,
                // so a one-logical-pixel border needs twice the scale. raylib
                // insets by the full thickness, and that inset is load-bearing
                // -- it is why the dialog panel is wider than the map.
                draw_rectangle_lines(x, y, w, h, frame.scale * 2.0, mq_color(*color));
            }
            DrawCmd::RectRounded {
                rect,
                roundness,
                segments,
                color,
            } => {
                frame.rounded(*rect, *roundness, *segments, mq_color(*color), true);
            }
            DrawCmd::RectRoundedLines {
                rect,
                roundness,
                segments,
                color,
            } => {
                frame.rounded(*rect, *roundness, *segments, mq_color(*color), false);
            }
            DrawCmd::Blit {
                sprite,
                dst,
                flip_x,
                tint,
            } => {
                let Some(texture) = self.texture(catalog, sprite) else {
                    // The pack names no file, or the file would not load.
                    // Skipping matches C, whose zero texture draws nothing.
                    return;
                };
                let (ox, oy) = dst.origin();
                let dest_size = match dst {
                    crate::draw::Dst::Stretch(rect) => {
                        Vec2::new(rect.w as f32 * frame.scale, rect.h as f32 * frame.scale)
                    }
                    // The one place C sizes from the texture's own pixels.
                    crate::draw::Dst::Native { .. } => texture.size() * frame.scale,
                };
                draw_texture_ex(
                    texture,
                    frame.x(ox),
                    frame.y(oy),
                    mq_color(*tint),
                    DrawTextureParams {
                        dest_size: Some(dest_size),
                        flip_x: *flip_x,
                        ..Default::default()
                    },
                );
            }
            DrawCmd::Text { text, x, y, color } => self.text(text, *x, *y, *color, frame),
            DrawCmd::ClipBegin(rect) => {
                let (cx, cy, cw, ch) = frame.rect(*rect);
                set_clip(Some((cx as i32, cy as i32, cw as i32, ch as i32)));
            }
            DrawCmd::ClipEnd => set_clip(None),
        }
    }

    /// One glyph per byte from the 1024x8 strip, advancing a fixed cell.
    fn text(&self, text: &str, x: i32, y: i32, color: Color, frame: &Frame) {
        let Some(strip) = self.font.as_ref() else {
            return;
        };
        let tint = mq_color(color);
        let cell = Vec2::new(
            font::GLYPH_W as f32 * frame.scale,
            font::GLYPH_H as f32 * frame.scale,
        );
        let mut cx = x;
        let mut cy = y;
        for byte in text.as_bytes() {
            if *byte == b'\n' {
                cx = x;
                cy += font::line_height();
                continue;
            }
            let glyph = font::glyph_index(*byte) as f32;
            draw_texture_ex(
                strip,
                frame.x(cx),
                frame.y(cy),
                tint,
                DrawTextureParams {
                    dest_size: Some(cell),
                    source: Some(MqRect::new(
                        glyph * font::GLYPH_W as f32,
                        0.0,
                        font::GLYPH_W as f32,
                        font::GLYPH_H as f32,
                    )),
                    ..Default::default()
                },
            );
            cx += font::GLYPH_W;
        }
    }
}

/// Logical-to-window transform for one frame.
struct Frame {
    origin_x: f32,
    origin_y: f32,
    scale: f32,
}

impl Frame {
    fn x(&self, x: i32) -> f32 {
        self.origin_x + x as f32 * self.scale
    }

    fn y(&self, y: i32) -> f32 {
        self.origin_y + y as f32 * self.scale
    }

    fn rect(&self, rect: Rect) -> (f32, f32, f32, f32) {
        (
            self.x(rect.x),
            self.y(rect.y),
            rect.w as f32 * self.scale,
            rect.h as f32 * self.scale,
        )
    }

    /// A rounded rectangle, which macroquad has no primitive for.
    ///
    /// Built the way raylib builds one: `roundness` is a fraction of the
    /// shorter side, capped at half of it, and the corners are quarter
    /// circles. `segments` is raylib's smoothness knob; here the circles are
    /// already smooth, so it only distinguishes a filled corner from an
    /// outlined one.
    fn rounded(&self, rect: Rect, roundness: f32, _segments: i32, color: MqColor, filled: bool) {
        let (x, y, w, h) = self.rect(rect);
        let radius = (roundness.clamp(0.0, 0.5) * w.min(h)).max(0.0);
        if radius <= 0.5 {
            if filled {
                draw_rectangle(x, y, w, h, color);
            } else {
                draw_rectangle_lines(x, y, w, h, self.scale * 2.0, color);
            }
            return;
        }
        if filled {
            // A cross of two rectangles covers everything but the corners.
            draw_rectangle(x + radius, y, w - radius * 2.0, h, color);
            draw_rectangle(x, y + radius, w, h - radius * 2.0, color);
            for (cx, cy) in corners(x, y, w, h, radius) {
                draw_circle(cx, cy, radius, color);
            }
            return;
        }
        // The outline: four straight runs between the corner arcs, with the
        // arcs drawn as rings of the same thickness.
        let t = self.scale;
        draw_rectangle(x + radius, y, w - radius * 2.0, t, color);
        draw_rectangle(x + radius, y + h - t, w - radius * 2.0, t, color);
        draw_rectangle(x, y + radius, t, h - radius * 2.0, color);
        draw_rectangle(x + w - t, y + radius, t, h - radius * 2.0, color);
        for (cx, cy) in corners(x, y, w, h, radius) {
            draw_circle(cx, cy, radius, color);
            draw_circle(cx, cy, radius - t, MqColor::from_rgba(0, 0, 0, 0));
        }
    }
}

fn corners(x: f32, y: f32, w: f32, h: f32, radius: f32) -> [(f32, f32); 4] {
    [
        (x + radius, y + radius),
        (x + w - radius, y + radius),
        (x + radius, y + h - radius),
        (x + w - radius, y + h - radius),
    ]
}

/// Clip everything drawn after this to a window-space rectangle, or clear the
/// clip with `None`. macroquad exposes the scissor only through its internal
/// GL handle, and converts to the GL origin itself, so this takes top-left
/// window coordinates.
fn set_clip(rect: Option<(i32, i32, i32, i32)>) {
    unsafe {
        get_internal_gl().quad_gl.scissor(rect);
    }
}

/// Reads keys from macroquad. The mapping from our [`Key`] to macroquad's
/// `KeyCode` is the only place the two vocabularies meet.
pub struct MacroquadKeys;

impl MacroquadKeys {
    fn code(key: Key) -> KeyCode {
        match key {
            Key::Up => KeyCode::Up,
            Key::Down => KeyCode::Down,
            Key::Left => KeyCode::Left,
            Key::Right => KeyCode::Right,
            Key::Kp0 => KeyCode::Kp0,
            Key::Kp1 => KeyCode::Kp1,
            Key::Kp2 => KeyCode::Kp2,
            Key::Kp3 => KeyCode::Kp3,
            Key::Kp4 => KeyCode::Kp4,
            Key::Kp5 => KeyCode::Kp5,
            Key::Kp6 => KeyCode::Kp6,
            Key::Kp7 => KeyCode::Kp7,
            Key::Kp8 => KeyCode::Kp8,
            Key::Kp9 => KeyCode::Kp9,
            Key::Home => KeyCode::Home,
            Key::End => KeyCode::End,
            Key::PageUp => KeyCode::PageUp,
            Key::PageDown => KeyCode::PageDown,
            Key::Escape => KeyCode::Escape,
            Key::Enter => KeyCode::Enter,
            Key::KpEnter => KeyCode::KpEnter,
            Key::Backspace => KeyCode::Backspace,
            Key::Space => KeyCode::Space,
            Key::Digit0 => KeyCode::Key0,
            Key::Digit1 => KeyCode::Key1,
            Key::Digit2 => KeyCode::Key2,
            Key::Digit3 => KeyCode::Key3,
            Key::Digit4 => KeyCode::Key4,
            Key::Digit5 => KeyCode::Key5,
            Key::Digit6 => KeyCode::Key6,
            Key::Digit7 => KeyCode::Key7,
            Key::Digit8 => KeyCode::Key8,
            Key::Digit9 => KeyCode::Key9,
            Key::A => KeyCode::A,
            Key::B => KeyCode::B,
            Key::C => KeyCode::C,
            Key::D => KeyCode::D,
            Key::E => KeyCode::E,
            Key::F => KeyCode::F,
            Key::G => KeyCode::G,
            Key::H => KeyCode::H,
            Key::I => KeyCode::I,
            Key::J => KeyCode::J,
            Key::K => KeyCode::K,
            Key::L => KeyCode::L,
            Key::M => KeyCode::M,
            Key::N => KeyCode::N,
            Key::O => KeyCode::O,
            Key::P => KeyCode::P,
            Key::Q => KeyCode::Q,
            Key::R => KeyCode::R,
            Key::S => KeyCode::S,
            Key::T => KeyCode::T,
            Key::U => KeyCode::U,
            Key::V => KeyCode::V,
            Key::W => KeyCode::W,
            Key::X => KeyCode::X,
            Key::Y => KeyCode::Y,
            Key::Z => KeyCode::Z,
            Key::LeftControl => KeyCode::LeftControl,
            Key::RightControl => KeyCode::RightControl,
        }
    }
}

impl KeySource for MacroquadKeys {
    fn pressed(&self, key: Key) -> bool {
        is_key_pressed(Self::code(key))
    }

    fn down(&self, key: Key) -> bool {
        is_key_down(Self::code(key))
    }

    fn any_pressed(&self) -> bool {
        get_keys_pressed().iter().any(|k| {
            !matches!(
                k,
                KeyCode::LeftShift
                    | KeyCode::RightShift
                    | KeyCode::LeftControl
                    | KeyCode::RightControl
                    | KeyCode::LeftAlt
                    | KeyCode::RightAlt
                    | KeyCode::LeftSuper
                    | KeyCode::RightSuper
            )
        })
    }
}
