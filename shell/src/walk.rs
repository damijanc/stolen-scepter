//! Overworld click-to-move. A DIVERGENCE, deliberate: C moves one tile per
//! key and has no pointer. See `specs/overworld-click-to-move-design.md`.
//!
//! The route only chooses which direction keys to press. Every tile of it is
//! one ordinary `game_step_basic` call, so interactions, foe following, fog,
//! days and week ends fire exactly as they do for keys. Walkability comes
//! from the engine's `step_preview`; nothing here re-derives a terrain rule.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use openbounty_engine::fog::Fog;
use openbounty_engine::game::Game;
use openbounty_engine::map::Map;
use openbounty_engine::step::{step_preview, StepInteraction, StepPreview, StepResult};
use openbounty_engine::tile::Interact;

/// The eight keypad directions.
const DIRS: [(i32, i32); 8] = [
    (0, -1),
    (0, 1),
    (-1, 0),
    (1, 0),
    (-1, -1),
    (1, -1),
    (-1, 1),
    (1, 1),
];

/// The cost of stepping onto `(x, y)` from next to it, or `None` when the
/// tile may not be on this route. Mid-route tiles must be plain; the goal may
/// interact or change the travel mode, but may not hold a foe (GAP 3).
fn step_cost(game: &Game, map: &Map, fog: &Fog, x: i32, y: i32, is_goal: bool) -> Option<i32> {
    if !fog.seen(x, y) {
        return None;
    }
    let StepPreview::Open {
        cost,
        interacts,
        changes_mode,
    } = step_preview(game, map, x, y)
    else {
        return None;
    };
    if is_goal {
        let foe = map
            .tile_i32(x, y)
            .is_some_and(|t| t.interactive == Interact::Foe);
        return (!foe).then_some(cost);
    }
    (!interacts && !changes_mode).then_some(cost)
}

/// A* from the hero to `goal`, fewest days first. The heuristic is the
/// keypad (Chebyshev) distance, admissible because every step costs >= 1.
/// Returns each step's direction, or `None` when there is no route (or the
/// goal is the hero's own tile).
pub fn plan(game: &Game, map: &Map, fog: &Fog, goal: (i32, i32)) -> Option<Vec<(i32, i32)>> {
    let start = (game.position.x, game.position.y);
    if start == goal || map.tile_i32(goal.0, goal.1).is_none() {
        return None;
    }
    let (w, h) = (map.width as i32, map.height as i32);
    let index = |(x, y): (i32, i32)| (y * w + x) as usize;
    let mut best = vec![i32::MAX; (w * h) as usize];
    let mut came_from: Vec<Option<(i32, i32)>> = vec![None; (w * h) as usize];
    let heuristic = |(x, y): (i32, i32)| (x - goal.0).abs().max((y - goal.1).abs());
    let mut open = BinaryHeap::new();
    best[index(start)] = 0;
    open.push(Reverse((heuristic(start), 0, start)));
    while let Some(Reverse((_, cost, pos))) = open.pop() {
        if pos == goal {
            let mut dirs = Vec::new();
            let mut at = goal;
            while let Some(prev) = came_from[index(at)] {
                dirs.push((at.0 - prev.0, at.1 - prev.1));
                at = prev;
            }
            dirs.reverse();
            return Some(dirs);
        }
        if cost > best[index(pos)] {
            continue;
        }
        for (dx, dy) in DIRS {
            let next = (pos.0 + dx, pos.1 + dy);
            if next.0 < 0 || next.1 < 0 || next.0 >= w || next.1 >= h {
                continue;
            }
            let Some(step) = step_cost(game, map, fog, next.0, next.1, next == goal) else {
                continue;
            };
            let total = cost + step;
            if total < best[index(next)] {
                best[index(next)] = total;
                came_from[index(next)] = Some(pos);
                open.push(Reverse((total + heuristic(next), total, next)));
            }
        }
    }
    None
}

/// How long each walked step holds before the next, in seconds.
pub const WALK_STEP_SECONDS: f64 = 0.12;

/// What a click on map tile `tile` asks for.
pub enum ClickMove {
    /// An adjacent tile: exactly that direction key, foes included.
    Step(i32, i32),
    /// A route to walk.
    Route(Walk),
    /// The hero's own tile, or no route.
    Nothing,
}

pub fn click(game: &Game, map: &Map, fog: &Fog, tile: (i32, i32), now: f64) -> ClickMove {
    let (dx, dy) = (tile.0 - game.position.x, tile.1 - game.position.y);
    if (dx, dy) == (0, 0) {
        return ClickMove::Nothing;
    }
    if dx.abs() <= 1 && dy.abs() <= 1 {
        return ClickMove::Step(dx, dy);
    }
    match plan(game, map, fog, tile) {
        Some(dirs) => ClickMove::Route(Walk {
            dirs,
            taken: 0,
            next_at: now,
        }),
        None => ClickMove::Nothing,
    }
}

/// A route being walked.
pub struct Walk {
    dirs: Vec<(i32, i32)>,
    taken: usize,
    next_at: f64,
}

pub enum WalkTick {
    /// Not time for the next step yet.
    Wait,
    /// Take this step now, through the ordinary move path.
    Step(i32, i32),
    /// Arrived, or the next tile no longer matches the plan.
    Done,
}

impl Walk {
    /// The next step, re-checked against the world as it is NOW: a tile that
    /// stopped being usable since planning (a foe stepped onto it) ends the
    /// walk rather than acting on a changed world.
    pub fn next(&mut self, game: &Game, map: &Map, fog: &Fog, now: f64) -> WalkTick {
        let Some(&(dx, dy)) = self.dirs.get(self.taken) else {
            return WalkTick::Done;
        };
        if now < self.next_at {
            return WalkTick::Wait;
        }
        let (x, y) = (game.position.x + dx, game.position.y + dy);
        let is_goal = self.taken + 1 == self.dirs.len();
        if step_cost(game, map, fog, x, y, is_goal).is_none() {
            return WalkTick::Done;
        }
        self.taken += 1;
        self.next_at = now + WALK_STEP_SECONDS;
        WalkTick::Step(dx, dy)
    }
}

/// Whether a walked step's result ends the walk: anything but a plain move.
/// The collision stop reads the result directly because the shell shows
/// nothing for a collision yet (engine-sweep GAP 4).
pub fn stops_walk(result: &StepResult) -> bool {
    match result {
        StepResult::Moved {
            advance,
            interaction,
            collision,
            ..
        } => *interaction != StepInteraction::None || collision.is_some() || advance.week_ended,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openbounty_engine::map::Tile;
    use openbounty_engine::tile::{Interact, Terrain};

    /// A map from rows of chars: `.` grass, `d` desert, `~` water, `^`
    /// mountain, `c` chest, `f` foe, `t` town gate. All of it seen.
    fn world(rows: &[&str]) -> (Map, Fog) {
        let h = rows.len();
        let w = rows[0].len();
        let mut tiles = Vec::new();
        for row in rows {
            for ch in row.chars() {
                tiles.push(match ch {
                    'd' => Tile::terrain(Terrain::Desert),
                    '~' => Tile::terrain(Terrain::Water),
                    '^' => Tile::terrain(Terrain::Mountain),
                    'c' => Tile::interactive(Interact::TreasureChest),
                    'f' => Tile::interactive(Interact::Foe),
                    't' => Tile::interactive(Interact::Town),
                    _ => Tile::plain_grass(),
                });
            }
        }
        let map = Map::from_tiles(w, h, tiles).expect("rectangular");
        // `Fog::reveal` ignores its radius and stamps 5x5, so a test that
        // needs exact fog writes the field, as map_render's tests do.
        let mut fog = Fog::new();
        for y in 0..h {
            for x in 0..w {
                fog.seen[y][x] = true;
            }
        }
        (map, fog)
    }

    fn hero_at(x: i32, y: i32) -> Game {
        let mut game = Game::default();
        game.position.x = x;
        game.position.y = y;
        game
    }

    fn walk_to(game: &Game, path: &[(i32, i32)]) -> (i32, i32) {
        path.iter()
            .fold((game.position.x, game.position.y), |(x, y), (dx, dy)| {
                (x + dx, y + dy)
            })
    }

    #[test]
    fn a_straight_route_is_one_step_per_tile() {
        let (map, fog) = world(&["....."]);
        let path = plan(&hero_at(0, 0), &map, &fog, (4, 0)).expect("route");
        assert_eq!(path, vec![(1, 0); 4]);
    }

    #[test]
    fn diagonals_are_used_like_the_keypad() {
        let (map, fog) = world(&["...", "...", "..."]);
        assert_eq!(
            plan(&hero_at(0, 0), &map, &fog, (2, 2)),
            Some(vec![(1, 1), (1, 1)])
        );
    }

    #[test]
    fn the_route_goes_around_mountains() {
        let (map, fog) = world(&[".^.", ".^.", "..."]);
        let game = hero_at(0, 0);
        let path = plan(&game, &map, &fog, (2, 0)).expect("route");
        assert_eq!(walk_to(&game, &path), (2, 0));
        assert_eq!(path.len(), 4);
    }

    /// Fewest days: four grass steps beat one desert step (cost 40).
    #[test]
    fn the_route_detours_around_desert_when_that_is_fewer_days() {
        let (map, fog) = world(&[".d.", "..."]);
        let path = plan(&hero_at(0, 0), &map, &fog, (2, 0)).expect("route");
        assert_eq!(path, vec![(1, 1), (1, -1)]);
    }

    #[test]
    fn unseen_tiles_are_neither_route_nor_goal() {
        let (map, _) = world(&["....."]);
        let mut fog = Fog::new();
        for x in 0..3 {
            fog.seen[0][x] = true;
        }
        assert_eq!(plan(&hero_at(0, 0), &map, &fog, (4, 0)), None);
        assert!(plan(&hero_at(0, 0), &map, &fog, (2, 0)).is_some());
    }

    #[test]
    fn an_interactive_tile_is_a_goal_but_never_a_waypoint() {
        let (map, fog) = world(&[".c.", "..."]);
        let game = hero_at(0, 0);
        let around = plan(&game, &map, &fog, (2, 0)).expect("route");
        assert!(!around.is_empty());
        let mut pos = (0, 0);
        for (dx, dy) in &around[..around.len() - 1] {
            pos = (pos.0 + dx, pos.1 + dy);
            assert_ne!(pos, (1, 0), "walked through the chest");
        }
        let (map, fog) = world(&["..t"]);
        assert_eq!(plan(&game, &map, &fog, (2, 0)), Some(vec![(1, 0), (1, 0)]));
    }

    /// GAP 3: no attack prompt yet, so a foe is never a route's goal.
    #[test]
    fn a_foe_is_never_a_goal() {
        let (map, fog) = world(&["...f"]);
        assert_eq!(plan(&hero_at(0, 0), &map, &fog, (3, 0)), None);
    }

    /// Boarding is allowed as the last step only, and a walker plans on land.
    #[test]
    fn boarding_the_boat_ends_a_route() {
        let (map, fog) = world(&["..~~"]);
        let mut game = hero_at(0, 0);
        game.boat.has_boat = true;
        game.boat.x = 2;
        game.boat.y = 0;
        assert_eq!(plan(&game, &map, &fog, (2, 0)), Some(vec![(1, 0), (1, 0)]));
        assert_eq!(
            plan(&game, &map, &fog, (3, 0)),
            None,
            "no sailing on past boarding"
        );
    }

    /// GAP 1 today: a flyer still avoids interactive tiles mid-route.
    #[test]
    fn a_flyer_crosses_mountains_but_avoids_interactions_today() {
        let (map, fog) = world(&[".^c.", "...."]);
        let mut game = hero_at(0, 0);
        game.character.mount = openbounty_engine::game::Mount::Fly;
        let path = plan(&game, &map, &fog, (3, 0)).expect("route");
        let mut pos = (0, 0);
        for (dx, dy) in &path[..path.len() - 1] {
            pos = (pos.0 + dx, pos.1 + dy);
            assert_ne!(pos, (2, 0));
        }
        assert_eq!(plan(&game, &map, &fog, (2, 1)).map(|p| p.len()), Some(2));
    }

    #[test]
    fn no_route_and_the_hero_tile_plan_nothing() {
        let (map, fog) = world(&[".^."]);
        let game = hero_at(0, 0);
        let (walled, wfog) = world(&[".^.", "^^."]);
        assert_eq!(plan(&game, &walled, &wfog, (2, 0)), None);
        assert_eq!(plan(&game, &map, &fog, (0, 0)), None);
    }

    use openbounty_engine::resources::Resources;
    use openbounty_engine::step::{game_step_basic, StepAdvance, StepInteraction, StepResult};

    fn resources() -> Resources {
        Resources::from_json_str(r#"{"render": {"mode": "legacy"}, "troops": []}"#)
            .expect("fixture parses")
    }

    #[test]
    fn an_adjacent_click_is_that_direction_key_even_onto_a_foe() {
        let (map, fog) = world(&["..f"]);
        let game = hero_at(1, 0);
        assert!(matches!(
            click(&game, &map, &fog, (2, 0), 0.0),
            ClickMove::Step(1, 0)
        ));
        assert!(matches!(
            click(&game, &map, &fog, (0, 0), 0.0),
            ClickMove::Step(-1, 0)
        ));
        assert!(matches!(
            click(&game, &map, &fog, (1, 0), 0.0),
            ClickMove::Nothing
        ));
    }

    #[test]
    fn a_far_click_routes_and_the_walk_paces_its_steps() {
        let (map, fog) = world(&["...."]);
        let game = hero_at(0, 0);
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (3, 0), 10.0) else {
            panic!("expected a route");
        };
        assert!(matches!(
            walk.next(&game, &map, &fog, 10.0),
            WalkTick::Step(1, 0)
        ));
        assert!(matches!(
            walk.next(&game, &map, &fog, 10.05),
            WalkTick::Wait
        ));
    }

    /// Driving the walk through `game_step_basic` leaves the world exactly as
    /// pressing the same arrows does.
    #[test]
    fn a_walk_equals_the_same_arrow_presses() {
        let (map0, fog0) = world(&["..d..", "....."]);
        let resources = resources();
        let mut game = hero_at(0, 0);
        game.stats.steps_left_today = 40;
        let (mut map, mut fog) = (map0.clone(), fog0.clone());
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (4, 0), 0.0) else {
            panic!("expected a route");
        };
        let mut dirs = Vec::new();
        let mut now = 0.0;
        loop {
            match walk.next(&game, &map, &fog, now) {
                WalkTick::Step(dx, dy) => {
                    dirs.push((dx, dy));
                    let result = game_step_basic(&mut game, &mut map, &mut fog, &resources, dx, dy);
                    if stops_walk(&result) {
                        break;
                    }
                }
                WalkTick::Wait => {}
                WalkTick::Done => break,
            }
            now += WALK_STEP_SECONDS;
        }
        let mut keyed = hero_at(0, 0);
        keyed.stats.steps_left_today = 40;
        let (mut kmap, mut kfog) = (map0, fog0);
        for (dx, dy) in &dirs {
            game_step_basic(&mut keyed, &mut kmap, &mut kfog, &resources, *dx, *dy);
        }
        assert_eq!((game.position.x, game.position.y), (4, 0));
        assert_eq!(game, keyed);
        assert_eq!(map, kmap);
        assert_eq!(fog, kfog);
    }

    /// A tile that changed since planning stops the walk instead of acting.
    #[test]
    fn a_tile_that_changed_since_planning_stops_the_walk() {
        let (mut map, fog) = world(&["...."]);
        let game = hero_at(0, 0);
        let ClickMove::Route(mut walk) = click(&game, &map, &fog, (3, 0), 0.0) else {
            panic!("expected a route");
        };
        map.stamp_foe(1, 0, "band0");
        assert!(matches!(walk.next(&game, &map, &fog, 0.0), WalkTick::Done));
    }

    #[test]
    fn the_walk_stops_on_interactions_collisions_week_ends_and_blocks() {
        let moved = |interaction, collision, week_ended| StepResult::Moved {
            terrain: Terrain::Grass,
            move_cost: 1,
            advance: StepAdvance {
                day_ended: false,
                week_ended,
                commission_paid: 0,
            },
            interaction,
            collision,
            bounced: false,
        };
        assert!(!stops_walk(&moved(StepInteraction::None, None, false)));
        assert!(stops_walk(&moved(StepInteraction::Alcove, None, false)));
        assert!(stops_walk(&moved(StepInteraction::None, None, true)));
        assert!(stops_walk(&moved(
            StepInteraction::None,
            Some(openbounty_engine::step::FoeCollision {
                id: "band0".into(),
                friendly: false,
                x: 0,
                y: 0
            }),
            false
        )));
        assert!(stops_walk(&StepResult::BlockedEdge));
        assert!(stops_walk(&StepResult::NoDirection));
    }
}
