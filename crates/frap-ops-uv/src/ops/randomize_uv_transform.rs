// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.randomize_uv_transform(random_seed=0, use_loc=True, loc=(0, 0), use_rot=True,
//! rot=0, use_scale=True, scale_even=False, scale=(1, 1))`, tier T1
//! (`tests/golden/cases/uv/edit_m8/randomize.py`). Port of the Python operator
//! `RandomizeUVTransform` (`get_random_transform`, `randomize_uv_transform_island`,
//! `randomize_uv_transform_bmesh`; `scripts/startup/bl_operators/uvcalc_transform.py`),
//! Blender 5.2.4, GPL-2.0-or-later, with CPython's `random` ([`mt`]).
//!
//! Poll `context.mode == 'EDIT_MESH'`. Every UV island (`bmesh_linked_uv_islands`) with a
//! selected face of every mesh in Edit Mode with a UV map gets a random offset (up to `loc`),
//! rotation (up to `rot`) and scale (between `2 - scale` and `scale` per axis, or the U one for
//! both with `scale_even`) about its bounds centre; the generator is seeded with
//! `random_seed` plus the island's lowest face index, so an island keeps its transform
//! whatever else is selected. Always finishes.

pub(crate) mod mt;

use frap_bmesh::FaceId;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::align_rotation::py;
use super::weld::common;
use crate::select::{UvSel, op};
use mt::PyRandom;

/// `transform_params` (`[seed, loc, rot, scale, scale_even]`) as Python floats.
#[derive(Copy, Clone, Debug)]
struct Params {
    seed: i64,
    loc: [f64; 2],
    rot: f64,
    scale: Option<[f64; 2]>,
    scale_even: bool,
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::RandomizeUvTransform) -> OpResult {
    let params = Params {
        seed: i64::from(p.random_seed.value),
        loc: if p.use_loc.value {
            p.loc.value.map(f64::from)
        } else {
            [0.0, 0.0]
        },
        rot: if p.use_rot.value {
            f64::from(p.rot.value)
        } else {
            0.0
        },
        scale: p.use_scale.value.then(|| p.scale.value.map(f64::from)),
        scale_even: p.scale_even.value,
    };
    let ts = op::ts(cx);
    for (_, me) in common::objects(cx) {
        common::edit(cx, me, ts, |s| {
            let fidx = common::face_index(s);
            let mut changed = false;
            for island in py::linked_uv_islands(s) {
                if py::island_uv_selected(s, &island, false) {
                    randomize_island(s, &island, &fidx, params);
                    changed = true;
                }
            }
            ((), changed)
        })?;
    }
    Ok(OpStatus::Finished)
}

/// `get_random_transform`: the 2x3 transform in homogeneous coordinates.
fn random_transform(p: Params, entropy: i64) -> [[f64; 3]; 2] {
    let mut rng = PyRandom::seeded(p.seed + entropy);
    let mut offset_u = rng.uniform(0.0, 1.0);
    let mut offset_v = rng.uniform(0.0, 1.0);
    let mut angle = rng.uniform(0.0, 1.0);
    let mut scale_u = rng.uniform(0.0, 1.0);
    let mut scale_v = rng.uniform(0.0, 1.0);
    // `if loc:` is always true (a two-item sequence).
    offset_u *= p.loc[0];
    offset_v *= p.loc[1];
    if p.rot != 0.0 {
        angle *= p.rot;
    } else {
        angle = 0.0;
    }
    match p.scale {
        Some(scale) => {
            scale_u = scale_u * (2.0 * scale[0] - 2.0) + 2.0 - scale[0];
            scale_v = scale_v * (2.0 * scale[1] - 2.0) + 2.0 - scale[1];
        }
        None => {
            scale_u = 1.0;
            scale_v = 1.0;
        }
    }
    if p.scale_even {
        scale_v = scale_u;
    }
    let (c, s) = (libm::cos(angle), libm::sin(angle));
    [
        [scale_u * c, -scale_v * s, offset_u],
        [scale_u * s, scale_v * c, offset_v],
    ]
}

/// `randomize_uv_transform_island`.
fn randomize_island(s: &mut UvSel, faces: &[FaceId], fidx: &[u32], p: Params) {
    let entropy = faces
        .iter()
        .map(|f| i64::from(fidx[f.slot()]))
        .min()
        .unwrap_or(0);
    let t = random_transform(p, entropy);
    let (mid_u, mid_v) = py::island_uv_bounds_center(s, faces);
    let del_u = t[0][2] + mid_u - t[0][0] * mid_u - t[0][1] * mid_v;
    let del_v = t[1][2] + mid_v - t[1][0] * mid_u - t[1][1] * mid_v;
    for &f in faces {
        for l in s.face_loops(f) {
            let pre = s.uv(l);
            let (pu, pv) = (f64::from(pre[0]), f64::from(pre[1]));
            let u = t[0][0] * pu + t[0][1] * pv + del_u;
            let v = t[1][0] * pu + t[1][1] * pv + del_v;
            s.uv[l.slot()] = [u as f32, v as f32];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_identity() {
        let p = Params {
            seed: 0,
            loc: [0.0, 0.0],
            rot: 0.0,
            scale: Some([1.0, 1.0]),
            scale_even: false,
        };
        assert_eq!(random_transform(p, 3), [[1.0, -0.0, 0.0], [0.0, 1.0, 0.0]]);
    }
}
