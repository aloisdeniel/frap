// SPDX-License-Identifier: GPL-3.0-or-later
//! CPython's `random` module as `uv.randomize_uv_transform` uses it: the Mersenne Twister
//! (MT19937), `random.seed(int)` (`init_by_array` over the 32-bit words of `abs(n)`) and
//! `random.random()` (53-bit floats).
//!
//! Ported from CPython 3.13 `Modules/_randommodule.c` (`init_genrand`, `init_by_array`,
//! `genrand_uint32`, `random_random`, `random_seed`), PSF-2.0 (the MT19937 code is BSD-3 by
//! Makoto Matsumoto and Takuji Nishimura), both compatible with GPL-3.0-or-later.

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// A `random.Random` state.
pub(crate) struct PyRandom {
    state: [u32; N],
    index: usize,
}

impl PyRandom {
    /// `random.seed(n)` for an integer `n`.
    pub(crate) fn seeded(n: i64) -> PyRandom {
        let mut r = PyRandom {
            state: [0; N],
            index: N,
        };
        let mut n = n.unsigned_abs();
        let mut key: Vec<u32> = Vec::new();
        while n != 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        r.init_by_array(&key);
        r
    }

    fn init_genrand(&mut self, s: u32) {
        self.state[0] = s;
        for i in 1..N {
            let prev = self.state[i - 1];
            self.state[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let mt = &mut self.state;
        let mut i = 1usize;
        let mut j = 0usize;
        let mut k = N.max(key.len());
        while k > 0 {
            let prev = mt[i - 1];
            mt[i] = (mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = N - 1;
        while k > 0 {
            let prev = mt[i - 1];
            mt[i] = (mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                mt[0] = mt[N - 1];
                i = 1;
            }
            k -= 1;
        }
        mt[0] = 0x8000_0000;
    }

    /// `genrand_uint32`.
    fn next_u32(&mut self) -> u32 {
        let mag01 = [0u32, MATRIX_A];
        if self.index >= N {
            let mt = &mut self.state;
            let mut kk = 0;
            while kk < N - M {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M] ^ (y >> 1) ^ mag01[(y & 1) as usize];
                kk += 1;
            }
            while kk < N - 1 {
                let y = (mt[kk] & UPPER_MASK) | (mt[kk + 1] & LOWER_MASK);
                mt[kk] = mt[kk + M - N] ^ (y >> 1) ^ mag01[(y & 1) as usize];
                kk += 1;
            }
            let y = (mt[N - 1] & UPPER_MASK) | (mt[0] & LOWER_MASK);
            mt[N - 1] = mt[M - 1] ^ (y >> 1) ^ mag01[(y & 1) as usize];
            self.index = 0;
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// `random.random()`.
    pub(crate) fn random(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        (f64::from(a) * 67_108_864.0 + f64::from(b)) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `random.uniform(a, b)`: `a + (b - a) * random()`.
    pub(crate) fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cpython() {
        // `python3 -c "import random; random.seed(0); print(repr(random.random()), ...)"`.
        let mut r = PyRandom::seeded(0);
        assert_eq!(r.random(), 0.8444218515250481);
        assert_eq!(r.random(), 0.7579544029403025);
        let mut r = PyRandom::seeded(42);
        assert_eq!(r.random(), 0.6394267984578837);
        let mut r = PyRandom::seeded(-42);
        assert_eq!(r.random(), 0.6394267984578837);
        let mut r = PyRandom::seeded(1 << 40);
        assert_eq!(r.random(), 0.1036394562812375);
        let mut r = PyRandom::seeded(5);
        assert_eq!(r.uniform(0.0, 1.0), 0.6229016948897019);
        assert_eq!(r.uniform(0.0, 1.0), 0.7417869892607294);
        assert_eq!(r.uniform(0.0, 1.0), 0.7951935655656966);
    }
}
