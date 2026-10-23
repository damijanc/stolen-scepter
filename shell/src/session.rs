//! One running game: everything the main loop holds once the startup
//! wizard has chosen, and the frame that advances it.
//!
//! `main.rs` feeds it real keys and the real pointer and paints what it
//! draws; `bin/playtest.rs` feeds it a script and paints it to PNG files with
//! `raster.rs`. Both run exactly this code, so a playtest exercises the game
//! the player gets.

use crate::anim::HeroAnimator;
use crate::audio::{self, AudioOut};
use crate::banner;
use crate::cheats::{CheatMenu, CheatOutcome, CheatResult};
use crate::chrome::{Chrome, StatusMode};
use crate::combat::{combat_render, combat_view_input, CombatLoop, CombatStatus, PointerInput};
use crate::dialog::Dialog;
use crate::draw::{self, DrawList};
use crate::hud::Hud;
use crate::input::{self, Action, InputState, Key, KeySource, DIGITS};
use crate::interact;
use crate::layout::Layout;
use crate::map_render::MapScene;
use crate::menu::{self, Menu, MenuAction, PickerPurpose};
use crate::palette::Palette;
use crate::pointer::{HitList, HitTarget};
use crate::present::Present;
use crate::prompt::{Prompt, PromptResult};
use crate::savepath::{scan_slots, SavePaths};
use crate::slot_picker::{load_slot, SlotChoice, SlotPicker};
use crate::sprites::SpriteCatalog;
use crate::startup::{StartupAction, StartupChoice};
use crate::views::{
    controls_visible, worldmap_has_orb, HomeCastleAction, OwnCastleAction, SpellsInput, TownInput,
    TownRow, ViewKind, Views,
};
use crate::walk;
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
use openbounty_engine::savegame::save_write;
use openbounty_engine::spells_adventure::{
    spell_index_by_adventure_effect, try_build_bridge, AdventureCast, AdventureEffect, SpellCast,
};
use openbounty_engine::step::{
    consume_step, game_step_basic, search_here, spend_week, SearchOutcome, StepInteraction,
    StepResult,
};

/// One frame's input, already in logical coordinates.
pub struct FrameInput<'k> {
    pub keys: &'k dyn KeySource,
    /// The pointer in logical pixels, `None` over the letterbox.
    pub pointer_at: Option<(i32, i32)>,
    /// The left and right buttons went down this frame.
    pub left: bool,
    pub right: bool,
}

/// Whether the game asked to end.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameStatus {
    Running,
    Quit,
}

/// The pack-derived context a session draws with; it outlives the session.
#[derive(Clone, Copy)]
pub struct Context<'r> {
    pub resources: &'r Resources,
    pub layout: &'r Layout,
    pub catalog: &'r SpriteCatalog,
    pub palette: &'r Palette,
    pub save_paths: &'r SavePaths,
}

pub struct Session<'r> {
    ctx: Context<'r>,
    /// The display scale, which the Controls panel can change.
    pub present: Present,
    audio: Box<dyn AudioOut + 'r>,
    has_idle: bool,
    last_step: Option<f64>,
    hero_anim: HeroAnimator,
    /// Rebuilt every frame during draw and hit-tested on the NEXT frame's
    /// input; see specs/mouse-support-design.md.
    hits: HitList,
    pub game: Game,
    pub map: Map,
    pub fog: Fog,
    /// Ctrl+Q arms a status-bar prompt rather than a dialog, which is why
    /// chrome has a mode for it.
    fast_quit: bool,
    /// A bridge cast has armed a direction prompt; the dialog owns the keys
    /// until an arrow or ESC resolves it.
    bridge_waiting: bool,
    /// The audience fanfare dialog chains into the king's words when it is
    /// dismissed, so the second page is stashed here.
    pending_audience_message: Option<String>,
    /// A villain-siege win queues the "Capture" dialog after the victory banner.
    pending_capture: Option<(String, String)>,
    /// The modal layers: one prompt and one dialog at a time, mirroring C's
    /// single-slot prompt and dialog globals.
    prompt: Prompt,
    dialog: Dialog,
    pending: PendingFlow,
    pending_week: Option<WeekEnd>,
    views: Views,
    /// A running interactive battle; while Some, combat owns the screen and
    /// the keys.
    combat: Option<CombatSession>,
    /// The F10 debug menu, C's `s_cheat_menu_active`.
    cheat_menu: CheatMenu,
    /// The ESC Game Menu's cursor; on the view stack as `ViewKind::Menu`.
    menu: Menu,
    /// The menu's slot picker while Save or Load is choosing a slot; it draws
    /// in place of the menu, and ESC steps back to the menu.
    menu_picker: Option<(SlotPicker, PickerPurpose)>,
    /// Overworld click-to-move: the route being walked, if any. See walk.rs.
    walk: Option<walk::Walk>,
    /// An occupied slot awaiting the overwrite yes/no. Held apart from
    /// `PendingFlow`, which ports C's `pending_flow`; this answer needs the
    /// save paths, which `dispatch_answer` does not have.
    pending_overwrite: Option<usize>,
}

impl<'r> Session<'r> {
    /// Build the game the startup wizard chose: a fresh one, or a saved slot.
    /// `seed_index` is `--seed`, -1 for C's time-derived seed.
    pub fn start(
        ctx: Context<'r>,
        present: Present,
        mut audio: Box<dyn AudioOut + 'r>,
        choice: &StartupChoice,
        seed_index: i32,
    ) -> Result<Self, String> {
        let Context {
            resources,
            catalog,
            save_paths,
            ..
        } = ctx;
        let has_idle = catalog.hero(draw::HeroAnim::Idle).is_present();
        let (mut game, map, fog) = if choice.action == StartupAction::Load {
            // Hydrate from the chosen slot. C pre-initializes a default game so a
            // failed load still leaves something playable; the recipe is shared
            // with the in-game menu, and only this fallback is startup's own.
            match load_slot(
                &save_paths,
                &resources,
                resources.pack_id.as_str(),
                choice.slot,
            ) {
                Ok(loaded) => loaded,
                Err(err) => {
                    eprintln!("load slot {} failed: {err:?}", choice.slot);
                    let mut game = Game::default();
                    game.init_seeded_with_resources(&resources, Some("Hero"), 0, 1, -1);
                    let zone = game.position.zone.as_str().to_owned();
                    let mut map = match Map::load_zone_with_placements(&resources, &zone, &game) {
                        Ok(map) => map,
                        Err(err) => {
                            return Err(format!("could not load zone {zone}: {err}"));
                        }
                    };
                    map.apply_consumed(&game, &zone);
                    (game, map, Fog::new())
                }
            }
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
                    return Err(format!("could not load zone {zone}: {err}"));
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
        let fast_quit = false;
        // A bridge cast has armed a direction prompt; the dialog owns the keys
        // until an arrow or ESC resolves it.
        let bridge_waiting = false;
        // The audience fanfare dialog chains into the king's words when it is
        // dismissed, so the second page is stashed here.
        let pending_audience_message: Option<String> = None;
        // A villain-siege win queues the "Capture" dialog after the victory banner.
        let pending_capture: Option<(String, String)> = None;
        // The modal layers: one prompt and one dialog at a time, mirroring C's
        // single-slot prompt and dialog globals.
        let prompt = Prompt::default();
        let mut dialog = Dialog::default();
        // C queues a post-init message (a new game set up, or a loaded one
        // restored). It follows the startup intro rather than
        // replacing it.
        dialog.open(
            "",
            &post_init_message(&game, choice.action == StartupAction::Load),
        );
        let pending = PendingFlow::None;
        let pending_week: Option<WeekEnd> = None;
        let mut views = Views::new();
        // A running interactive battle; while Some, combat owns the screen and
        // the keys.
        let combat: Option<CombatSession> = None;
        // The F10 debug menu, C's `s_cheat_menu_active`.
        let cheat_menu = CheatMenu::default();
        // Open the device and load the pack's audio, porting `main.c`'s
        // `audio_init` block. A machine with no device plays nothing and grays
        // the audio rows. Files come through the pack, as C's `LoadAssetBytes`
        // does, so a `.openbounty` archive plays as a directory does.
        views.set_audio_available(audio.is_available());
        if !audio.is_available() {
            audio::pin_options_without_device(&mut game.stats.options);
        }
        push_audio_options(audio.as_mut(), &game);
        audio.set_track(audio::Track::Openworld);
        // The ESC Game Menu's cursor; on the view stack as `ViewKind::Menu`.
        let menu = Menu::new();
        // The menu's slot picker while Save or Load is choosing a slot; it draws
        // in place of the menu, and ESC steps back to the menu.
        let menu_picker: Option<(SlotPicker, PickerPurpose)> = None;
        // Overworld click-to-move: the route being walked, if any. See walk.rs.
        let walk: Option<walk::Walk> = None;
        // An occupied slot awaiting the overwrite yes/no. Held apart from
        // `PendingFlow`, which ports C's `pending_flow`; this answer needs the
        // save paths, which `dispatch_answer` does not have.
        let pending_overwrite: Option<usize> = None;
        Ok(Self {
            ctx,
            present,
            audio,
            has_idle,
            last_step: None,
            hero_anim: HeroAnimator::new(),
            hits: HitList::new(),
            game,
            map,
            fog,
            fast_quit,
            bridge_waiting,
            pending_audience_message,
            pending_capture,
            prompt,
            dialog,
            pending,
            pending_week,
            views,
            combat,
            cheat_menu,
            menu,
            menu_picker,
            walk,
            pending_overwrite,
        })
    }

    /// Advance one frame on `input` at wall-clock `now` (seconds) and draw it
    /// into `list`. The caller adds the pointer and paints the list.
    pub fn frame(&mut self, frame_in: &FrameInput, now: f64, list: &mut DrawList) -> FrameStatus {
        let Context {
            resources,
            layout,
            catalog,
            palette,
            save_paths,
        } = self.ctx;
        let keys = frame_in.keys;
        let has_idle = self.has_idle;
        let Self {
            present,
            audio,
            last_step,
            hero_anim,
            hits,
            game,
            map,
            fog,
            fast_quit,
            bridge_waiting,
            pending_audience_message,
            pending_capture,
            prompt,
            dialog,
            pending,
            pending_week,
            views,
            combat,
            cheat_menu,
            menu,
            menu_picker,
            walk,
            pending_overwrite,
            ..
        } = self;
        // C pushes the toggles every frame because the Controls panel changes
        // them under the player's hands (`main.c:955-958`). Its combat loop
        // only ticks, so a change made mid-battle lands when the battle ends.
        if combat.is_none() {
            push_audio_options(audio.as_mut(), game);
        }
        audio.tick();

        // F10 -> debug cheat menu, first thing in the frame as in `main.c`.
        // C cannot reach it during combat or the win cartoon because both run
        // their own loops there; this shell runs them in this one, so they are
        // excluded here instead.
        let cheat = if combat.is_none() && views.active() != Some(ViewKind::WinCartoon) {
            cheat_menu.tick(keys, game, resources, map, fog, dialog)
        } else {
            CheatResult::Idle
        };
        match &cheat {
            CheatResult::Terminal(CheatOutcome::Win) => show_win_game(views, resources, game),
            CheatResult::Terminal(CheatOutcome::Lose) => show_lose_game(views, resources, game),
            _ => {}
        }
        // The menu owns the keys while it is open and on the frame it handled
        // one; see the DIVERGENCE note in `cheats.rs`.
        let cheat_owns_frame = cheat != CheatResult::Idle || cheat_menu.is_active();

        // Pop pending week-end screens (astrology -> budget) before input, as
        // C does, but only when nothing modal is up.
        if pending_week.is_some()
            && combat.is_none()
            && !dialog.is_active()
            && !prompt.is_active()
            && !views.is_active()
        {
            pump_week_end(pending_week, game, resources, dialog);
        }

        // The win cutscene yields to the win screen when it runs out of frames.
        if views.active() == Some(ViewKind::WinCartoon) && views.win_cartoon_done(resources, now) {
            views.finish_win_cartoon();
        }

        hero_anim.update(now, *last_step, &game.stats.options, has_idle);

        // Where the pointer is, in logical coordinates, and what it is over.
        // It hit-tests the PREVIOUS frame's HitList (one frame of latency).
        let pointer_at = frame_in.pointer_at;
        let hovered = pointer_at.and_then(|(lx, ly)| hits.hit(lx, ly));
        let clicked = if frame_in.left { hovered } else { None };
        views.set_hover(hovered);
        menu.set_hover(hovered);
        if let Some(session) = combat.as_mut() {
            session.combat.set_hover(hovered);
        }
        if let Some((picker, _)) = menu_picker.as_mut() {
            picker.set_hover(hovered);
        }

        // Input priority: combat first, then the message dialog owns the keys
        // while it is open (C's issue #19 gate), then the prompt, then the
        // fast-quit status
        // prompt, then the overworld.
        if cheat_owns_frame {
            // Nothing else reads input this frame.
        } else if let Some(session) = combat.as_mut() {
            if views.is_active() {
                // A view is open over the battlefield and the fight is paused:
                // no step, so no AI and no animation tick. ESC closes it, C/O
                // swap Options and Controls, digits cycle Controls rows.
                combat_view_input(views, keys, clicked, game, resources, layout, present);
            } else {
                // A click reaches the fight only through `step_input`,
                // behind the same gates as the keys; right-click is ESC.
                let input = PointerInput {
                    click: clicked,
                    cancel: frame_in.right,
                };
                let status = session
                    .combat
                    .step_input(keys, input, game, resources, now, prompt);
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
                        // `combat_loop.c:696` / `705`: a loss plays the
                        // defeat tune, and either way the openworld returns.
                        if result == CombatResult::Loss {
                            audio.play_tune(audio::Tune::Defeat);
                        }
                        audio.set_track(audio::Track::Openworld);
                        if let Some((header, body)) =
                            finish_combat(game, map, fog, resources, session, result, dialog)
                        {
                            *pending_capture = Some((header, body));
                        }
                    }
                }
            }
        } else if dialog.is_active() {
            let ctrl = keys.down(Key::LeftControl) || keys.down(Key::RightControl);
            if ctrl && keys.pressed(Key::Q) {
                return FrameStatus::Quit;
            }
            if *bridge_waiting {
                // The bridge prompt owns the dialog: an arrow builds toward
                // that direction, ESC cancels. Nothing else dismisses.
                let dir = input::poll(keys);
                if dir.dx != 0 || dir.dy != 0 {
                    *bridge_waiting = false;
                    let built = try_build_bridge(game, map, dir.dx, dir.dy);
                    if built > 0 {
                        let bridge =
                            spell_index_by_adventure_effect(resources, AdventureEffect::Bridge);
                        if bridge >= 0 {
                            game.spells.counts[bridge as usize] -= 1;
                        }
                        let (header, body) = interact::bridge_built(resources, built);
                        dialog.open(&header, &body);
                    } else {
                        let (header, body) = interact::bridge_invalid(resources);
                        dialog.open(&header, &body);
                    }
                } else if keys.pressed(Key::Escape) {
                    *bridge_waiting = false;
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
                None => prompt.update(keys),
            };
            if let Some(slot) = pending_overwrite.filter(|_| answer != PromptResult::None) {
                // The Game Menu's overwrite confirm. Yes writes; any other
                // answer leaves the picker open to choose again.
                *pending_overwrite = None;
                if answer == PromptResult::Yes {
                    menu_save(
                        save_paths,
                        resources,
                        game,
                        map,
                        fog,
                        slot,
                        menu_picker,
                        views,
                        dialog,
                    );
                }
            } else if answer != PromptResult::None {
                let combat_flow = dispatch_answer(
                    game,
                    map,
                    fog,
                    resources,
                    answer,
                    pending,
                    pending_week,
                    prompt,
                    dialog,
                    views,
                );
                if let Some(flow) = combat_flow {
                    *combat = Some(start_combat(game, resources, flow));
                    audio.set_track(audio::Track::Combat);
                }
            }
        } else if views.active() == Some(ViewKind::Controls) {
            // Navigate rows with Up/Down; Enter/Space (or a digit) advances
            // the value; ESC or C closes. The Scale row is appended last,
            // modern only.
            let vis = controls_visible(resources);
            let count = vis.len() + usize::from(layout.is_modern);
            let scale_row = if layout.is_modern {
                vis.len()
            } else {
                usize::MAX
            };
            if count > 0 {
                let cur = views.controls_cursor() as usize;
                // A click selects (and advances) a row, exactly as its digit
                // key does.
                let click_row = clicked
                    .and_then(|target| views.controls_click(target))
                    .filter(|row| *row < count);
                if let Some(row) = click_row {
                    views.controls_set_cursor(row as i32);
                    if row == scale_row {
                        present.advance_scale();
                    } else {
                        views.controls_advance(game, resources, vis[row]);
                    }
                } else if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
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
                        views.controls_advance(game, resources, vis[cur]);
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
                                views.controls_advance(game, resources, vis[k]);
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
            // ESC cancels. Any other key (or a click that names no spell)
            // dismisses the read-only book.
            let input = match clicked {
                Some(target) => views.spells_click(target),
                None => views.spells_update(keys),
            };
            match input {
                SpellsInput::Cast(spell_idx) => {
                    dispatch_spell(
                        game,
                        resources,
                        spell_idx,
                        prompt,
                        dialog,
                        pending,
                        bridge_waiting,
                        views,
                    );
                }
                SpellsInput::Handled => {}
                SpellsInput::None => {
                    if keys.any_pressed() || clicked.is_some() {
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
                None => views.gate_update(keys),
            };
            if confirmed {
                let chosen = views.gate_take_chosen();
                if let Some(index) = chosen {
                    let destination = views.gate_dest(index).copied();
                    let is_town = views.gate_is_town();
                    if let Some(destination) = destination {
                        let spell_id = if is_town { "town_gate" } else { "castle_gate" };
                        game.gate_teleport(resources, map, fog, &destination, spell_id);
                    }
                }
            }
        } else if views.active() == Some(ViewKind::Town) {
            // The town menu owns its arrows/letter/Enter input; executing a row
            // runs the engine transaction and shows its info toast. A click
            // resolves to the same TownInput the keyboard produces.
            let town = match clicked {
                Some(target) => views.town_click(target),
                None => views.town_update(keys),
            };
            match town {
                TownInput::RunRow(row) => run_town_row(game, resources, views, row),
                TownInput::Dismissed | TownInput::Handled | TownInput::None => {}
            }
        } else if views.active() == Some(ViewKind::HomeCastle) {
            // Throne room / barracks: ESC pops, A opens the recruit screen, B
            // runs the audience-with-the-king flow. A click on a menu line
            // resolves to the same action as its key.
            let action = match clicked {
                Some(target) => views.home_castle_click(target),
                None => views.home_castle_key(keys),
            };
            match action {
                HomeCastleAction::Dismiss => views.dismiss(),
                HomeCastleAction::Recruit => views.recruit_soldiers_open(resources, game),
                HomeCastleAction::Audience => {
                    let (outcome, needed) = game.audience_with_king(resources);
                    let (fanfare, king) =
                        interact::audience_messages(resources, game, outcome, needed);
                    dialog.open("", &fanfare);
                    *pending_audience_message = Some(king);
                }
                HomeCastleAction::None => {}
            }
        } else if views.active() == Some(ViewKind::RecruitSoldiers) {
            // The recruit screen owns its letters, digits and Enter/ESC. A
            // click selects a troop row like its letter.
            let close = match clicked {
                Some(target) => views.recruit_click(game, resources, target),
                None => views.recruit_soldiers_update(game, resources, keys),
            };
            if close {
                views.dismiss();
            }
        } else if views.active() == Some(ViewKind::OwnCastle) {
            // SPACE toggles garrison/remove; A..E transfer the chosen stack. A
            // click on a slot resolves to the same transfer.
            let action = match clicked {
                Some(target) => views.own_castle_click(target),
                None => views.own_castle_key(keys),
            };
            match action {
                OwnCastleAction::Dismiss => views.dismiss(),
                OwnCastleAction::Toggle => views.own_castle_toggle_mode(),
                OwnCastleAction::Slot(slot) => {
                    let castle_id = views.own_castle_castle_id().to_owned();
                    let garrison_mode = views.own_castle_garrison_mode();
                    let outcome = if garrison_mode {
                        game.garrison_troop(&castle_id, slot)
                    } else {
                        game.ungarrison_troop(&castle_id, slot)
                    };
                    if outcome != GarrisonOutcome::Ok {
                        dialog.open("", interact::garrison_refusal(resources, outcome));
                    }
                }
                OwnCastleAction::None => {}
            }
        } else if views.active() == Some(ViewKind::Menu) {
            // The Game Menu owns its arrows, digits, Enter and ESC. It is only
            // ever opened from the overworld branch below, so combat never
            // reaches it: the save format carries no combat state.
            // A click resolves to the same value as the matching key.
            if let Some((picker, purpose)) = menu_picker.as_mut() {
                let purpose = *purpose;
                let choice = match clicked {
                    Some(target) => picker.click(target),
                    None => picker.update(keys),
                };
                match choice {
                    None | Some(SlotChoice::Extra) => {}
                    Some(SlotChoice::Cancel) => *menu_picker = None,
                    Some(SlotChoice::Slot(slot)) => {
                        let header = picker.slots().headers[slot];
                        match (purpose, header) {
                            (PickerPurpose::Save, Some(header)) => {
                                let (title, body) =
                                    menu::overwrite_prompt(resources, slot, header.name.as_str());
                                prompt.yes_no(&title, &body);
                                *pending_overwrite = Some(slot);
                            }
                            (PickerPurpose::Save, None) => menu_save(
                                save_paths,
                                resources,
                                game,
                                map,
                                fog,
                                slot,
                                menu_picker,
                                views,
                                dialog,
                            ),
                            (PickerPurpose::Load, None) => {
                                let (title, body) = menu::empty_slot_message(resources);
                                dialog.open(&title, &body);
                            }
                            (PickerPurpose::Load, Some(_)) => {
                                match load_slot(
                                    save_paths,
                                    resources,
                                    resources.pack_id.as_str(),
                                    slot,
                                ) {
                                    Ok((new_game, new_map, new_fog)) => {
                                        // Swap only on success: a failed load
                                        // must not disturb play.
                                        *game = new_game;
                                        *map = new_map;
                                        *fog = new_fog;
                                        // Settle the hero on the loaded tile,
                                        // as startup does.
                                        game.position.last_x = game.position.x;
                                        game.position.last_y = game.position.y;
                                        // Everything else main holds was
                                        // derived from the old session. Reset
                                        // it all: a stale week phase would
                                        // fire the old game's week-end screen.
                                        *last_step = None;
                                        *hero_anim = HeroAnimator::new();
                                        *fast_quit = false;
                                        *bridge_waiting = false;
                                        *pending_audience_message = None;
                                        *pending_capture = None;
                                        *prompt = Prompt::default();
                                        *dialog = Dialog::default();
                                        *pending = PendingFlow::None;
                                        *pending_week = None;
                                        *views = Views::new();
                                        // The audio device stays open; what
                                        // it derived from the old game is
                                        // re-applied, as startup does: the
                                        // grayed rows, the no-device pin of
                                        // the loaded save's options, and the
                                        // options themselves.
                                        views.set_audio_available(audio.is_available());
                                        if !audio.is_available() {
                                            audio::pin_options_without_device(
                                                &mut game.stats.options,
                                            );
                                        }
                                        push_audio_options(audio.as_mut(), game);
                                        *combat = None;
                                        *cheat_menu = CheatMenu::default();
                                        *menu = Menu::new();
                                        *menu_picker = None;
                                        *walk = None;
                                        *pending_overwrite = None;
                                        dialog.open("", &post_init_message(game, true));
                                    }
                                    Err(err) => {
                                        eprintln!("load slot {slot} failed: {err:?}");
                                        *menu_picker = None;
                                        let (title, body) =
                                            menu::load_failed_message(resources, &err);
                                        dialog.open(&title, &body);
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                let action = match clicked {
                    Some(target) => menu.click(target),
                    None => menu.update(keys),
                };
                match action {
                    Some(MenuAction::Close) => views.dismiss(),
                    Some(MenuAction::Save) => {
                        let slots = scan_slots(save_paths, resources.pack_id.as_str());
                        *menu_picker = Some((SlotPicker::new(slots, false), PickerPurpose::Save));
                    }
                    Some(MenuAction::Load) => {
                        let slots = scan_slots(save_paths, resources.pack_id.as_str());
                        *menu_picker = Some((SlotPicker::new(slots, false), PickerPurpose::Load));
                    }
                    Some(MenuAction::Quit) => {
                        // Reuse the ported Ctrl+Q confirm; no second one.
                        views.dismiss();
                        *fast_quit = true;
                    }
                    None => {}
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
                if worldmap_has_orb(game, resources) {
                    views.toggle_whole_map();
                } else {
                    views.dismiss();
                }
            } else if keys.any_pressed() || clicked == Some(HitTarget::ViewDismiss) {
                views.dismiss();
            }
        } else if *fast_quit {
            // The prompt is modal: nothing else reads input until it is
            // answered.
            if keys.pressed(Key::Y) {
                return FrameStatus::Quit;
            }
            if keys.pressed(Key::N) || keys.pressed(Key::Escape) {
                *fast_quit = false;
            }
        } else if game.is_over() {
            if game_over_quits(game, keys) {
                return FrameStatus::Quit;
            }
        } else {
            let mut input = input::poll(keys);
            // Click-to-move (walk.rs). Any key or click ends a walk; a map
            // click may start a new one. A walked step is fed in as the same
            // move value an arrow key produces, never as a fake key.
            // Read the buttons, not `clicked`: a click on the sidebar, which
            // publishes no region, must stop the hero too.
            if keys.any_pressed() || frame_in.left || frame_in.right {
                *walk = None;
            }
            if let Some(HitTarget::MapTile(x, y)) = clicked {
                match walk::click(game, map, fog, (x, y), now) {
                    walk::ClickMove::Step(dx, dy) => {
                        input.dx = dx;
                        input.dy = dy;
                    }
                    walk::ClickMove::Route(route) => *walk = Some(route),
                    walk::ClickMove::Nothing => {}
                }
            }
            if input == InputState::default() {
                if let Some(route) = walk.as_mut() {
                    match route.next(game, map, fog, now) {
                        walk::WalkTick::Step(dx, dy) => {
                            input.dx = dx;
                            input.dy = dy;
                        }
                        walk::WalkTick::Wait => {}
                        walk::WalkTick::Done => *walk = None,
                    }
                }
            }
            match input.action {
                // Legality lives in the engine, and both are silent on
                // failure exactly as C is.
                Action::Fly => {
                    game.mount_fly(resources);
                }
                Action::Land => {
                    game.land_here(map);
                }
                Action::EndWeek => {
                    let (_weeks, paid) = spend_week(game, resources);
                    if paid > 0 {
                        schedule_week_end(pending_week, game, resources, paid);
                    }
                    if game.stats.game_over {
                        show_lose_game(views, resources, game);
                    }
                }
                Action::Rest => {
                    let absorbed = step_absorbed(game);
                    let advance = consume_step(game, resources, false);
                    if let Some(tune) = audio::on_step_tune(absorbed) {
                        audio.play_tune(tune);
                    }
                    if advance.week_ended {
                        schedule_week_end(pending_week, game, resources, advance.commission_paid);
                    }
                    if game.stats.game_over {
                        show_lose_game(views, resources, game);
                    }
                }
                Action::FastQuit => *fast_quit = true,
                Action::Search => {
                    let days = resources.tuning.search_cost_days.to_string();
                    let body = banner::formatted(
                        resources,
                        "banners",
                        "body_search",
                        DEFAULT_BODY_SEARCH,
                        &[("DAYS", &days)],
                    );
                    let header =
                        banner::text(resources, "dialog_titles", "search", DEFAULT_DT_SEARCH);
                    prompt.yes_no(header, &body);
                    *pending = PendingFlow::Search;
                }
                Action::DismissArmy => {
                    let occupied = (0..GAME_ARMY_SLOTS)
                        .filter(|&i| !game.army[i].id.is_empty() && game.army[i].count > 0)
                        .count();
                    if occupied > 0 {
                        let body = banner::text(
                            resources,
                            "banners",
                            "body_dismiss_pick",
                            DEFAULT_BODY_DISMISS_PICK,
                        );
                        let header = banner::text(
                            resources,
                            "dialog_titles",
                            "dismiss_army",
                            DEFAULT_DT_DISMISS_ARMY,
                        );
                        prompt.numeric(header, body, GAME_ARMY_SLOTS as i32);
                        *pending = PendingFlow::DismissArmy;
                    }
                }
                Action::NewContinent => {
                    if game.travel_mode != TravelMode::Boat {
                        let (header, body) = interact::must_be_sailing(resources);
                        dialog.open(&header, &body);
                    } else {
                        let targets = interact::navigate_targets(resources, game);
                        if targets.is_empty() {
                            let (header, body) = interact::no_continents(resources);
                            dialog.open(&header, &body);
                        } else {
                            let (header, body) = interact::navigate_body(resources, &targets);
                            prompt.numeric(&header, &body, targets.len() as i32);
                            *pending = PendingFlow::Navigate {
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
                        let (header, body) = interact::no_spell_banner(resources);
                        dialog.open(&header, &body);
                    } else {
                        views.set(ViewKind::Spells);
                        views.spells_set_cast_mode(true);
                    }
                }
                Action::None => {}
                Action::OptionsMenu => views.set(ViewKind::Options),
                // Bare ESC. Every other ESC consumer -- a view, the dialog,
                // the prompt, the quit confirm -- sits earlier in this chain,
                // so reaching here means nothing was open to dismiss.
                Action::GameMenu => {
                    *menu = Menu::new();
                    views.set(ViewKind::Menu);
                }
                Action::SaveQuit => {
                    // C saves slot 0 unconditionally, then shows the confirm
                    // banner; Ctrl-Q during that dialog exits.
                    if let Some(path) = save_paths.slot(resources.pack_id.as_str(), 0) {
                        if let Err(err) = save_write(&path, game, resources, map, fog) {
                            eprintln!("save failed: {err:?}");
                        }
                    }
                    let body = banner::text(
                        resources,
                        "banners",
                        "body_save_confirm",
                        DEFAULT_BODY_SAVE_CONFIRM,
                    );
                    dialog.open("", body);
                }
            }

            if input.is_move() {
                let absorbed = step_absorbed(game);
                let result = game_step_basic(game, map, fog, resources, input.dx, input.dy);
                if walk::stops_walk(&result) {
                    *walk = None;
                }
                for tune in audio::tunes_for_step(&result, absorbed)
                    .into_iter()
                    .flatten()
                {
                    audio.play_tune(tune);
                }
                if let StepResult::Moved {
                    interaction,
                    advance,
                    ..
                } = &result
                {
                    // The clock the hero animates on: he walks for a moment
                    // after each step, then holds still.
                    *last_step = Some(now);
                    if advance.week_ended {
                        schedule_week_end(pending_week, game, resources, advance.commission_paid);
                    }
                    match interaction {
                        StepInteraction::Sign { title, body, .. } => dialog.open(title, body),
                        StepInteraction::TreasureChest { roll, .. } => {
                            if let Some(body) = interact::chest_choice(resources, roll) {
                                prompt.ab("", &body);
                                *pending = PendingFlow::ChestChoice {
                                    gold: roll.pending.pending_gold,
                                    leadership: roll.pending.pending_leadership,
                                };
                            } else if let Some((header, body)) =
                                interact::chest_message(resources, roll)
                            {
                                dialog.open(&header, &body);
                            }
                        }
                        StepInteraction::Alcove => {
                            if game.stats.knows_magic {
                                let (header, body) = interact::alcove_already(resources);
                                dialog.open(&header, &body);
                            } else {
                                let (header, body) = interact::alcove_offer(resources);
                                prompt.yes_no(&header, &body);
                                *pending = PendingFlow::Alcove;
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
                                            resources,
                                            game,
                                            id.as_str(),
                                        );
                                        prompt.yes_no(&header, &body);
                                        *pending = PendingFlow::SiegeVillain {
                                            castle_id: id.clone(),
                                        };
                                    }
                                    _ => {
                                        let (header, body) =
                                            interact::siege_monster_prompt(resources, id);
                                        prompt.yes_no(&header, &body);
                                        *pending = PendingFlow::SiegeMonster {
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
                                *combat = Some(start_combat(
                                    game,
                                    resources,
                                    CombatFlow::HostileFoe {
                                        foe_id: id.clone(),
                                        foe_x,
                                        foe_y,
                                    },
                                ));
                                audio.set_track(audio::Track::Combat);
                            } else if let Some(offer) = game.begin_friendly_encounter(
                                map,
                                resources,
                                id.as_str(),
                                foe_x,
                                foe_y,
                            ) {
                                let body = interact::friendly_offer_prompt(resources, &offer);
                                prompt.yes_no("", &body);
                                *pending = PendingFlow::FriendlyOffer { offer };
                            } else {
                                dialog.open("", &interact::friendly_flees(resources));
                            }
                        }
                        StepInteraction::Dwelling {
                            kind,
                            troop_id,
                            count,
                            ..
                        } => {
                            if troop_id.is_empty() {
                                dialog.open("", &interact::dwelling_empty(resources));
                            } else if *count <= 0 {
                                dialog.open("", &interact::dwelling_none_this_week(resources));
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
                                let cap = game.dwelling_recruit_cap(resources, troop_id);
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
                                    resources,
                                    &troop_name,
                                    *count,
                                    cost,
                                    gold,
                                    cap,
                                );
                                prompt.text_input(&troop_name, &body, 4, cap);
                                *pending = PendingFlow::Dwelling {
                                    troop_id: troop_id.clone(),
                                };
                            }
                        }
                        StepInteraction::None => {}
                        other => {
                            if let Some((header, body)) = interact::pickup_message(resources, other)
                            {
                                dialog.open(&header, &body);
                            } else {
                                println!("{other:?}");
                            }
                        }
                    }
                }
                if game.stats.game_over {
                    show_lose_game(views, resources, game);
                }
            }
        }

        // A walk never runs under anything modal, nor past the game's end.
        if dialog.is_active()
            || prompt.is_active()
            || views.is_active()
            || combat.is_some()
            || *fast_quit
            || game.is_over()
        {
            *walk = None;
        }

        list.clear();
        hits.clear();
        if let Some(session) = combat.as_ref() {
            // Combat owns the full inner area; the chrome shows its title.
            let title = session.combat.title(resources);
            Chrome {
                layout: layout,
                palette: palette,
                resources: resources,
                game: game,
                mode: StatusMode::Combat { title },
            }
            .draw(list);
            combat_render(
                list,
                hits,
                &session.combat.combat,
                session.combat.pick_cursor(),
                palette,
                catalog,
            );
            // An open view draws over the still-visible field, porting
            // combat_present (combat_loop.c:321-327). C passes NULL map and
            // fog because no view combat can open reads them; the overworld's
            // are passed only to satisfy the signature.
            views.draw(
                list, hits, layout, palette, resources, catalog, map, fog, game, present, now,
            );
            // The spell-pick menu (and the mouse's action menu) draw after any
            // view and before the dialog and prompt, as in combat_present
            // (combat_loop.c:328).
            session
                .combat
                .draw_menus(list, hits, palette, resources, game);
        } else {
            Chrome {
                layout: layout,
                palette: palette,
                resources: resources,
                game: game,
                mode: if *fast_quit {
                    StatusMode::FastQuit
                } else if views.active() == Some(ViewKind::Worldmap) {
                    StatusMode::Worldmap {
                        whole_map: views.whole_map(),
                    }
                } else if views.wants_exit_hint(game) {
                    StatusMode::ExitHint
                } else {
                    StatusMode::Adventure
                },
            }
            .draw(list);
            let scene = MapScene {
                layout: layout,
                catalog: catalog,
                palette: palette,
                resources: resources,
                game: game,
                map: map,
                fog: fog,
                tick: hero_anim.tick(),
                moving: hero_anim.moving(),
            };
            scene.draw(list);
            scene.publish_hits(hits);
            // The sidebar panels, drawn over the map and under the views --
            // `shell_frame.c`'s chrome -> map -> hud -> overlay order. A view
            // that covers the sidebar must paint over these.
            Hud {
                layout: layout,
                palette: palette,
                catalog: catalog,
                resources: resources,
                game: game,
                time: now,
            }
            .draw(list);

            // A view covers the map area while the overworld keeps running
            // below. Each view derives its own animation clock from the wall
            // time, as C does with GetTime().
            views.draw(
                list, hits, layout, palette, resources, catalog, map, fog, game, present, now,
            );
            if views.active() == Some(ViewKind::Menu) {
                match &*menu_picker {
                    Some((picker, purpose)) => {
                        menu::draw_picker(picker, list, hits, layout, palette, resources, *purpose)
                    }
                    None => menu.draw(list, hits, layout, palette, resources),
                }
            }
        }

        // The prompt replaces the bottom frame; the dialog draws LAST so it
        // covers a prompt that is up at the same time, exactly as C's overlay
        // orders them.
        if prompt.is_active() {
            prompt.draw(list, hits, layout, palette, resources);
        }
        if dialog.is_active() {
            dialog.draw(list, hits, layout, palette);
        }

        FrameStatus::Running
    }
}

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
const DEFAULT_BODY_SEARCH: &str = "Searching here takes\n%DAYS% days.\nSearch?";
const DEFAULT_DT_DISMISS_ARMY: &str = "Dismiss Army";
const DEFAULT_BODY_DISMISS_PICK: &str = "Dismiss which troop?";
const DEFAULT_DT_DISMISS_LAST: &str = "Dismiss last army";
const DEFAULT_BODY_DISMISS_LAST: &str = "Dismissing your last army\n\
    sends you home to the\n\
    crown in disgrace.\n\
    Dismiss it?";
const DEFAULT_TEMP_DEATH: &str = "After being disgraced on the\n\
    field of battle, King\n\
    Maximus summons you to his\n\
    castle. After a lesson in\n\
    tactics, he reluctantly re-\n\
    issues your commission and\n\
    sends you on your way.";
const DEFAULT_BODY_SAVE_CONFIRM: &str = "Ctrl-Q quits; any other\nkey returns to the game.";

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
pub(crate) fn show_win_game(views: &mut Views, resources: &Resources, game: &mut Game) {
    game.stats.game_over = true;
    let body = interact::end_message(resources, game, true);
    views.win_cartoon_open(resources, &body);
}

/// Ports `show_lose_game`: open the ending screen. `game_over` is already set
/// by whatever ran out of days.
pub(crate) fn show_lose_game(views: &mut Views, resources: &Resources, game: &Game) {
    let body = interact::end_message(resources, game, false);
    views.open_end_game(false, &body);
}

/// Whether `GameOnStep` will return before playing the walk tune: a banked
/// time stop or a finished game. Sampled before the step, which spends the
/// time stop.
fn step_absorbed(game: &Game) -> bool {
    game.stats.game_over || game.stats.time_stop > 0
}

/// Hand the three audio options to the backend, `main.c:904-906`.
fn push_audio_options(audio: &mut dyn AudioOut, game: &Game) {
    let options = &game.stats.options;
    audio.set_sounds_enabled(options[audio::OPTION_SOUNDS] != 0);
    audio.set_music_enabled(options[audio::OPTION_MUSIC] != 0);
    audio.set_master_volume(options[audio::OPTION_VOLUME]);
}

/// C's post-init message, in our own words: a new game is being set up, or
/// a loaded one restored. The in-game menu shows the loaded form after a load too.
fn post_init_message(game: &Game, loaded: bool) -> String {
    let name = game.character.name.as_str();
    let rank = game.character.cls.rank_title.as_str();
    if loaded {
        format!("{name} the {rank},\n\nYour saved realm is being restored. One moment.")
    } else {
        format!(
            "{name} the {rank},\n\nYour realm is being prepared. One moment while the world takes shape."
        )
    }
}

/// Write the live game to `slot` from the Game Menu and report it. A success
/// closes the picker and the menu and returns to play; a failure reports and
/// steps back to the menu, leaving play untouched.
fn menu_save(
    save_paths: &SavePaths,
    resources: &Resources,
    game: &Game,
    map: &Map,
    fog: &Fog,
    slot: usize,
    menu_picker: &mut Option<(SlotPicker, PickerPurpose)>,
    views: &mut Views,
    dialog: &mut Dialog,
) {
    *menu_picker = None;
    let (header, body) = match save_paths.slot(resources.pack_id.as_str(), slot) {
        Some(path) => match save_write(&path, game, resources, map, fog) {
            Ok(()) => {
                views.dismiss();
                menu::saved_message(resources, slot)
            }
            Err(err) => {
                eprintln!("save slot {slot} failed: {err:?}");
                menu::save_failed_message(resources, false)
            }
        },
        None => menu::save_failed_message(resources, true),
    };
    dialog.open(&header, &body);
}

/// Ports `main.c`'s game-over branch: with nothing modal up, a finished game
/// ends the session on any key. It sits after the view branch in the input
/// chain, so the key that dismisses the Win or Lose screen is not also the
/// key that quits.
fn game_over_quits(game: &Game, keys: &dyn KeySource) -> bool {
    game.is_over() && keys.any_pressed()
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
                let clamped = requested.min(game.max_recruitable(resources, &troop_id));
                let outcome =
                    game.recruit_from_dwelling(resources, &zone, x, y, &troop_id, requested);
                if let Some(message) = interact::dwelling_refusal(resources, clamped, outcome) {
                    dialog.open("", &message);
                }
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

#[cfg(test)]
mod tests {
    use super::{game_over_quits, Context, FrameInput, FrameStatus, Session};
    use crate::audio::Silent;
    use crate::draw::DrawList;
    use crate::input::{Key, Keys};
    use crate::layout::Layout;
    use crate::palette::Palette;
    use crate::present::Present;
    use crate::raster;
    use crate::savepath::SavePaths;
    use crate::sprites::SpriteCatalog;
    use crate::startup::{StartupAction, StartupChoice};
    use openbounty_engine::game::Game;

    /// The headless path the playtest runner takes: the game's own pack, a
    /// new game, real key frames. A step moves the hero and the frame says so.
    #[test]
    fn a_session_plays_the_games_own_pack_headlessly() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/stolen-scepter");
        let pack = crate::pack::Pack::open(&dir).expect("the pack opens");
        let resources = crate::pack::load_resources(&pack).expect("the pack loads");
        let layout = Layout::from_resources(&resources).expect("lays out");
        let catalog = SpriteCatalog::from_resources(&resources);
        let palette = Palette::fallback();
        let saves = std::env::temp_dir().join("openbounty-session-test");
        let save_paths = SavePaths::with_override(&saves.display().to_string());
        let ctx = Context {
            resources: &resources,
            layout: &layout,
            catalog: &catalog,
            palette: &palette,
            save_paths: &save_paths,
        };
        let choice = StartupChoice {
            action: StartupAction::New,
            slot: 0,
            class_id: "knight".into(),
            name: "Tester".into(),
            difficulty: 0,
        };
        let mut session = Session::start(
            ctx,
            Present::for_layout(&layout),
            Box::new(Silent),
            &choice,
            7,
        )
        .expect("a new game starts");
        let mut list = DrawList::new();
        let mut frame = |session: &mut Session, keys: Keys, n: f64| {
            let input = FrameInput {
                keys: &keys,
                pointer_at: None,
                left: false,
                right: false,
            };
            session.frame(&input, n / 60.0, &mut list)
        };

        // The post-init message is up first; any key dismisses it.
        assert_eq!(
            frame(&mut session, Keys::default(), 0.0),
            FrameStatus::Running
        );
        frame(&mut session, Keys::pressed(&[Key::Enter]), 1.0);
        let start = (session.game.position.x, session.game.position.y);
        frame(&mut session, Keys::pressed(&[Key::Down]), 2.0);
        frame(&mut session, Keys::default(), 3.0);
        assert_ne!((session.game.position.x, session.game.position.y), start);
        assert!(raster::texts(&list).iter().any(|t| t.contains("Days")));
    }

    fn finished() -> Game {
        let mut game = Game::default();
        game.stats.days_left = 3;
        game.stats.game_over = true;
        game
    }

    #[test]
    fn a_finished_game_quits_on_any_key() {
        assert!(game_over_quits(&finished(), &Keys::pressed(&[Key::A])));
        assert!(game_over_quits(&finished(), &Keys::pressed(&[Key::Enter])));
    }

    #[test]
    fn a_finished_game_waits_for_a_key() {
        assert!(!game_over_quits(&finished(), &Keys::default()));
    }

    /// `ui_any_key_pressed` skips modifiers; `any_pressed` already does.
    #[test]
    fn a_modifier_alone_does_not_quit() {
        assert!(!game_over_quits(
            &finished(),
            &Keys::pressed(&[Key::LeftControl])
        ));
    }

    #[test]
    fn a_live_game_never_quits_here() {
        let mut game = Game::default();
        game.stats.days_left = 3;
        assert!(!game_over_quits(&game, &Keys::pressed(&[Key::A])));
    }

    /// The `days_left == 0` half of `GameIsOver` quits too.
    #[test]
    fn an_empty_calendar_quits_without_the_flag() {
        let mut game = Game::default();
        game.stats.days_left = 0;
        assert!(game_over_quits(&game, &Keys::pressed(&[Key::A])));
    }
}
