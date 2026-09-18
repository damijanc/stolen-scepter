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
pub mod dialog;
pub mod draw;
pub mod font;
pub mod input;
pub mod interact;
pub mod layout;
pub mod map_render;
pub mod palette;
pub mod present;
pub mod prompt;
pub mod sprites;

use macroquad::prelude::{get_time, next_frame, Conf};

use anim::HeroAnimator;
use backend::{Backend, MacroquadKeys};
use chrome::{Chrome, StatusMode};
use dialog::Dialog;
use draw::DrawList;
use input::{Action, Key, KeySource};
use layout::Layout;
use map_render::MapScene;
use openbounty_engine::fog::Fog;
use openbounty_engine::game::{Game, TravelMode, GAME_ARMY_SLOTS};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::step::{
    consume_step, game_step_basic, search_here, spend_week, SearchOutcome, StepInteraction,
    StepResult,
};
use present::Present;
use prompt::{Prompt, PromptResult};
use sprites::SpriteCatalog;

/// Which decision a prompt answer resolves, porting C's `pending_flow`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
enum PendingFlow {
    #[default]
    None,
    Search,
    DismissArmy,
    /// Confirming the dismissal of the LAST stack; a yes sends the hero home.
    DismissLast,
    /// A chest offered a gold/leadership choice; the amounts are pending.
    ChestChoice {
        gold: i32,
        leadership: i32,
    },
    /// The Archmage Aurange offer to teach magic for gold.
    Alcove,
    /// Sail to a discovered neighbor continent; the ids are pending.
    Navigate {
        zones: Vec<String>,
    },
}

/// Text C compiles in for the prompts and dialogs the shell opens. Kept
/// verbatim; the pack overrides them under `strings.banners` and
/// `strings.dialog_titles`.
const DEFAULT_DT_SEARCH: &str = "Search...";
const DEFAULT_BODY_SEARCH: &str = "It will take %DAYS% days to\nsearch this area.\nSearch?";
const DEFAULT_DT_DISMISS_ARMY: &str = "Dismiss Army";
const DEFAULT_BODY_DISMISS_PICK: &str = "Dismiss which troop?";
const DEFAULT_DT_DISMISS_LAST: &str = "Dismiss last army";
const DEFAULT_BODY_DISMISS_LAST: &str = "If you dismiss your last\n\
    army, you will be sent back\n\
    to the King in disgrace.\n\
    Dismiss last army?";

/// Route a prompt answer into the engine, porting the shell half of
/// `shell_promptdispatch.c`. The engine cores are called directly rather than
/// through a mirrored queue; each answer either mutates game state, opens a
/// message dialog, or chains the next prompt.
fn dispatch_answer(
    game: &mut Game,
    map: &mut Map,
    fog: &mut Fog,
    resources: &Resources,
    answer: PromptResult,
    pending: &mut PendingFlow,
    prompt: &mut Prompt,
    dialog: &mut Dialog,
) {
    let flow = std::mem::take(pending);
    match flow {
        PendingFlow::Search => {
            if answer == PromptResult::Yes {
                match search_here(game, resources) {
                    SearchOutcome::Won => {
                        // The win cartoon and screen are the end-game slice.
                    }
                    SearchOutcome::NothingFound { .. } => {
                        dialog.open("", "Your search of this area has\nrevealed nothing.");
                    }
                    SearchOutcome::OutOfTime => {
                        // game_over is now set; the lose screen is the
                        // end-game slice.
                    }
                }
            }
        }
        PendingFlow::DismissArmy => {
            let slot = match answer {
                PromptResult::One => Some(0),
                PromptResult::Two => Some(1),
                PromptResult::Three => Some(2),
                PromptResult::Four => Some(3),
                PromptResult::Five => Some(4),
                _ => None,
            };
            if let Some(slot) = slot {
                // True means this was the last stack: nothing is dismissed yet,
                // and the caller must confirm the disgrace.
                if game.dismiss_army_slot(slot) {
                    let body = banner::text(
                        resources,
                        "banners",
                        "body_dismiss_last",
                        DEFAULT_BODY_DISMISS_LAST,
                    );
                    let header = banner::text(
                        resources,
                        "dialog_titles",
                        "dismiss_last",
                        DEFAULT_DT_DISMISS_LAST,
                    );
                    prompt.yes_no(header, body);
                    *pending = PendingFlow::DismissLast;
                }
            }
        }
        PendingFlow::DismissLast => {
            if answer == PromptResult::Yes {
                game.temp_death(resources, map, fog);
            }
        }
        PendingFlow::ChestChoice { gold, leadership } => {
            match answer {
                // A = gold, B = distribute to the peasants for leadership.
                PromptResult::One => game.accept_chest_gold(gold),
                PromptResult::Two => game.accept_chest_leadership(leadership),
                _ => {}
            }
        }
        PendingFlow::Alcove => {
            if answer == PromptResult::Yes {
                if game.accept_alcove(map, resources) {
                    let (header, body) = interact::alcove_taught(resources);
                    dialog.open(&header, &body);
                } else {
                    let (header, body) = interact::alcove_no_gold(resources);
                    dialog.open(&header, &body);
                }
            }
        }
        PendingFlow::Navigate { zones } => {
            let index = match answer {
                PromptResult::One => Some(0),
                PromptResult::Two => Some(1),
                PromptResult::Three => Some(2),
                PromptResult::Four => Some(3),
                PromptResult::Five => Some(4),
                _ => None,
            };
            if let Some(target) = index.and_then(|i| zones.get(i)) {
                if game.switch_zone(resources, map, fog, target) {
                    spend_week(game, resources);
                } else {
                    dialog.open("", "Cannot reach that continent.");
                }
            }
        }
        PendingFlow::None => {}
    }
}

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
    // The modal layers: one prompt and one dialog at a time, mirroring C's
    // single-slot prompt and dialog globals.
    let mut prompt = Prompt::default();
    let mut dialog = Dialog::default();
    let mut pending = PendingFlow::None;

    loop {
        hero_anim.update(get_time(), last_step, &game.stats.options, has_idle);

        // Input priority: the message dialog owns the keys while it is open
        // (C's issue #19 gate), then the prompt, then the fast-quit status
        // prompt, then the overworld.
        if dialog.is_active() {
            let ctrl = keys.down(Key::LeftControl) || keys.down(Key::RightControl);
            if ctrl && keys.pressed(Key::Q) {
                return;
            }
            if keys.any_pressed() {
                if !dialog.advance() {
                    dialog.dismiss();
                }
            }
        } else if prompt.is_active() {
            let answer = prompt.update(&keys);
            if answer != PromptResult::None {
                dispatch_answer(
                    &mut game,
                    &mut map,
                    &mut fog,
                    &resources,
                    answer,
                    &mut pending,
                    &mut prompt,
                    &mut dialog,
                );
            }
        } else if fast_quit {
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
                Action::Search => {
                    let days = resources.tuning.search_cost_days.to_string();
                    let body = banner::formatted(
                        &resources,
                        "banners",
                        "body_search",
                        DEFAULT_BODY_SEARCH,
                        &[("DAYS", &days)],
                    );
                    let header =
                        banner::text(&resources, "dialog_titles", "search", DEFAULT_DT_SEARCH);
                    prompt.yes_no(header, &body);
                    pending = PendingFlow::Search;
                }
                Action::DismissArmy => {
                    let occupied = (0..GAME_ARMY_SLOTS)
                        .filter(|&i| !game.army[i].id.is_empty() && game.army[i].count > 0)
                        .count();
                    if occupied > 0 {
                        let body = banner::text(
                            &resources,
                            "banners",
                            "body_dismiss_pick",
                            DEFAULT_BODY_DISMISS_PICK,
                        );
                        let header = banner::text(
                            &resources,
                            "dialog_titles",
                            "dismiss_army",
                            DEFAULT_DT_DISMISS_ARMY,
                        );
                        prompt.numeric(header, body, GAME_ARMY_SLOTS as i32);
                        pending = PendingFlow::DismissArmy;
                    }
                }
                Action::NewContinent => {
                    if game.travel_mode != TravelMode::Boat {
                        let (header, body) = interact::must_be_sailing(&resources);
                        dialog.open(&header, &body);
                    } else {
                        let targets = interact::navigate_targets(&resources, &game);
                        if targets.is_empty() {
                            let (header, body) = interact::no_continents(&resources);
                            dialog.open(&header, &body);
                        } else {
                            let (header, body) = interact::navigate_body(&resources, &targets);
                            prompt.numeric(&header, &body, targets.len() as i32);
                            pending = PendingFlow::Navigate {
                                zones: targets.into_iter().map(|(id, _)| id).collect(),
                            };
                        }
                    }
                }
                Action::None => {}
                // Views and saving still need layers that do not exist yet.
                // Say so rather than swallowing the key.
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
                    match interaction {
                        StepInteraction::Sign { title, body, .. } => dialog.open(title, body),
                        StepInteraction::TreasureChest { roll, .. } => {
                            if let Some(body) = interact::chest_choice(&resources, roll) {
                                prompt.ab("", &body);
                                pending = PendingFlow::ChestChoice {
                                    gold: roll.pending.pending_gold,
                                    leadership: roll.pending.pending_leadership,
                                };
                            } else if let Some((header, body)) =
                                interact::chest_message(&resources, roll)
                            {
                                dialog.open(&header, &body);
                            }
                        }
                        StepInteraction::Alcove => {
                            if game.stats.knows_magic {
                                let (header, body) = interact::alcove_already(&resources);
                                dialog.open(&header, &body);
                            } else {
                                let (header, body) = interact::alcove_offer(&resources);
                                prompt.yes_no(&header, &body);
                                pending = PendingFlow::Alcove;
                            }
                        }
                        StepInteraction::None => {}
                        other => {
                            if let Some((header, body)) =
                                interact::pickup_message(&resources, other)
                            {
                                dialog.open(&header, &body);
                            } else {
                                println!("{other:?}");
                            }
                        }
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

        // The prompt replaces the bottom frame; the dialog draws LAST so it
        // covers a prompt that is up at the same time, exactly as C's overlay
        // orders them.
        if prompt.is_active() {
            prompt.draw(&mut list, &layout, backend.palette(), &resources);
        }
        if dialog.is_active() {
            dialog.draw(&mut list, &layout, backend.palette());
        }

        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    }
}
