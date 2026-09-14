use crate::game::{Game, Mount, TravelMode};
use crate::map::Map;
use crate::resources::Resources;
use crate::tables::FixedStr;
use crate::tile::Terrain;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StepAdvance {
    pub day_ended: bool,
    pub week_ended: bool,
    pub commission_paid: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepResult {
    NoDirection,
    BlockedEdge,
    BlockedTerrain {
        terrain: Terrain,
    },
    Moved {
        terrain: Terrain,
        move_cost: i32,
        advance: StepAdvance,
    },
}

impl StepResult {
    pub fn moved(self) -> bool {
        matches!(self, Self::Moved { .. })
    }
}

pub fn game_step_basic(
    game: &mut Game,
    map: &Map,
    resources: &Resources,
    dx: i32,
    dy: i32,
) -> StepResult {
    if dx == 0 && dy == 0 {
        return StepResult::NoDirection;
    }

    game.position.in_town = FixedStr::empty();

    let nx = game.position.x + dx;
    let ny = game.position.y + dy;
    let Some(tile) = map.tile_i32(nx, ny) else {
        return StepResult::BlockedEdge;
    };

    let prev_x = game.position.x;
    let prev_y = game.position.y;
    let walking = game.travel_mode == TravelMode::Walk;
    let in_boat = game.travel_mode == TravelMode::Boat;
    let flying = game.character.mount == Mount::Fly;
    let will_board = !flying
        && walking
        && game.boat.has_boat
        && nx == game.boat.x
        && ny == game.boat.y
        && (game.boat.zone.is_empty() || game.boat.zone.as_str() == game.position.zone.as_str());

    let can_move = if flying {
        true
    } else if will_board {
        true
    } else if walking {
        walkable_on_foot(tile)
    } else {
        walkable_in_boat(tile)
    };

    if !can_move {
        return StepResult::BlockedTerrain {
            terrain: tile.terrain,
        };
    }

    game.position.last_x = prev_x;
    game.position.last_y = prev_y;
    game.position.x = nx;
    game.position.y = ny;

    if will_board {
        game.travel_mode = TravelMode::Boat;
    } else if in_boat && !flying {
        let still_water = tile.terrain == Terrain::Water || tile.is_bridge;
        if still_water {
            game.boat.x = nx;
            game.boat.y = ny;
        } else {
            game.travel_mode = TravelMode::Walk;
            game.boat.x = prev_x;
            game.boat.y = prev_y;
        }
    }

    let terrain_costs_full_day = !flying && tile.terrain == Terrain::Desert;
    let advance = consume_step(game, resources, terrain_costs_full_day);

    StepResult::Moved {
        terrain: tile.terrain,
        move_cost: step_move_cost(tile.terrain, flying),
        advance,
    }
}

fn walkable_on_foot(tile: &crate::map::Tile) -> bool {
    if tile.interactive != crate::tile::Interact::None {
        return true;
    }
    if tile.is_bridge {
        return true;
    }
    if tile.blocks_foot {
        return false;
    }
    tile.terrain.walkable()
}

fn walkable_in_boat(tile: &crate::map::Tile) -> bool {
    tile.is_bridge || tile.terrain == Terrain::Water || walkable_on_foot(tile)
}

fn step_move_cost(terrain: Terrain, flying: bool) -> i32 {
    if !flying && terrain == Terrain::Desert {
        40
    } else {
        1
    }
}

fn consume_step(
    game: &mut Game,
    resources: &Resources,
    terrain_costs_full_day: bool,
) -> StepAdvance {
    let mut advance = StepAdvance::default();
    if game.stats.game_over {
        return advance;
    }

    if game.stats.time_stop > 0 {
        game.stats.time_stop -= 1;
        return advance;
    }

    if terrain_costs_full_day {
        game.stats.steps_left_today = 0;
    } else if game.stats.steps_left_today > 0 {
        game.stats.steps_left_today -= 1;
    }

    while game.stats.steps_left_today <= 0 && !game.stats.game_over {
        end_day(game, resources, &mut advance);
    }

    advance
}

fn end_day(game: &mut Game, resources: &Resources, advance: &mut StepAdvance) {
    if game.stats.days_left > 0 {
        game.stats.days_left -= 1;
    }
    game.stats.steps_left_today = resources.time.day_steps;
    game.stats.time_stop = 0;
    advance.day_ended = true;

    if game.stats.days_left == 0 {
        game.stats.game_over = true;
        return;
    }

    let week_days = resources.time.week_days;
    if week_days > 0 && game.stats.days_left % week_days == 0 {
        advance.week_ended = true;
        advance.commission_paid = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::{game_step_basic, StepResult};
    use crate::game::{Game, Mount, TravelMode};
    use crate::map::{Map, Tile};
    use crate::resources::Resources;
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
        let resources = time_fixture();
        game.position.x = 0;
        game.position.y = 0;
        game.position.last_x = 9;
        game.position.last_y = 9;
        game.position.in_town.copy_from_str("town0");
        game.stats.steps_left_today = 5;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: Default::default()
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
        let resources = time_fixture();
        game.stats.steps_left_today = 33;
        game.stats.days_left = 10;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Desert,
                move_cost: 40,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                }
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 33);
        assert_eq!(game.stats.days_left, 9);
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
        let resources = time_fixture();
        game.stats.steps_left_today = 33;
        game.stats.time_stop = 2;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

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
        let resources = time_fixture();
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, &resources, 1, 0),
            StepResult::BlockedTerrain {
                terrain: Terrain::Forest
            }
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);

        assert_eq!(
            game_step_basic(&mut game, &map, &resources, -1, 0),
            StepResult::BlockedEdge
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_blocks_plain_water_when_walking_without_boat() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, &resources, 1, 0),
            StepResult::BlockedTerrain {
                terrain: Terrain::Water
            }
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_boards_owned_boat_on_target_tile() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.boat.has_boat = true;
        game.boat.zone.copy_from_str("home");
        game.boat.x = 1;
        game.boat.y = 0;
        game.stats.steps_left_today = 5;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Water,
                move_cost: 1,
                advance: Default::default()
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_does_not_board_boat_left_in_other_zone() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.boat.has_boat = true;
        game.boat.zone.copy_from_str("away");
        game.boat.x = 1;
        game.boat.y = 0;
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, &resources, 1, 0),
            StepResult::BlockedTerrain {
                terrain: Terrain::Water
            }
        );
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_boat_moves_on_water_and_bridge_then_disembarks_to_land() {
        let mut bridge = Tile::terrain(Terrain::Grass);
        bridge.is_bridge = true;
        let map = Map::from_tiles(
            4,
            1,
            vec![
                Tile::terrain(Terrain::Water),
                Tile::terrain(Terrain::Water),
                bridge,
                Tile::plain_grass(),
            ],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.position.x = 0;
        game.position.y = 0;
        game.travel_mode = TravelMode::Boat;
        game.boat.has_boat = true;
        game.boat.x = 0;
        game.boat.y = 0;
        game.stats.steps_left_today = 5;

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (2, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 3);

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (3, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!((game.boat.x, game.boat.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 2);
    }

    #[test]
    fn step_basic_flying_bypasses_terrain_and_blockers() {
        let map = Map::from_tiles(
            5,
            1,
            vec![
                Tile::plain_grass(),
                Tile::terrain(Terrain::Forest),
                Tile::terrain(Terrain::Mountain),
                Tile::terrain(Terrain::Water),
                Tile::blocked_grass(),
            ],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.character.mount = Mount::Fly;
        game.stats.steps_left_today = 5;

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 3);

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (3, 0));
        assert_eq!(game.stats.steps_left_today, 2);

        assert!(game_step_basic(&mut game, &map, &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (4, 0));
        assert_eq!(game.stats.steps_left_today, 1);
    }

    #[test]
    fn step_basic_flying_over_desert_costs_one_step_not_full_day() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Desert)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.character.mount = Mount::Fly;
        game.stats.steps_left_today = 33;
        game.stats.days_left = 10;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Desert,
                move_cost: 1,
                advance: Default::default()
            }
        );
        assert_eq!(game.stats.steps_left_today, 32);
        assert_eq!(game.stats.days_left, 10);
    }

    #[test]
    fn step_basic_flying_does_not_board_boat_on_target_tile() {
        let map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.character.mount = Mount::Fly;
        game.position.zone.copy_from_str("home");
        game.boat.has_boat = true;
        game.boat.zone.copy_from_str("home");
        game.boat.x = 1;
        game.boat.y = 0;
        game.stats.steps_left_today = 5;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Water,
                move_cost: 1,
                advance: Default::default()
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_zero_delta_is_noop() {
        let map = Map::new(1, 1);
        let mut game = Game::default();
        let resources = time_fixture();
        game.position.in_town.copy_from_str("town0");
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &map, &resources, 0, 0),
            StepResult::NoDirection
        );
        assert_eq!(game.position.in_town.as_str(), "town0");
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_grass_rolls_day_when_steps_reach_zero() {
        let map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 10;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                }
            }
        );
        assert_eq!(game.stats.steps_left_today, 33);
        assert_eq!(game.stats.days_left, 9);
        assert!(!game.stats.game_over);
    }

    #[test]
    fn step_basic_marks_week_boundary_without_economy_side_effects() {
        let map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 6;
        game.stats.gold = 100;
        game.stats.commission_weekly = 25;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: true,
                    commission_paid: 0
                }
            }
        );
        assert_eq!(game.stats.days_left, 5);
        assert_eq!(game.stats.gold, 100);
    }

    #[test]
    fn step_basic_sets_game_over_when_final_day_ends() {
        let map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 1;

        let result = game_step_basic(&mut game, &map, &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                }
            }
        );
        assert_eq!(game.stats.days_left, 0);
        assert_eq!(game.stats.steps_left_today, 33);
        assert!(game.stats.game_over);
    }

    fn time_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "time": {"day_steps": 33, "week_days": 5},
              "troops": []
            }"#,
        )
        .expect("fixture parses")
    }
}
