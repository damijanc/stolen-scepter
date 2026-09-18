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
pub mod input;
pub mod layout;
pub mod map_render;
pub mod palette;
pub mod present;
pub mod sprites;

use macroquad::prelude::{get_time, next_frame, Conf};

use anim::HeroAnimator;
use backend::{Backend, MacroquadKeys};
use chrome::{Chrome, StatusMode};
use draw::DrawList;
use input::{Action, Key, KeySource};
use layout::Layout;
use map_render::MapScene;
use openbounty_engine::fog::Fog;
use openbounty_engine::game::Game;
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::step::{consume_step, game_step_basic, spend_week, StepResult};
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

    // The class picker and name entry are `startup.c`, still unported, so the
    // hero is seeded here instead.
    let mut game = Game::default();
    game.init_seeded_with_resources(&resources, Some("sir rust"), 0, 0, 42);
    let zone = game.position.zone.as_str().to_owned();
    let mut map = match Map::load_zone_with_placements(&resources, &zone, &game) {
        Ok(map) => map,
        Err(err) => {
            eprintln!("could not load zone {zone}: {err}");
            return;
        }
    };
    let mut fog = Fog::new();
    fog.reveal(&map, game.position.x, game.position.y, 2);

    let has_idle = catalog.hero(draw::HeroAnim::Idle).is_present();
    let mut last_step = None;
    let mut hero_anim = HeroAnimator::new();
    let mut list = DrawList::new();
    let keys = MacroquadKeys;
    // Ctrl+Q arms a status-bar prompt rather than a dialog, which is why
    // chrome has a mode for it.
    let mut fast_quit = false;

    loop {
        hero_anim.update(get_time(), last_step, &game.stats.options, has_idle);

        if fast_quit {
            // The prompt is modal: nothing else reads input until it is
            // answered.
            if keys.pressed(Key::Y) {
                return;
            }
            if keys.pressed(Key::N) || keys.pressed(Key::Escape) {
                fast_quit = false;
            }
        } else if !game.stats.game_over {
            let input = input::poll(&keys);
            match input.action {
                // Legality lives in the engine, and both are silent on
                // failure exactly as C is.
                Action::Fly => {
                    game.mount_fly(&resources);
                }
                Action::Land => {
                    game.land_here(&map);
                }
                Action::EndWeek => {
                    spend_week(&mut game, &resources);
                }
                Action::Rest => {
                    consume_step(&mut game, &resources, false);
                }
                Action::FastQuit => fast_quit = true,
                Action::None => {}
                // Views, prompts and saving all need layers that do not
                // exist yet. Say so rather than swallowing the key.
                other => println!("{other:?} is not wired up yet"),
            }

            if input.is_move() {
                let result = game_step_basic(
                    &mut game, &mut map, &mut fog, &resources, input.dx, input.dy,
                );
                if let StepResult::Moved { interaction, .. } = &result {
                    // The clock the hero animates on: he walks for a moment
                    // after each step, then holds still.
                    last_step = Some(get_time());
                    if *interaction != openbounty_engine::step::StepInteraction::None {
                        println!("{interaction:?}");
                    }
                }
            }
        }

        list.clear();
        Chrome {
            layout: &layout,
            palette: backend.palette(),
            resources: &resources,
            game: &game,
            mode: if fast_quit {
                StatusMode::FastQuit
            } else {
                StatusMode::Adventure
            },
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
