// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.extrude_region(use_normal_flip=False, use_dissolve_ortho_edges=False)`, tier
//! T1 (`tests/golden/cases/edit/extrude/*.py`). Port of `edbm_extrude_region_exec` and
//! `edbm_extrude_mesh` (`editors/mesh/editmesh_extrude.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Every mesh in Edit Mode with a selected vertex takes part. What is extruded depends on the
//! select mode and the selection (`edbm_extrude_mesh`):
//! - vertex mode: one selected vertex, or no selected edge → each vertex alone
//!   (`extrude_vert_indiv`, as `mesh.extrude_verts_indiv`); otherwise the region;
//! - edge mode: no selected edge → nothing; no selected face → each edge alone
//!   (`extrude_edge_only`, as `mesh.extrude_edges_indiv`); otherwise the region;
//! - face mode: no selected face → nothing; otherwise the region.
//!
//! "Nothing" leaves the mesh as it is and the operator still finishes (Blender's `NONE` case
//! returns before its `Not a valid selection for extrude` report; verified on 5.2.4). The region (`extrude_face_region` on the selected vertices, edges and faces, as
//! the select mode allows) replaces the selection by the new geometry; the selection history
//! follows the copies. Edges on the plane of a clipping Mirror modifier get no side face.
//!
//! The shared helpers of the extrude family live in [`common`].

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "Region extrusions create their side faces in a deterministic order; Blender's order \
         changes from run to run (pointer-keyed GHash), so compare those meshes order-free.",
    ],
)]
fn exec(cx: &mut OpCtx, p: mesh::ExtrudeRegion) -> OpResult {
    common::extrude_mesh_all(
        cx,
        p.use_normal_flip.value,
        p.use_dissolve_ortho_edges.value,
    )?;
    Ok(OpStatus::Finished)
}

/// Helpers shared by the `mesh.extrude_*` operators (statics of `editmesh_extrude.cc`).
pub(crate) mod common {
    use frap_bmesh::ops::extrude_discrete_faces::{ExtrudeDiscreteFacesIn, extrude_discrete_faces};
    use frap_bmesh::ops::extrude_edge_only::{ExtrudeEdgeOnlyIn, extrude_edge_only};
    use frap_bmesh::ops::extrude_face_region::{ExtrudeFaceRegionIn, extrude_face_region};
    use frap_bmesh::ops::extrude_vert_indiv::{ExtrudeVertIndivIn, extrude_vert_indiv};
    use frap_bmesh::{BMElem, BMesh, BmError, EdgeId, ElemFlag, SelectMode};
    use frap_op::prelude::*;
    use frap_rna::{IdRef, MeshId, ObjectId};
    use frap_scene::id::ModifierSettings;
    use frap_scene::id::transform::world_matrix;
    use glam::{Mat4, Vec3};

    /// A bmesh-op failure as an operator error (only reachable with a corrupt BMesh).
    pub(crate) fn bm_err(e: BmError) -> OpError {
        OpError::Runtime(e.to_string())
    }

    /// `bm->totvertsel`, `totedgesel`, `totfacesel`.
    pub(crate) fn counts(bm: &BMesh) -> (usize, usize, usize) {
        bm.selected_counts()
    }

    /// Element types to gather (`BM_VERT | BM_EDGE | BM_FACE` bits).
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    pub(crate) struct HType {
        pub(crate) vert: bool,
        pub(crate) edge: bool,
        pub(crate) face: bool,
    }

    impl HType {
        pub(crate) const ALL: HType = HType {
            vert: true,
            edge: true,
            face: true,
        };
    }

    /// `edbm_extrude_htype_from_em_select`.
    pub(crate) fn htype_from_select(bm: &BMesh) -> HType {
        let mut h = HType::ALL;
        if bm.select_mode.contains(SelectMode::VERT) {
            // pass
        } else if bm.select_mode.contains(SelectMode::EDGE) {
            h.vert = false;
        } else {
            h.vert = false;
            h.edge = false;
        }
        let (_, e, f) = counts(bm);
        if e == 0 {
            h.edge = false;
            h.face = false;
        } else if f == 0 {
            h.face = false;
        }
        h
    }

    /// `BMO_slot_buffer_from_enabled_hflag(…, htype, BM_ELEM_SELECT)`: the visible selected
    /// elements of the given types, vertices then edges then faces, in iteration order.
    pub(crate) fn selected_geom(bm: &BMesh, h: HType) -> Vec<BMElem> {
        let ok = |f: ElemFlag| f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN);
        let mut g = Vec::new();
        if h.vert {
            g.extend(
                bm.verts
                    .iter()
                    .filter(|(_, v)| ok(v.flag))
                    .map(|(i, _)| BMElem::Vert(frap_bmesh::VertId(i))),
            );
        }
        if h.edge {
            g.extend(
                bm.edges
                    .iter()
                    .filter(|(_, e)| ok(e.flag))
                    .map(|(i, _)| BMElem::Edge(EdgeId(i))),
            );
        }
        if h.face {
            g.extend(
                bm.faces
                    .iter()
                    .filter(|(_, f)| ok(f.flag))
                    .map(|(i, _)| BMElem::Face(frap_bmesh::FaceId(i))),
            );
        }
        g
    }

    /// `EDBM_flag_disable_all(em, BM_ELEM_SELECT)` between `BM_SELECT_HISTORY_BACKUP` and
    /// `BM_SELECT_HISTORY_RESTORE`: deselects everything, keeps the history.
    pub(crate) fn deselect_all_keep_history(bm: &mut BMesh) {
        let h = std::mem::take(&mut bm.select_history);
        bm.deselect_all();
        bm.select_history = h;
    }

    /// The local-space data a clipping Mirror modifier needs (`edbm_extrude_edge_exclude_mirror`):
    /// for every visible `MIRROR` modifier with `use_clip`, its axes, tolerance and the matrix
    /// from object space to the mirror object's space.
    #[derive(Clone, Debug)]
    pub(crate) struct MirrorClip {
        axes: [bool; 3],
        tolerance: f32,
        mtx: Option<Mat4>,
    }

    /// The clipping mirrors of `ob`.
    pub(crate) fn mirror_clips(cx: &OpCtx, ob: IdRef<ObjectId>) -> Vec<MirrorClip> {
        let main = cx.main();
        let Some(o) = main.objects.get(ob) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for md in &o.modifiers {
            let ModifierSettings::Mirror(m) = &md.settings else {
                continue;
            };
            if !md.show_viewport || !m.use_clip {
                continue;
            }
            let mtx = m.mirror_object.and_then(|id| {
                let mob = id.typed::<ObjectId>()?;
                main.objects.get(mob)?;
                let imtx = world_matrix(&main.objects, mob).inverse();
                Some(imtx * world_matrix(&main.objects, ob))
            });
            out.push(MirrorClip {
                axes: m.use_axis,
                tolerance: m.merge_threshold,
                mtx,
            });
        }
        out
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc
    // (edbm_extrude_edge_exclude_mirror).
    /// The selected boundary edges of selected faces that lie on a clipping mirror plane.
    pub(crate) fn mirror_exclude(bm: &BMesh, clips: &[MirrorClip]) -> Vec<EdgeId> {
        let mut out: Vec<EdgeId> = Vec::new();
        for m in clips {
            for e in bm.edge_ids() {
                let ed = bm.edge(e);
                if !ed.flag.contains(ElemFlag::SELECT) || !bm.edge_is_boundary(e) {
                    continue;
                }
                let l = ed.l.expect("boundary edge has a face");
                if !bm.face(bm.lp(l).f).flag.contains(ElemFlag::SELECT) {
                    continue;
                }
                let mut co1 = bm.vert(ed.v[0]).co;
                let mut co2 = bm.vert(ed.v[1]).co;
                if let Some(mtx) = m.mtx {
                    co1 = mtx.transform_point3(Vec3::from(co1)).to_array();
                    co2 = mtx.transform_point3(Vec3::from(co2)).to_array();
                }
                for axis in 0..3 {
                    if m.axes[axis]
                        && co1[axis].abs() < m.tolerance
                        && co2[axis].abs() < m.tolerance
                        && !out.contains(&e)
                    {
                        out.push(e);
                    }
                }
            }
        }
        out
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc (edbm_extrude_ex).
    /// `edbm_extrude_ex`: extrudes the selected elements of the types `h` as a region; the new
    /// geometry becomes the selection.
    pub(crate) fn extrude_ex(
        bm: &mut BMesh,
        mut h: HType,
        use_normal_flip: bool,
        use_dissolve_ortho_edges: bool,
        edges_exclude: Vec<EdgeId>,
        use_select_history: bool,
    ) -> Result<(), OpError> {
        // Needed to remove the faces left behind.
        if h.face {
            h.edge = true;
        }
        let geom = selected_geom(bm, h);
        deselect_all_keep_history(bm);
        let out = extrude_face_region(
            bm,
            &ExtrudeFaceRegionIn {
                geom,
                edges_exclude,
                use_normal_flip,
                use_dissolve_ortho_edges,
                use_select_history,
                ..Default::default()
            },
        )
        .map_err(bm_err)?;
        for g in out.geom {
            bm.select_set(g, true);
        }
        Ok(())
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc
    // (edbm_extrude_verts_indiv).
    /// `edbm_extrude_verts_indiv`: every selected vertex extruded alone; the copies become the
    /// selection.
    pub(crate) fn extrude_verts_indiv(bm: &mut BMesh) -> Result<(), OpError> {
        let verts: Vec<frap_bmesh::VertId> = selected_geom(
            bm,
            HType {
                vert: true,
                edge: false,
                face: false,
            },
        )
        .into_iter()
        .filter_map(|g| match g {
            BMElem::Vert(v) => Some(v),
            _ => None,
        })
        .collect();
        // Deselect the original vertices.
        for &v in &verts {
            bm.vert_select_set(v, false);
        }
        let out = extrude_vert_indiv(
            bm,
            &ExtrudeVertIndivIn {
                verts,
                use_select_history: true,
            },
        )
        .map_err(bm_err)?;
        for v in out.verts {
            bm.vert_select_set(v, true);
        }
        Ok(())
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc
    // (EDBM_extrude_edges_indiv).
    /// `EDBM_extrude_edges_indiv`: every selected edge extruded alone; the new edges and
    /// vertices become the selection.
    pub(crate) fn extrude_edges_indiv(bm: &mut BMesh, use_normal_flip: bool) -> Result<(), OpError> {
        let edges: Vec<EdgeId> = selected_geom(
            bm,
            HType {
                vert: false,
                edge: true,
                face: false,
            },
        )
        .into_iter()
        .filter_map(|g| match g {
            BMElem::Edge(e) => Some(e),
            _ => None,
        })
        .collect();
        deselect_all_keep_history(bm);
        let out = extrude_edge_only(
            bm,
            &ExtrudeEdgeOnlyIn {
                edges,
                use_normal_flip,
                use_select_history: true,
            },
        )
        .map_err(bm_err)?;
        for g in out.geom {
            if !matches!(g, BMElem::Face(_)) {
                bm.select_set(g, true);
            }
        }
        Ok(())
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc
    // (edbm_extrude_discrete_faces).
    /// `edbm_extrude_discrete_faces`: every selected face extruded alone; the copies become
    /// the selection (the history is cleared) and their vertices take the face normal.
    pub(crate) fn extrude_discrete(bm: &mut BMesh) -> Result<(), OpError> {
        let faces: Vec<frap_bmesh::FaceId> = selected_geom(
            bm,
            HType {
                vert: false,
                edge: false,
                face: true,
            },
        )
        .into_iter()
        .filter_map(|g| match g {
            BMElem::Face(f) => Some(f),
            _ => None,
        })
        .collect();
        // Deselect the original vertices (and everything else; the history too).
        bm.deselect_all();
        let out = extrude_discrete_faces(
            bm,
            &ExtrudeDiscreteFacesIn {
                faces,
                use_select_history: true,
                ..Default::default()
            },
        )
        .map_err(bm_err)?;
        for f in out.faces {
            bm.face_select_set(f, true);
            // Set the face's vertex normals to the face normal.
            let no = bm.face(f).no;
            for v in bm.face_verts(f) {
                bm.vert_mut(v).no = no;
            }
        }
        Ok(())
    }

    /// What `edbm_extrude_mesh` does for a selection.
    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum Nr {
        None,
        ElemFlag,
        VertOnly,
        EdgeOnly,
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_extrude.cc (edbm_extrude_mesh).
    /// `edbm_extrude_mesh`; `Ok(false)` when the selection gives nothing to extrude (the
    /// `NONE` case returns before Blender's `Not a valid selection for extrude` report, which
    /// the other cases never reach).
    pub(crate) fn extrude_mesh(
        bm: &mut BMesh,
        clips: &[MirrorClip],
        use_normal_flip: bool,
        use_dissolve_ortho_edges: bool,
    ) -> Result<bool, OpError> {
        let h = htype_from_select(bm);
        let (v, e, f) = counts(bm);
        let nr = if bm.select_mode.contains(SelectMode::VERT) {
            if v == 0 {
                Nr::None
            } else if v == 1 || e == 0 {
                Nr::VertOnly
            } else {
                Nr::ElemFlag
            }
        } else if bm.select_mode.contains(SelectMode::EDGE) {
            if e == 0 {
                Nr::None
            } else if f == 0 {
                Nr::EdgeOnly
            } else {
                Nr::ElemFlag
            }
        } else if f == 0 {
            Nr::None
        } else {
            Nr::ElemFlag
        };
        match nr {
            Nr::None => return Ok(false),
            Nr::ElemFlag => {
                let exclude = mirror_exclude(bm, clips);
                extrude_ex(
                    bm,
                    h,
                    use_normal_flip,
                    use_dissolve_ortho_edges,
                    exclude,
                    true,
                )?;
            }
            Nr::VertOnly => extrude_verts_indiv(bm)?,
            Nr::EdgeOnly => extrude_edges_indiv(bm, use_normal_flip)?,
        }
        Ok(true)
    }

    /// `edbm_extrude_region_exec` / `edbm_extrude_context_exec`: [`extrude_mesh`] on every
    /// mesh in Edit Mode with a selected vertex, then the normals (`EDBM_update` with
    /// `calc_normals`).
    pub(crate) fn extrude_mesh_all(
        cx: &mut OpCtx,
        use_normal_flip: bool,
        use_dissolve_ortho_edges: bool,
    ) -> Result<(), OpError> {
        for (ob, me) in helpers::iter_edit_meshes(cx) {
            if !has_selected_vert(cx, me) {
                continue;
            }
            let clips = mirror_clips(cx, ob);
            let bm = cx.bmesh_mut(me)?;
            if extrude_mesh(bm, &clips, use_normal_flip, use_dissolve_ortho_edges)? {
                bm.normals_update();
            }
        }
        Ok(())
    }

    /// `em->bm->totvertsel != 0`, without touching the BMesh (no generation bump).
    pub(crate) fn has_selected_vert(cx: &mut OpCtx, me: IdRef<MeshId>) -> bool {
        selected_counts_of(cx, me).0 > 0
    }

    /// The selection counts of `me`'s edit BMesh, read-only.
    pub(crate) fn selected_counts_of(cx: &mut OpCtx, me: IdRef<MeshId>) -> (usize, usize, usize) {
        let (_, edit) = cx.split_edit();
        edit.meshes
            .get(&me.key())
            .map(|em| counts(&em.bm))
            .unwrap_or((0, 0, 0))
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use frap_bmesh::{CreateFlag, VertId};

        /// A quad from x = 0 to x = 2 (normal +Z), everything selected.
        fn quad() -> BMesh {
            let mut bm = BMesh::new();
            let v: Vec<VertId> = [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
                .into_iter()
                .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
                .collect();
            let f = bm
                .face_create_verts(&v, None, CreateFlag::empty(), true)
                .unwrap();
            bm.select_set(BMElem::Face(f), true);
            bm.normals_update();
            bm
        }

        #[test]
        fn mirror_clip_excludes_the_edge_on_the_plane() {
            let bm = quad();
            let x = MirrorClip {
                axes: [true, false, false],
                tolerance: 0.001,
                mtx: None,
            };
            let ex = mirror_exclude(&bm, std::slice::from_ref(&x));
            assert_eq!(ex.len(), 1);
            assert_eq!(bm.edge(ex[0]).v.map(|v| bm.vert(v).co[0]), [0.0, 0.0]);
            // Another axis, or a mirror object moving the plane, changes the result.
            let y = MirrorClip {
                axes: [false, true, false],
                ..x.clone()
            };
            assert_eq!(mirror_exclude(&bm, &[y]).len(), 1);
            let moved = MirrorClip {
                mtx: Some(Mat4::from_translation(Vec3::new(-2.0, 0.0, 0.0))),
                ..x.clone()
            };
            let ex = mirror_exclude(&bm, &[moved]);
            assert_eq!(bm.edge(ex[0]).v.map(|v| bm.vert(v).co[0]), [2.0, 2.0]);
            // Two clipping mirrors on the same axis list an edge once.
            assert_eq!(mirror_exclude(&bm, &[x.clone(), x]).len(), 1);
        }

        #[test]
        fn htype_follows_mode_and_selection() {
            let mut bm = quad();
            assert_eq!(htype_from_select(&bm), HType::ALL);
            bm.select_mode = SelectMode::EDGE;
            assert_eq!(
                htype_from_select(&bm),
                HType {
                    vert: false,
                    edge: true,
                    face: true
                }
            );
            bm.select_mode = SelectMode::FACE;
            bm.deselect_all();
            assert_eq!(
                htype_from_select(&bm),
                HType {
                    vert: false,
                    edge: false,
                    face: false
                }
            );
            // Nothing selected: nothing to extrude, nothing changed.
            assert!(!extrude_mesh(&mut bm, &[], false, false).unwrap());
            assert_eq!(bm.verts_num(), 4);
        }
    }
}
