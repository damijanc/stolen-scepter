//! OpenBounty Rust shell. See `specs/shell-renderer-plan.md` for the agreed
//! architecture and work order.
//!
//! Run it against a pack:
//!
//! ```text
//! cargo run -p openbounty-shell -- path/to/game.json
//! ```

pub mod backend;
pub mod draw;
pub mod font;
pub mod layout;
pub mod palette;
pub mod present;
pub mod sprites;

use macroquad::prelude::{next_frame, Conf};

use backend::Backend;
use draw::{Color, DrawList, Dst, Rect, Sprite};
use layout::Layout;
use openbounty_engine::resources::Resources;
use palette::Pal;
use present::Present;
use sprites::SpriteCatalog;

fn window_conf() -> Conf {
    // The real size needs the pack's geometry, which is not loaded yet, so
    // this opens at the legacy default; the present layer centres whatever
    // the pack turns out to want inside it.
    Conf {
        window_title: "OpenBounty".to_owned(),
        window_width: 640,
        window_height: 400,
        high_dpi: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let Some(pack_path) = std::env::args().nth(1) else {
        eprintln!("usage: openbounty-shell <path to game.json>");
        return;
    };

    let resources = match Resources::from_json_file(std::path::Path::new(&pack_path)) {
        Ok(resources) => resources,
        Err(err) => {
            eprintln!("could not load pack {pack_path}: {err:?}");
            return;
        }
    };
    let layout = match Layout::from_resources(&resources) {
        Ok(layout) => layout,
        Err(err) => {
            eprintln!("pack {pack_path} has no usable render geometry: {err:?}");
            return;
        }
    };

    let catalog = SpriteCatalog::from_resources(&resources);
    let backend = Backend::load(&resources, &catalog).await;
    let present = Present::for_layout(&layout);

    println!(
        "loaded {} textures for a {}x{} {} screen",
        backend.texture_count(),
        layout.screen_w,
        layout.screen_h,
        if layout.is_modern { "modern" } else { "legacy" }
    );
    if !backend.missing().is_empty() {
        eprintln!("missing art: {:?}", backend.missing());
    }

    // A smoke scene, not the real chrome: it exists to prove the pipeline end
    // to end -- pack load, texture load, blit, bitmap text, integer present.
    // `chrome.rs` replaces it in the next slice.
    let list = smoke_scene(&layout, backend.palette());

    loop {
        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    }
}

fn smoke_scene(layout: &Layout, palette: &palette::Palette) -> DrawList {
    let mut list = DrawList::new();
    list.rect(
        Rect::new(0, 0, layout.screen_w, layout.screen_h),
        palette.clr(Pal::Black),
    );
    list.rect(
        Rect::new(
            layout::STATUS_X,
            layout::STATUS_Y,
            layout.status_w(),
            layout::STATUS_H,
        ),
        palette.clr(Pal::DCyan),
    );
    list.blit(
        Sprite::HudBarStrip,
        Dst::rect(0, layout::BAR_Y, layout.screen_w, layout::BAR_H),
    );
    list.blit(
        Sprite::ChromeOverworld,
        Dst::rect(0, 0, layout.screen_w, layout.screen_h),
    );
    list.text(
        " Options / Controls / Days Left:900 ",
        layout::STATUS_X + 1,
        layout::STATUS_Y + 1,
        palette.clr(Pal::White),
    );
    list.text_centered(
        "smoke scene",
        layout::MAP_X + layout.map_w / 2,
        layout::MAP_Y + layout.map_h / 2,
        Color::WHITE,
    );
    list
}
