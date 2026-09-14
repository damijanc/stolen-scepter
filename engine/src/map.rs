use crate::game::Game;
use crate::resources::{ResZone, Resources};
use crate::tables::FixedStr;
use crate::tile::{Interact, Terrain};

pub const TILE_ID_LEN: usize = 24;
pub const TILE_SIGN_BODY_LEN: usize = 96;
pub const TILE_SIGN_TITLE_LEN: usize = 48;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub interactive: Interact,
    pub id: FixedStr<TILE_ID_LEN>,
    pub blocks_foot: bool,
    pub is_bridge: bool,
    pub sign_title: FixedStr<TILE_SIGN_TITLE_LEN>,
    pub sign_body: FixedStr<TILE_SIGN_BODY_LEN>,
    pub boat_spawn_x: i32,
    pub boat_spawn_y: i32,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            terrain: Terrain::Grass,
            interactive: Interact::None,
            id: FixedStr::empty(),
            blocks_foot: false,
            is_bridge: false,
            sign_title: FixedStr::empty(),
            sign_body: FixedStr::empty(),
            boat_spawn_x: -1,
            boat_spawn_y: -1,
        }
    }
}

impl Tile {
    pub fn plain_grass() -> Self {
        Self::default()
    }

    pub fn terrain(terrain: Terrain) -> Self {
        Self {
            terrain,
            ..Self::default()
        }
    }

    pub fn interactive(interactive: Interact) -> Self {
        Self {
            interactive,
            ..Self::default()
        }
    }

    pub fn blocked_grass() -> Self {
        Self {
            blocks_foot: true,
            ..Self::default()
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Map {
    pub width: usize,
    pub height: usize,
    tiles: Vec<Tile>,
}

impl Map {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            tiles: vec![Tile::default(); width.saturating_mul(height)],
        }
    }

    pub fn from_tiles(width: usize, height: usize, tiles: Vec<Tile>) -> Option<Self> {
        if width.checked_mul(height)? != tiles.len() {
            return None;
        }
        Some(Self {
            width,
            height,
            tiles,
        })
    }

    pub fn load_zone(resources: &Resources, zone_id: &str) -> Result<Self, MapLoadError> {
        let zone = resources
            .zone_by_id(zone_id)
            .ok_or_else(|| MapLoadError::UnknownZone(zone_id.to_owned()))?;
        let path = resources
            .resolve_map_path(zone)
            .ok_or_else(|| MapLoadError::MissingMapPath(zone_id.to_owned()))?;
        let bytes = std::fs::read(&path).map_err(MapLoadError::Io)?;
        Self::load_zone_bytes(resources, zone, &bytes)
    }

    pub fn load_zone_with_placements(
        resources: &Resources,
        zone_id: &str,
        game: &Game,
    ) -> Result<Self, MapLoadError> {
        let zone = resources
            .zone_by_id(zone_id)
            .ok_or_else(|| MapLoadError::UnknownZone(zone_id.to_owned()))?;
        let path = resources
            .resolve_map_path(zone)
            .ok_or_else(|| MapLoadError::MissingMapPath(zone_id.to_owned()))?;
        let bytes = std::fs::read(&path).map_err(MapLoadError::Io)?;
        Self::load_zone_bytes_with_placements(resources, zone, &bytes, game)
    }

    pub fn load_zone_bytes(
        resources: &Resources,
        zone: &ResZone,
        bytes: &[u8],
    ) -> Result<Self, MapLoadError> {
        Self::load_zone_bytes_inner(resources, zone, bytes, None)
    }

    pub fn load_zone_bytes_with_placements(
        resources: &Resources,
        zone: &ResZone,
        bytes: &[u8],
        game: &Game,
    ) -> Result<Self, MapLoadError> {
        Self::load_zone_bytes_inner(resources, zone, bytes, Some(game))
    }

    fn load_zone_bytes_inner(
        resources: &Resources,
        zone: &ResZone,
        bytes: &[u8],
        game: Option<&Game>,
    ) -> Result<Self, MapLoadError> {
        let width = usize::try_from(zone.width).map_err(|_| MapLoadError::InvalidDimensions {
            zone_id: zone.id.as_str().to_owned(),
            width: zone.width,
            height: zone.height,
        })?;
        let height = usize::try_from(zone.height).map_err(|_| MapLoadError::InvalidDimensions {
            zone_id: zone.id.as_str().to_owned(),
            width: zone.width,
            height: zone.height,
        })?;
        if width == 0 || height == 0 {
            return Err(MapLoadError::InvalidDimensions {
                zone_id: zone.id.as_str().to_owned(),
                width: zone.width,
                height: zone.height,
            });
        }

        let mut map = Self::new(width, height);
        let mut p = skip_blank_and_comments(bytes, 0);
        for y in 0..height {
            if p >= bytes.len() {
                break;
            }

            let mut x = 0;
            while p < bytes.len() && bytes[p] != b'\n' && bytes[p] != b'\r' && x < width {
                let code = bytes[p];
                p += 1;
                let tile_code = resources
                    .tile_code(code)
                    .ok_or(MapLoadError::UnknownTileCode { x, y, code })?;
                let tile = map.tile_mut(x, y).expect("loader writes in bounds");
                tile.terrain = tile_code.terrain;
                tile.blocks_foot = tile_code.blocks_foot;
                tile.is_bridge = tile_code.is_bridge;
                tile.interactive = Interact::None;
                x += 1;
            }

            while p < bytes.len() && bytes[p] != b'\n' {
                p += 1;
            }
            if p < bytes.len() && bytes[p] == b'\n' {
                p += 1;
            }
        }

        stamp_objects(&mut map, resources, zone, game);
        if let Some(game) = game {
            stamp_placements(&mut map, game, zone.id.as_str());
            map.apply_consumed(game, zone.id.as_str());
        }
        Ok(map)
    }

    pub fn tile(&self, x: usize, y: usize) -> Option<&Tile> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.tiles.get(y * self.width + x)
    }

    pub fn tile_mut(&mut self, x: usize, y: usize) -> Option<&mut Tile> {
        if x >= self.width || y >= self.height {
            return None;
        }
        self.tiles.get_mut(y * self.width + x)
    }

    pub fn eligible_scepter_tile_count(&self) -> usize {
        self.tiles
            .iter()
            .filter(|tile| is_scepter_tile(tile))
            .count()
    }

    pub fn nth_eligible_scepter_tile(&self, target: usize) -> Option<(usize, usize)> {
        let mut count = 0;
        for y in 0..self.height {
            for x in 0..self.width {
                let Some(tile) = self.tile(x, y) else {
                    continue;
                };
                if !is_scepter_tile(tile) {
                    continue;
                }
                if count == target {
                    return Some((x, y));
                }
                count += 1;
            }
        }
        None
    }
}

pub fn is_scepter_tile(tile: &Tile) -> bool {
    tile.terrain == Terrain::Grass && tile.interactive == Interact::None && !tile.blocks_foot
}

#[derive(Debug)]
pub enum MapLoadError {
    UnknownZone(String),
    MissingMapPath(String),
    InvalidDimensions {
        zone_id: String,
        width: i32,
        height: i32,
    },
    UnknownTileCode {
        x: usize,
        y: usize,
        code: u8,
    },
    Io(std::io::Error),
}

impl std::fmt::Display for MapLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownZone(zone) => write!(f, "unknown zone `{zone}`"),
            Self::MissingMapPath(zone) => write!(f, "zone `{zone}` has no map path"),
            Self::InvalidDimensions {
                zone_id,
                width,
                height,
            } => write!(
                f,
                "zone `{zone_id}` has invalid dimensions {width}x{height}"
            ),
            Self::UnknownTileCode { x, y, code } => {
                write!(f, "unknown tile code 0x{code:02x} at {x},{y}")
            }
            Self::Io(err) => write!(f, "could not load map: {err}"),
        }
    }
}

impl std::error::Error for MapLoadError {}

fn skip_blank_and_comments(bytes: &[u8], mut p: usize) -> usize {
    while p < bytes.len() {
        if bytes[p] == b'\n' || bytes[p] == b'\r' {
            p += 1;
            continue;
        }
        if bytes[p] == b'#' {
            while p < bytes.len() && bytes[p] != b'\n' {
                p += 1;
            }
            if p < bytes.len() {
                p += 1;
            }
            continue;
        }
        break;
    }
    p
}

fn stamp_objects(map: &mut Map, resources: &Resources, zone: &ResZone, game: Option<&Game>) {
    for sign in &zone.signs[..zone.sign_count] {
        if let Some(tile) = map.tile_mut_i32(sign.x, sign.y) {
            tile.interactive = Interact::Sign;
            tile.id.copy_from_str(sign.id.as_str());
            tile.sign_title.copy_from_str(sign.title.as_str());
            tile.sign_body.copy_from_str(sign.body.as_str());
        }
    }

    for index in 0..resources.town_count() {
        let Some(town) = resources.town_by_index(index) else {
            continue;
        };
        if town.zone.as_str() != zone.id.as_str() {
            continue;
        }
        if let Some(tile) = map.tile_mut_i32(town.x, town.y) {
            tile.interactive = Interact::Town;
            tile.id.copy_from_str(town.id.as_str());
            tile.boat_spawn_x = town.boat_x;
            tile.boat_spawn_y = town.boat_y;
        }
    }

    if zone.castle_count > 0 {
        for castle in &zone.castles[..zone.castle_count] {
            stamp_castle(map, castle.x, castle.y, castle.id.as_str());
            for decoration in &castle.decorations[..castle.decor_count] {
                if decoration.art.is_empty() {
                    continue;
                }
                if let Some(tile) =
                    map.tile_mut_i32(castle.x + decoration.dx, castle.y + decoration.dy)
                {
                    tile.blocks_foot = true;
                }
            }
        }
    } else {
        for index in 0..resources.castle_count() {
            let Some(castle) = resources.castle_by_index(index) else {
                continue;
            };
            if castle.zone.as_str() != zone.id.as_str() {
                continue;
            }
            stamp_castle(map, castle.x, castle.y, castle.id.as_str());
        }
    }

    for chest in &zone.chests[..zone.chest_count] {
        if game
            .map(|game| game_friendly_foe_origin_at(game, zone.id.as_str(), chest.x, chest.y))
            .unwrap_or(false)
        {
            continue;
        }
        if let Some(tile) = map.tile_mut_i32(chest.x, chest.y) {
            tile.interactive = Interact::TreasureChest;
            tile.id.copy_from_str(chest.id.as_str());
        }
    }

    for artifact in &zone.artifacts[..zone.artifact_count] {
        if let Some(tile) = map.tile_mut_i32(artifact.x, artifact.y) {
            tile.interactive = Interact::Artifact;
            tile.id.copy_from_str(artifact.id.as_str());
        }
    }

    for dwelling in &zone.dwellings[..zone.dwelling_count] {
        if let Some(tile) = map.tile_mut_i32(dwelling.x, dwelling.y) {
            tile.interactive = Interact::from_dwelling_kind(dwelling.kind.as_str());
            tile.id.copy_from_str(dwelling.id.as_str());
        }
    }

    if zone.magic_alcove_x >= 0 && zone.magic_alcove_y >= 0 {
        if let Some(tile) = map.tile_mut_i32(zone.magic_alcove_x, zone.magic_alcove_y) {
            tile.interactive = Interact::Alcove;
            tile.id.copy_from_str("alcove");
            tile.blocks_foot = false;
        }
    }
}

fn stamp_placements(map: &mut Map, game: &Game, zone_id: &str) {
    for placement in &game.placements[..game.placement_count] {
        if placement.zone.as_str() != zone_id {
            continue;
        }
        if let Some(tile) = map.tile_mut_i32(placement.x, placement.y) {
            tile.interactive = placement.kind;
            tile.id.copy_from_str(placement.id.as_str());
        }
    }

    for foe in &game.foes[..game.foe_count] {
        if !foe.alive || foe.zone.as_str() != zone_id {
            continue;
        }
        map.stamp_foe(foe.x, foe.y, foe.placement_id.as_str());
    }
}

fn game_friendly_foe_origin_at(game: &Game, zone: &str, x: i32, y: i32) -> bool {
    game.foes[..game.foe_count].iter().any(|foe| {
        foe.friendly && foe.zone.as_str() == zone && foe.origin_x == x && foe.origin_y == y
    })
}

fn stamp_castle(map: &mut Map, x: i32, y: i32, id: &str) {
    for (dx, dy, is_gate) in [
        (-1, -1, false),
        (0, -1, false),
        (1, -1, false),
        (-1, 0, false),
        (0, 0, true),
        (1, 0, false),
    ] {
        let Some(tile) = map.tile_mut_i32(x + dx, y + dy) else {
            continue;
        };
        if is_gate {
            tile.interactive = Interact::CastleGate;
            tile.id.copy_from_str(id);
            tile.blocks_foot = false;
        } else {
            tile.blocks_foot = true;
        }
    }
}

impl Map {
    pub fn clear_interactive(&mut self, x: i32, y: i32) {
        let Some(tile) = self.tile_mut_i32(x, y) else {
            return;
        };
        tile.interactive = Interact::None;
        tile.id = FixedStr::empty();
        tile.sign_title = FixedStr::empty();
        tile.sign_body = FixedStr::empty();
        tile.boat_spawn_x = -1;
        tile.boat_spawn_y = -1;
        if tile.terrain != Terrain::Water {
            tile.terrain = Terrain::Grass;
            tile.blocks_foot = false;
            tile.is_bridge = false;
        }
    }

    pub fn apply_consumed(&mut self, game: &Game, zone_id: &str) {
        for mutation in &game.consumed[..game.consumed_count] {
            if mutation.zone.as_str() != zone_id {
                continue;
            }
            self.clear_interactive(mutation.x, mutation.y);
        }
    }

    pub fn stamp_foe(&mut self, x: i32, y: i32, placement_id: &str) {
        let Some(tile) = self.tile_mut_i32(x, y) else {
            return;
        };
        let spurious_chest = tile.interactive == Interact::TreasureChest;
        if tile.interactive != Interact::None
            && tile.interactive != Interact::Foe
            && !spurious_chest
        {
            return;
        }
        tile.interactive = Interact::Foe;
        tile.id.copy_from_str(placement_id);
    }

    fn tile_mut_i32(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        self.tile_mut(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::{Map, Tile};
    use crate::game::Game;
    use crate::resources::Resources;
    use crate::tile::{Interact, Terrain};

    #[test]
    fn scepter_candidates_are_plain_unblocked_grass_in_row_major_order() {
        let map = Map::from_tiles(
            4,
            2,
            vec![
                Tile::plain_grass(),
                Tile::terrain(Terrain::Forest),
                Tile::blocked_grass(),
                Tile::plain_grass(),
                Tile::interactive(Interact::TreasureChest),
                Tile::terrain(Terrain::Desert),
                Tile::plain_grass(),
                Tile::terrain(Terrain::Water),
            ],
        )
        .expect("fixture dimensions match");

        assert_eq!(map.eligible_scepter_tile_count(), 3);
        assert_eq!(map.nth_eligible_scepter_tile(0), Some((0, 0)));
        assert_eq!(map.nth_eligible_scepter_tile(1), Some((3, 0)));
        assert_eq!(map.nth_eligible_scepter_tile(2), Some((2, 1)));
        assert_eq!(map.nth_eligible_scepter_tile(3), None);
    }

    #[test]
    fn load_zone_bytes_resolves_tile_codes_and_stamps_existing_objects() {
        let resources = Resources::from_json_str(
            r##"{
              "tile_codes": {
                ".": {"art":"grass","terrain":"grass"},
                "F": {"art":"forest","terrain":"forest"},
                "W": {"art":"water","terrain":"water"},
                "B": {"art":"wall","terrain":"grass","blocks_foot":true}
              },
              "zones": [{
                "id":"home",
                "map":"maps/home.dat",
                "width":8,
                "height":3,
                "signs":[{"x":0,"y":1,"id":"sign0","title":"Treasure","body":"Dig here"}],
                "castles":[{"x":6,"y":2,"id":"castle0","decorations":[{"dx":-2,"dy":0,"art":"tower"}]}],
                "chests":[{"x":1,"y":1,"id":"chest0"}],
                "artifacts":[{"x":3,"y":1,"id":"artifact0"}],
                "dwellings":[{"x":2,"y":1,"id":"dw0","kind":"hills"}],
                "magic_alcove":{"x":4,"y":1}
              }],
              "towns": [{"index":0,"id":"town0","zone":"home","x":5,"y":0,"boat":{"x":6,"y":0}}],
              "castles": [{"index":0,"id":"fallback_castle","zone":"home","x":3,"y":1}],
              "troops": []
            }"##,
        )
        .expect("fixture parses");
        let zone = resources.zone_by_id("home").expect("zone exists");

        let map = Map::load_zone_bytes(
            &resources,
            zone,
            b"# comment\n.F......\n........\n........\n",
        )
        .expect("map bytes load");

        assert_eq!(map.tile(1, 0).expect("in bounds").terrain, Terrain::Forest);
        assert_eq!(
            map.tile(5, 0).expect("in bounds").interactive,
            Interact::Town
        );
        assert_eq!(map.tile(5, 0).expect("in bounds").id.as_str(), "town0");
        assert_eq!(
            (
                map.tile(5, 0).expect("in bounds").boat_spawn_x,
                map.tile(5, 0).expect("in bounds").boat_spawn_y
            ),
            (6, 0)
        );
        assert_eq!(
            map.tile(0, 1).expect("in bounds").interactive,
            Interact::Sign
        );
        assert_eq!(map.tile(0, 1).expect("in bounds").id.as_str(), "sign0");
        assert_eq!(
            map.tile(0, 1).expect("in bounds").sign_title.as_str(),
            "Treasure"
        );
        assert_eq!(
            map.tile(0, 1).expect("in bounds").sign_body.as_str(),
            "Dig here"
        );
        assert_eq!(
            map.tile(1, 1).expect("in bounds").interactive,
            Interact::TreasureChest
        );
        assert_eq!(map.tile(1, 1).expect("in bounds").id.as_str(), "chest0");
        assert_eq!(
            map.tile(2, 1).expect("in bounds").interactive,
            Interact::DwellingHills
        );
        assert_eq!(map.tile(2, 1).expect("in bounds").id.as_str(), "dw0");
        assert_eq!(
            map.tile(3, 1).expect("in bounds").interactive,
            Interact::Artifact
        );
        assert_eq!(map.tile(3, 1).expect("in bounds").id.as_str(), "artifact0");
        assert_eq!(
            map.tile(4, 1).expect("in bounds").interactive,
            Interact::Alcove
        );
        assert_eq!(map.tile(4, 1).expect("in bounds").id.as_str(), "alcove");
        assert_eq!(
            map.tile(6, 2).expect("in bounds").interactive,
            Interact::CastleGate
        );
        assert_eq!(map.tile(6, 2).expect("in bounds").id.as_str(), "castle0");
        assert!(map.tile(4, 2).expect("in bounds").blocks_foot);
        assert_eq!(map.eligible_scepter_tile_count(), 10);
        assert_eq!(map.nth_eligible_scepter_tile(0), Some((0, 0)));
    }

    #[test]
    fn load_zone_bytes_with_placements_replays_salted_state_and_live_foes() {
        let resources = Resources::from_json_str(
            r##"{
              "tile_codes": {
                ".": {"art":"grass","terrain":"grass"},
                "W": {"art":"water","terrain":"water"}
              },
              "zones": [{
                "id":"home",
                "map":"maps/home.dat",
                "width":6,
                "height":2,
                "chests":[{"x":0,"y":0,"id":"spent_chest"}]
              }],
              "towns": [{"index":0,"id":"town0","zone":"home","x":1,"y":0}],
              "troops": []
            }"##,
        )
        .expect("fixture parses");
        let zone = resources.zone_by_id("home").expect("zone exists");
        let mut game = Game::default();
        game.add_placement("home", 2, 0, Interact::Artifact, "sword");
        game.add_placement("home", 3, 0, Interact::Telecave, "telecave0");
        game.add_placement("home", 5, 1, Interact::Orb, "orb0");
        game.add_consumed("home", 2, 0);
        game.add_consumed("home", 3, 0);
        game.add_consumed("home", 5, 1);
        game.add_consumed("away", 4, 0);

        game.foes[0].zone.copy_from_str("home");
        game.foes[0].x = 0;
        game.foes[0].y = 0;
        game.foes[0].origin_x = 0;
        game.foes[0].origin_y = 0;
        game.foes[0].placement_id.copy_from_str("friendly0");
        game.foes[0].friendly = true;
        game.foes[0].alive = false;

        game.foes[1].zone.copy_from_str("home");
        game.foes[1].x = 4;
        game.foes[1].y = 0;
        game.foes[1].origin_x = 4;
        game.foes[1].origin_y = 0;
        game.foes[1].placement_id.copy_from_str("hostile0");
        game.foes[1].alive = true;

        game.foes[2].zone.copy_from_str("home");
        game.foes[2].x = 1;
        game.foes[2].y = 0;
        game.foes[2].origin_x = 1;
        game.foes[2].origin_y = 0;
        game.foes[2].placement_id.copy_from_str("blocked_by_town");
        game.foes[2].alive = true;
        game.foe_count = 3;

        let map =
            Map::load_zone_bytes_with_placements(&resources, zone, b"......\n.....W\n", &game)
                .expect("map bytes load");

        assert_eq!(
            map.tile(0, 0).expect("in bounds").interactive,
            Interact::None
        );
        assert_eq!(
            map.tile(1, 0).expect("in bounds").interactive,
            Interact::Town
        );
        assert_eq!(map.tile(1, 0).expect("in bounds").id.as_str(), "town0");
        assert_eq!(
            map.tile(2, 0).expect("in bounds").interactive,
            Interact::None
        );
        assert_eq!(map.tile(2, 0).expect("in bounds").id.as_str(), "");
        assert_eq!(map.tile(2, 0).expect("in bounds").terrain, Terrain::Grass);
        assert_eq!(
            map.tile(3, 0).expect("in bounds").interactive,
            Interact::None
        );
        assert_eq!(map.tile(3, 0).expect("in bounds").id.as_str(), "");
        assert_eq!(map.tile(3, 0).expect("in bounds").terrain, Terrain::Grass);
        assert_eq!(
            map.tile(4, 0).expect("in bounds").interactive,
            Interact::Foe
        );
        assert_eq!(map.tile(4, 0).expect("in bounds").id.as_str(), "hostile0");
        assert_eq!(
            map.tile(5, 1).expect("in bounds").interactive,
            Interact::None
        );
        assert_eq!(map.tile(5, 1).expect("in bounds").id.as_str(), "");
        assert_eq!(map.tile(5, 1).expect("in bounds").terrain, Terrain::Water);
    }
}
