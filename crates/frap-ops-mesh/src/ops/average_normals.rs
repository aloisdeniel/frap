// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.average_normals(average_type='CUSTOM_NORMAL', weight=50, threshold=0.01)`,
//! tier T1 (`tests/golden/cases/edit/normals_m8/average_normals.py`). Port of
//! `edbm_average_normals_exec` (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data) the normal spaces are rebuilt, then every smooth
//! fan at a selected vertex (walked over the smooth manifold edges, ignoring face smoothness)
//! gets one normal for all its corners: the normalised weighted sum of their current custom
//! normals (`CUSTOM_NORMAL`) or of their face normals weighted by face area (`FACE_AREA`) or by
//! corner angle (`CORNER_ANGLE`). The corners are taken by increasing key (`1`, `1 / area` or
//! `1 / angle`, through Blender's `HeapSimple`); a corner counts `key⁻¹ · weight⁻ⁿ` where `n`
//! grows by one each time the key differs from the previous group's by more than `threshold`,
//! and `weight` is `weight / 50` (`(weight / 50 - 1) · 25` above 1.04, 32767 at 100, 1/32767
//! at 1). A corner between two sharp edges gets its face normal. A sum shorter than 1e-4 resets
//! the fan to its automatic normal. The `custom_normal` layer is created when missing. The
//! operator always finishes.

use frap_bmesh::{BMesh, LoopId};
use frap_geom::normals::{V3, normalize_v3, scale};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, average_normals::AverageType};

use super::flip_normals::{clnor, edit_meshes_where};

mod heap;

/// The `weight` factor of `edbm_average_normals_exec`.
fn weight_factor(absweight: f32) -> f32 {
    let mut weight = absweight / 50.0;
    if absweight == 100.0 {
        weight = f32::from(i16::MAX);
    } else if absweight == 1.0 {
        weight = 1.0 / f32::from(i16::MAX);
    } else if (weight - 1.0) * 25.0 > 1.0 {
        weight = (weight - 1.0) * 25.0;
    }
    weight
}

/// `compare_ff`.
fn compare_ff(a: f32, b: f32, max_diff: f32) -> bool {
    (a - b).abs() <= max_diff
}

fn average_fan(
    bm: &mut BMesh,
    sp: &clnor::Spaces,
    fan: &[LoopId],
    ty: AverageType,
    weight: f32,
    threshold: f32,
) {
    let mut h: heap::HeapSimple<LoopId> = heap::HeapSimple::new();
    for &l in fan {
        let val = match ty {
            AverageType::FaceArea => 1.0 / clnor::face_calc_area(bm, bm.lp(l).f),
            AverageType::CornerAngle => 1.0 / clnor::loop_calc_face_angle(bm, l),
            AverageType::CustomNormal => 1.0,
        };
        h.insert(val, l);
    }
    let mut avg = [0.0f32; 3];
    let mut count = 0.0f32;
    let mut val = h.top_value();
    let mut stack: Vec<LoopId> = Vec::with_capacity(fan.len());
    while !h.is_empty() {
        let cur_val = h.top_value();
        if !compare_ff(val, cur_val, threshold) {
            count += 1.0;
            val = cur_val;
        }
        let l = h.pop_min();
        stack.push(l);
        let n_weight = libm::powf(weight, count);
        let wnor: V3 = match ty {
            AverageType::CustomNormal => sp.decode(bm, l),
            _ => bm.face(bm.lp(l).f).no,
        };
        let wnor = scale(wnor, (1.0 / cur_val) * (1.0 / n_weight));
        avg[0] += wnor[0];
        avg[1] += wnor[1];
        avg[2] += wnor[2];
    }
    let (mut avg, len) = normalize_v3(avg);
    if len < clnor::CLNORS_VALID_VEC_LEN {
        avg = [0.0; 3];
    }
    while let Some(l) = stack.pop() {
        let v = sp.encode(l, avg);
        clnor::set(bm, l, v);
    }
}

fn average_normals(bm: &mut BMesh, ty: AverageType, weight: f32, threshold: f32) {
    let sp = clnor::rebuild(bm);
    clnor::ensure_layer(bm);
    for fan in clnor::selected_vert_fans(bm) {
        match fan {
            clnor::VertFan::Single(l) => {
                let no = bm.face(bm.lp(l).f).no;
                let v = sp.encode(l, no);
                clnor::set(bm, l, v);
            }
            clnor::VertFan::Fan(loops) => average_fan(bm, &sp, &loops, ty, weight, threshold),
        }
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::AverageNormals) -> OpResult {
    let ty = p.average_type.value;
    let weight = weight_factor(p.weight.value as f32);
    let threshold = p.threshold.value;
    for me in edit_meshes_where(cx, |_| true) {
        average_normals(cx.bmesh_mut(me)?, ty, weight, threshold);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weight_factor_like_blender() {
        assert_eq!(weight_factor(50.0), 1.0);
        assert_eq!(weight_factor(100.0), 32767.0);
        assert_eq!(weight_factor(1.0), 1.0 / 32767.0);
        assert_eq!(weight_factor(75.0), 12.5);
        assert_eq!(
            weight_factor(51.0),
            51.0 / 50.0,
            "(1.02 - 1) * 25 = 0.5 is not above 1"
        );
        assert_eq!(weight_factor(25.0), 0.5);
    }
}
