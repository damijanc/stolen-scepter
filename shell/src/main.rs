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
pub mod views;

use macroquad::prelude::{get_time, next_frame, Conf};

use anim::HeroAnimator;
use backend::{Backend, MacroquadKeys};
use chrome::{Chrome, StatusMode};
use dialog::Dialog;
use draw::DrawList;
use input::{Action, Key, KeySource, DIGITS};
use layout::Layout;
use map_render::MapScene;
use openbounty_engine::fog::Fog;
use openbounty_engine::game::{
    CastleOwnerKind, Game, GarrisonOutcome, GateDestKind, GateDestination, TravelMode,
    GAME_ARMY_SLOTS, GAME_GATE_DESTS_MAX,
};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::spells_adventure::{
    spell_index_by_adventure_effect, try_build_bridge, AdventureCast, AdventureEffect, SpellCast,
};
use openbounty_engine::step::{
    consume_step, game_step_basic, search_here, spend_week, SearchOutcome, StepInteraction,
    StepResult,
};
use present::Present;
use prompt::{Prompt, PromptResult};
use sprites::SpriteCatalog;
use views::{controls_visible, worldmap_has_orb, SpellsInput, TownInput, TownRow, ViewKind, Views};

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
    /// A combat spell was chosen in the field; the charge is pending discard.
    DiscardSpell {
        spell_idx: i32,
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
        PendingFlow::DiscardSpell { spell_idx } => {
            if answer == PromptResult::Yes {
                game.discard_spell(spell_idx);
            }
        }
        PendingFlow::None => {}
    }
}

/// Route a chosen spellbook slot into the engine and present the result,
/// porting the shell half of `dispatch_adventure_spell`. The bridge cast arms
/// a direction prompt, signalled through `bridge_waiting`; the gate picker is
/// still its own slice.
fn dispatch_spell(
    game: &mut Game,
    resources: &Resources,
    spell_idx: i32,
    prompt: &mut Prompt,
    dialog: &mut Dialog,
    pending: &mut PendingFlow,
    bridge_waiting: &mut bool,
    views: &mut Views,
) {
    match game.dispatch_adventure_spell(resources, spell_idx) {
        SpellCast::NotKnown => {
            dialog.open("", interact::spell_banner(resources, "spell_not_known"));
        }
        SpellCast::Unavailable => {
            dialog.open("", interact::spell_banner(resources, "spell_unavailable"));
        }
        SpellCast::OfferDiscard => {
            let display = resources
                .spell_by_index(spell_idx)
                .map(|spell| spell.name.as_str())
                .unwrap_or("");
            let name = if display.is_empty() {
                "This spell"
            } else {
                display
            };
            let header = if display.is_empty() { "Spell" } else { display };
            let body =
                format!("{name} cannot be cast in the field.\n\nDiscard it to free a spell slot?");
            prompt.yes_no(header, &body);
            *pending = PendingFlow::DiscardSpell { spell_idx };
        }
        SpellCast::Applied(cast) => match &cast {
            AdventureCast::BridgeArmed => {
                let (header, body) = interact::bridge_prompt(resources);
                dialog.open(&header, &body);
                *bridge_waiting = true;
            }
            AdventureCast::GateArmed { mode, .. } => {
                // The destination list is built fresh from game state, as C's
                // gate_menu_tick does, then handed to the picker view.
                let kind = match mode {
                    openbounty_engine::spells_adventure::GateMode::Castle => GateDestKind::Castle,
                    openbounty_engine::spells_adventure::GateMode::Town => GateDestKind::Town,
                };
                let mut destinations = [GateDestination::default(); GAME_GATE_DESTS_MAX];
                let count = game.gate_destinations(resources, kind, &mut destinations);
                views.gate_open(&destinations[..count], kind == GateDestKind::Town);
            }
            AdventureCast::NoCharge => {}
            _ => {
                if let Some((header, body)) =
                    interact::adventure_cast_message(resources, game, &cast)
                {
                    dialog.open(&header, &body);
                }
            }
        },
    }
}

/// Run one town action row: the engine applies the transaction and the result
/// is shown through the town info overlay. Ports `town_do_row`.
fn run_town_row(game: &mut Game, resources: &Resources, views: &mut Views, row: TownRow) {
    let record_key = views.town_record_key().to_owned();
    let toast = match row {
        TownRow::Contract => {
            let villain_id = game.take_next_contract(resources).map(|id| id.to_owned());
            interact::town_contract_toast(resources, villain_id.as_deref())
        }
        TownRow::Boat => {
            let (boat_x, boat_y) = views.town_boat();
            let zone = game.position.zone.as_str().to_owned();
            let outcome = if game.boat.has_boat {
                game.cancel_boat()
            } else {
                game.rent_boat(resources, boat_x, boat_y, &zone)
            };
            match interact::town_boat_toast(resources, outcome) {
                Some(body) => body,
                None => return,
            }
        }
        TownRow::Info => interact::town_intel_toast(resources, game, &record_key),
        TownRow::Spell => {
            let outcome = game.buy_spell(resources, &record_key);
            interact::town_spell_toast(resources, game, outcome)
        }
        TownRow::Siege => {
            let outcome = game.buy_siege(resources);
            interact::town_siege_toast(resources, outcome)
        }
    };
    views.town_show_info(&toast);
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
    let mut present = Present::for_layout(&layout);

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
    // A bridge cast has armed a direction prompt; the dialog owns the keys
    // until an arrow or ESC resolves it.
    let mut bridge_waiting = false;
    // The modal layers: one prompt and one dialog at a time, mirroring C's
    // single-slot prompt and dialog globals.
    let mut prompt = Prompt::default();
    let mut dialog = Dialog::default();
    let mut pending = PendingFlow::None;
    let mut views = Views::new();

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
            if bridge_waiting {
                // The bridge prompt owns the dialog: an arrow builds toward
                // that direction, ESC cancels. Nothing else dismisses.
                let dir = input::poll(&keys);
                if dir.dx != 0 || dir.dy != 0 {
                    bridge_waiting = false;
                    let built = try_build_bridge(&game, &mut map, dir.dx, dir.dy);
                    if built > 0 {
                        let bridge =
                            spell_index_by_adventure_effect(&resources, AdventureEffect::Bridge);
                        if bridge >= 0 {
                            game.spells.counts[bridge as usize] -= 1;
                        }
                        let (header, body) = interact::bridge_built(&resources, built);
                        dialog.open(&header, &body);
                    } else {
                        let (header, body) = interact::bridge_invalid(&resources);
                        dialog.open(&header, &body);
                    }
                } else if keys.pressed(Key::Escape) {
                    bridge_waiting = false;
                    dialog.dismiss();
                }
            } else if keys.any_pressed() {
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
        } else if views.active() == Some(ViewKind::Controls) {
            // Navigate rows with Up/Down; Enter/Space (or a digit) advances
            // the value; ESC or C closes. The Scale row is appended last,
            // modern only.
            let vis = controls_visible(&resources);
            let count = vis.len() + usize::from(layout.is_modern);
            let scale_row = if layout.is_modern {
                vis.len()
            } else {
                usize::MAX
            };
            if count > 0 {
                let cur = views.controls_cursor() as usize;
                if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
                    views.controls_set_cursor(((cur + count - 1) % count) as i32);
                } else if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
                    views.controls_set_cursor(((cur + 1) % count) as i32);
                } else if keys.pressed(Key::Enter)
                    || keys.pressed(Key::KpEnter)
                    || keys.pressed(Key::Space)
                {
                    if cur == scale_row {
                        present.advance_scale();
                    } else {
                        views.controls_advance(&mut game, &resources, vis[cur]);
                    }
                } else if keys.pressed(Key::Escape) || keys.pressed(Key::C) {
                    views.dismiss();
                } else {
                    for k in 0..count.min(9) {
                        if keys.pressed(DIGITS[1 + k]) {
                            views.controls_set_cursor(k as i32);
                            if k == scale_row {
                                present.advance_scale();
                            } else {
                                views.controls_advance(&mut game, &resources, vis[k]);
                            }
                            break;
                        }
                    }
                }
            } else if keys.any_pressed() {
                views.dismiss();
            }
        } else if views.active() == Some(ViewKind::Spells) {
            // Left/Right switch columns, A-G cast from the current column,
            // ESC cancels. Any other key dismisses the read-only book.
            match views.spells_update(&keys) {
                SpellsInput::Cast(spell_idx) => {
                    dispatch_spell(
                        &mut game,
                        &resources,
                        spell_idx,
                        &mut prompt,
                        &mut dialog,
                        &mut pending,
                        &mut bridge_waiting,
                        &mut views,
                    );
                }
                SpellsInput::Handled => {}
                SpellsInput::None => {
                    if keys.any_pressed() {
                        views.dismiss();
                    }
                }
            }
        } else if views.active() == Some(ViewKind::Gate) {
            // The gate picker owns its arrows/letter/Enter input; a confirmed
            // choice teleports through the engine, which also spends the charge.
            if views.gate_update(&keys) {
                let chosen = views.gate_take_chosen();
                if let Some(index) = chosen {
                    let destination = views.gate_dest(index).copied();
                    let is_town = views.gate_is_town();
                    if let Some(destination) = destination {
                        let spell_id = if is_town { "town_gate" } else { "castle_gate" };
                        game.gate_teleport(&resources, &mut map, &mut fog, &destination, spell_id);
                    }
                }
            }
        } else if views.active() == Some(ViewKind::Town) {
            // The town menu owns its arrows/letter/Enter input; executing a row
            // runs the engine transaction and shows its info toast.
            match views.town_update(&keys) {
                TownInput::RunRow(row) => run_town_row(&mut game, &resources, &mut views, row),
                TownInput::Dismissed | TownInput::Handled | TownInput::None => {}
            }
        } else if views.active() == Some(ViewKind::HomeCastle) {
            // Throne room / barracks: ESC pops, A and B run the (still
            // deferred) recruit and audience sub-flows.
            if keys.pressed(Key::Escape) {
                views.dismiss();
            } else if keys.pressed(Key::A) {
                eprintln!("recruit soldiers not yet wired");
            } else if keys.pressed(Key::B) {
                eprintln!("audience with the king not yet wired");
            }
        } else if views.active() == Some(ViewKind::OwnCastle) {
            // SPACE toggles garrison/remove; A..E transfer the chosen stack.
            if keys.pressed(Key::Escape) {
                views.dismiss();
            } else if keys.pressed(Key::Space) {
                views.own_castle_toggle_mode();
            } else {
                let castle_id = views.own_castle_castle_id().to_owned();
                let garrison_mode = views.own_castle_garrison_mode();
                let keys_abcd = [Key::A, Key::B, Key::C, Key::D, Key::E];
                for (slot, key) in keys_abcd.iter().enumerate().take(GAME_ARMY_SLOTS) {
                    if keys.pressed(*key) {
                        let outcome = if garrison_mode {
                            game.garrison_troop(&castle_id, slot)
                        } else {
                            game.ungarrison_troop(&castle_id, slot)
                        };
                        if outcome != GarrisonOutcome::Ok {
                            dialog.open("", interact::garrison_refusal(&resources, outcome));
                        }
                        break;
                    }
                }
            }
        } else if views.is_active() {
            // The worldmap alone treats SPACE specially: with the crystal orb
            // it toggles fog-only vs whole-map; without it SPACE dismisses
            // like any other key. Every other read-only view dismisses on any
            // key, as C does.
            if views.active() == Some(ViewKind::Worldmap) && keys.pressed(Key::Space) {
                if worldmap_has_orb(&game, &resources) {
                    views.toggle_whole_map();
                } else {
                    views.dismiss();
                }
            } else if keys.any_pressed() {
                views.dismiss();
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
                Action::ViewCharacter => views.set(ViewKind::Character),
                Action::ViewArmy => views.set(ViewKind::Army),
                Action::ViewContract => views.set(ViewKind::Contract),
                Action::ViewPuzzle => views.set(ViewKind::Puzzle),
                Action::ViewMap => views.set(ViewKind::Worldmap),
                Action::ViewControls => views.set(ViewKind::Controls),
                Action::CastSpell => {
                    if !game.stats.knows_magic {
                        let (header, body) = interact::no_spell_banner(&resources);
                        dialog.open(&header, &body);
                    } else {
                        views.set(ViewKind::Spells);
                        views.spells_set_cast_mode(true);
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
                        StepInteraction::Town { id, boat_x, boat_y } => {
                            let display_name = resources
                                .town_by_id(id.as_str())
                                .map(|town| {
                                    if town.name.is_empty() {
                                        town.id.as_str()
                                    } else {
                                        town.name.as_str()
                                    }
                                })
                                .unwrap_or(id.as_str());
                            views.town_open(display_name, id.as_str(), *boat_x, *boat_y);
                        }
                        StepInteraction::CastleGate { id } => {
                            if resources
                                .castle_by_id(id.as_str())
                                .is_some_and(|c| c.is_home())
                            {
                                views.home_castle_open();
                            } else if game.castles.iter().any(|c| {
                                c.id.as_str() == id && c.owner_kind == CastleOwnerKind::Player
                            }) {
                                views.own_castle_open(id.as_str());
                            } else {
                                // Hostile castle: the siege flow is the combat slice.
                                eprintln!("hostile castle siege not yet wired");
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
            } else if views.active() == Some(ViewKind::Worldmap) {
                StatusMode::Worldmap {
                    whole_map: views.whole_map(),
                }
            } else if views.wants_exit_hint(&game) {
                StatusMode::ExitHint
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

        // A view covers the map area while the overworld keeps running below.
        // Each view derives its own animation clock from the wall time, as C
        // does with GetTime().
        views.draw(
            &mut list,
            &layout,
            backend.palette(),
            &resources,
            &catalog,
            &map,
            &fog,
            &game,
            &present,
            get_time(),
        );

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
