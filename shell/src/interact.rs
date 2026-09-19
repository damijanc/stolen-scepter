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

use openbounty_engine::game::{
    AudienceOutcome, BoatActionOutcome, CastleOwnerKind, ChestOutcome, ChestRoll, FriendlyOffer,
    Game, GarrisonOutcome, RecruitOutcome, SiegeBuyOutcome, SpellBuyOutcome, VillainSiegeOutcome,
};
use openbounty_engine::resources::Resources;
use openbounty_engine::spells_adventure::AdventureCast;
use openbounty_engine::step::StepInteraction;
use openbounty_engine::tile::Interact;

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

/// Town banner defaults, porting `load_banner_defaults`. The pack overrides
/// them under `strings.banners`.
const DEFAULT_TOWN_HEADER: &str = "Town of %NAME%";
const DEFAULT_TOWN_GOLD_LABEL: &str = "GP=%GOLD%K";
const DEFAULT_TOWN_ROW_CONTRACT: &str = "A) Get New Contract";
const DEFAULT_TOWN_ROW_BOAT_RENT: &str = "B) Rent boat (%COST% week)";
const DEFAULT_TOWN_ROW_BOAT_CANCEL: &str = "B) Cancel boat rental";
const DEFAULT_TOWN_ROW_INFO: &str = "C) Gather information";
const DEFAULT_TOWN_ROW_SPELL: &str = "D) %SPELL% spell (%SPELL_COST%)";
const DEFAULT_TOWN_ROW_SPELL_NONE: &str = "D) (no spell available)";
const DEFAULT_TOWN_ROW_SIEGE_BUY: &str = "E) Buy seige weapons (%SIEGE_COST%)";
const DEFAULT_TOWN_ROW_SIEGE_OWNED: &str = "E) Siege weapons (owned)";
const DEFAULT_TOWN_CONTRACT_NEW: &str =
    "New contract: %VILLAIN%.\nReward: %REWARD% gold.\nLast seen on %ZONE%.";
const DEFAULT_TOWN_CONTRACT_NONE: &str = "No contracts are available right now.";
const DEFAULT_TOWN_BOAT_VACATE_FIRST: &str = "\n\nPlease vacate the boat first";
const DEFAULT_TOWN_NO_GOLD: &str = "\n\n\nYou don't have enough gold!";
const DEFAULT_TOWN_INTEL_UNAVAILABLE: &str = "No intelligence is available here.";
const DEFAULT_TOWN_INTEL_CASTLE_UNDER: &str = "Castle %NAME% is under\n";
const DEFAULT_TOWN_INTEL_OWNER_RULE: &str = "%OWNER%'s rule.\n\n";
const DEFAULT_TOWN_INTEL_OWNER_NONE: &str = "no one";
const DEFAULT_TOWN_INTEL_OWNER_PLAYER: &str = "your";
const DEFAULT_TOWN_INTEL_OWNER_KING: &str = "the King";
const DEFAULT_TOWN_INTEL_COUNT_NAMED: &str = "  %LABEL% %TROOP%\n";
const DEFAULT_TOWN_INTEL_COUNT_NUMERIC: &str = "  %COUNT% %TROOP%\n";
const DEFAULT_TOWN_INTEL_MONSTERS_GENERIC: &str =
    "  Various groups of monsters\n  occupy the castle.";
const DEFAULT_TOWN_INTEL_NO_GARRISON: &str = "  (no garrison)";
const DEFAULT_TOWN_SPELL_UNAVAILABLE: &str = "No spell is available here.";
const DEFAULT_TOWN_SPELL_AT_CAP: &str = "You have learned your maximum number of spells.";
const DEFAULT_TOWN_SPELL_CAN_LEARN: &str = "You can learn %LEFT% more spell%S%.";
const DEFAULT_TOWN_SIEGE_ALREADY: &str = "You have siege weapons!";
const DEFAULT_TOWN_SIEGE_PURCHASED: &str = "Siege weapons purchased.";

/// Castle screen defaults, porting `load_banner_defaults` and the `ui` block.
const DEFAULT_BODY_HOME_CASTLE: &str = "1. Recruit Soldiers\n2. Audience with the King";
const DEFAULT_OWN_CASTLE_MODE_GARRISON: &str = "Garrison troops (Space=Remove)";
const DEFAULT_OWN_CASTLE_MODE_REMOVE: &str = "Remove troops (Space=Garrison)";
const DEFAULT_CANNOT_GARRISON_LAST: &str = "You cannot garrison your\nlast army!";
const DEFAULT_NO_TROOP_SLOTS: &str = "No troop slots left!";
const DEFAULT_RECRUIT_SOLDIERS_TITLE: &str = "Recruit Soldiers";
const DEFAULT_RECRUIT_SOLDIERS_HOW_MANY: &str = "How Many";
const DEFAULT_AUDIENCE_INTRO: &str = "Trumpets announce your\narrival with regal fanfare.\n\nKing Maximus rises from his\nthrone to greet you and\nproclaims:           (space)";
const DEFAULT_DWELLING_RECRUIT_PROMPT: &str =
    "%COUNT% %TROOP% are available\nCost=%COST% each.  GP=%GOLD%\nYou may recruit up to %CAP%.";
const DEFAULT_DWELLING_NONE_THIS_WEEK: &str = "No troops are available here\nthis week.";
const DEFAULT_DWELLING_EMPTY: &str = "This dwelling is empty.";
const DEFAULT_DWELLING_KIND_PLAINS: &str = "Plains";
const DEFAULT_DWELLING_KIND_FOREST: &str = "Forest";
const DEFAULT_DWELLING_KIND_HILL: &str = "Hill";
const DEFAULT_DWELLING_KIND_DUNGEON: &str = "Dungeon";
const DEFAULT_DWELLING_HOW_MANY: &str = "Recruit how many";
const DEFAULT_ENCOUNTER_JOIN_NAMED: &str = "You encounter:\n  %LABEL% %TROOP%\n\n\
    They offer to join your\narmy for %COST% gold.\n\n\
    Press Enter to accept, Esc\nto refuse.";
const DEFAULT_ENCOUNTER_JOIN_NUMERIC: &str = "You encounter:\n  %COUNT% %TROOP%\n\n\
    They offer to join your\narmy for %COST% gold.\n\n\
    Press Enter to accept, Esc\nto refuse.";
const DEFAULT_ENCOUNTER_WANDERERS: &str = "A band of wanderers passes\nby, looking grim.";

/// The town header and gold label, used by the town menu panel.
pub fn town_header_text(resources: &Resources, name: &str) -> String {
    banner::formatted(
        resources,
        "banners",
        "town_header",
        DEFAULT_TOWN_HEADER,
        &[("NAME", name)],
    )
}

/// The right-aligned "GP=%GOLD%K" label, porting `town_gold_label`.
pub fn town_gold_text(resources: &Resources, gold: i32) -> String {
    let gold_k = (gold / 1000).to_string();
    banner::formatted(
        resources,
        "banners",
        "town_gold_label",
        DEFAULT_TOWN_GOLD_LABEL,
        &[("GOLD", &gold_k)],
    )
}

/// One town action row's label, porting `town_format_row`. `row` is 0..4:
/// contract, boat, info, spell, siege.
pub fn town_row_text(resources: &Resources, game: &Game, record_key: &str, row: i32) -> String {
    let text = |key, default| banner::text(resources, "banners", key, default);
    match row {
        0 => text("town_row_contract", DEFAULT_TOWN_ROW_CONTRACT).to_owned(),
        1 => {
            if game.boat.has_boat {
                text("town_row_boat_cancel", DEFAULT_TOWN_ROW_BOAT_CANCEL).to_owned()
            } else {
                let cost = game.boat_cost(resources).to_string();
                banner::formatted(
                    resources,
                    "banners",
                    "town_row_boat_rent",
                    DEFAULT_TOWN_ROW_BOAT_RENT,
                    &[("COST", &cost)],
                )
            }
        }
        2 => text("town_row_info", DEFAULT_TOWN_ROW_INFO).to_owned(),
        3 => {
            let spell_for_sale = game
                .towns
                .iter()
                .find(|town| town.id.as_str() == record_key)
                .map(|town| town.spell_for_sale.as_str())
                .unwrap_or("");
            match resources.spell_by_id(spell_for_sale) {
                Some(spell) if !spell_for_sale.is_empty() => {
                    let cost = spell.cost.to_string();
                    banner::formatted(
                        resources,
                        "banners",
                        "town_row_spell",
                        DEFAULT_TOWN_ROW_SPELL,
                        &[("SPELL", spell.name.as_str()), ("SPELL_COST", &cost)],
                    )
                }
                _ => text("town_row_spell_none", DEFAULT_TOWN_ROW_SPELL_NONE).to_owned(),
            }
        }
        4 => {
            if game.stats.siege_weapons != 0 {
                text("town_row_siege_owned", DEFAULT_TOWN_ROW_SIEGE_OWNED).to_owned()
            } else {
                let cost = resources.economy.siege_cost.to_string();
                banner::formatted(
                    resources,
                    "banners",
                    "town_row_siege_buy",
                    DEFAULT_TOWN_ROW_SIEGE_BUY,
                    &[("SIEGE_COST", &cost)],
                )
            }
        }
        _ => String::new(),
    }
}

/// The contract toast, porting `town_do_contract`. `villain_id` is the id
/// `Game::take_next_contract` returned (already applied to the active
/// contract); `None` means no contract was available.
pub fn town_contract_toast(resources: &Resources, villain_id: Option<&str>) -> String {
    match villain_id.and_then(|id| resources.villain_by_id(id)) {
        Some(villain) => {
            let reward = villain.reward.to_string();
            let zone_label = resources
                .zone_by_id(villain.zone.as_str())
                .map(|zone| {
                    if zone.name.is_empty() {
                        zone.id.as_str()
                    } else {
                        zone.name.as_str()
                    }
                })
                .unwrap_or(villain.zone.as_str());
            banner::formatted(
                resources,
                "banners",
                "town_contract_new",
                DEFAULT_TOWN_CONTRACT_NEW,
                &[
                    ("VILLAIN", villain.name.as_str()),
                    ("REWARD", &reward),
                    ("ZONE", zone_label),
                ],
            )
        }
        None => banner::text(
            resources,
            "banners",
            "town_contract_none",
            DEFAULT_TOWN_CONTRACT_NONE,
        )
        .to_owned(),
    }
}

/// The boat toast, porting `town_do_boat`. `None` is the silent success path
/// (the row label itself confirms the change).
pub fn town_boat_toast(resources: &Resources, outcome: BoatActionOutcome) -> Option<String> {
    match outcome {
        BoatActionOutcome::RentOk | BoatActionOutcome::CancelOk => None,
        BoatActionOutcome::CancelAtSea => Some(
            banner::text(
                resources,
                "banners",
                "town_boat_vacate_first",
                DEFAULT_TOWN_BOAT_VACATE_FIRST,
            )
            .to_owned(),
        ),
        BoatActionOutcome::RentNoGold => Some(
            banner::text(resources, "banners", "town_no_gold", DEFAULT_TOWN_NO_GOLD).to_owned(),
        ),
    }
}

/// The spell-buy toast, porting `town_do_spell`. The engine applies the buy;
/// this writes the result message.
pub fn town_spell_toast(resources: &Resources, game: &Game, outcome: SpellBuyOutcome) -> String {
    match outcome {
        SpellBuyOutcome::NoSpell => banner::text(
            resources,
            "banners",
            "town_spell_unavailable",
            DEFAULT_TOWN_SPELL_UNAVAILABLE,
        )
        .to_owned(),
        SpellBuyOutcome::AtCap => banner::text(
            resources,
            "banners",
            "town_spell_at_cap",
            DEFAULT_TOWN_SPELL_AT_CAP,
        )
        .to_owned(),
        SpellBuyOutcome::NoGold => {
            banner::text(resources, "banners", "town_no_gold", DEFAULT_TOWN_NO_GOLD).to_owned()
        }
        SpellBuyOutcome::Ok => {
            let left = game.stats.max_spells - game.known_spells();
            let left = left.to_string();
            let s = if game.stats.max_spells - game.known_spells() == 1 {
                ""
            } else {
                "s"
            };
            banner::formatted(
                resources,
                "banners",
                "town_spell_can_learn",
                DEFAULT_TOWN_SPELL_CAN_LEARN,
                &[("LEFT", &left), ("S", s)],
            )
        }
    }
}

/// The siege-buy toast, porting `town_do_siege`.
pub fn town_siege_toast(resources: &Resources, outcome: SiegeBuyOutcome) -> String {
    let (key, default) = match outcome {
        SiegeBuyOutcome::Already => ("town_siege_already", DEFAULT_TOWN_SIEGE_ALREADY),
        SiegeBuyOutcome::NoGold => ("town_no_gold", DEFAULT_TOWN_NO_GOLD),
        SiegeBuyOutcome::Ok => ("town_siege_purchased", DEFAULT_TOWN_SIEGE_PURCHASED),
    };
    banner::text(resources, "banners", key, default).to_owned()
}

/// The gather-information toast, porting `town_do_info`: castle owner, rule,
/// and garrison stacks (fuzzy-count labelled when the pack ships `number_names`).
pub fn town_intel_toast(resources: &Resources, game: &Game, record_key: &str) -> String {
    let unavailable = || {
        banner::text(
            resources,
            "banners",
            "town_intel_unavailable",
            DEFAULT_TOWN_INTEL_UNAVAILABLE,
        )
        .to_owned()
    };

    let Some(town) = resources.town_by_id(record_key) else {
        return unavailable();
    };
    if town.intel_castle.is_empty() {
        return unavailable();
    }
    let Some(castle) = resources.castle_by_id(town.intel_castle.as_str()) else {
        return unavailable();
    };
    if castle.special.excluded_from_intel {
        return unavailable();
    }
    let Some(record) = game
        .castles
        .iter()
        .find(|record| record.id.as_str() == town.intel_castle.as_str())
    else {
        return unavailable();
    };

    let display_name = if castle.name.is_empty() {
        castle.id.as_str()
    } else {
        castle.name.as_str()
    };
    let mut body = banner::formatted(
        resources,
        "banners",
        "town_intel_castle_under",
        DEFAULT_TOWN_INTEL_CASTLE_UNDER,
        &[("NAME", display_name)],
    );

    let owner = match record.owner_kind {
        CastleOwnerKind::Player => banner::text(
            resources,
            "banners",
            "town_intel_owner_player",
            DEFAULT_TOWN_INTEL_OWNER_PLAYER,
        ),
        CastleOwnerKind::Villain => {
            let label = resources
                .villain_by_id(record.villain_id.as_str())
                .map(|v| {
                    if v.name.is_empty() {
                        v.id.as_str()
                    } else {
                        v.name.as_str()
                    }
                })
                .unwrap_or(record.villain_id.as_str());
            label
        }
        CastleOwnerKind::Special => banner::text(
            resources,
            "banners",
            "town_intel_owner_king",
            DEFAULT_TOWN_INTEL_OWNER_KING,
        ),
        _ => banner::text(
            resources,
            "banners",
            "town_intel_owner_none",
            DEFAULT_TOWN_INTEL_OWNER_NONE,
        ),
    };
    body.push_str(&banner::formatted(
        resources,
        "banners",
        "town_intel_owner_rule",
        DEFAULT_TOWN_INTEL_OWNER_RULE,
        &[("OWNER", owner)],
    ));

    let mut stacks_shown = 0;
    for unit in record.garrison.iter() {
        if unit.id.is_empty() || unit.count == 0 {
            continue;
        }
        let troop_name = resources
            .troop_by_id(unit.id.as_str())
            .map(|t| t.name.as_str())
            .unwrap_or(unit.id.as_str());
        let count_label = resources.number_name(unit.count);
        if !count_label.is_empty() {
            body.push_str(&banner::formatted(
                resources,
                "banners",
                "town_intel_count_named",
                DEFAULT_TOWN_INTEL_COUNT_NAMED,
                &[("LABEL", count_label), ("TROOP", troop_name)],
            ));
        } else {
            let count = unit.count.to_string();
            body.push_str(&banner::formatted(
                resources,
                "banners",
                "town_intel_count_numeric",
                DEFAULT_TOWN_INTEL_COUNT_NUMERIC,
                &[("COUNT", &count), ("TROOP", troop_name)],
            ));
        }
        stacks_shown += 1;
    }
    if stacks_shown == 0 {
        let line = if record.owner_kind == CastleOwnerKind::Monsters {
            banner::text(
                resources,
                "banners",
                "town_intel_monsters_generic",
                DEFAULT_TOWN_INTEL_MONSTERS_GENERIC,
            )
        } else {
            banner::text(
                resources,
                "banners",
                "town_intel_no_garrison",
                DEFAULT_TOWN_INTEL_NO_GARRISON,
            )
        };
        body.push_str(line);
    }
    body
}

/// The home-castle menu body, porting the `body_home_castle` banner.
pub fn home_castle_body(resources: &Resources) -> String {
    banner::text(
        resources,
        "banners",
        "body_home_castle",
        DEFAULT_BODY_HOME_CASTLE,
    )
    .to_owned()
}

/// The own-castle mode subtitle, porting the `own_castle_mode_*` UI strings.
pub fn own_castle_mode_label(resources: &Resources, garrison_mode: bool) -> String {
    if garrison_mode {
        banner::text(
            resources,
            "ui",
            "own_castle_mode_garrison",
            DEFAULT_OWN_CASTLE_MODE_GARRISON,
        )
        .to_owned()
    } else {
        banner::text(
            resources,
            "ui",
            "own_castle_mode_remove",
            DEFAULT_OWN_CASTLE_MODE_REMOVE,
        )
        .to_owned()
    }
}

/// One own-castle row. In garrison mode it lists the player army (move into the
/// castle); in remove mode the castle garrison (move into the army). Ports
/// `screen_own_castle_draw`'s row formatting.
pub fn own_castle_row(
    resources: &Resources,
    game: &Game,
    castle_id: &str,
    garrison_mode: bool,
    slot: usize,
) -> String {
    let letter = char::from(b'A' + slot as u8);
    let (id, count) = if garrison_mode {
        game.army
            .get(slot)
            .filter(|stack| !stack.id.is_empty() && stack.count > 0)
            .map(|stack| (stack.id.as_str(), stack.count))
            .unwrap_or(("", 0))
    } else {
        game.castles
            .iter()
            .find(|castle| castle.id.as_str() == castle_id)
            .and_then(|castle| castle.garrison.get(slot))
            .filter(|unit| !unit.id.is_empty() && unit.count > 0)
            .map(|unit| (unit.id.as_str(), unit.count))
            .unwrap_or(("", 0))
    };

    if id.is_empty() {
        return format!("{letter}) {:<11}-", "(empty)");
    }
    let name = resources
        .troop_by_id(id)
        .map(|troop| troop.name.as_str())
        .unwrap_or(id);
    format!("{letter}) {:<11}{count}", name)
}

/// The refusal message for a garrison/remove transfer, porting the shell half
/// of the own-castle input: `WouldLeaveNoArmy` is garrison-only.
pub fn garrison_refusal(resources: &Resources, outcome: GarrisonOutcome) -> &str {
    match outcome {
        GarrisonOutcome::WouldLeaveNoArmy => banner::text(
            resources,
            "banners",
            "cannot_garrison_last",
            DEFAULT_CANNOT_GARRISON_LAST,
        ),
        _ => banner::text(
            resources,
            "banners",
            "no_troop_slots",
            DEFAULT_NO_TROOP_SLOTS,
        ),
    }
}

/// The recruit-soldiers screen title and "how many" prompt, from the `ui` block.
pub fn recruit_soldiers_title(resources: &Resources) -> String {
    banner::text(
        resources,
        "ui",
        "recruit_soldiers_title",
        DEFAULT_RECRUIT_SOLDIERS_TITLE,
    )
    .to_owned()
}

pub fn recruit_soldiers_how_many(resources: &Resources) -> String {
    banner::text(
        resources,
        "ui",
        "recruit_soldiers_how_many",
        DEFAULT_RECRUIT_SOLDIERS_HOW_MANY,
    )
    .to_owned()
}

/// The recruit-commit refusal popup, porting `recruit_commit`'s error mapping:
/// no gold reads the town banner, no slots the troop-slots banner.
pub fn recruit_refusal(resources: &Resources, outcome: RecruitOutcome) -> Option<String> {
    match outcome {
        RecruitOutcome::NoGold => Some(
            banner::text(resources, "banners", "town_no_gold", DEFAULT_TOWN_NO_GOLD).to_owned(),
        ),
        RecruitOutcome::NoSlotOrInvalid => Some(
            banner::text(
                resources,
                "banners",
                "no_troop_slots",
                DEFAULT_NO_TROOP_SLOTS,
            )
            .to_owned(),
        ),
        _ => None,
    }
}

/// The audience flow's two messages, porting `run_audience_dialog`: the
/// fanfare (page 1) and the king's words (page 2), with %NAME%/%RANK%/%NEEDED%
/// /%S% substituted.
pub fn audience_messages(
    resources: &Resources,
    game: &Game,
    outcome: AudienceOutcome,
    needed: i32,
) -> (String, String) {
    let castle = resources.home_castle();
    let (intro, branch) = match castle {
        Some(castle) => {
            let intro = if castle.special.audience_intro.is_empty() {
                DEFAULT_AUDIENCE_INTRO
            } else {
                castle.special.audience_intro.as_str()
            };
            let branch = match outcome {
                AudienceOutcome::MoreNeeded => castle.special.audience_more_needed.as_str(),
                AudienceOutcome::Promoted => castle.special.audience_rank_up.as_str(),
                AudienceOutcome::FinalRank => castle.special.audience_final_rank.as_str(),
            };
            (intro, branch)
        }
        None => ("", ""),
    };

    let needed = needed.max(0);
    let needed_str = needed.to_string();
    let s = if needed == 1 { "" } else { "s" };
    let vars: &[banner::Var<'_>] = &[
        ("NAME", game.character.name.as_str()),
        ("RANK", game.character.cls.rank_title.as_str()),
        ("NEEDED", &needed_str),
        ("S", s),
    ];
    let fanfare = banner::format(intro, vars);
    let king = banner::format(branch, vars);
    (fanfare, king)
}

/// A dwelling kind's display name, porting `dwelling_kind_name`.
pub fn dwelling_kind_name(resources: &Resources, kind: Interact) -> String {
    let (key, default) = match kind {
        Interact::DwellingForest => ("dwelling_kind_forest", DEFAULT_DWELLING_KIND_FOREST),
        Interact::DwellingHills => ("dwelling_kind_hill", DEFAULT_DWELLING_KIND_HILL),
        Interact::DwellingDungeon => ("dwelling_kind_dungeon", DEFAULT_DWELLING_KIND_DUNGEON),
        _ => ("dwelling_kind_plains", DEFAULT_DWELLING_KIND_PLAINS),
    };
    banner::text(resources, "ui", key, default).to_owned()
}

/// The dwelling recruit prompt body, porting `dwelling_recruit_prompt`.
pub fn dwelling_prompt_body(
    resources: &Resources,
    troop_name: &str,
    count: i32,
    cost: i32,
    gold: i32,
    cap: i32,
) -> String {
    let count = count.to_string();
    let cost = cost.to_string();
    let gold = gold.to_string();
    let cap = cap.to_string();
    banner::formatted(
        resources,
        "banners",
        "dwelling_recruit_prompt",
        DEFAULT_DWELLING_RECRUIT_PROMPT,
        &[
            ("COUNT", &count),
            ("TROOP", troop_name),
            ("COST", &cost),
            ("GOLD", &gold),
            ("CAP", &cap),
        ],
    )
}

/// The "no troops this week" refusal, porting `dwelling_none_this_week`.
pub fn dwelling_none_this_week(resources: &Resources) -> String {
    banner::text(
        resources,
        "banners",
        "dwelling_none_this_week",
        DEFAULT_DWELLING_NONE_THIS_WEEK,
    )
    .to_owned()
}

/// The empty-dwelling message, porting `dwelling_empty`.
pub fn dwelling_empty(resources: &Resources) -> String {
    banner::text(
        resources,
        "banners",
        "dwelling_empty",
        DEFAULT_DWELLING_EMPTY,
    )
    .to_owned()
}

/// The "Recruit how many" label from the `ui` block.
pub fn dwelling_how_many(resources: &Resources) -> String {
    banner::text(
        resources,
        "ui",
        "dwelling_recruit_how_many",
        DEFAULT_DWELLING_HOW_MANY,
    )
    .to_owned()
}

/// The friendly-foe join-offer body, porting `start_foe_friendly_flow`. C names
/// the count with a fuzzy label when one exists and a number otherwise; the
/// offer is always free, so neither template's `%COST%` is ever filled.
pub fn friendly_offer_prompt(resources: &Resources, offer: &FriendlyOffer) -> String {
    let troop_name = resources
        .troop_by_id(offer.troop_id.as_str())
        .map(|troop| troop.name.as_str())
        .unwrap_or(offer.troop_id.as_str())
        .to_owned();
    let label = resources.number_name(offer.count);
    if label.is_empty() {
        let count = offer.count.to_string();
        banner::formatted(
            resources,
            "banners",
            "encounter_join_numeric",
            DEFAULT_ENCOUNTER_JOIN_NUMERIC,
            &[("COUNT", &count), ("TROOP", &troop_name)],
        )
    } else {
        banner::formatted(
            resources,
            "banners",
            "encounter_join_named",
            DEFAULT_ENCOUNTER_JOIN_NAMED,
            &[("LABEL", label), ("TROOP", &troop_name)],
        )
    }
}

/// The "wanderers pass by" banner shown when a friendly foe flees (no army
/// slot for the offered troop), porting the `!has_slot` branch of
/// `start_foe_friendly_flow`.
pub fn friendly_flees(resources: &Resources) -> String {
    banner::text(
        resources,
        "banners",
        "encounter_wanderers",
        DEFAULT_ENCOUNTER_WANDERERS,
    )
    .to_owned()
}

pub fn castle_display_name(resources: &Resources, castle_id: &str) -> String {
    resources
        .castle_by_id(castle_id)
        .map(|castle| {
            if castle.name.is_empty() {
                castle.id.as_str()
            } else {
                castle.name.as_str()
            }
        })
        .unwrap_or(castle_id)
        .to_owned()
}

/// The siege target's display name, porting `run_castle_combat`: a named
/// villain first, else the castle name, else the castle id. Display-only -- the
/// RNG identity is still the castle id.
pub fn siege_target_name(resources: &Resources, game: &Game, castle_id: &str) -> String {
    let villain_name = game
        .castles
        .iter()
        .find(|castle| castle.id.as_str() == castle_id)
        .filter(|castle| !castle.villain_id.is_empty())
        .and_then(|castle| resources.villain_by_id(castle.villain_id.as_str()))
        .map(|villain| villain.name.as_str())
        .filter(|name| !name.is_empty());
    if let Some(name) = villain_name {
        name.to_owned()
    } else {
        castle_display_name(resources, castle_id)
    }
}

/// The monster-castle siege prompt, porting the `FLOW_SIEGE_MONSTER` branch.
pub fn siege_monster_prompt(resources: &Resources, castle_id: &str) -> (String, String) {
    let name = castle_display_name(resources, castle_id);
    (
        format!("Castle {name}"),
        "Various groups of monsters\noccupy this castle.\n\n\n                Lay Siege".to_owned(),
    )
}

/// The villain-castle siege prompt, porting the `FLOW_SIEGE_VILLAIN` branch.
pub fn siege_villain_prompt(
    resources: &Resources,
    game: &Game,
    castle_id: &str,
) -> (String, String) {
    let name = castle_display_name(resources, castle_id);
    let villain_name = game
        .castles
        .iter()
        .find(|castle| castle.id.as_str() == castle_id)
        .map(|castle| castle.villain_id.as_str())
        .and_then(|id| resources.villain_by_id(id))
        .map(|villain| villain.name.as_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Foe");
    (
        format!("Castle {name}"),
        format!("{villain_name} and\narmy occupy this castle.\n\n\n                Lay Siege"),
    )
}

/// The villain-siege "Capture" dialog, porting the inline message in
/// `flow_apply_siege_villain`. `None` for a repelled siege or a missing castle.
pub fn capture_message(
    outcome: VillainSiegeOutcome,
    villain_name: &str,
    reward: i32,
) -> Option<(String, String)> {
    match outcome {
        VillainSiegeOutcome::Captured => Some((
            "Capture".to_owned(),
            format!(
                "...and the capture of {villain_name}.\n\n\
                 For fulfilling your contract\n\
                 you receive an additional\n\
                 {reward} gold as bounty...\n\
                 and a piece of the map to\n\
                 the stolen scepter."
            ),
        )),
        VillainSiegeOutcome::SetFree => Some((
            "Capture".to_owned(),
            format!(
                "...and the capture of {villain_name}.\n\n\
                 Since you did not have the\n\
                 proper contract, the Lord\n\
                 has been set free."
            ),
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::game::{ChestOutcome, ChestRoll, Game, GarrisonOutcome};
    use openbounty_engine::step::StepInteraction;
    use openbounty_engine::tables::FixedStr;

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

    fn town_resources() -> Resources {
        Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [
                 {"index":0,"id":"peasants","name":"Peasants","skill_level":1,"hit_points":1,
                  "move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,
                  "spoils_factor":1,"abilities":"","morale_group":"A"}
               ],
               "zones": [{"id":"home","name":"Continentia"}],
               "towns": [{"id":"town0","name":"Riverton","zone":"home","x":1,"y":1,"intel_castle":"castle0"}],
               "castles": [{"id":"castle0","name":"Darkstone","zone":"home","x":2,"y":2}],
               "spells": [{"id":"fireball","name":"Fireball","kind":"combat","cost":50}],
               "villains": [{"index":0,"id":"murray","name":"Murray","zone":"home","reward":1000}],
               "combat": {"number_names": [{"min":10,"label":"Some"},{"min":1,"label":"A few"}]}}"#,
        )
        .expect("fixture parses")
    }

    #[test]
    fn town_rows_reflect_game_state() {
        let resources = town_resources();
        let mut game = Game::default();
        game.towns[0].id.copy_from_str("town0");
        game.towns[0].spell_for_sale.copy_from_str("fireball");

        assert!(town_row_text(&resources, &game, "town0", 0).contains("Get New Contract"));
        assert!(town_row_text(&resources, &game, "town0", 1).contains("Rent boat"));
        assert!(town_row_text(&resources, &game, "town0", 3).contains("Fireball"));
        assert!(town_row_text(&resources, &game, "town0", 3).contains("50"));
        assert!(town_row_text(&resources, &game, "town0", 4).contains("Buy seige"));

        game.boat.has_boat = true;
        assert!(town_row_text(&resources, &game, "town0", 1).contains("Cancel boat"));

        game.stats.siege_weapons = 1;
        assert!(town_row_text(&resources, &game, "town0", 4).contains("owned"));
    }

    #[test]
    fn town_contract_toast_names_the_villain() {
        let resources = town_resources();
        let toast = town_contract_toast(&resources, Some("murray"));
        assert!(toast.contains("Murray"));
        assert!(toast.contains("1000"));
        assert!(toast.contains("Continentia"));

        let none = town_contract_toast(&resources, None);
        assert!(none.contains("No contracts"));
    }

    #[test]
    fn town_intel_toast_reports_castle_owner_and_garrison() {
        let resources = town_resources();
        let mut game = Game::default();
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].owner_kind = openbounty_engine::game::CastleOwnerKind::Villain;
        game.castles[0].villain_id.copy_from_str("murray");
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 5;

        let toast = town_intel_toast(&resources, &game, "town0");
        assert!(toast.contains("Darkstone"));
        assert!(toast.contains("Murray"));
        assert!(toast.contains("A few"));
        assert!(toast.contains("Peasants"));
    }

    #[test]
    fn town_boat_toast_only_responds_to_failures() {
        let resources = town_resources();
        assert_eq!(town_boat_toast(&resources, BoatActionOutcome::RentOk), None);
        assert_eq!(
            town_boat_toast(&resources, BoatActionOutcome::CancelOk),
            None
        );
        assert!(town_boat_toast(&resources, BoatActionOutcome::RentNoGold)
            .unwrap()
            .contains("gold"));
        assert!(town_boat_toast(&resources, BoatActionOutcome::CancelAtSea)
            .unwrap()
            .contains("vacate"));
    }

    #[test]
    fn town_spell_and_siege_toasts_report_each_outcome() {
        let resources = town_resources();
        let mut game = Game::default();
        game.stats.max_spells = 5;
        game.spells.counts[0] = 2;

        assert!(town_spell_toast(&resources, &game, SpellBuyOutcome::NoSpell).contains("No spell"));
        assert!(town_spell_toast(&resources, &game, SpellBuyOutcome::AtCap).contains("maximum"));
        assert!(town_spell_toast(&resources, &game, SpellBuyOutcome::NoGold).contains("gold"));
        assert!(town_spell_toast(&resources, &game, SpellBuyOutcome::Ok).contains("3 more spells"));

        assert!(town_siege_toast(&resources, SiegeBuyOutcome::Already).contains("siege weapons"));
        assert!(town_siege_toast(&resources, SiegeBuyOutcome::NoGold).contains("gold"));
        assert!(town_siege_toast(&resources, SiegeBuyOutcome::Ok).contains("purchased"));
    }

    #[test]
    fn the_home_castle_body_and_mode_labels_are_pack_text() {
        let resources = town_resources();
        assert!(home_castle_body(&resources).contains("Recruit"));
        assert!(own_castle_mode_label(&resources, true).contains("Garrison"));
        assert!(own_castle_mode_label(&resources, false).contains("Remove"));
    }

    #[test]
    fn the_own_castle_rows_list_the_right_source() {
        let resources = town_resources();
        let mut game = Game::default();
        game.army[0].id.copy_from_str("peasants");
        game.army[0].count = 12;
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].garrison[0].id.copy_from_str("peasants");
        game.castles[0].garrison[0].count = 5;

        // Garrison mode lists the player army.
        let row = own_castle_row(&resources, &game, "castle0", true, 0);
        assert_eq!(row, "A) Peasants   12");
        // Remove mode lists the castle garrison.
        let row = own_castle_row(&resources, &game, "castle0", false, 0);
        assert_eq!(row, "A) Peasants   5");
        // An empty slot reads "(empty) -".
        assert!(own_castle_row(&resources, &game, "castle0", true, 1).contains("(empty)"));
    }

    #[test]
    fn garrison_refusals_name_the_banner() {
        let resources = town_resources();
        assert!(garrison_refusal(&resources, GarrisonOutcome::Refused).contains("slots"));
        assert!(garrison_refusal(&resources, GarrisonOutcome::WouldLeaveNoArmy).contains("last"));
    }

    #[test]
    fn recruit_soldiers_text_and_refusals() {
        let resources = town_resources();
        assert_eq!(recruit_soldiers_title(&resources), "Recruit Soldiers");
        assert_eq!(recruit_soldiers_how_many(&resources), "How Many");
        assert!(recruit_refusal(&resources, RecruitOutcome::NoGold)
            .unwrap()
            .contains("gold"));
        assert!(recruit_refusal(&resources, RecruitOutcome::NoSlotOrInvalid)
            .unwrap()
            .contains("slots"));
        assert_eq!(recruit_refusal(&resources, RecruitOutcome::Ok), None);
    }

    #[test]
    fn audience_messages_substitute_name_rank_and_needed() {
        let resources = Resources::from_json_str(
            r#"{"render": {"mode": "legacy"}, "troops": [],
                "classes": [{"index":0,"id":"knight","name":"Knight","portrait":"k.png",
                  "ranks":[{"id":"general","name":"General","villains_needed":2}]}],
                "castles": [{"id":"home","zone":"home","x":0,"y":0,
                  "special":{"flow":"audience",
                    "audience":{"intro":"Fanfare %NAME%",
                                "rank_up":"Promoted to %RANK%",
                                "more_needed":"Need %NEEDED% more villain%S%",
                                "final_rank":"Final %RANK%"}}}]}"#,
        )
        .expect("fixture parses");

        let mut game = Game::default();
        game.character.name.copy_from_str("Sir Rust");
        game.character.cls.rank_title.copy_from_str("General");

        let (fanfare, king) = audience_messages(&resources, &game, AudienceOutcome::MoreNeeded, 3);
        assert_eq!(fanfare, "Fanfare Sir Rust");
        assert_eq!(king, "Need 3 more villains");

        let (_, king) = audience_messages(&resources, &game, AudienceOutcome::MoreNeeded, 1);
        assert_eq!(king, "Need 1 more villain");

        let (_, king) = audience_messages(&resources, &game, AudienceOutcome::Promoted, 0);
        assert_eq!(king, "Promoted to General");
    }

    #[test]
    fn dwelling_text_and_prompt_body() {
        let resources = town_resources();

        assert_eq!(
            dwelling_kind_name(&resources, Interact::DwellingPlains),
            "Plains"
        );
        assert_eq!(
            dwelling_kind_name(&resources, Interact::DwellingForest),
            "Forest"
        );
        assert_eq!(
            dwelling_kind_name(&resources, Interact::DwellingHills),
            "Hill"
        );
        assert_eq!(
            dwelling_kind_name(&resources, Interact::DwellingDungeon),
            "Dungeon"
        );

        let body = dwelling_prompt_body(&resources, "Peasants", 5, 10, 5000, 5);
        assert!(body.contains("5 Peasants are available"));
        assert!(body.contains("Cost=10 each"));
        assert!(body.contains("GP=5000"));
        assert!(body.contains("up to 5"));

        assert!(dwelling_none_this_week(&resources).contains("this week"));
        assert!(dwelling_empty(&resources).contains("empty"));
        assert_eq!(dwelling_how_many(&resources), "Recruit how many");
    }

    #[test]
    fn siege_prompts_name_the_occupant() {
        let resources = town_resources();
        let (header, body) = siege_monster_prompt(&resources, "castle0");
        assert_eq!(header, "Castle Darkstone");
        assert!(body.contains("monsters"));
        assert!(body.contains("Lay Siege"));

        let mut game = Game::default();
        game.castles[0].id.copy_from_str("castle0");
        game.castles[0].villain_id.copy_from_str("murray");
        let (header, body) = siege_villain_prompt(&resources, &game, "castle0");
        assert_eq!(header, "Castle Darkstone");
        assert!(body.contains("Murray"));
    }

    #[test]
    fn the_capture_dialog_names_the_villain() {
        let (header, body) =
            capture_message(VillainSiegeOutcome::Captured, "Murray", 1000).expect("captured");
        assert_eq!(header, "Capture");
        assert!(body.contains("Murray"));
        assert!(body.contains("1000"));
        assert!(body.contains("bounty"));

        let (header, body) =
            capture_message(VillainSiegeOutcome::SetFree, "Murray", 0).expect("set free");
        assert_eq!(header, "Capture");
        assert!(body.contains("Murray"));
        assert!(body.contains("set free"));

        assert_eq!(
            capture_message(VillainSiegeOutcome::Repelled, "Murray", 0),
            None
        );
        assert_eq!(
            capture_message(VillainSiegeOutcome::NoCastle, "Murray", 0),
            None
        );
    }

    #[test]
    fn siege_target_name_names_the_villain_first() {
        let resources = town_resources();

        let mut game = Game::default();
        game.castles[0].id.copy_from_str("castle0");
        assert_eq!(siege_target_name(&resources, &game, "castle0"), "Darkstone");

        game.castles[0].villain_id.copy_from_str("murray");
        assert_eq!(siege_target_name(&resources, &game, "castle0"), "Murray");

        assert_eq!(siege_target_name(&resources, &game, "nope"), "nope");
    }

    #[test]
    fn friendly_offer_prompt_and_flees() {
        let resources = town_resources();

        let offer = FriendlyOffer {
            troop_id: FixedStr::from_str("peasants"),
            count: 5,
            foe_id: FixedStr::from_str("foe0"),
            zone: FixedStr::from_str("home"),
            x: 0,
            y: 0,
        };
        let body = friendly_offer_prompt(&resources, &offer);
        assert!(body.contains("A few Peasants"));
        // A free offer never fills %COST%, which the verbatim default leaves in.
        assert!(body.contains("%COST%"));

        assert!(friendly_flees(&resources).contains("wanderers"));
    }
}
