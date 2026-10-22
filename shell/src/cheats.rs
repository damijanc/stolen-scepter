//! The F10 debug cheat menu, porting `src/shell_cheats.c`.
//!
//! C host code, not an engine core: every cheat writes a `Game` (or live
//! `Fog`) field directly, exactly as `cheat_menu_tick` does, so there is no
//! engine method to call and none is added. [`apply_cheat`] is the pure
//! letter-to-effect core; [`CheatMenu::tick`] is the key routing around it.
//! Neither touches a graphics library.
//!
//! Gating, ported as C has it: none. `main.c` calls `cheat_menu_tick` at the
//! top of every main-loop frame, in release and debug builds alike, with no
//! flag. The only places it cannot fire are the ones C runs in a SEPARATE
//! loop -- the startup wizard, combat (`RunCombat`) and the blocking win
//! cartoon (`run_end_cartoon`) -- and `main.rs` excludes the last two
//! explicitly, because this shell runs them inside its one loop.
//!
//! DIVERGENCE, deliberate: C drains raylib's `GetKeyPressed` queue, which
//! `ui_any_key_pressed` (the dialog) shares but which the `IsKeyPressed`
//! handlers further down the frame do not, so an ESC that closes the menu
//! over, say, the home castle also leaves the castle that same frame. This
//! shell's `KeySource` has no queue, so `main.rs` skips the whole input
//! chain on any frame the menu is open or has just handled a key. C pops one
//! key per frame in press order; with no queue, a frame holding two letters
//! resolves to the first in A..Z order.
//!
//! DIVERGENCE, deliberate: C queues the menu text with `player_io_message`,
//! so F10 over an open dialog waits behind it (and the next menu key's
//! `dialog_dismiss` tears down the covered dialog, not the menu). This shell
//! has no message queue by design, so the menu replaces the open dialog.
//!
//! DIVERGENCE, deliberate: the additive cheats use `wrapping_add`, per the
//! parity rule. C's signed `int` overflow is undefined; a debug Rust build
//! would panic instead.

use openbounty_engine::fog::{Fog, MAP_MAX_H, MAP_MAX_W};
use openbounty_engine::game::{Game, Mount};
use openbounty_engine::map::Map;
use openbounty_engine::resources::Resources;

use crate::dialog::Dialog;
use crate::input::{Key, KeySource};

/// The menu dialog's header, C's `player_io_message(game, "Debug menu", ..)`.
pub const MENU_HEADER: &str = "Debug menu";

/// C's `CHEAT_MENU_BODY`, verbatim. Seven lines: exactly one dialog page.
pub const MENU_BODY: &str = "G  Gold        N  Zone\n\
                             V  Leadership  O  Fog\n\
                             M  Magic       W  Win\n\
                             U  Spells      L  Lose\n\
                             S  Siege\n\
                             F  Flight\n\
                             F10/ESC  close";

/// The header every non-terminal cheat's result is shown under.
pub const RESULT_HEADER: &str = "Debug";

/// `U` touches at most this many spell slots: C's hard-coded `si < 14`, the
/// stock 7 combat + 7 adventure. The same bound `Game::discard_spell` uses.
const SPELL_SLOTS: usize = 14;

/// The letter keys in A..Z order, which is also the tie-break when two land
/// in one frame.
const LETTERS: [(Key, char); 26] = [
    (Key::A, 'A'),
    (Key::B, 'B'),
    (Key::C, 'C'),
    (Key::D, 'D'),
    (Key::E, 'E'),
    (Key::F, 'F'),
    (Key::G, 'G'),
    (Key::H, 'H'),
    (Key::I, 'I'),
    (Key::J, 'J'),
    (Key::K, 'K'),
    (Key::L, 'L'),
    (Key::M, 'M'),
    (Key::N, 'N'),
    (Key::O, 'O'),
    (Key::P, 'P'),
    (Key::Q, 'Q'),
    (Key::R, 'R'),
    (Key::S, 'S'),
    (Key::T, 'T'),
    (Key::U, 'U'),
    (Key::V, 'V'),
    (Key::W, 'W'),
    (Key::X, 'X'),
    (Key::Y, 'Y'),
    (Key::Z, 'Z'),
];

/// What one letter did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheatOutcome {
    /// State was changed (or the letter was unknown); show this body under
    /// [`RESULT_HEADER`].
    Message(String),
    /// `W`: the caller plays the win ending. Nothing is written here --
    /// `show_win_game` sets `game_over`, as C's does.
    Win,
    /// `L`: the caller opens the lose screen. C's `show_lose_game` writes no
    /// state at all, so the game is still live once the screen is dismissed.
    Lose,
}

/// Apply one cheat letter, porting the `switch (letter)` in
/// `cheat_menu_tick`. `fog` is the LIVE fog, the one `main.c` passes; the
/// per-continent snapshots in `game.world.continent_fog` are not touched.
pub fn apply_cheat(
    letter: char,
    game: &mut Game,
    resources: &Resources,
    map: &Map,
    fog: &mut Fog,
) -> CheatOutcome {
    let body = match letter {
        'G' => {
            game.stats.gold = game.stats.gold.wrapping_add(50000);
            "Gold +50000".to_owned()
        }
        'V' => {
            game.stats.leadership_current = game.stats.leadership_current.wrapping_add(100);
            game.stats.leadership_base = game.stats.leadership_base.wrapping_add(100);
            "Leadership +100".to_owned()
        }
        'M' => {
            game.stats.spell_power = game.stats.spell_power.wrapping_add(1);
            game.stats.max_spells = game.stats.max_spells.wrapping_add(1);
            "Magic boosted".to_owned()
        }
        'U' => {
            let n = resources.spells_count();
            let mut si = 0;
            while si < n && si < SPELL_SLOTS {
                game.spells.counts[si] = game.spells.counts[si].wrapping_add(1);
                si += 1;
            }
            "+1 of every spell".to_owned()
        }
        'S' => {
            game.stats.siege_weapons = 1;
            "Siege weapons granted".to_owned()
        }
        'F' => {
            // Straight to the field, as C: no `player_can_fly` check, and the
            // travel mode is left alone.
            game.character.mount = Mount::Fly;
            "Flight granted".to_owned()
        }
        'N' => {
            let mut found = None;
            for zi in 0..resources.zone_count() {
                if !game.world.zones_discovered[zi] {
                    game.world.zones_discovered[zi] = true;
                    found = Some(zi);
                    break;
                }
            }
            match found.and_then(|zi| resources.zone_by_index(zi)) {
                Some(zone) => format!("Revealed: {}", zone.id.as_str()),
                None => "All zones already known".to_owned(),
            }
        }
        'O' => {
            // C walks map->height x map->width over the fixed 64x64 grid; a
            // loaded map never exceeds it (`MapLoadZone` refuses), so the
            // clamp only guards a hand-built test map.
            let mut revealed = 0;
            for y in 0..map.height.min(MAP_MAX_H) {
                for x in 0..map.width.min(MAP_MAX_W) {
                    if !fog.seen[y][x] {
                        fog.seen[y][x] = true;
                        revealed += 1;
                    }
                }
            }
            format!("Map fog cleared ({revealed} tiles)")
        }
        'W' => return CheatOutcome::Win,
        'L' => return CheatOutcome::Lose,
        other => format!("Unknown cheat: {other}"),
    };
    CheatOutcome::Message(body)
}

/// What a frame of the menu did, porting `CheatResult`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheatResult {
    /// Nothing consumed: the menu is closed and F10 was not pressed, or the
    /// menu is open and no key it reads went down.
    Idle,
    /// F10 opened the menu.
    Opened,
    /// ESC or F10 closed it.
    Dismissed,
    /// A non-terminal letter was applied and its result is in the dialog.
    Dispatched,
    /// `W` or `L`: the caller opens the ending. C's `CHEAT_DISPATCHED_TERMINAL`.
    Terminal(CheatOutcome),
}

/// The menu's one piece of state, C's `s_cheat_menu_active`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CheatMenu {
    active: bool,
}

impl CheatMenu {
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// One frame, porting `cheat_menu_tick`. The menu and its results are
    /// shown in `dialog`; the endings are left to the caller, which owns the
    /// views. The open branch returns before anything else is read, the way
    /// C drains the key queue after opening, and ESC/F10 are tested before
    /// the letters, as C tests them before `KEY_A..KEY_Z`.
    pub fn tick(
        &mut self,
        keys: &dyn KeySource,
        game: &mut Game,
        resources: &Resources,
        map: &Map,
        fog: &mut Fog,
        dialog: &mut Dialog,
    ) -> CheatResult {
        if !self.active {
            if keys.pressed(Key::F10) {
                self.active = true;
                dialog.open(MENU_HEADER, MENU_BODY);
                return CheatResult::Opened;
            }
            return CheatResult::Idle;
        }

        if keys.pressed(Key::Escape) || keys.pressed(Key::F10) {
            self.active = false;
            dialog.dismiss();
            return CheatResult::Dismissed;
        }
        let Some(letter) = LETTERS
            .iter()
            .find(|(key, _)| keys.pressed(*key))
            .map(|(_, letter)| *letter)
        else {
            return CheatResult::Idle;
        };

        self.active = false;
        dialog.dismiss();
        match apply_cheat(letter, game, resources, map, fog) {
            CheatOutcome::Message(body) => {
                dialog.open(RESULT_HEADER, &body);
                CheatResult::Dispatched
            }
            ending => CheatResult::Terminal(ending),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::Keys;
    use crate::views::{ViewKind, Views};

    /// Three zones and sixteen spells: enough to see `N` walk the zone list
    /// and `U` stop at slot 14 with slots left over.
    fn resources() -> Resources {
        let spells: Vec<String> = (0..16)
            .map(|i| {
                format!(r#"{{"index":{i},"id":"s{i}","name":"S{i}","kind":"combat","cost":1}}"#)
            })
            .collect();
        Resources::from_json_str(&format!(
            r#"{{"troops": [],
                "zones": [{{"id":"home"}}, {{"id":"greenreach"}}, {{"id":"isleward"}}],
                "spells": [{}]}}"#,
            spells.join(",")
        ))
        .expect("fixture parses")
    }

    fn apply(letter: char, game: &mut Game) -> CheatOutcome {
        let mut fog = Fog::new();
        apply_cheat(letter, game, &resources(), &Map::new(4, 4), &mut fog)
    }

    fn message(body: &str) -> CheatOutcome {
        CheatOutcome::Message(body.to_owned())
    }

    #[test]
    fn the_menu_body_is_c_verbatim_and_fits_one_dialog_page() {
        assert_eq!(MENU_BODY.lines().count(), 7);
        assert_eq!(MENU_BODY.lines().next(), Some("G  Gold        N  Zone"));
        assert_eq!(MENU_BODY.lines().last(), Some("F10/ESC  close"));
        let mut dialog = Dialog::default();
        dialog.open(MENU_HEADER, MENU_BODY);
        assert_eq!(dialog.page_count(), 1);
    }

    #[test]
    fn g_adds_fifty_thousand_gold() {
        let mut game = Game::default();
        game.stats.gold = 1000;
        assert_eq!(apply('G', &mut game), message("Gold +50000"));
        assert_eq!(game.stats.gold, 51000);
    }

    #[test]
    fn v_raises_current_and_base_leadership_together() {
        let mut game = Game::default();
        game.stats.leadership_base = 100;
        game.stats.leadership_current = 60;
        assert_eq!(apply('V', &mut game), message("Leadership +100"));
        assert_eq!(game.stats.leadership_base, 200);
        assert_eq!(game.stats.leadership_current, 160);
    }

    #[test]
    fn m_raises_spell_power_and_max_spells_but_not_knows_magic() {
        let mut game = Game::default();
        game.stats.spell_power = 1;
        game.stats.max_spells = 2;
        assert_eq!(apply('M', &mut game), message("Magic boosted"));
        assert_eq!(game.stats.spell_power, 2);
        assert_eq!(game.stats.max_spells, 3);
        assert!(!game.stats.knows_magic, "C leaves knows_magic alone");
    }

    #[test]
    fn u_adds_one_of_every_spell_in_the_first_fourteen_slots_only() {
        let mut game = Game::default();
        game.spells.counts[0] = 2;
        assert_eq!(apply('U', &mut game), message("+1 of every spell"));
        assert_eq!(game.spells.counts[0], 3);
        for si in 1..14 {
            assert_eq!(game.spells.counts[si], 1, "slot {si}");
        }
        // The pack has 16 spells; C's `si < 14` leaves the last two alone.
        assert_eq!(game.spells.counts[14], 0);
        assert_eq!(game.spells.counts[15], 0);
    }

    #[test]
    fn u_stops_at_the_catalog_when_it_has_fewer_than_fourteen_spells() {
        let resources = Resources::from_json_str(
            r#"{"troops": [], "spells": [
                {"index":0,"id":"a","name":"A","kind":"combat","cost":1},
                {"index":1,"id":"b","name":"B","kind":"adventure","cost":1}
            ]}"#,
        )
        .expect("fixture parses");
        let mut game = Game::default();
        let mut fog = Fog::new();
        apply_cheat('U', &mut game, &resources, &Map::new(1, 1), &mut fog);
        assert_eq!(&game.spells.counts[..3], &[1, 1, 0]);
    }

    #[test]
    fn s_sets_siege_weapons_to_one_rather_than_adding() {
        let mut game = Game::default();
        assert_eq!(apply('S', &mut game), message("Siege weapons granted"));
        assert_eq!(game.stats.siege_weapons, 1);
        apply('S', &mut game);
        assert_eq!(game.stats.siege_weapons, 1);
    }

    #[test]
    fn f_sets_the_fly_mount_without_asking_whether_the_army_can_fly() {
        let mut game = Game::default();
        assert_eq!(game.character.mount, Mount::Ride);
        assert_eq!(apply('F', &mut game), message("Flight granted"));
        assert_eq!(game.character.mount, Mount::Fly);
    }

    #[test]
    fn n_discovers_the_first_undiscovered_zone_and_names_it_by_id() {
        let mut game = Game::default();
        game.world.zones_discovered[0] = true;
        assert_eq!(apply('N', &mut game), message("Revealed: greenreach"));
        assert!(game.world.zones_discovered[1]);
        assert!(!game.world.zones_discovered[2], "one zone per press");

        assert_eq!(apply('N', &mut game), message("Revealed: isleward"));
        assert_eq!(apply('N', &mut game), message("All zones already known"));
        // Bounded by the pack's zone count, not the array's capacity.
        assert!(!game.world.zones_discovered[3]);
    }

    #[test]
    fn o_clears_the_live_fog_over_the_map_and_counts_what_was_hidden() {
        let resources = resources();
        let mut game = Game::default();
        let map = Map::new(10, 6);
        let mut fog = Fog::new();
        fog.reveal(&map, 2, 2, 2); // a 5x5 square already seen

        let outcome = apply_cheat('O', &mut game, &resources, &map, &mut fog);
        assert_eq!(outcome, message("Map fog cleared (35 tiles)"));
        for y in 0..6 {
            for x in 0..10 {
                assert!(fog.seen(x, y), "({x},{y})");
            }
        }
        // Only the map's own rectangle, not the rest of the 64x64 grid.
        assert!(!fog.seen(10, 0));
        assert!(!fog.seen(0, 6));
        // The per-continent snapshot is not the fog C passes.
        assert_eq!(game.world.continent_fog[0], Fog::new());

        let again = apply_cheat('O', &mut game, &resources, &map, &mut fog);
        assert_eq!(again, message("Map fog cleared (0 tiles)"));
    }

    #[test]
    fn w_and_l_are_terminal_and_write_no_state_themselves() {
        let mut game = Game::default();
        assert_eq!(apply('W', &mut game), CheatOutcome::Win);
        assert_eq!(apply('L', &mut game), CheatOutcome::Lose);
        assert_eq!(game, Game::default());
    }

    #[test]
    fn an_unbound_letter_reports_itself_and_changes_nothing() {
        let mut game = Game::default();
        assert_eq!(apply('X', &mut game), message("Unknown cheat: X"));
        // A menu letter is not an overworld binding: A is not "view army".
        assert_eq!(apply('A', &mut game), message("Unknown cheat: A"));
        assert_eq!(game, Game::default());
    }

    struct Frame {
        menu: CheatMenu,
        game: Game,
        resources: Resources,
        map: Map,
        fog: Fog,
        dialog: Dialog,
    }

    impl Frame {
        fn new() -> Self {
            Self {
                menu: CheatMenu::default(),
                game: Game::default(),
                resources: resources(),
                map: Map::new(4, 4),
                fog: Fog::new(),
                dialog: Dialog::default(),
            }
        }

        fn press(&mut self, keys: &[Key]) -> CheatResult {
            self.menu.tick(
                &Keys::pressed(keys),
                &mut self.game,
                &self.resources,
                &self.map,
                &mut self.fog,
                &mut self.dialog,
            )
        }
    }

    #[test]
    fn the_menu_is_closed_until_f10_and_letters_do_nothing_meanwhile() {
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::G]), CheatResult::Idle);
        assert_eq!(frame.game.stats.gold, 0);
        assert!(!frame.menu.is_active());
        assert!(!frame.dialog.is_active());
    }

    #[test]
    fn f10_opens_the_menu_dialog() {
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::F10]), CheatResult::Opened);
        assert!(frame.menu.is_active());
        assert!(frame.dialog.is_active());
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
        assert_eq!(frame.dialog.body_text(), MENU_BODY);
    }

    #[test]
    fn the_opening_press_does_not_also_close_the_menu() {
        // C drains the key queue after opening so the same F10 is not read
        // again as "close"; here the open path returns before the close check.
        let mut frame = Frame::new();
        assert_eq!(frame.press(&[Key::F10, Key::G]), CheatResult::Opened);
        assert!(frame.menu.is_active());
        assert_eq!(
            frame.game.stats.gold, 0,
            "the G in the opening frame is drained"
        );
    }

    #[test]
    fn escape_or_a_second_f10_closes_the_menu() {
        for close in [Key::Escape, Key::F10] {
            let mut frame = Frame::new();
            frame.press(&[Key::F10]);
            assert_eq!(frame.press(&[close]), CheatResult::Dismissed, "{close:?}");
            assert!(!frame.menu.is_active());
            assert!(!frame.dialog.is_active());
        }
    }

    #[test]
    fn a_non_letter_leaves_the_menu_open() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::Up]), CheatResult::Idle);
        assert_eq!(frame.press(&[Key::Digit1]), CheatResult::Idle);
        assert_eq!(frame.press(&[]), CheatResult::Idle);
        assert!(frame.menu.is_active());
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
    }

    #[test]
    fn a_letter_applies_closes_the_menu_and_shows_the_result() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::G]), CheatResult::Dispatched);
        assert_eq!(frame.game.stats.gold, 50000);
        assert!(!frame.menu.is_active(), "one cheat per F10");
        assert!(frame.dialog.is_active());
        assert_eq!(frame.dialog.header_text(), RESULT_HEADER);
        assert_eq!(frame.dialog.body_text(), "Gold +50000");

        // Closed again, so a second G is idle.
        assert_eq!(frame.press(&[Key::G]), CheatResult::Idle);
        assert_eq!(frame.game.stats.gold, 50000);
    }

    #[test]
    fn an_unknown_letter_still_closes_the_menu() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(frame.press(&[Key::Q]), CheatResult::Dispatched);
        assert!(!frame.menu.is_active());
        assert_eq!(frame.dialog.body_text(), "Unknown cheat: Q");
    }

    #[test]
    fn two_letters_in_one_frame_resolve_in_alphabetical_order() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        frame.press(&[Key::V, Key::G]);
        assert_eq!(frame.game.stats.gold, 50000);
        assert_eq!(frame.game.stats.leadership_base, 0);
    }

    #[test]
    fn f10_over_an_open_dialog_replaces_it_with_the_menu() {
        let mut frame = Frame::new();
        frame.dialog.open("Sign", "Welcome");
        frame.press(&[Key::F10]);
        assert_eq!(frame.dialog.header_text(), MENU_HEADER);
    }

    #[test]
    fn w_closes_the_menu_and_hands_the_win_to_the_caller() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(
            frame.press(&[Key::W]),
            CheatResult::Terminal(CheatOutcome::Win)
        );
        assert!(!frame.menu.is_active());
        assert!(
            !frame.dialog.is_active(),
            "no Debug message after an ending"
        );
        assert!(
            !frame.game.stats.game_over,
            "the caller's show_win_game sets it"
        );

        // What main.rs then does with it. The fixture ships no ending tiles,
        // so the cartoon is skipped and the win screen opens directly.
        let mut views = Views::new();
        crate::show_win_game(&mut views, &frame.resources, &mut frame.game);
        assert!(frame.game.stats.game_over);
        assert!(!frame.game.stats.won, "C's show_win_game never sets won");
        assert_eq!(views.active(), Some(ViewKind::Win));
    }

    #[test]
    fn l_opens_the_lose_screen_without_ending_the_game() {
        let mut frame = Frame::new();
        frame.press(&[Key::F10]);
        assert_eq!(
            frame.press(&[Key::L]),
            CheatResult::Terminal(CheatOutcome::Lose)
        );
        assert!(!frame.dialog.is_active());

        let mut views = Views::new();
        crate::show_lose_game(&mut views, &frame.resources, &frame.game);
        assert_eq!(views.active(), Some(ViewKind::Lose));
        // C's show_lose_game writes nothing, so dismissing the screen returns
        // to a live board.
        assert!(!frame.game.stats.game_over);
    }
}
