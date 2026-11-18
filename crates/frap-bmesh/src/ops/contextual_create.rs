// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.contextual_create` (Blender 5.2.4 `bmo_contextual_create_exec`,
//! `operators/bmo_create.cc`), implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.contextual_create(bmesh, geom=[], mat_nr=0, use_smooth=False)`.
//!
//! **Create/kill order** (the first stage that produces something wins):
//! 1. exactly two vertices and nothing else: one edge (`BM_CREATE_NO_DOUBLE`);
//! 2. a chain of edges plus one free vertex (`totv == tote + 2`): two edges joining the free
//!    vertex to the chain's ends (free–a, then free–b), then the edge-net stages run;
//! 3. with edges: `edgenet_prepare` (up to two closing edges) then `edgenet_fill` (faces in
//!    path-search order, see `contextual_create/edgenet.rs`), attributes and winding from the
//!    neighbouring faces (`face_attribute_fill`), lone faces oriented by
//!    `recalc_face_normals`;
//! 4. with faces: `dissolve_faces` (WP 2.7's op; its `region.out` faces are the result);
//! 5. more than two edges: `edgeloop_fill` (one n-gon per isolated closed loop);
//! 6. more than two vertices: the vertex cloud sorted around its normal
//!    (`BM_verts_sort_radial_plane`) becomes one n-gon (`BM_face_create_ngon_verts`, missing
//!    edges created in vertex order, winding from attached faces), loop data copied from
//!    neighbours (`BM_face_copy_shared`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.
//!
//! Submodules hold the shared helpers of WP 2.8's ops (rule 22.1: submodules of an owned
//! file): BLI math (`bli`), construction/query helpers (`construct`), the edge-net fill
//! (`edgenet`), `edgeloop_fill` and `face_attribute_fill` (`attribute_fill`).

pub mod attribute_fill;
pub mod bli;
pub mod construct;
pub mod edgeloop_fill;
pub mod edgenet;
pub mod split_edgenet;

use rustc_hash::FxHashSet;

use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};

/// Input slots of `contextual_create` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContextualCreateIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `mat_nr`.
    pub mat_nr: i32,
    /// `use_smooth`.
    pub use_smooth: bool,
}

/// Output slots of `contextual_create`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ContextualCreateOut {
    /// `faces.out` then `edges.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the new (or dissolved) faces.
    pub faces: Vec<FaceId>,
    /// `edges.out`: the new edge of the two-vertex case.
    pub edges: Vec<EdgeId>,
}

impl ContextualCreateOut {
    fn new(faces: Vec<FaceId>, edges: Vec<EdgeId>) -> Self {
        let mut geom: Vec<BMElem> = faces.iter().map(|&f| BMElem::Face(f)).collect();
        geom.extend(edges.iter().map(|&e| BMElem::Edge(e)));
        ContextualCreateOut { geom, faces, edges }
    }
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_contextual_create_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// Runs `contextual_create` on `bm`.
pub fn contextual_create(
    bm: &mut BMesh,
    i: &ContextualCreateIn,
) -> Result<ContextualCreateOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "contextual_create: dead element {dead:?} in geom"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

/// The flagged input of one kind, in mesh order (`%fv`/`%fe`/`%ff` slot fills).
fn in_mesh_order<T: Copy + Eq + std::hash::Hash>(all: Vec<T>, set: &FxHashSet<T>) -> Vec<T> {
    all.into_iter().filter(|x| set.contains(x)).collect()
}

fn exec(bm: &mut BMesh, i: &ContextualCreateIn) -> ContextualCreateOut {
    let mat_nr = i.mat_nr;
    let use_smooth = i.use_smooth;
    // `geom` in slot order; ELE_NEW flags.
    let mut v_in: Vec<VertId> = Vec::new();
    let mut new_e: FxHashSet<EdgeId> = FxHashSet::default();
    let mut f_in: Vec<FaceId> = Vec::new();
    let mut seen: FxHashSet<BMElem> = FxHashSet::default();
    for &g in &i.geom {
        if !seen.insert(g) {
            continue;
        }
        match g {
            BMElem::Vert(v) => v_in.push(v),
            BMElem::Edge(e) => {
                new_e.insert(e);
            }
            BMElem::Face(f) => f_in.push(f),
        }
    }
    let totv = v_in.len();
    let mut tote = new_e.len();
    let totf = f_in.len();

    // --- Edge creation.
    if totv == 2 && tote == 0 && totf == 0 {
        let e = bm.edge_create(v_in[0], v_in[1], None, CreateFlag::NO_DOUBLE);
        return ContextualCreateOut::new(Vec::new(), vec![e]);
    }

    // --- A chain of edges with one isolated vertex.
    if totf == 0 && totv >= 4 && totv == tote + 2 {
        let (mut v_free, mut v_a, mut v_b) = (None, None, None);
        let mut ok = true;
        for &v in &v_in {
            let tot_edges = bm
                .vert_edges(v)
                .into_iter()
                .filter(|e| new_e.contains(e))
                .count();
            match tot_edges {
                0 => {
                    if v_free.is_none() {
                        v_free = Some(v);
                    } else {
                        ok = false;
                    }
                }
                1 => {
                    if v_a.is_none() {
                        v_a = Some(v);
                    } else if v_b.is_none() {
                        v_b = Some(v);
                    } else {
                        ok = false;
                    }
                }
                2 => {}
                _ => ok = false,
            }
            if !ok {
                break;
            }
        }
        if ok && let (Some(vf), Some(va), Some(vb)) = (v_free, v_a, v_b) {
            let e = bm.edge_create(vf, va, None, CreateFlag::NO_DOUBLE);
            new_e.insert(e);
            let e = bm.edge_create(vf, vb, None, CreateFlag::NO_DOUBLE);
            new_e.insert(e);
            tote += 2;
        }
    }

    // --- Edge-net create.
    if tote != 0 {
        let edges = in_mesh_order(bm.edge_ids(), &new_e);
        let created = edgenet::edgenet_prepare(bm, &edges);
        new_e.extend(created);
        let edges = in_mesh_order(bm.edge_ids(), &new_e);
        let r = edgenet::edgenet_fill(bm, &edges, mat_nr, use_smooth);
        if !r.faces.is_empty() {
            return ContextualCreateOut::new(r.faces, Vec::new());
        }
    }

    // --- Dissolve faces (WP 2.7's `dissolve_faces`).
    if totf != 0 {
        let faces = in_mesh_order(bm.face_ids(), &f_in.iter().copied().collect());
        let r = crate::ops::dissolve_faces::dissolve_faces(
            bm,
            &crate::ops::dissolve_faces::DissolveFacesIn {
                faces,
                use_verts: false,
            },
        );
        if let Ok(out) = r {
            let region: Vec<FaceId> = out
                .geom
                .iter()
                .filter_map(|g| match g {
                    BMElem::Face(f) => Some(*f),
                    _ => None,
                })
                .collect();
            if !region.is_empty() {
                return ContextualCreateOut::new(region, Vec::new());
            }
        }
    }

    // --- Fill isolated edge loops.
    if tote > 2 {
        let edges = in_mesh_order(bm.edge_ids(), &new_e);
        let faces = edgeloop_fill::edgeloop_fill(bm, &edges, mat_nr, use_smooth);
        if !faces.is_empty() {
            return ContextualCreateOut::new(faces, Vec::new());
        }
    }

    // --- Fill the vertex cloud.
    if totv > 2 {
        let mut vert_arr = v_in.clone();
        construct::verts_sort_radial_plane(bm, &mut vert_arr);
        if let Some(f) = construct::face_create_ngon_verts(
            bm,
            &vert_arr,
            None,
            CreateFlag::NO_DOUBLE,
            true,
            true,
        ) {
            bm.face_mut(f).mat_nr = mat_nr as i16;
            if use_smooth {
                bm.face_mut(f).flag.insert(ElemFlag::SMOOTH);
            }
            construct::face_copy_shared(bm, f, None);
            return ContextualCreateOut::new(vec![f], Vec::new());
        }
    }
    ContextualCreateOut::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_verts_make_an_edge_and_a_loop_makes_a_face() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]]
            .iter()
            .map(|&c| bm.vert_create(c, None, CreateFlag::empty()))
            .collect();
        let out = contextual_create(
            &mut bm,
            &ContextualCreateIn {
                geom: vec![BMElem::Vert(v[0]), BMElem::Vert(v[1])],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.edges.len(), 1);
        let out = contextual_create(
            &mut bm,
            &ContextualCreateIn {
                geom: v.iter().map(|&x| BMElem::Vert(x)).collect(),
                mat_nr: 2,
                use_smooth: true,
            },
        )
        .unwrap();
        assert_eq!(out.faces.len(), 1);
        let f = bm.face(out.faces[0]);
        assert_eq!((f.len, f.mat_nr), (4, 2));
        assert!(f.flag.contains(ElemFlag::SMOOTH));
        assert!(bm.validate().is_empty());
    }
}
