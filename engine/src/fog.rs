use crate::map::Map;

pub const MAP_MAX_W: usize = 64;
pub const MAP_MAX_H: usize = 64;

/// Per-tile visibility for one map. One `Fog` is kept per continent so a
/// revisited zone retains the terrain it has already revealed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Fog {
    pub seen: [[bool; MAP_MAX_W]; MAP_MAX_H],
}

impl Default for Fog {
    fn default() -> Self {
        Self {
            seen: [[false; MAP_MAX_W]; MAP_MAX_H],
        }
    }
}

impl Fog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reveal the tiles around (cx, cy).
    ///
    /// `radius` is accepted for call-shape parity and then IGNORED: the
    /// original's `clear_fog` always reveals a 5x5 square, -2..=+2 on both
    /// axes, whatever sight value it is handed. Clamped to the map's bounds,
    /// which may be smaller than the fixed array.
    pub fn reveal(&mut self, map: &Map, cx: i32, cy: i32, radius: i32) {
        let _ = radius;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let x = cx + dx;
                let y = cy + dy;
                if x < 0 || y < 0 || x as usize >= map.width || y as usize >= map.height {
                    continue;
                }
                if (x as usize) < MAP_MAX_W && (y as usize) < MAP_MAX_H {
                    self.seen[y as usize][x as usize] = true;
                }
            }
        }
    }

    pub fn seen(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 || x as usize >= MAP_MAX_W || y as usize >= MAP_MAX_H {
            return false;
        }
        self.seen[y as usize][x as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_fog_has_seen_nothing() {
        let fog = Fog::new();
        assert!(!fog.seen(0, 0));
        assert!(!fog.seen(10, 10));
    }

    #[test]
    fn reveal_stamps_a_five_by_five_square() {
        let map = Map::new(20, 20);
        let mut fog = Fog::new();

        fog.reveal(&map, 10, 10, 3);

        for y in 8..=12 {
            for x in 8..=12 {
                assert!(fog.seen(x, y), "({x},{y}) should be revealed");
            }
        }
        assert!(!fog.seen(7, 10), "one column short of the square");
        assert!(!fog.seen(13, 10));
        assert!(!fog.seen(10, 7));
        assert!(!fog.seen(10, 13));
    }

    #[test]
    fn the_radius_argument_is_ignored() {
        let map = Map::new(20, 20);
        let mut wide = Fog::new();
        let mut narrow = Fog::new();

        // The original's clear_fog reveals 5x5 whatever sight it is handed,
        // so a radius of 0 and a radius of 9 must agree exactly.
        wide.reveal(&map, 10, 10, 9);
        narrow.reveal(&map, 10, 10, 0);

        assert_eq!(wide, narrow);
        assert!(wide.seen(12, 12));
        assert!(!wide.seen(13, 13));
    }

    #[test]
    fn reveal_clamps_to_the_map_not_the_array() {
        // A map smaller than the fixed 64x64 fog array must not mark tiles
        // outside its own bounds.
        let map = Map::new(4, 4);
        let mut fog = Fog::new();

        fog.reveal(&map, 0, 0, 3);

        assert!(fog.seen(0, 0));
        assert!(fog.seen(2, 2));
        assert!(!fog.seen(4, 0), "past the map's width");
        assert!(!fog.seen(0, 4), "past the map's height");
    }

    #[test]
    fn reveal_near_a_corner_keeps_the_in_bounds_half() {
        let map = Map::new(20, 20);
        let mut fog = Fog::new();

        fog.reveal(&map, 0, 0, 3);

        assert!(fog.seen(0, 0));
        assert!(fog.seen(2, 2));
        assert!(!fog.seen(3, 0));
    }

    #[test]
    fn seen_rejects_coordinates_outside_the_array() {
        let map = Map::new(20, 20);
        let mut fog = Fog::new();
        fog.reveal(&map, 10, 10, 3);

        assert!(!fog.seen(-1, 10));
        assert!(!fog.seen(10, -1));
        assert!(!fog.seen(MAP_MAX_W as i32, 10));
        assert!(!fog.seen(10, MAP_MAX_H as i32));
    }

    #[test]
    fn revealing_twice_accumulates_rather_than_replacing() {
        let map = Map::new(20, 20);
        let mut fog = Fog::new();

        fog.reveal(&map, 2, 2, 3);
        fog.reveal(&map, 10, 10, 3);

        assert!(fog.seen(2, 2), "the first square is retained");
        assert!(fog.seen(10, 10));
    }
}
