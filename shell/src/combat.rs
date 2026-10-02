//! The combat field renderer and interactive loop, porting
//! `src/combat_render.c` and `src/combat_loop.c`.
//!
//! Combat owns the full inner area of the screen -- no sidebar -- on a fixed
//! 6x5 grid of 48x34 cells, whatever the overworld tile size. The engine's
//! [`Combat`] is the only rules state; this module reads it and emits
//! [`crate::draw::DrawList`] commands, so the field is testable headlessly the
//! way the views are.
//!
//! The action banner (the title-bar line that tells the player what the picker
//! wants or why an action was refused) is shell state, not engine state: C
//! stores `banner[80]` on its `Combat`, but that struct there doubles as the
//! shell's whole UI state. Here it lives on [`CombatLoop`] and mirrors the
//! combat log: hits, IMMUNE no-effects and spell announcements are formatted at
//! their call sites, and the clone/resurrect counts arrive as [`CombatEvent`]s.

use openbounty_engine::combat::{
    combat_ai_action, combat_cast_spell, combat_cell_passes_filter, combat_compact,
    combat_fly_unit, combat_hit_unit, combat_in_bounds, combat_init, combat_move_unit,
    combat_next_turn, combat_next_unit, combat_prepare_castle, combat_prepare_foe,
    combat_prepare_player, combat_reset_match, combat_reset_turn, combat_seed_rng,
    combat_spell_target_filter, combat_test_dead, combat_unit_surrounded, CastResult, Combat,
    CombatEvent, CombatMode, CombatTarget, PickFilter, COMBAT_H, COMBAT_SIDES, COMBAT_SIDE_AI,
    COMBAT_SIDE_PLAYER, COMBAT_SLOTS, COMBAT_SPELL_FIREBALL, COMBAT_SPELL_FREEZE,
    COMBAT_SPELL_LIGHTNING, COMBAT_SPELL_TELEPORT, COMBAT_SPELL_TURN_UNDEAD, COMBAT_W,
};
use openbounty_engine::game::Game;
use openbounty_engine::resources::Resources;
use openbounty_engine::tables::TROOP_ABIL_FLY;

use crate::banner;
use crate::draw::{DrawList, Dst, Rect, Sprite};
use crate::font;
use crate::input::{Key, KeySource, DIGITS};
use crate::layout::{Layout, MAP_X, MAP_Y};
use crate::palette::{Pal, Palette};
use crate::pointer::HitTarget;
use crate::present::Present;
use crate::prompt::{Prompt, PromptResult};
use crate::sprites::SpriteCatalog;
use crate::views::{controls_visible, troop_sprite, ViewKind, Views};

/// The A-G keys the cast menu reads, in order.
const COMBAT_SPELL_KEYS: [Key; 7] = [Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::G];

/// Combat-log banner defaults, porting `load_combat_log_defaults`. Only the
/// lines the shell itself emits are carried here; the pack overrides them
/// under `strings.combat_log`.
const DEFAULT_CL_NO_AMMO: &str = "No ammo";
const DEFAULT_CL_CANT_SHOOT: &str = "Can't Shoot";
const DEFAULT_CL_CANT_FLY: &str = "Can't Fly";
const DEFAULT_CL_ONLY_ONE_SPELL: &str = "Only 1 spell per round!";
const DEFAULT_CL_NO_SPELL_TYPE: &str = "No spells of that type";
const DEFAULT_CL_CANNOT_CAST: &str = "You cannot cast magic";
const DEFAULT_CL_FLY: &str = "%TROOP% fly";
const DEFAULT_CL_MELEE_HIT: &str = "%ATK% vs %TGT%, %COUNT% die";
const DEFAULT_CL_RANGED_HIT: &str = "%ATK% shoot %TGT% killing %COUNT%";
const DEFAULT_CL_NO_EFFECT: &str = "The spell seems to have no effect!";
const DEFAULT_CL_CLONED: &str = "%COUNT% %TROOP% cloned";
const DEFAULT_CL_TELEPORTED: &str = "Teleported";
const DEFAULT_CL_IMMUNE: &str = "%TROOP% are immune";
const DEFAULT_CL_FROZEN: &str = "%TROOP% are frozen";
const DEFAULT_CL_RESURRECTED: &str = "%COUNT% %TROOP% resurrected";
const DEFAULT_CL_CAST_FIREBALL: &str = "Fireball!";
const DEFAULT_CL_CAST_LIGHTNING: &str = "Lightning!";
const DEFAULT_CL_CAST_TURN_UNDEAD: &str = "Turn Undead!";

/// The give-up confirm, ported from the `combat_give_up_*` banners (which the
/// shipped pack leaves at their defaults).
const DEFAULT_GIVE_UP_HEADER: &str = "Press ESC to exit";
const DEFAULT_GIVE_UP_BODY: &str =
    "Giving up will forfeit your\narmies and send you back to\nthe King. Give up";

/// Combat field origin, C's `CL_COMBAT_X` / `CL_COMBAT_Y`.
pub const COMBAT_X: i32 = MAP_X;
pub const COMBAT_Y: i32 = MAP_Y;

/// Combat cell size, C's `CL_COMBAT_CELL_W` / `CL_COMBAT_CELL_H`. Combat is
/// always 6x5 at 48x34 even in a modern pack: the field fills the inner area
/// instead of the overworld's tile grid.
pub const COMBAT_CELL_W: i32 = 48;
pub const COMBAT_CELL_H: i32 = 34;

fn cell_origin(gx: i32, gy: i32) -> (i32, i32) {
    (COMBAT_X + gx * COMBAT_CELL_W, COMBAT_Y + gy * COMBAT_CELL_H)
}

/// A troop's display name, or `fallback` when the catalog has none.
fn troop_name(resources: &Resources, troop_index: i32, fallback: &str) -> String {
    resources
        .troop_by_index(troop_index)
        .map(|troop| {
            if troop.name.is_empty() {
                fallback.to_owned()
            } else {
                troop.name.as_str().to_owned()
            }
        })
        .unwrap_or_else(|| fallback.to_owned())
}

/// Turn one engine log event into its `strings.combat_log` banner. Only the
/// events the engine cannot express as a return value reach here: the clone and
/// resurrect counts. Hits, IMMUNE no-effects and the spell announcements are
/// formatted at their call sites instead.
fn format_event(resources: &Resources, event: CombatEvent) -> String {
    match event {
        CombatEvent::CountTroop {
            count,
            troop,
            resurrected,
        } => {
            let troop = troop_name(resources, troop, "creatures");
            let count = count.to_string();
            let (key, default) = if resurrected {
                ("resurrected", DEFAULT_CL_RESURRECTED)
            } else {
                ("cloned", DEFAULT_CL_CLONED)
            };
            banner::formatted(
                resources,
                "combat_log",
                key,
                default,
                &[("COUNT", &count), ("TROOP", &troop)],
            )
        }
    }
}

/// The status-bar title, porting `combat_format_title`. Before the first kill
/// it names the acting unit and its remaining moves/shots; after, it names the
/// largest stack on each side and the kill count.
pub fn combat_title(combat: &Combat, resources: &Resources) -> String {
    if !combat.first_kill_seen {
        let (name, moves, shots) = if combat.unit_id >= 0 {
            let unit = &combat.units[combat.side][combat.unit_id as usize];
            (
                troop_name(resources, unit.troop_idx, "-"),
                unit.moves,
                unit.shots,
            )
        } else {
            ("-".to_owned(), 0, 0)
        };
        if shots > 0 {
            format!(" Options / {name} M{moves},S{shots}")
        } else {
            format!(" Options / {name} M{moves}")
        }
    } else {
        let mut player_name = "Army".to_owned();
        let mut foe_name = "Foe".to_owned();
        let mut player_max = 0;
        let mut foe_max = 0;
        for slot in 0..COMBAT_SLOTS {
            let player = &combat.units[COMBAT_SIDE_PLAYER][slot];
            if player.count > player_max {
                player_max = player.count;
                player_name = troop_name(resources, player.troop_idx, "Army");
            }
            let foe = &combat.units[COMBAT_SIDE_AI][slot];
            if foe.count > foe_max {
                foe_max = foe.count;
                foe_name = troop_name(resources, foe.troop_idx, "Foe");
            }
        }
        format!(
            " {player_name} vs {foe_name} killing {}",
            combat.stacks_destroyed
        )
    }
}

/// C's `Combat.cursor_frame`. The ring is drawn at `11 + (cursor_frame & 3)`,
/// but nothing in C ever writes the field after the struct is zeroed, so the
/// four-frame ring animation is dead and the ring is always frame 11.
const CURSOR_FRAME: usize = 0;

/// The stock combat spell names, hard-coded in C's spell menu
/// (`combat_loop.c:339-342`) rather than read from the pack.
const COMBAT_SPELL_NAMES: [&str; 7] = [
    "Clone",
    "Teleport",
    "Fireball",
    "Lightning",
    "Freeze",
    "Resurrect",
    "Turn Undead",
];

/// Draw the combat field: the grass tiles, the obstacles, the units with their
/// count badges, the damage bursts, and -- while a picker is armed -- the
/// target ring at `pick_cursor`. Ports `combat_render_frame`; the title bar
/// it ends with is the chrome's.
pub fn combat_render(
    list: &mut DrawList,
    combat: &Combat,
    pick_cursor: Option<(i32, i32)>,
    palette: &Palette,
    catalog: &SpriteCatalog,
) {
    // Grass field, one tile per cell.
    for y in 0..COMBAT_H as i32 {
        for x in 0..COMBAT_W as i32 {
            let (px, py) = cell_origin(x, y);
            list.blit(
                Sprite::CombatTile(0),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }

    // Obstacles: omap codes 1..3 are field obstacles, 5..10 castle wall pieces.
    for y in 0..COMBAT_H as i32 {
        for x in 0..COMBAT_W as i32 {
            let code = combat.omap[y as usize][x as usize];
            if code == 0 {
                continue;
            }
            let (px, py) = cell_origin(x, y);
            list.blit(
                Sprite::CombatTile(code as usize),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }

    // Units, drawn over the obstacles. The AI side faces left (mirrored).
    for side in 0..COMBAT_SIDES {
        for slot in 0..COMBAT_SLOTS {
            let unit = &combat.units[side][slot];
            if unit.troop_idx < 0 || unit.count == 0 {
                continue;
            }
            let (px, py) = cell_origin(unit.x, unit.y);
            if let Some(sprite) = troop_sprite(catalog, unit.troop_idx, unit.frame) {
                list.blit_flipped(
                    sprite,
                    Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
                    side == COMBAT_SIDE_AI,
                );
            }
            // Count badge: white digits on a black band, bottom-centre.
            let digits = unit.count.to_string();
            let w = font::measure_width(&digits);
            let bx = px + (COMBAT_CELL_W - w) / 2;
            let by = py + COMBAT_CELL_H - font::GLYPH_H - 1;
            list.rect(
                Rect::new(bx - 1, by - 1, w + 2, font::GLYPH_H + 2),
                palette.clr(Pal::Black),
            );
            list.text(&digits, bx, by, palette.clr(Pal::White));
        }
    }

    // Damage burst (combat tile frame 4) over any unit that just took a hit.
    for side in 0..COMBAT_SIDES {
        for slot in 0..COMBAT_SLOTS {
            let unit = &combat.units[side][slot];
            if unit.troop_idx < 0 || unit.hit_flash <= 0 {
                continue;
            }
            let (px, py) = cell_origin(unit.x, unit.y);
            list.blit(
                Sprite::CombatTile(4),
                Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
            );
        }
    }

    // Target cursor (`combat_render.c:191-200`): shown only while picking; in
    // normal play the acting unit's own frame cycling marks whose turn it is.
    if let Some((x, y)) = pick_cursor {
        let (px, py) = cell_origin(x, y);
        list.blit(
            Sprite::CombatTile(11 + (CURSOR_FRAME & 3)),
            Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H),
        );
    }
}

/// The spell-pick menu, porting the overlay `combat_present` draws while the
/// cast is in its pick-spell phase (`combat_loop.c:328-349`), at C's literal
/// coordinates. `counts` is the caster's `spells.counts`, stock order.
pub fn combat_spell_menu(
    list: &mut DrawList,
    palette: &Palette,
    resources: &Resources,
    counts: &[i32],
) {
    list.rect(Rect::new(40, 30, 240, 130), palette.clr(Pal::DBlue));
    list.rect_lines(Rect::new(40, 30, 240, 130), palette.clr(Pal::Yellow));
    let title = banner::text(resources, "ui", "combat_spells_title", "Spells");
    list.text(title, 140, 36, palette.clr(Pal::Yellow));
    let column = banner::text(resources, "ui", "combat_spells_col_combat", "Combat");
    list.text(column, 60, 50, palette.clr(Pal::Yellow));
    for (i, name) in COMBAT_SPELL_NAMES.iter().enumerate() {
        let count = counts.get(i).copied().unwrap_or(0);
        let letter = (b'A' + i as u8) as char;
        let line = format!("{count} {name:<12} {letter}");
        list.text(&line, 56, 64 + i as i32 * 10, palette.clr(Pal::White));
    }
    let prompt = banner::text(resources, "ui", "combat_spells_prompt", "Cast which (A-G)?");
    list.text(prompt, 70, 144, palette.clr(Pal::White));
}

/// What a pick is for, porting `CombatPickReason`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PickReason {
    None,
    Shoot,
    Fly,
    SpellTarget,
    TeleportDest,
}

/// The cast workflow phase, porting `COMBAT_CAST_*`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastPhase {
    None,
    PickSpell,
    PickTarget,
    Apply,
}

/// Where a combat step left the fight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatStatus {
    Running,
    Win,
    Loss,
    /// A view was requested over the battlefield; the caller opens it and
    /// pauses combat until it is dismissed.
    OpenView(ViewKind),
}

/// The shell-side combat loop: the engine's [`Combat`] plus the cursor, picker
/// and cast state C keeps on the same struct. Ports `RunCombat`'s per-frame
/// input dispatch, minus the animation tick, the give-up prompt and the
/// end-of-combat dialog, which arrive with the flow wiring.
pub struct CombatLoop {
    pub combat: Combat,
    picker_active: bool,
    cursor_x: i32,
    cursor_y: i32,
    pick_filter: PickFilter,
    pick_reason: PickReason,
    cast_phase: CastPhase,
    cast_spell_idx: i32,
    pick_t1_side: usize,
    pick_t1_slot: usize,
    cast_dest_x: i32,
    cast_dest_y: i32,
    /// The action banner, C's `Combat.banner`: the title-bar line that
    /// overrides the standard "Options / <actor>" until the turn advances.
    banner: String,
    /// A turn advanced on the previous step; the banner is cleared at the top
    /// of the next step so the action that ended the turn is still shown once.
    pending_banner_clear: bool,
    /// A view the player action asked for this step. C's `views_set` from
    /// inside `combat_player_action_full` (`combat_loop.c:306-312`); the
    /// action returns "not acted" and `step` reports the view.
    requested_view: Option<ViewKind>,
    /// When the next SYN tick fires, pacing the damage burst and the AI to
    /// C's 150ms cadence.
    next_tick: f64,
}

impl CombatLoop {
    /// Set up a fight and prime the first acting unit. Ports the head of
    /// `RunCombat`.
    pub fn new(
        game: &Game,
        resources: &Resources,
        mode: CombatMode,
        target: &CombatTarget,
    ) -> CombatLoop {
        let mut combat = Combat::default();
        combat_init(&mut combat, game, resources, mode);
        combat_seed_rng(&mut combat, game, mode, target);
        combat_prepare_player(&mut combat, game, resources);
        if mode == CombatMode::Castle {
            combat_prepare_castle(&mut combat, resources, target);
        } else {
            combat_prepare_foe(&mut combat, resources, target);
        }
        combat_reset_match(&mut combat, resources);
        combat_reset_turn(&mut combat, resources, COMBAT_SIDE_AI);
        combat.unit_id = -1;
        let next = combat_next_unit(&mut combat);
        if next >= 0 {
            combat.unit_id = next;
        }
        CombatLoop {
            combat,
            picker_active: false,
            cursor_x: 0,
            cursor_y: 0,
            pick_filter: PickFilter::Any,
            pick_reason: PickReason::None,
            cast_phase: CastPhase::None,
            cast_spell_idx: -1,
            pick_t1_side: 0,
            pick_t1_slot: 0,
            cast_dest_x: 0,
            cast_dest_y: 0,
            banner: String::new(),
            pending_banner_clear: false,
            requested_view: None,
            next_tick: 0.0,
        }
    }

    /// The status-bar line, porting the three-branch `combat_format_title`:
    /// the action banner when one is set, else the acting-unit line before the
    /// first kill, else the running kill tally.
    pub fn title(&self, resources: &Resources) -> String {
        if !self.banner.is_empty() {
            return self.banner.clone();
        }
        combat_title(&self.combat, resources)
    }

    /// Where the target ring goes: the pick cursor, while a picker is armed.
    pub fn pick_cursor(&self) -> Option<(i32, i32)> {
        self.picker_active.then_some((self.cursor_x, self.cursor_y))
    }

    /// True while the cast waits for a spell letter, which is when the spell
    /// menu is drawn.
    pub fn picking_spell(&self) -> bool {
        self.cast_phase == CastPhase::PickSpell
    }

    /// Replace the action banner from a `strings.combat_log` line.
    fn log(&mut self, resources: &Resources, key: &str, default: &str) {
        self.banner = banner::text(resources, "combat_log", key, default).to_owned();
    }

    /// Replace the action banner from a formatted `strings.combat_log` line.
    fn log_formatted(
        &mut self,
        resources: &Resources,
        key: &str,
        default: &str,
        vars: &[banner::Var<'_>],
    ) {
        self.banner = banner::formatted(resources, "combat_log", key, default, vars);
    }

    /// Format a resolved hit from `combat_hit_unit`'s kill count: `kills > 0`
    /// is the melee/ranged line, `-1` is the IMMUNE no-effect, `0` is silent.
    fn emit_hit(
        &mut self,
        resources: &Resources,
        attacker: i32,
        target: i32,
        kills: i32,
        ranged: bool,
    ) {
        if kills > 0 {
            let atk = troop_name(resources, attacker, "Attackers");
            let tgt = troop_name(resources, target, "Defenders");
            let kills = kills.to_string();
            let (key, default) = if ranged {
                ("ranged_hit", DEFAULT_CL_RANGED_HIT)
            } else {
                ("melee_hit", DEFAULT_CL_MELEE_HIT)
            };
            self.log_formatted(
                resources,
                key,
                default,
                &[("ATK", &atk), ("TGT", &tgt), ("COUNT", &kills)],
            );
        } else if kills == -1 {
            self.log(resources, "no_effect_msg", DEFAULT_CL_NO_EFFECT);
        }
    }

    /// Format the announcement for the spell just applied. The clone/resurrect
    /// counts come from the engine's `CountTroop` event (drained by `step`),
    /// so only the count-less spell lines are set here.
    fn emit_spell_banner(&mut self, resources: &Resources, result: CastResult) {
        let ok = result == CastResult::Ok;
        match self.cast_spell_idx {
            COMBAT_SPELL_FIREBALL if ok => {
                self.log(resources, "cast_fireball", DEFAULT_CL_CAST_FIREBALL)
            }
            COMBAT_SPELL_LIGHTNING if ok => {
                self.log(resources, "cast_lightning", DEFAULT_CL_CAST_LIGHTNING)
            }
            COMBAT_SPELL_TURN_UNDEAD if ok => {
                self.log(resources, "cast_turn_undead", DEFAULT_CL_CAST_TURN_UNDEAD)
            }
            COMBAT_SPELL_FIREBALL | COMBAT_SPELL_LIGHTNING | COMBAT_SPELL_TURN_UNDEAD => {
                self.log(resources, "no_effect_msg", DEFAULT_CL_NO_EFFECT)
            }
            COMBAT_SPELL_TELEPORT if ok => self.log(resources, "teleported", DEFAULT_CL_TELEPORTED),
            COMBAT_SPELL_FREEZE => {
                let troop = self.combat.units[self.pick_t1_side][self.pick_t1_slot].troop_idx;
                let troop = troop_name(resources, troop, "Foe");
                let (key, default) = if ok {
                    ("frozen", DEFAULT_CL_FROZEN)
                } else {
                    ("immune", DEFAULT_CL_IMMUNE)
                };
                self.log_formatted(resources, key, default, &[("TROOP", &troop)]);
            }
            _ => {}
        }
    }

    /// Drive one frame of combat input. The player's actions dispatch through
    /// the engine's move/hit/fly/cast cores; the AI is paced to the SYN-tick
    /// rollover so units visibly walk between tiles.
    pub fn step(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
        time: f64,
        prompt: &mut Prompt,
    ) -> CombatStatus {
        if self.combat.result != 0 {
            return self.status();
        }

        // The give-up confirm is modal over the battlefield: it owns the keys
        // until answered. A yes forfeits the fight.
        if prompt.is_active() {
            if prompt.update(keys) == PromptResult::Yes {
                self.combat.result = 2;
            }
            return self.status();
        }

        // A turn advanced last step: revert the title to the standard
        // "Options / <actor>" line now that the action it described has been
        // shown.
        if self.pending_banner_clear {
            self.pending_banner_clear = false;
            self.banner.clear();
        }

        // Advance the damage-burst and acting-unit animation on the SYN cadence.
        let rolled_over = self.tick_anim(time);

        // View openers work even on the AI's turn, but not while a cast or a
        // target picker is mid-flight. C's order is O, A, V
        // (combat_loop.c:549-561).
        if self.cast_phase == CastPhase::None && !self.picker_active {
            if keys.pressed(Key::O) {
                return CombatStatus::OpenView(ViewKind::Options);
            }
            if keys.pressed(Key::A) {
                return CombatStatus::OpenView(ViewKind::Army);
            }
            if keys.pressed(Key::V) {
                return CombatStatus::OpenView(ViewKind::Character);
            }
        }

        let log_len_before = self.combat.log.len();
        let acted = if self.cast_phase != CastPhase::None {
            self.cast_step(keys, game, resources)
        } else if self.picker_active {
            self.pick_dispatch(keys, game, resources)
        } else {
            let player_turn = self.combat.side == COMBAT_SIDE_PLAYER
                && self.combat.unit_id >= 0
                && !self.combat.units[self.combat.side][self.combat.unit_id as usize]
                    .out_of_control;
            if player_turn {
                self.player_action(keys, game, resources, prompt)
            } else if rolled_over {
                combat_ai_action(&mut self.combat, resources) != 0
            } else {
                false
            }
        };

        // The engine recorded this action's line; show the latest one.
        if self.combat.log.len() > log_len_before {
            if let Some(event) = self.combat.log.last() {
                self.banner = format_event(resources, *event);
            }
        }

        // The player action opened a view (C's KEY_C). It spent no action, so
        // nothing below would run anyway.
        if let Some(view) = self.requested_view.take() {
            return CombatStatus::OpenView(view);
        }

        if !acted {
            return self.status();
        }

        combat_compact(&mut self.combat);
        if combat_test_dead(&self.combat, COMBAT_SIDE_AI) {
            self.combat.result = 1;
            return self.status();
        }
        if combat_test_dead(&self.combat, COMBAT_SIDE_PLAYER) {
            self.combat.result = 2;
            return self.status();
        }

        // Advance to the next unit only when the acting one is truly done.
        let done = self.combat.unit_id < 0 || {
            let unit = &self.combat.units[self.combat.side][self.combat.unit_id as usize];
            unit.troop_idx < 0 || unit.count == 0 || unit.acted
        };
        if done {
            if self.combat.unit_id >= 0 {
                let side = self.combat.side;
                self.combat.units[side][self.combat.unit_id as usize].frame = 0;
            }
            let next = combat_next_unit(&mut self.combat);
            if next < 0 {
                combat_next_turn(&mut self.combat, resources);
            } else {
                self.combat.unit_id = next;
            }
            // The new unit's turn reverts the title to the standard line, as C
            // clears the banner in combat_next_unit -- but on the NEXT step,
            // so the action that ended this turn is still shown once.
            self.pending_banner_clear = true;
        }
        self.status()
    }

    fn status(&self) -> CombatStatus {
        match self.combat.result {
            1 => CombatStatus::Win,
            2 => CombatStatus::Loss,
            _ => CombatStatus::Running,
        }
    }

    /// The SYN-tick animation pass, porting `combat_tick_anim`: every 150ms it
    /// decays the damage burst on every stack and advances the acting unit's
    /// frame, reporting true when the frame wrapped (which paces the AI).
    fn tick_anim(&mut self, time: f64) -> bool {
        if time < self.next_tick {
            return false;
        }
        self.next_tick = time + 0.15;

        for side in 0..COMBAT_SIDES {
            for slot in 0..COMBAT_SLOTS {
                let unit = &mut self.combat.units[side][slot];
                if unit.troop_idx < 0 {
                    continue;
                }
                if unit.hit_flash > 0 {
                    unit.hit_flash -= 1;
                }
            }
        }

        if self.combat.unit_id >= 0 {
            let side = self.combat.side;
            let slot = self.combat.unit_id as usize;
            let unit = &mut self.combat.units[side][slot];
            if unit.troop_idx >= 0 && unit.count > 0 {
                unit.frame += 1;
                if unit.frame > 3 {
                    unit.frame = 0;
                    return true;
                }
            }
        }
        false
    }

    fn player_action(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
        prompt: &mut Prompt,
    ) -> bool {
        if let Some((dx, dy)) = combat_read_dir(keys) {
            if self.combat.unit_id < 0 {
                return false;
            }
            return self.player_move(resources, dx, dy);
        }
        if keys.pressed(Key::Space) || keys.pressed(Key::W) {
            if self.combat.unit_id >= 0 {
                let side = self.combat.side;
                self.combat.units[side][self.combat.unit_id as usize].acted = true;
            }
            return true;
        }
        if keys.pressed(Key::S) {
            if self.combat.unit_id < 0 {
                return false;
            }
            let (side, id) = (self.combat.side, self.combat.unit_id as usize);
            let unit = &self.combat.units[side][id];
            if unit.shots <= 0 {
                self.log(resources, "no_ammo", DEFAULT_CL_NO_AMMO);
                return false;
            }
            if combat_unit_surrounded(&self.combat, side, id) {
                self.log(resources, "cant_shoot", DEFAULT_CL_CANT_SHOOT);
                return false;
            }
            self.cursor_x = unit.x;
            self.cursor_y = unit.y;
            self.pick_filter = PickFilter::Enemy;
            self.pick_reason = PickReason::Shoot;
            self.picker_active = true;
            return false;
        }
        if keys.pressed(Key::F) {
            if self.combat.unit_id < 0 {
                return false;
            }
            let (side, id) = (self.combat.side, self.combat.unit_id as usize);
            let unit = &self.combat.units[side][id];
            let flies = resources
                .troop_by_index(unit.troop_idx)
                .is_some_and(|troop| troop.abilities & TROOP_ABIL_FLY != 0);
            if !flies || unit.flights <= 0 {
                self.log(resources, "cant_fly", DEFAULT_CL_CANT_FLY);
                return false;
            }
            self.cursor_x = unit.x;
            self.cursor_y = unit.y;
            self.pick_filter = PickFilter::Empty;
            self.pick_reason = PickReason::Fly;
            self.picker_active = true;
            return false;
        }
        if keys.pressed(Key::U) {
            if self.combat.spells_this_round >= 1 {
                self.log(resources, "only_one_spell", DEFAULT_CL_ONLY_ONE_SPELL);
                return false;
            }
            if !game.stats.knows_magic {
                self.log(resources, "cannot_cast", DEFAULT_CL_CANNOT_CAST);
                return false;
            }
            self.cast_phase = CastPhase::PickSpell;
            return false;
        }
        if keys.pressed(Key::G) {
            let header = banner::text(
                resources,
                "banners",
                "combat_give_up_header",
                DEFAULT_GIVE_UP_HEADER,
            );
            let body = banner::text(
                resources,
                "banners",
                "combat_give_up_body",
                DEFAULT_GIVE_UP_BODY,
            );
            prompt.yes_no(header, body);
            return false;
        }
        if keys.pressed(Key::C) {
            // Opening a view pauses combat in the outer loop; the C<->O swap
            // lives there too (combat_loop.c:306-312). Not an action.
            self.requested_view = Some(ViewKind::Controls);
            return false;
        }
        false
    }

    /// One step in a direction: attack the occupant, or move into an empty
    /// cell. The attack is resolved here (not via `combat_move_unit`) so the
    /// kill count is in hand for the melee banner.
    fn player_move(&mut self, resources: &Resources, dx: i32, dy: i32) -> bool {
        let (side, id) = (self.combat.side, self.combat.unit_id as usize);
        let unit = &self.combat.units[side][id];
        let (nx, ny) = (unit.x + dx, unit.y + dy);
        if !combat_in_bounds(nx, ny) || self.combat.omap[ny as usize][nx as usize] != 0 {
            return false;
        }
        let uid = self.combat.umap[ny as usize][nx as usize];
        if uid != 0 {
            let o_side = (uid - 1) as usize / COMBAT_SLOTS;
            let o_slot = (uid - 1) as usize % COMBAT_SLOTS;
            // A same-side, in-control stack blocks the move, exactly as
            // combat_move_unit refuses it. The acting unit is always in
            // control here (the player turn gates on it).
            if o_side == side && !self.combat.units[o_side][o_slot].out_of_control {
                return false;
            }
            let attacker = unit.troop_idx;
            let target = self.combat.units[o_side][o_slot].troop_idx;
            let kills =
                combat_hit_unit(&mut self.combat, resources, side, id, o_side, o_slot, false);
            self.emit_hit(resources, attacker, target, kills, false);
            self.combat.units[side][id].acted = true;
            return true;
        }
        combat_move_unit(&mut self.combat, resources, side, id, dx, dy) != 0
    }

    /// The shoot/fly picker: one `combat_pick_step` per frame, then dispatch.
    fn pick_dispatch(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
    ) -> bool {
        let mut tx = 0usize;
        let mut ty = 0usize;
        let mut cancelled = false;
        if !self.pick_step(keys, resources, &mut tx, &mut ty, &mut cancelled) {
            if cancelled {
                self.picker_active = false;
                self.pick_reason = PickReason::None;
            }
            return false;
        }

        let (side, id) = (self.combat.side, self.combat.unit_id as usize);
        let reason = self.pick_reason;
        let acted = match reason {
            PickReason::Shoot => {
                let uid = self.combat.umap[ty][tx];
                let target_side = (uid - 1) as usize / COMBAT_SLOTS;
                let target_slot = (uid - 1) as usize % COMBAT_SLOTS;
                let attacker = self.combat.units[side][id].troop_idx;
                let target = self.combat.units[target_side][target_slot].troop_idx;
                let kills = combat_hit_unit(
                    &mut self.combat,
                    resources,
                    side,
                    id,
                    target_side,
                    target_slot,
                    true,
                );
                self.emit_hit(resources, attacker, target, kills, true);
                self.combat.units[side][id].acted = true;
                true
            }
            PickReason::Fly => {
                let flew = combat_fly_unit(&mut self.combat, side, id, tx as i32, ty as i32) != 0;
                if flew {
                    let troop = resources
                        .troop_by_index(self.combat.units[side][id].troop_idx)
                        .map(|t| t.name.as_str())
                        .unwrap_or("Troop");
                    self.log_formatted(resources, "fly", DEFAULT_CL_FLY, &[("TROOP", troop)]);
                }
                flew
            }
            _ => false,
        };
        self.picker_active = false;
        self.pick_reason = PickReason::None;
        let _ = game;
        acted
    }

    /// One target-picker step, porting `combat_pick_step`.
    fn pick_step(
        &mut self,
        keys: &dyn KeySource,
        resources: &Resources,
        out_x: &mut usize,
        out_y: &mut usize,
        out_cancelled: &mut bool,
    ) -> bool {
        *out_cancelled = false;
        let mut dx = 0;
        let mut dy = 0;
        if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
            dy = -1;
        } else if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
            dy = 1;
        }
        if keys.pressed(Key::Left) || keys.pressed(Key::Kp4) {
            dx = -1;
        } else if keys.pressed(Key::Right) || keys.pressed(Key::Kp6) {
            dx = 1;
        }
        if keys.pressed(Key::Kp7) || keys.pressed(Key::Home) {
            dx = -1;
            dy = -1;
        } else if keys.pressed(Key::Kp9) || keys.pressed(Key::PageUp) {
            dx = 1;
            dy = -1;
        } else if keys.pressed(Key::Kp1) || keys.pressed(Key::End) {
            dx = -1;
            dy = 1;
        } else if keys.pressed(Key::Kp3) || keys.pressed(Key::PageDown) {
            dx = 1;
            dy = 1;
        }
        if dx != 0 || dy != 0 {
            let nx = self.cursor_x + dx;
            let ny = self.cursor_y + dy;
            if combat_in_bounds(nx, ny) {
                self.cursor_x = nx;
                self.cursor_y = ny;
            }
            return false;
        }

        if keys.pressed(Key::Enter)
            || keys.pressed(Key::KpEnter)
            || keys.pressed(Key::Space)
            || keys.pressed(Key::A)
            || keys.pressed(Key::C)
        {
            if combat_cell_passes_filter(
                &self.combat,
                resources,
                self.cursor_x as usize,
                self.cursor_y as usize,
                self.combat.side,
                self.pick_filter,
            ) {
                *out_x = self.cursor_x as usize;
                *out_y = self.cursor_y as usize;
                return true;
            }
        }
        if keys.pressed(Key::Escape) {
            *out_cancelled = true;
        }
        false
    }

    /// The cast state machine, porting `combat_cast_step`.
    fn cast_step(&mut self, keys: &dyn KeySource, game: &mut Game, resources: &Resources) -> bool {
        match self.cast_phase {
            CastPhase::PickSpell => {
                if keys.pressed(Key::Escape) {
                    self.cast_phase = CastPhase::None;
                    return false;
                }
                let mut picked = -1;
                for (i, key) in COMBAT_SPELL_KEYS.iter().enumerate() {
                    if keys.pressed(*key) {
                        picked = i as i32;
                        break;
                    }
                }
                if picked < 0 {
                    return false;
                }
                if game.spells.counts[picked as usize] <= 0 {
                    self.log(resources, "no_spell_type", DEFAULT_CL_NO_SPELL_TYPE);
                    self.cast_phase = CastPhase::None;
                    return false;
                }
                self.cast_spell_idx = picked;
                self.pick_filter = combat_spell_target_filter(picked);
                self.pick_reason = PickReason::SpellTarget;
                self.picker_active = true;
                if self.combat.unit_id >= 0 {
                    let unit = &self.combat.units[self.combat.side][self.combat.unit_id as usize];
                    self.cursor_x = unit.x;
                    self.cursor_y = unit.y;
                }
                self.cast_phase = CastPhase::PickTarget;
                false
            }
            CastPhase::PickTarget => {
                let mut tx = 0usize;
                let mut ty = 0usize;
                let mut cancelled = false;
                if !self.pick_step(keys, resources, &mut tx, &mut ty, &mut cancelled) {
                    if cancelled {
                        self.cast_phase = CastPhase::None;
                        self.pick_reason = PickReason::None;
                        self.picker_active = false;
                    }
                    return false;
                }
                if self.cast_spell_idx == COMBAT_SPELL_TELEPORT
                    && self.pick_reason == PickReason::SpellTarget
                {
                    let uid = self.combat.umap[ty][tx];
                    self.pick_t1_side = (uid - 1) as usize / COMBAT_SLOTS;
                    self.pick_t1_slot = (uid - 1) as usize % COMBAT_SLOTS;
                    self.pick_filter = PickFilter::Empty;
                    self.pick_reason = PickReason::TeleportDest;
                    if self.combat.unit_id >= 0 {
                        let unit =
                            &self.combat.units[self.combat.side][self.combat.unit_id as usize];
                        self.cursor_x = unit.x;
                        self.cursor_y = unit.y;
                    }
                    return false;
                }
                if self.pick_reason == PickReason::TeleportDest {
                    self.cast_dest_x = tx as i32;
                    self.cast_dest_y = ty as i32;
                } else {
                    let uid = self.combat.umap[ty][tx];
                    self.pick_t1_side = (uid - 1) as usize / COMBAT_SLOTS;
                    self.pick_t1_slot = (uid - 1) as usize % COMBAT_SLOTS;
                }
                self.picker_active = false;
                self.pick_reason = PickReason::None;
                self.cast_phase = CastPhase::Apply;
                self.apply_cast(game, resources)
            }
            CastPhase::Apply => self.apply_cast(game, resources),
            CastPhase::None => false,
        }
    }

    fn apply_cast(&mut self, game: &mut Game, resources: &Resources) -> bool {
        let side = self.combat.side;
        let result = combat_cast_spell(
            &mut self.combat,
            resources,
            game,
            side,
            self.cast_spell_idx,
            self.pick_t1_side,
            self.pick_t1_slot,
            self.cast_dest_x,
            self.cast_dest_y,
        );
        self.cast_phase = CastPhase::None;
        self.emit_spell_banner(resources, result);
        result == CastResult::Ok
    }
}

/// Read one directional step, porting `combat_read_dir`.
fn combat_read_dir(keys: &dyn KeySource) -> Option<(i32, i32)> {
    if keys.pressed(Key::Up) || keys.pressed(Key::Kp8) {
        return Some((0, -1));
    }
    if keys.pressed(Key::Down) || keys.pressed(Key::Kp2) {
        return Some((0, 1));
    }
    if keys.pressed(Key::Left) || keys.pressed(Key::Kp4) {
        return Some((-1, 0));
    }
    if keys.pressed(Key::Right) || keys.pressed(Key::Kp6) {
        return Some((1, 0));
    }
    if keys.pressed(Key::Home) || keys.pressed(Key::Kp7) {
        return Some((-1, -1));
    }
    if keys.pressed(Key::PageUp) || keys.pressed(Key::Kp9) {
        return Some((1, -1));
    }
    if keys.pressed(Key::End) || keys.pressed(Key::Kp1) {
        return Some((-1, 1));
    }
    if keys.pressed(Key::PageDown) || keys.pressed(Key::Kp3) {
        return Some((1, 1));
    }
    None
}

/// One frame of input while a view is open over the battlefield. Combat is
/// paused: nothing here steps the fight. Ports `RunCombat`'s ESC branch
/// (`combat_loop.c:489-493`) and its paused-view branch (`:502-519`).
///
/// ESC is the ONLY key that closes a view here. C's paused branch reads no
/// other dismiss key, so Army and Character do not close on any key as they do
/// on the overworld. `C` on Options swaps to Controls and `O` on Controls swaps
/// back. A digit on Controls cycles the control at CATALOG row `digit - 1`, not
/// the visible row the panel labels with that digit: C passes its loop index
/// straight to `views_controls_advance`, skipping the `vis_map` the overworld
/// branch goes through (`main.c:1138-1142`), and moves no cursor. With a hidden
/// row ahead of it the digit and the label disagree -- ported as written.
///
/// A click is shell-side (C has no pointer) and keeps the meaning its region
/// has on the overworld: a click on a read-only view closes it, and a click on
/// a Controls row selects and cycles the row drawn under the pointer, the
/// modern Scale row included.
pub fn combat_view_input(
    views: &mut Views,
    keys: &dyn KeySource,
    clicked: Option<HitTarget>,
    game: &mut Game,
    resources: &Resources,
    layout: &Layout,
    present: &mut Present,
) {
    if let Some(target) = clicked {
        if target == HitTarget::ViewDismiss {
            views.dismiss();
        } else if let Some(row) = views.controls_click(target) {
            let vis = controls_visible(resources);
            if layout.is_modern && row == vis.len() {
                views.controls_set_cursor(row as i32);
                present.advance_scale();
            } else if let Some(&index) = vis.get(row) {
                views.controls_set_cursor(row as i32);
                views.controls_advance(game, resources, index);
            }
        }
        return;
    }

    if keys.pressed(Key::Escape) {
        views.dismiss();
        return;
    }

    let active = views.active();
    if active == Some(ViewKind::Options) && keys.pressed(Key::C) {
        views.set(ViewKind::Controls);
    } else if active == Some(ViewKind::Controls) && keys.pressed(Key::O) {
        views.set(ViewKind::Options);
    } else if active == Some(ViewKind::Controls) {
        for i in 0..9 {
            if keys.pressed(DIGITS[1 + i]) {
                views.controls_advance(game, resources, i);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::DrawCmd;
    use openbounty_engine::combat::{
        CombatUnit, COMBAT_SIDE_AI, COMBAT_SIDE_PLAYER, COMBAT_SPELL_CLONE,
    };

    fn fixtures() -> (Resources, SpriteCatalog, Palette, Combat) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":50,
                  "spoils_factor":1,"abilities":"","morale_group":"A"},
                 {"index":1,"id":"archers","name":"Archers","skill_level":1,"hit_points":3,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":250,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ]}"#,
        )
        .expect("fixture parses");
        let catalog = SpriteCatalog::from_resources(&resources);
        let palette = Palette::fallback();
        let mut combat = Combat::default();

        let mut player = CombatUnit::empty();
        player.troop_idx = 0;
        player.count = 10;
        player.x = 0;
        player.y = 0;
        player.moves = 5;
        player.shots = 2;
        combat.units[COMBAT_SIDE_PLAYER][0] = player;

        let mut foe = CombatUnit::empty();
        foe.troop_idx = 1;
        foe.count = 8;
        foe.x = 4;
        foe.y = 4;
        combat.units[COMBAT_SIDE_AI][0] = foe;

        combat.unit_id = 0;
        combat.side = COMBAT_SIDE_PLAYER;
        combat.omap[2][3] = 2;

        (resources, catalog, palette, combat)
    }

    #[test]
    fn the_title_names_the_acting_unit_before_the_first_kill() {
        let (resources, _, _, combat) = fixtures();
        assert_eq!(
            combat_title(&combat, &resources),
            " Options / Militia M5,S2"
        );
    }

    #[test]
    fn the_title_names_the_largest_stack_after_the_first_kill() {
        let (resources, _, _, mut combat) = fixtures();
        combat.first_kill_seen = true;
        combat.stacks_destroyed = 3;
        assert_eq!(
            combat_title(&combat, &resources),
            " Militia vs Archers killing 3"
        );
    }

    #[test]
    fn a_melee_only_unit_omits_the_shot_count() {
        let (resources, _, _, mut combat) = fixtures();
        combat.units[COMBAT_SIDE_PLAYER][0].shots = 0;
        assert_eq!(combat_title(&combat, &resources), " Options / Militia M5");
    }

    #[test]
    fn the_field_draws_grass_obstacles_and_count_badges() {
        let (_, catalog, palette, combat) = fixtures();
        let mut list = DrawList::new();
        combat_render(&mut list, &combat, None, &palette, &catalog);

        // 6x5 grass tiles, one obstacle tile, and both count badges.
        assert_eq!(list.blits_of(Sprite::CombatTile(0)).len(), 30);
        assert!(!list.blits_of(Sprite::CombatTile(2)).is_empty());
        assert!(list.has_text_containing("10"));
        assert!(list.has_text_containing("8"));
    }

    /// `combat_render.c:191-200`: the target ring is drawn only while a
    /// picker is armed, over the cursor cell, and after the units.
    #[test]
    fn the_target_ring_is_drawn_at_the_pick_cursor_only_while_picking() {
        let (_, catalog, palette, combat) = fixtures();
        let mut list = DrawList::new();
        combat_render(&mut list, &combat, None, &palette, &catalog);
        assert!(list.blits_of(Sprite::CombatTile(11)).is_empty());

        let mut list = DrawList::new();
        combat_render(&mut list, &combat, Some((2, 3)), &palette, &catalog);
        let (px, py) = cell_origin(2, 3);
        assert_eq!(
            list.blits_of(Sprite::CombatTile(11)),
            vec![Dst::rect(px, py, COMBAT_CELL_W, COMBAT_CELL_H)]
        );
    }

    /// C indexes the ring with `cursor_frame & 3`, but nothing in C ever
    /// writes `cursor_frame`, so the ring is always its first frame (11),
    /// never 12-14. Ported as written: the four-frame animation is dead.
    #[test]
    fn the_target_ring_never_animates_because_c_never_advances_its_frame() {
        let (_, catalog, palette, combat) = fixtures();
        for _ in 0..8 {
            let mut list = DrawList::new();
            combat_render(&mut list, &combat, Some((0, 0)), &palette, &catalog);
            for frame in 12..=14 {
                assert!(list.blits_of(Sprite::CombatTile(frame)).is_empty());
            }
        }
    }

    /// `combat_loop.c:328-349`, at C's literal coordinates: the panel, the
    /// yellow title and column header, one white "count name letter" row per
    /// stock combat spell, and the prompt.
    #[test]
    fn the_spell_menu_matches_cs_panel() {
        let (resources, _, palette, _) = fixtures();
        let mut counts = [0i32; 7];
        counts[2] = 3;
        let mut list = DrawList::new();
        combat_spell_menu(&mut list, &palette, &resources, &counts);
        assert!(list.as_slice().contains(&DrawCmd::Rect {
            rect: Rect::new(40, 30, 240, 130),
            color: palette.clr(Pal::DBlue),
        }));
        assert!(list.as_slice().contains(&DrawCmd::RectLines {
            rect: Rect::new(40, 30, 240, 130),
            color: palette.clr(Pal::Yellow),
        }));
        let texts = list.texts();
        assert!(texts.contains(&(140, 36, "Spells")));
        assert!(texts.contains(&(60, 50, "Combat")));
        assert!(texts.contains(&(56, 64, "0 Clone        A")));
        assert!(texts.contains(&(56, 84, "3 Fireball     C")));
        assert!(texts.contains(&(56, 124, "0 Turn Undead  G")));
        assert!(texts.contains(&(70, 144, "Cast which (A-G)?")));
    }

    /// The three labels come from `strings.ui`; the spell names are
    /// hard-coded in C, so a pack cannot rename them here.
    #[test]
    fn the_spell_menu_labels_come_from_the_pack() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [], "strings": {"ui": {
                 "combat_spells_title": "Zauber", "combat_spells_col_combat": "Kampf",
                 "combat_spells_prompt": "Welcher?"}}}"#,
        )
        .expect("fixture parses");
        let mut list = DrawList::new();
        combat_spell_menu(&mut list, &Palette::fallback(), &resources, &[1; 7]);
        let texts = list.texts();
        assert!(texts.contains(&(140, 36, "Zauber")));
        assert!(texts.contains(&(60, 50, "Kampf")));
        assert!(texts.contains(&(70, 144, "Welcher?")));
        assert!(texts.contains(&(56, 64, "1 Clone        A")));
    }

    use crate::input::Keys;

    /// A player militia (0,0) with moves and shots, facing an AI archer at
    /// (4,4). The player acts first.
    fn loop_fixture() -> (CombatLoop, Resources, Game) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,
                  "move_rate":3,"melee":[1,1],"ranged":[2,2,4],"recruit_cost":50,
                  "spoils_factor":1,"abilities":"","morale_group":"A"},
                 {"index":1,"id":"archers","name":"Archers","skill_level":1,"hit_points":3,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":250,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ]}"#,
        )
        .expect("fixture parses");

        let mut combat = Combat::default();
        let mut player = CombatUnit::empty();
        player.troop_idx = 0;
        player.count = 10;
        player.max_count = 10;
        player.x = 0;
        player.y = 0;
        player.moves = 3;
        player.shots = 2;
        combat.units[COMBAT_SIDE_PLAYER][0] = player;
        combat.umap[0][0] = 1;

        let mut foe = CombatUnit::empty();
        foe.troop_idx = 1;
        foe.count = 8;
        foe.max_count = 8;
        foe.x = 4;
        foe.y = 4;
        combat.units[COMBAT_SIDE_AI][0] = foe;
        combat.umap[4][4] = 6;

        combat.unit_id = 0;
        combat.side = COMBAT_SIDE_PLAYER;
        combat.heroes_present[COMBAT_SIDE_PLAYER] = true;

        let game = Game::default();
        let loop_ = CombatLoop {
            combat,
            picker_active: false,
            cursor_x: 0,
            cursor_y: 0,
            pick_filter: PickFilter::Any,
            pick_reason: PickReason::None,
            cast_phase: CastPhase::None,
            cast_spell_idx: -1,
            pick_t1_side: 0,
            pick_t1_slot: 0,
            cast_dest_x: 0,
            cast_dest_y: 0,
            banner: String::new(),
            pending_banner_clear: false,
            requested_view: None,
            next_tick: 0.0,
        };
        (loop_, resources, game)
    }

    #[test]
    fn a_player_move_steps_the_unit() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(
            &Keys::pressed(&[Key::Right]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::Running);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].x, 1);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].y, 0);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].moves, 2);
    }

    #[test]
    fn waiting_ends_the_players_turn() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.step(
            &Keys::pressed(&[Key::Space]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        // The player's lone stack is done, so the turn rolls over to the AI.
        assert_eq!(loop_.combat.side, COMBAT_SIDE_AI);
        assert_eq!(loop_.combat.unit_id, 0);
    }

    #[test]
    fn shooting_arms_the_picker() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert!(loop_.picker_active);
        assert_eq!(loop_.pick_reason, PickReason::Shoot);
        assert_eq!(loop_.cursor_x, 0);
        assert_eq!(loop_.cursor_y, 0);
    }

    #[test]
    fn a_unit_with_no_shots_cannot_shoot() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].shots = 0;
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert!(!loop_.picker_active);
    }

    #[test]
    fn the_title_prefers_the_action_banner() {
        let (mut loop_, resources, _) = loop_fixture();
        loop_.banner = "A banner".to_owned();
        assert_eq!(loop_.title(&resources), "A banner");
    }

    #[test]
    fn a_refused_shoot_logs_no_ammo() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].shots = 0;
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(loop_.banner, "No ammo");
    }

    #[test]
    fn a_second_spell_in_one_round_is_refused() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.spells_this_round = 1;
        game.stats.knows_magic = true;
        loop_.step(
            &Keys::pressed(&[Key::U]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(loop_.banner, "Only 1 spell per round!");
    }

    #[test]
    fn picking_a_spell_arms_the_picker_without_an_instruction() {
        let (mut loop_, resources, mut game) = loop_fixture();
        game.stats.knows_magic = true;
        game.spells.counts[COMBAT_SPELL_CLONE as usize] = 1;
        loop_.step(
            &Keys::pressed(&[Key::U]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        loop_.step(
            &Keys::pressed(&[Key::A]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        // C arms the picker with no instruction banner; `select_*` strings
        // are declared but never read.
        assert!(loop_.picker_active);
        assert!(!loop_.banner.contains("Select"));
    }

    /// The ring follows the real arming paths: `S` puts it on the acting
    /// unit, ESC takes it away.
    #[test]
    fn shoot_shows_the_ring_on_the_acting_unit_until_cancelled() {
        let (mut loop_, resources, mut game) = loop_fixture();
        assert_eq!(loop_.pick_cursor(), None);
        let mut prompt = Prompt::default();
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(loop_.pick_cursor(), Some((0, 0)));
        loop_.step(
            &Keys::pressed(&[Key::Escape]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(loop_.pick_cursor(), None);
    }

    /// `U` opens the spell menu; the letter closes it into the target picker,
    /// whose ring then shows.
    #[test]
    fn the_spell_menu_shows_from_u_until_a_letter_is_picked() {
        let (mut loop_, resources, mut game) = loop_fixture();
        game.stats.knows_magic = true;
        game.spells.counts[COMBAT_SPELL_CLONE as usize] = 1;
        let mut prompt = Prompt::default();
        assert!(!loop_.picking_spell());
        loop_.step(
            &Keys::pressed(&[Key::U]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert!(loop_.picking_spell());
        loop_.step(
            &Keys::pressed(&[Key::A]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert!(!loop_.picking_spell());
        assert!(loop_.pick_cursor().is_some());
    }

    #[test]
    fn the_banner_clears_on_the_step_after_a_turn_advance() {
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.banner = "stale".to_owned();
        // Wait ends the lone player stack's turn; the banner is shown once,
        // then cleared on the next step.
        loop_.step(
            &Keys::pressed(&[Key::Space]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(loop_.banner, "stale");
        loop_.step(
            &Keys::pressed(&[]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert!(loop_.banner.is_empty());
    }

    #[test]
    fn a_hit_banner_names_both_troops() {
        let (mut loop_, resources, _) = loop_fixture();
        loop_.emit_hit(&resources, 0, 1, 2, false);
        assert_eq!(loop_.banner, "Militia vs Archers, 2 die");
    }

    #[test]
    fn a_retaliation_produces_no_banner_line() {
        let (mut loop_, resources, mut game) = loop_fixture();
        // Put the player militia adjacent to the AI archer, then walk into it
        // so the move becomes an attack.
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].x = 3;
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].y = 4;
        loop_.combat.umap[4][3] = 1;
        loop_.step(
            &Keys::pressed(&[Key::Right]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        // C's retaliation calls combat_deal_damage directly and emits nothing;
        // the banner shows only the melee hit, never a "retaliate" line.
        assert!(loop_.banner.contains("vs Archers"));
        assert!(!loop_.banner.contains("retaliate"));
    }

    #[test]
    fn the_animation_tick_decays_the_damage_burst() {
        let (mut loop_, _, _) = loop_fixture();
        loop_.combat.units[COMBAT_SIDE_PLAYER][0].hit_flash = 3;
        loop_.tick_anim(0.0);
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].hit_flash, 2);
    }

    #[test]
    fn the_animation_tick_wraps_the_frame_every_four_ticks() {
        let (mut loop_, _, _) = loop_fixture();
        assert!(!loop_.tick_anim(0.0));
        assert!(!loop_.tick_anim(0.16));
        assert!(!loop_.tick_anim(0.31));
        assert!(loop_.tick_anim(0.46));
        assert_eq!(loop_.combat.units[COMBAT_SIDE_PLAYER][0].frame, 0);
    }

    #[test]
    fn giving_up_forfeits_the_fight() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let mut prompt = Prompt::default();

        // G opens the modal confirm without ending the turn.
        loop_.step(
            &Keys::pressed(&[Key::G]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert!(prompt.is_active());

        // Answering yes forfeits the fight.
        let status = loop_.step(
            &Keys::pressed(&[Key::Y]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::Loss);
        assert_eq!(loop_.combat.result, 2);
    }

    #[test]
    fn a_view_key_opens_a_view_over_the_battlefield() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let mut prompt = Prompt::default();

        let status = loop_.step(
            &Keys::pressed(&[Key::A]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Army));

        let status = loop_.step(
            &Keys::pressed(&[Key::V]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Character));
    }

    #[test]
    fn o_opens_the_options_view_on_either_sides_turn() {
        // combat_loop.c:549-553: the top-level opener runs before the
        // player_turn dispatch, so the AI's turn does not gate it.
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Options));

        // Reuse the one fixture: a second `loop_fixture()` in the same test
        // frame overflows the default test-thread stack in a debug build.
        loop_.combat.side = COMBAT_SIDE_AI;
        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Options));
    }

    #[test]
    fn c_opens_the_controls_view_on_the_players_turn_without_spending_it() {
        let (mut loop_, resources, mut game) = loop_fixture();
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::OpenView(ViewKind::Controls));
        // combat_loop.c:310-311 returns 0: no action, the same unit still acts.
        assert!(!loop_.combat.units[COMBAT_SIDE_PLAYER][0].acted);
        assert_eq!(loop_.combat.side, COMBAT_SIDE_PLAYER);
        assert_eq!(loop_.combat.unit_id, 0);
    }

    #[test]
    fn c_is_a_player_action_so_the_ai_turn_ignores_it() {
        // combat_loop.c:306-312 lives in combat_player_action_full, reached
        // only from the player_turn arm (:612-613).
        let (mut loop_, resources, mut game) = loop_fixture();
        loop_.combat.side = COMBAT_SIDE_AI;
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut Prompt::default(),
        );
        assert_eq!(status, CombatStatus::Running);
    }

    #[test]
    fn an_armed_picker_keeps_o_and_c_from_opening_a_view() {
        // O is gated on !picker_active (combat_loop.c:549); C is the picker's
        // confirm key (combat_pick_step), and the cursor sits on our own
        // stack, which the Enemy filter refuses.
        let (mut loop_, resources, mut game) = loop_fixture();
        let mut prompt = Prompt::default();
        loop_.step(
            &Keys::pressed(&[Key::S]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert!(loop_.picker_active);

        let status = loop_.step(
            &Keys::pressed(&[Key::O]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::Running);
        let status = loop_.step(
            &Keys::pressed(&[Key::C]),
            &mut game,
            &resources,
            100.0,
            &mut prompt,
        );
        assert_eq!(status, CombatStatus::Running);
        assert!(
            loop_.picker_active,
            "a refused confirm leaves the picker armed"
        );
    }

    /// A legacy pack with a hidden control AHEAD of a visible one, so the
    /// visible row a digit labels and the catalog row C's combat branch
    /// cycles are different rows. Visible rows: Delay (0), Animation (2).
    fn view_fixture() -> (Resources, Layout, Game) {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "controls": {"settings": [
                  {"id":"delay","label":"Delay","type":"numeric","range":10,"default":5},
                  {"id":"cga","label":"CGA","type":"numeric","range":8,"hidden":true},
                  {"id":"animation","label":"Animation","type":"bool"}
                ]}}"#,
        )
        .expect("fixture parses");
        let layout = Layout::from_resources(&resources).expect("lays out");
        (resources, layout, Game::default())
    }

    fn press(
        views: &mut Views,
        keys: &[Key],
        game: &mut Game,
        resources: &Resources,
        layout: &Layout,
    ) {
        combat_view_input(
            views,
            &Keys::pressed(keys),
            None,
            game,
            resources,
            layout,
            &mut Present::new(),
        );
    }

    #[test]
    fn only_escape_closes_a_view_over_the_battlefield() {
        // combat_loop.c:489-493 is the only dismiss; the paused branch
        // (:502-519) reads no other key for it.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Army);
        for key in [Key::Space, Key::A, Key::Enter, Key::Digit1] {
            press(&mut views, &[key], &mut game, &resources, &layout);
            assert_eq!(
                views.active(),
                Some(ViewKind::Army),
                "{key:?} must not close it"
            );
        }
        press(&mut views, &[Key::Escape], &mut game, &resources, &layout);
        assert_eq!(views.active(), None);
    }

    #[test]
    fn c_and_o_swap_options_and_controls_in_place() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Options);

        press(&mut views, &[Key::C], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Controls));
        // Unlike the overworld (main.c:1132-1135), C does not close Controls.
        press(&mut views, &[Key::C], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Controls));

        press(&mut views, &[Key::O], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Options));
        press(&mut views, &[Key::O], &mut game, &resources, &layout);
        assert_eq!(views.active(), Some(ViewKind::Options));

        // Digits cycle only on Controls.
        let before = game.stats.options;
        press(&mut views, &[Key::Digit1], &mut game, &resources, &layout);
        assert_eq!(game.stats.options, before);
    }

    #[test]
    fn a_combat_digit_cycles_the_catalog_row_not_the_labelled_row() {
        // The panel labels Animation "2" because CGA is hidden, but
        // combat_loop.c:511-513 hands the raw loop index to
        // views_controls_advance, skipping the overworld's vis_map.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);

        press(&mut views, &[Key::Digit2], &mut game, &resources, &layout);
        assert_eq!(game.stats.options[1], 2, "hidden CGA 1 -> 2");
        assert_eq!(game.stats.options[2], 1, "Animation untouched");
        assert_eq!(
            views.controls_cursor(),
            0,
            "the combat branch moves no cursor"
        );

        press(&mut views, &[Key::Digit1], &mut game, &resources, &layout);
        assert_eq!(game.stats.options[0], 2, "Delay 1 -> 2");
    }

    #[test]
    fn the_overworld_controls_keys_are_inert_in_combat() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        let before = game.stats.options;

        // combat_loop.c:509-517 reads digits only: no Up/Down, no
        // Enter/Space, and a digit past the catalog is views_controls_advance's
        // own out-of-range no-op (views.c:945).
        for key in [Key::Down, Key::Up, Key::Enter, Key::Space, Key::Digit9] {
            press(&mut views, &[key], &mut game, &resources, &layout);
        }
        assert_eq!(game.stats.options, before);
        assert_eq!(views.controls_cursor(), 0);
        assert_eq!(views.active(), Some(ViewKind::Controls));
    }

    #[test]
    fn a_click_on_a_controls_row_cycles_the_row_drawn_under_it() {
        // Shell-side: C has no pointer. A click names the drawn row, so the
        // second VISIBLE row is Animation, not the hidden CGA a digit reaches.
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Controls);
        combat_view_input(
            &mut views,
            &Keys::pressed(&[]),
            Some(HitTarget::ControlsRow(1)),
            &mut game,
            &resources,
            &layout,
            &mut Present::new(),
        );
        assert_eq!(game.stats.options[2], 0, "Animation 1 -> 0");
        assert_eq!(game.stats.options[1], 1, "CGA untouched");
        assert_eq!(views.controls_cursor(), 1);
    }

    #[test]
    fn a_click_closes_a_read_only_view_over_the_battlefield() {
        let (resources, layout, mut game) = view_fixture();
        let mut views = Views::new();
        views.set(ViewKind::Character);
        combat_view_input(
            &mut views,
            &Keys::pressed(&[]),
            Some(HitTarget::ViewDismiss),
            &mut game,
            &resources,
            &layout,
            &mut Present::new(),
        );
        assert_eq!(views.active(), None);
    }
}
