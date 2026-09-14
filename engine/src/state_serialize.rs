use crate::game::{
    CastleOwnerKind, Game, Mount, TravelMode, Unit, CONTRACT_CYCLE_MAX, GAME_ARMY_SLOTS,
};
use crate::resources::Resources;

pub const SAVE_VERSION: u32 = 11;

pub fn build_snapshot(game: Option<&Game>) -> serde_json::Value {
    build_snapshot_with_resources(game, None)
}

pub fn build_snapshot_with_resources(
    game: Option<&Game>,
    resources: Option<&Resources>,
) -> serde_json::Value {
    let mut root = serde_json::Map::new();
    root.insert("version".to_owned(), serde_json::json!(SAVE_VERSION));

    let Some(game) = game else {
        root.insert("mode".to_owned(), serde_json::json!("unattached"));
        return serde_json::Value::Object(root);
    };

    root.insert("mode".to_owned(), serde_json::json!("adventure"));
    root.insert(
        "seed_from_catalog".to_owned(),
        serde_json::json!(game.seed_from_catalog),
    );
    if game.seed_from_catalog {
        root.insert("seed_index".to_owned(), serde_json::json!(game.seed_index));
    } else {
        root.insert("seed".to_owned(), serde_json::json!(game.seed));
    }

    root.insert(
        "character".to_owned(),
        serde_json::json!({
            "name": game.character.name.as_str(),
            "class": game.character.cls.id.as_str(),
            "rank_index": game.character.cls.rank_index,
            "rank_title": game.character.cls.rank_title.as_str(),
            "difficulty": difficulty_id(game.character.difficulty),
            "mount": mount_id(game.character.mount),
        }),
    );

    root.insert(
        "stats".to_owned(),
        serde_json::json!({
            "gold": game.stats.gold,
            "commission_weekly": game.stats.commission_weekly,
            "leadership_base": game.stats.leadership_base,
            "leadership_current": game.stats.leadership_current,
            "followers_killed": game.stats.followers_killed,
            "score": game.stats.score,
            "spell_power": game.stats.spell_power,
            "max_spells": game.stats.max_spells,
            "knows_magic": game.stats.knows_magic,
            "siege_weapons": game.stats.siege_weapons,
            "time_stop": game.stats.time_stop,
            "steps_left_today": game.stats.steps_left_today,
            "days_left": game.stats.days_left,
            "game_over": game.stats.game_over,
            "won": game.stats.won,
            "last_commission": game.stats.last_commission,
            "options": game.stats.options,
        }),
    );

    root.insert(
        "position".to_owned(),
        serde_json::json!({
            "zone": game.position.zone.as_str(),
            "x": game.position.x,
            "y": game.position.y,
            "last_x": game.position.last_x,
            "last_y": game.position.last_y,
            "travel_mode": travel_mode_id(game.travel_mode),
        }),
    );

    let army: Vec<_> = game
        .army
        .iter()
        .enumerate()
        .filter(|(_, stack)| !stack.id.is_empty() && stack.count != 0)
        .map(|(slot, stack)| {
            serde_json::json!({
                "slot": slot,
                "troop": stack.id.as_str(),
                "count": stack.count,
            })
        })
        .collect();
    root.insert("army".to_owned(), serde_json::Value::Array(army));

    root.insert("contract".to_owned(), contract_snapshot(game, resources));

    let towns: Vec<_> = game
        .towns
        .iter()
        .filter(|town| !town.id.is_empty())
        .map(|town| {
            let mut value = serde_json::Map::new();
            value.insert("id".to_owned(), serde_json::json!(town.id.as_str()));
            value.insert("visited".to_owned(), serde_json::json!(town.visited));
            if !town.spell_for_sale.is_empty() {
                value.insert(
                    "spell_for_sale".to_owned(),
                    serde_json::json!(town.spell_for_sale.as_str()),
                );
            }
            if let Some(resource_town) =
                resources.and_then(|resources| resources.town_by_id(town.id.as_str()))
            {
                value.insert(
                    "zone".to_owned(),
                    serde_json::json!(resource_town.zone.as_str()),
                );
                value.insert("x".to_owned(), serde_json::json!(resource_town.x));
                value.insert("y".to_owned(), serde_json::json!(resource_town.y));
                value.insert("gate_x".to_owned(), serde_json::json!(resource_town.gate_x));
                value.insert("gate_y".to_owned(), serde_json::json!(resource_town.gate_y));
            }
            serde_json::Value::Object(value)
        })
        .collect();
    root.insert("towns".to_owned(), serde_json::Value::Array(towns));

    let castles: Vec<_> = game
        .castles
        .iter()
        .filter(|castle| !castle.id.is_empty())
        .map(|castle| {
            let mut value = serde_json::Map::new();
            value.insert("id".to_owned(), serde_json::json!(castle.id.as_str()));
            value.insert("visited".to_owned(), serde_json::json!(castle.visited));
            value.insert("known".to_owned(), serde_json::json!(castle.known));
            value.insert(
                "owner".to_owned(),
                serde_json::json!(castle_owner_id(castle.owner_kind)),
            );
            if !castle.villain_id.is_empty() {
                value.insert(
                    "villain".to_owned(),
                    serde_json::json!(castle.villain_id.as_str()),
                );
            }
            if let Some(resource_castle) =
                resources.and_then(|resources| resources.castle_by_id(castle.id.as_str()))
            {
                value.insert(
                    "zone".to_owned(),
                    serde_json::json!(resource_castle.zone.as_str()),
                );
                value.insert("x".to_owned(), serde_json::json!(resource_castle.x));
                value.insert("y".to_owned(), serde_json::json!(resource_castle.y));
                value.insert(
                    "gate_x".to_owned(),
                    serde_json::json!(resource_castle.gate_x),
                );
                value.insert(
                    "gate_y".to_owned(),
                    serde_json::json!(resource_castle.gate_y),
                );
            }
            value.insert(
                "garrison".to_owned(),
                serde_json::Value::Array(unit_array_snapshot(&castle.garrison)),
            );
            serde_json::Value::Object(value)
        })
        .collect();
    root.insert("castles".to_owned(), serde_json::Value::Array(castles));

    serde_json::Value::Object(root)
}

pub fn snapshot_to_string(game: Option<&Game>) -> String {
    serde_json::to_string(&build_snapshot(game)).expect("snapshot value serializes")
}

fn difficulty_id(difficulty: i32) -> &'static str {
    match difficulty {
        1 => "normal",
        2 => "hard",
        3 => "impossible",
        _ => "easy",
    }
}

fn mount_id(mount: Mount) -> &'static str {
    match mount {
        Mount::Sail => "sail",
        Mount::Fly => "fly",
        Mount::Ride => "ride",
    }
}

fn travel_mode_id(travel_mode: TravelMode) -> &'static str {
    match travel_mode {
        TravelMode::Boat => "boat",
        TravelMode::Walk => "walk",
    }
}

fn castle_owner_id(owner: CastleOwnerKind) -> &'static str {
    match owner {
        CastleOwnerKind::Monsters => "monsters",
        CastleOwnerKind::Villain => "villain",
        CastleOwnerKind::Special => "special",
        CastleOwnerKind::Player => "player",
    }
}

fn contract_snapshot(game: &Game, resources: Option<&Resources>) -> serde_json::Value {
    let mut cycle = Vec::with_capacity(5);
    for slot in game.contract.cycle.iter().take(5.min(CONTRACT_CYCLE_MAX)) {
        cycle.push(serde_json::json!(slot.as_str()));
    }

    serde_json::json!({
        "active": game.contract.active_id.as_str(),
        "cycle": cycle,
        "last_contract": game.contract.last_contract,
        "max_contract": game.contract.max_contract,
        "villains_caught": villain_flags_snapshot(&game.contract.villains_caught, resources),
        "villains_prefought": villain_flags_snapshot(&game.contract.villains_prefought, resources),
    })
}

fn villain_flags_snapshot(flags: &[bool], resources: Option<&Resources>) -> serde_json::Value {
    let ids: Vec<_> = flags
        .iter()
        .enumerate()
        .filter(|(_, caught)| **caught)
        .filter_map(|(index, _)| {
            resources
                .and_then(|resources| resources.villain_by_index(index as i32))
                .map(|villain| serde_json::json!(villain.id.as_str()))
        })
        .collect();
    serde_json::Value::Array(ids)
}

fn unit_array_snapshot(units: &[Unit; GAME_ARMY_SLOTS]) -> Vec<serde_json::Value> {
    units
        .iter()
        .filter(|unit| !unit.id.is_empty() && unit.count != 0)
        .map(|unit| {
            serde_json::json!({
                "troop": unit.id.as_str(),
                "count": unit.count,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{build_snapshot, build_snapshot_with_resources, snapshot_to_string, SAVE_VERSION};
    use crate::game::Game;
    use crate::resources::Resources;

    #[test]
    fn catalog_seed_snapshot_uses_openbounty_seed_identity_fields() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 42);

        let snapshot = build_snapshot(Some(&game));
        assert_eq!(snapshot["version"], SAVE_VERSION);
        assert_eq!(snapshot["mode"], "adventure");
        assert_eq!(snapshot["seed_from_catalog"], true);
        assert_eq!(snapshot["seed_index"], 42);
        assert_eq!(snapshot["character"]["name"], "Hero");
        assert_eq!(snapshot["character"]["difficulty"], "easy");
        assert_eq!(snapshot["character"]["mount"], "ride");
    }

    #[test]
    fn raw_seed_snapshot_writes_seed_instead_of_catalog_index() {
        let mut game = Game::with_raw_seed(12_345);
        game.init_seeded(Some("Hero"), 0, 0, -1);

        let snapshot = build_snapshot(Some(&game));
        assert_eq!(snapshot["version"], SAVE_VERSION);
        assert_eq!(snapshot["mode"], "adventure");
        assert_eq!(snapshot["seed_from_catalog"], false);
        assert_eq!(snapshot["seed"], 12_345_u64);
        assert!(snapshot.get("seed_index").is_none());
    }

    #[test]
    fn unattached_snapshot_matches_openbounty_mode() {
        assert_eq!(
            build_snapshot(None),
            serde_json::json!({
                "version": SAVE_VERSION,
                "mode": "unattached"
            })
        );
    }

    #[test]
    fn snapshot_string_is_compact_json() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 1);

        let json = snapshot_to_string(Some(&game));
        assert!(json.starts_with('{'));
        assert!(!json.contains('\n'));
        assert!(json.contains(r#""seed_index":1"#));
    }

    #[test]
    fn resource_backed_snapshot_uses_openbounty_state_field_names() {
        let resources = Resources::from_json_str(
            r#"{
              "time": {"day_steps": 33, "days_per_difficulty": {"easy": 901, "normal": 601, "hard": 401, "impossible": 201}},
              "world": {"starting_zone": "home", "default_options": [4,1,1,1,1,0,5]},
              "contract": {"cycle_length": 3, "initial_last_contract": 2},
              "zones": [{"id":"home","is_home":true,"home_spawn":{"x":3,"y":4}}],
              "towns": [{"index":0,"id":"town0","zone":"home","x":1,"y":2,"gate":{"x":1,"y":3},"pinned_spell":"bridge"}],
              "castles": [{"index":0,"id":"castle0","zone":"home","x":6,"y":7}],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":1,"melee":[1,1],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"}
              ],
              "spells": [
                {"index":0,"id":"bridge","name":"Bridge","kind":"adventure","cost":100}
              ],
              "classes": [
                {"index":0,"id":"knight","starting_gold":1000,"starting_troops":[{"id":"peasants","count":25}],"ranks":[{"id":"general","name":"General","leadership":120,"max_spells":2,"spell_power":1,"commission":100,"knows_magic":false}]}
              ],
              "villains": [
                {"index":0,"id":"villain0","zone":"home","reward":1000},
                {"index":1,"id":"villain1","zone":"home","reward":2000}
              ]
            }"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        game.init_seeded_with_resources(&resources, Some("arthur"), 0, 2, 42);
        game.salt_spells(&resources);

        let snapshot = build_snapshot_with_resources(Some(&game), Some(&resources));
        assert_eq!(snapshot["character"]["name"], "Arthur");
        assert_eq!(snapshot["character"]["class"], "knight");
        assert_eq!(snapshot["character"]["rank_title"], "General");
        assert_eq!(snapshot["character"]["difficulty"], "hard");
        assert_eq!(snapshot["stats"]["gold"], 1000);
        assert_eq!(snapshot["stats"]["leadership_current"], 120);
        assert_eq!(snapshot["stats"]["days_left"], 401);
        assert_eq!(snapshot["position"]["zone"], "home");
        assert_eq!(snapshot["position"]["travel_mode"], "walk");
        assert_eq!(snapshot["army"][0]["slot"], 0);
        assert_eq!(snapshot["army"][0]["troop"], "peasants");
        assert_eq!(snapshot["army"][0]["count"], 25);
        assert_eq!(snapshot["contract"]["active"], "");
        assert_eq!(snapshot["contract"]["cycle"][0], "villain0");
        assert_eq!(snapshot["contract"]["cycle"][1], "villain1");
        assert_eq!(snapshot["contract"]["last_contract"], 2);
        assert_eq!(snapshot["contract"]["max_contract"], 3);
        assert_eq!(
            snapshot["contract"]["villains_caught"],
            serde_json::json!([])
        );
        assert_eq!(snapshot["towns"][0]["id"], "town0");
        assert_eq!(snapshot["towns"][0]["visited"], false);
        assert_eq!(snapshot["towns"][0]["spell_for_sale"], "bridge");
        assert_eq!(snapshot["towns"][0]["zone"], "home");
        assert_eq!(snapshot["towns"][0]["x"], 1);
        assert_eq!(snapshot["towns"][0]["y"], 2);
        assert_eq!(snapshot["towns"][0]["gate_x"], 1);
        assert_eq!(snapshot["towns"][0]["gate_y"], 3);
        assert_eq!(snapshot["castles"][0]["id"], "castle0");
        assert_eq!(snapshot["castles"][0]["owner"], "monsters");
        assert_eq!(snapshot["castles"][0]["zone"], "home");
        assert_eq!(snapshot["castles"][0]["x"], 6);
        assert_eq!(snapshot["castles"][0]["y"], 7);
        assert_eq!(snapshot["castles"][0]["gate_x"], 6);
        assert_eq!(snapshot["castles"][0]["gate_y"], 8);
        assert_eq!(snapshot["castles"][0]["garrison"], serde_json::json!([]));
    }
}
