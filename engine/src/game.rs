use crate::rng::{seed_from_index, Rng};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScepterState {
    pub key: i32,
    pub continent: i32,
}

impl Default for ScepterState {
    fn default() -> Self {
        Self {
            key: 0,
            continent: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Game {
    pub seed_index: i32,
    pub seed_from_catalog: bool,
    pub seed: u64,
    pub scepter: ScepterState,
    world_rng: Rng,
}

impl Default for Game {
    fn default() -> Self {
        Self {
            seed_index: 0,
            seed_from_catalog: false,
            seed: 0,
            scepter: ScepterState::default(),
            world_rng: Rng::from_state(0),
        }
    }
}

impl Game {
    pub fn init_seeded(
        &mut self,
        name: Option<&str>,
        player_class: i32,
        _difficulty: i32,
        seed_index: i32,
    ) {
        let seed_saved = self.seed;
        *self = Self::default();

        if seed_index >= 0 {
            self.seed_index = seed_index & 0xFF;
            self.seed_from_catalog = true;
            self.seed = seed_from_index(self.seed_index as u8);
        } else if seed_saved != 0 {
            self.seed = seed_saved;
            self.seed_from_catalog = false;
        } else {
            let generated_index = generated_seed_index(name, player_class);
            self.seed_index = generated_index;
            self.seed_from_catalog = true;
            self.seed = seed_from_index(generated_index as u8);
        }

        self.world_rng = Rng::seed(self.seed);
        self.scepter.key = self.world_rng.next(0, 255);
        self.scepter.continent = self.world_rng.next(0, 3);
    }

    pub fn rng_next(&mut self, min: i32, max: i32) -> i32 {
        self.world_rng.next(min, max)
    }

    pub fn rng_snapshot(&self) -> u64 {
        self.world_rng.state()
    }

    pub fn rng_restore(&mut self, state: u64) {
        self.world_rng = Rng::from_state(state);
    }
}

fn generated_seed_index(name: Option<&str>, player_class: i32) -> i32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let mut h = seconds;
    h ^= (player_class as u64).wrapping_mul(2_654_435_761);
    for byte in name.unwrap_or("").bytes() {
        h = h.wrapping_mul(31).wrapping_add(byte as u64);
    }
    (h & 0xFF) as i32
}

#[cfg(test)]
mod tests {
    use super::Game;

    #[test]
    fn init_seeded_matches_openbounty_first_world_rng_rolls() {
        let cases = [
            (0, 0xe220_a839_7b1d_cdaf, 249, 0, 0x413f_5b10_e08e_6d4c),
            (1, 0x910a_2dec_8902_5cc1, 172, 3, 0x80ff_c0ab_e8e3_2f46),
            (2, 0x9758_35de_1c97_56ce, 176, 1, 0xe80d_b8f1_06ef_b795),
            (42, 0xbdd7_3226_2feb_6e95, 107, 2, 0x9f2a_ee8a_789e_c472),
            (255, 0x338c_5071_4628_3fb4, 133, 1, 0x3af1_ff29_32cd_babb),
        ];

        for (seed_index, seed, key, continent, rng_state) in cases {
            let mut game = Game::default();
            game.init_seeded(Some("Hero"), 0, 0, seed_index);

            assert_eq!(game.seed_index, seed_index);
            assert!(game.seed_from_catalog);
            assert_eq!(game.seed, seed);
            assert_eq!(game.scepter.key, key);
            assert_eq!(game.scepter.continent, continent);
            assert_eq!(game.rng_snapshot(), rng_state);
        }
    }

    #[test]
    fn init_seeded_masks_catalog_seed_index_to_u8() {
        let mut game = Game::default();
        game.init_seeded(None, 0, 0, 257);

        assert_eq!(game.seed_index, 1);
        assert_eq!(game.seed, 0x910a_2dec_8902_5cc1);
    }

    #[test]
    fn raw_seed_path_preserves_caller_supplied_seed() {
        let mut game = Game {
            seed: 12_345,
            ..Game::default()
        };

        game.init_seeded(Some("Hero"), 0, 0, -1);

        assert_eq!(game.seed, 12_345);
        assert!(!game.seed_from_catalog);
        assert_eq!(game.scepter.key, 159);
        assert_eq!(game.scepter.continent, 1);
    }

    #[test]
    fn rng_snapshot_restore_round_trips_world_rng_state() {
        let mut game = Game::default();
        game.init_seeded(Some("Hero"), 0, 0, 42);

        let snap = game.rng_snapshot();
        let first = game.rng_next(0, 99);
        assert_ne!(game.rng_snapshot(), snap);

        game.rng_restore(snap);
        assert_eq!(game.rng_next(0, 99), first);
    }
}
