// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.faces_select_linked_flat(sharpness=0.0174533)`, tier T1
//! (`tests/golden/cases/edit/select/flat_*.py`). Port of
//! `edbm_select_linked_flat_faces_exec` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode with selected faces, the selection grows from every selected visible
//! face across edges to neighbouring visible faces whose normal is within `sharpness` of the
//! face it is reached from (`dot(n_a, n_b) > cos(sharpness)`), so a flat region is selected
//! from any of its faces. The selection is not flushed (as in Blender).

use frap_bmesh::FaceId;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use rustc_hash::FxHashSet;

use super::edges_select_sharp::dot;
use super::select_more::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::FacesSelectLinkedFlat) -> OpResult {
    let angle_limit_cos = p.sharpness.value.cos();
    for (_, me) in common::edit_meshes(cx) {
        let any_face = common::read(cx, me, |bm| bm.selected_counts().2 != 0).unwrap_or(false);
        if !any_face {
            continue;
        }
        let bm = cx.bmesh_mut(me)?;
        // `BM_ELEM_TAG` of the faces.
        let mut tag: FxHashSet<FaceId> = FxHashSet::default();
        let mut stack: Vec<FaceId> = Vec::new();
        for start in bm.face_ids() {
            if common::hidden(bm, start) || tag.contains(&start) || !common::selected(bm, start) {
                continue;
            }
            let mut f = start;
            loop {
                bm.face_select_set(f, true);
                tag.insert(f);
                let no = bm.face(f).no;
                for l in bm.face_loops(f) {
                    // `BM_LOOPS_OF_LOOP`: the other loops around the edge.
                    let mut l2 = bm.lp(l).radial_next;
                    while l2 != l {
                        let f2 = bm.lp(l2).f;
                        l2 = bm.lp(l2).radial_next;
                        if tag.contains(&f2) || common::hidden(bm, f2) {
                            continue;
                        }
                        if dot(no, bm.face(f2).no) > angle_limit_cos {
                            stack.push(f2);
                        }
                    }
                }
                match stack.pop() {
                    Some(next) => f = next,
                    None => break,
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}
