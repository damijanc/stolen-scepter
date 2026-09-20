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
pub mod combat;
pub mod dialog;
pub mod draw;
pub mod font;
pub mod input;
pub mod interact;
pub mod layout;
pub mod map_render;
pub mod palette;
pub mod pointer;
pub mod present;
pub mod prompt;
pub mod savepath;
pub mod sprites;
pub mod startup;
pub mod views;

use macroquad::prelude::{get_time, next_frame, Conf};

use anim::HeroAnimator;
use backend::{Backend, MacroquadKeys, MacroquadPointer};
use chrome::{Chrome, StatusMode};
use combat::{combat_render, CombatLoop, CombatStatus};
use dialog::Dialog;
use draw::DrawList;
use input::{Action, Key, KeySource, DIGITS};
use layout::Layout;
use map_render::MapScene;
use openbounty_engine::combat::{
    CombatMode, CombatResult, CombatTarget, COMBAT_SIDE_AI, COMBAT_SIDE_PLAYER, COMBAT_SLOTS,
};
use openbounty_engine::fog::Fog;
use openbounty_engine::game::{
    ArmyStack, CastleOwnerKind, FriendlyOffer, Game, GarrisonOutcome, GateDestKind,
    GateDestination, TravelMode, VillainSiegeOutcome, GAME_ARMY_SLOTS, GAME_GATE_DESTS_MAX,
};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::savegame::{save_read, save_write};
use openbounty_engine::spells_adventure::{
    spell_index_by_adventure_effect, try_build_bridge, AdventureCast, AdventureEffect, SpellCast,
};
use openbounty_engine::step::{
    consume_step, game_step_basic, search_here, spend_week, SearchOutcome, StepInteraction,
    StepResult,
};
use pointer::{Button, HitList, PointerSource};
use present::Present;
use prompt::{Prompt, PromptResult};
use savepath::SavePaths;
use sprites::SpriteCatalog;
use startup::{Startup, StartupAction};
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
    /// A dwelling offered a troop; the "how many" entry is pending.
    Dwelling {
        troop_id: String,
    },
    /// A hostile castle asked whether to lay siege.
    SiegeMonster {
        castle_id: String,
    },
    SiegeVillain {
        castle_id: String,
    },
    /// A friendly foe offered to join; the deterministic troop stack is pending.
    FriendlyOffer {
        offer: FriendlyOffer,
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
const DEFAULT_TEMP_DEATH: &str = "After being disgraced on the\n\
    field of battle, King\n\
    Maximus summons you to his\n\
    castle. After a lesson in\n\
    tactics, he reluctantly re-\n\
    issues your commission and\n\
    sends you on your way.";
const DEFAULT_BODY_SAVE_CONFIRM: &str = "Press Control-Q to Quit or\nany other key to continue.";

/// Which battle a combat session is resolving.
enum CombatFlow {
    MonsterSiege {
        castle_id: String,
    },
    VillainSiege {
        castle_id: String,
    },
    HostileFoe {
        foe_id: String,
        foe_x: i32,
        foe_y: i32,
    },
}

/// A running interactive battle plus the flow that armed it.
struct CombatSession {
    flow: CombatFlow,
    combat: CombatLoop,
    target_name: String,
}

/// Build the interactive combat for a flow, porting the shell half of
/// `RunCombat`'s setup: the target garrison is copied out of game state so the
/// borrow does not fight the `&mut Game` the loop needs.
fn start_combat(game: &Game, resources: &Resources, flow: CombatFlow) -> CombatSession {
    match &flow {
        CombatFlow::MonsterSiege { castle_id } | CombatFlow::VillainSiege { castle_id } => {
            let garrison = game
                .castles
                .iter()
                .find(|castle| castle.id.as_str() == castle_id)
                .map(|castle| castle.garrison)
                .unwrap_or_default();
            let name = interact::siege_target_name(resources, game, castle_id);
            let target = CombatTarget {
                name: name.as_str(),
                garrison: &garrison,
                seed_key: castle_id.as_str(),
            };
            let combat = CombatLoop::new(game, resources, CombatMode::Castle, &target);
            CombatSession {
                flow,
                combat,
                target_name: name,
            }
        }
        CombatFlow::HostileFoe { foe_id, .. } => {
            let garrison = game.foes[..game.foe_count]
                .iter()
                .find(|foe| foe.placement_id.as_str() == foe_id)
                .map(|foe| foe.garrison)
                .unwrap_or_default();
            let target = CombatTarget {
                name: "Hostile band",
                garrison: &garrison,
                seed_key: foe_id.as_str(),
            };
            let combat = CombatLoop::new(game, resources, CombatMode::Foe, &target);
            CombatSession {
                flow,
                combat,
                target_name: "Hostile band".to_owned(),
            }
        }
    }
}

/// Apply a finished battle: credit spoils and write the surviving army back on
/// a win, then resolve the flow (castle ownership, foe removal, or the loss
/// handling) and show the victory banner.
fn finish_combat(
    game: &mut Game,
    map: &mut Map,
    fog: &mut Fog,
    resources: &Resources,
    session: CombatSession,
    result: CombatResult,
    dialog: &mut Dialog,
) -> Option<(String, String)> {
    if result == CombatResult::Win {
        let spoils = session.combat.combat.spoils[COMBAT_SIDE_AI];
        game.stats.gold = game.stats.gold.wrapping_add(spoils);
        for slot in 0..COMBAT_SLOTS {
            let unit = &session.combat.combat.units[COMBAT_SIDE_PLAYER][slot];
            if unit.troop_idx < 0 || unit.count == 0 {
                game.army[slot] = ArmyStack::default();
            } else if let Some(troop) = resources.troop_by_index(unit.troop_idx) {
                game.army[slot].id.copy_from_str(troop.id.as_str());
                game.army[slot].count = unit.count;
            }
        }
        game.compact_army();
    }

    let mut capture = None;
    let temp_death = match &session.flow {
        CombatFlow::MonsterSiege { castle_id } => game.resolve_monster_siege(castle_id, result),
        CombatFlow::VillainSiege { castle_id } => {
            // Snapshot the lord's name and bounty before the resolution clears
            // the castle record.
            let villain_id = game
                .castles
                .iter()
                .find(|castle| castle.id.as_str() == castle_id)
                .map(|castle| castle.villain_id.as_str())
                .unwrap_or("");
            let (name, reward) = resources
                .villain_by_id(villain_id)
                .map(|villain| {
                    let name = if villain.name.is_empty() {
                        villain_id.to_owned()
                    } else {
                        villain.name.as_str().to_owned()
                    };
                    (name, villain.reward)
                })
                .unwrap_or_else(|| (villain_id.to_owned(), 0));
            let outcome = game.resolve_villain_siege(resources, castle_id, result);
            capture = interact::capture_message(outcome, &name, reward);
            outcome == VillainSiegeOutcome::Repelled
        }
        CombatFlow::HostileFoe {
            foe_id,
            foe_x,
            foe_y,
        } => game.resolve_hostile_foe(map, foe_id, *foe_x, *foe_y, result),
    };
    if temp_death {
        game.temp_death(resources, map, fog);
        // `perform_temp_death`: the engine owns the transition; the shell adds
        // only the untitled temp_death banner, following the battle.
        let body = banner::text(resources, "banners", "temp_death", DEFAULT_TEMP_DEATH);
        dialog.open("", body);
    }

    if result == CombatResult::Win {
        let hero = if game.character.name.is_empty() {
            "warrior"
        } else {
            game.character.name.as_str()
        };
        let body = format!(
            "Well done {hero}, you have\nsuccessfully vanquished\n{}.\n\nSpoils of War: {} gold",
            session.target_name, session.combat.combat.spoils[COMBAT_SIDE_AI]
        );
        dialog.open("Victory!", &body);
    }

    capture
}

/// A pending end-of-week screen, porting `pending_week_phase`: the astrology
/// banner first, then the budget panel once it is dismissed.
enum WeekEnd {
    Astrology { week_id: i32, troop: i32, paid: i32 },
    Budget { week_id: i32, paid: i32 },
}

/// Arm the week-end screens, porting `schedule_week_end`. `week_id` and the
/// astrology troop are read now, while the boundary that produced them is
/// still current.
fn schedule_week_end(pending: &mut Option<WeekEnd>, game: &Game, resources: &Resources, paid: i32) {
    *pending = Some(WeekEnd::Astrology {
        week_id: game.week_id(resources),
        troop: game.stats.last_astrology_troop,
        paid,
    });
}

/// Ports `show_win_game`: record the win and open the ending, starting with
/// the cartoon cutscene when the pack ships its tiles.
fn show_win_game(views: &mut Views, resources: &Resources, game: &mut Game) {
    game.stats.game_over = true;
    let body = interact::end_message(resources, game, true);
    views.win_cartoon_open(resources, &body);
}

/// Ports `show_lose_game`: open the ending screen. `game_over` is already set
/// by whatever ran out of days.
fn show_lose_game(views: &mut Views, resources: &Resources, game: &Game) {
    let body = interact::end_message(resources, game, false);
    views.open_end_game(false, &body);
}

/// Show the next pending week-end screen when nothing modal is up, porting
/// `pump_week_end_dialog`. Astrology queues the budget behind it.
fn pump_week_end(
    pending: &mut Option<WeekEnd>,
    game: &Game,
    resources: &Resources,
    dialog: &mut Dialog,
) {
    let Some(week) = pending.take() else {
        return;
    };
    match week {
        WeekEnd::Astrology {
            week_id,
            troop,
            paid,
        } => {
            let name = resources
                .troop_by_index(troop)
                .map(|troop| troop.name.as_str())
                .filter(|name| !name.is_empty())
                .unwrap_or("Peasants");
            let (header, body) = interact::astrology_message(resources, week_id, name);
            dialog.open(&header, &body);
            *pending = Some(WeekEnd::Budget { week_id, paid });
        }
        WeekEnd::Budget { week_id, paid } => {
            let (header, body) = interact::budget_message(resources, game, week_id, paid);
            dialog.open(&header, &body);
        }
    }
}

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
    pending_week: &mut Option<WeekEnd>,
    prompt: &mut Prompt,
    dialog: &mut Dialog,
    views: &mut Views,
) -> Option<CombatFlow> {
    let flow = std::mem::take(pending);
    match flow {
        PendingFlow::Search => {
            if answer == PromptResult::Yes {
                match search_here(game, resources) {
                    SearchOutcome::Won => {
                        // The win ends input and shows the ending; the cartoon
                        // is a later slice.
                        show_win_game(views, resources, game);
                    }
                    SearchOutcome::NothingFound {
                        week_commission, ..
                    } => {
                        dialog.open("", "Your search of this area has\nrevealed nothing.");
                        if week_commission > 0 {
                            schedule_week_end(pending_week, game, resources, week_commission);
                        }
                    }
                    SearchOutcome::OutOfTime => {
                        // `game_over` is already set; show the ending.
                        show_lose_game(views, resources, game);
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
                let body = banner::text(resources, "banners", "temp_death", DEFAULT_TEMP_DEATH);
                dialog.open("", body);
            }
        }
        PendingFlow::FriendlyOffer { offer } => {
            // Yes accepts the free troop stack; either answer consumes the foe.
            game.resolve_friendly_offer(map, offer, answer == PromptResult::Yes);
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
                    let (_weeks, paid) = spend_week(game, resources);
                    if paid > 0 {
                        schedule_week_end(pending_week, game, resources, paid);
                    }
                    if game.stats.game_over {
                        show_lose_game(views, resources, game);
                    }
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
        PendingFlow::Dwelling { troop_id } => {
            if answer == PromptResult::Yes {
                let requested = prompt.text_input_value();
                let zone = game.position.zone.as_str().to_owned();
                let x = game.position.dwelling_x;
                let y = game.position.dwelling_y;
                game.recruit_from_dwelling(resources, &zone, x, y, &troop_id, requested);
            }
        }
        PendingFlow::SiegeMonster { castle_id } => {
            if answer == PromptResult::Yes {
                return Some(CombatFlow::MonsterSiege { castle_id });
            }
        }
        PendingFlow::SiegeVillain { castle_id } => {
            if answer == PromptResult::Yes {
                return Some(CombatFlow::VillainSiege { castle_id });
            }
        }
        PendingFlow::None => {}
    }
    None
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
    let mut pack_path: Option<String> = None;
    let mut seed_index: i32 = -1;
    let mut save_dir: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--seed" {
            seed_index = args
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(-1);
        } else if arg == "--save-dir" {
            save_dir = args.next();
        } else if pack_path.is_none() {
            pack_path = Some(arg);
        }
    }
    let Some(pack_path) = pack_path else {
        eprintln!("usage: openbounty-shell [--seed <n>] [--save-dir <dir>] <path to game.json>");
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

    let has_idle = catalog.hero(draw::HeroAnim::Idle).is_present();
    let mut last_step = None;
    let mut hero_anim = HeroAnimator::new();
    let mut list = DrawList::new();
    // Rebuilt every frame during draw and hit-tested on the NEXT frame's
    // input; see specs/mouse-support-design.md.
    let mut hits = HitList::new();
    let keys = MacroquadKeys;
    let pointer = MacroquadPointer;

    // Pre-game wizard: class select, then name/difficulty. It blocks on the
    // player, exactly as C's `startup_flow` does before the game exists.
    let save_paths = match &save_dir {
        Some(dir) => SavePaths::with_override(dir),
        None => SavePaths::new(),
    };
    let mut startup = Startup::new(&resources);
    let choice = loop {
        if let Some(choice) = startup.update(&keys, &resources, &save_paths, get_time()) {
            break choice;
        }
        list.clear();
        startup.draw(
            &mut list,
            &layout,
            backend.palette(),
            &resources,
            get_time(),
        );
        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    };
    if choice.action == StartupAction::Quit {
        return;
    }

    let (mut game, mut map, mut fog) = if choice.action == StartupAction::Load {
        // Hydrate from the chosen slot. C pre-initializes a default game so a
        // failed load still leaves something playable.
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("Hero"), 0, 1, -1);
        let mut fog = Fog::new();
        match save_paths.slot(resources.pack_id.as_str(), choice.slot) {
            Some(path) => {
                if let Err(err) = save_read(&path, &mut game, &resources, &Map::new(1, 1), &mut fog)
                {
                    eprintln!("load slot {} failed: {err:?}", choice.slot);
                }
            }
            None => eprintln!("load slot {} has no path", choice.slot),
        }
        let zone = game.position.zone.as_str().to_owned();
        let mut map = match Map::load_zone_with_placements(&resources, &zone, &game) {
            Ok(map) => map,
            Err(err) => {
                eprintln!("could not load zone {zone}: {err}");
                return;
            }
        };
        map.apply_consumed(&game, &zone);
        (game, map, fog)
    } else {
        let player_class = resources
            .class_by_id(choice.class_id.as_str())
            .map(|class| class.index)
            .unwrap_or(0);
        let mut game = Game::default();
        game.init_seeded_with_resources(
            &resources,
            Some(choice.name.as_str()),
            player_class,
            choice.difficulty,
            seed_index,
        );
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
        (game, map, fog)
    };
    // C settles the hero on the loaded tile so the first frame is not a step.
    game.position.last_x = game.position.x;
    game.position.last_y = game.position.y;

    // Ctrl+Q arms a status-bar prompt rather than a dialog, which is why
    // chrome has a mode for it.
    let mut fast_quit = false;
    // A bridge cast has armed a direction prompt; the dialog owns the keys
    // until an arrow or ESC resolves it.
    let mut bridge_waiting = false;
    // The audience fanfare dialog chains into the king's words when it is
    // dismissed, so the second page is stashed here.
    let mut pending_audience_message: Option<String> = None;
    // A villain-siege win queues the "Capture" dialog after the victory banner.
    let mut pending_capture: Option<(String, String)> = None;
    // The modal layers: one prompt and one dialog at a time, mirroring C's
    // single-slot prompt and dialog globals.
    let mut prompt = Prompt::default();
    let mut dialog = Dialog::default();
    // C queues a post-init message: a new game is being created, or a loaded
    // one is being prepared. It follows the startup intro rather than
    // replacing it.
    {
        let name = game.character.name.as_str();
        let rank = game.character.cls.rank_title.as_str();
        let body = if choice.action == StartupAction::Load {
            format!(
                "{name} the {rank},\n\nPlease wait while I prepare a suitable environment for your bountying enjoyment!"
            )
        } else {
            format!(
                "{name} the {rank},\n\nA new game is being created. Please wait while I perform godlike actions to make this game playable."
            )
        };
        dialog.open("", &body);
    }
    let mut pending = PendingFlow::None;
    let mut pending_week: Option<WeekEnd> = None;
    let mut views = Views::new();
    // A running interactive battle; while Some, combat owns the screen and
    // the keys.
    let mut combat: Option<CombatSession> = None;

    loop {
        // Pop pending week-end screens (astrology -> budget) before input, as
        // C does, but only when nothing modal is up.
        if pending_week.is_some()
            && combat.is_none()
            && !dialog.is_active()
            && !prompt.is_active()
            && !views.is_active()
        {
            pump_week_end(&mut pending_week, &game, &resources, &mut dialog);
        }

        // The win cutscene yields to the win screen when it runs out of frames.
        if views.active() == Some(ViewKind::WinCartoon)
            && views.win_cartoon_done(&resources, get_time())
        {
            views.finish_win_cartoon();
        }

        hero_anim.update(get_time(), last_step, &game.stats.options, has_idle);

        // Where the pointer is, in logical coordinates, and what it is over.
        // It hit-tests the PREVIOUS frame's HitList (one frame of latency).
        let (win_w, win_h) = backend.window_size();
        let hovered = pointer
            .position()
            .and_then(|(px, py)| {
                present.window_to_logical(&layout, win_w, win_h, present::Target::Desktop, px, py)
            })
            .and_then(|(lx, ly)| hits.hit(lx, ly));
        let clicked = if pointer.pressed(Button::Left) {
            hovered
        } else {
            None
        };
        views.set_hover(hovered);

        // Input priority: combat first, then the message dialog owns the keys
        // while it is open (C's issue #19 gate), then the prompt, then the
        // fast-quit status
        // prompt, then the overworld.
        if let Some(session) = &mut combat {
            if views.is_active() {
                // A view is open over the battlefield; the read-only Army and
                // Character views dismiss on any key, pausing the fight.
                if keys.any_pressed() {
                    views.dismiss();
                }
            } else {
                let status =
                    session
                        .combat
                        .step(&keys, &mut game, &resources, get_time(), &mut prompt);
                match status {
                    CombatStatus::Running => {}
                    CombatStatus::OpenView(view) => views.set(view),
                    CombatStatus::Win | CombatStatus::Loss => {
                        let result = if status == CombatStatus::Win {
                            CombatResult::Win
                        } else {
                            CombatResult::Loss
                        };
                        let session = combat.take().expect("combat was Some");
                        if let Some((header, body)) = finish_combat(
                            &mut game,
                            &mut map,
                            &mut fog,
                            &resources,
                            session,
                            result,
                            &mut dialog,
                        ) {
                            pending_capture = Some((header, body));
                        }
                    }
                }
            }
        } else if dialog.is_active() {
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
            } else {
                // A click dismisses like any key. While a dialog is up only the
                // dialog drew regions, so `clicked` can only be its dismiss
                // target.
                let advanced = match clicked {
                    Some(target) => dialog.click(target),
                    None if keys.any_pressed() => dialog.advance(),
                    None => true,
                };
                if !advanced {
                    dialog.dismiss();
                    if let Some(king) = pending_audience_message.take() {
                        // The king's words render as the header (yellow), as C
                        // passes them through player_io_message(body, "").
                        dialog.open(&king, "");
                    } else if let Some((header, body)) = pending_capture.take() {
                        dialog.open(&header, &body);
                    }
                }
            }
        } else if prompt.is_active() {
            let answer = match clicked {
                Some(target) => prompt.click(target),
                None => prompt.update(&keys),
            };
            if answer != PromptResult::None {
                let combat_flow = dispatch_answer(
                    &mut game,
                    &mut map,
                    &mut fog,
                    &resources,
                    answer,
                    &mut pending,
                    &mut pending_week,
                    &mut prompt,
                    &mut dialog,
                    &mut views,
                );
                if let Some(flow) = combat_flow {
                    combat = Some(start_combat(&game, &resources, flow));
                }
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
            // A click resolves to the same confirmation as Enter.
            let confirmed = match clicked {
                Some(target) => views.gate_click(target),
                None => views.gate_update(&keys),
            };
            if confirmed {
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
            // runs the engine transaction and shows its info toast. A click
            // resolves to the same TownInput the keyboard produces.
            let town = match clicked {
                Some(target) => views.town_click(target),
                None => views.town_update(&keys),
            };
            match town {
                TownInput::RunRow(row) => run_town_row(&mut game, &resources, &mut views, row),
                TownInput::Dismissed | TownInput::Handled | TownInput::None => {}
            }
        } else if views.active() == Some(ViewKind::HomeCastle) {
            // Throne room / barracks: ESC pops, A opens the recruit screen, B
            // runs the audience-with-the-king flow.
            if keys.pressed(Key::Escape) {
                views.dismiss();
            } else if keys.pressed(Key::A) {
                views.recruit_soldiers_open(&resources, &game);
            } else if keys.pressed(Key::B) {
                let (outcome, needed) = game.audience_with_king(&resources);
                let (fanfare, king) =
                    interact::audience_messages(&resources, &game, outcome, needed);
                dialog.open("", &fanfare);
                pending_audience_message = Some(king);
            }
        } else if views.active() == Some(ViewKind::RecruitSoldiers) {
            // The recruit screen owns its letters, digits and Enter/ESC.
            if views.recruit_soldiers_update(&mut game, &resources, &keys) {
                views.dismiss();
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
        } else if views.active() == Some(ViewKind::WinCartoon) {
            // Any key skips the win cutscene; the frame clock's own end is
            // handled at the top of the loop.
            if keys.any_pressed() {
                views.finish_win_cartoon();
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
                    let (_weeks, paid) = spend_week(&mut game, &resources);
                    if paid > 0 {
                        schedule_week_end(&mut pending_week, &game, &resources, paid);
                    }
                    if game.stats.game_over {
                        show_lose_game(&mut views, &resources, &game);
                    }
                }
                Action::Rest => {
                    let advance = consume_step(&mut game, &resources, false);
                    if advance.week_ended {
                        schedule_week_end(
                            &mut pending_week,
                            &game,
                            &resources,
                            advance.commission_paid,
                        );
                    }
                    if game.stats.game_over {
                        show_lose_game(&mut views, &resources, &game);
                    }
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
                Action::OptionsMenu => views.set(ViewKind::Options),
                Action::SaveQuit => {
                    // C saves slot 0 unconditionally, then shows the confirm
                    // banner; Ctrl-Q during that dialog exits.
                    if let Some(path) = save_paths.slot(resources.pack_id.as_str(), 0) {
                        if let Err(err) = save_write(&path, &game, &resources, &map, &fog) {
                            eprintln!("save failed: {err:?}");
                        }
                    }
                    let body = banner::text(
                        &resources,
                        "banners",
                        "body_save_confirm",
                        DEFAULT_BODY_SAVE_CONFIRM,
                    );
                    dialog.open("", body);
                }
            }

            if input.is_move() {
                let result = game_step_basic(
                    &mut game, &mut map, &mut fog, &resources, input.dx, input.dy,
                );
                if let StepResult::Moved {
                    interaction,
                    advance,
                    ..
                } = &result
                {
                    // The clock the hero animates on: he walks for a moment
                    // after each step, then holds still.
                    last_step = Some(get_time());
                    if advance.week_ended {
                        schedule_week_end(
                            &mut pending_week,
                            &game,
                            &resources,
                            advance.commission_paid,
                        );
                    }
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
                            } else if game.stats.siege_weapons != 0 {
                                let owner_kind = game
                                    .castles
                                    .iter()
                                    .find(|c| c.id.as_str() == id)
                                    .map(|c| c.owner_kind)
                                    .unwrap_or(CastleOwnerKind::Monsters);
                                match owner_kind {
                                    CastleOwnerKind::Villain => {
                                        let (header, body) = interact::siege_villain_prompt(
                                            &resources,
                                            &game,
                                            id.as_str(),
                                        );
                                        prompt.yes_no(&header, &body);
                                        pending = PendingFlow::SiegeVillain {
                                            castle_id: id.clone(),
                                        };
                                    }
                                    _ => {
                                        let (header, body) =
                                            interact::siege_monster_prompt(&resources, id);
                                        prompt.yes_no(&header, &body);
                                        pending = PendingFlow::SiegeMonster {
                                            castle_id: id.clone(),
                                        };
                                    }
                                }
                            }
                            // Without siege weapons the hero stays on the gate;
                            // C bounces them back, but this engine never bounces.
                        }
                        StepInteraction::Foe { id, friendly } => {
                            let (foe_x, foe_y) = game.foes[..game.foe_count]
                                .iter()
                                .find(|foe| foe.placement_id.as_str() == id)
                                .map(|foe| (foe.x, foe.y))
                                .unwrap_or((0, 0));
                            if !friendly {
                                combat = Some(start_combat(
                                    &game,
                                    &resources,
                                    CombatFlow::HostileFoe {
                                        foe_id: id.clone(),
                                        foe_x,
                                        foe_y,
                                    },
                                ));
                            } else if let Some(offer) = game.begin_friendly_encounter(
                                &mut map,
                                &resources,
                                id.as_str(),
                                foe_x,
                                foe_y,
                            ) {
                                let body = interact::friendly_offer_prompt(&resources, &offer);
                                prompt.yes_no("", &body);
                                pending = PendingFlow::FriendlyOffer { offer };
                            } else {
                                dialog.open("", &interact::friendly_flees(&resources));
                            }
                        }
                        StepInteraction::Dwelling {
                            kind,
                            troop_id,
                            count,
                            ..
                        } => {
                            if troop_id.is_empty() {
                                dialog.open("", &interact::dwelling_empty(&resources));
                            } else if *count <= 0 {
                                dialog.open("", &interact::dwelling_none_this_week(&resources));
                            } else {
                                let troop_name = resources
                                    .troop_by_id(troop_id.as_str())
                                    .map(|t| t.name.as_str())
                                    .unwrap_or(troop_id.as_str())
                                    .to_owned();
                                let cost = resources
                                    .troop_by_id(troop_id.as_str())
                                    .map(|t| t.recruit_cost)
                                    .unwrap_or(0);
                                let troop_index = resources
                                    .troop_by_id(troop_id.as_str())
                                    .map(|t| t.index)
                                    .unwrap_or(-1);
                                let gold = game.stats.gold;
                                let cap = game.dwelling_recruit_cap(&resources, troop_id);
                                views.dwelling_open(
                                    *kind,
                                    troop_index,
                                    &troop_name,
                                    *count,
                                    cost,
                                    gold,
                                    cap,
                                );
                                let body = interact::dwelling_prompt_body(
                                    &resources,
                                    &troop_name,
                                    *count,
                                    cost,
                                    gold,
                                    cap,
                                );
                                prompt.text_input(&troop_name, &body, 4, cap);
                                pending = PendingFlow::Dwelling {
                                    troop_id: troop_id.clone(),
                                };
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
                if game.stats.game_over {
                    show_lose_game(&mut views, &resources, &game);
                }
            }
        }

        list.clear();
        hits.clear();
        if let Some(session) = &combat {
            // Combat owns the full inner area; the chrome shows its title.
            let title = session.combat.title(&resources);
            Chrome {
                layout: &layout,
                palette: backend.palette(),
                resources: &resources,
                game: &game,
                mode: StatusMode::Combat { title },
            }
            .draw(&mut list);
            combat_render(
                &mut list,
                &session.combat.combat,
                backend.palette(),
                &catalog,
            );
        } else {
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

            // A view covers the map area while the overworld keeps running
            // below. Each view derives its own animation clock from the wall
            // time, as C does with GetTime().
            views.draw(
                &mut list,
                &mut hits,
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
        }

        // The prompt replaces the bottom frame; the dialog draws LAST so it
        // covers a prompt that is up at the same time, exactly as C's overlay
        // orders them.
        if prompt.is_active() {
            prompt.draw(&mut list, &mut hits, &layout, backend.palette(), &resources);
        }
        if dialog.is_active() {
            dialog.draw(&mut list, &mut hits, &layout, backend.palette());
        }

        backend.execute(&list, &catalog, &layout, &present);
        next_frame().await;
    }
}
