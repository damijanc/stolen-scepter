//! Adventure spell effects, porting OpenBounty's `engine/spells_adventure.c`
//! together with the `GameCast*` cores it drives in `engine/game.c`.
//!
//! C's `cast_*` helpers interleave state mutation with `player_io_message`
//! calls. Only the mutation belongs here: each cast returns an
//! [`AdventureCast`] carrying the values a shell needs to fill the matching
//! banner template, and the banner text itself stays outside the engine.
//!
//! The two gate spells classify and arm here but do not teleport, which is
//! C's split too: a cast only opens the picker, and the shell then calls
//! [`Game::gate_destinations`] for the list and [`Game::gate_teleport`] for
//! the move. [`AdventureCast::GateArmed`] reports the eligible count, and C
//! really does count destinations twice -- once in `cast_castle_gate` and
//! again in `GameGateDestinations`. Both scans are ported as written.

use crate::game::{CastleOwnerKind, Game, GAME_CASTLES, GAME_TOWNS};
use crate::map::Map;
use crate::resources::Resources;
use crate::tables::{FixedStr, SpellKind, CAT_ID_LEN};
use crate::tile::{Interact, Terrain};

/// Leadership ceiling applied by the raise-control cast. Ports
/// `GAME_LEADERSHIP_MAX` (`engine/include/game.h`).
pub const GAME_LEADERSHIP_MAX: i32 = 65_000;

/// The id-to-effect classification, porting `AdventureEffect`. A pack spell id
/// absent from the table is an adventure spell the engine cannot apply.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AdventureEffect {
    #[default]
    None,
    Bridge,
    TimeStop,
    FindVillain,
    GateCastle,
    GateTown,
    InstantArmy,
    RaiseControl,
}

/// Which gate spell armed the destination picker. C carries this as the
/// `gate_mode` int, 0 for castles and 1 for towns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GateMode {
    Castle,
    Town,
}

/// What a cast did, in place of the message C queues for each outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdventureCast {
    /// The slot classified to no effect this engine applies.
    /// `GameApplyAdventureSpellEffect` returns false for exactly this case.
    Unknown,
    /// The effect was recognised but the hero holds no charge, so nothing
    /// happened. Every C `cast_*` opens with this guard and returns silently.
    NoCharge,
    TimeStop {
        /// Steps added by this cast, which is 0 at spell power 0.
        steps: i32,
        /// The resulting bank, which is what C's banner reports.
        banked: i32,
    },
    /// No active contract, so there is no villain to find. The charge is NOT
    /// spent.
    FindVillainNoContract,
    /// The contract villain's castle was revealed, or was not found.
    FindVillain { castle: Option<usize> },
    /// Armed and waiting for a direction. The charge is spent by the caller
    /// once [`try_build_bridge`] actually places tiles, not here.
    BridgeArmed,
    /// Nothing to gate to: no visited, eligible destination of this kind.
    GateNone { mode: GateMode },
    /// Armed the destination picker. The charge is spent only on a chosen
    /// teleport, never on the cast or a cancel.
    GateArmed { mode: GateMode, destinations: i32 },
    /// The class and rank name no troop to conjure. The charge is NOT spent.
    InstantArmyFizzle,
    /// The army has no slot for the conjured troop. The charge is NOT spent.
    InstantArmyNoRoom,
    InstantArmy {
        troop: FixedStr<CAT_ID_LEN>,
        count: i32,
    },
    RaiseControl {
        /// The UNCAPPED amount, which is what C's banner reports even when
        /// the cap swallowed part of it.
        amount: i32,
    },
}

impl AdventureCast {
    /// C's `GameApplyAdventureSpellEffect` return value: true whenever the
    /// slot classified to a known adventure effect, including the casts that
    /// then declined to do anything.
    pub fn applied(&self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

/// What the spellbook dispatcher decided, porting `dispatch_adventure_spell`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpellCast {
    /// A combat spell holding a charge. C cannot cast it in the field and
    /// instead offers to discard the charge to free a spellbook slot.
    OfferDiscard,
    /// The slot holds no spell at all.
    Unavailable,
    /// The spell is not known, meaning no charge is held. C reaches this for
    /// a charge-less combat spell as well as a charge-less adventure spell.
    NotKnown,
    Applied(AdventureCast),
}

/// Classify catalog slot `spell_idx`. Ports `spell_adventure_effect`
/// (`engine/tables.c`), the single id-to-effect table.
pub fn spell_adventure_effect(resources: &Resources, spell_idx: i32) -> AdventureEffect {
    let Some(spell) = resources.spell_by_index(spell_idx) else {
        return AdventureEffect::None;
    };
    if spell.kind != SpellKind::Adventure {
        return AdventureEffect::None;
    }
    match spell.id.as_str() {
        "bridge" => AdventureEffect::Bridge,
        "time_stop" => AdventureEffect::TimeStop,
        "find_villain" => AdventureEffect::FindVillain,
        "castle_gate" => AdventureEffect::GateCastle,
        "town_gate" => AdventureEffect::GateTown,
        "instant_army" => AdventureEffect::InstantArmy,
        "raise_control" => AdventureEffect::RaiseControl,
        _ => AdventureEffect::None,
    }
}

/// The first catalog slot carrying `effect`, or -1. Ports
/// `spell_index_by_adventure_effect` without its memo table: C caches the
/// effect-to-index map purely because the autoplay mover calls this millions
/// of times, and both its cached and uncached paths answer with the first
/// matching index.
pub fn spell_index_by_adventure_effect(resources: &Resources, effect: AdventureEffect) -> i32 {
    if effect == AdventureEffect::None {
        return -1;
    }
    let count = i32::try_from(resources.spells_count()).unwrap_or(i32::MAX);
    for index in 0..count {
        if spell_adventure_effect(resources, index) == effect {
            return index;
        }
    }
    -1
}

/// Build a bridge from the hero's tile in direction `(dx, dy)`, returning the
/// number of tiles placed. Ports `try_build_bridge`.
///
/// The caller spends the charge when this returns a positive count; a cast
/// that finds no water spends nothing.
pub fn try_build_bridge(game: &Game, map: &mut Map, dx: i32, dy: i32) -> i32 {
    let mut built = 0_i32;
    // C's loop bound of 5 is unreachable past the second step: every
    // iteration that does not break increments `built`, and `built >= 2`
    // breaks. The header comment's "or up to 5 water tiles" describes an
    // intent the code does not implement; the cap is always 2.
    for i in 1..=5_i32 {
        let nx = game.position.x.wrapping_add(i.wrapping_mul(dx));
        let ny = game.position.y.wrapping_add(i.wrapping_mul(dy));

        let Some(tile) = map.tile_mut_i32(nx, ny) else {
            break;
        };
        if tile.terrain != Terrain::Water {
            break;
        }

        // C also stamps the "bridge_v"/"bridge_h" sprite name into `art`.
        // The Rust tile carries no `art`: map.c documents it as the
        // renderer's sprite key, and `is_bridge` is what the engine reads.
        tile.terrain = Terrain::Grass;
        tile.interactive = Interact::None;
        tile.blocks_foot = false;
        tile.is_bridge = true;

        built = built.wrapping_add(1);
        if built >= 2 {
            break;
        }
    }
    built
}

impl Game {
    /// Steps banked per time-stop cast: `spell_power * 10`. Ports
    /// `GameTimeStopStepsPerCast`, which applies NO minimum, so a rank with
    /// spell power 0 banks nothing.
    pub fn time_stop_steps_per_cast(&self) -> i32 {
        self.stats.spell_power.wrapping_mul(10)
    }

    /// Leadership gained per raise-control cast: `spell_power * 100`. Ports
    /// `GameRaiseControlAmount`, likewise with no minimum.
    pub fn raise_control_amount(&self) -> i32 {
        self.stats.spell_power.wrapping_mul(100)
    }

    /// The catalog slot for `effect` when the hero holds a charge for it.
    /// Ports the `idx < 0 || counts[idx] <= 0` guard every `cast_*` opens
    /// with.
    fn charged_spell_slot(&self, resources: &Resources, effect: AdventureEffect) -> Option<usize> {
        let index = spell_index_by_adventure_effect(resources, effect);
        let index = usize::try_from(index).ok()?;
        if index >= self.spells.counts.len() || self.spells.counts[index] <= 0 {
            return None;
        }
        Some(index)
    }

    /// Ports `GameCastTimeStop`.
    pub fn cast_time_stop(&mut self, resources: &Resources) -> AdventureCast {
        let Some(index) = self.charged_spell_slot(resources, AdventureEffect::TimeStop) else {
            return AdventureCast::NoCharge;
        };
        self.spells.counts[index] = self.spells.counts[index].wrapping_sub(1);
        let steps = self.time_stop_steps_per_cast();
        self.stats.time_stop = self.stats.time_stop.wrapping_add(steps);
        AdventureCast::TimeStop {
            steps,
            banked: self.stats.time_stop,
        }
    }

    /// Ports `GameCastRaiseControl`. The charge IS spent at spell power 0,
    /// where the amount is 0: the cast happens, it just does nothing. The
    /// boost is temporary -- `leadership_current` resets to
    /// `leadership_base` at the next week boundary.
    pub fn cast_raise_control(&mut self, resources: &Resources) -> AdventureCast {
        let Some(index) = self.charged_spell_slot(resources, AdventureEffect::RaiseControl) else {
            return AdventureCast::NoCharge;
        };
        self.spells.counts[index] = self.spells.counts[index].wrapping_sub(1);
        let amount = self.raise_control_amount();
        self.stats.leadership_current = self.stats.leadership_current.wrapping_add(amount);
        if self.stats.leadership_current > GAME_LEADERSHIP_MAX {
            self.stats.leadership_current = GAME_LEADERSHIP_MAX;
        }
        // C's banner reports the uncapped amount even when the cap swallowed
        // part of it.
        AdventureCast::RaiseControl { amount }
    }

    /// Ports `GameCastFindVillain`. Note the guard order: the charge check
    /// comes first, but the decrement comes AFTER the contract check, so a
    /// cast with no active contract spends nothing.
    pub fn cast_find_villain(&mut self, resources: &Resources) -> AdventureCast {
        let Some(index) = self.charged_spell_slot(resources, AdventureEffect::FindVillain) else {
            return AdventureCast::NoCharge;
        };
        if self.contract.active_id.is_empty() {
            return AdventureCast::FindVillainNoContract;
        }
        self.spells.counts[index] = self.spells.counts[index].wrapping_sub(1);
        for castle in 0..GAME_CASTLES {
            if self.castles[castle].id.is_empty() {
                continue;
            }
            if self.castles[castle].owner_kind != CastleOwnerKind::Villain {
                continue;
            }
            if self.castles[castle].villain_id.as_str() != self.contract.active_id.as_str() {
                continue;
            }
            self.castles[castle].known = true;
            return AdventureCast::FindVillain {
                castle: Some(castle),
            };
        }
        AdventureCast::FindVillain { castle: None }
    }

    /// The troop this class and rank conjures: `classes[class][rank].instant_army`.
    /// Ports `GameInstantArmyTroop`.
    pub fn instant_army_troop(&self, resources: &Resources) -> Option<FixedStr<CAT_ID_LEN>> {
        let class = resources.class_by_id(self.character.cls.id.as_str())?;
        let mut rank = self.character.cls.rank_index;
        if rank < 0 || rank >= class.rank_count as i32 {
            rank = 0;
        }
        let rank = usize::try_from(rank).ok()?;
        let troop = resources.troop_by_index(class.ranks.get(rank)?.instant_army)?;
        Some(troop.id)
    }

    /// `(spell_power + 1) * instant_army_multiplier[rank]`, floored at 1.
    /// Ports `GameInstantArmyPerCast`, including its quirk that an unknown
    /// class still yields a count off rank 0 rather than nothing.
    pub fn instant_army_per_cast(&self, resources: &Resources) -> i32 {
        let class = resources.class_by_id(self.character.cls.id.as_str());
        let mut rank = self.character.cls.rank_index;
        if class.is_none() || rank < 0 || rank >= class.map_or(0, |c| c.rank_count as i32) {
            rank = 0;
        }
        let multipliers = &resources.tuning.instant_army_multiplier;
        let mult_rank = if rank >= 0 && (rank as usize) < multipliers.len() {
            rank as usize
        } else {
            0
        };
        let mut multiplier = multipliers[mult_rank];
        if multiplier < 1 {
            multiplier = 1;
        }
        let count = self
            .stats
            .spell_power
            .wrapping_add(1)
            .wrapping_mul(multiplier);
        if count < 1 {
            1
        } else {
            count
        }
    }

    /// Ports `cast_instant_army`. Unlike time stop and raise control, the
    /// charge is spent only on success: a fizzle and a full army both leave
    /// it in the book.
    pub fn cast_instant_army(&mut self, resources: &Resources) -> AdventureCast {
        let Some(index) = self.charged_spell_slot(resources, AdventureEffect::InstantArmy) else {
            return AdventureCast::NoCharge;
        };
        let Some(troop) = self.instant_army_troop(resources) else {
            return AdventureCast::InstantArmyFizzle;
        };
        if troop.is_empty() {
            return AdventureCast::InstantArmyFizzle;
        }
        let count = self.instant_army_per_cast(resources);
        if self.add_troop(troop.as_str(), count) != 0 {
            return AdventureCast::InstantArmyNoRoom;
        }
        self.spells.counts[index] = self.spells.counts[index].wrapping_sub(1);
        AdventureCast::InstantArmy { troop, count }
    }

    /// Ports `cast_bridge`: the cast only arms the direction prompt. The
    /// charge is spent by whoever calls [`try_build_bridge`] and gets a
    /// positive count back.
    pub fn cast_bridge(&mut self, resources: &Resources) -> AdventureCast {
        match self.charged_spell_slot(resources, AdventureEffect::Bridge) {
            Some(_) => AdventureCast::BridgeArmed,
            None => AdventureCast::NoCharge,
        }
    }

    /// Ports `cast_castle_gate`. Eligible destinations are visited castles
    /// that are neither the home/audience castle nor zone-less; the spell
    /// cannot gate to the castle that grants audiences.
    pub fn cast_castle_gate(&mut self, resources: &Resources) -> AdventureCast {
        if self
            .charged_spell_slot(resources, AdventureEffect::GateCastle)
            .is_none()
        {
            return AdventureCast::NoCharge;
        }
        let mut destinations = 0_i32;
        for index in 0..GAME_CASTLES.min(resources.castle_count()) {
            if !self.castles[index].visited {
                continue;
            }
            let Some(castle) = resources.castle_by_index(index) else {
                continue;
            };
            if castle.is_home() {
                continue;
            }
            if castle.zone.is_empty() {
                continue;
            }
            destinations = destinations.wrapping_add(1);
        }
        if destinations == 0 {
            return AdventureCast::GateNone {
                mode: GateMode::Castle,
            };
        }
        AdventureCast::GateArmed {
            mode: GateMode::Castle,
            destinations,
        }
    }

    /// Ports `cast_town_gate`. Unlike the castle gate this has no home
    /// exclusion: any visited town with a zone is a destination.
    pub fn cast_town_gate(&mut self, resources: &Resources) -> AdventureCast {
        if self
            .charged_spell_slot(resources, AdventureEffect::GateTown)
            .is_none()
        {
            return AdventureCast::NoCharge;
        }
        let mut destinations = 0_i32;
        for index in 0..GAME_TOWNS.min(resources.town_count()) {
            if !self.towns[index].visited {
                continue;
            }
            let Some(town) = resources.town_by_index(index) else {
                continue;
            };
            if town.zone.is_empty() {
                continue;
            }
            destinations = destinations.wrapping_add(1);
        }
        if destinations == 0 {
            return AdventureCast::GateNone {
                mode: GateMode::Town,
            };
        }
        AdventureCast::GateArmed {
            mode: GateMode::Town,
            destinations,
        }
    }

    /// Apply the effect of adventure spell `spell_idx`. Ports
    /// `GameApplyAdventureSpellEffect`; [`AdventureCast::applied`] is its
    /// boolean return.
    pub fn apply_adventure_spell_effect(
        &mut self,
        resources: &Resources,
        spell_idx: i32,
    ) -> AdventureCast {
        match spell_adventure_effect(resources, spell_idx) {
            AdventureEffect::Bridge => self.cast_bridge(resources),
            AdventureEffect::TimeStop => self.cast_time_stop(resources),
            AdventureEffect::FindVillain => self.cast_find_villain(resources),
            AdventureEffect::GateCastle => self.cast_castle_gate(resources),
            AdventureEffect::GateTown => self.cast_town_gate(resources),
            AdventureEffect::InstantArmy => self.cast_instant_army(resources),
            AdventureEffect::RaiseControl => self.cast_raise_control(resources),
            AdventureEffect::None => AdventureCast::Unknown,
        }
    }

    /// Cast the spell in catalog slot `spell_idx` from the spellbook. Ports
    /// `dispatch_adventure_spell`.
    pub fn dispatch_adventure_spell(&mut self, resources: &Resources, spell_idx: i32) -> SpellCast {
        let held = usize::try_from(spell_idx)
            .ok()
            .and_then(|index| self.spells.counts.get(index).copied())
            .unwrap_or(0);
        let spell = resources.spell_by_index(spell_idx);
        if let Some(spell) = spell {
            if spell.kind == SpellKind::Combat {
                // A combat spell cannot be cast in the field. Rather than a
                // dead "unavailable", C offers to discard one charge to free
                // a spellbook slot, which `max_spells` otherwise caps.
                if held <= 0 {
                    return SpellCast::NotKnown;
                }
                return SpellCast::OfferDiscard;
            }
        }
        let Some(spell) = spell else {
            return SpellCast::Unavailable;
        };
        if spell.kind != SpellKind::Adventure {
            return SpellCast::Unavailable;
        }
        if held <= 0 {
            return SpellCast::NotKnown;
        }
        SpellCast::Applied(self.apply_adventure_spell_effect(resources, spell_idx))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        spell_adventure_effect, spell_index_by_adventure_effect, try_build_bridge, AdventureCast,
        AdventureEffect, GateMode, SpellCast, GAME_LEADERSHIP_MAX,
    };
    use crate::game::{CastleOwnerKind, Game};
    use crate::map::{Map, Tile};
    use crate::resources::Resources;
    use crate::tile::Terrain;

    /// A catalog carrying every adventure id the effect table names, plus a
    /// combat spell and an adventure spell the table does not know.
    fn spell_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "spells": [
                {"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100},
                {"index":1,"id":"time_stop","name":"Time Stop","kind":"adventure","cost":200},
                {"index":2,"id":"find_villain","name":"Find Villain","kind":"adventure","cost":300},
                {"index":3,"id":"castle_gate","name":"Castle Gate","kind":"adventure","cost":400},
                {"index":4,"id":"town_gate","name":"Town Gate","kind":"adventure","cost":500},
                {"index":5,"id":"instant_army","name":"Instant Army","kind":"adventure","cost":600},
                {"index":6,"id":"raise_control","name":"Raise Control","kind":"adventure","cost":700},
                {"index":7,"id":"fireball","name":"Fireball","kind":"combat","cost":800},
                {"index":8,"id":"moonbeam","name":"Moonbeam","kind":"adventure","cost":900}
              ],
              "troops": [
                {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"},
                {"index":1,"id":"militia","name":"Militia","skill_level":1,"hit_points":2,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":20,"spoils_factor":1,"abilities":"","morale_group":"A","dwelling":"castle"}
              ],
              "classes": [
                {"index":0,"id":"knight","name":"Knight","starting_gold":0,"starting_troops":[],
                 "ranks":[
                   {"id":"general","name":"General","villains_needed":0,"leadership":100,"max_spells":2,"spell_power":1,"commission":10,"knows_magic":false,"instant_army":1},
                   {"id":"captain","name":"Captain","villains_needed":1,"leadership":200,"max_spells":3,"spell_power":2,"commission":20,"knows_magic":true,"instant_army":0}
                 ]}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    fn knight() -> Game {
        let mut game = Game::default();
        game.character.cls.id.copy_from_str("knight");
        game.character.cls.rank_index = 0;
        game.stats.spell_power = 2;
        game
    }

    #[test]
    fn the_effect_table_classifies_by_id_and_only_for_adventure_spells() {
        let resources = spell_fixture();

        assert_eq!(
            spell_adventure_effect(&resources, 0),
            AdventureEffect::Bridge
        );
        assert_eq!(
            spell_adventure_effect(&resources, 6),
            AdventureEffect::RaiseControl
        );
        // A combat spell never classifies, whatever its id.
        assert_eq!(spell_adventure_effect(&resources, 7), AdventureEffect::None);
        // An adventure spell the table does not name is one the engine
        // cannot apply.
        assert_eq!(spell_adventure_effect(&resources, 8), AdventureEffect::None);
        // Out-of-range slots classify as None rather than panicking.
        assert_eq!(
            spell_adventure_effect(&resources, -1),
            AdventureEffect::None
        );
        assert_eq!(
            spell_adventure_effect(&resources, 99),
            AdventureEffect::None
        );
    }

    #[test]
    fn the_reverse_lookup_finds_the_slot_and_refuses_the_none_effect() {
        let resources = spell_fixture();

        assert_eq!(
            spell_index_by_adventure_effect(&resources, AdventureEffect::TimeStop),
            1
        );
        assert_eq!(
            spell_index_by_adventure_effect(&resources, AdventureEffect::None),
            -1
        );
        // An effect no catalog slot carries.
        let bare = Resources::from_json_str(r#"{"spells":[],"troops":[]}"#).expect("parses");
        assert_eq!(
            spell_index_by_adventure_effect(&bare, AdventureEffect::Bridge),
            -1
        );
    }

    #[test]
    fn time_stop_banks_ten_steps_per_spell_power_and_spends_a_charge() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[1] = 2;
        game.stats.time_stop = 5;

        assert_eq!(
            game.cast_time_stop(&resources),
            AdventureCast::TimeStop {
                steps: 20,
                banked: 25
            }
        );
        assert_eq!(game.spells.counts[1], 1);
        assert_eq!(game.stats.time_stop, 25);
    }

    #[test]
    fn time_stop_at_spell_power_zero_spends_the_charge_and_banks_nothing() {
        let resources = spell_fixture();
        let mut game = knight();
        game.stats.spell_power = 0;
        game.spells.counts[1] = 1;

        // GameTimeStopStepsPerCast applies NO minimum: the first Barbarian
        // rank really does get nothing from the spell.
        assert_eq!(
            game.cast_time_stop(&resources),
            AdventureCast::TimeStop {
                steps: 0,
                banked: 0
            }
        );
        assert_eq!(game.spells.counts[1], 0);
    }

    #[test]
    fn a_cast_without_a_charge_does_nothing() {
        let resources = spell_fixture();
        let mut game = knight();
        game.stats.time_stop = 7;

        assert_eq!(game.cast_time_stop(&resources), AdventureCast::NoCharge);
        assert_eq!(game.stats.time_stop, 7);
        assert_eq!(game.spells.counts[1], 0);
    }

    #[test]
    fn raise_control_adds_a_hundred_per_spell_power_and_caps_leadership() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[6] = 1;
        game.stats.leadership_current = 300;

        assert_eq!(
            game.cast_raise_control(&resources),
            AdventureCast::RaiseControl { amount: 200 }
        );
        assert_eq!(game.stats.leadership_current, 500);
    }

    #[test]
    fn raise_control_reports_the_uncapped_amount_the_cap_swallowed() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[6] = 1;
        game.stats.leadership_current = GAME_LEADERSHIP_MAX - 10;

        // The cap clamps the stat, but C's banner reports the raw formula
        // result, so the outcome carries the uncapped amount.
        assert_eq!(
            game.cast_raise_control(&resources),
            AdventureCast::RaiseControl { amount: 200 }
        );
        assert_eq!(game.stats.leadership_current, GAME_LEADERSHIP_MAX);
    }

    #[test]
    fn find_villain_reveals_the_contract_villains_castle() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[2] = 1;
        game.contract.active_id.copy_from_str("murray");
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].owner_kind = CastleOwnerKind::Villain;
        game.castles[0].villain_id.copy_from_str("someone_else");
        game.castles[1].id.copy_from_str("castle1");
        game.castles[1].owner_kind = CastleOwnerKind::Villain;
        game.castles[1].villain_id.copy_from_str("murray");

        assert_eq!(
            game.cast_find_villain(&resources),
            AdventureCast::FindVillain { castle: Some(1) }
        );
        assert!(game.castles[1].known);
        assert!(!game.castles[0].known);
        assert_eq!(game.spells.counts[2], 0);
    }

    #[test]
    fn find_villain_without_a_contract_spends_no_charge() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[2] = 1;

        // The charge guard runs first, but the decrement runs only AFTER the
        // contract check, so a contract-less cast costs nothing.
        assert_eq!(
            game.cast_find_villain(&resources),
            AdventureCast::FindVillainNoContract
        );
        assert_eq!(game.spells.counts[2], 1);
    }

    #[test]
    fn find_villain_spends_the_charge_even_when_no_castle_matches() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[2] = 1;
        game.contract.active_id.copy_from_str("murray");
        // Held by the villain, but not by a VILLAIN-owned castle.
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].owner_kind = CastleOwnerKind::Player;
        game.castles[0].villain_id.copy_from_str("murray");

        assert_eq!(
            game.cast_find_villain(&resources),
            AdventureCast::FindVillain { castle: None }
        );
        assert_eq!(game.spells.counts[2], 0);
        assert!(!game.castles[0].known);
    }

    #[test]
    fn instant_army_conjures_the_rank_troop_and_spends_the_charge() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[5] = 1;

        // rank 0 -> instant_army 1 -> "militia"; multiplier[0] is 3, so
        // (spell_power 2 + 1) * 3 = 9.
        let cast = game.cast_instant_army(&resources);
        let AdventureCast::InstantArmy { troop, count } = cast else {
            panic!("expected a conjured army, got {cast:?}");
        };
        assert_eq!(troop.as_str(), "militia");
        assert_eq!(count, 9);
        assert_eq!(game.army[0].id.as_str(), "militia");
        assert_eq!(game.army[0].count, 9);
        assert_eq!(game.spells.counts[5], 0);
    }

    #[test]
    fn instant_army_keeps_the_charge_when_the_army_is_full() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[5] = 1;
        for slot in game.army.iter_mut() {
            slot.id.copy_from_str("peasants");
            slot.count = 1;
        }

        // A full army of some OTHER troop leaves add_troop nowhere to put the
        // conjured stack, and C returns before the decrement.
        assert_eq!(
            game.cast_instant_army(&resources),
            AdventureCast::InstantArmyNoRoom
        );
        assert_eq!(game.spells.counts[5], 1);
    }

    #[test]
    fn instant_army_fizzles_and_keeps_the_charge_when_the_rank_names_no_troop() {
        let resources = spell_fixture();
        let mut game = knight();
        game.character.cls.id.copy_from_str("no_such_class");
        game.spells.counts[5] = 1;

        assert_eq!(
            game.cast_instant_army(&resources),
            AdventureCast::InstantArmyFizzle
        );
        assert_eq!(game.spells.counts[5], 1);
    }

    #[test]
    fn the_per_cast_count_falls_back_to_rank_zero_for_an_unknown_class() {
        let resources = spell_fixture();
        let mut game = knight();
        game.character.cls.id.copy_from_str("no_such_class");
        game.character.cls.rank_index = 3;

        // GameInstantArmyPerCast does NOT bail on a missing class: it clamps
        // the rank to 0 and answers off multiplier[0] anyway. Only the troop
        // lookup fizzles.
        assert_eq!(game.instant_army_per_cast(&resources), 9);
        assert_eq!(game.instant_army_troop(&resources), None);
    }

    #[test]
    fn the_per_cast_count_is_floored_at_one() {
        let resources = spell_fixture();
        let mut game = knight();
        game.stats.spell_power = -5;

        // (-5 + 1) * 3 is negative; the floor makes it 1.
        assert_eq!(game.instant_army_per_cast(&resources), 1);
    }

    #[test]
    fn a_bridge_converts_at_most_two_water_tiles() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[0] = 1;
        game.position.x = 0;
        game.position.y = 0;
        let mut map =
            Map::from_tiles(6, 1, vec![Tile::plain_grass(); 6]).expect("dimensions match");
        for x in 1..6 {
            map.tile_mut_i32(x, 0).expect("tile exists").terrain = Terrain::Water;
        }

        // The cast only arms the prompt; the direction does the work.
        assert_eq!(game.cast_bridge(&resources), AdventureCast::BridgeArmed);
        // C's loop runs to 5, but `built >= 2` breaks first every time, so
        // the "or up to 5 water tiles" in its comment is unreachable.
        assert_eq!(try_build_bridge(&game, &mut map, 1, 0), 2);
        for x in 1..3 {
            let tile = map.tile_i32(x, 0).expect("tile exists");
            assert_eq!(tile.terrain, Terrain::Grass);
            assert!(tile.is_bridge);
            assert!(!tile.blocks_foot);
        }
        assert_eq!(
            map.tile_i32(3, 0).expect("tile exists").terrain,
            Terrain::Water
        );
    }

    #[test]
    fn a_bridge_stops_at_the_first_non_water_tile_and_at_the_map_edge() {
        let mut game = knight();
        game.position.x = 0;
        game.position.y = 0;
        let mut map =
            Map::from_tiles(4, 1, vec![Tile::plain_grass(); 4]).expect("dimensions match");
        map.tile_mut_i32(1, 0).expect("tile exists").terrain = Terrain::Water;

        // One water tile then grass: the run stops at the grass.
        assert_eq!(try_build_bridge(&game, &mut map, 1, 0), 1);
        // Facing off the map builds nothing at all.
        assert_eq!(try_build_bridge(&game, &mut map, -1, 0), 0);
    }

    #[test]
    fn the_castle_gate_excludes_the_home_castle_and_the_town_gate_does_not() {
        let resources = Resources::from_json_str(
            r#"{
              "spells": [
                {"index":0,"id":"castle_gate","name":"Castle Gate","kind":"adventure","cost":100},
                {"index":1,"id":"town_gate","name":"Town Gate","kind":"adventure","cost":100}
              ],
              "troops": [],
              "castles": [
                {"index":0,"id":"kings_castle","name":"Kings Castle","zone":"home","x":1,"y":1,"special":{"flow":"audience"}},
                {"index":1,"id":"castle1","name":"Castle One","zone":"home","x":3,"y":3}
              ],
              "towns": [
                {"index":0,"id":"town0","name":"Town Zero","zone":"home","x":2,"y":2}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = knight();
        game.spells.counts[0] = 1;
        game.spells.counts[1] = 1;
        game.castles[0].visited = true;
        game.towns[0].visited = true;

        // Only the audience castle has been visited, and the gate spell
        // cannot return there, so there is nowhere to go.
        assert_eq!(
            game.cast_castle_gate(&resources),
            AdventureCast::GateNone {
                mode: GateMode::Castle
            }
        );
        // The town gate has no such exclusion.
        assert_eq!(
            game.cast_town_gate(&resources),
            AdventureCast::GateArmed {
                mode: GateMode::Town,
                destinations: 1
            }
        );

        game.castles[1].visited = true;
        assert_eq!(
            game.cast_castle_gate(&resources),
            AdventureCast::GateArmed {
                mode: GateMode::Castle,
                destinations: 1
            }
        );
        // Arming spends nothing: the charge goes on the chosen teleport.
        assert_eq!(game.spells.counts[0], 1);
    }

    #[test]
    fn the_dispatcher_offers_to_discard_a_held_combat_spell() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[7] = 1;

        // A combat spell cannot be cast in the field; C offers to discard a
        // charge to free a spellbook slot instead.
        assert_eq!(
            game.dispatch_adventure_spell(&resources, 7),
            SpellCast::OfferDiscard
        );
        // The offer alone changes nothing.
        assert_eq!(game.spells.counts[7], 1);
        // Without a charge there is nothing to discard.
        game.spells.counts[7] = 0;
        assert_eq!(
            game.dispatch_adventure_spell(&resources, 7),
            SpellCast::NotKnown
        );
    }

    #[test]
    fn the_dispatcher_separates_an_empty_slot_from_an_unknown_effect() {
        let resources = spell_fixture();
        let mut game = knight();

        // Slot 99 holds no spell at all.
        assert_eq!(
            game.dispatch_adventure_spell(&resources, 99),
            SpellCast::Unavailable
        );
        // "moonbeam" is an adventure spell the effect table does not name,
        // which C reports as a distinct "unknown" rather than "unavailable".
        game.spells.counts[8] = 1;
        assert_eq!(
            game.dispatch_adventure_spell(&resources, 8),
            SpellCast::Applied(AdventureCast::Unknown)
        );
        assert!(!AdventureCast::Unknown.applied());
        assert!(AdventureCast::BridgeArmed.applied());
    }

    #[test]
    fn the_dispatcher_routes_a_known_adventure_spell_to_its_effect() {
        let resources = spell_fixture();
        let mut game = knight();
        game.spells.counts[6] = 1;

        assert_eq!(
            game.dispatch_adventure_spell(&resources, 6),
            SpellCast::Applied(AdventureCast::RaiseControl { amount: 200 })
        );
        assert_eq!(game.stats.leadership_current, 200);
    }
}
