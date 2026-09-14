use crate::resources::{ResZone, Resources};
use crate::tile::{Interact, Terrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub interactive: Interact,
    pub blocks_foot: bool,
    pub is_bridge: bool,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            terrain: Terrain::Grass,
            interactive: Interact::None,
            blocks_foot: false,
            is_bridge: false,
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

    pub fn load_zone_bytes(
        resources: &Resources,
        zone: &ResZone,
        bytes: &[u8],
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

        stamp_objects(&mut map, resources, zone);
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

fn stamp_objects(map: &mut Map, resources: &Resources, zone: &ResZone) {
    for index in 0..resources.town_count() {
        let Some(town) = resources.town_by_index(index) else {
            continue;
        };
        if town.zone.as_str() != zone.id.as_str() {
            continue;
        }
        if let Some(tile) = map.tile_mut_i32(town.x, town.y) {
            tile.interactive = Interact::Town;
        }
    }

    for index in 0..resources.castle_count() {
        let Some(castle) = resources.castle_by_index(index) else {
            continue;
        };
        if castle.zone.as_str() != zone.id.as_str() {
            continue;
        }
        for (dx, dy, is_gate) in [
            (-1, -1, false),
            (0, -1, false),
            (1, -1, false),
            (-1, 0, false),
            (0, 0, true),
            (1, 0, false),
        ] {
            let Some(tile) = map.tile_mut_i32(castle.x + dx, castle.y + dy) else {
                continue;
            };
            if is_gate {
                tile.interactive = Interact::CastleGate;
                tile.blocks_foot = false;
            } else {
                tile.blocks_foot = true;
            }
        }
    }

    for chest in &zone.chests[..zone.chest_count] {
        if let Some(tile) = map.tile_mut_i32(chest.x, chest.y) {
            tile.interactive = Interact::TreasureChest;
        }
    }

    for dwelling in &zone.dwellings[..zone.dwelling_count] {
        if let Some(tile) = map.tile_mut_i32(dwelling.x, dwelling.y) {
            tile.interactive = Interact::from_dwelling_kind(dwelling.kind.as_str());
        }
    }

    if zone.magic_alcove_x >= 0 && zone.magic_alcove_y >= 0 {
        if let Some(tile) = map.tile_mut_i32(zone.magic_alcove_x, zone.magic_alcove_y) {
            tile.interactive = Interact::Alcove;
            tile.blocks_foot = false;
        }
    }
}

impl Map {
    fn tile_mut_i32(&mut self, x: i32, y: i32) -> Option<&mut Tile> {
        let x = usize::try_from(x).ok()?;
        let y = usize::try_from(y).ok()?;
        self.tile_mut(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::{Map, Tile};
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
                "width":4,
                "height":2,
                "chests":[{"x":0,"y":1,"id":"chest0"}],
                "dwellings":[{"x":1,"y":1,"id":"dw0","kind":"hills"}],
                "magic_alcove":{"x":2,"y":1}
              }],
              "towns": [{"index":0,"id":"town0","zone":"home","x":2,"y":0}],
              "castles": [{"index":0,"id":"castle0","zone":"home","x":3,"y":1}],
              "troops": []
            }"##,
        )
        .expect("fixture parses");
        let zone = resources.zone_by_id("home").expect("zone exists");

        let map = Map::load_zone_bytes(&resources, zone, b"# comment\n.F..\n....\n")
            .expect("map bytes load");

        assert_eq!(map.tile(1, 0).expect("in bounds").terrain, Terrain::Forest);
        assert_eq!(
            map.tile(2, 0).expect("in bounds").interactive,
            Interact::Town
        );
        assert_eq!(
            map.tile(0, 1).expect("in bounds").interactive,
            Interact::TreasureChest
        );
        assert_eq!(
            map.tile(1, 1).expect("in bounds").interactive,
            Interact::DwellingHills
        );
        assert_eq!(
            map.tile(2, 1).expect("in bounds").interactive,
            Interact::Alcove
        );
        assert_eq!(
            map.tile(3, 1).expect("in bounds").interactive,
            Interact::CastleGate
        );
        assert_eq!(map.eligible_scepter_tile_count(), 1);
        assert_eq!(map.nth_eligible_scepter_tile(0), Some((0, 0)));
    }
}
