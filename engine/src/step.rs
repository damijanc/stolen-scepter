use crate::game::Game;
use crate::map::Map;
use crate::tables::FixedStr;
use crate::tile::Terrain;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepResult {
    NoDirection,
    BlockedEdge,
    BlockedTerrain { terrain: Terrain },
    Moved { terrain: Terrain, move_cost: i32 },
}

impl StepResult {
    pub fn moved(self) -> bool {
        matches!(self, Self::Moved { .. })
    }
}

pub fn game_step_basic(game: &mut Game, map: &Map, dx: i32, dy: i32) -> StepResult {
    if dx == 0 && dy == 0 {
        return StepResult::NoDirection;
    }

    game.position.in_town = FixedStr::empty();

    let nx = game.position.x + dx;
    let ny = game.position.y + dy;
    let Some(tile) = map.tile_i32(nx, ny) else {
        return StepResult::BlockedEdge;
    };

    if !tile.terrain.walkable() {
        return StepResult::BlockedTerrain {
            terrain: tile.terrain,
        };
    }

    let prev_x = game.position.x;
    let prev_y = game.position.y;
    game.position.last_x = prev_x;
    game.position.last_y = prev_y;
    game.position.x = nx;
    game.position.y = ny;

    consume_step(game, tile.terrain);

    StepResult::Moved {
        terrain: tile.terrain,
        move_cost: tile.terrain.move_cost(),
    }
}

fn consume_step(game: &mut Game, terrain: Terrain) {
    if game.stats.game_over {
        return;
    }

    if game.stats.time_stop > 0 {
        game.stats.time_stop -= 1;
        return;
    }

    if terrain == Terrain::Desert {
        game.stats.steps_left_today = 0;
    } else if game.stats.steps_left_today > 0 {
        game.stats.steps_left_today -= 1;
    }
}

#[cfg(test)]
mod tests {
    use super::{game_step_basic, StepResult};
    use crate::game::Game;
    use crate::map::{Map, Tile};
    use crate::tile::{Interact, Terrain};

    #[test]
    fn step_basic_moves_updates_last_position_and_consumes_grass_step() {
        let map = Map::from_tiles(
            3,
            1,
            vec![
                Tile::plain_grass(),
                Tile::interactive(Interact::TreasureChest),
                Tile::terrain(Terrain::Desert),
            ],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.position.x = 0;
        game.position.y = 0;
        game.position.last_x = 9;
        game.position.last_y = 9;
        game.position.in_town.copy_from_str("town0");
        game.stats.steps_left_today = 5;

        let result = game_step_basic(&mut game, &map, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (0, 0));
        assert_eq!(game.position.in_town.as_str(), "");
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_desert_sets_steps_left_to_zero() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Desert)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.stats.steps_left_today = 33;

        let result = game_step_basic(&mut game, &map, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Desert,
                move_cost: 40
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 0);
    }

    #[test]
    fn step_basic_time_stop_absorbs_step_cost() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Desert)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.stats.steps_left_today = 33;
        game.stats.time_stop = 2;

        let result = game_step_basic(&mut game, &map, 1, 0);

        assert!(result.moved());
        assert_eq!(game.stats.time_stop, 1);
        assert_eq!(game.stats.steps_left_today, 33);
    }

    #[test]
    fn step_basic_blocks_edge_and_nonwalkable_terrain_without_moving() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Forest)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, 1, 0),
            StepResult::BlockedTerrain {
                terrain: Terrain::Forest
            }
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);

        assert_eq!(
            game_step_basic(&mut game, &map, -1, 0),
            StepResult::BlockedEdge
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_zero_delta_is_noop() {
        let map = Map::new(1, 1);
        let mut game = Game::default();
        game.position.in_town.copy_from_str("town0");
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, 0, 0),
            StepResult::NoDirection
        );
        assert_eq!(game.position.in_town.as_str(), "town0");
        assert_eq!(game.stats.steps_left_today, 5);
    }
}
