//! A software painter for a [`DrawList`]: the headless counterpart of
//! `backend.rs`, used by the playtest runner to turn a frame into a PNG.
//!
//! It follows the backend's rules rather than C's, because it stands in for
//! the backend: one scale for the whole frame, nearest-neighbour sampling, a
//! sprite stretched to its layout cell, a 1-pixel `RectLines` band inside the
//! rectangle, text from the font strip, and one clip rectangle at a time. A
//! pixel-exact match with the GPU is not the goal; the screen a player sees is.

use std::collections::HashMap;
use std::io::Cursor;

use openbounty_engine::resources::Resources;

use crate::draw::{Color, DrawCmd, DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::pack::Pack;
use crate::sprites::{tile_art_names, tile_art_path, SpriteCatalog};

/// A decoded RGBA image.
#[derive(Clone, Debug, Default)]
pub struct Image {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
}

impl Image {
    fn at(&self, x: i32, y: i32) -> [u8; 4] {
        let o = ((y * self.w + x) * 4) as usize;
        [self.px[o], self.px[o + 1], self.px[o + 2], self.px[o + 3]]
    }
}

/// Decode a PNG to RGBA, whatever its colour type.
pub fn decode_png(bytes: &[u8]) -> Option<Image> {
    let mut decoder = png::Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).ok()?;
    let data = &buf[..info.buffer_size()];
    let px: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => data
            .chunks(3)
            .flat_map(|c| [c[0], c[1], c[2], 255])
            .collect(),
        png::ColorType::Grayscale => data.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::GrayscaleAlpha => data
            .chunks(2)
            .flat_map(|c| [c[0], c[0], c[0], c[1]])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some(Image {
        w: info.width as i32,
        h: info.height as i32,
        px,
    })
}

/// Encode RGBA pixels as a PNG.
pub fn encode_png(w: i32, h: i32, px: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, w as u32, h as u32);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer.write_image_data(px).expect("png data");
    }
    out
}

/// Every image the pack names, decoded once, keyed as the backend keys its
/// textures.
#[derive(Debug, Default)]
pub struct Images {
    images: HashMap<String, Image>,
    font: Option<Image>,
    /// Paths that would not load, as `Backend::missing` reports them.
    pub missing: Vec<String>,
}

impl Images {
    /// Load what `Backend::load` loads: the manifest, the tile art and the font.
    pub fn load(resources: &Resources, catalog: &SpriteCatalog, pack: &Pack) -> Self {
        let mut images = Self::default();
        let mut paths: Vec<String> = catalog.all_paths().into_iter().map(str::to_owned).collect();
        paths.extend(
            tile_art_names(resources)
                .iter()
                .map(|art| tile_art_path(art)),
        );
        for path in paths {
            if path.is_empty() || images.images.contains_key(&path) {
                continue;
            }
            match pack.read(&path).and_then(|bytes| decode_png(&bytes)) {
                Some(image) => {
                    images.images.insert(path, image);
                }
                None => images.missing.push(path),
            }
        }
        for path in catalog.optional_paths() {
            if let Some(image) = pack.read(path).and_then(|bytes| decode_png(&bytes)) {
                images.images.insert(path.to_owned(), image);
            }
        }
        images.font = pack
            .read(catalog.font_path())
            .and_then(|bytes| decode_png(&bytes));
        images
    }

    fn sprite(&self, catalog: &SpriteCatalog, sprite: &Sprite) -> Option<&Image> {
        match sprite {
            Sprite::Tile(art) => self.images.get(&tile_art_path(art.as_str())),
            other => self.images.get(catalog.path(*other)?),
        }
    }
}

/// An RGBA frame at a whole-number scale of the logical screen.
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<u8>,
    scale: i32,
    /// The active clip in canvas pixels: x0, y0, x1, y1 (exclusive).
    clip: (i32, i32, i32, i32),
}

impl Canvas {
    pub fn new(screen_w: i32, screen_h: i32, scale: i32) -> Self {
        let (w, h) = (screen_w * scale, screen_h * scale);
        let mut px = vec![0; (w * h * 4) as usize];
        for pixel in px.chunks_mut(4) {
            pixel[3] = 255;
        }
        Self {
            w,
            h,
            px,
            scale,
            clip: (0, 0, w, h),
        }
    }

    /// The frame as a PNG file's bytes.
    pub fn to_png(&self) -> Vec<u8> {
        encode_png(self.w, self.h, &self.px)
    }

    fn blend(&mut self, x: i32, y: i32, rgba: [u8; 4]) {
        let (x0, y0, x1, y1) = self.clip;
        if x < x0 || y < y0 || x >= x1 || y >= y1 || rgba[3] == 0 {
            return;
        }
        let o = ((y * self.w + x) * 4) as usize;
        let a = rgba[3] as u32;
        for c in 0..3 {
            let d = self.px[o + c] as u32;
            self.px[o + c] = ((rgba[c] as u32 * a + d * (255 - a)) / 255) as u8;
        }
    }

    fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, color: Color) {
        let rgba = [color.r, color.g, color.b, color.a];
        for yy in y.max(0)..(y + h).min(self.h) {
            for xx in x.max(0)..(x + w).min(self.w) {
                self.blend(xx, yy, rgba);
            }
        }
    }

    fn rect_px(&self, rect: Rect) -> (i32, i32, i32, i32) {
        let s = self.scale;
        (rect.x * s, rect.y * s, rect.w * s, rect.h * s)
    }

    /// Paint `list` in order, as `Backend::execute` does.
    pub fn paint(&mut self, list: &DrawList, catalog: &SpriteCatalog, images: &Images) {
        for cmd in list.as_slice() {
            self.run(cmd, catalog, images);
        }
        self.clip = (0, 0, self.w, self.h);
    }

    fn run(&mut self, cmd: &DrawCmd, catalog: &SpriteCatalog, images: &Images) {
        match cmd {
            DrawCmd::Rect { rect, color } => {
                let (x, y, w, h) = self.rect_px(*rect);
                self.fill(x, y, w, h, *color);
            }
            DrawCmd::RectLines { rect, color } => {
                let (x, y, w, h) = self.rect_px(*rect);
                self.outline(x, y, w, h, self.scale, *color);
            }
            DrawCmd::RectRounded {
                rect,
                roundness,
                color,
                ..
            } => self.rounded(*rect, *roundness, *color, true),
            DrawCmd::RectRoundedLines {
                rect,
                roundness,
                color,
                ..
            } => self.rounded(*rect, *roundness, *color, false),
            DrawCmd::Blit {
                sprite,
                dst,
                flip_x,
                tint,
            } => {
                let Some(image) = images.sprite(catalog, sprite) else {
                    return;
                };
                let (ox, oy) = dst.origin();
                let s = self.scale;
                let (w, h) = match dst {
                    Dst::Stretch(rect) => (rect.w * s, rect.h * s),
                    Dst::Native { .. } => (image.w * s, image.h * s),
                };
                self.blit(image, ox * s, oy * s, w, h, *flip_x, *tint);
            }
            DrawCmd::Text { text, x, y, color } => self.text(images, text, *x, *y, *color),
            DrawCmd::ClipBegin(rect) => {
                let (x, y, w, h) = self.rect_px(*rect);
                self.clip = (x.max(0), y.max(0), (x + w).min(self.w), (y + h).min(self.h));
            }
            DrawCmd::ClipEnd => self.clip = (0, 0, self.w, self.h),
        }
    }

    fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, t: i32, color: Color) {
        self.fill(x, y, w, t, color);
        self.fill(x, y + h - t, w, t, color);
        self.fill(x, y + t, t, h - 2 * t, color);
        self.fill(x + w - t, y + t, t, h - 2 * t, color);
    }

    /// raylib's rounded rectangle: `roundness` is a fraction of the shorter
    /// side, capped at half of it.
    fn rounded(&mut self, rect: Rect, roundness: f32, color: Color, filled: bool) {
        let (x, y, w, h) = self.rect_px(rect);
        let radius = (roundness.clamp(0.0, 0.5) * w.min(h) as f32).max(0.0);
        let t = self.scale as f32;
        let rgba = [color.r, color.g, color.b, color.a];
        for yy in y..y + h {
            for xx in x..x + w {
                let (px, py) = (xx as f32 + 0.5, yy as f32 + 0.5);
                // Distance outside the rectangle shrunk by the radius.
                let dx = (x as f32 + radius - px)
                    .max(px - (x + w) as f32 + radius)
                    .max(0.0);
                let dy = (y as f32 + radius - py)
                    .max(py - (y + h) as f32 + radius)
                    .max(0.0);
                let d = (dx * dx + dy * dy).sqrt();
                let inside = if radius > 0.5 { d <= radius } else { true };
                if !inside {
                    continue;
                }
                let on_edge = if radius > 0.5 && (dx > 0.0 || dy > 0.0) {
                    d > radius - t
                } else {
                    px - (x as f32) < t
                        || (x + w) as f32 - px < t
                        || py - (y as f32) < t
                        || (y + h) as f32 - py < t
                };
                if filled || on_edge {
                    self.blend(xx, yy, rgba);
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn blit(&mut self, image: &Image, x: i32, y: i32, w: i32, h: i32, flip: bool, tint: Color) {
        if w <= 0 || h <= 0 || image.w <= 0 || image.h <= 0 {
            return;
        }
        for dy in 0..h {
            let sy = ((dy as i64 * image.h as i64) / h as i64) as i32;
            for dx in 0..w {
                let mut sx = ((dx as i64 * image.w as i64) / w as i64) as i32;
                if flip {
                    sx = image.w - 1 - sx;
                }
                let mut rgba = image.at(sx, sy);
                rgba[0] = (rgba[0] as u32 * tint.r as u32 / 255) as u8;
                rgba[1] = (rgba[1] as u32 * tint.g as u32 / 255) as u8;
                rgba[2] = (rgba[2] as u32 * tint.b as u32 / 255) as u8;
                rgba[3] = (rgba[3] as u32 * tint.a as u32 / 255) as u8;
                self.blend(x + dx, y + dy, rgba);
            }
        }
    }

    /// One glyph per byte from the font strip, advancing a fixed cell.
    fn text(&mut self, images: &Images, text: &str, x: i32, y: i32, color: Color) {
        let Some(strip) = images.font.as_ref() else {
            return;
        };
        let glyph_w = (strip.w / font::GLYPH_COUNT as i32).max(1);
        let s = self.scale;
        let (cell_w, cell_h) = (font::GLYPH_W * s, font::GLYPH_H * s);
        let (mut cx, mut cy) = (x, y);
        for byte in text.as_bytes() {
            if *byte == b'\n' {
                cx = x;
                cy += font::line_height();
                continue;
            }
            let gx = font::glyph_index(*byte) as i32 * glyph_w;
            for dy in 0..cell_h {
                let sy = dy * strip.h / cell_h;
                for dx in 0..cell_w {
                    let sx = gx + dx * glyph_w / cell_w;
                    let mut rgba = strip.at(sx, sy);
                    rgba[0] = (rgba[0] as u32 * color.r as u32 / 255) as u8;
                    rgba[1] = (rgba[1] as u32 * color.g as u32 / 255) as u8;
                    rgba[2] = (rgba[2] as u32 * color.b as u32 / 255) as u8;
                    rgba[3] = (rgba[3] as u32 * color.a as u32 / 255) as u8;
                    self.blend(cx * s + dx, cy * s + dy, rgba);
                }
            }
            cx += font::GLYPH_W;
        }
    }
}

/// Every string a frame draws, in draw order: what a screen SAYS, without
/// looking at it.
pub fn texts(list: &DrawList) -> Vec<String> {
    list.as_slice()
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(canvas: &Canvas, x: i32, y: i32) -> [u8; 4] {
        let o = ((y * canvas.w + x) * 4) as usize;
        [
            canvas.px[o],
            canvas.px[o + 1],
            canvas.px[o + 2],
            canvas.px[o + 3],
        ]
    }

    fn empty_catalog() -> SpriteCatalog {
        let resources = Resources::from_json_str(r#"{"troops": []}"#).expect("fixture parses");
        SpriteCatalog::from_resources(&resources)
    }

    #[test]
    fn a_rect_fills_its_scaled_area_and_nothing_else() {
        let mut canvas = Canvas::new(10, 10, 2);
        let mut list = DrawList::new();
        list.rect(Rect::new(1, 1, 2, 2), Color::rgb(200, 0, 0));
        canvas.paint(&list, &empty_catalog(), &Images::default());
        assert_eq!(pixel(&canvas, 2, 2), [200, 0, 0, 255]);
        assert_eq!(pixel(&canvas, 5, 5), [200, 0, 0, 255]);
        assert_eq!(pixel(&canvas, 6, 6), [0, 0, 0, 255]);
    }

    #[test]
    fn a_clip_hides_what_falls_outside_it() {
        let mut canvas = Canvas::new(10, 10, 1);
        let mut list = DrawList::new();
        list.push(DrawCmd::ClipBegin(Rect::new(0, 0, 5, 10)));
        list.rect(Rect::new(0, 0, 10, 10), Color::WHITE);
        list.push(DrawCmd::ClipEnd);
        canvas.paint(&list, &empty_catalog(), &Images::default());
        assert_eq!(pixel(&canvas, 4, 4), [255, 255, 255, 255]);
        assert_eq!(pixel(&canvas, 5, 4), [0, 0, 0, 255]);
    }

    #[test]
    fn a_png_round_trips() {
        let px = vec![10, 20, 30, 255, 40, 50, 60, 128];
        let image = decode_png(&encode_png(2, 1, &px)).expect("decodes");
        assert_eq!((image.w, image.h), (2, 1));
        assert_eq!(image.px, px);
    }
}
