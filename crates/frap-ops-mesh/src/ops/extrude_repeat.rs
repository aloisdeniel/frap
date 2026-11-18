// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_repeat(steps=10, offset=(0, 0, 0), scale_offset=1.0)`, tier T1
//! (`tests/golden/cases/edit/extrude/repeat*.py`). Port of `edbm_extrude_repeat_exec`
//! (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! `steps` times: the selection (vertices, edges and faces, whatever the select mode) is
//! extruded as a region (no mirror clipping) and its vertices are moved by `offset ·
//! scale_offset`, given in world space and converted to each object's space (the inverse of
//! the object's 3 × 3 world matrix). In 5.x `offset` is a vector; when it is not set, the
//! direction is the view's: Blender normalises the third column of the 3D View's inverse
//! perspective matrix, which for frap's virtual perspective view (D-10) is the direction from
//! the viewpoint towards the world origin.
//!
//! `steps` goes up to Blender's hard maximum (1 000 000) and every step rescans the mesh, so
//! the counts the run would create are estimated first and checked against `--max-elements`
//! (ARCHITECTURE §12, [`frap_math::budget::reserve_corners`]): an absurd step count fails at
//! once with `LIMIT_EXCEEDED` instead of running for minutes.

use frap_bmesh::{BMesh, ElemFlag, VertId};
use frap_math::glam::{DMat4, Mat3, Vec3};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix;
use frap_spec_ops_mesh::mesh;

use super::extrude_region::common;

/// `normalize_v3_v3(offset, rv3d->persinv[2])`: for a perspective window matrix the third
/// column of `inverse(winmat · viewmat)` is the viewpoint's position divided by the negative
/// `winmat[3][2]`, so the offset points from the viewpoint towards the origin.
fn view_offset(cx: &OpCtx) -> [f32; 3] {
    let vm: DMat4 = cx.ctx().win.view3d.view_matrix.as_dmat4();
    let cam = vm.inverse().w_axis.truncate();
    let len = cam.length();
    if len > 0.0 {
        (-cam / len).as_vec3().to_array()
    } else {
        [0.0, 0.0, 1.0]
    }
}

/// `bmesh.ops.translate(verts=<selected>, vec=off)`: `mul_m4_v3` with a translation matrix.
fn translate_selected(bm: &mut BMesh, off: [f32; 3]) {
    let verts: Vec<VertId> = bm
        .verts
        .iter()
        .filter(|(_, v)| v.flag.contains(ElemFlag::SELECT) && !v.flag.contains(ElemFlag::HIDDEN))
        .map(|(i, _)| VertId(i))
        .collect();
    for v in verts {
        let c = bm.vert(v).co;
        bm.vert_mut(v).co = [
            c[0] * 1.0 + c[1] * 0.0 + c[2] * 0.0 + off[0],
            c[0] * 0.0 + c[1] * 1.0 + c[2] * 0.0 + off[1],
            c[0] * 0.0 + c[1] * 0.0 + c[2] * 1.0 + off[2],
        ];
    }
}

/// Upper bound of the elements one region-extrude step adds, from the selected (visible)
/// elements it duplicates: `[verts, edges, faces, corners]`. A step duplicates every selected
/// vertex (`sv`) and edge (`se`), joins each vertex to its copy (`sv` edges), adds one side
/// quad per selected edge at most (`se` faces, `4·se` corners) and, when the region has no
/// boundary and its originals are kept, duplicates the selected faces (`sf` faces, `sc`
/// corners).
fn step_growth(bm: &BMesh) -> [u64; 4] {
    let visible_sel = |f: ElemFlag| f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN);
    let sv = bm.verts.iter().filter(|(_, v)| visible_sel(v.flag)).count() as u64;
    let se = bm.edges.iter().filter(|(_, e)| visible_sel(e.flag)).count() as u64;
    let (sf, sc) = bm
        .faces
        .iter()
        .filter(|(_, f)| visible_sel(f.flag))
        .fold((0u64, 0u64), |(n, c), (_, f)| (n + 1, c + u64::from(f.len)));
    [sv, se + sv, se + sf, 4 * se + sc]
}

/// Totals after `steps` steps (`None` on overflow): `current + steps · growth` per domain.
fn estimate_totals(current: [u64; 4], growth: [u64; 4], steps: u64) -> Option<[u64; 4]> {
    let mut out = [0u64; 4];
    for i in 0..4 {
        out[i] = growth[i].checked_mul(steps)?.checked_add(current[i])?;
    }
    Some(out)
}

/// The `--max-elements` pre-check over every mesh in Edit Mode (one statement, one budget).
fn reserve(cx: &mut OpCtx, steps: u64) -> Result<(), OpError> {
    let meshes = helpers::iter_edit_meshes(cx);
    let (_, edit) = cx.split_edit();
    let mut total = Some([0u64; 4]);
    for (_, me) in meshes {
        let Some(em) = edit.meshes.get(&me.key()) else {
            continue;
        };
        let bm = &em.bm;
        let current = [
            bm.verts_num() as u64,
            bm.edges_num() as u64,
            bm.faces_num() as u64,
            bm.loops_num() as u64,
        ];
        let one = estimate_totals(current, step_growth(bm), steps);
        total = total.zip(one).and_then(|(t, o)| {
            let mut s = [0u64; 4];
            for i in 0..4 {
                s[i] = t[i].checked_add(o[i])?;
            }
            Some(s)
        });
    }
    frap_math::budget::reserve_corners(
        total.map(|t| t[3]),
        total.map(|t| t.into_iter().max().unwrap_or(0)),
    )
    .map_err(|e| OpError::Limit(e.to_string()))
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::ExtrudeRepeat) -> OpResult {
    let steps = p.steps.value.max(0);
    let scale_offset = p.scale_offset.value;
    let mut offset = if p.offset.is_set {
        p.offset.value
    } else {
        view_offset(cx)
    };
    for c in &mut offset {
        *c *= scale_offset;
    }
    reserve(cx, steps as u64)?;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        // `copy_m3_m4`, `invert_m3`, `mul_v3_m3v3`.
        let tmat = Mat3::from_mat4(world_matrix(&cx.main().objects, ob));
        let tmat = if tmat.determinant() != 0.0 {
            tmat.inverse()
        } else {
            tmat
        };
        let off = (tmat * Vec3::from(offset)).to_array();
        let bm = cx.bmesh_mut(me)?;
        for _ in 0..steps {
            frap_math::budget::check();
            common::extrude_ex(bm, common::HType::ALL, false, false, Vec::new(), true)?;
            // The translate op runs top-level: it updates the normals.
            translate_selected(bm, off);
            bm.normals_update();
        }
        bm.normals_update();
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn growth_of_a_selected_cube_face() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, frap_bmesh::CreateFlag::empty()))
        .collect();
        let f = bm
            .face_create_verts(&v, None, frap_bmesh::CreateFlag::empty(), true)
            .unwrap();
        bm.select_set(frap_bmesh::BMElem::Face(f), true);
        // A step on a selected quad adds at most 4 verts, 8 edges, 4 side quads + 1 copy.
        let growth = step_growth(&bm);
        assert_eq!(growth, [4, 8, 5, 20]);
        // One exact extrude step stays within the bound.
        let counts = |bm: &BMesh| {
            [
                bm.verts_num(),
                bm.edges_num(),
                bm.faces_num(),
                bm.loops_num(),
            ]
        };
        let before = counts(&bm);
        common::extrude_ex(&mut bm, common::HType::ALL, false, false, Vec::new(), true).unwrap();
        let after = counts(&bm);
        for i in 0..4 {
            assert!((after[i] - before[i]) as u64 <= growth[i], "domain {i}");
        }
    }

    #[test]
    fn totals_overflow_is_none() {
        assert_eq!(
            estimate_totals([8, 12, 6, 24], [8, 20, 18, 72], 1_000_000),
            Some([8_000_008, 20_000_012, 18_000_006, 72_000_024])
        );
        assert_eq!(estimate_totals([0; 4], [u64::MAX; 4], 2), None);
        // Over the default `--max-elements` (50 M corners): rejected before running.
        let _s = frap_math::budget::scope(frap_math::budget::Budget {
            timeout: None,
            max_elements: 50_000_000,
        });
        assert!(frap_math::budget::reserve_corners(Some(72_000_024), Some(72_000_024)).is_err());
    }
}
