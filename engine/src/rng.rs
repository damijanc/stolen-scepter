const MULT: u64 = 25_214_903_917;
const INC: u64 = 11;
const SCRAMBLE: u64 = 0x5DEECE66D;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    pub fn seed(seed: u64) -> Self {
        Self {
            state: seed ^ SCRAMBLE,
        }
    }

    pub fn from_state(state: u64) -> Self {
        Self { state }
    }

    pub fn state(self) -> u64 {
        self.state
    }

    pub fn next(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }

        self.state = self.state.wrapping_mul(MULT).wrapping_add(INC);
        let result = (self.state >> 32) as u32;
        min + (result % ((max - min + 1) as u32)) as i32
    }
}

pub fn seed_from_index(index: u8) -> u64 {
    let mut z = (index as u64).wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::{seed_from_index, Rng};

    #[test]
    fn seed_from_index_matches_openbounty_splitmix64_vectors() {
        let cases = [
            (0, 0xe220_a839_7b1d_cdaf),
            (1, 0x910a_2dec_8902_5cc1),
            (2, 0x9758_35de_1c97_56ce),
            (3, 0x1d0b_14e4_db01_8fed),
            (42, 0xbdd7_3226_2feb_6e95),
            (255, 0x338c_5071_4628_3fb4),
        ];

        for (index, expected) in cases {
            assert_eq!(seed_from_index(index), expected, "index {index}");
        }
    }

    #[test]
    fn next_matches_openbounty_lcg_vectors() {
        let mut rng = Rng::seed(12_345);
        let got = [
            rng.next(1, 100),
            rng.next(1, 100),
            rng.next(1, 100),
            rng.next(1, 100),
            rng.next(1, 100),
        ];

        assert_eq!(got, [72, 78, 41, 45, 74]);
        assert_eq!(rng.state(), 0x325b_d545_798e_f4c3);
    }

    #[test]
    fn min_greater_or_equal_max_does_not_advance_state() {
        let mut rng = Rng::seed(77);
        let before = rng.state();

        assert_eq!(rng.next(5, 5), 5);
        assert_eq!(rng.state(), before);

        assert_eq!(rng.next(6, 5), 6);
        assert_eq!(rng.state(), before);
    }
}
