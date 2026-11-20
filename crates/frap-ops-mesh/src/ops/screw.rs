// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.screw(steps=9, turns=1, center=(0, 0, 0), axis=(0, 0, 0))`, tier T2
//! (`tests/golden/cases/edit/misc/screw.py`). Port of `edbm_screw_exec`
//! (`editors/mesh/editmesh_extrude_screw.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with at least two selected vertices: the selection must be an
//! **open** chain (exactly two vertices with one selected edge, else the mesh is skipped);
//! the helix offset `dvec` is the world-space difference of the chain's two ends divided by
//! `steps`, pointing against the object's Z axis; then `bmesh.ops.spin` runs `turns · steps`
//! steps over `360° · turns` in world space with that `dvec`, and the selection becomes the
//! last step. A zero `axis` skips every mesh. When every mesh with a selection was skipped
//! for its axis the error is `Invalid/unset axis`, when every one lacked a chain it is `You
//! have to select a string of connected vertices too` (ERROR reports, raised as
//! `RuntimeError`; the operator still finishes, as in Blender). 3-vertex chain, 8 steps:
//! V32 E48 F16 ([MESH §4.12]). The extrusion is `extrude_face_region` (WP 2.5).

use frap_bmesh::ops::spin::{SpinIn, spin};
use frap_bmesh::{ElemFlag, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::Screw) -> OpResult {
    let turns = p.turns.value;
    let steps = p.steps.value;
    let cent = p.center.value;
    let axis = p.axis.value;
    // `--max-elements`: one extrusion of the selection per step (ARCHITECTURE §12).
    let total_steps = u64::try_from(turns.max(0))
        .unwrap_or(0)
        .saturating_mul(u64::try_from(steps.max(0)).unwrap_or(0));
    super::_growth::reserve(cx, |_, bm| {
        super::_growth::extrude_steps_corners(bm, total_steps, false)
    })?;
    let objects = helpers::iter_edit_meshes(cx);
    let (mut objects_empty_len, mut failed_axis_len, mut failed_verts_len) = (0usize, 0, 0);
    for &(ob, me) in &objects {
        let (vsel, _, _) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if vsel < 2 {
            if vsel == 0 {
                objects_empty_len += 1;
            }
            continue;
        }
        if axis == [0.0; 3] {
            failed_axis_len += 1;
            continue;
        }
        // Find two vertices with one selected edge each; more or less is wrong.
        let ends = edbm::read(cx, me, |bm| {
            let (mut v1, mut v2): (Option<VertId>, Option<VertId>) = (None, None);
            for v in bm.vert_ids() {
                let valence = bm
                    .vert_edges(v)
                    .into_iter()
                    .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
                    .count();
                if valence == 1 {
                    if v1.is_none() {
                        v1 = Some(v);
                    } else if v2.is_none() {
                        v2 = Some(v);
                    } else {
                        v1 = None;
                        break;
                    }
                }
            }
            match (v1, v2) {
                (Some(a), Some(b)) => Some((bm.vert(a).co, bm.vert(b).co)),
                _ => None,
            }
        })
        .flatten();
        let Some((co1, co2)) = ends else {
            failed_verts_len += 1;
            continue;
        };
        let obmat = world_matrix_in(cx.main(), ob);
        let nor = obmat.z_axis.truncate().to_array();
        let m = obmat.to_cols_array_2d();
        let mul_m4 = |c: [f32; 3]| -> [f32; 3] {
            [
                c[0] * m[0][0] + c[1] * m[1][0] + m[2][0] * c[2] + m[3][0],
                c[0] * m[0][1] + c[1] * m[1][1] + m[2][1] * c[2] + m[3][1],
                c[0] * m[0][2] + c[1] * m[1][2] + m[2][2] * c[2] + m[3][2],
            ]
        };
        let (g1, g2) = (mul_m4(co1), mul_m4(co2));
        let inv = 1.0 / steps as f32;
        let mut dvec = [
            (g1[0] - g2[0]) * inv,
            (g1[1] - g2[1]) * inv,
            (g1[2] - g2[2]) * inv,
        ];
        if nor[0] * dvec[0] + nor[1] * dvec[1] + nor[2] * dvec[2] > 0.0 {
            dvec = [-dvec[0], -dvec[1], -dvec[2]];
        }
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        let r = spin(
            bm,
            &SpinIn {
                geom,
                cent,
                axis,
                dvec,
                steps: turns * steps,
                angle: (360.0f32 * turns as f32) * (std::f32::consts::PI / 180.0),
                space: m,
                use_duplicate: false,
                ..SpinIn::default()
            },
        );
        let Some(out) = edbm::finish(cx, r)? else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        edbm::deselect_all(bm);
        edbm::select_elems(bm, &out.geom_last);
    }
    let considered = objects.len() - objects_empty_len;
    if failed_axis_len == considered {
        return Err(OpError::Runtime("Invalid/unset axis".into()));
    }
    if failed_verts_len == considered {
        return Err(OpError::Runtime(
            "You have to select a string of connected vertices too".into(),
        ));
    }
    Ok(OpStatus::Finished)
}
