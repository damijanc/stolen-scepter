//! The game's own pack (`packs/stolen-scepter/`) loads and plays with nothing
//! from the original game on disk: it is the only pack this test opens.
//!
//! Built by `tools/gen_maps.py` + `tools/build_pack.py`; see
//! `specs/publish-plan.md`, Phase 2.

use std::collections::{HashMap, HashSet, VecDeque};

use openbounty_engine::combat::CombatResult;
use openbounty_engine::fog::Fog;
use openbounty_engine::game::{BoatActionOutcome, Game, VillainSiegeOutcome};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;
use openbounty_engine::step::{
    game_step_basic, search_here, SearchOutcome, StepInteraction, StepResult,
};
use openbounty_engine::tile::{Interact, Terrain};

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

/// A castle's five walls block the hero and its gate bounces them back, so a
/// castle dropped into a one-tile pass would wall off everything behind it.
/// The pack's generator refuses those sites; this checks the shipped maps, so
/// a hand-edited placement cannot reintroduce the trap.
#[test]
fn no_castle_footprint_severs_a_zones_walkable_land() {
    const FOOTPRINT: [(i32, i32); 6] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (0, 0), (1, 0)];

    fn walk_components(
        map: &Map,
        footprint: &HashSet<(i32, i32)>,
        block: bool,
    ) -> Vec<Vec<(i32, i32)>> {
        let walkable = |x: i32, y: i32| {
            let Some(tile) = map.tile_i32(x, y) else {
                return false;
            };
            if footprint.contains(&(x, y)) {
                // The stamped tiles sit on grass; only the stamp blocks them.
                return !block;
            }
            tile.is_bridge || (!tile.blocks_foot && tile.terrain.walkable())
        };
        let (width, height) = (map.width as i32, map.height as i32);
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for y in 0..height {
            for x in 0..width {
                if seen.contains(&(x, y)) || !walkable(x, y) {
                    continue;
                }
                let mut comp = Vec::new();
                let mut stack = vec![(x, y)];
                seen.insert((x, y));
                while let Some((cx, cy)) = stack.pop() {
                    comp.push((cx, cy));
                    for dx in -1..=1 {
                        for dy in -1..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let next = (cx + dx, cy + dy);
                            if next.0 < 0
                                || next.1 < 0
                                || next.0 >= width
                                || next.1 >= height
                                || seen.contains(&next)
                                || !walkable(next.0, next.1)
                            {
                                continue;
                            }
                            seen.insert(next);
                            stack.push(next);
                        }
                    }
                }
                out.push(comp);
            }
        }
        out
    }

    let resources = resources();
    for index in 0..resources.zone_count() {
        let zone = resources.zone_by_index(index).expect("zone");
        let id = zone.id.as_str();
        let map = Map::load_zone(&resources, id).unwrap_or_else(|e| panic!("{id}: {e}"));

        let mut footprint = HashSet::new();
        for castle_index in 0..resources.castle_count() {
            let castle = resources.castle_by_index(castle_index).expect("castle");
            if castle.zone.as_str() != id {
                continue;
            }
            for (dx, dy) in FOOTPRINT {
                footprint.insert((castle.x + dx, castle.y + dy));
            }
        }

        let owner: HashMap<(i32, i32), usize> = walk_components(&map, &footprint, true)
            .iter()
            .enumerate()
            .flat_map(|(i, comp)| comp.iter().map(move |tile| (*tile, i)))
            .collect();
        for comp in walk_components(&map, &footprint, false) {
            // The castle tiles themselves can vanish; everything else on the
            // landmass must still connect to the same region.
            let regions: HashSet<usize> = comp
                .iter()
                .filter(|tile| !footprint.contains(tile))
                .map(|tile| owner[tile])
                .collect();
            assert_eq!(
                regions.len(),
                1,
                "{id}: a castle footprint severs the walkable land"
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

/// The walk `tools/gen_maps.py` promises from the King's gate to a sea town
/// (`KING_TOWN_STEPS`): the original always had one there, and without it a
/// new hero can be stuck on the home continent with no boat in reach.
const KING_TOWN_STEPS: usize = 8;

/// Whether this water tile is joined to the map's border: where a boat sails.
fn on_open_sea(map: &Map, start: (i32, i32)) -> bool {
    let is_water = |x: i32, y: i32| {
        map.tile_i32(x, y)
            .is_some_and(|t| t.terrain == Terrain::Water)
    };
    if !is_water(start.0, start.1) {
        return false;
    }
    let (w, h) = (map.width as i32, map.height as i32);
    let mut seen = HashSet::from([start]);
    let mut queue = VecDeque::from([start]);
    while let Some((x, y)) = queue.pop_front() {
        if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
            return true;
        }
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let next = (x + dx, y + dy);
            if is_water(next.0, next.1) && seen.insert(next) {
                queue.push_back(next);
            }
        }
    }
    false
}

/// The shortest walk from the hero onto the nearest town, as step offsets.
/// Everything that bounces the hero (towns on the way, castle gates,
/// dwellings, the alcove), teleport caves and hostile armies count as walls.
fn walk_to_nearest_town(game: &Game, map: &Map) -> Option<Vec<(i32, i32)>> {
    let hostile: HashSet<(i32, i32)> = game.foes[..game.foe_count]
        .iter()
        .filter(|f| f.alive && !f.friendly && f.zone == game.position.zone)
        .map(|f| (f.x, f.y))
        .collect();
    let start = (game.position.x, game.position.y);
    let mut prev = HashMap::from([(start, start)]);
    let mut queue = VecDeque::from([start]);
    while let Some(here) = queue.pop_front() {
        for dx in -1..=1 {
            for dy in -1..=1 {
                let next = (here.0 + dx, here.1 + dy);
                if (dx, dy) == (0, 0) || prev.contains_key(&next) || hostile.contains(&next) {
                    continue;
                }
                let Some(tile) = map.tile_i32(next.0, next.1) else {
                    continue;
                };
                let passable = match tile.interactive {
                    Interact::None => !tile.blocks_foot && tile.terrain.walkable(),
                    Interact::TreasureChest
                    | Interact::Sign
                    | Interact::Artifact
                    | Interact::Orb
                    | Interact::Navmap => true,
                    _ => false,
                };
                if tile.interactive == Interact::Town {
                    prev.insert(next, here);
                    let mut steps = Vec::new();
                    let mut at = next;
                    while at != start {
                        let from = prev[&at];
                        steps.push((at.0 - from.0, at.1 - from.1));
                        at = from;
                    }
                    steps.reverse();
                    return Some(steps);
                }
                if passable {
                    prev.insert(next, here);
                    queue.push_back(next);
                }
            }
        }
    }
    None
}

#[test]
fn a_new_hero_walks_from_the_king_to_a_sea_town_and_hires_a_boat() {
    let resources = resources();
    for class in 0..4 {
        for seed in [0, 1, 2, 3, 7, 42, 99, 200, 1234, 65_535] {
            let mut game = Game::default();
            game.init_seeded_with_resources(&resources, Some("Tester"), class, 1, seed);
            let zone = game.position.zone.as_str().to_owned();
            let mut map =
                Map::load_zone_with_placements(&resources, &zone, &game).expect("home map");
            let mut fog = Fog::default();
            let what = format!("class {class} seed {seed}");

            let steps = walk_to_nearest_town(&game, &map)
                .unwrap_or_else(|| panic!("{what}: no town can be walked to"));
            assert!(
                steps.len() <= KING_TOWN_STEPS,
                "{what}: the nearest town is {} steps from the King, not {KING_TOWN_STEPS}",
                steps.len()
            );

            // Walk it for real: no army catches the hero on the way, and the
            // last step bounces off the town and opens it.
            let (last, walk) = steps.split_last().expect("at least one step");
            for &(dx, dy) in walk {
                match game_step_basic(&mut game, &mut map, &mut fog, &resources, dx, dy) {
                    StepResult::Moved {
                        collision: None,
                        bounced: false,
                        ..
                    } => {}
                    other => panic!("{what}: the walk to town was stopped: {other:?}"),
                }
            }
            let (boat_x, boat_y) =
                match game_step_basic(&mut game, &mut map, &mut fog, &resources, last.0, last.1) {
                    StepResult::Moved {
                        interaction: StepInteraction::Town { boat_x, boat_y, .. },
                        bounced: true,
                        ..
                    } => (boat_x, boat_y),
                    other => panic!("{what}: the last step did not open a town: {other:?}"),
                };
            assert!(!game.position.in_town.is_empty(), "{what}: not in town");

            assert!(
                on_open_sea(&map, (boat_x, boat_y)),
                "{what}: the town's dock ({boat_x},{boat_y}) is not on the open sea"
            );
            assert_eq!(
                game.rent_boat(&resources, boat_x, boat_y, &zone),
                BoatActionOutcome::RentOk,
                "{what}: the starting purse cannot hire a boat"
            );
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
