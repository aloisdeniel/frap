// SPDX-License-Identifier: GPL-3.0-or-later
//! Temporal accumulation over `eevee.taa_render_samples` (WP 5.1): the per-sample random
//! dimensions of EEVEE's `Sampling::step()` (fixed Halton sequences, so a render is
//! deterministic), the sub-pixel jitter of the projection, and the light-shape positions the
//! soft shadows are rendered from (`shadow_setup`'s `SAMPLING_SHADOW_I` dimensions).
//!
//! Every sample re-renders the shadow maps from a point jittered on the light shape and the
//! scene with a jittered projection; the film accumulates the samples in `Rgba32Float`
//! ([`super::film`]). The average over the samples is the anti-aliased image with soft shadows.

use glam::{Mat4, Vec2, Vec3};

// Ported from blender/source/blender/blenlib/intern/rand.cc (`halton_ex`, v5.2.4).
/// Incremental Halton generator ("Instant Radiosity", Keller).
fn halton_ex(invprime: f64, offset: &mut f64) -> f64 {
    let e = ((1.0 - *offset) - 1e-10).abs();
    if invprime >= e {
        let mut lasth;
        let mut h = invprime;
        loop {
            lasth = h;
            h *= invprime;
            if h < e {
                break;
            }
        }
        *offset += (lasth + h) - 1.0;
    } else {
        *offset += invprime;
    }
    *offset
}

/// `BLI_halton_2d`/`BLI_halton_3d`: the `n`-th point of the Halton sequence of `primes`
/// (offset 0), i.e. `n` steps of `halton_ex`.
pub fn halton<const N: usize>(primes: [u32; N], n: u64) -> [f64; N] {
    let mut offset = [0.0f64; N];
    let mut r = [0.0f64; N];
    for _ in 0..n {
        for i in 0..N {
            r[i] = halton_ex(1.0 / f64::from(primes[i]), &mut offset[i]);
        }
    }
    r
}

fn fract(x: f64) -> f32 {
    (x - x.floor()) as f32
}

/// The random dimensions of one TAA sample (`Sampling::step()` for render, no motion blur,
/// no depth of field, film scaling 1).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sample {
    /// Sample index (0-based).
    pub index: u32,
    /// `SAMPLING_FILTER_U/V`: Halton (2, 3), shifted so the first sample is `(0.5, 0.5)`.
    pub filter: [f32; 2],
    /// `SAMPLING_SHADOW_I/J/K`: Halton (2, 3, 5), shifted so the first sample is
    /// `(0.5, 0.5, 0.5)` — the light-shape position of the soft shadows.
    pub shadow: [f32; 3],
    /// `SAMPLING_TRANSPARENCY` (Halton 7, the lens sequence's second dimension): the hashed
    /// (dithered) transparency threshold offset.
    pub transparency: f32,
    /// `SAMPLING_SHADOW_X/Y` (leaped Halton (2, 3)): rotation of the PCF kernel.
    pub pcf: [f32; 2],
}

impl Sample {
    /// The dimensions of sample `index` (0-based).
    pub fn new(index: u32) -> Sample {
        let n = u64::from(index) + 1;
        let f = halton([2, 3], n);
        let lens = halton([5, 7, 3], n);
        let s = halton([2, 3, 5], n);
        let pcf = halton([2, 3], u64::from(index) * 5 + 1);
        Sample {
            index,
            filter: [fract(f[0] + 0.5), fract(f[1] + 2.0 / 3.0)],
            shadow: [
                fract(s[0] + 0.5),
                fract(s[1] + 2.0 / 3.0),
                fract(s[2] + 0.8),
            ],
            transparency: lens[1] as f32,
            pcf: [pcf[0] as f32, pcf[1] as f32],
        }
    }

    /// The sub-pixel offset of the sample in render pixels (`Film::pixel_jitter_get`): a
    /// uniform disk of radius `filter_radius / 2` for filters under `1/√2` pixel, else the
    /// whole pixel `[-0.5, 0.5]²`. `x` right, `y` down (image rows).
    pub fn pixel_offset(&self, filter_radius: f32) -> [f32; 2] {
        if filter_radius < std::f32::consts::FRAC_1_SQRT_2 {
            let d = sample_disk(Vec2::from(self.filter)) * 0.5 * filter_radius;
            [d.x, d.y]
        } else {
            [self.filter[0] - 0.5, self.filter[1] - 0.5]
        }
    }
}

/// The samples of a render: `taa_render_samples` (0 means 1, as EEVEE clamps it).
pub fn samples(taa_render_samples: u32) -> Vec<Sample> {
    (0..taa_render_samples.max(1)).map(Sample::new).collect()
}

/// `Sampling::sample_disk`: uniform disk of radius 1.
pub fn sample_disk(rand: Vec2) -> Vec2 {
    let omega = rand.y * 2.0 * std::f32::consts::PI;
    rand.x.sqrt() * Vec2::new(omega.cos(), omega.sin())
}

/// `Sampling::sample_ball`: uniform ball of radius 1.
pub fn sample_ball(rand: Vec3) -> Vec3 {
    let z = rand.x * 2.0 - 1.0;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let omega = rand.y * 2.0 * std::f32::consts::PI;
    Vec3::new(r * omega.cos(), r * omega.sin(), z) * rand.z.sqrt().sqrt()
}

/// `sample_uniform_cone`: a direction in the cone of cosine `cos_angle` around +Z.
pub fn sample_uniform_cone(rand: Vec2, cos_angle: f32) -> Vec3 {
    let z = cos_angle + (1.0 - cos_angle) * rand.x; // cos theta
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = 2.0 * std::f32::consts::PI * rand.y;
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

/// The projection whose image is shifted so that render pixel `p` samples the scene at
/// `p + 0.5 + offset` (pixels, `y` down): a clip-space translation before the perspective
/// divide, valid for perspective and orthographic projections.
pub fn jitter_projection(projection: Mat4, offset: [f32; 2], size: [u32; 2]) -> Mat4 {
    let dx = -2.0 * offset[0] / size[0] as f32;
    let dy = 2.0 * offset[1] / size[1] as f32;
    Mat4::from_translation(Vec3::new(dx, dy, 0.0)) * projection
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halton_matches_blender() {
        // BLI_halton_2d({2, 3}, {0, 0}, n): the radical inverses of n in bases 2 and 3.
        assert_eq!(halton([2, 3], 1), [0.5, 1.0 / 3.0]);
        let h = halton([2, 3], 2);
        assert!((h[0] - 0.25).abs() < 1e-12 && (h[1] - 2.0 / 3.0).abs() < 1e-12);
        let h = halton([2, 3], 3);
        assert!((h[0] - 0.75).abs() < 1e-12 && (h[1] - 1.0 / 9.0).abs() < 1e-12);
        let h = halton([5], 6);
        assert!((h[0] - 0.04 - 0.2).abs() < 1e-12, "{h:?}");
    }

    #[test]
    fn first_sample_is_centred() {
        // The WORKAROUND offsets of `Sampling::step()` make the first sample the pixel centre
        // and the light centre.
        let s = Sample::new(0);
        assert_eq!(s.filter, [0.0, 0.0]);
        assert_eq!(s.pixel_offset(1.5), [-0.5, -0.5]);
        let s1 = Sample::new(1);
        assert!(s1.filter.iter().all(|v| (0.0..1.0).contains(v)));
        assert_eq!(samples(0).len(), 1);
        assert_eq!(samples(64).len(), 64);
    }

    #[test]
    fn jitter_is_stratified() {
        // 64 samples cover the pixel evenly: every quadrant gets about a quarter of them (the
        // base-3 dimension is shifted by 2/3, so the split is 17/15).
        let s = samples(64);
        let mut q = [0; 4];
        for x in &s {
            let o = x.pixel_offset(1.5);
            q[usize::from(o[0] >= 0.0) + 2 * usize::from(o[1] >= 0.0)] += 1;
        }
        assert!(q.iter().all(|n| (15..=17).contains(n)), "{q:?}");
        let small = Sample::new(5).pixel_offset(0.5);
        assert!(small[0].hypot(small[1]) <= 0.25 + 1e-6);
    }

    #[test]
    fn jitter_projection_shifts_by_pixels() {
        let p = crate::eevee::shadows::orthographic(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0);
        let j = jitter_projection(p, [0.5, 0.25], [100, 50]);
        let c = j * glam::Vec4::new(0.0, 0.0, -1.0, 1.0);
        // Half a pixel right in the sample = the image moves half a pixel left.
        assert!((c.x / c.w + 0.01).abs() < 1e-6);
        assert!((c.y / c.w - 0.01).abs() < 1e-6);
    }

    #[test]
    fn shape_samples_stay_on_the_shape() {
        for s in samples(32) {
            let b = sample_ball(Vec3::from(s.shadow));
            assert!(b.length() <= 1.0 + 1e-5);
            let c = sample_uniform_cone(Vec2::new(s.shadow[0], s.shadow[1]), 0.9);
            assert!(c.z >= 0.9 - 1e-5 && (c.length() - 1.0).abs() < 1e-5);
        }
    }
}
