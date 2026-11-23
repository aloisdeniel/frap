// SPDX-License-Identifier: GPL-3.0-or-later
//! WAVE (`WaveModifier`): a Gaussian ripple travelling from `start_position_x/y` (or
//! `start_position_object`) along X, Y or both (circles), moving the vertices along local Z
//! or their normals (`use_normal_x/y/z`); time-dependent (the scene frame, `time_offset`,
//! `speed`, `lifetime`, `damping_time`), with `falloff_radius`, `vertex_group` and an optional
//! texture.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_wave.cc (Blender 5.2.4,
//! GPL-2.0-or-later).

use frap_rna::IdAny;
use frap_scene::generated::modifier_settings::WaveSettings;
use smallvec::SmallVec;

use super::warp::shared::{
    Mapping, ModTexture, inverted, object_world, texture_coords, vgroup, vgroup_weight,
};
use super::weld::util::{m4_from_glam, mul_m4_m4m4};
use crate::prelude::*;

fn wave_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Wave(w) = s {
        v.extend(w.start_position_object);
        v.extend(w.texture);
        v.extend(w.texture_coords_object);
    }
    v
}

#[modifier(
    WAVE,
    kind = OnlyDeform,
    flags = ACCEPTS_CVS | ACCEPTS_VERTEX_COS_ONLY | SUPPORTS_EDITMODE,
    depends_on = wave_deps,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Wave(s) = &md.settings else {
        return Ok(mesh);
    };
    let mut start = [s.start_position_x, s.start_position_y];
    if let Some(center) = object_world(cx, s.start_position_object) {
        // The control object's location in local coordinates.
        let world_to_object = inverted(&m4_from_glam(cx.object_matrix_world(cx.object_ref())));
        let mat = mul_m4_m4m4(&world_to_object, &center);
        start = [mat[3][0], mat[3][1]];
    }
    let texture = ModTexture::load(cx, s.texture, "WAVE")?;
    let tex_co = match &texture {
        Some(_) => {
            let m = Mapping {
                texture_coords: &s.texture_coords,
                uv_layer: &s.uv_layer,
                object: s.texture_coords_object,
                bone: &s.texture_coords_bone,
            };
            let cos = mesh.positions().to_vec();
            Some(texture_coords(cx, &m, &mesh, &cos))
        }
        None => None,
    };
    wave(
        s,
        cx.frame(),
        start,
        texture.as_ref().zip(tex_co.as_deref()),
        &mut mesh,
    );
    Ok(mesh)
}

// Ported from blender/source/blender/modifiers/intern/MOD_wave.cc (waveModifier_do).
/// Deforms `mesh` at scene time `ctime` (`DEG_get_ctime`). `start` is the wave's origin in
/// local X/Y.
pub(crate) fn wave(
    s: &WaveSettings,
    ctime: f32,
    start: [f32; 2],
    texture: Option<(&ModTexture, &[[f32; 3]])>,
    mesh: &mut Mesh,
) {
    let minfac =
        (1.0 / libm::exp(f64::from(s.width * s.narrowness * s.width * s.narrowness))) as f32;
    let mut lifefac = s.height;
    let falloff = s.falloff_radius;
    let mut falloff_fac = 1.0f32;
    let invert_group = s.invert_vertex_group;
    let axis = (s.use_x, s.use_y);

    let vert_normals = s.use_normal.then(|| mesh.vert_normals());
    let (defgrp, dvert) = vgroup(mesh, &s.vertex_group);
    let dvert = dvert.map(<[_]>::to_vec);

    let damp = if s.damping_time == 0.0 {
        10.0
    } else {
        s.damping_time
    };
    if s.lifetime != 0.0 {
        let x = ctime - s.time_offset;
        if x > s.lifetime {
            lifefac = x - s.lifetime;
            lifefac = if lifefac > damp {
                0.0
            } else {
                s.height * (1.0 - (lifefac / damp).sqrt())
            };
        }
    }
    if lifefac == 0.0 {
        return;
    }
    // Avoid divide-by-zero checks within the loop.
    let falloff_inv = if falloff != 0.0 { 1.0 / falloff } else { 1.0 };
    let positions = mesh.positions_mut();
    for (i, co) in positions.iter_mut().enumerate() {
        let x = co[0] - start[0];
        let y = co[1] - start[1];
        let mut amplit = 0.0f32;
        let mut def_weight = 1.0f32;
        if let (Some(g), Some(dv)) = (defgrp, &dvert) {
            def_weight = vgroup_weight(dv, i, g, invert_group);
            // A vertex outside the group is not deformed.
            if def_weight == 0.0 {
                continue;
            }
        }
        match axis {
            (true, true) => amplit = (x * x + y * y).sqrt(),
            (true, false) => amplit = x,
            (false, true) => amplit = y,
            (false, false) => {}
        }
        // This way it makes nice circles.
        amplit -= (ctime - s.time_offset) * s.speed;
        if s.use_cyclic {
            amplit = libm::fmodf(amplit - s.width, 2.0 * s.width) + s.width;
        }
        if falloff != 0.0 {
            let dist = match axis {
                (true, true) => (x * x + y * y).sqrt(),
                (true, false) => x.abs(),
                (false, true) => y.abs(),
                (false, false) => 0.0,
            };
            falloff_fac = (1.0 - dist * falloff_inv).clamp(0.0, 1.0);
        }
        // Gaussian.
        if falloff_fac != 0.0 && amplit > -s.width && amplit < s.width {
            amplit *= s.narrowness;
            amplit = 1.0 / libm::expf(amplit * amplit) - minfac;
            if let Some((tex, tex_co)) = texture {
                amplit *= tex.value(tex_co[i]).tin;
            }
            // Weight and falloff.
            amplit *= def_weight * falloff_fac;
            if let Some(no) = &vert_normals {
                // Along the normals.
                if s.use_normal_x {
                    co[0] += (lifefac * amplit) * no[i][0];
                }
                if s.use_normal_y {
                    co[1] += (lifefac * amplit) * no[i][1];
                }
                if s.use_normal_z {
                    co[2] += (lifefac * amplit) * no[i][2];
                }
            } else {
                // Along local Z.
                co[2] += lifefac * amplit;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crest_is_at_the_start_at_time_zero() {
        let mut m = frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
            x_subdivisions: 4,
            y_subdivisions: 4,
            size: 2.0,
            ..Default::default()
        });
        let s = WaveSettings::default();
        let before = m.positions().to_vec();
        // time_offset 0, speed 0.25: at frame 0 the wave is centred on the start.
        wave(&s, 0.0, [0.0, 0.0], None, &mut m);
        let centre = before
            .iter()
            .position(|p| p[0].abs() < 1e-6 && p[1].abs() < 1e-6)
            .unwrap();
        let wn = s.width * s.narrowness * s.width * s.narrowness;
        let expect = s.height * (1.0 - (1.0 / libm::exp(f64::from(wn))) as f32);
        assert!((m.positions()[centre][2] - expect).abs() < 1e-6);
    }
}
