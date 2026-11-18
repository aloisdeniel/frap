// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.bridge_edge_loops(type='SINGLE', use_merge=False, merge_factor=0.5,
//! twist_offset=0, number_cuts=0, interpolation='PATH', smoothness=1.0,
//! profile_shape_factor=0.0, profile_shape='SMOOTH')`, tier T2
//! (`tests/golden/cases/edit/create/bridge_*.py`). Port of `edbm_bridge_edge_loops_exec` and
//! `edbm_bridge_edge_loops_for_single_editmesh` (`editors/mesh/editmesh_tools.cc`, Blender
//! 5.2.4, GPL-2.0-or-later) over `bmesh.ops.bridge_loops`.
//!
//! With faces selected, their boundary edges are bridged and the faces deleted first
//! (`FACES_KEEP_BOUNDARY`). Unless merging, the selection becomes the bridge faces; with
//! `number_cuts` the new edge ring is subdivided (WP 2.6's `subdivide_edgering`). Loops of
//! different sizes go through WP 2.10's `triangulate`; merging through WP 2.7's
//! `weld_verts`. `bridge_loops` errors (`Select at least two edge loops`, …) are INFO
//! reports; the operator always finishes.

use frap_bmesh::ops::bridge_loops::{BridgeLoopsIn, bridge_loops};
use frap_bmesh::ops::contextual_create::construct;
use frap_bmesh::{BMElem, BMesh, BmError, ElemFlag, FaceId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, bridge_edge_loops::Type};

use super::edge_face_add::edbm;

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (edbm_bridge_tag_boundary_edges).
fn tag_boundary_edges(bm: &mut BMesh) -> Vec<FaceId> {
    construct::edges_flag_disable_all(bm, ElemFlag::TAG);
    construct::faces_flag_disable_all(bm, ElemFlag::TAG);
    for e in bm.edge_ids() {
        if !bm.edge(e).flag.contains(ElemFlag::SELECT) {
            continue;
        }
        if bm.edge_is_wire(e) || bm.edge_is_boundary(e) {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        } else {
            let mut is_all_sel = true;
            for f in bm.edge_faces(e) {
                if bm.face(f).flag.contains(ElemFlag::SELECT) {
                    bm.face_mut(f).flag.insert(ElemFlag::TAG);
                } else {
                    is_all_sel = false;
                }
            }
            if !is_all_sel {
                bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            }
        }
    }
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::TAG))
        .collect()
}

/// `subdivide_edgering` enum identifiers of the operator's props.
fn interp_id(i: mesh::bridge_edge_loops::Interpolation) -> &'static str {
    i.identifier()
}

#[operator(poll = poll::EDITMESH, tier = T2)]
fn exec(cx: &mut OpCtx, p: mesh::BridgeEdgeLoops) -> OpResult {
    let ty = p.r#type.value;
    let use_pairs = ty == Type::Pairs;
    let use_cyclic = ty == Type::Closed;
    let use_merge = p.use_merge.value;
    for (_, me) in helpers::iter_edit_meshes(cx) {
        let Some((tv, _, tf)) = edbm::with_bm(cx, me, |bm| bm.selected_counts()) else {
            continue;
        };
        if tv == 0 {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        let use_faces = tf != 0;
        let mut del_faces = Vec::new();
        let edge_flag = if use_faces {
            del_faces = tag_boundary_edges(bm);
            ElemFlag::TAG
        } else {
            ElemFlag::SELECT
        };
        let edges = edbm::edges_with(bm, edge_flag);
        if use_faces && !del_faces.is_empty() {
            let geom: Vec<BMElem> = del_faces.iter().map(|&f| BMElem::Face(f)).collect();
            let _ = frap_bmesh::ops::delete::delete(
                bm,
                &frap_bmesh::ops::delete::DeleteIn {
                    geom,
                    context: "FACES_KEEP_BOUNDARY",
                },
            );
        }
        let edges: Vec<_> = edges.into_iter().filter(|e| bm.edges.contains(e.0)).collect();
        let r = bridge_loops(
            bm,
            &BridgeLoopsIn {
                edges,
                use_pairs,
                use_cyclic,
                use_merge,
                merge_factor: p.merge_factor.value,
                twist_offset: p.twist_offset.value,
            },
        );
        let cancelled = matches!(r, Err(BmError::Cancel(_)));
        let out = edbm::finish(cx, r)?;
        if cancelled {
            continue;
        }
        let Some(out) = out else {
            continue;
        };
        let bm = cx.bmesh_mut(me)?;
        if !use_merge {
            edbm::deselect_all(bm);
            let faces: Vec<BMElem> = out.faces.iter().map(|&f| BMElem::Face(f)).collect();
            edbm::select_enable(bm, &faces, true);
            let cuts = p.number_cuts.value;
            if cuts != 0 {
                bm.normals_update();
                let r = frap_bmesh::ops::subdivide_edgering::subdivide_edgering(
                    bm,
                    &frap_bmesh::ops::subdivide_edgering::SubdivideEdgeringIn {
                        edges: out.edges.clone(),
                        interp_mode: interp_id(p.interpolation.value),
                        smooth: p.smoothness.value,
                        cuts,
                        profile_shape: p.profile_shape.value.identifier(),
                        profile_shape_factor: p.profile_shape_factor.value,
                    },
                );
                if let Ok(sub) = r {
                    let faces = edbm::faces_of(&sub.geom);
                    edbm::select_enable(bm, &faces, true);
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}
