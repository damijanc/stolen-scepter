use crate::tile::{Interact, Terrain};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tile {
    pub terrain: Terrain,
    pub interactive: Interact,
    pub blocks_foot: bool,
}

impl Default for Tile {
    fn default() -> Self {
        Self {
            terrain: Terrain::Grass,
            interactive: Interact::None,
            blocks_foot: false,
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

#[cfg(test)]
mod tests {
    use super::{Map, Tile};
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
}
