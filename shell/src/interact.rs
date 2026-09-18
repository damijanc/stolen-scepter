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

use openbounty_engine::resources::Resources;
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

#[cfg(test)]
mod tests {
    use super::*;
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
                outcome: openbounty_engine::game::ChestOutcome::Gold,
                pending: openbounty_engine::game::ChestPending::default(),
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
}
