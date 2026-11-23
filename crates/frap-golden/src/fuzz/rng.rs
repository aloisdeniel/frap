// SPDX-License-Identifier: GPL-3.0-or-later
//! The fuzzer's own generator: SplitMix64 (Steele, Lea, Flood 2014). It only draws the
//! statements; the statements themselves carry every seed the operators use (`seed=` of
//! `select_random`, `randomize_transform`, …), so both tools replay a case from its text and
//! Blender's own generators (`BliRng`) are never involved (WP 8.7).

/// A SplitMix64 stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// A stream from `seed`.
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    /// The next 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in `0..n` (`n > 0`; Lemire's multiply-shift, bias below 2^-32 for small `n`).
    pub fn below(&mut self, n: usize) -> usize {
        debug_assert!(n > 0);
        ((u128::from(self.next_u64()) * n as u128) >> 64) as usize
    }

    /// Uniform in `lo..=hi`.
    pub fn range_i64(&mut self, lo: i64, hi: i64) -> i64 {
        debug_assert!(lo <= hi);
        let span = (hi as i128 - lo as i128 + 1) as u128;
        (lo as i128 + ((u128::from(self.next_u64()) * span) >> 64) as i128) as i64
    }

    /// Uniform in `[0, 1)` (53 bits).
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / (1u64 << 53) as f64)
    }

    /// True with probability `p`.
    pub fn chance(&mut self, p: f64) -> bool {
        self.unit() < p
    }

    /// One element of a non-empty slice.
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len())]
    }

    /// The stream for item `index` of the run seeded with `seed` (independent streams per
    /// sequence, so `--seed S --count N` and `replay S+i` agree).
    pub fn sequence_seed(seed: u64, index: u64) -> u64 {
        Rng::new(seed ^ index.wrapping_mul(0xD1B5_4A32_D192_ED03)).next_u64()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_reference_values() {
        // SplitMix64 from seed 0 (the reference implementation's first outputs).
        let mut r = Rng::new(0);
        assert_eq!(r.next_u64(), 0xE220_A839_7B1D_CDAF);
        assert_eq!(r.next_u64(), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(r.next_u64(), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn ranges_stay_in_bounds() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            assert!(r.below(3) < 3);
            let v = r.range_i64(-2, 5);
            assert!((-2..=5).contains(&v));
            let u = r.unit();
            assert!((0.0..1.0).contains(&u));
        }
        assert_eq!(Rng::new(1).range_i64(4, 4), 4);
        // The full range does not overflow.
        let _ = Rng::new(1).range_i64(i64::MIN, i64::MAX);
    }
}
