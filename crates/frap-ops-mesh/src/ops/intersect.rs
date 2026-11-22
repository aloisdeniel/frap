// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.intersect(mode='SELECT_UNSELECT', separate_mode='CUT', threshold=1e-06,
//! solver='EXACT')` (Intersect (Knife)): EXACT at T3, FLOAT at T2.
//!
//! Port of `edbm_intersect_exec` and `edbm_intersect_select`
//! (blender/source/blender/editors/mesh/editmesh_intersect.cc) over `BM_mesh_boolean_knife`
//! (blender/source/blender/bmesh/tools/bmesh_boolean.cc), `BM_mesh_intersect`
//! (blender/source/blender/bmesh/tools/bmesh_intersect.cc, `frap_bmesh::ops::intersect`) and
//! `BM_mesh_separate_faces` (blender/source/blender/bmesh/tools/bmesh_separate.cc), Blender
//! 5.2.4, GPL-2.0-or-later. For every mesh in Edit Mode with selected faces, the faces are cut
//! where they intersect: the selected faces with the unselected visible ones
//! (`SELECT_UNSELECT`), or the selected faces with each other (`SELECT`); nothing is removed.
//! The cut lines are shared by the faces on both sides (`NONE`, "Merge"), split apart
//! everywhere (`ALL`, `BM_mesh_edgesplit` on the cut edges), or, with `CUT`
//! (`SELECT_UNSELECT` only; `SELECT` falls back to `ALL` as in Blender), the selected faces are
//! detached from the unselected ones. The cut edges are selected (vertex and edge select
//! mode); WARNING `No intersections found` when no mesh changed.
//!
//! `solver='FLOAT'` is `BM_mesh_intersect(em->looptris, use_dissolve, use_island_connect,
//! use_partial_connect, use_edge_tag)` with `threshold` as its epsilon. `solver='EXACT'`'s cuts
//! come from `frap_boolean::knife` (manifold-rust's exact intersection graph; Blender's exact
//! mesh arrangement is M9, deviations D-101): Blender's cut lines but not always its face
//! layout (T3); a connected part of the selection is not cut by itself in `SELECT` mode; the
//! result is written back like `mesh.intersect_boolean` (new faces copy and interpolate their
//! original face's data).

use frap_bmesh::ops::intersect::tessellate::mesh_calc_tessellation;
use frap_bmesh::ops::intersect::{BooleanMode, IntersectIn, intersect};
use frap_bmesh::ops::split_edges::mesh_edgesplit;
use frap_bmesh::{BMElem, BMesh, CreateFlag, EdgeId, ElemFlag, FaceId, VertId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use frap_spec_ops_mesh::mesh::intersect::{Mode, SeparateMode, Solver};
use rustc_hash::{FxHashMap, FxHashSet};

use super::intersect_boolean::common;

#[operator(
    poll = poll::EDITMESH,
    tier = T3,
    support = Partial(
        "solver='FLOAT' is Blender's BM_mesh_intersect (T2); EXACT's cuts come from \
         manifold-rust's exact intersection graph (Blender's exact mesh arrangement is M9): \
         Blender's cut lines, not always its face layout; EXACT mode='SELECT' does not cut a \
         connected part of the selection with itself"
    ),
)]
fn exec(cx: &mut OpCtx, p: mesh::Intersect) -> OpResult {
    let (side, use_self): (fn(&BMesh, FaceId) -> i32, bool) = match p.mode.value {
        Mode::Select => (common::side_self, true),
        Mode::SelectUnselect => (common::side_pair, false),
    };
    let (use_separate_all, use_separate_cut) = match p.separate_mode.value {
        SeparateMode::All => (true, false),
        SeparateMode::Cut => (use_self, !use_self),
        SeparateMode::None => (false, false),
    };
    let float = p.solver.value == Solver::Float;
    let eps = p.threshold.value;
    let objects = helpers::iter_edit_meshes(cx);
    let mut isect_len = 0;
    for (_, me) in &objects {
        let bm = cx.bmesh_mut(*me)?;
        if bm.selected_counts().2 == 0 {
            continue;
        }
        let has_isect = if float {
            float_knife(bm, side, use_self, use_separate_all, eps)?
        } else {
            knife(bm, side, use_self, use_separate_all)?
        };
        if use_separate_cut {
            separate_faces(bm);
        }
        common::intersect_select(bm, has_isect);
        if !has_isect {
            isect_len += 1;
        }
    }
    if isect_len == objects.len() {
        cx.report(ReportType::Warning, "No intersections found");
    }
    Ok(OpStatus::Finished)
}

/// `BM_mesh_intersect(bm, em->looptris, test_fn, nullptr, use_self, use_separate_all,
/// use_dissolve = true, use_island_connect = true, use_partial_connect = true,
/// use_edge_tag = true, BMESH_ISECT_BOOLEAN_NONE, eps)`: returns whether the mesh changed.
fn float_knife(
    bm: &mut BMesh,
    side: fn(&BMesh, FaceId) -> i32,
    use_self: bool,
    use_separate_all: bool,
    eps: f32,
) -> Result<bool, OpError> {
    // `em->looptris` with valid face normals (`EDBM_update`).
    bm.normals_update();
    let looptris = mesh_calc_tessellation(bm);
    let out = intersect(
        bm,
        &IntersectIn {
            looptris: &looptris,
            test_fn: &side,
            use_self,
            use_separate: use_separate_all,
            use_dissolve: true,
            use_island_connect: true,
            use_partial_connect: true,
            use_edge_tag: true,
            boolean_mode: BooleanMode::None,
            eps,
        },
    )
    .map_err(|e| OpError::Runtime(e.to_string()))?;
    Ok(out.changed)
}

/// `BM_mesh_boolean_knife(bm, looptris, test_fn, nullptr, nshapes, use_self,
/// use_separate_all, hole_tolerant = false, keep_hidden = true)`: returns whether the mesh
/// changed.
fn knife(
    bm: &mut BMesh,
    side: fn(&BMesh, FaceId) -> i32,
    use_self: bool,
    use_separate_all: bool,
) -> Result<bool, OpError> {
    for e in bm.edge_ids() {
        bm.edge_mut(e).flag.remove(ElemFlag::TAG);
    }
    let nshapes = if use_self { 1 } else { 2 };
    let (shapes, rest) = common::shapes(bm, side, nshapes);
    let ops: Vec<frap_boolean::BoolMesh<'_>> =
        shapes.iter().map(common::Shape::bool_mesh).collect();
    let r =
        frap_boolean::knife::knife(&ops, use_self).map_err(|e| OpError::Runtime(e.to_string()))?;
    let isect: FxHashSet<[u32; 2]> = r.isect_edges.iter().copied().collect();
    let out = common::out_mesh(
        bm,
        &shapes,
        &r.positions,
        &r.vert_src,
        &r.face_offsets,
        &r.corner_verts,
        &r.face_src,
        &|a, b| isect.contains(&[a.min(b), a.max(b)]),
    );
    let changed = common::apply(bm, &out, true, &rest);
    if use_separate_all {
        // Separate all faces incident on the cut edges.
        mesh_edgesplit(bm, false, true, false);
    }
    Ok(changed)
}

/// `BM_mesh_separate_faces(bm, selected)`: every vertex shared by a selected and an
/// unselected face gets a copy, which the selected faces use instead (Blender splits their
/// loops off with `BM_face_loop_separate_multi`; frap rebuilds those faces over the copies,
/// with their attributes, corner data and edges' flags, so the cut tags carry over).
fn separate_faces(bm: &mut BMesh) {
    let sel = |bm: &BMesh, f: FaceId| bm.face(f).flag.contains(ElemFlag::SELECT);
    let mut copy: FxHashMap<VertId, VertId> = FxHashMap::default();
    for v in bm.vert_ids() {
        let faces = bm.vert_faces(v);
        let n_sel = faces.iter().filter(|&&f| sel(bm, f)).count();
        if n_sel > 0 && n_sel < faces.len() {
            let co = bm.vert(v).co;
            let nv = bm.vert_create(co, Some(v), CreateFlag::empty());
            bm.elem_select_copy(BMElem::Vert(nv), BMElem::Vert(v));
            copy.insert(v, nv);
        }
    }
    if copy.is_empty() {
        return;
    }
    let faces: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| sel(bm, f) && bm.face_verts(f).iter().any(|v| copy.contains_key(v)))
        .collect();
    for f in faces {
        let old: Vec<VertId> = bm.face_verts(f);
        let verts: Vec<VertId> = old.iter().map(|v| *copy.get(v).unwrap_or(v)).collect();
        let k = verts.len();
        let mut edges: Vec<EdgeId> = Vec::with_capacity(k);
        for i in 0..k {
            let (a, b) = (verts[i], verts[(i + 1) % k]);
            let e = match bm.edge_exists(a, b) {
                Some(e) => e,
                None => {
                    let ex = bm.edge_exists(old[i], old[(i + 1) % k]);
                    let e = bm.edge_create(a, b, ex, CreateFlag::empty());
                    if let Some(ex) = ex {
                        bm.elem_select_copy(BMElem::Edge(e), BMElem::Edge(ex));
                    }
                    e
                }
            };
            edges.push(e);
        }
        if let Some(nf) = bm.face_create(&verts, &edges, Some(f), CreateFlag::empty()) {
            bm.elem_select_copy(BMElem::Face(nf), BMElem::Face(f));
            for l in bm.face_loops(nf) {
                bm.loop_interp_from_face(l, f, false);
            }
            bm.face_kill_loose(f);
        }
    }
}

#[cfg(test)]
mod tests {
    use frap_bmesh::{BMesh, CreateFlag, SelectMode};

    use super::*;
    use crate::ops::intersect_boolean::tests::{add_cube, counts, volume_bounds};

    #[test]
    fn plane_cuts_the_cube_like_blender() {
        // Blender 5.2.4: a plane of size 3 at z = 0.25 added in Edit Mode, then
        // `mesh.intersect(separate_mode='NONE')`: V16 E26 F13, the cut square selected
        // (4 verts, 4 edges) and the plane's inner face; no extra vertex where the cube's
        // triangulation diagonals cross the cut.
        let mut bm = BMesh::new();
        bm.select_mode = SelectMode::VERT;
        add_cube(&mut bm, 2.0, [0.0; 3], false);
        let v: Vec<_> = [[-1.5, -1.5], [1.5, -1.5], [1.5, 1.5], [-1.5, 1.5]]
            .iter()
            .map(|p| bm.vert_create([p[0], p[1], 0.25], None, CreateFlag::empty()))
            .collect();
        let f = bm
            .face_create_verts(&v, None, CreateFlag::empty(), true)
            .unwrap();
        bm.face_select_set(f, true);
        let changed = knife(&mut bm, common::side_pair, false, false).unwrap();
        common::intersect_select(&mut bm, changed);
        assert_eq!(counts(&bm), (16, 26, 13));
        assert_eq!(bm.selected_counts(), (4, 4, 1));
        // The cube (8) and the plane's faces (z * area / 3 = 0.25 * 9 / 3 against the
        // origin), as in Blender.
        let (vol, b) = volume_bounds(&bm);
        assert!((vol - 8.75).abs() < 1e-5, "{vol}");
        assert_eq!(b, [[-1.5, -1.5, -1.0], [1.5, 1.5, 1.0]]);
    }

    #[test]
    fn float_plane_cuts_the_cube_like_blender() {
        // Blender 5.2.4, `mesh.intersect(solver='FLOAT')` with the plane above: NONE V16 E26
        // F13 (4 vertices, 4 edges and the plane's inner face selected), ALL V28 E38 F13
        // (16, 16, 1), CUT V20 E30 F13 (8, 8, 1); the volume stays 8.75.
        for (sep_all, sep_cut, c, sel) in [
            (false, false, (16, 26, 13), (4, 4, 1)),
            (true, false, (28, 38, 13), (16, 16, 1)),
            (false, true, (20, 30, 13), (8, 8, 1)),
        ] {
            let mut bm = BMesh::new();
            bm.select_mode = SelectMode::VERT;
            add_cube(&mut bm, 2.0, [0.0; 3], false);
            let v: Vec<_> = [[-1.5, -1.5], [1.5, -1.5], [1.5, 1.5], [-1.5, 1.5]]
                .iter()
                .map(|p| bm.vert_create([p[0], p[1], 0.25], None, CreateFlag::empty()))
                .collect();
            let f = bm
                .face_create_verts(&v, None, CreateFlag::empty(), true)
                .unwrap();
            bm.face_select_set(f, true);
            let changed = float_knife(&mut bm, common::side_pair, false, sep_all, 1e-6).unwrap();
            if sep_cut {
                separate_faces(&mut bm);
            }
            common::intersect_select(&mut bm, changed);
            assert_eq!(counts(&bm), c, "all {sep_all} cut {sep_cut}");
            assert_eq!(bm.selected_counts(), sel, "all {sep_all} cut {sep_cut}");
            let (vol, _) = volume_bounds(&bm);
            assert!((vol - 8.75).abs() < 1e-5, "{vol}");
        }
    }
}
