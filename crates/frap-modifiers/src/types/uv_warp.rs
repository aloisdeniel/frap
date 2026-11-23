// SPDX-License-Identifier: GPL-3.0-or-later
//! UV_WARP (`UVWarpModifier`): transforms a UV map by the offset between two objects (or
//! bones, `object_from`/`object_to`, projected on `axis_u`/`axis_v`) and a direct 2D
//! transform (`offset`, `scale`, `rotation` about `center`), weighted per corner by
//! `vertex_group`.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_uvwarp.cc (Blender 5.2.4,
//! GPL-2.0-or-later) and the `BLI_math` calls it makes (`translate_m4`, `rescale_m4`,
//! `rotate_m4`, `shuffle_m4`, blenlib/intern/math_matrix_c.cc).

use frap_mesh::{AttrData, Domain};
use frap_rna::IdAny;
use frap_scene::generated::modifier_settings::UVWarpSettings;
use smallvec::SmallVec;

use super::warp::shared::{inverted, obj_pchan_world, vgroup, vgroup_weight};
use super::weld::util::{M4, float2_mut, mul_m4_m4m4, mul_m4_v3, unit_m4, uv_map_names};
use crate::prelude::*;

fn uv_warp_deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::UvWarp(w) = s {
        v.extend(w.object_from);
        v.extend(w.object_to);
    }
    v
}

#[modifier(
    UV_WARP,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    depends_on = uv_warp_deps,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::UvWarp(s) = &md.settings else {
        return Ok(mesh);
    };
    let objects = match (s.object_from, s.object_to) {
        (Some(_), Some(_)) => obj_pchan_world(cx, s.object_from, &s.bone_from)
            .zip(obj_pchan_world(cx, s.object_to, &s.bone_to)),
        _ => None,
    };
    uv_warp(s, objects, &mut mesh);
    Ok(mesh)
}

/// `axis_u`/`axis_v` as an index.
fn axis_index(a: &str) -> usize {
    match a {
        "Y" => 1,
        "Z" => 2,
        _ => 0,
    }
}

/// `translate_m4`
#[allow(clippy::needless_range_loop)] // Blender's index loops
fn translate_m4(m: &mut M4, tx: f32, ty: f32, tz: f32) {
    for k in 0..3 {
        m[3][k] += tx * m[0][k] + ty * m[1][k] + tz * m[2][k];
    }
}

/// `rescale_m4`
#[allow(clippy::needless_range_loop)] // Blender's index loops
fn rescale_m4(m: &mut M4, scale: [f32; 3]) {
    for (c, s) in scale.iter().enumerate() {
        for k in 0..3 {
            m[c][k] *= s;
        }
    }
}

/// `rotate_m4(mat, 'Z', angle)`
#[allow(clippy::needless_range_loop)] // Blender's index loops
fn rotate_m4_z(m: &mut M4, angle: f32) {
    let (c, s) = (libm::cosf(angle), libm::sinf(angle));
    for col in 0..4 {
        let temp = c * m[0][col] + s * m[1][col];
        m[1][col] = -s * m[0][col] + c * m[1][col];
        m[0][col] = temp;
    }
}

/// `shuffle_m4`
fn shuffle_m4(index: [i32; 4]) -> M4 {
    let mut r = [[0.0f32; 4]; 4];
    for (k, &i) in index.iter().enumerate() {
        if i >= 0 {
            r[i as usize][k] = 1.0;
        }
    }
    r
}

/// `transpose_m4`
fn transpose_m4(m: &M4) -> M4 {
    let mut r = [[0.0f32; 4]; 4];
    for (i, ri) in r.iter_mut().enumerate() {
        for (j, rij) in ri.iter_mut().enumerate() {
            *rij = m[j][i];
        }
    }
    r
}

// Ported from blender/source/blender/modifiers/intern/MOD_uvwarp.cc (modify_mesh,
// uv_warp_compute).
/// Warps the modifier's UV map of `mesh` in place. `objects` are the world matrices of
/// `object_from` and `object_to` (bones applied) when both are set.
pub(crate) fn uv_warp(s: &UVWarpSettings, objects: Option<(M4, M4)>, mesh: &mut Mesh) {
    let names = uv_map_names(mesh);
    if names.is_empty() {
        return;
    }
    let axis_u = axis_index(&s.axis_u);
    let axis_v = axis_index(&s.axis_v);
    let mut warp_mat = match objects {
        Some((mat_src, mat_dst)) => {
            let imat_dst = inverted(&mat_dst);
            let mut warp_mat = mul_m4_m4m4(&imat_dst, &mat_src);
            if s.center != [0.0, 0.0] {
                let mut mat_cent = unit_m4();
                mat_cent[3][axis_u] = s.center[0];
                mat_cent[3][axis_v] = s.center[1];
                let imat_cent = inverted(&mat_cent);
                warp_mat = mul_m4_m4m4(&warp_mat, &imat_cent);
                warp_mat = mul_m4_m4m4(&mat_cent, &warp_mat);
            }
            let shuf_mat = shuffle_m4([axis_u as i32, axis_v as i32, -1, 3]);
            warp_mat = mul_m4_m4m4(&shuf_mat, &warp_mat);
            mul_m4_m4m4(&warp_mat, &transpose_m4(&shuf_mat))
        }
        None => unit_m4(),
    };
    // The direct 2D transform.
    translate_m4(&mut warp_mat, s.center[0], s.center[1], 0.0);
    rescale_m4(&mut warp_mat, [s.scale[0], s.scale[1], 1.0]);
    rotate_m4_z(&mut warp_mat, s.rotation);
    translate_m4(&mut warp_mat, s.offset[0], s.offset[1], 0.0);
    translate_m4(&mut warp_mat, -s.center[0], -s.center[1], 0.0);

    // An existing layer: the named one, else the active one.
    let uvname = if names.iter().any(|n| n == s.uv_layer.as_str()) {
        s.uv_layer.as_str().into()
    } else {
        mesh.uv_active.clone().unwrap_or_else(|| names[0].clone())
    };
    let (defgrp, dvert) = vgroup(mesh, &s.vertex_group);
    let dvert = defgrp.zip(dvert.map(<[_]>::to_vec));
    let corner_verts = mesh.corner_verts().to_vec();
    let is_uv = matches!(
        mesh.attributes().get(&uvname).map(|a| (&a.domain, &a.data)),
        Some((Domain::Corner, AttrData::Float2(_)))
    );
    if !is_uv {
        return;
    }
    let Some(uvs) = float2_mut(mesh, &uvname) else {
        return;
    };
    let warp = |uv: [f32; 2]| {
        let t = mul_m4_v3(&warp_mat, [uv[0], uv[1], 0.0]);
        [t[0], t[1]]
    };
    for (c, uv) in uvs.iter_mut().enumerate() {
        match &dvert {
            Some((g, dv)) => {
                let w = vgroup_weight(dv, corner_verts[c] as usize, *g, s.invert_vertex_group);
                let t = warp(*uv);
                // `interp_v2_v2v2`
                let k = 1.0 - w;
                *uv = [k * uv[0] + w * t[0], k * uv[1] + w * t[1]];
            }
            None => *uv = warp(*uv),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plane() -> Mesh {
        let mut m = frap_mesh::primitives::grid::grid(&frap_mesh::primitives::grid::Grid {
            x_subdivisions: 1,
            y_subdivisions: 1,
            size: 2.0,
            calc_uvs: true,
            ..Default::default()
        });
        if m.uv_active.is_none() {
            m.uv_active = uv_map_names(&m).first().cloned();
        }
        m
    }

    #[test]
    fn offset_and_scale_about_the_centre() {
        let mut m = plane();
        let before = m.uv_maps()[0].1.to_vec();
        let s = UVWarpSettings {
            offset: [0.25, 0.0],
            scale: [2.0, 1.0],
            ..UVWarpSettings::default()
        };
        uv_warp(&s, None, &mut m);
        for (a, b) in m.uv_maps()[0].1.iter().zip(&before) {
            assert!(
                (a[0] - ((b[0] + 0.25 - 0.5) * 2.0 + 0.5)).abs() < 1e-6,
                "{a:?} {b:?}"
            );
            assert!((a[1] - b[1]).abs() < 1e-6);
        }
    }

    #[test]
    fn no_uv_map_is_a_no_op() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        let before = m.clone();
        uv_warp(&UVWarpSettings::default(), None, &mut m);
        assert_eq!(m.positions(), before.positions());
    }
}
