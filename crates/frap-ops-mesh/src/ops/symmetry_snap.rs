// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.symmetry_snap(direction='NEGATIVE_X', threshold=0.05, factor=0.5,
//! use_center=True, use_topology=False)`, tier T1
//! (`tests/golden/cases/edit/tools_m8/symmetry_snap.py`). Port of `mesh_symmetry_snap_exec`
//! (`editors/mesh/editmesh_tools.cc`) with the mirror cache of
//! `EDBM_verts_mirror_cache_begin_ex` ([`mirror`]), Blender 5.2.4, GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with a selected vertex, each selected vertex is paired with the
//! vertex nearest to its mirrored position along the direction's axis (within `threshold`;
//! with `use_topology` the topological mirror instead). For a pair, the vertex on the
//! direction's source side keeps `lerp(v, mirror(v_mirr), factor)` and its partner gets the
//! mirror of that; a vertex that is its own mirror is put on the axis with `use_center`.
//! Reports `N already symmetrical, M pairs mirrored` (INFO), with `, K failed` (WARNING) when
//! a selected vertex had no mirror. A mesh whose active shape key is locked is skipped with
//! an ERROR (a `RuntimeError` when no other mesh was processed). `CANCELLED` when no mesh was
//! processed.

use frap_bmesh::{ElemFlag, ElemTypes, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, symmetry_snap::Direction};

use super::blend_from_shape::active_block_locked;
use super::split::edbm;

pub(crate) mod mirror;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SymmetrySnap) -> OpResult {
    let eps = 0.00001f32;
    let eps_sq = eps * eps;
    let thresh = p.threshold.value;
    let fac = p.factor.value;
    let use_center = p.use_center.value;
    let use_topology = p.use_topology.value;
    let axis_dir = match p.direction.value {
        Direction::NegativeX => 0,
        Direction::NegativeY => 1,
        Direction::NegativeZ => 2,
        Direction::PositiveX => 3,
        Direction::PositiveY => 4,
        Direction::PositiveZ => 5,
    };
    let axis = axis_dir % 3;
    let axis_sign = axis != axis_dir;

    let (mut totvertfound, mut totvertmirr, mut totvertfail, mut totobjects) = (0, 0, 0, 0);
    let mut locked_msg = String::new();
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        if edbm::read(cx, me, edbm::sel_counts).unwrap_or_default().0 == 0 {
            continue;
        }
        if active_block_locked(cx, ob) {
            let name = cx.main().objects.name_of(ob).unwrap_or_default().to_owned();
            locked_msg = format!("The active shape key of {name} is locked");
            cx.report(ReportType::Error, locked_msg.clone());
            continue;
        }
        totobjects += 1;
        let bm = cx.bmesh_mut(me)?;
        bm.elem_index_ensure(ElemTypes::VERT);
        let verts: Vec<VertId> = bm.vert_ids();
        let positions: Vec<[f32; 3]> = verts.iter().map(|&v| bm.vert(v).co).collect();
        let selected: Vec<bool> = verts
            .iter()
            .map(|&v| bm.vert(v).flag.contains(ElemFlag::SELECT))
            .collect();
        let edges: Vec<[usize; 2]> = bm
            .edges
            .iter()
            .map(|(_, e)| {
                [
                    bm.index_get(e.v[0].into()) as usize,
                    bm.index_get(e.v[1].into()) as usize,
                ]
            })
            .collect();
        let index = mirror::mirror_cache(&positions, &edges, &selected, axis, use_topology, thresh);
        let mut tag = vec![false; verts.len()];
        for (i, &v0) in verts.iter().enumerate() {
            if !selected[i] || tag[i] {
                continue;
            }
            let Some(i_mirr) = index[i] else {
                totvertfail += 1;
                continue;
            };
            if i_mirr != i {
                let (mut v, mut vi) = (v0, i);
                let (mut v_mirr, mut vmi) = (verts[i_mirr], i_mirr);
                if (bm.vert(v).co[axis] > bm.vert(v_mirr).co[axis]) == axis_sign {
                    std::mem::swap(&mut v, &mut v_mirr);
                    std::mem::swap(&mut vi, &mut vmi);
                }
                let mut co_mirr = bm.vert(v_mirr).co;
                co_mirr[axis] *= -1.0;
                let vco = bm.vert(v).co;
                let d = [
                    vco[0] - co_mirr[0],
                    vco[1] - co_mirr[1],
                    vco[2] - co_mirr[2],
                ];
                if d[0] * d[0] + d[1] * d[1] + d[2] * d[2] > eps_sq {
                    totvertmirr += 1;
                }
                // `interp_v3_v3v3(co, v->co, co_mirr, fac)`.
                let s = 1.0 - fac;
                let mut co = [
                    s * vco[0] + fac * co_mirr[0],
                    s * vco[1] + fac * co_mirr[1],
                    s * vco[2] + fac * co_mirr[2],
                ];
                bm.vert_mut(v).co = co;
                co[axis] *= -1.0;
                bm.vert_mut(v_mirr).co = co;
                tag[vi] = true;
                tag[vmi] = true;
                totvertfound += 1;
            } else {
                if use_center {
                    if bm.vert(v0).co[axis].abs() > eps {
                        totvertmirr += 1;
                    }
                    bm.vert_mut(v0).co[axis] = 0.0;
                }
                tag[i] = true;
                totvertfound += 1;
            }
        }
        bm.normals_update();
    }
    if totvertfail > 0 {
        cx.report(
            ReportType::Warning,
            format!(
                "{} already symmetrical, {} pairs mirrored, {} failed",
                totvertfound - totvertmirr,
                totvertmirr,
                totvertfail
            ),
        );
    } else if totobjects > 0 {
        cx.report(
            ReportType::Info,
            format!(
                "{} already symmetrical, {} pairs mirrored",
                totvertfound - totvertmirr,
                totvertmirr
            ),
        );
    }
    if totobjects == 0 && !locked_msg.is_empty() {
        // Blender's error report makes the call raise.
        return Err(OpError::Runtime(locked_msg));
    }
    Ok(if totobjects > 0 {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
