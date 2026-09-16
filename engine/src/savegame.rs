use crate::fog::Fog;
use crate::game::{
    ArmyStack, CastleOwnerKind, DwellingState, FoeState, Game, Mount, SaltedPlacement,
    TileMutation, TravelMode, Unit, GAME_ARMY_SLOTS, GAME_CONTINENTS,
};
use crate::map::Map;
use crate::resources::Resources;
use crate::rng::seed_from_index;
use crate::state_serialize::{build_snapshot_full, SAVE_VERSION};
use crate::tables::{Facing, FixedStr};

/// Why a save could not be written or read. Mirrors OpenBounty's
/// `SaveResult` minus its OK case, which Rust expresses as `Ok(())`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveError {
    /// The file could not be read or written.
    Io,
    /// The file is not valid JSON.
    Parse,
    /// `version` is absent or is not exactly `SAVE_VERSION`.
    Version,
    /// The file is internally inconsistent, e.g. a fog row whose width does
    /// not match the zone it belongs to.
    Mismatch,
    /// Written against a different pack. Its ids belong to that pack's
    /// catalog and would resolve wrong here, so the load is refused.
    Pack,
}

/// Write a save. OpenBounty's `SaveGameWrite` is just the state snapshot
/// printed to a file, and the loader ignores the harness-only keys.
pub fn save_write(
    path: &std::path::Path,
    game: &Game,
    resources: &Resources,
    map: &Map,
    fog: &Fog,
) -> Result<(), SaveError> {
    let root = build_snapshot_full(Some(game), Some(resources), Some(map), Some(fog));
    let text = serde_json::to_string_pretty(&root).map_err(|_| SaveError::Io)?;
    std::fs::write(path, text).map_err(|_| SaveError::Io)
}

/// Read a save back over `game`, `map` and `fog`. Ports `SaveGameRead`.
pub fn save_read(
    path: &std::path::Path,
    game: &mut Game,
    resources: &Resources,
    map: &Map,
    fog: &mut Fog,
) -> Result<(), SaveError> {
    let text = std::fs::read_to_string(path).map_err(|_| SaveError::Io)?;
    let root: serde_json::Value = serde_json::from_str(&text).map_err(|_| SaveError::Parse)?;

    // The version must match exactly: an older loader cannot be trusted with
    // a newer schema, and vice versa.
    if root.get("version").and_then(|v| v.as_u64()) != Some(SAVE_VERSION as u64) {
        return Err(SaveError::Version);
    }

    // Cross-pack guard: ids in the save belong to the other pack's catalog.
    if !resources.pack_id.is_empty() {
        if let Some(pack) = root.get("pack_id").and_then(|v| v.as_str()) {
            if pack != resources.pack_id.as_str() {
                return Err(SaveError::Pack);
            }
        }
    }

    *game = Game::default();

    read_seed(&root, game);
    read_character(&root, game, resources);
    read_stats(&root, game);
    read_position(&root, game);
    read_army(&root, game);
    read_spells(&root, game, resources);
    read_contract(&root, game, resources);
    read_artifacts(&root, game, resources);
    read_world(&root, game, resources);
    read_boat(&root, game);
    read_towns(&root, game);
    read_castles(&root, game);
    read_scepter(&root, game);
    read_consumed(&root, game);
    read_placements(&root, game);
    read_foes(&root, game);
    read_dwellings(&root, game);
    read_map_state(&root, game, resources, map, fog)?;

    Ok(())
}

fn as_i32(value: Option<&serde_json::Value>) -> Option<i32> {
    value.and_then(|v| v.as_i64()).map(|v| v as i32)
}

fn as_str<'a>(value: Option<&'a serde_json::Value>) -> &'a str {
    value.and_then(|v| v.as_str()).unwrap_or("")
}

fn as_bool(value: Option<&serde_json::Value>) -> bool {
    value.and_then(|v| v.as_bool()).unwrap_or(false)
}

fn read_seed(root: &serde_json::Value, game: &mut Game) {
    // A catalog game re-derives its seed from the stored index, reproducing
    // the world exactly; only a non-catalog game carries a raw seed.
    let from_catalog = as_bool(root.get("seed_from_catalog"));
    let index = as_i32(root.get("seed_index"));
    if from_catalog {
        if let Some(index) = index {
            game.seed_from_catalog = true;
            game.seed_index = index & 0xFF;
            game.seed = seed_from_index(game.seed_index as u8);
            return;
        }
    }
    if let Some(seed) = root.get("seed").and_then(|v| v.as_u64()) {
        game.seed = seed;
    }
}

fn read_character(root: &serde_json::Value, game: &mut Game, resources: &Resources) {
    let Some(section) = root.get("character") else {
        return;
    };
    game.character
        .name
        .copy_from_str(as_str(section.get("name")));
    game.character
        .cls
        .id
        .copy_from_str(as_str(section.get("class")));
    game.character.cls.rank_index = as_i32(section.get("rank_index")).unwrap_or(0);
    game.character
        .cls
        .rank_title
        .copy_from_str(as_str(section.get("rank_title")));
    // The rank id is not stored: it is re-derived from the class catalog,
    // clamped to the four ranks C allows.
    if let Some(class) = resources.class_by_id(game.character.cls.id.as_str()) {
        let rank = game.character.cls.rank_index.clamp(0, 3) as usize;
        if rank < class.ranks.len() {
            game.character
                .cls
                .rank_id
                .copy_from_str(class.ranks[rank].id.as_str());
        }
    }
    game.character.difficulty = match as_str(section.get("difficulty")) {
        "normal" => 1,
        "hard" => 2,
        "impossible" => 3,
        _ => 0,
    };
    game.character.mount = match as_str(section.get("mount")) {
        "sail" => Mount::Sail,
        "fly" => Mount::Fly,
        _ => Mount::Ride,
    };
}

fn read_stats(root: &serde_json::Value, game: &mut Game) {
    let Some(section) = root.get("stats") else {
        return;
    };
    let stats = &mut game.stats;
    let int = |key: &str| as_i32(section.get(key));
    stats.gold = int("gold").unwrap_or(stats.gold);
    stats.commission_weekly = int("commission_weekly").unwrap_or(stats.commission_weekly);
    stats.leadership_base = int("leadership_base").unwrap_or(stats.leadership_base);
    stats.leadership_current = int("leadership_current").unwrap_or(stats.leadership_current);
    stats.followers_killed = int("followers_killed").unwrap_or(stats.followers_killed);
    stats.score = int("score").unwrap_or(stats.score);
    stats.spell_power = int("spell_power").unwrap_or(stats.spell_power);
    stats.max_spells = int("max_spells").unwrap_or(stats.max_spells);
    stats.siege_weapons = int("siege_weapons").unwrap_or(stats.siege_weapons);
    stats.time_stop = int("time_stop").unwrap_or(stats.time_stop);
    stats.steps_left_today = int("steps_left_today").unwrap_or(stats.steps_left_today);
    stats.days_left = int("days_left").unwrap_or(stats.days_left);
    stats.last_commission = int("last_commission").unwrap_or(stats.last_commission);
    stats.knows_magic = as_bool(section.get("knows_magic"));
    stats.game_over = as_bool(section.get("game_over"));
    stats.won = as_bool(section.get("won"));
    stats.last_astrology_troop = int("last_astrology_troop").unwrap_or(stats.last_astrology_troop);
    if let Some(options) = section.get("options").and_then(|v| v.as_array()) {
        for (slot, value) in options.iter().take(stats.options.len()).enumerate() {
            stats.options[slot] = value.as_i64().unwrap_or(0) as i32;
        }
    }
}

fn read_position(root: &serde_json::Value, game: &mut Game) {
    let Some(section) = root.get("position") else {
        return;
    };
    game.position
        .zone
        .copy_from_str(as_str(section.get("zone")));
    game.position.x = as_i32(section.get("x")).unwrap_or(0);
    game.position.y = as_i32(section.get("y")).unwrap_or(0);
    game.position.last_x = as_i32(section.get("last_x")).unwrap_or(0);
    game.position.last_y = as_i32(section.get("last_y")).unwrap_or(0);
    game.position.facing_left = as_bool(section.get("facing_left"));
    // Absent in saves written before facings existed, so derive it from
    // facing_left rather than bumping the save version and orphaning them.
    // A stored number that is out of range falls back to south.
    game.position.facing = match as_i32(section.get("facing")) {
        Some(stored) => Facing::from_i32(stored).unwrap_or(Facing::South),
        None if game.position.facing_left => Facing::West,
        None => Facing::East,
    };
    game.travel_mode = match as_str(section.get("travel_mode")) {
        "boat" => TravelMode::Boat,
        _ => TravelMode::Walk,
    };
}

fn read_army(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("army").and_then(|v| v.as_array()) else {
        return;
    };
    game.army = [ArmyStack::default(); GAME_ARMY_SLOTS];
    for entry in entries {
        let Some(slot) = as_i32(entry.get("slot")) else {
            continue;
        };
        if slot < 0 || slot as usize >= GAME_ARMY_SLOTS {
            continue;
        }
        game.army[slot as usize]
            .id
            .copy_from_str(as_str(entry.get("troop")));
        game.army[slot as usize].count = as_i32(entry.get("count")).unwrap_or(0);
    }
}

fn read_spells(root: &serde_json::Value, game: &mut Game, resources: &Resources) {
    let Some(section) = root.get("spells").and_then(|v| v.as_object()) else {
        return;
    };
    for (id, count) in section {
        let Some(spell) = resources.spell_by_id(id) else {
            continue;
        };
        let Ok(index) = usize::try_from(spell.index) else {
            continue;
        };
        if index < game.spells.counts.len() {
            game.spells.counts[index] = count.as_i64().unwrap_or(0) as i32;
        }
    }
}

fn read_contract(root: &serde_json::Value, game: &mut Game, resources: &Resources) {
    let Some(section) = root.get("contract") else {
        return;
    };
    game.contract
        .active_id
        .copy_from_str(as_str(section.get("active")));
    game.contract.last_contract = as_i32(section.get("last_contract")).unwrap_or(0);
    game.contract.max_contract = as_i32(section.get("max_contract")).unwrap_or(0);
    if let Some(cycle) = section.get("cycle").and_then(|v| v.as_array()) {
        for (slot, value) in cycle.iter().take(game.contract.cycle.len()).enumerate() {
            game.contract.cycle[slot].copy_from_str(value.as_str().unwrap_or(""));
        }
    }
    read_villain_flags(
        section.get("villains_caught"),
        &mut game.contract.villains_caught,
        resources,
    );
    read_villain_flags(
        section.get("villains_prefought"),
        &mut game.contract.villains_prefought,
        resources,
    );
}

fn read_villain_flags(
    value: Option<&serde_json::Value>,
    flags: &mut [bool],
    resources: &Resources,
) {
    let Some(ids) = value.and_then(|v| v.as_array()) else {
        return;
    };
    for id in ids {
        let Some(villain) = resources.villain_by_id(id.as_str().unwrap_or("")) else {
            continue;
        };
        if let Ok(index) = usize::try_from(villain.index) {
            if index < flags.len() {
                flags[index] = true;
            }
        }
    }
}

fn read_artifacts(root: &serde_json::Value, game: &mut Game, resources: &Resources) {
    let Some(found) = root
        .get("artifacts")
        .and_then(|v| v.get("found"))
        .and_then(|v| v.as_array())
    else {
        return;
    };
    for id in found {
        let Some(artifact) = resources.artifact_by_id(id.as_str().unwrap_or("")) else {
            continue;
        };
        if let Ok(index) = usize::try_from(artifact.index) {
            if index < game.artifacts.found.len() {
                game.artifacts.found[index] = true;
            }
        }
    }
}

fn read_world(root: &serde_json::Value, game: &mut Game, resources: &Resources) {
    let Some(section) = root.get("world") else {
        return;
    };
    let mark = |key: &str, flags: &mut [bool]| {
        let Some(ids) = section.get(key).and_then(|v| v.as_array()) else {
            return;
        };
        for id in ids {
            let Some(index) = resources.zone_index_by_id(id.as_str().unwrap_or("")) else {
                continue;
            };
            if index < flags.len() {
                flags[index] = true;
            }
        }
    };
    mark("zones_discovered", &mut game.world.zones_discovered);
    mark("orbs_found", &mut game.world.orbs_found);
}

fn read_boat(root: &serde_json::Value, game: &mut Game) {
    let Some(section) = root.get("boat") else {
        return;
    };
    game.boat.has_boat = as_bool(section.get("has_boat"));
    game.boat.x = as_i32(section.get("x")).unwrap_or(-1);
    game.boat.y = as_i32(section.get("y")).unwrap_or(-1);
    game.boat.zone.copy_from_str(as_str(section.get("zone")));
}

fn read_towns(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("towns").and_then(|v| v.as_array()) else {
        return;
    };
    for (slot, entry) in entries.iter().take(game.towns.len()).enumerate() {
        let town = &mut game.towns[slot];
        town.id.copy_from_str(as_str(entry.get("id")));
        town.visited = as_bool(entry.get("visited"));
        town.spell_for_sale
            .copy_from_str(as_str(entry.get("spell_for_sale")));
    }
}

fn read_castles(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("castles").and_then(|v| v.as_array()) else {
        return;
    };
    for (slot, entry) in entries.iter().take(game.castles.len()).enumerate() {
        let castle = &mut game.castles[slot];
        castle.id.copy_from_str(as_str(entry.get("id")));
        castle.visited = as_bool(entry.get("visited"));
        castle.known = as_bool(entry.get("known"));
        castle.owner_kind = match as_str(entry.get("owner")) {
            "player" => CastleOwnerKind::Player,
            "villain" => CastleOwnerKind::Villain,
            "special" => CastleOwnerKind::Special,
            _ => CastleOwnerKind::Monsters,
        };
        castle
            .villain_id
            .copy_from_str(as_str(entry.get("villain")));
        castle.garrison = [Unit::default(); GAME_ARMY_SLOTS];
        // NOTE the asymmetry: a castle garrison names its troop "troop"
        // (C's shared build_unit_array / parse_unit_array), while a foe
        // garrison writes "id" inline. Both are C's shape, not a slip.
        if let Some(units) = entry.get("garrison").and_then(|v| v.as_array()) {
            for (index, unit) in units.iter().take(GAME_ARMY_SLOTS).enumerate() {
                castle.garrison[index]
                    .id
                    .copy_from_str(as_str(unit.get("troop")));
                castle.garrison[index].count = as_i32(unit.get("count")).unwrap_or(0);
            }
        }
    }
}

fn read_scepter(root: &serde_json::Value, game: &mut Game) {
    let Some(section) = root.get("scepter") else {
        return;
    };
    game.scepter.zone.copy_from_str(as_str(section.get("zone")));
    game.scepter.x = as_i32(section.get("x")).unwrap_or(-1);
    game.scepter.y = as_i32(section.get("y")).unwrap_or(-1);
}

fn read_consumed(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("consumed").and_then(|v| v.as_array()) else {
        return;
    };
    game.consumed_count = 0;
    for entry in entries.iter().take(game.consumed.len()) {
        let mutation = TileMutation {
            zone: crate::tables::FixedStr::from_str(as_str(entry.get("zone"))),
            x: as_i32(entry.get("x")).unwrap_or(0),
            y: as_i32(entry.get("y")).unwrap_or(0),
        };
        game.consumed[game.consumed_count] = mutation;
        game.consumed_count += 1;
    }
}

fn read_placements(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("placements").and_then(|v| v.as_array()) else {
        return;
    };
    game.placement_count = 0;
    for entry in entries.iter().take(game.placements.len()) {
        let mut placement = SaltedPlacement::default();
        placement.zone.copy_from_str(as_str(entry.get("zone")));
        placement.x = as_i32(entry.get("x")).unwrap_or(0);
        placement.y = as_i32(entry.get("y")).unwrap_or(0);
        placement.kind = crate::tile::Interact::from_str(as_str(entry.get("kind")));
        placement.id.copy_from_str(as_str(entry.get("id")));
        game.placements[game.placement_count] = placement;
        game.placement_count += 1;
    }
}

fn read_foes(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("foes").and_then(|v| v.as_array()) else {
        return;
    };
    game.foe_count = 0;
    for entry in entries.iter().take(game.foes.len()) {
        let mut foe = FoeState::default();
        foe.zone.copy_from_str(as_str(entry.get("zone")));
        foe.x = as_i32(entry.get("x")).unwrap_or(0);
        foe.y = as_i32(entry.get("y")).unwrap_or(0);
        foe.origin_x = as_i32(entry.get("origin_x")).unwrap_or(0);
        foe.origin_y = as_i32(entry.get("origin_y")).unwrap_or(0);
        foe.placement_id.copy_from_str(as_str(entry.get("id")));
        foe.alive = as_bool(entry.get("alive"));
        foe.friendly = as_bool(entry.get("friendly"));
        if let Some(units) = entry.get("garrison").and_then(|v| v.as_array()) {
            for (index, unit) in units.iter().take(GAME_ARMY_SLOTS).enumerate() {
                foe.garrison[index].id.copy_from_str(as_str(unit.get("id")));
                foe.garrison[index].count = as_i32(unit.get("count")).unwrap_or(0);
            }
        }
        game.foes[game.foe_count] = foe;
        game.foe_count += 1;
    }
}

fn read_dwellings(root: &serde_json::Value, game: &mut Game) {
    let Some(entries) = root.get("dwellings").and_then(|v| v.as_array()) else {
        return;
    };
    game.dwelling_count = 0;
    for entry in entries.iter().take(game.dwellings.len()) {
        let mut dwelling = DwellingState::default();
        dwelling.zone.copy_from_str(as_str(entry.get("zone")));
        dwelling.x = as_i32(entry.get("x")).unwrap_or(0);
        dwelling.y = as_i32(entry.get("y")).unwrap_or(0);
        dwelling.troop_id.copy_from_str(as_str(entry.get("troop")));
        dwelling.count = as_i32(entry.get("count")).unwrap_or(0);
        dwelling.max_population = as_i32(entry.get("max_population")).unwrap_or(0);
        game.dwellings[game.dwelling_count] = dwelling;
        game.dwelling_count += 1;
    }
}

/// Decode one hex fog row. The string length must match the zone's width
/// exactly, which is how a save written for a differently-sized zone is
/// caught.
fn decode_fog_row(fog: &mut Fog, row: usize, width: usize, text: &str) -> bool {
    let nibbles = width.div_ceil(4);
    if text.len() != nibbles {
        return false;
    }
    for (n, ch) in text.chars().enumerate() {
        let Some(v) = ch.to_digit(16) else {
            return false;
        };
        for b in 0..4 {
            let x = n * 4 + b;
            if x >= width {
                break;
            }
            if row < fog.seen.len() && x < fog.seen[row].len() {
                fog.seen[row][x] = (v >> (3 - b)) & 1 == 1;
            }
        }
    }
    true
}

fn read_map_state(
    root: &serde_json::Value,
    game: &mut Game,
    resources: &Resources,
    map: &Map,
    fog: &mut Fog,
) -> Result<(), SaveError> {
    let Some(section) = root.get("map_state").and_then(|v| v.as_object()) else {
        return Ok(());
    };
    let _ = map;
    let active = game.position.zone;
    for (zone_id, entry) in section {
        let Some(zone_index) = resources.zone_index_by_id(zone_id) else {
            continue;
        };
        if zone_index >= GAME_CONTINENTS {
            continue;
        }
        let Some(zone) = resources.zone_by_index(zone_index) else {
            continue;
        };
        // Both dimensions come from the zone's DECLARED size, even for the
        // active zone whose rows were written at the live map's width.
        let width = zone.width.max(0) as usize;
        let height = zone.height.max(0) as usize;
        let Some(rows) = entry.get("fog").and_then(|v| v.as_array()) else {
            continue;
        };
        let destination = &mut game.world.continent_fog[zone_index];
        for (y, row) in rows.iter().enumerate() {
            if y >= height {
                break;
            }
            let Some(text) = row.as_str() else {
                continue;
            };
            if !decode_fog_row(destination, y, width, text) {
                return Err(SaveError::Mismatch);
            }
        }
        // The active zone's snapshot is also the live fog.
        if zone_id.as_str() == active.as_str() {
            *fog = *destination;
        }
    }
    Ok(())
}

/// Lightweight summary for the save-slot picker, with no map or fog load.
/// Ports OpenBounty's `SaveHeader`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SaveHeader {
    pub exists: bool,
    pub name: FixedStr<32>,
    pub class_id: FixedStr<24>,
    /// Denormalized for display, so the picker needs no class catalog.
    pub rank_title: FixedStr<32>,
    pub zone: FixedStr<24>,
    pub days_left: i32,
    pub gold: i32,
    pub pack_id: FixedStr<64>,
    /// Whole-pack zip hash. Advisory only; the Rust engine never writes it,
    /// but a save written by the C engine carries one.
    pub pack_hash: FixedStr<17>,
}

/// Read just enough of a save to list it in a slot picker. Ports
/// `SaveGameReadHeader`, which applies the version guard but NOT the
/// cross-pack guard: the picker shows a save from another pack, and the
/// refusal comes later when it is actually loaded.
pub fn save_read_header(path: &std::path::Path) -> Result<SaveHeader, SaveError> {
    let text = std::fs::read_to_string(path).map_err(|_| SaveError::Io)?;
    let root: serde_json::Value = serde_json::from_str(&text).map_err(|_| SaveError::Parse)?;

    if root.get("version").and_then(|v| v.as_u64()) != Some(SAVE_VERSION as u64) {
        return Err(SaveError::Version);
    }

    let mut header = SaveHeader {
        exists: true,
        ..SaveHeader::default()
    };
    if let Some(character) = root.get("character") {
        header.name.copy_from_str(as_str(character.get("name")));
        header
            .class_id
            .copy_from_str(as_str(character.get("class")));
        header
            .rank_title
            .copy_from_str(as_str(character.get("rank_title")));
    }
    if let Some(stats) = root.get("stats") {
        header.days_left = as_i32(stats.get("days_left")).unwrap_or(0);
        header.gold = as_i32(stats.get("gold")).unwrap_or(0);
    }
    if let Some(position) = root.get("position") {
        header.zone.copy_from_str(as_str(position.get("zone")));
    }
    header.pack_id.copy_from_str(as_str(root.get("pack_id")));
    header
        .pack_hash
        .copy_from_str(as_str(root.get("pack_hash")));
    Ok(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tile::Interact;

    fn save_resources() -> Resources {
        Resources::from_json_str(
            r#"{
              "pack_id": "openbounty",
              "world": {"starting_zone": "home"},
              "zones": [
                {"id":"home","width":8,"height":4},
                {"id":"away","width":4,"height":2}
              ],
              "classes": [
                {"index":0,"id":"knight","starting_gold":1000,
                 "ranks":[
                   {"id":"general","name":"General","villains_needed":0,"leadership":100,"max_spells":1,"spell_power":1,"commission":10,"knows_magic":false},
                   {"id":"captain","name":"Captain","villains_needed":2,"leadership":50,"max_spells":2,"spell_power":3,"commission":20,"knows_magic":false}
                 ]}
              ],
              "spells": [
                {"index":0,"id":"bridge","kind":"adventure","cost":100},
                {"index":1,"id":"fireball","kind":"combat","cost":200}
              ],
              "artifacts": [
                {"index":0,"id":"anchor","power":"cheaper_boats"},
                {"index":1,"id":"crown","power":"double_leadership"}
              ],
              "villains": [
                {"index":0,"id":"lord0","name":"Lord Zero","zone":"home","reward":1000},
                {"index":1,"id":"lord1","name":"Lord One","zone":"home","reward":2000}
              ],
              "troops": [
                {"index":0,"id":"peasants","skill_level":1,"hit_points":1,"move_rate":2,"melee":[1,2],"ranged":[0,0,0],"recruit_cost":10,"spoils_factor":1,"abilities":"","morale_group":"A"},
                {"index":1,"id":"ogres","skill_level":4,"hit_points":5,"move_rate":2,"melee":[3,6],"ranged":[0,0,0],"recruit_cost":40,"spoils_factor":4,"abilities":"","morale_group":"A"}
              ]
            }"#,
        )
        .expect("fixture parses")
    }

    /// A game with something set in every section the save format carries.
    fn populated_game() -> Game {
        let mut game = Game::default();
        game.seed = 987_654;
        game.character.name.copy_from_str("Arthur");
        game.character.cls.id.copy_from_str("knight");
        game.character.cls.rank_index = 1;
        game.character.cls.rank_title.copy_from_str("Captain");
        game.character.cls.rank_id.copy_from_str("captain");
        game.character.difficulty = 2;
        game.character.mount = Mount::Fly;

        game.stats.gold = 4_321;
        game.stats.commission_weekly = 250;
        game.stats.leadership_base = 300;
        game.stats.leadership_current = 275;
        game.stats.followers_killed = 42;
        game.stats.score = 7;
        game.stats.spell_power = 3;
        game.stats.max_spells = 5;
        game.stats.siege_weapons = 1;
        game.stats.time_stop = 2;
        game.stats.steps_left_today = 17;
        game.stats.days_left = 300;
        game.stats.last_commission = 250;
        game.stats.knows_magic = true;
        game.stats.last_astrology_troop = 1;
        game.stats.options = [4, 1, 1, 1, 1, 0, 5];

        game.position.zone.copy_from_str("home");
        game.position.x = 3;
        game.position.y = 2;
        game.position.last_x = 2;
        game.position.last_y = 2;
        game.position.facing = Facing::North;
        game.position.facing_left = true;
        game.travel_mode = TravelMode::Boat;

        game.army[0].id.copy_from_str("ogres");
        game.army[0].count = 12;
        game.army[2].id.copy_from_str("peasants");
        game.army[2].count = 30;

        game.spells.counts[0] = 3;
        game.spells.counts[1] = 1;

        game.contract.active_id.copy_from_str("lord1");
        game.contract.last_contract = 1;
        game.contract.max_contract = 2;
        game.contract.cycle[0].copy_from_str("lord0");
        game.contract.cycle[1].copy_from_str("lord1");
        game.contract.villains_caught[0] = true;
        game.contract.villains_prefought[1] = true;

        game.artifacts.found[0] = true;

        game.world.zones_discovered[0] = true;
        game.world.zones_discovered[1] = true;
        game.world.orbs_found[0] = true;

        game.boat.has_boat = true;
        game.boat.x = 5;
        game.boat.y = 1;
        game.boat.zone.copy_from_str("home");

        game.towns[0].id.copy_from_str("town0");
        game.towns[0].visited = true;
        game.towns[0].spell_for_sale.copy_from_str("bridge");

        game.castles[0].id.copy_from_str("keep");
        game.castles[0].visited = true;
        game.castles[0].known = true;
        game.castles[0].owner_kind = CastleOwnerKind::Villain;
        game.castles[0].villain_id.copy_from_str("lord0");
        game.castles[0].garrison[0].id.copy_from_str("ogres");
        game.castles[0].garrison[0].count = 9;

        game.scepter.zone.copy_from_str("away");
        game.scepter.x = 1;
        game.scepter.y = 1;

        game.consumed[0] = TileMutation {
            zone: FixedStr::from_str("home"),
            x: 6,
            y: 3,
        };
        game.consumed_count = 1;

        let mut placement = SaltedPlacement::default();
        placement.zone.copy_from_str("home");
        placement.x = 2;
        placement.y = 1;
        placement.kind = Interact::Artifact;
        placement.id.copy_from_str("anchor");
        game.placements[0] = placement;
        game.placement_count = 1;

        let mut foe = FoeState::default();
        foe.zone.copy_from_str("home");
        foe.x = 4;
        foe.y = 3;
        foe.origin_x = 4;
        foe.origin_y = 3;
        foe.placement_id.copy_from_str("band0");
        foe.alive = true;
        foe.friendly = true;
        foe.garrison[0].id.copy_from_str("peasants");
        foe.garrison[0].count = 8;
        game.foes[0] = foe;
        game.foe_count = 1;

        let mut dwelling = DwellingState::default();
        dwelling.zone.copy_from_str("home");
        dwelling.x = 7;
        dwelling.y = 0;
        dwelling.troop_id.copy_from_str("peasants");
        dwelling.count = 6;
        dwelling.max_population = 10;
        game.dwellings[0] = dwelling;
        game.dwelling_count = 1;

        game.world.continent_fog[1].seen[1][0] = true;
        game
    }

    fn temp_path(name: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("openbounty-{}-{}.json", name, std::process::id()));
        path
    }

    #[test]
    fn a_save_round_trips_every_field_the_format_carries() {
        let resources = save_resources();
        let game = populated_game();
        let map = Map::new(8, 4);
        let mut fog = Fog::new();
        fog.reveal(&map, 3, 2, 3);
        let path = temp_path("roundtrip");

        save_write(&path, &game, &resources, &map, &fog).expect("write succeeds");

        let mut loaded = Game::default();
        let mut loaded_fog = Fog::new();
        save_read(&path, &mut loaded, &resources, &map, &mut loaded_fog).expect("read succeeds");
        let _ = std::fs::remove_file(&path);

        // Compare section by section so a failure names the culprit.
        assert_eq!(loaded.seed, game.seed, "seed");
        assert_eq!(loaded.character, game.character, "character");
        assert_eq!(loaded.stats, game.stats, "stats");
        assert_eq!(loaded.position, game.position, "position");
        assert_eq!(loaded.travel_mode, game.travel_mode, "travel_mode");
        assert_eq!(loaded.army, game.army, "army");
        assert_eq!(loaded.spells, game.spells, "spells");
        assert_eq!(loaded.contract, game.contract, "contract");
        assert_eq!(loaded.artifacts, game.artifacts, "artifacts");
        assert_eq!(
            loaded.world.zones_discovered, game.world.zones_discovered,
            "zones"
        );
        assert_eq!(loaded.world.orbs_found, game.world.orbs_found, "orbs");
        assert_eq!(loaded.boat, game.boat, "boat");
        assert_eq!(loaded.towns, game.towns, "towns");
        assert_eq!(loaded.castles, game.castles, "castles");
        assert_eq!(loaded.scepter, game.scepter, "scepter");
        assert_eq!(loaded.consumed_count, game.consumed_count, "consumed_count");
        assert_eq!(
            loaded.placement_count, game.placement_count,
            "placement_count"
        );
        assert_eq!(loaded.placements[0], game.placements[0], "placement");
        assert_eq!(loaded.foe_count, game.foe_count, "foe_count");
        assert_eq!(loaded.foes[0], game.foes[0], "foe");
        assert_eq!(loaded.dwelling_count, game.dwelling_count, "dwelling_count");
        assert_eq!(loaded.dwellings[0], game.dwellings[0], "dwelling");
        // A load writes every zone's decoded rows into continent_fog,
        // INCLUDING the active zone, and then copies that into the live fog.
        // So the active slot comes back upgraded to the live fog rather than
        // to whatever stale snapshot the source game held -- C does the same.
        let mut expected = game;
        expected.world.continent_fog[0] = fog;
        assert_eq!(loaded, expected);
        assert_eq!(loaded_fog, fog, "the active zone's live fog is restored");
        assert_eq!(
            loaded.world.continent_fog[1], game.world.continent_fog[1],
            "an inactive zone keeps its own snapshot"
        );
    }

    #[test]
    fn a_version_mismatch_is_refused() {
        let resources = save_resources();
        let path = temp_path("version");
        std::fs::write(&path, r#"{"version": 8, "pack_id": "openbounty"}"#).expect("write");

        let mut game = Game::default();
        let map = Map::new(8, 4);
        let mut fog = Fog::new();
        let result = save_read(&path, &mut game, &resources, &map, &mut fog);
        let _ = std::fs::remove_file(&path);

        assert_eq!(result, Err(SaveError::Version));
    }

    #[test]
    fn a_save_from_another_pack_is_refused() {
        let resources = save_resources();
        let path = temp_path("pack");
        std::fs::write(
            &path,
            format!(r#"{{"version": {SAVE_VERSION}, "pack_id": "someone_elses_pack"}}"#),
        )
        .expect("write");

        let mut game = Game::default();
        let map = Map::new(8, 4);
        let mut fog = Fog::new();
        let result = save_read(&path, &mut game, &resources, &map, &mut fog);
        let _ = std::fs::remove_file(&path);

        assert_eq!(result, Err(SaveError::Pack));
    }

    /// Load a hand-written save whose `position` section is `body`.
    fn read_position_json(tag: &str, body: &str) -> Game {
        let resources = save_resources();
        let path = temp_path(tag);
        std::fs::write(
            &path,
            format!(
                r#"{{"version": {SAVE_VERSION}, "pack_id": "openbounty",
                     "position": {body}}}"#
            ),
        )
        .expect("write");

        let mut game = Game::default();
        let map = Map::new(8, 4);
        let mut fog = Fog::new();
        let result = save_read(&path, &mut game, &resources, &map, &mut fog);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Ok(()), "the fixture save is otherwise valid");
        game
    }

    #[test]
    fn a_save_written_before_facings_existed_derives_one_from_the_mirror() {
        // Four-way facing was added after the mirror flag, so a save may carry
        // only facing_left. C derives the facing rather than bumping the save
        // version, which would have orphaned every existing save.
        let looking_west = read_position_json("facing-legacy-w", r#"{"facing_left": true}"#);
        assert_eq!(looking_west.position.facing, Facing::West);
        assert!(looking_west.position.facing_left);

        let looking_east = read_position_json("facing-legacy-e", r#"{"facing_left": false}"#);
        assert_eq!(looking_east.position.facing, Facing::East);
        assert!(!looking_east.position.facing_left);
    }

    #[test]
    fn a_stored_facing_wins_over_the_mirror_unless_it_is_out_of_range() {
        // A pack with four authored facings writes both, and the number is
        // the one that counts -- here north, which the mirror cannot express.
        let north = read_position_json("facing-both", r#"{"facing_left": true, "facing": 3}"#);
        assert_eq!(north.position.facing, Facing::North);
        assert!(north.position.facing_left);

        // Out of range is not an error: C clamps it to south rather than
        // refusing the save.
        let junk = read_position_json("facing-junk", r#"{"facing_left": true, "facing": 9}"#);
        assert_eq!(junk.position.facing, Facing::South);
    }

    #[test]
    fn malformed_json_and_missing_files_are_distinguished() {
        let resources = save_resources();
        let map = Map::new(8, 4);
        let mut game = Game::default();
        let mut fog = Fog::new();

        let missing = temp_path("nope-does-not-exist");
        assert_eq!(
            save_read(&missing, &mut game, &resources, &map, &mut fog),
            Err(SaveError::Io)
        );

        let path = temp_path("garbage");
        std::fs::write(&path, "{not json").expect("write");
        let result = save_read(&path, &mut game, &resources, &map, &mut fog);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Err(SaveError::Parse));
    }

    #[test]
    fn a_fog_row_of_the_wrong_width_is_a_mismatch() {
        let resources = save_resources();
        let path = temp_path("fogwidth");
        // Zone "home" is 8 wide, so each row needs 2 nibbles, not 5.
        std::fs::write(
            &path,
            format!(
                r#"{{"version": {SAVE_VERSION}, "pack_id": "openbounty",
                     "position": {{"zone": "home"}},
                     "map_state": {{"home": {{"fog": ["fffff"]}}}}}}"#
            ),
        )
        .expect("write");

        let mut game = Game::default();
        let map = Map::new(8, 4);
        let mut fog = Fog::new();
        let result = save_read(&path, &mut game, &resources, &map, &mut fog);
        let _ = std::fs::remove_file(&path);

        assert_eq!(result, Err(SaveError::Mismatch));
    }

    #[test]
    fn a_catalog_game_rederives_its_seed_from_the_stored_index() {
        let resources = save_resources();
        let mut game = populated_game();
        game.seed_from_catalog = true;
        game.seed_index = 7;
        game.seed = crate::rng::seed_from_index(7);
        let map = Map::new(8, 4);
        let fog = Fog::new();
        let path = temp_path("catalog");

        save_write(&path, &game, &resources, &map, &fog).expect("write succeeds");
        let mut loaded = Game::default();
        let mut loaded_fog = Fog::new();
        save_read(&path, &mut loaded, &resources, &map, &mut loaded_fog).expect("read succeeds");
        let _ = std::fs::remove_file(&path);

        assert!(loaded.seed_from_catalog);
        assert_eq!(loaded.seed_index, 7);
        assert_eq!(loaded.seed, crate::rng::seed_from_index(7));
    }

    #[test]
    fn a_load_does_not_carry_over_the_previous_games_state() {
        let resources = save_resources();
        let game = Game::default();
        let map = Map::new(8, 4);
        let fog = Fog::new();
        let path = temp_path("wipe");
        save_write(&path, &game, &resources, &map, &fog).expect("write succeeds");

        // Load over a game carrying unrelated state, which must be cleared.
        let mut loaded = populated_game();
        let mut loaded_fog = Fog::new();
        save_read(&path, &mut loaded, &resources, &map, &mut loaded_fog).expect("read succeeds");
        let _ = std::fs::remove_file(&path);

        assert_eq!(loaded.stats.gold, 0);
        assert_eq!(loaded.foe_count, 0);
        assert!(loaded.army[0].id.is_empty());
        assert!(loaded.character.name.is_empty());
    }

    #[test]
    fn a_header_summarises_a_save_without_loading_the_world() {
        let resources = save_resources();
        let game = populated_game();
        let map = Map::new(8, 4);
        let fog = Fog::new();
        let path = temp_path("header");
        save_write(&path, &game, &resources, &map, &fog).expect("write succeeds");

        let header = save_read_header(&path).expect("header reads");
        let _ = std::fs::remove_file(&path);

        assert!(header.exists);
        assert_eq!(header.name.as_str(), "Arthur");
        assert_eq!(header.class_id.as_str(), "knight");
        assert_eq!(header.rank_title.as_str(), "Captain");
        assert_eq!(header.zone.as_str(), "home");
        assert_eq!(header.days_left, 300);
        assert_eq!(header.gold, 4_321);
        assert_eq!(header.pack_id.as_str(), "openbounty");
        // The Rust engine never writes pack_hash.
        assert!(header.pack_hash.is_empty());
    }

    #[test]
    fn a_header_still_enforces_the_version_guard() {
        let path = temp_path("header-version");
        std::fs::write(&path, r#"{"version": 8}"#).expect("write");
        let result = save_read_header(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Err(SaveError::Version));
    }

    #[test]
    fn a_header_reads_a_save_from_another_pack_rather_than_refusing_it() {
        let path = temp_path("header-pack");
        std::fs::write(
            &path,
            format!(
                r#"{{"version": {SAVE_VERSION}, "pack_id": "someone_elses_pack",
                     "character": {{"name": "Stranger"}}}}"#
            ),
        )
        .expect("write");

        // The picker lists it; the refusal comes later, at load time.
        let header = save_read_header(&path).expect("header reads");
        let _ = std::fs::remove_file(&path);

        assert_eq!(header.pack_id.as_str(), "someone_elses_pack");
        assert_eq!(header.name.as_str(), "Stranger");
    }

    #[test]
    fn a_missing_or_malformed_save_has_no_header() {
        assert_eq!(
            save_read_header(&temp_path("header-missing")),
            Err(SaveError::Io)
        );

        let path = temp_path("header-garbage");
        std::fs::write(&path, "not json at all").expect("write");
        let result = save_read_header(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(result, Err(SaveError::Parse));
    }
}
