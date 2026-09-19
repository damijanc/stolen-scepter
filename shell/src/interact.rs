//! Presentation for the one-shot pick-up interactions, porting the message
//! halves of `step.c`'s `ir.opened_*` branches.
//!
//! The engine mutates game state and reports a [`StepInteraction`]; for the
//! interactions that are a plain message rather than a prompt or a view, this
//! turns the interaction into the dialog text the shell shows. Interactions
//! that need a decision or a screen -- a chest, a foe, an alcove, a castle,
//! a town, a dwelling -- return `None` and are handled elsewhere.
//!
//! Nothing here touches a graphics library, so the outcome-to-text mapping is
//! testable headlessly the way the engine's combat digests are.

use openbounty_engine::game::{ChestOutcome, ChestRoll, Game};
use openbounty_engine::resources::Resources;
use openbounty_engine::spells_adventure::AdventureCast;
use openbounty_engine::step::StepInteraction;

use crate::banner;

/// Text C compiles in for each pickup. The pack overrides them under
/// `strings.banners` / `strings.dialog_titles`.
const DEFAULT_DT_TELEPORT_CAVE: &str = "Teleport Cave";
const DEFAULT_DT_CRYSTAL_BALL: &str = "Crystal Ball";
const DEFAULT_TELECAVE_TELEPORT: &str =
    "You step into the cave and\nemerge elsewhere on this\ncontinent.";
const DEFAULT_TELECAVE_INERT: &str = "This cave hums with magic,\nbut nothing happens.";
const DEFAULT_NAVMAP_PICKUP: &str = "A navigation map of %ZONE%!";
const DEFAULT_CRYSTAL_BALL_PICKUP: &str = "You gaze into a crystal ball\nand see all of %ZONE%!";
const ARTIFACT_FOOTER: &str = "...and a piece of the map to\nthe stolen scepter.";

/// Chest message templates, one per immediate outcome plus the deferred gold
/// choice. The pack overrides them under `strings.banners`.
const DEFAULT_CHEST_GOLD: &str = "After scouring the area,\n\
    you fall upon a hidden\n\
    treasure cache. You may:\n\n\
    A) Take the %GOLD% gold.\n\
    B) Distribute the gold to\n\
       the peasants, increasing\n\
       your leadership by %LEADERSHIP%.";
const DEFAULT_CHEST_COMMISSION: &str = "After surveying the area,\n\
    you discover that it is\n\
    rich in mineral deposits.\n\n\
    The King rewards you for\n\
    your find by increasing\n\
    your weekly income by %POINTS%";
const DEFAULT_CHEST_SPELL_POWER: &str = "Traversing the area, you\n\
    stumble upon a time worn\n\
    cannister. Curious, you un-\n\
    stop the bottle, releasing\n\
    a powerful genie who raises\n\
    your Spell Power by %POINTS% and\n\
    vanishes.";
const DEFAULT_CHEST_MAX_SPELLS: &str = "A tribe of nomads greet you\n\
    and your army warmly. Their\n\
    shaman, in awe of your\n\
    prowess, teaches you the\n\
    secret of his tribe's magic.\n\
    Your maximum spell capacity\n\
    is increased by %POINTS%";
const DEFAULT_CHEST_NEW_SPELL: &str = "You have captured a\n\
    mischevious imp which has\n\
    been terrorizing the\n\
    region. In exchange for\n\
    its release, you receive:\n\n\
       %COUNT% %SPELL% spell.";
const DEFAULT_CHEST_EMPTY: &str = "The chest was empty!";

/// The Archmage Aurange alcove offer and its results. The pack overrides them
/// under `strings.banners` / `strings.dialog_titles`.
const DEFAULT_DT_ALCOVE_OFFER: &str = "Archmage Aurange";
const DEFAULT_DT_ALCOVE_RESULT: &str = "Aurange";
const DEFAULT_ALCOVE_OFFER: &str = "The venerable Archmage,\n\
    Aurange, will teach you the\n\
    secrets of spell casting for\n\
    %COST% gold.\n\n\
    Accept?";
const DEFAULT_ALCOVE_ALREADY: &str = "The archmage nods.\nYou already know magic.";
const DEFAULT_ALCOVE_TAUGHT: &str = "Aurange teaches you the\n\
    arcane arts. You now know\n\
    the ways of magic.";
const DEFAULT_ALCOVE_NO_GOLD: &str = "You have not enough gold!\n\
    %COST% is required.\n\
    Begone until you do!";

/// Navigation banners, used when the hero sails to another continent.
const DEFAULT_BODY_MUST_BE_SAILING: &str =
    "You must be sailing to\nnavigate to another\ncontinent.";
const DEFAULT_BODY_NO_CONTINENTS: &str = "No known continents from\nthis zone.";
const DEFAULT_BODY_NAVIGATE_ROW: &str = "%INDEX%. %ZONE%\n";
const DEFAULT_DT_NAVIGATE: &str = "Go to which continent?";

/// The dialog (header, body) for a one-shot pick-up, or `None` when the
/// interaction is not a plain message.
pub fn pickup_message(
    resources: &Resources,
    interaction: &StepInteraction,
) -> Option<(String, String)> {
    match interaction {
        StepInteraction::Artifact { id } => {
            let artifact = resources.artifact_by_id(id)?;
            let header = format!(
                "You have found {}!\n{}",
                artifact.name.as_str(),
                artifact.effect.as_str()
            );
            Some((header, ARTIFACT_FOOTER.to_owned()))
        }
        StepInteraction::Orb { zone_index, .. } => {
            // C defaults to zone 0 when it cannot resolve the current zone.
            let zone_index = zone_index.unwrap_or(0);
            let name = resources
                .zone_by_index(zone_index)
                .map(|zone| zone.name.as_str())
                .unwrap_or("");
            let body = banner::formatted(
                resources,
                "banners",
                "crystal_ball_pickup",
                DEFAULT_CRYSTAL_BALL_PICKUP,
                &[("ZONE", name)],
            );
            let header = banner::text(
                resources,
                "dialog_titles",
                "crystal_ball",
                DEFAULT_DT_CRYSTAL_BALL,
            )
            .to_owned();
            Some((header, body))
        }
        StepInteraction::Navmap {
            discovered_zone, ..
        } => {
            // The navmap is consumed silently when every zone is already
            // discovered, which the engine reports as `None`.
            let zone_index = (*discovered_zone)?;
            let zone = resources.zone_by_index(zone_index)?;
            // C prefers the name and falls back to the id, unlike the crystal
            // ball, which uses the name as-is.
            let name = if zone.name.is_empty() {
                zone.id.as_str()
            } else {
                zone.name.as_str()
            };
            let body = banner::formatted(
                resources,
                "banners",
                "navmap_pickup",
                DEFAULT_NAVMAP_PICKUP,
                &[("ZONE", name)],
            );
            Some((String::new(), body))
        }
        StepInteraction::Telecave { destination, .. } => {
            let (key, default) = if destination.is_some() {
                ("telecave_teleport", DEFAULT_TELECAVE_TELEPORT)
            } else {
                ("telecave_inert", DEFAULT_TELECAVE_INERT)
            };
            let body = banner::text(resources, "banners", key, default).to_owned();
            let header = banner::text(
                resources,
                "dialog_titles",
                "teleport_cave",
                DEFAULT_DT_TELEPORT_CAVE,
            )
            .to_owned();
            Some((header, body))
        }
        _ => None,
    }
}

/// The A/B choice body for a chest whose gold outcome is deferred until the
/// player picks gold or leadership. `None` for every other outcome.
pub fn chest_choice(resources: &Resources, roll: &ChestRoll) -> Option<String> {
    if roll.outcome != ChestOutcome::Gold || roll.pending.pending_gold <= 0 {
        return None;
    }
    Some(banner::formatted(
        resources,
        "banners",
        "chest_gold",
        DEFAULT_CHEST_GOLD,
        &[
            ("GOLD", &roll.pending.pending_gold.to_string()),
            ("LEADERSHIP", &roll.pending.pending_leadership.to_string()),
        ],
    ))
}

/// The message for a chest whose reward is already applied, or `None` for the
/// deferred gold choice (and the unused `Leadership` variant).
pub fn chest_message(resources: &Resources, roll: &ChestRoll) -> Option<(String, String)> {
    let body = match roll.outcome {
        ChestOutcome::Commission => banner::formatted(
            resources,
            "banners",
            "chest_commission",
            DEFAULT_CHEST_COMMISSION,
            &[("POINTS", &roll.points.to_string())],
        ),
        ChestOutcome::SpellPower => banner::formatted(
            resources,
            "banners",
            "chest_spell_power",
            DEFAULT_CHEST_SPELL_POWER,
            &[("POINTS", &roll.points.to_string())],
        ),
        ChestOutcome::MaxSpells => banner::formatted(
            resources,
            "banners",
            "chest_max_spells",
            DEFAULT_CHEST_MAX_SPELLS,
            &[("POINTS", &roll.points.to_string())],
        ),
        ChestOutcome::NewSpell => {
            let spell = resources
                .spell_by_id(roll.spell_id.as_str())
                .map(|spell| spell.name.as_str())
                .unwrap_or("(unknown)");
            banner::formatted(
                resources,
                "banners",
                "chest_new_spell",
                DEFAULT_CHEST_NEW_SPELL,
                &[("COUNT", &roll.spell_count.to_string()), ("SPELL", spell)],
            )
        }
        ChestOutcome::Empty => {
            banner::text(resources, "banners", "chest_empty", DEFAULT_CHEST_EMPTY).to_owned()
        }
        _ => return None,
    };
    Some((String::new(), body))
}

/// The alcove offer, named after its archmage and quoting the cost.
pub fn alcove_offer(resources: &Resources) -> (String, String) {
    let header = banner::text(
        resources,
        "dialog_titles",
        "alcove_offer",
        DEFAULT_DT_ALCOVE_OFFER,
    )
    .to_owned();
    let cost = resources.economy.alcove_cost.to_string();
    let body = banner::formatted(
        resources,
        "banners",
        "alcove_offer",
        DEFAULT_ALCOVE_OFFER,
        &[("COST", &cost)],
    );
    (header, body)
}

/// The "you already know magic" brush-off, shown without a header.
pub fn alcove_already(resources: &Resources) -> (String, String) {
    (
        String::new(),
        banner::text(
            resources,
            "banners",
            "alcove_already",
            DEFAULT_ALCOVE_ALREADY,
        )
        .to_owned(),
    )
}

/// The success message after magic is granted.
pub fn alcove_taught(resources: &Resources) -> (String, String) {
    let header = banner::text(
        resources,
        "dialog_titles",
        "alcove_result",
        DEFAULT_DT_ALCOVE_RESULT,
    )
    .to_owned();
    let body =
        banner::text(resources, "banners", "alcove_taught", DEFAULT_ALCOVE_TAUGHT).to_owned();
    (header, body)
}

/// The refusal when the hero cannot afford the cost.
pub fn alcove_no_gold(resources: &Resources) -> (String, String) {
    let header = banner::text(
        resources,
        "dialog_titles",
        "alcove_result",
        DEFAULT_DT_ALCOVE_RESULT,
    )
    .to_owned();
    let cost = resources.economy.alcove_cost.to_string();
    let body = banner::formatted(
        resources,
        "banners",
        "alcove_no_gold",
        DEFAULT_ALCOVE_NO_GOLD,
        &[("COST", &cost)],
    );
    (header, body)
}

/// The discovered neighbor zones the hero can sail to, as `(id, name)` pairs
/// in pack order. C caps the list at five, the numeric prompt's ceiling.
pub fn navigate_targets(resources: &Resources, game: &Game) -> Vec<(String, String)> {
    let Some(current) = resources.zone_by_id(game.position.zone.as_str()) else {
        return Vec::new();
    };
    let mut targets = Vec::new();
    for slot in 0..current.neighbor_count {
        let neighbor_id = current.neighbors[slot].as_str();
        let Some(neighbor) = resources.zone_by_id(neighbor_id) else {
            continue;
        };
        let Some(index) = resources.zone_index_by_id(neighbor_id) else {
            continue;
        };
        if index >= game.world.zones_discovered.len() || !game.world.zones_discovered[index] {
            continue;
        }
        if targets.len() >= 5 {
            break;
        }
        let name = if neighbor.name.is_empty() {
            neighbor.id.as_str()
        } else {
            neighbor.name.as_str()
        };
        targets.push((neighbor_id.to_owned(), name.to_owned()));
    }
    targets
}

/// The numbered body listing the sailable continents, and the prompt title.
pub fn navigate_body(resources: &Resources, targets: &[(String, String)]) -> (String, String) {
    let mut body = String::new();
    for (index, (_, name)) in targets.iter().enumerate() {
        body.push_str(&banner::formatted(
            resources,
            "banners",
            "body_navigate_row",
            DEFAULT_BODY_NAVIGATE_ROW,
            &[("INDEX", &(index + 1).to_string()), ("ZONE", name)],
        ));
    }
    let header =
        banner::text(resources, "dialog_titles", "navigate", DEFAULT_DT_NAVIGATE).to_owned();
    (header, body)
}

/// The "you must be sailing" refusal, shown without a header.
pub fn must_be_sailing(resources: &Resources) -> (String, String) {
    (
        String::new(),
        banner::text(
            resources,
            "banners",
            "body_must_be_sailing",
            DEFAULT_BODY_MUST_BE_SAILING,
        )
        .to_owned(),
    )
}

/// The "no known continents" refusal, shown without a header.
pub fn no_continents(resources: &Resources) -> (String, String) {
    (
        String::new(),
        banner::text(
            resources,
            "banners",
            "body_no_continents",
            DEFAULT_BODY_NO_CONTINENTS,
        )
        .to_owned(),
    )
}

/// Spell banner defaults, porting `load_banner_defaults`. The pack overrides
/// them under `strings.banners`.
const DEFAULT_SPELL_UNAVAILABLE: &str = "That spell is not available.";
const DEFAULT_SPELL_NOT_KNOWN: &str = "You don't have that spell.";
const DEFAULT_SPELL_UNKNOWN: &str = "Unknown spell.";
const DEFAULT_SPELL_TIME_STOP: &str = "Time has stopped for %STEPS% steps.";
const DEFAULT_SPELL_FIND_VILLAIN_NO_CONTRACT: &str = "You have no contract.";
const DEFAULT_SPELL_FIND_VILLAIN_SUCCESS: &str = "You have found %CASTLE%!";
const DEFAULT_SPELL_FIND_VILLAIN_NONE: &str = "Your search reveals nothing.";
const DEFAULT_SPELL_BRIDGE_PROMPT: &str = "Build bridge in which\ndirection? Use arrows.";
const DEFAULT_SPELL_BRIDGE_BUILT: &str = "Bridge constructed with\n%COUNT% tiles.";
const DEFAULT_SPELL_BRIDGE_INVALID: &str = "Not a suitable location\nfor a bridge.";
const DEFAULT_SPELL_CASTLE_GATE_NONE: &str = "You have not visited\nany castles yet.";
const DEFAULT_SPELL_TOWN_GATE_NONE: &str = "You have not visited\nany towns yet.";
const DEFAULT_SPELL_INSTANT_ARMY_FIZZLE: &str = "Spell fizzles.";
const DEFAULT_SPELL_INSTANT_ARMY_NO_ROOM: &str = "Your army has no room!";
const DEFAULT_SPELL_INSTANT_ARMY_SUCCESS: &str = "%QTY% %TROOP%\nhave joined your army.";
const DEFAULT_SPELL_RAISE_CONTROL_SUCCESS: &str = "Your leadership has\nincreased by %AMOUNT%.";
const DEFAULT_NO_SPELL_BANNER: &str = "You have not been trained in\n\
    the art of spellcasting yet.\n\
    Visit the Archmage Aurange\n\
    in Continentia at 11,19 for\n\
    this ability.";

/// A plain spell-refusal message body, keyed by its banner name. Used for the
/// `spell_not_known` and `spell_unavailable` refusals, which have no header.
pub fn spell_banner<'a>(resources: &'a Resources, key: &str) -> &'a str {
    let default = match key {
        "spell_not_known" => DEFAULT_SPELL_NOT_KNOWN,
        "spell_unavailable" => DEFAULT_SPELL_UNAVAILABLE,
        "spell_unknown" => DEFAULT_SPELL_UNKNOWN,
        _ => "",
    };
    banner::text(resources, "banners", key, default)
}

/// A spell's display name, C's `spell_header`: the catalog name when the id
/// resolves and is non-empty, else the fallback the caller supplies.
pub fn spell_header(resources: &Resources, id: &str, fallback: &str) -> String {
    resources
        .spell_by_id(id)
        .map(|spell| {
            if spell.name.is_empty() {
                fallback
            } else {
                spell.name.as_str()
            }
        })
        .unwrap_or(fallback)
        .to_owned()
}

/// The "you have not been trained in magic" banner for the U action, porting
/// the shell's no-spell branch of `INPUT_ACTION_CAST_SPELL`. The alcove
/// coordinates are substituted from the first zone that declares one.
pub fn no_spell_banner(resources: &Resources) -> (String, String) {
    let mut zone_label = "";
    let mut alcove_x = 0;
    let mut alcove_y = 0;
    for index in 0..resources.zone_count() {
        if let Some(zone) = resources.zone_by_index(index) {
            if zone.magic_alcove_x >= 0 && zone.magic_alcove_y >= 0 {
                zone_label = if zone.name.is_empty() {
                    zone.id.as_str()
                } else {
                    zone.name.as_str()
                };
                alcove_x = zone.magic_alcove_x;
                alcove_y = zone.magic_alcove_y;
                break;
            }
        }
    }
    let x = alcove_x.to_string();
    let y = alcove_y.to_string();
    let body = banner::formatted(
        resources,
        "banners",
        "no_spell_banner",
        DEFAULT_NO_SPELL_BANNER,
        &[("ZONE", zone_label), ("X", &x), ("Y", &y)],
    );
    (String::new(), body)
}

/// Turn an adventure-cast outcome into the `(header, body)` dialog it presents,
/// porting the `player_io_message` calls in each C `cast_*`.
///
/// Returns `None` for the outcomes that need more than a dialog: `NoCharge`
/// (silent), `BridgeArmed` (a direction prompt) and `GateArmed` (a destination
/// picker). The caller routes those.
pub fn adventure_cast_message(
    resources: &Resources,
    game: &Game,
    cast: &AdventureCast,
) -> Option<(String, String)> {
    let spell = |id, fallback| spell_header(resources, id, fallback);
    let text = |key, default| banner::text(resources, "banners", key, default).to_owned();
    match cast {
        AdventureCast::Unknown => {
            Some((String::new(), text("spell_unknown", DEFAULT_SPELL_UNKNOWN)))
        }
        AdventureCast::NoCharge => None,
        AdventureCast::TimeStop { banked, .. } => {
            let steps = banked.to_string();
            Some((
                spell("time_stop", "Time Stop"),
                banner::formatted(
                    resources,
                    "banners",
                    "spell_time_stop",
                    DEFAULT_SPELL_TIME_STOP,
                    &[("STEPS", &steps)],
                ),
            ))
        }
        AdventureCast::FindVillainNoContract => Some((
            spell("find_villain", "Find Villain"),
            text(
                "spell_find_villain_no_contract",
                DEFAULT_SPELL_FIND_VILLAIN_NO_CONTRACT,
            ),
        )),
        AdventureCast::FindVillain { castle } => {
            let body = match castle {
                Some(index) => {
                    let label = game
                        .castles
                        .get(*index)
                        .map(|record| record.id.as_str())
                        .and_then(|id| resources.castle_by_id(id))
                        .map(|castle| {
                            if castle.name.is_empty() {
                                castle.id.as_str()
                            } else {
                                castle.name.as_str()
                            }
                        })
                        .unwrap_or("");
                    banner::formatted(
                        resources,
                        "banners",
                        "spell_find_villain_success",
                        DEFAULT_SPELL_FIND_VILLAIN_SUCCESS,
                        &[("CASTLE", label)],
                    )
                }
                None => text("spell_find_villain_none", DEFAULT_SPELL_FIND_VILLAIN_NONE),
            };
            Some((spell("find_villain", "Find Villain"), body))
        }
        AdventureCast::BridgeArmed => None,
        AdventureCast::GateNone { mode } => {
            let (key, default, id, fallback) = match mode {
                openbounty_engine::spells_adventure::GateMode::Castle => (
                    "spell_castle_gate_none",
                    DEFAULT_SPELL_CASTLE_GATE_NONE,
                    "castle_gate",
                    "Castle Gate",
                ),
                openbounty_engine::spells_adventure::GateMode::Town => (
                    "spell_town_gate_none",
                    DEFAULT_SPELL_TOWN_GATE_NONE,
                    "town_gate",
                    "Town Gate",
                ),
            };
            Some((spell(id, fallback), text(key, default)))
        }
        AdventureCast::GateArmed { .. } => None,
        AdventureCast::InstantArmyFizzle => Some((
            spell("instant_army", "Instant Army"),
            text(
                "spell_instant_army_fizzle",
                DEFAULT_SPELL_INSTANT_ARMY_FIZZLE,
            ),
        )),
        AdventureCast::InstantArmyNoRoom => Some((
            spell("instant_army", "Instant Army"),
            text(
                "spell_instant_army_no_room",
                DEFAULT_SPELL_INSTANT_ARMY_NO_ROOM,
            ),
        )),
        AdventureCast::InstantArmy { troop, count } => {
            let troop_name = resources
                .troop_by_id(troop.as_str())
                .map(|t| t.name.as_str())
                .unwrap_or("");
            let qty = count.to_string();
            // DIVERGENCE: C renders %QTY% through its fuzzy count-bucket
            // labels, which this engine does not port; the raw count stands in.
            Some((
                spell("instant_army", "Instant Army"),
                banner::formatted(
                    resources,
                    "banners",
                    "spell_instant_army_success",
                    DEFAULT_SPELL_INSTANT_ARMY_SUCCESS,
                    &[("QTY", &qty), ("TROOP", troop_name)],
                ),
            ))
        }
        AdventureCast::RaiseControl { amount } => {
            let amount = amount.to_string();
            Some((
                spell("raise_control", "Raise Control"),
                banner::formatted(
                    resources,
                    "banners",
                    "spell_raise_control_success",
                    DEFAULT_SPELL_RAISE_CONTROL_SUCCESS,
                    &[("AMOUNT", &amount)],
                ),
            ))
        }
    }
}

/// The bridge-cast prompt, shown while the shell waits for a direction. The
/// charge is NOT spent here; the caller spends it once a bridge is built.
pub fn bridge_prompt(resources: &Resources) -> (String, String) {
    (
        spell_header(resources, "bridge", "Bridge"),
        banner::text(
            resources,
            "banners",
            "spell_bridge_prompt",
            DEFAULT_SPELL_BRIDGE_PROMPT,
        )
        .to_owned(),
    )
}

/// The bridge-built message, porting the `spell_bridge_built` banner with its
/// `%COUNT%` substitution.
pub fn bridge_built(resources: &Resources, count: i32) -> (String, String) {
    let count = count.to_string();
    (
        spell_header(resources, "bridge", "Bridge"),
        banner::formatted(
            resources,
            "banners",
            "spell_bridge_built",
            DEFAULT_SPELL_BRIDGE_BUILT,
            &[("COUNT", &count)],
        ),
    )
}

/// The bridge-invalid message, shown when the chosen direction has no water.
pub fn bridge_invalid(resources: &Resources) -> (String, String) {
    (
        spell_header(resources, "bridge", "Bridge"),
        banner::text(
            resources,
            "banners",
            "spell_bridge_invalid",
            DEFAULT_SPELL_BRIDGE_INVALID,
        )
        .to_owned(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::game::{ChestOutcome, ChestRoll, Game};
    use openbounty_engine::step::StepInteraction;

    fn resources(zones: &str) -> Resources {
        Resources::from_json_str(&format!(
            r#"{{"render": {{"mode": "legacy"}}, "troops": [], "zones": {zones},
                "artifacts": [
                  {{"index":0,"id":"sword","name":"Sword of Prowess","icon":"s.png","power":"increased_damage","effect":"More damage"}}
                ]}}"#
        ))
        .expect("fixture parses")
    }

    fn zones_fixture() -> &'static str {
        r#"[
          {"id":"home","name":"Continentia"},
          {"id":"away","name":"Forestria"}
        ]"#
    }

    #[test]
    fn an_artifact_names_the_relic_and_its_effect() {
        let resources = resources(zones_fixture());
        let interaction = StepInteraction::Artifact {
            id: "sword".to_owned(),
        };
        let (header, body) = pickup_message(&resources, &interaction).expect("a message");
        assert_eq!(header, "You have found Sword of Prowess!\nMore damage");
        assert_eq!(body, "...and a piece of the map to\nthe stolen scepter.");
    }

    #[test]
    fn an_unknown_artifact_yields_no_message() {
        let resources = resources(zones_fixture());
        let interaction = StepInteraction::Artifact {
            id: "missing".to_owned(),
        };
        assert_eq!(pickup_message(&resources, &interaction), None);
    }

    #[test]
    fn a_crystal_ball_names_the_current_zone() {
        let resources = resources(zones_fixture());
        let interaction = StepInteraction::Orb {
            id: "orb_0".to_owned(),
            zone_index: Some(0),
        };
        let (header, body) = pickup_message(&resources, &interaction).expect("a message");
        assert_eq!(header, "Crystal Ball");
        assert_eq!(
            body,
            "You gaze into a crystal ball\nand see all of Continentia!"
        );
    }

    #[test]
    fn a_navmap_names_the_discovered_zone() {
        let resources = resources(zones_fixture());
        let interaction = StepInteraction::Navmap {
            id: "navmap_1".to_owned(),
            discovered_zone: Some(1),
        };
        let (header, body) = pickup_message(&resources, &interaction).expect("a message");
        assert!(header.is_empty());
        assert_eq!(body, "A navigation map of Forestria!");
    }

    #[test]
    fn a_navmap_with_no_undiscovered_zone_is_silent() {
        let resources = resources(zones_fixture());
        let interaction = StepInteraction::Navmap {
            id: "navmap_1".to_owned(),
            discovered_zone: None,
        };
        assert_eq!(pickup_message(&resources, &interaction), None);
    }

    #[test]
    fn a_telecave_that_teleports_announces_the_jump() {
        let resources = resources(zones_fixture());
        let teleport = StepInteraction::Telecave {
            id: "cave_0".to_owned(),
            destination: Some((5, 5)),
        };
        let (header, body) = pickup_message(&resources, &teleport).expect("a message");
        assert_eq!(header, "Teleport Cave");
        assert_eq!(
            body,
            "You step into the cave and\nemerge elsewhere on this\ncontinent."
        );

        let inert = StepInteraction::Telecave {
            id: "cave_0".to_owned(),
            destination: None,
        };
        let (_, body) = pickup_message(&resources, &inert).expect("a message");
        assert_eq!(body, "This cave hums with magic,\nbut nothing happens.");
    }

    #[test]
    fn the_pack_may_reword_a_pickup() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "zones": [{"id":"home","name":"Continentia"}],
                "strings": {"banners": {"crystal_ball_pickup": "All of %ZONE% revealed."}}}"#,
        )
        .expect("fixture parses");
        let interaction = StepInteraction::Orb {
            id: "orb_0".to_owned(),
            zone_index: Some(0),
        };
        let (_, body) = pickup_message(&resources, &interaction).expect("a message");
        assert_eq!(body, "All of Continentia revealed.");
    }

    #[test]
    fn interactions_that_open_a_prompt_or_view_are_not_messages() {
        let resources = resources(zones_fixture());
        // A chest, foe, alcove, castle, town and dwelling are not one-shot
        // messages; they need a decision or a screen.
        for interaction in [
            StepInteraction::TreasureChest {
                id: "chest_0".to_owned(),
                roll: openbounty_engine::game::ChestRoll::default(),
            },
            StepInteraction::Foe {
                id: "foe_0".to_owned(),
                friendly: false,
            },
            StepInteraction::Alcove,
            StepInteraction::CastleGate {
                id: "castle_0".to_owned(),
            },
            StepInteraction::Town {
                id: "town_0".to_owned(),
                boat_x: 0,
                boat_y: 0,
            },
            StepInteraction::Sign {
                id: "sign_0".to_owned(),
                title: "T".to_owned(),
                body: "B".to_owned(),
            },
        ] {
            assert_eq!(pickup_message(&resources, &interaction), None);
        }
    }

    fn chest_roll(outcome: ChestOutcome) -> ChestRoll {
        ChestRoll {
            outcome,
            ..ChestRoll::default()
        }
    }

    #[test]
    fn a_gold_chest_offers_the_choice_and_no_message() {
        let resources = resources(zones_fixture());
        let roll = ChestRoll {
            outcome: ChestOutcome::Gold,
            pending: openbounty_engine::game::ChestPending {
                pending_gold: 1200,
                pending_leadership: 24,
            },
            ..ChestRoll::default()
        };
        let body = chest_choice(&resources, &roll).expect("a choice");
        assert!(body.contains("A) Take the 1200 gold."));
        assert!(body.contains("your leadership by 24."));
        assert_eq!(chest_message(&resources, &roll), None);
    }

    #[test]
    fn a_commission_chest_reports_the_income_increase() {
        let resources = resources(zones_fixture());
        let roll = ChestRoll {
            outcome: ChestOutcome::Commission,
            points: 7,
            ..ChestRoll::default()
        };
        assert_eq!(chest_choice(&resources, &roll), None);
        let (header, body) = chest_message(&resources, &roll).expect("a message");
        assert!(header.is_empty());
        assert!(body.contains("weekly income by 7"));
    }

    #[test]
    fn a_new_spell_chest_names_the_spell() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "zones": [{"id":"home","name":"Continentia"}],
                "spells": [{"index":0,"id":"fireball","name":"Fireball","kind":"combat","cost":50}]}"#,
        )
        .expect("fixture parses");
        let roll = ChestRoll {
            outcome: ChestOutcome::NewSpell,
            spell_count: 2,
            spell_id: {
                let mut id = openbounty_engine::tables::FixedStr::empty();
                id.copy_from_str("fireball");
                id
            },
            ..ChestRoll::default()
        };
        let (_, body) = chest_message(&resources, &roll).expect("a message");
        assert!(body.contains("2 Fireball spell."));
    }

    #[test]
    fn an_empty_chest_is_a_message_not_a_choice() {
        let resources = resources(zones_fixture());
        let roll = chest_roll(ChestOutcome::Empty);
        assert_eq!(chest_choice(&resources, &roll), None);
        let (_, body) = chest_message(&resources, &roll).expect("a message");
        assert_eq!(body, "The chest was empty!");
    }

    #[test]
    fn the_alcove_offer_quotes_the_cost() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [], "economy": {"alcove_cost": 5000},
                "zones": [{"id":"home"}]}"#,
        )
        .expect("fixture parses");
        let (header, body) = alcove_offer(&resources);
        assert_eq!(header, "Archmage Aurange");
        assert!(body.contains("5000 gold."));
        assert!(body.ends_with("Accept?"));

        // The taught, no-gold and already messages carry the result title.
        let (th_header, th_body) = alcove_taught(&resources);
        assert_eq!(th_header, "Aurange");
        assert!(th_body.contains("the ways of magic"));
        let (ng_header, ng_body) = alcove_no_gold(&resources);
        assert_eq!(ng_header, "Aurange");
        assert!(ng_body.contains("5000 is required"));
        let (al_header, al_body) = alcove_already(&resources);
        assert!(al_header.is_empty());
        assert!(al_body.contains("already know magic"));
    }

    #[test]
    fn navigate_targets_lists_only_discovered_neighbors() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "zones": [
                  {"id":"home","neighbors":["away","far","undiscovered"]},
                  {"id":"away","name":"Forestria"},
                  {"id":"far","name":"Saharia"},
                  {"id":"undiscovered"}
                ]}"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.world.zones_discovered[1] = true;
        game.world.zones_discovered[2] = true;

        let targets = navigate_targets(&resources, &game);
        assert_eq!(
            targets,
            vec![
                ("away".to_owned(), "Forestria".to_owned()),
                ("far".to_owned(), "Saharia".to_owned()),
            ]
        );

        let (header, body) = navigate_body(&resources, &targets);
        assert_eq!(header, "Go to which continent?");
        assert!(body.contains("1. Forestria"));
        assert!(body.contains("2. Saharia"));
    }

    fn spell_resources() -> Resources {
        Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"ogres","name":"Ogres","skill_level":1,"hit_points":1,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":1,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ],
               "zones": [
                 {"id":"home","name":"Continentia","magic_alcove":{"x":11,"y":19}}
               ],
               "spells": [
                 {"id":"time_stop","name":"Time Stop","kind":"adventure"}
               ]}"#,
        )
        .expect("fixture parses")
    }

    #[test]
    fn spell_header_resolves_a_name_and_falls_back() {
        let resources = spell_resources();
        assert_eq!(spell_header(&resources, "time_stop", "TS"), "Time Stop");
        assert_eq!(spell_header(&resources, "missing", "TS"), "TS");
    }

    #[test]
    fn no_spell_banner_substitutes_the_alcove_location() {
        let resources = spell_resources();
        let (header, body) = no_spell_banner(&resources);
        assert!(header.is_empty());
        assert!(body.contains("Continentia"));
        assert!(body.contains("11"));
        assert!(body.contains("19"));
    }

    #[test]
    fn time_stop_reports_the_banked_steps() {
        let resources = spell_resources();
        let game = Game::default();
        let (header, body) = adventure_cast_message(
            &resources,
            &game,
            &AdventureCast::TimeStop {
                steps: 3,
                banked: 7,
            },
        )
        .expect("a dialog");
        assert_eq!(header, "Time Stop");
        assert!(body.contains("7"));
    }

    #[test]
    fn find_villain_without_a_castle_reads_nothing() {
        let resources = spell_resources();
        let game = Game::default();
        let (header, body) = adventure_cast_message(
            &resources,
            &game,
            &AdventureCast::FindVillain { castle: None },
        )
        .expect("a dialog");
        assert_eq!(header, "Find Villain");
        assert!(body.contains("nothing"));
    }

    #[test]
    fn bridge_and_gate_arms_are_not_dialogs() {
        let resources = spell_resources();
        let game = Game::default();
        assert_eq!(
            adventure_cast_message(&resources, &game, &AdventureCast::BridgeArmed),
            None
        );
        assert_eq!(
            adventure_cast_message(
                &resources,
                &game,
                &AdventureCast::GateArmed {
                    mode: openbounty_engine::spells_adventure::GateMode::Town,
                    destinations: 3,
                }
            ),
            None
        );
    }

    #[test]
    fn a_gate_with_no_destinations_reads_the_refusal() {
        let resources = spell_resources();
        let game = Game::default();
        let (header, body) = adventure_cast_message(
            &resources,
            &game,
            &AdventureCast::GateNone {
                mode: openbounty_engine::spells_adventure::GateMode::Town,
            },
        )
        .expect("a dialog");
        assert_eq!(header, "Town Gate");
        assert!(body.contains("towns"));
    }

    #[test]
    fn the_bridge_messages_use_the_bridge_header() {
        let resources = spell_resources();

        let (header, body) = bridge_prompt(&resources);
        assert_eq!(header, "Bridge");
        assert!(body.contains("direction"));

        let (header, body) = bridge_built(&resources, 2);
        assert_eq!(header, "Bridge");
        assert!(body.contains("2"));

        let (header, body) = bridge_invalid(&resources);
        assert_eq!(header, "Bridge");
        assert!(body.contains("suitable"));
    }
}
