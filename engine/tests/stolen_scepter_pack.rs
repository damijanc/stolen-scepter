//! The game's own pack (`packs/stolen-scepter/`) loads and plays with nothing
//! from the original game on disk: it is the only pack this test opens.
//!
//! Built by `tools/gen_maps.py` + `tools/build_pack.py`; see
//! `specs/publish-plan.md`, Phase 2.

use openbounty_engine::combat::CombatResult;
use openbounty_engine::game::{Game, VillainSiegeOutcome};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::step::{search_here, SearchOutcome};
use openbounty_engine::tile::Terrain;

const PACK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../packs/stolen-scepter/game.json"
);

fn resources() -> Resources {
    Resources::from_json_file(PACK).expect("the stolen-scepter pack parses")
}

#[test]
fn every_zone_map_loads_and_its_placements_sit_on_usable_tiles() {
    let resources = resources();
    assert_eq!(resources.zone_count(), 4);
    for index in 0..resources.zone_count() {
        let zone = resources.zone_by_index(index).expect("zone");
        let id = zone.id.as_str();
        let map = Map::load_zone(&resources, id).unwrap_or_else(|e| panic!("{id}: {e}"));

        let spawn = map
            .tile_i32(zone.hero_spawn_x, zone.hero_spawn_y)
            .expect("spawn tile");
        assert!(!spawn.blocks_foot, "{id}: hero spawn is blocked");

        for chest in &zone.chests[..zone.chest_count] {
            let tile = map.tile_i32(chest.x, chest.y).expect("chest tile");
            assert_eq!(
                tile.terrain,
                Terrain::Grass,
                "{id}: chest {} off grass",
                chest.id.as_str()
            );
        }
        for town in 0..resources.town_count() {
            let town = resources.town_by_index(town).expect("town");
            if town.zone.as_str() != id {
                continue;
            }
            let dock = map.tile_i32(town.boat_x, town.boat_y).expect("dock tile");
            assert_eq!(
                dock.terrain,
                Terrain::Water,
                "{id}: town {} has a dry dock",
                town.id.as_str()
            );
        }
    }
}

#[test]
fn a_new_game_starts_for_every_class_and_buries_the_scepter() {
    let resources = resources();
    for class in 0..4 {
        for seed in [0, 7, 99, 200] {
            let mut game = Game::default();
            game.init_seeded_with_resources(&resources, Some("Tester"), class, 1, seed);
            assert!(
                !game.position.zone.is_empty(),
                "class {class} seed {seed}: no start zone"
            );
            assert!(
                game.scepter.x >= 0 && game.scepter.y >= 0,
                "class {class} seed {seed}: the scepter was not buried"
            );
            let map = Map::load_zone(&resources, game.scepter.zone.as_str()).expect("scepter zone");
            let tile = map
                .tile_i32(game.scepter.x, game.scepter.y)
                .expect("scepter tile");
            assert!(!tile.blocks_foot, "the scepter lies under a blocked tile");
        }
    }
}

/// One whole game on our pack: a contract from a town, a fight with a
/// wandering army, the contracted villain's castle taken, then the Scepter
/// dug up. The hero is handed a dragon army so every battle is a sure win;
/// what this proves is that the pack's own data -- its contract cycle,
/// villain garrisons, foe placements, rewards and scoring -- carries a game
/// from start to finish, not that a starting army is balanced.
#[test]
fn a_full_game_runs_from_contract_to_scepter() {
    let resources = resources();
    let mut game = Game::default();
    game.init_seeded_with_resources(&resources, Some("Tester"), 0, 1, 7);
    assert!(!game.is_over());

    game.army = Default::default();
    game.army[0].id.copy_from_str("dragons");
    game.army[0].count = 400;
    game.stats.leadership_base = 1_000_000;
    game.stats.leadership_current = 1_000_000;

    // A contract, taken while standing in a town.
    let town = resources.town_by_index(0).expect("a town");
    game.position.in_town.copy_from_str(town.id.as_str());
    let villain_id = game
        .take_next_contract(&resources)
        .expect("the first contract is offered")
        .to_owned();
    let villain = *resources
        .villain_by_id(&villain_id)
        .expect("a known villain");

    // A wandering army, attacked and beaten.
    let foe = game.foes[..game.foe_count]
        .iter()
        .find(|foe| foe.alive && !foe.friendly)
        .copied()
        .expect("the pack places hostile armies");
    let mut map = Map::load_zone(&resources, foe.zone.as_str()).expect("foe zone map");
    let gold_before = game.stats.gold;
    let (result, temp_death) = game.attack_hostile_foe(
        &mut map,
        &resources,
        foe.placement_id.as_str(),
        foe.x,
        foe.y,
    );
    assert_eq!(result, CombatResult::Win);
    assert!(!temp_death);
    assert!(!game.foes[..game.foe_count]
        .iter()
        .any(|f| f.placement_id == foe.placement_id && f.alive));
    assert!(game.stats.gold > gold_before, "a won battle pays spoils");

    // The contracted villain's castle, besieged and taken.
    let castle_id = game
        .castles
        .iter()
        .find(|castle| castle.villain_id.as_str() == villain_id)
        .map(|castle| castle.id.as_str().to_owned())
        .expect("the contracted villain holds a castle");
    let gold_before = game.stats.gold;
    let (result, outcome) = game.siege_villain_castle(&resources, &castle_id);
    assert_eq!(result, CombatResult::Win);
    assert_eq!(outcome, VillainSiegeOutcome::Captured);
    assert!(game.contract.villains_caught[villain.index as usize]);
    assert!(game.stats.gold >= gold_before + villain.reward);

    // The Scepter, dug up: the game is won and scores.
    game.position.zone = game.scepter.zone;
    game.position.x = game.scepter.x;
    game.position.y = game.scepter.y;
    assert!(matches!(
        search_here(&mut game, &resources),
        SearchOutcome::Won
    ));
    assert!(game.stats.won);
    assert!(game.compute_score(&resources) > 0);
}

#[test]
fn digging_on_the_scepter_wins_the_game() {
    let resources = resources();
    let mut game = Game::default();
    game.init_seeded_with_resources(&resources, Some("Tester"), 0, 1, 7);
    game.position.zone = game.scepter.zone;
    game.position.x = game.scepter.x;
    game.position.y = game.scepter.y;
    assert!(matches!(
        search_here(&mut game, &resources),
        SearchOutcome::Won
    ));
    assert!(game.stats.won);
}
