// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.randomize_transform(random_seed=0, use_delta=False, use_loc=True,
//! loc=(0, 0, 0), use_rot=True, rot=(0, 0, 0), use_scale=True, scale_even=False,
//! scale=(1, 1, 1))`, tier T1 (`tests/golden/cases/object/transform_m8/randomize_*.py`,
//! WP 8.3.5).
//!
//! Ported from blender/scripts/startup/bl_operators/object_randomize_transform.py
//! (`randomize_selected`), Blender 5.2.4, GPL-2.0-or-later, and from CPython's `random` module
//! (`Modules/_randommodule.c`: MT19937 `init_by_array`, `random_seed`, `random_random`, and
//! `Lib/random.py` `uniform`), which the add-on draws from. Poll `context.mode == 'OBJECT'`.
//!
//! The values are Blender's per seed: the generator is seeded with `random_seed`, then each
//! selected object (base order) draws three numbers for the location, three for the rotation
//! and three for the scale, whether or not that part is used (`uniform(0, 0)` keeps the
//! sequence). Location: `location += uniform(-loc, loc)` (or `delta_location`); rotation: the
//! Euler angles (`delta_rotation_euler`) get `uniform(-rot, rot)`, a quaternion or axis-angle
//! object being switched to `XYZ` for the addition and back (the rotation converted both
//! ways); scale: `scale = uniform(2 - scale, scale) · scale` per axis (the X factor for every
//! axis with `scale_even`). The arithmetic follows Python's: `mathutils.Vector` additions in
//! single precision, the scalar products in double precision rounded to the float channel.

use frap_math::EulerOrder;
use frap_math::matrix::RotationMode;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

/// CPython's `random.Random` (MT19937 with CPython's seeding and 53-bit `random()`).
#[derive(Clone)]
pub struct PyRandom {
    mt: [u32; 624],
    index: usize,
}

impl PyRandom {
    const N: usize = 624;
    const M: usize = 397;

    /// `init_genrand`.
    fn init_genrand(s: u32) -> PyRandom {
        let mut mt = [0u32; 624];
        mt[0] = s;
        for i in 1..Self::N {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        PyRandom { mt, index: Self::N }
    }

    /// `random.seed(n)` for an integer: `init_by_array` with the 32-bit words of `|n|`
    /// (`[0]` for zero).
    pub fn seed(n: i64) -> PyRandom {
        let mut n = n.unsigned_abs();
        let mut key: Vec<u32> = Vec::new();
        while n != 0 {
            key.push(n as u32);
            n >>= 32;
        }
        if key.is_empty() {
            key.push(0);
        }
        let mut r = PyRandom::init_genrand(19_650_218);
        let mt = &mut r.mt;
        let (mut i, mut j) = (1usize, 0usize);
        let mut k = Self::N.max(key.len());
        while k > 0 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= Self::N {
                mt[0] = mt[Self::N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
            k -= 1;
        }
        k = Self::N - 1;
        while k > 0 {
            mt[i] = (mt[i] ^ (mt[i - 1] ^ (mt[i - 1] >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= Self::N {
                mt[0] = mt[Self::N - 1];
                i = 1;
            }
            k -= 1;
        }
        mt[0] = 0x8000_0000;
        r
    }

    /// `genrand_uint32`.
    fn next_u32(&mut self) -> u32 {
        const UPPER: u32 = 0x8000_0000;
        const LOWER: u32 = 0x7fff_ffff;
        const MATRIX_A: u32 = 0x9908_b0df;
        let mag = |y: u32| if y & 1 == 0 { 0 } else { MATRIX_A };
        if self.index >= Self::N {
            let mt = &mut self.mt;
            let mut kk = 0;
            while kk < Self::N - Self::M {
                let y = (mt[kk] & UPPER) | (mt[kk + 1] & LOWER);
                mt[kk] = mt[kk + Self::M] ^ (y >> 1) ^ mag(y);
                kk += 1;
            }
            while kk < Self::N - 1 {
                let y = (mt[kk] & UPPER) | (mt[kk + 1] & LOWER);
                mt[kk] = mt[kk + Self::M - Self::N] ^ (y >> 1) ^ mag(y);
                kk += 1;
            }
            let y = (mt[Self::N - 1] & UPPER) | (mt[0] & LOWER);
            mt[Self::N - 1] = mt[Self::M - 1] ^ (y >> 1) ^ mag(y);
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// `random.random()`: 53 random bits in `[0, 1)`.
    pub fn random(&mut self) -> f64 {
        let a = f64::from(self.next_u32() >> 5);
        let b = f64::from(self.next_u32() >> 6);
        (a * 67_108_864.0 + b) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// `random.uniform(a, b)`: `a + (b - a) · random()`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

/// `rand_vec(vec_range)`: `Vector(uniform(-v, v) for v in vec_range)` (single precision).
fn rand_vec(rng: &mut PyRandom, range: [f32; 3]) -> [f32; 3] {
    range.map(|v| {
        let v = f64::from(v);
        rng.uniform(-v, v) as f32
    })
}

#[operator(poll = poll::OBJECTMODE, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::RandomizeTransform) -> OpResult {
    let mut rng = PyRandom::seed(i64::from(p.random_seed.value));
    let delta = p.use_delta.value;
    let loc = p.use_loc.value.then_some(p.loc.value);
    let rot = p.use_rot.value.then_some(p.rot.value);
    let scale = p.use_scale.value.then_some(p.scale.value);
    let scale_even = p.scale_even.value;
    let objects = cx.ctx().selected_objects();
    for ob in objects {
        let o = cx.object_mut(ob)?;
        match loc {
            Some(range) => {
                let v = rand_vec(&mut rng, range);
                let target = if delta {
                    &mut o.delta_location
                } else {
                    &mut o.location
                };
                for i in 0..3 {
                    target[i] += v[i];
                }
            }
            None => {
                for _ in 0..3 {
                    rng.uniform(0.0, 0.0);
                }
            }
        }
        match rot {
            Some(range) => {
                let vec = rand_vec(&mut rng, range);
                let mode = o.rotation_mode_enum();
                let switch = matches!(mode, RotationMode::Quaternion | RotationMode::AxisAngle);
                if switch {
                    let mut t = o.transform();
                    t.set_rotation_mode(RotationMode::Euler(EulerOrder::XYZ));
                    o.set_transform(&t);
                }
                let target = if delta {
                    &mut o.delta_rotation_euler
                } else {
                    &mut o.rotation_euler
                };
                for i in 0..3 {
                    // `obj.rotation_euler[i] += vec[i]`: Python floats, stored as float.
                    target[i] = (f64::from(target[i]) + f64::from(vec[i])) as f32;
                }
                if switch {
                    let mut t = o.transform();
                    t.set_rotation_mode(mode);
                    o.set_transform(&t);
                }
            }
            None => {
                for _ in 0..3 {
                    rng.uniform(0.0, 0.0);
                }
            }
        }
        match scale {
            Some(s) => {
                let org = if delta { o.delta_scale } else { o.scale };
                let sca: [f64; 3] = [0, 1, 2].map(|i| {
                    let v = f64::from(s[i]);
                    rng.uniform(-v + 2.0, v)
                });
                let out: [f32; 3] = [0, 1, 2].map(|i| {
                    let f = if scale_even { sca[0] } else { sca[i] };
                    (f * f64::from(org[i])) as f32
                });
                if delta {
                    o.delta_scale = out;
                } else {
                    o.scale = out;
                }
            }
            None => {
                for _ in 0..3 {
                    rng.uniform(0.0, 0.0);
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cpython_random() {
        // CPython 3: `random.seed(0); [random.random() for _ in range(3)]`.
        let mut r = PyRandom::seed(0);
        assert_eq!(r.random(), 0.8444218515250481);
        assert_eq!(r.random(), 0.7579544029403025);
        assert_eq!(r.random(), 0.420571580830845);
        // `random.seed(42); random.uniform(-1.0, 2.0)`.
        let mut r = PyRandom::seed(42);
        assert_eq!(r.uniform(-1.0, 2.0), 0.9182803953736514);
        // A seed above 32 bits uses two key words (`random.seed(1 << 40)`).
        let mut r = PyRandom::seed(1 << 40);
        assert_eq!(r.random(), 0.1036394562812375);
    }
}
