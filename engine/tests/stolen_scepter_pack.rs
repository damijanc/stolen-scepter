//! The game's own pack (`packs/stolen-scepter/`) loads and plays with nothing
//! from the original game on disk: it is the only pack this test opens.
//!
//! Built by `tools/gen_maps.py` + `tools/build_pack.py`; see
//! `specs/publish-plan.md`, Phase 2.

use openbounty_engine::game::Game;
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
