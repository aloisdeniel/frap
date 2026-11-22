// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.merge_normals()`, tier T1
//! (`tests/golden/cases/edit/normals_m8/merge_split.py`). Port of `edbm_merge_normals_exec`,
//! `normals_split_merge` and `normals_merge` (`editors/mesh/editmesh_tools.cc`), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! On every mesh in Edit Mode (unique data): the custom normals of every corner of the selected
//! elements' vertices are read (whole vertices, by select mode and history), the selected faces
//! and edges are made smooth, the normal spaces are rebuilt, and each smooth fan containing a
//! read corner gets the normalised sum of its corners' old normals (zero, i.e. the automatic
//! normal, when shorter than 1e-4). Corners between two sharp edges keep theirs. The
//! `custom_normal` layer is created when missing. The operator always finishes.
//!
//! [`split_merge`] also serves `mesh.split_normals`.

use frap_bmesh::{BMesh, ElemFlag};
use frap_geom::normals::normalize_v3;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

use super::faces_shade_smooth::set_smooth_faces;
use super::flip_normals::{clnor, edit_meshes_where};

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc (normals_merge).
fn normals_merge(bm: &mut BMesh, sp: &clnor::Spaces, arr: &clnor::EditArray) {
    clnor::loops_edges_tag(bm, false);
    for d in &arr.data {
        if bm.lp(d.loop_).flag.contains(ElemFlag::TAG) {
            continue;
        }
        let Some(space) = sp.of(d.loop_) else {
            continue;
        };
        if space.single {
            continue;
        }
        let mut avg = [0.0f32; 3];
        let mut fan = Vec::with_capacity(space.loops.len());
        for &l in &space.loops {
            if let Some(e) = arr.by_index[sp.index(l)] {
                let n = arr.data[e].nloc;
                avg[0] += n[0];
                avg[1] += n[1];
                avg[2] += n[2];
            }
            fan.push(l);
            bm.lp_mut(l).flag.insert(ElemFlag::TAG);
        }
        let (mut avg, len) = normalize_v3(avg);
        if len < clnor::CLNORS_VALID_VEC_LEN {
            avg = [0.0; 3];
        }
        for l in fan {
            let v = clnor::custom_normal_to_data(&space.space, avg);
            clnor::set(bm, l, v);
        }
    }
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc (normals_split).
fn normals_split(bm: &mut BMesh, sp: &clnor::Spaces) {
    clnor::ensure_layer(bm);
    for fan in clnor::selected_vert_fans(bm) {
        match fan {
            clnor::VertFan::Single(l) => {
                let no = bm.face(bm.lp(l).f).no;
                let v = sp.encode(l, no);
                clnor::set(bm, l, v);
            }
            clnor::VertFan::Fan(loops) => {
                let mut avg = [0.0f32; 3];
                for &l in &loops {
                    let no = bm.face(bm.lp(l).f).no;
                    avg[0] += no[0];
                    avg[1] += no[1];
                    avg[2] += no[2];
                }
                let (mut avg, len) = normalize_v3(avg);
                if len < clnor::CLNORS_VALID_VEC_LEN {
                    avg = [0.0; 3];
                }
                for &l in loops.iter().rev() {
                    let v = sp.encode(l, avg);
                    clnor::set(bm, l, v);
                }
            }
        }
    }
}

/// `normals_split_merge(C, do_merge)` on one edit BMesh.
pub(crate) fn split_merge(bm: &mut BMesh, do_merge: bool) {
    let sp = clnor::update(bm);
    // Edit data for every corner of the affected vertices: making faces and edges smooth changes
    // the spaces (#65809).
    let arr = do_merge.then(|| clnor::editdata_array(bm, &sp, true, clnor::Mark::Indiv));
    set_smooth_faces(bm, do_merge);
    for e in bm.edge_ids() {
        if bm.edge(e).flag.contains(ElemFlag::SELECT) {
            bm.edge_mut(e).flag.set(ElemFlag::SMOOTH, do_merge);
        }
    }
    let sp = clnor::rebuild(bm);
    match arr {
        Some(arr) => normals_merge(bm, &sp, &arr),
        None => normals_split(bm, &sp),
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::MergeNormals) -> OpResult {
    for me in edit_meshes_where(cx, |_| true) {
        split_merge(cx.bmesh_mut(me)?, true);
    }
    Ok(OpStatus::Finished)
}
