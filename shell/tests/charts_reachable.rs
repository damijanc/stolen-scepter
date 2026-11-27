//! Every continent's sailing chart can be found, whatever the seed.
//!
//! A new game puts each continent's chart (and its other salted finds) in
//! one of that continent's fixed chest spots, chosen by the seed, and a
//! chart is the only way to unlock the next continent. So the game can only
//! be finished if, on every continent, EVERY chest spot can be reached from
//! where the hero lands there: on foot, or by boat over water.

use std::collections::VecDeque;
use std::path::Path;

use openbounty_engine::map::Map;
use openbounty_engine::tile::Terrain;
use openbounty_shell::pack;

/// Tiles a hero can reach from `(sx, sy)`: walkable land, plus water by boat.
fn reachable(map: &Map, sx: i32, sy: i32) -> Vec<Vec<bool>> {
    let (w, h) = (map.width as i32, map.height as i32);
    let mut seen = vec![vec![false; w as usize]; h as usize];
    let mut queue = VecDeque::from([(sx, sy)]);
    seen[sy as usize][sx as usize] = true;
    while let Some((x, y)) = queue.pop_front() {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let (nx, ny) = (x + dx, y + dy);
                if nx < 0 || ny < 0 || nx >= w || ny >= h || seen[ny as usize][nx as usize] {
                    continue;
                }
                let Some(tile) = map.tile_i32(nx, ny) else {
                    continue;
                };
                if tile.terrain == Terrain::Water || !tile.blocks_foot {
                    seen[ny as usize][nx as usize] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
    }
    seen
}

#[test]
fn every_chest_spot_on_every_continent_can_be_reached() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/stolen-scepter");
    let pack = pack::Pack::open(&dir).expect("the pack opens");
    let resources = pack::load_resources(&pack).expect("the pack loads");
    let mut unreachable = Vec::new();
    for index in 0..resources.zone_count() {
        let zone = resources.zone_by_index(index).expect("zone");
        let id = zone.id.as_str();
        let salted = zone.salt.artifacts
            + zone.salt.navmaps
            + zone.salt.orbs
            + zone.salt.telecaves
            + zone.salt.dwellings
            + zone.salt.friendly_foes;
        assert!(zone.salt.navmaps >= 1, "{id} hides no chart");
        assert!(
            zone.chest_count >= salted as usize,
            "{id}: {} chest spots for {salted} salted finds, so nothing is placed",
            zone.chest_count
        );
        let map = Map::load_zone(&resources, id).expect("map loads");
        let seen = reachable(&map, zone.hero_spawn_x, zone.hero_spawn_y);
        for chest in &zone.chests[..zone.chest_count] {
            if !seen[chest.y as usize][chest.x as usize] {
                unreachable.push(format!("{id} ({},{})", chest.x, chest.y));
            }
        }
    }
    assert!(
        unreachable.is_empty(),
        "chest spots no hero can reach: {unreachable:?}"
    );
}
