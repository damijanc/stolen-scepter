use crate::fog::Fog;
use crate::game::{CastleOwnerKind, ChestOutcome, ChestPending, Game, Mount, TravelMode};
use crate::map::{Map, Tile};
use crate::resources::Resources;
use crate::tables::FixedStr;
use crate::tile::{Interact, Terrain};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StepAdvance {
    pub day_ended: bool,
    pub week_ended: bool,
    pub commission_paid: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StepInteraction {
    None,
    CastleGate {
        id: String,
    },
    Town {
        id: String,
        boat_x: i32,
        boat_y: i32,
    },
    TreasureChest {
        id: String,
        outcome: ChestOutcome,
        pending: ChestPending,
    },
    Sign {
        id: String,
        title: String,
        body: String,
    },
    Artifact {
        id: String,
    },
    Dwelling {
        id: String,
        kind: Interact,
        troop_id: String,
        count: i32,
    },
    Alcove,
    Orb {
        id: String,
        zone_index: Option<usize>,
    },
    Telecave {
        id: String,
        destination: Option<(i32, i32)>,
    },
    Navmap {
        id: String,
        discovered_zone: Option<usize>,
    },
    Foe {
        id: String,
        friendly: bool,
    },
}

impl Default for StepInteraction {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
        interaction: StepInteraction,
        /// A foe walked onto the hero during this step's follow pass. The
        /// shell opens the attack or recruit prompt against it; `friendly`
        /// says which.
        collision: Option<FoeCollision>,
    },
}

/// A foe that stepped onto the hero, and the tile they now share.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoeCollision {
    pub id: String,
    pub friendly: bool,
    pub x: i32,
    pub y: i32,
}

impl StepResult {
    pub fn moved(self) -> bool {
        matches!(self, Self::Moved { .. })
    }
}

pub fn game_step_basic(
    game: &mut Game,
    map: &mut Map,
    fog: &mut Fog,
    resources: &Resources,
    dx: i32,
    dy: i32,
) -> StepResult {
    if dx == 0 && dy == 0 {
        return StepResult::NoDirection;
    }

    // The recruit-source markers share in_town's lifetime: a transaction is
    // only legal in the step that stepped onto the gate or dwelling tile, so
    // clear them up front and let the paths below re-set them.
    game.position.in_town = FixedStr::empty();
    game.position.home_castle = FixedStr::empty();
    game.position.own_castle = FixedStr::empty();
    game.position.dwelling_troop = FixedStr::empty();
    game.position.dwelling_x = -1;
    game.position.dwelling_y = -1;

    let nx = game.position.x + dx;
    let ny = game.position.y + dy;
    let Some(tile) = map.tile_i32(nx, ny).copied() else {
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
        walkable_on_foot(&tile)
    } else {
        walkable_in_boat(&tile)
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

    // C reveals immediately after the move and before any interaction fires,
    // so a bounce-back still leaves the destination revealed.
    fog.reveal(map, nx, ny, resources.world.fog_sight);

    let terrain_costs_full_day = !flying && tile.terrain == Terrain::Desert;
    // Timestop freezes the world, so the foe-follow pass is skipped with it.
    // Sample the flag BEFORE consuming the step: consume_step decrements
    // time_stop, so reading it afterwards cannot tell "last frozen step" from
    // "not frozen at all".
    let frozen = game.stats.time_stop > 0;
    let advance = consume_step(game, resources, terrain_costs_full_day);
    let collision = if frozen {
        None
    } else {
        game.foes_follow(map).map(|index| {
            let foe = game.foes[index];
            FoeCollision {
                id: foe.placement_id.as_str().to_owned(),
                friendly: foe.friendly,
                x: game.position.x,
                y: game.position.y,
            }
        })
    };
    let interaction = resolve_interaction(game, map, resources, &tile, nx, ny);

    StepResult::Moved {
        terrain: tile.terrain,
        move_cost: step_move_cost(tile.terrain, flying),
        advance,
        interaction,
        collision,
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

fn resolve_interaction(
    game: &mut Game,
    map: &mut Map,
    resources: &Resources,
    tile: &Tile,
    x: i32,
    y: i32,
) -> StepInteraction {
    let zone = game.position.zone.as_str().to_owned();
    let id = tile.id.as_str().to_owned();

    match tile.interactive {
        Interact::None => StepInteraction::None,
        Interact::CastleGate => {
            if let Some(castle) = game
                .castles
                .iter_mut()
                .find(|castle| castle.id.as_str() == id)
            {
                castle.visited = true;
                castle.known = true;
            }
            // The hero is now AT this castle. C tests audience first and only
            // then player ownership, so the audience castle never sets
            // own_castle. Both markers are set before any bounce-back so a
            // transaction this step is legal, mirroring the in_town set.
            if resources
                .castle_by_id(id.as_str())
                .is_some_and(|castle| castle.is_home())
            {
                game.position.home_castle.copy_from_str(id.as_str());
            } else if game.castles.iter().any(|castle| {
                castle.id.as_str() == id && castle.owner_kind == CastleOwnerKind::Player
            }) {
                game.position.own_castle.copy_from_str(id.as_str());
            }
            StepInteraction::CastleGate { id }
        }
        Interact::Town => {
            if let Some(town) = game.towns.iter_mut().find(|town| town.id.as_str() == id) {
                town.visited = true;
            }
            game.position.in_town.copy_from_str(id.as_str());
            let (boat_x, boat_y) = resources
                .town_by_id(id.as_str())
                .map(|town| (town.boat_x, town.boat_y))
                .unwrap_or((tile.boat_spawn_x, tile.boat_spawn_y));
            StepInteraction::Town { id, boat_x, boat_y }
        }
        Interact::TreasureChest => {
            let zone_index = resources.zone_index_by_id(zone.as_str()).unwrap_or(0);
            let (outcome, pending) = game.roll_chest(resources, zone_index, x, y);
            consume_tile(game, map, zone.as_str(), x, y);
            StepInteraction::TreasureChest {
                id,
                outcome,
                pending,
            }
        }
        Interact::Sign => StepInteraction::Sign {
            id,
            title: tile.sign_title.as_str().to_owned(),
            body: tile.sign_body.as_str().to_owned(),
        },
        Interact::Artifact => {
            game.claim_artifact_by_id(resources, id.as_str());
            consume_tile(game, map, zone.as_str(), x, y);
            StepInteraction::Artifact { id }
        }
        Interact::DwellingPlains
        | Interact::DwellingForest
        | Interact::DwellingHills
        | Interact::DwellingDungeon => {
            let kind = dwelling_kind_name(tile.interactive);
            let dwelling = game.touch_dwelling(resources, zone.as_str(), x, y, kind);
            StepInteraction::Dwelling {
                id,
                kind: tile.interactive,
                troop_id: dwelling
                    .map(|dwelling| dwelling.troop_id.as_str().to_owned())
                    .unwrap_or_default(),
                count: dwelling.map(|dwelling| dwelling.count).unwrap_or(0),
            }
        }
        Interact::Alcove => {
            game.stats.knows_magic = true;
            consume_tile(game, map, zone.as_str(), x, y);
            game.consume_magic_alcoves(resources);
            StepInteraction::Alcove
        }
        Interact::Orb => {
            let zone_index = game.activate_orb(resources);
            consume_tile(game, map, zone.as_str(), x, y);
            StepInteraction::Orb { id, zone_index }
        }
        Interact::Telecave => {
            let destination = game.resolve_telecave();
            StepInteraction::Telecave { id, destination }
        }
        Interact::Navmap => {
            let discovered_zone = game.resolve_navmap(resources, id.as_str());
            consume_tile(game, map, zone.as_str(), x, y);
            StepInteraction::Navmap {
                id,
                discovered_zone,
            }
        }
        Interact::Foe => {
            let foe = game.foes[..game.foe_count]
                .iter()
                .find(|foe| foe.alive && foe.zone.as_str() == zone && foe.x == x && foe.y == y);
            StepInteraction::Foe {
                id: foe
                    .map(|foe| foe.placement_id.as_str().to_owned())
                    .filter(|placement_id| !placement_id.is_empty())
                    .unwrap_or(id),
                friendly: foe.map(|foe| foe.friendly).unwrap_or(false),
            }
        }
    }
}

fn consume_tile(game: &mut Game, map: &mut Map, zone: &str, x: i32, y: i32) {
    map.clear_interactive(x, y);
    game.add_consumed(zone, x, y);
}

fn dwelling_kind_name(interactive: Interact) -> &'static str {
    match interactive {
        Interact::DwellingForest => "forest",
        Interact::DwellingHills => "hills",
        Interact::DwellingDungeon => "dungeon",
        _ => "plains",
    }
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
        advance.commission_paid = game.apply_week_end(resources);
    }
}

/// What a dig turned up. C's `flow_apply_search` reports this through three
/// out-parameters plus a bool return.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchOutcome {
    /// The hero was standing on the buried Scepter. The game is won and
    /// `stats.won` is now set; no days are spent.
    Won,
    /// Nothing here. `week_commission` is non-zero only when the days spent
    /// crossed a week boundary, which the shell must settle for parity.
    NothingFound { weeks: i32, week_commission: i32 },
    /// The search ran the calendar out. C reports this instead of the
    /// "revealed nothing" message.
    OutOfTime,
}

/// Advance the calendar by `days`, returning the number of week boundaries
/// crossed and the commission credited across them. Ports `GameSpendDays`.
///
/// C keeps this next to its own file-local `end_day`; so does this port,
/// even though that puts it in `step.rs` rather than alongside the rest of
/// `game.c`.
pub fn spend_days(game: &mut Game, resources: &Resources, days: i32) -> (i32, i32) {
    let days = days.min(game.stats.days_left);
    let mut weeks = 0_i32;
    let mut total_commission = 0_i32;
    let mut spent = 0_i32;
    while spent < days && !game.stats.game_over {
        let mut advance = StepAdvance::default();
        end_day(game, resources, &mut advance);
        if advance.week_ended {
            weeks = weeks.wrapping_add(1);
        }
        if advance.commission_paid > 0 {
            total_commission = total_commission.wrapping_add(advance.commission_paid);
        }
        spent = spent.wrapping_add(1);
    }
    (weeks, total_commission)
}

/// Dig where the hero stands. Ports `flow_apply_search`, minus the yes/no
/// prompt: C bails unless the answer was yes, so the engine core takes the
/// yes as given and the shell asks the question.
///
/// Finding the Scepter costs nothing. Anything else costs
/// `tuning.search_cost_days`, which is why a fruitless dig can end the game.
pub fn search_here(game: &mut Game, resources: &Resources) -> SearchOutcome {
    let on_scepter = game.scepter.zone.as_str() == game.position.zone.as_str()
        && game.scepter.x == game.position.x
        && game.scepter.y == game.position.y;
    if on_scepter {
        // C persists the win on the Game as well as reporting it, so
        // consumers that never see the return value can still observe it.
        game.stats.won = true;
        return SearchOutcome::Won;
    }

    let (weeks, paid) = spend_days(game, resources, resources.tuning.search_cost_days);
    if game.stats.game_over {
        return SearchOutcome::OutOfTime;
    }
    SearchOutcome::NothingFound {
        weeks,
        // C reports the commission only when a boundary was actually
        // crossed, even though the accumulator is the same either way.
        week_commission: if weeks > 0 { paid } else { 0 },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        game_step_basic, search_here, spend_days, FoeCollision, SearchOutcome, StepInteraction,
        StepResult,
    };
    use crate::fog::Fog;
    use crate::game::{CastleOwnerKind, FoeState, Game, Mount, TravelMode};
    use crate::map::{Map, Tile};
    use crate::resources::Resources;
    use crate::tile::{Interact, Terrain};

    #[test]
    fn step_basic_moves_updates_last_position_and_consumes_grass_step() {
        let mut map = Map::from_tiles(
            3,
            1,
            vec![
                Tile::plain_grass(),
                Tile::plain_grass(),
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

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: Default::default(),
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (0, 0));
        assert_eq!(game.position.in_town.as_str(), "");
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_desert_sets_steps_left_to_zero() {
        let mut map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Desert)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        let resources = time_fixture();
        game.stats.steps_left_today = 33;
        game.stats.days_left = 10;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Desert,
                move_cost: 40,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                },
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 33);
        assert_eq!(game.stats.days_left, 9);
    }

    #[test]
    fn step_basic_time_stop_absorbs_step_cost() {
        let mut map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Desert)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        let resources = time_fixture();
        game.stats.steps_left_today = 33;
        game.stats.time_stop = 2;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert!(result.moved());
        assert_eq!(game.stats.time_stop, 1);
        assert_eq!(game.stats.steps_left_today, 33);
    }

    /// A grass strip with the hero at (1, 0) and one live foe at (3, 0).
    fn following_foe_game(friendly: bool) -> (Game, Map, Resources) {
        let map =
            Map::from_tiles(6, 1, vec![Tile::plain_grass(); 6]).expect("fixture dimensions match");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 1;
        game.position.y = 0;
        game.stats.steps_left_today = 40;
        let mut foe = FoeState::default();
        foe.zone = game.position.zone;
        foe.x = 3;
        foe.y = 0;
        foe.placement_id.copy_from_str("band0");
        foe.alive = true;
        foe.friendly = friendly;
        game.foes[0] = foe;
        game.foe_count = 1;
        (game, map, time_fixture())
    }

    fn step_collision(result: &StepResult) -> Option<FoeCollision> {
        match result {
            StepResult::Moved { collision, .. } => collision.clone(),
            other => panic!("expected movement, got {other:?}"),
        }
    }

    #[test]
    fn step_basic_reveals_fog_around_the_destination() {
        let mut map =
            Map::from_tiles(8, 8, vec![Tile::plain_grass(); 64]).expect("fixture dimensions match");
        let mut game = Game::default();
        let resources = time_fixture();
        game.position.x = 4;
        game.position.y = 4;
        game.stats.steps_left_today = 40;
        let mut fog = Fog::new();

        game_step_basic(&mut game, &mut map, &mut fog, &resources, 1, 0);

        // The 5x5 square is centred on where the hero landed, not where it
        // started.
        assert!(fog.seen(5, 4));
        assert!(fog.seen(7, 6), "corner of the square around (5,4)");
        assert!(!fog.seen(2, 4), "two columns behind the start stays dark");
    }

    #[test]
    fn step_basic_reveals_even_when_the_move_is_blocked_by_an_interaction() {
        // C reveals before the interaction block runs, so a destination that
        // bounces the hero back is still revealed.
        let mut map =
            Map::from_tiles(8, 8, vec![Tile::plain_grass(); 64]).expect("fixture dimensions match");
        let mut game = Game::default();
        let resources = time_fixture();
        game.position.x = 4;
        game.position.y = 4;
        game.stats.steps_left_today = 40;
        let mut fog = Fog::new();

        game_step_basic(&mut game, &mut map, &mut fog, &resources, 0, 1);

        assert!(fog.seen(4, 5));
        assert!(fog.seen(4, 7));
    }

    #[test]
    fn step_basic_advances_foes_during_the_step() {
        let (mut game, mut map, resources) = following_foe_game(false);

        // Hero retreats (1,0) -> (0,0). The foe chases the tile just left,
        // (1,0), closing to (2,0) without reaching the hero.
        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, -1, 0);

        assert_eq!(step_collision(&result), None);
        assert_eq!((game.foes[0].x, game.foes[0].y), (2, 0));
        assert_eq!(
            map.tile_i32(2, 0).expect("tile exists").interactive,
            Interact::Foe,
            "the follow pass ran inside the step"
        );
    }

    #[test]
    fn step_basic_reports_the_foe_that_walked_onto_the_hero() {
        let (mut game, mut map, resources) = following_foe_game(false);

        // Hero advances (1,0) -> (2,0). The cell closest to the tile it left
        // IS the tile it moved to, so the foe walks onto the hero.
        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        let collision = step_collision(&result).expect("the foe reached the hero");
        assert_eq!(collision.id, "band0");
        assert!(!collision.friendly);
        assert_eq!(
            (collision.x, collision.y),
            (game.position.x, game.position.y)
        );
        assert_eq!((game.foes[0].x, game.foes[0].y), (2, 0));
    }

    #[test]
    fn step_basic_marks_a_friendly_collision_so_the_shell_can_pick_the_prompt() {
        let (mut game, mut map, resources) = following_foe_game(true);

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        let collision = step_collision(&result).expect("the foe reached the hero");
        assert!(collision.friendly);
    }

    #[test]
    fn step_basic_freezes_foes_while_time_stop_is_banked() {
        let (mut game, mut map, resources) = following_foe_game(false);
        game.stats.time_stop = 2;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(step_collision(&result), None);
        assert_eq!(
            (game.foes[0].x, game.foes[0].y),
            (3, 0),
            "timestop freezes the world, foes included"
        );
        // The flag is sampled before the step consumes it, so the last frozen
        // step still freezes.
        assert_eq!(game.stats.time_stop, 1);
    }

    #[test]
    fn step_basic_resumes_following_on_the_step_after_time_stop_lapses() {
        let (mut game, mut map, resources) = following_foe_game(false);
        game.stats.time_stop = 1;

        game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);
        assert_eq!((game.foes[0].x, game.foes[0].y), (3, 0), "still frozen");
        assert_eq!(game.stats.time_stop, 0);

        game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);
        assert_ne!((game.foes[0].x, game.foes[0].y), (3, 0), "moving again");
    }

    #[test]
    fn step_basic_blocks_edge_and_nonwalkable_terrain_without_moving() {
        let mut map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Forest)],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        let resources = time_fixture();
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0),
            StepResult::BlockedTerrain {
                terrain: Terrain::Forest
            }
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!((game.position.last_x, game.position.last_y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);

        assert_eq!(
            game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, -1, 0),
            StepResult::BlockedEdge
        );
        assert_eq!((game.position.x, game.position.y), (0, 0));
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_blocks_plain_water_when_walking_without_boat() {
        let mut map = Map::from_tiles(
            2,
            1,
            vec![Tile::plain_grass(), Tile::terrain(Terrain::Water)],
        )
        .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0),
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
        let mut map = Map::from_tiles(
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

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Water,
                move_cost: 1,
                advance: Default::default(),
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_does_not_board_boat_left_in_other_zone() {
        let mut map = Map::from_tiles(
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
            game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0),
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
        let mut map = Map::from_tiles(
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

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (2, 0));
        assert_eq!(game.travel_mode, TravelMode::Boat);
        assert_eq!((game.boat.x, game.boat.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 3);

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (3, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!((game.boat.x, game.boat.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 2);
    }

    #[test]
    fn step_basic_flying_bypasses_terrain_and_blockers() {
        let mut map = Map::from_tiles(
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

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (2, 0));
        assert_eq!(game.stats.steps_left_today, 3);

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (3, 0));
        assert_eq!(game.stats.steps_left_today, 2);

        assert!(game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0).moved());
        assert_eq!((game.position.x, game.position.y), (4, 0));
        assert_eq!(game.stats.steps_left_today, 1);
    }

    #[test]
    fn step_basic_flying_over_desert_costs_one_step_not_full_day() {
        let mut map = Map::from_tiles(
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

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Desert,
                move_cost: 1,
                advance: Default::default(),
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!(game.stats.steps_left_today, 32);
        assert_eq!(game.stats.days_left, 10);
    }

    #[test]
    fn step_basic_flying_does_not_board_boat_on_target_tile() {
        let mut map = Map::from_tiles(
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

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Water,
                move_cost: 1,
                advance: Default::default(),
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!((game.position.x, game.position.y), (1, 0));
        assert_eq!(game.travel_mode, TravelMode::Walk);
        assert_eq!((game.boat.x, game.boat.y), (1, 0));
        assert_eq!(game.stats.steps_left_today, 4);
    }

    #[test]
    fn step_basic_zero_delta_is_noop() {
        let mut map = Map::new(1, 1);
        let mut game = Game::default();
        let resources = time_fixture();
        game.position.in_town.copy_from_str("town0");
        game.stats.steps_left_today = 5;

        assert_eq!(
            game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 0, 0),
            StepResult::NoDirection
        );
        assert_eq!(game.position.in_town.as_str(), "town0");
        assert_eq!(game.stats.steps_left_today, 5);
    }

    #[test]
    fn step_basic_grass_rolls_day_when_steps_reach_zero() {
        let mut map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 10;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                },
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!(game.stats.steps_left_today, 33);
        assert_eq!(game.stats.days_left, 9);
        assert!(!game.stats.game_over);
    }

    #[test]
    fn step_basic_applies_week_boundary_economy() {
        let mut map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 6;
        game.stats.gold = 100;
        game.stats.commission_weekly = 25;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: true,
                    commission_paid: 25
                },
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!(game.stats.days_left, 5);
        assert_eq!(game.stats.gold, 125);
        assert_eq!(game.stats.last_commission, 25);
        assert_eq!(game.stats.last_astrology_troop, 0);
    }

    #[test]
    fn step_basic_sets_game_over_when_final_day_ends() {
        let mut map = Map::from_tiles(2, 1, vec![Tile::plain_grass(), Tile::plain_grass()])
            .expect("fixture dimensions match");
        let resources = time_fixture();
        let mut game = Game::default();
        game.stats.steps_left_today = 1;
        game.stats.days_left = 1;

        let result = game_step_basic(&mut game, &mut map, &mut Fog::new(), &resources, 1, 0);

        assert_eq!(
            result,
            StepResult::Moved {
                terrain: Terrain::Grass,
                move_cost: 1,
                advance: super::StepAdvance {
                    day_ended: true,
                    week_ended: false,
                    commission_paid: 0
                },
                interaction: StepInteraction::None,
                collision: None
            }
        );
        assert_eq!(game.stats.days_left, 0);
        assert_eq!(game.stats.steps_left_today, 33);
        assert!(game.stats.game_over);
    }

    #[test]
    fn step_basic_reports_fixture_interactions_and_consumes_pickups() {
        let mut map = Map::from_tiles(
            12,
            1,
            vec![
                Tile::plain_grass(),
                tile_with_id(Interact::Town, "town0"),
                tile_with_id(Interact::CastleGate, "castle0"),
                sign_tile("sign0", "Beware", "Keep your army close."),
                tile_with_id(Interact::TreasureChest, "chest0"),
                tile_with_id(Interact::Artifact, "sword"),
                tile_with_id(Interact::DwellingForest, "dwelling0"),
                tile_with_id(Interact::Alcove, "alcove0"),
                tile_with_id(Interact::Navmap, "navmap0"),
                tile_with_id(Interact::Orb, "orb0"),
                tile_with_id(Interact::Telecave, "telecave0"),
                tile_with_id(Interact::Foe, "foe_tile"),
            ],
        )
        .expect("fixture dimensions match");
        let resources = interaction_fixture();
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.world.zones_discovered[0] = true;
        game.stats.steps_left_today = 40;
        game.towns[0].id.copy_from_str("town0");
        game.castles[0].id.copy_from_str("castle0");
        game.dwellings[0].zone.copy_from_str("home");
        game.dwellings[0].x = 6;
        game.dwellings[0].y = 0;
        game.dwellings[0].troop_id.copy_from_str("sprites");
        game.dwellings[0].count = 12;
        game.dwelling_count = 1;
        game.foes[0].zone.copy_from_str("home");
        game.foes[0].x = 11;
        game.foes[0].y = 0;
        game.foes[0].placement_id.copy_from_str("foe0");
        game.foes[0].alive = true;
        game.foes[0].friendly = true;
        game.foe_count = 1;

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Town {
                id: "town0".to_owned(),
                boat_x: 20,
                boat_y: 21
            }
        );
        assert_eq!(game.position.in_town.as_str(), "town0");
        assert!(game.towns[0].visited);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::CastleGate {
                id: "castle0".to_owned()
            }
        );
        assert!(game.castles[0].visited);
        assert!(game.castles[0].known);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Sign {
                id: "sign0".to_owned(),
                title: "Beware".to_owned(),
                body: "Keep your army close.".to_owned()
            }
        );

        let (chest_outcome, chest_pending) = game.peek_chest(&resources, 0, 4, 0);
        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::TreasureChest {
                id: "chest0".to_owned(),
                outcome: chest_outcome,
                pending: chest_pending
            }
        );
        assert_consumed(&game, &map, "home", 4, 0);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Artifact {
                id: "sword".to_owned()
            }
        );
        assert!(game.artifacts.found[0]);
        assert_consumed(&game, &map, "home", 5, 0);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Dwelling {
                id: "dwelling0".to_owned(),
                kind: Interact::DwellingForest,
                troop_id: "sprites".to_owned(),
                count: 12
            }
        );
        assert_eq!(game.position.dwelling_troop.as_str(), "sprites");
        assert_eq!((game.position.dwelling_x, game.position.dwelling_y), (6, 0));

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Alcove
        );
        assert!(game.position.dwelling_troop.is_empty());
        assert!(game.stats.knows_magic);
        assert_consumed(&game, &map, "home", 7, 0);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Navmap {
                id: "navmap0".to_owned(),
                discovered_zone: Some(1)
            }
        );
        assert!(game.world.zones_discovered[1]);
        assert_consumed(&game, &map, "home", 8, 0);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Orb {
                id: "orb0".to_owned(),
                zone_index: Some(0)
            }
        );
        assert!(game.world.orbs_found[0]);
        assert_consumed(&game, &map, "home", 9, 0);

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Telecave {
                id: "telecave0".to_owned(),
                destination: None
            }
        );
        assert_eq!(
            map.tile_i32(10, 0).expect("telecave tile").interactive,
            Interact::Telecave
        );

        assert_eq!(
            move_interaction(&mut game, &mut map, &resources),
            StepInteraction::Foe {
                id: "foe0".to_owned(),
                friendly: true
            }
        );
    }

    fn move_interaction(game: &mut Game, map: &mut Map, resources: &Resources) -> StepInteraction {
        match game_step_basic(game, map, &mut Fog::new(), resources, 1, 0) {
            StepResult::Moved { interaction, .. } => interaction,
            other => panic!("expected movement, got {other:?}"),
        }
    }

    fn assert_consumed(game: &Game, map: &Map, zone: &str, x: i32, y: i32) {
        assert!(game.is_consumed(zone, x, y));
        assert_eq!(
            map.tile_i32(x, y)
                .expect("consumed tile exists")
                .interactive,
            Interact::None
        );
    }

    fn tile_with_id(interactive: Interact, id: &str) -> Tile {
        let mut tile = Tile::interactive(interactive);
        tile.id.copy_from_str(id);
        tile
    }

    fn sign_tile(id: &str, title: &str, body: &str) -> Tile {
        let mut tile = tile_with_id(Interact::Sign, id);
        tile.sign_title.copy_from_str(title);
        tile.sign_body.copy_from_str(body);
        tile
    }

    fn search_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "time": {"day_steps": 33, "week_days": 5},
              "tuning": {"search_cost_days": 10},
              "troops": []
            }"#,
        )
        .expect("fixture parses")
    }

    fn digger() -> Game {
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.position.x = 3;
        game.position.y = 4;
        game.scepter.zone.copy_from_str("home");
        game.scepter.x = 9;
        game.scepter.y = 9;
        game.stats.days_left = 100;
        game
    }

    #[test]
    fn digging_on_the_scepter_wins_and_costs_no_days() {
        let resources = search_fixture();
        let mut game = digger();
        game.scepter.x = game.position.x;
        game.scepter.y = game.position.y;

        assert_eq!(search_here(&mut game, &resources), SearchOutcome::Won);
        // The win is persisted, not merely returned.
        assert!(game.stats.won);
        assert_eq!(game.stats.days_left, 100);
    }

    #[test]
    fn the_scepter_only_counts_in_its_own_zone() {
        let resources = search_fixture();
        let mut game = digger();
        game.scepter.x = game.position.x;
        game.scepter.y = game.position.y;
        game.scepter.zone.copy_from_str("isle");

        // Same coordinates, different continent: still a fruitless dig.
        assert!(matches!(
            search_here(&mut game, &resources),
            SearchOutcome::NothingFound { .. }
        ));
        assert!(!game.stats.won);
    }

    #[test]
    fn a_fruitless_dig_spends_the_search_cost_and_reports_a_crossed_week() {
        let resources = search_fixture();
        let mut game = digger();
        game.stats.days_left = 52;
        game.stats.commission_weekly = 25;
        game.stats.gold = 0;

        // 10 days from 52 lands on 42, crossing the boundaries at 50 and 45.
        assert_eq!(
            search_here(&mut game, &resources),
            SearchOutcome::NothingFound {
                weeks: 2,
                week_commission: 50
            }
        );
        assert_eq!(game.stats.days_left, 42);
        assert_eq!(game.stats.gold, 50);
    }

    #[test]
    fn spending_days_that_cross_no_week_boundary_credits_nothing() {
        let resources = search_fixture();
        let mut game = digger();
        game.stats.days_left = 53;
        game.stats.commission_weekly = 25;
        game.stats.gold = 0;

        // 53 down to 51 touches no multiple of the 5-day week. A full dig
        // cannot show this: 10 days always spans two boundaries.
        assert_eq!(spend_days(&mut game, &resources, 2), (0, 0));
        assert_eq!(game.stats.days_left, 51);
        assert_eq!(game.stats.gold, 0);
    }

    #[test]
    fn a_dig_cannot_spend_more_days_than_remain() {
        let resources = search_fixture();
        let mut game = digger();
        game.stats.days_left = 3;

        // C clamps `days` to days_left, so the loop stops at the calendar's
        // end rather than running days_left negative.
        assert_eq!(search_here(&mut game, &resources), SearchOutcome::OutOfTime);
        assert_eq!(game.stats.days_left, 0);
        assert!(game.stats.game_over);
    }

    #[test]
    fn spending_days_stops_as_soon_as_the_game_is_over() {
        let resources = search_fixture();
        let mut game = digger();
        game.stats.days_left = 4;

        let (weeks, commission) = spend_days(&mut game, &resources, 4);
        assert_eq!(game.stats.days_left, 0);
        assert!(game.stats.game_over);
        // The final end_day returns before the week block, so a game-ending
        // day never credits commission even when it lands on a boundary.
        assert_eq!((weeks, commission), (0, 0));
    }

    #[test]
    fn spending_zero_or_negative_days_changes_nothing() {
        let resources = search_fixture();
        let mut game = digger();

        assert_eq!(spend_days(&mut game, &resources, 0), (0, 0));
        assert_eq!(spend_days(&mut game, &resources, -5), (0, 0));
        assert_eq!(game.stats.days_left, 100);
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

    #[test]
    fn audience_gate_sets_home_castle_marker_and_the_next_step_revokes_it() {
        let resources = Resources::from_json_str(
            r#"{
              "time": {"day_steps": 40},
              "zones": [{"id": "home"}],
              "castles": [
                {"index": 0, "id": "keep", "zone": "home", "x": 1, "y": 0,
                 "special": {"flow": "audience"}},
                {"index": 1, "id": "outpost", "zone": "home", "x": 3, "y": 0}
              ],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut map = Map::from_tiles(
            4,
            1,
            vec![
                Tile::plain_grass(),
                tile_with_id(Interact::CastleGate, "keep"),
                Tile::plain_grass(),
                tile_with_id(Interact::CastleGate, "outpost"),
            ],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.world.zones_discovered[0] = true;
        game.stats.steps_left_today = 40;
        game.castles[0].id.copy_from_str("keep");
        game.castles[1].id.copy_from_str("outpost");

        move_interaction(&mut game, &mut map, &resources);
        assert_eq!(game.position.home_castle.as_str(), "keep");

        // Stepping away revokes the recruit privilege, exactly like in_town.
        move_interaction(&mut game, &mut map, &resources);
        assert!(game.position.home_castle.is_empty());

        // A non-audience castle gate never sets the home marker; an owned one
        // sets own_castle instead.
        game.castles[1].owner_kind = CastleOwnerKind::Player;
        move_interaction(&mut game, &mut map, &resources);
        assert!(game.position.home_castle.is_empty());
        assert_eq!(game.position.own_castle.as_str(), "outpost");
    }

    #[test]
    fn audience_castle_never_sets_the_own_castle_marker() {
        let resources = Resources::from_json_str(
            r#"{
              "time": {"day_steps": 40},
              "zones": [{"id": "home"}],
              "castles": [
                {"index": 0, "id": "keep", "zone": "home", "x": 1, "y": 0,
                 "special": {"flow": "audience"}}
              ],
              "troops": []
            }"#,
        )
        .expect("fixture parses");
        let mut map = Map::from_tiles(
            2,
            1,
            vec![
                Tile::plain_grass(),
                tile_with_id(Interact::CastleGate, "keep"),
            ],
        )
        .expect("fixture dimensions match");
        let mut game = Game::default();
        game.position.zone.copy_from_str("home");
        game.world.zones_discovered[0] = true;
        game.stats.steps_left_today = 40;
        game.castles[0].id.copy_from_str("keep");
        // Player-owned AND the audience castle: C tests audience first, so
        // only the home marker is set.
        game.castles[0].owner_kind = CastleOwnerKind::Player;

        move_interaction(&mut game, &mut map, &resources);

        assert_eq!(game.position.home_castle.as_str(), "keep");
        assert!(game.position.own_castle.is_empty());
    }

    fn interaction_fixture() -> Resources {
        Resources::from_json_str(
            r#"{
              "time": {"day_steps": 40, "week_days": 5},
              "zones": [
                {"id": "home"},
                {"id": "away"}
              ],
              "towns": [
                {"index": 0, "id": "town0", "name": "Town", "zone": "home", "x": 1, "y": 0,
                 "boat": {"x": 20, "y": 21}}
              ],
              "artifacts": [
                {"index": 0, "id": "sword", "name": "Sword", "power": "increased_damage"}
              ],
              "troops": []
            }"#,
        )
        .expect("fixture parses")
    }
}
