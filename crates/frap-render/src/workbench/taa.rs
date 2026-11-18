// SPDX-License-Identifier: GPL-3.0-or-later
//! Workbench anti-aliasing for final renders (`scene.display.render_aa`): the jittered
//! sample offsets, the per-sample 3 x 3 Blackman-Harris reconstruction weights and the
//! projection jitter.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later):
//! - `source/blender/draw/engines/workbench/workbench_effect_antialiasing.cc`
//!   (`TaaSamples::init_samples`, `filter_blackman_harris`, `setup_taa_weights`,
//!   `AntiAliasingPass::setup_view`);
//! - `source/blender/blenlib/intern/jitter_2d.cc` (`BLI_jitter_init`, `BLI_jitterate1`,
//!   `BLI_jitterate2`) on `BLI_rng` ([`frap_math::BliRng`]);
//! - `source/blender/blenlib/intern/math_geom.cc` (`window_translate_m4`).
//!
//! The accumulation itself (`workbench_effect_taa_frag.glsl`: neighbours in `log2(c + 1)`
//! space, half-float targets) and the final division (`workbench_effect_smaa_frag.glsl`
//! stage 2, `exp2(c) - 1`) are in `workbench.wgsl` and [`super::resolve_taa`].

use frap_math::BliRng;
use glam::{Mat4, Vec3};

/// `render_aa` → `(draw_aa, samples_len)` (`SceneState::init`): `OFF` = 0 samples (no
/// anti-aliasing), `FXAA` = 1, else the number. Unknown values read as the factory `'8'`.
pub fn samples_from_render_aa(render_aa: &str) -> (bool, u32) {
    let n: u32 = match render_aa {
        "OFF" => 0,
        "FXAA" => 1,
        other => other.parse().unwrap_or(8),
    };
    (n > 0, n.max(1))
}

/// `BLI_jitterate1`
fn jitterate1(jit1: &mut [[f32; 2]], jit2: &mut [[f32; 2]], radius1: f32) {
    let num = jit1.len();
    for i in (0..num).rev() {
        let (mut dvecx, mut dvecy) = (0.0f32, 0.0f32);
        let mut x = jit1[i][0];
        let mut y = jit1[i][1];
        for j in (0..num).rev() {
            if i == j {
                continue;
            }
            let mut vecx = jit1[j][0] - x - 1.0;
            let mut vecy = jit1[j][1] - y - 1.0;
            for _k in 0..3 {
                for step in 0..3 {
                    if vecx.abs() < radius1 && vecy.abs() < radius1 {
                        let mut len = (vecx * vecx + vecy * vecy).sqrt();
                        if len > 0.0 && len < radius1 {
                            len /= radius1;
                            dvecx += vecx / len;
                            dvecy += vecy / len;
                        }
                    }
                    if step < 2 {
                        vecx += 1.0;
                    }
                }
                vecx -= 2.0;
                vecy += 1.0;
            }
        }
        x -= dvecx / 18.0;
        y -= dvecy / 18.0;
        x -= x.floor();
        y -= y.floor();
        jit2[i] = [x, y];
    }
    jit1.copy_from_slice(jit2);
}

/// `BLI_jitterate2`
fn jitterate2(jit1: &mut [[f32; 2]], jit2: &mut [[f32; 2]], radius2: f32) {
    let num = jit1.len();
    for i in (0..num).rev() {
        let (mut dvecx, mut dvecy) = (0.0f32, 0.0f32);
        let mut x = jit1[i][0];
        let mut y = jit1[i][1];
        for j in (0..num).rev() {
            if i == j {
                continue;
            }
            let mut vecx = jit1[j][0] - x - 1.0;
            let mut vecy = jit1[j][1] - y - 1.0;
            for step in 0..3 {
                if vecx.abs() < radius2 {
                    dvecx += vecx * radius2;
                }
                if step < 2 {
                    vecx += 1.0;
                }
            }
            for step in 0..3 {
                if vecy.abs() < radius2 {
                    dvecy += vecy * radius2;
                }
                if step < 2 {
                    vecy += 1.0;
                }
            }
        }
        x -= dvecx / 2.0;
        y -= dvecy / 2.0;
        x -= x.floor();
        y -= y.floor();
        jit2[i] = [x, y];
    }
    jit1.copy_from_slice(jit2);
}

/// `BLI_jitter_init(jitarr, num)`: `num` stratified offsets centred on `(0, 0)`.
pub fn jitter_init(num: usize) -> Vec<[f32; 2]> {
    if num == 0 {
        return vec![];
    }
    let number_fl = num as f32;
    let number_fl_sqrt = number_fl.sqrt();
    let rad1 = 1.0 / number_fl_sqrt;
    let rad2 = 1.0 / number_fl;
    let rad3 = number_fl_sqrt / number_fl;
    let mut rng = BliRng::new(31_415_926u32.wrapping_add(num as u32));
    let mut jit = vec![[0.0f32; 2]; num];
    let mut x = 0.0f32;
    for (i, j) in jit.iter_mut().enumerate() {
        j[0] = x + rad1 * (0.5 - rng.get_double()) as f32;
        j[1] = i as f32 / number_fl + rad1 * (0.5 - rng.get_double()) as f32;
        x += rad3;
        x -= x.floor();
    }
    let mut jit2 = vec![[0.0f32; 2]; num];
    for _ in 0..24 {
        jitterate1(&mut jit, &mut jit2, rad1);
        jitterate1(&mut jit, &mut jit2, rad1);
        jitterate2(&mut jit, &mut jit2, rad2);
    }
    for j in &mut jit {
        j[0] -= 0.5;
        j[1] -= 0.5;
    }
    jit
}

/// `TaaSamples::init_samples`: the jitter re-centred on its sample closest to the centre,
/// wrapped into `[-1, 1)`, that sample first, then each next sample the farthest from the
/// previous one.
pub fn taa_samples(num: usize) -> Vec<[f32; 2]> {
    let mut samples = jitter_init(num);
    if samples.is_empty() {
        return samples;
    }
    let mut closest_index = 0;
    let mut closest_squared_distance = 1.0f32;
    for (i, s) in samples.iter().enumerate() {
        let d = s[0] * s[0] + s[1] * s[1];
        if d < closest_squared_distance {
            closest_squared_distance = d;
            closest_index = i;
        }
    }
    let closest = samples[closest_index];
    for s in &mut samples {
        let x = s[0] - closest[0];
        let y = s[1] - closest[1];
        // C `fmodf` keeps the sign of the dividend.
        let x = (x + 0.5) % 1.0;
        let y = (y + 0.5) % 1.0;
        *s = [x * 2.0 - 1.0, y * 2.0 - 1.0];
    }
    if closest_index != 0 {
        samples.swap(0, closest_index);
    }
    let n = samples.len();
    for i in 0..n.saturating_sub(2) {
        let mut squared_dist = 0.0f32;
        let mut index = i;
        for j in i + 1..n {
            let dx = samples[i][0] - samples[j][0];
            let dy = samples[i][1] - samples[j][1];
            let d = dx * dx + dy * dy;
            if d > squared_dist {
                squared_dist = d;
                index = j;
            }
        }
        samples.swap(i + 1, index);
    }
    samples
}

/// The offset of sample `sample` of a `samples_len`-sample render
/// (`AntiAliasingPass::setup_view`: tables of 5, 8, 11, 16 and 32 samples; any other count
/// uses the 5-sample table, as Blender's `default:` case does).
pub fn sample_offset(samples_len: u32, sample: u32) -> [f32; 2] {
    let table = match samples_len {
        8 => 8,
        11 => 11,
        16 => 16,
        32 => 32,
        _ => 5,
    };
    let s = taa_samples(table);
    s.get(sample as usize).copied().unwrap_or([0.0, 0.0])
}

/// `filter_blackman_harris(x, width)`
pub fn filter_blackman_harris(x: f32, width: f32) -> f32 {
    if x > width * 0.5 {
        return 0.0;
    }
    let x = 2.0 * std::f32::consts::PI * (x / width + 0.5).clamp(0.0, 1.0);
    0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos() - 0.01168 * (3.0 * x).cos()
}

/// `setup_taa_weights(offset)`: the 3 x 3 neighbourhood weights (index `(x + 1) * 3 + (y +
/// 1)`, `x`/`y` in Blender's bottom-up texel space) and their sum.
pub fn taa_weights(offset: [f32; 2]) -> ([f32; 9], f32) {
    let filter_width = 2.0;
    let mut w = [0.0f32; 9];
    let mut sum = 0.0f32;
    let mut i = 0;
    for x in -1..=1 {
        for y in -1..=1 {
            let sx = x as f32 - offset[0];
            let sy = y as f32 - offset[1];
            let r = (sx * sx + sy * sy).sqrt();
            let weight = filter_blackman_harris(r, filter_width);
            sum += weight;
            w[i] = weight;
            i += 1;
        }
    }
    (w, sum)
}

/// The clip-space translation `window_translate_m4(winmat, persmat, x, y)` amounts to, with
/// `x = offset.x / width` and `y = offset.y / height` (`setup_view`): an NDC shift, applied by
/// pre-multiplying the projection. `gl_winmat` is Blender's window matrix (OpenGL depth),
/// `view` the world → view matrix.
pub fn jitter_matrix(gl_winmat: &Mat4, view: &Mat4, offset: [f32; 2], size: [u32; 2]) -> Mat4 {
    let x = offset[0] / size[0] as f32;
    let y = offset[1] / size[1] as f32;
    let w = gl_winmat.to_cols_array_2d();
    let (dx, dy) = if w[2][3] == -1.0 {
        // Perspective: `winmat[2][0] -= len1 * winmat[0][0] * x`, where `len1` is the inverse
        // length of the first row of `persmat`'s 3 x 3 part; the NDC shift is that term.
        let persmat = (*gl_winmat * *view).to_cols_array_2d();
        let v1 = Vec3::new(persmat[0][0], persmat[1][0], persmat[2][0]);
        let v2 = Vec3::new(persmat[0][1], persmat[1][1], persmat[2][1]);
        let len1 = 1.0 / v1.length();
        let len2 = 1.0 / v2.length();
        (len1 * w[0][0] * x, len2 * w[1][1] * y)
    } else {
        // Orthographic: `winmat[3][0] += x`.
        (x, y)
    };
    Mat4::from_translation(Vec3::new(dx, dy, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_aa_values() {
        assert_eq!(samples_from_render_aa("OFF"), (false, 1));
        assert_eq!(samples_from_render_aa("FXAA"), (true, 1));
        assert_eq!(samples_from_render_aa("8"), (true, 8));
        assert_eq!(samples_from_render_aa("32"), (true, 32));
    }

    #[test]
    fn samples_start_centred_and_stay_in_range() {
        for n in [5usize, 8, 11, 16, 32] {
            let s = taa_samples(n);
            assert_eq!(s.len(), n);
            assert_eq!(
                s[0],
                [-1.0 + 1.0, -1.0 + 1.0],
                "{n}: first sample is the centre"
            );
            // C `fmodf` keeps the dividend's sign, so Blender's "wrap around" leaves offsets
            // re-centred below -0.5 in (-2, -1): kept as Blender computes them.
            for p in &s {
                assert!(
                    (-2.0..1.0).contains(&p[0]) && (-2.0..1.0).contains(&p[1]),
                    "{p:?}"
                );
            }
            // Deterministic.
            assert_eq!(taa_samples(n), s);
        }
    }

    #[test]
    fn weights_peak_at_the_offset() {
        let (w, sum) = taa_weights([0.0, 0.0]);
        // Centre weight: BH(0) = 1.0 (Blackman-Harris peak).
        assert!((w[4] - 1.0).abs() < 1e-5, "{}", w[4]);
        assert!(w.iter().all(|&x| x <= w[4]));
        assert!((w.iter().sum::<f32>() - sum).abs() < 1e-6);
        // Diagonal neighbours are beyond the 1-pixel radius of a width-2 filter.
        assert_eq!(w[0], 0.0);
        let (w2, _) = taa_weights([0.5, 0.0]);
        assert!(w2[7] > w2[1], "right neighbour weighs more for a +x offset");
    }

    #[test]
    fn ortho_jitter_is_a_plain_ndc_shift() {
        let ortho =
            frap_render_scene::build::camera::orthographic_m4(-1.0, 1.0, -1.0, 1.0, 0.1, 10.0);
        let m = jitter_matrix(&ortho, &Mat4::IDENTITY, [0.5, -0.25], [100, 50]);
        assert_eq!(m.w_axis.x, 0.005);
        assert_eq!(m.w_axis.y, -0.005);
        let persp =
            frap_render_scene::build::camera::perspective_m4(-0.1, 0.1, -0.05, 0.05, 0.1, 10.0);
        let m = jitter_matrix(&persp, &Mat4::IDENTITY, [1.0, 1.0], [100, 100]);
        assert!((m.w_axis.x - 0.01).abs() < 1e-6);
    }
}
