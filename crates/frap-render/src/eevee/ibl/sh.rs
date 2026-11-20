// SPDX-License-Identifier: GPL-3.0-or-later
//! L1 spherical harmonics of the world's radiance, as EEVEE stores them for diffuse world
//! lighting. Ported from blender/source/blender/draw/engines/eevee/shaders/
//! eevee_spherical_harmonics.bsl.hh (`SphericalHarmonicL1`, `evaluate_lambert`, `dering`).
//!
//! Basis (Blender's, with the Condon-Shortley phase): `Y00 = 0.282094792`,
//! `Y1-1 = -0.488602512 y`, `Y10 = 0.488602512 z`, `Y11 = -0.488602512 x`. Coefficients are
//! projections of the radiance (`Σ L(ω) Y(ω) dω`).
//!
//! `WorldLighting::sh9` is exported in the conventional real basis instead (no Condon-Shortley
//! phase, Ramamoorthi & Hanrahan 2001: `Y1-1 = 0.488603 y`, `Y10 = 0.488603 z`,
//! `Y11 = 0.488603 x`, band 2 in the order `xy, yz, 3z²-1, xz, x²-y²`), in the world's frame
//! ([`ShL1::to_sh9`]); [`sh9_irradiance`] is the consumer's formula. For the L1 band,
//! `sh9_irradiance(N) / π` is exactly EEVEE's `evaluate_lambert(N)`:
//! `π·Y00 = 0.886227` and `π · 2/3 · Y1 = 1.023328`.

/// `L0.M0`
pub const L0_M0: f32 = 0.282_094_8;
/// `|L1|` factor
pub const L1: f32 = 0.488_602_5;

/// L1 spherical harmonics of an RGB signal (`SphericalHarmonicL1<float4>` without the
/// visibility channel).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShL1 {
    /// `L0.M0`
    pub l0: [f32; 3],
    /// `L1.Mn1`
    pub l1n1: [f32; 3],
    /// `L1.M0`
    pub l10: [f32; 3],
    /// `L1.Mp1`
    pub l1p1: [f32; 3],
}

fn basis(d: [f32; 3]) -> [f32; 4] {
    [L0_M0, -L1 * d[1], L1 * d[2], -L1 * d[0]]
}

impl ShL1 {
    /// The four coefficient vectors in storage order (`L0.M0, L1.Mn1, L1.M0, L1.Mp1`).
    pub fn coefs(&self) -> [[f32; 3]; 4] {
        [self.l0, self.l1n1, self.l10, self.l1p1]
    }

    /// From the coefficients in storage order.
    pub fn from_coefs(c: [[f32; 3]; 4]) -> ShL1 {
        ShL1 {
            l0: c[0],
            l1n1: c[1],
            l10: c[2],
            l1p1: c[3],
        }
    }

    /// `encode_signal_sample(direction, amplitude)` (`amplitude` already scaled by the
    /// sample's solid angle).
    pub fn encode_signal_sample(&mut self, direction: [f32; 3], amplitude: [f32; 3]) {
        let b = basis(direction);
        let mut c = self.coefs();
        for (k, coef) in c.iter_mut().enumerate() {
            for i in 0..3 {
                coef[i] += b[k] * amplitude[i];
            }
        }
        *self = ShL1::from_coefs(c);
    }

    /// `evaluate(direction)`: the reconstructed signal, clamped at 0.
    pub fn evaluate(&self, direction: [f32; 3]) -> [f32; 3] {
        self.eval_weighted(direction, 1.0)
    }

    /// `evaluate_lambert(N)`: the Lambertian radiance (irradiance / π) for unit albedo.
    pub fn evaluate_lambert(&self, n: [f32; 3]) -> [f32; 3] {
        self.eval_weighted(n, 2.0 / 3.0)
    }

    fn eval_weighted(&self, d: [f32; 3], l1_weight: f32) -> [f32; 3] {
        let b = basis(d);
        let c = self.coefs();
        std::array::from_fn(|i| {
            let l1 = b[1] * c[1][i] + b[2] * c[2][i] + b[3] * c[3][i];
            (b[0] * c[0][i] + l1 * l1_weight).max(0.0)
        })
    }

    /// `spherical_harmonics::dering`: scales the L1 band down so the Lambertian reconstruction
    /// never rings negative (applied when EEVEE loads the world into the volume probe atlas,
    /// `eevee_lightprobe_volume_load.bsl.hh::load_world`).
    pub fn dering(&self) -> ShL1 {
        let l0_weight = L0_M0;
        let l1_weight = L1 * (2.0 / 3.0) + 0.05;
        let c = self.coefs();
        let fac = (0..3)
            .map(|i| {
                let m0 = (c[0][i].abs() * l0_weight).max(1e-8);
                let len = (c[1][i] * c[1][i] + c[2][i] * c[2][i] + c[3][i] * c[3][i]).sqrt();
                m0 / (len * l1_weight).max(1e-8)
            })
            .fold(f32::INFINITY, f32::min);
        let mut out = *self;
        if fac < 1.0 {
            for v in [&mut out.l1n1, &mut out.l10, &mut out.l1p1] {
                for x in v.iter_mut() {
                    *x *= fac;
                }
            }
        }
        out
    }

    /// The coefficients as the `sh9` of `WorldLighting`: the conventional real basis (entries
    /// `L00, L1-1 (y), L10 (z), L11 (x)`, band 2 zero) of the radiance of a world whose
    /// environment lookup is `Rz(rotation) · direction`, `self` being the harmonics of the
    /// environment. The L1 coefficients form a vector `a` (`L(d) ≈ … + a·d`) per channel; the
    /// world sees `a · Rz(rotation)·d = (Rz(-rotation)·a) · d`.
    pub fn to_sh9(&self, rotation: f32) -> [[f32; 4]; 9] {
        let (s, c) = rotation.sin_cos();
        // Blender's basis → conventional: `L11 = -Mp1`, `L1-1 = -Mn1`, `L10 = M0`.
        let ax = self.l1p1.map(|v| -v);
        let ay = self.l1n1.map(|v| -v);
        // Rz(-rotation)
        let wx: [f32; 3] = std::array::from_fn(|i| c * ax[i] + s * ay[i]);
        let wy: [f32; 3] = std::array::from_fn(|i| -s * ax[i] + c * ay[i]);
        let v4 = |v: [f32; 3]| [v[0], v[1], v[2], 0.0];
        let mut out = [[0.0; 4]; 9];
        out[0] = v4(self.l0);
        out[1] = v4(wy);
        out[2] = v4(self.l10);
        out[3] = v4(wx);
        out
    }

    /// The inverse of [`ShL1::to_sh9`] at rotation 0 (the L1 part of `sh9`, Blender's basis).
    pub fn from_sh9(sh9: &[[f32; 4]; 9]) -> ShL1 {
        let c = |k: usize, sign: f32| [sh9[k][0], sh9[k][1], sh9[k][2]].map(|x| x * sign);
        ShL1::from_coefs([c(0, 1.0), c(1, -1.0), c(2, 1.0), c(3, -1.0)])
    }
}

/// The irradiance `E(N)` of `sh9` (`WorldLighting::sh9`), Ramamoorthi & Hanrahan 2001,
/// clamped at 0: the formula every consumer evaluates (the diffuse world radiance of a closure
/// is `diffuse_color · E(N) / π`; WGSL `ibl_sh9_irradiance`).
pub fn sh9_irradiance(sh9: &[[f32; 4]; 9], n: [f32; 3]) -> [f32; 3] {
    let [x, y, z] = n;
    let w = [
        0.886_227,
        1.023_328 * y,
        1.023_328 * z,
        1.023_328 * x,
        0.858_086 * x * y,
        0.858_086 * y * z,
        0.247_708 * (3.0 * z * z - 1.0),
        0.858_086 * x * z,
        0.429_043 * (x * x - y * y),
    ];
    std::array::from_fn(|i| {
        w.iter()
            .zip(sh9)
            .map(|(w, c)| w * c[i])
            .sum::<f32>()
            .max(0.0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Projects `f` with a fine latitude-longitude quadrature.
    fn project(f: impl Fn([f32; 3]) -> [f32; 3]) -> ShL1 {
        let (nt, np) = (256, 512);
        let mut sh = ShL1::default();
        for i in 0..nt {
            let theta = (i as f64 + 0.5) / nt as f64 * std::f64::consts::PI;
            for j in 0..np {
                let phi = (j as f64 + 0.5) / np as f64 * std::f64::consts::TAU;
                let d = [
                    (theta.sin() * phi.cos()) as f32,
                    (theta.sin() * phi.sin()) as f32,
                    theta.cos() as f32,
                ];
                let w = (theta.sin() * std::f64::consts::PI / nt as f64 * std::f64::consts::TAU
                    / np as f64) as f32;
                let v = f(d);
                sh.encode_signal_sample(d, v.map(|x| x * w));
            }
        }
        sh
    }

    #[test]
    fn constant_world_gives_its_radiance() {
        let sh = project(|_| [0.5, 1.0, 2.0]);
        for n in [[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]] {
            let e = sh.evaluate_lambert(n);
            for (a, b) in e.iter().zip([0.5, 1.0, 2.0]) {
                assert!((a - b).abs() < 1e-3 * b, "{e:?}");
            }
        }
        assert!(
            sh.l1n1
                .iter()
                .chain(&sh.l10)
                .chain(&sh.l1p1)
                .all(|x| x.abs() < 1e-4)
        );
        assert_eq!(sh.dering(), sh);
    }

    #[test]
    fn sky_from_above_lights_up_facing_normals() {
        // Radiance 1 from the upper hemisphere only: irradiance π at the zenith, 0 at the
        // nadir. L1 Lambert reconstruction: 1/2 + 2/3 · 3/4 · ... — exact values of the
        // truncated expansion: 0.5 ± 0.5.
        let sh = project(|d| if d[2] > 0.0 { [1.0; 3] } else { [0.0; 3] });
        let up = sh.evaluate_lambert([0.0, 0.0, 1.0])[0];
        let side = sh.evaluate_lambert([1.0, 0.0, 0.0])[0];
        let down = sh.evaluate_lambert([0.0, 0.0, -1.0])[0];
        assert!((up - 1.0).abs() < 2e-3, "{up}");
        assert!((side - 0.5).abs() < 2e-3, "{side}");
        assert!(down.abs() < 2e-3, "{down}");
        // Dering's bias (+0.05 on the L1 weight) trims even this ring-free signal:
        // fac = (π^½ · Y00) / (π · Y10 · (2/3 · Y10 + 0.05)) ≈ 0.867.
        let d = sh.dering();
        let fac = d.l10[0] / sh.l10[0];
        assert!((fac - 0.8670).abs() < 2e-3, "{fac}");
        assert_eq!(d.l0, sh.l0);
    }

    #[test]
    fn dering_bounds_a_directional_world() {
        // A tiny bright source: the raw L1 reconstruction rings negative opposite to it.
        let sh = project(|d| if d[0] > 0.995 { [100.0; 3] } else { [0.0; 3] });
        assert!(
            sh.l0[0] * L0_M0 - (sh.l1p1[0] * L1).abs() * (2.0 / 3.0) < 0.0,
            "rings before dering"
        );
        let d = sh.dering();
        let len = (d.l1n1[0].powi(2) + d.l10[0].powi(2) + d.l1p1[0].powi(2)).sqrt();
        assert!(
            (d.l0[0] * L0_M0 - len * (L1 * 2.0 / 3.0 + 0.05)).abs() < 1e-3 * d.l0[0],
            "dering scales L1 to the limit"
        );
        // Lit side unchanged in direction, dark side non-negative.
        assert!(d.evaluate_lambert([1.0, 0.0, 0.0])[0] > d.evaluate_lambert([-1.0, 0.0, 0.0])[0]);
        assert!(d.evaluate_lambert([-1.0, 0.0, 0.0])[0] >= 0.0);
        let s9 = d.to_sh9(0.0);
        assert_eq!(s9[3][0], -d.l1p1[0]);
        assert!(s9[4..].iter().all(|c| *c == [0.0; 4]));
        assert_eq!(ShL1::from_sh9(&s9), d);
    }

    #[test]
    fn sh9_is_the_conventional_basis_in_the_world_frame() {
        // An environment brighter towards +X (Blender's basis: `Mp1` negative).
        let env = project(|d| [1.0 + d[0].max(0.0) * 3.0, 1.0, 1.0 + d[1].max(0.0)]);
        assert!(env.l1p1[0] < 0.0);
        let pi = std::f32::consts::PI;
        for rotation in [0.0, 0.7, -2.0] {
            let sh9 = env.to_sh9(rotation);
            // Conventional sign: the +X-bright channel has a positive x coefficient at 0.
            if rotation == 0.0 {
                assert!(sh9[3][0] > 0.0 && sh9[1][2] > 0.0, "{sh9:?}");
            }
            for n in [
                [1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
                [0.6, 0.0, 0.8],
                [-0.48, 0.6, -0.64],
            ] {
                // The world at N is the environment at `Rz(rotation) · N`.
                let (s, c) = rotation.sin_cos();
                let lookup = [c * n[0] - s * n[1], s * n[0] + c * n[1], n[2]];
                let want = env.evaluate_lambert(lookup);
                let got = sh9_irradiance(&sh9, n).map(|e| e / pi);
                for i in 0..3 {
                    assert!(
                        (got[i] - want[i]).abs() < 1e-5,
                        "{rotation} {n:?}: {got:?} {want:?}"
                    );
                }
            }
        }
    }
}
