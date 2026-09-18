//! OpenBounty Rust shell. See `specs/shell-renderer-plan.md` for the agreed
//! architecture and work order.
//!
//! Run it against a pack:
//!
//! ```text
//! cargo run -p openbounty-shell -- path/to/game.json
//! ```

pub mod anim;
pub mod backend;
pub mod banner;
pub mod chrome;
pub mod draw;
pub mod font;
pub mod layout;
pub mod map_render;
pub mod palette;
pub mod present;
pub mod sprites;

use macroquad::prelude::{get_time, next_frame, Conf};

use anim::HeroAnimator;
use backend::Backend;
use chrome::{Chrome, StatusMode};
use draw::DrawList;
use layout::Layout;
use map_render::MapScene;
use openbounty_engine::fog::Fog;
use openbounty_engine::game::Game;
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
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

    // A world to look at. Starting a real game is the decision loop's job
    // (work order step 7); this seeds one so the viewport has something in
    // it, and the hero does not move yet.
    let mut game = Game::default();
    game.init_seeded_with_resources(&resources, Some("sir rust"), 0, 0, 42);
    let zone = game.position.zone.as_str().to_owned();
    let map = match Map::load_zone_with_placements(&resources, &zone, &game) {
        Ok(map) => map,
        Err(err) => {
            eprintln!("could not load zone {zone}: {err}");
            return;
        }
    };
    let mut fog = Fog::new();
    fog.reveal(&map, game.position.x, game.position.y, 2);

    let has_idle = catalog.hero(draw::HeroAnim::Idle).is_present();
    // The hero has never stepped, because there is no input yet. He therefore
    // stands perfectly still, which is what the shipped pack should look like.
    let last_step = None;
    let mut hero_anim = HeroAnimator::new();
    let mut list = DrawList::new();

    loop {
        hero_anim.update(get_time(), last_step, &game.stats.options, has_idle);
        list.clear();
        Chrome {
            layout: &layout,
            palette: backend.palette(),
            resources: &resources,
            game: &game,
            mode: StatusMode::Adventure,
        }
        .draw(&mut list);
        MapScene {
            layout: &layout,
            catalog: &catalog,
            palette: backend.palette(),
            resources: &resources,
            game: &game,
            map: &map,
            fog: &fog,
            tick: hero_anim.tick(),
            moving: hero_anim.moving(),
        }
        .draw(&mut list);

        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    }
}
