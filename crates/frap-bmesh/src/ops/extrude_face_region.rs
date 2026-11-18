// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.extrude_face_region` (Blender 5.2.4 `bmo_extrude_face_region_exec`,
//! `operators/bmo_extrude.cc`), implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.extrude_face_region(bmesh, geom=[], edges_exclude={}, use_keep_orig=False, use_normal_flip=False, use_normal_from_adjacent=False, use_dissolve_ortho_edges=False, use_select_history=False, skip_input_flip=False)`.
//!
//! **Create/kill order** (verified on the factory cube [MESH §2.4, App. B.2]):
//! 1. `bmesh.ops.duplicate` copies the whole input `geom` (vertices, then edges, then faces:
//!    see `duplicate.rs`); the copies of the input faces are the "cap".
//! 2. Unless `use_keep_orig`, when some input edge also borders a face that is not input
//!    ("`delorig`"), the originals are deleted (`delete context=TAGGED_ONLY`): the input faces,
//!    the input edges used only by input faces (two or more of them), and the vertices whose
//!    every edge is such an edge and every face is input; faces first, then edges, then
//!    vertices, each in iteration order. Their slots are reused LIFO by step 3.
//!    Without `delorig` the input faces are kept and flipped instead (unless
//!    `skip_input_flip`).
//! 3. For every copied edge with fewer than two input faces (`boundary_map.out`), one side quad
//!    joining it to its copy: the two missing side edges are created first (`(v1, v2)` then
//!    `(v3, v0)` of the quad's corners, each only when it does not exist yet, taking their
//!    seam/sharp flags from the neighbouring edges), then the quad, which takes the face
//!    attributes and loop data of the face across its first edge.
//! 4. Every copied isolated vertex (`isovert_map.out`) is joined to its original by a wire
//!    edge.
//! 5. With `use_dissolve_ortho_edges`, side quads whose original edge bordered a face
//!    perpendicular to the region's mean normal are joined into that face and their in-plane
//!    vertices collapsed (`extrude_manifold`).
//!
//! **Order of the side quads (deviation from Blender's run-to-run order):** Blender iterates
//! `boundary_map.out` and `isovert_map.out`, `GHash`es keyed by element pointers, so with two
//! or more boundary edges (or isolated vertices) the creation order of the side edges and
//! quads changes from one run of Blender to the next (verified: four runs of the same
//! extrusion gave three orders). frap uses the order in which the duplicate copied them
//! (iteration order), one of Blender's possible orders. The cap (vertices, edges, faces) and
//! everything before step 3 are deterministic and match Blender index for index.
//!
//! Operator type flags: `NORMALS_CALC`.

use crate::ops::delete::{DeleteIn, delete};
use crate::ops::duplicate::{DuplicateIn, duplicate};
use crate::ops::extrude_edge_only::copy_face_loop_attributes;
use crate::ops::extrude_vert_indiv::vert_is_wire_endpoint;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};

/// Input slots of `extrude_face_region` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ExtrudeFaceRegionIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `edges_exclude` (a set of edges).
    pub edges_exclude: Vec<EdgeId>,
    /// `use_keep_orig`.
    pub use_keep_orig: bool,
    /// `use_normal_flip`.
    pub use_normal_flip: bool,
    /// `use_normal_from_adjacent`.
    pub use_normal_from_adjacent: bool,
    /// `use_dissolve_ortho_edges`.
    pub use_dissolve_ortho_edges: bool,
    /// `use_select_history`.
    pub use_select_history: bool,
    /// `skip_input_flip`.
    pub skip_input_flip: bool,
}

/// Output slots of `extrude_face_region`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ExtrudeFaceRegionOut {
    /// `geom.out`: the copies made by the duplicate (the new region: vertices, then edges, then
    /// faces, in iteration order at the time of the copy).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_extrude_face_region_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

const EXT_INPUT: u16 = 1;
const EXT_DEL: u16 = 4;
const EXT_TAG: u16 = 8;

// Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
// (bm_extrude_region_edge_flag, USE_EDGE_REGION_FLAGS).
/// The flags a new side edge at `v` takes from the face edges around `v`: `SEAM` when any of
/// them has it (`.0`, to enable), `SMOOTH` cleared when any of them is sharp (`.1`, to
/// disable). `None` when `v` has no edge with two or more faces.
fn region_edge_flag(bm: &BMesh, v: VertId) -> Option<(ElemFlag, ElemFlag)> {
    let e_first = bm.vert(v).e?;
    let mut any = ElemFlag::empty();
    let mut all = ElemFlag::all();
    let mut ok = false;
    let mut e = e_first;
    loop {
        if bm.edge(e).l.is_some() && !bm.edge_is_boundary(e) {
            any |= bm.edge(e).flag;
            all &= bm.edge(e).flag;
            ok = true;
        }
        e = bm.disk_edge_next(e, v);
        if e == e_first {
            break;
        }
    }
    ok.then(|| (any & ElemFlag::SEAM, ElemFlag::SMOOTH & !all))
}

/// Creates the side edge `a`–`b` unless it exists, with the flags of [`region_edge_flag`] at
/// `flag_vert`.
fn side_edge(bm: &mut BMesh, a: VertId, b: VertId, flag_vert: VertId) -> EdgeId {
    if let Some(e) = bm.edge_exists(a, b) {
        return e;
    }
    let fl = region_edge_flag(bm, flag_vert);
    let e = bm.edge_create(a, b, None, CreateFlag::empty());
    if let Some((enable, disable)) = fl {
        let ed = bm.edge_mut(e);
        ed.flag.insert(enable);
        ed.flag.remove(disable);
    }
    e
}

/// Runs `extrude_face_region` on `bm`.
pub fn extrude_face_region(
    bm: &mut BMesh,
    i: &ExtrudeFaceRegionIn,
) -> Result<ExtrudeFaceRegionOut, BmError> {
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "extrude_face_region: dead element {dead:?} in geom"
        )));
    }
    if let Some(dead) = i.edges_exclude.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "extrude_face_region: dead edge {dead:?} in edges_exclude"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

// Ported from blender/source/blender/bmesh/operators/bmo_extrude.cc
// (bmo_extrude_face_region_exec). No skin layer in frap (`CD_MVERT_SKIN`).
fn exec(bm: &mut BMesh, i: &ExtrudeFaceRegionIn) -> Result<ExtrudeFaceRegionOut, BmError> {
    for &g in &i.geom {
        if matches!(g, BMElem::Edge(_) | BMElem::Face(_)) {
            bm.oflag_enable(g, EXT_INPUT);
        }
    }

    // If one flagged face is bordered by an unflagged face, the originals are deleted unless
    // the caller explicitly asked to keep them.
    let mut delorig = false;
    if !i.use_keep_orig {
        for e in bm.edge_ids() {
            if !bm.oflag_test(e, EXT_INPUT) {
                continue;
            }
            let mut found = false;
            let mut edge_face_tot = 0;
            for f in bm.edge_faces(e) {
                if !bm.oflag_test(f, EXT_INPUT) {
                    found = true;
                    delorig = true;
                    break;
                }
                edge_face_tot += 1;
            }
            if edge_face_tot > 1 && !found {
                bm.oflag_enable(e, EXT_DEL);
            }
        }
    }

    // Vertices to delete (only those attached to geometry, #33651).
    for v in bm.vert_ids() {
        if bm.vert(v).e.is_none() {
            continue;
        }
        let mut found = bm
            .vert_edges(v)
            .into_iter()
            .any(|e| !bm.oflag_test(e, EXT_INPUT) || !bm.oflag_test(e, EXT_DEL));
        if !found {
            found = bm
                .vert_faces(v)
                .into_iter()
                .any(|f| !bm.oflag_test(f, EXT_INPUT));
        }
        if !found {
            bm.oflag_enable(v, EXT_DEL);
        }
    }

    for f in bm.face_ids() {
        if bm.oflag_test(f, EXT_INPUT) {
            bm.oflag_enable(f, EXT_DEL);
        }
    }

    // `delop` is initialised here (its slot holds the flagged elements now) and run after the
    // duplicate.
    let del_geom: Vec<BMElem> = if delorig {
        let mut g: Vec<BMElem> = bm
            .vert_ids()
            .into_iter()
            .filter(|&v| bm.oflag_test(v, EXT_DEL))
            .map(BMElem::Vert)
            .collect();
        g.extend(
            bm.edge_ids()
                .into_iter()
                .filter(|&e| bm.oflag_test(e, EXT_DEL))
                .map(BMElem::Edge),
        );
        g.extend(
            bm.face_ids()
                .into_iter()
                .filter(|&f| bm.oflag_test(f, EXT_DEL))
                .map(BMElem::Face),
        );
        g
    } else {
        Vec::new()
    };

    let dupe = duplicate(
        bm,
        &DuplicateIn {
            geom: i.geom.clone(),
            use_select_history: i.use_select_history,
            ..Default::default()
        },
    )?;

    // The active face follows its copy (the duplicate's history remap already does this when
    // `use_select_history`).
    if let Some(af) = bm.act_face
        && bm.faces.contains(af.0)
        && bm.oflag_test(af, EXT_INPUT)
        && let Some(&(_, copy)) = dupe.face_map.iter().find(|(s, _)| *s == af)
    {
        bm.act_face = Some(copy);
    }

    if delorig {
        delete(
            bm,
            &DeleteIn {
                geom: del_geom,
                context: "TAGGED_ONLY",
            },
        )?;
    }

    // Flip the input faces only when the originals are kept.
    if !delorig && !i.skip_input_flip {
        for f in bm.face_ids() {
            if bm.oflag_test(f, EXT_INPUT) {
                bm.face_normal_flip(f);
            }
        }
    }

    let mut average_normal = [0.0f32; 3];
    let mut dissolve_verts: Vec<VertId> = Vec::new();
    if i.use_dissolve_ortho_edges {
        for g in &dupe.geom {
            if let BMElem::Face(f) = *g {
                let n = bm.face(f).no;
                average_normal = [
                    average_normal[0] + n[0],
                    average_normal[1] + n[1],
                    average_normal[2] + n[2],
                ];
            }
        }
        let (n, len) = crate::query::geom::normalize_v3(average_normal);
        average_normal = n;
        if len == 0.0 {
            average_normal[2] = 1.0;
        }
    }

    let geom_out = dupe.geom.clone();
    let excluded = |e: EdgeId| i.edges_exclude.contains(&e);

    for &(e, e_new) in &dupe.boundary_map {
        frap_math::budget::check();
        if !bm.edges.contains(e.0) {
            continue;
        }
        // This should always be wire, so this is mainly a speed-up to avoid a map lookup.
        if bm.edge_is_wire(e) && excluded(e) {
            // The original edge was excluded: it would be a standalone wire edge (#30399).
            let [v1, v2] = bm.edge(e).v;
            bm.edge_kill(e);
            // Kill standalone vertices from this edge (#32341).
            for v in [v1, v2] {
                if bm.verts.contains(v.0) && bm.vert(v).e.is_none() {
                    bm.vert_kill(v);
                }
            }
            continue;
        }
        // Skip creating the face for excluded edges (#35503).
        if excluded(e) {
            continue;
        }

        let mut join_face: Option<FaceId> = None;
        if i.use_dissolve_ortho_edges && bm.edge_is_boundary(e) {
            let f = bm.lp(bm.edge(e).l.expect("boundary edge has a loop")).f;
            if crate::query::geom::dot(average_normal, bm.face(f).no).abs() <= 0.0001 {
                join_face = Some(f);
            }
        }

        let [ev1, ev2] = bm.edge(e).v;
        let [nv1, nv2] = bm.edge(e_new).v;
        let edge_normal_flip = if !i.use_normal_from_adjacent {
            // Orient the loop to give the same normal as a loop of `e_new` if it exists (one
            // of the region's faces), else the same normal as a loop of `e`, if it exists.
            !match bm.edge(e_new).l {
                Some(l) => bm.lp(l).v == nv1,
                None => match bm.edge(e).l {
                    None => true,
                    Some(l) => bm.lp(l).v != ev1,
                },
            }
        } else {
            // Special case for repetitive extrusions that use the normals of the previously
            // created faces.
            !bm.edge(e).l.is_some_and(|l| ev1 != bm.lp(l).v)
        };
        let f_verts: [VertId; 4] = if edge_normal_flip == i.use_normal_flip {
            [ev1, ev2, nv2, nv1]
        } else {
            [ev2, ev1, nv1, nv2]
        };

        // Handle the new edges (USE_EDGE_REGION_FLAGS).
        let e1 = side_edge(bm, f_verts[1], f_verts[2], f_verts[2]);
        let e3 = side_edge(bm, f_verts[3], f_verts[0], f_verts[3]);
        let f = bm
            .face_create(&f_verts, &[e, e1, e_new, e3], None, CreateFlag::empty())
            .expect("four corners");
        copy_face_loop_attributes(bm, f);

        if let Some(jf) = join_face {
            for v in [ev1, ev2] {
                if !bm.oflag_test(v, EXT_TAG) {
                    bm.oflag_enable(v, EXT_TAG);
                    dissolve_verts.push(v);
                }
            }
            // Tag the edges that can collapse.
            bm.oflag_enable(e, EXT_TAG);
            bm.oflag_enable(e1, EXT_TAG);
            bm.join_face_kill_edge(jf, f, e);
        }
    }

    // Link the isolated vertices.
    for &(v_src, v_copy) in &dupe.isovert_map {
        let (mut a, mut b) = (v_src, v_copy);
        // Not essential, but ensures face normals from extruded edges are contiguous.
        if vert_is_wire_endpoint(bm, a) {
            let e = bm.vert(a).e.expect("wire endpoint has an edge");
            if bm.edge(e).v[0] == a {
                std::mem::swap(&mut a, &mut b);
            }
        }
        bm.edge_create(a, b, None, CreateFlag::NO_DOUBLE);
    }

    for v in dissolve_verts {
        if !bm.verts.contains(v.0) {
            continue;
        }
        let Some(e) = bm.vert(v).e else {
            continue;
        };
        let e_other = bm.disk_edge_next(e, v);
        if e_other == e || bm.disk_edge_next(e_other, v) == e {
            // Loose edge or the vertex is an edge pair.
            let e_kill = if bm.oflag_test(e, EXT_TAG) {
                e
            } else {
                e_other
            };
            bm.join_vert_kill_edge(e_kill, v, true, true, true);
        }
    }

    debug_assert!(geom_out.iter().all(|&g| bm.elem_alive(g)));
    Ok(ExtrudeFaceRegionOut { geom: geom_out })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two quads sharing the edge 1–2: `[0,1,2,3]` and `[1,4,5,2]`, normals +Z.
    fn strip() -> (BMesh, Vec<VertId>, [FaceId; 2]) {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [2.0, 0.0, 0.0],
            [2.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
        let a = bm
            .face_create_verts(&[v[0], v[1], v[2], v[3]], None, CreateFlag::empty(), true)
            .unwrap();
        let b = bm
            .face_create_verts(&[v[1], v[4], v[5], v[2]], None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        (bm, v, [a, b])
    }

    fn face_geom(bm: &BMesh, f: FaceId) -> Vec<BMElem> {
        let mut g: Vec<BMElem> = bm.face_verts(f).into_iter().map(BMElem::Vert).collect();
        g.extend(bm.face_edges(f).into_iter().map(BMElem::Edge));
        g.push(BMElem::Face(f));
        g
    }

    #[test]
    fn face_with_a_neighbour_is_moved_to_the_cap() {
        let (mut bm, _, [a, _]) = strip();
        let geom = face_geom(&bm, a);
        let out = extrude_face_region(
            &mut bm,
            &ExtrudeFaceRegionIn {
                geom,
                ..Default::default()
            },
        )
        .unwrap();
        // The input face is deleted and replaced by its copy, plus 4 side quads.
        assert_eq!(
            (bm.verts_num(), bm.edges_num(), bm.faces_num()),
            (10, 15, 6)
        );
        let caps: Vec<FaceId> = out
            .geom
            .iter()
            .filter_map(|g| match g {
                BMElem::Face(f) => Some(*f),
                _ => None,
            })
            .collect();
        assert_eq!(caps, vec![FaceId(2)]);
        // The first side quad reuses the deleted face's slot.
        assert!(bm.faces.contains(a.0));
        for e in bm.edge_ids() {
            assert!(bm.edge_face_count(e) <= 2);
        }
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn isolated_region_is_kept_and_flipped() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]]
            .into_iter()
            .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
            .collect();
        let f = bm
            .face_create_verts(&v, None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        let geom = face_geom(&bm, f);
        let out = extrude_face_region(
            &mut bm,
            &ExtrudeFaceRegionIn {
                geom,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((bm.verts_num(), bm.edges_num(), bm.faces_num()), (6, 9, 5));
        assert_eq!(bm.face(f).no, [0.0, 0.0, -1.0]);
        assert_eq!(out.geom.len(), 3 + 3 + 1);
        for e in bm.edge_ids() {
            assert!(bm.edge_is_contiguous(e), "{e:?}");
        }
    }

    #[test]
    fn single_vertex_becomes_a_wire_edge() {
        let (mut bm, v, _) = strip();
        let out = extrude_face_region(
            &mut bm,
            &ExtrudeFaceRegionIn {
                geom: vec![BMElem::Vert(v[0])],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.geom, vec![BMElem::Vert(VertId(6))]);
        assert!(bm.edge_exists(v[0], VertId(6)).is_some());
        assert_eq!(bm.faces_num(), 2);
    }

    #[test]
    fn excluded_edges_get_no_side_quad() {
        let (mut bm, v, [a, _]) = strip();
        let excl = bm.edge_exists(v[0], v[3]).unwrap();
        let geom = face_geom(&bm, a);
        extrude_face_region(
            &mut bm,
            &ExtrudeFaceRegionIn {
                geom,
                edges_exclude: vec![excl],
                ..Default::default()
            },
        )
        .unwrap();
        // The neighbour, the cap and three side quads: none over the excluded edge.
        assert_eq!(bm.faces_num(), 5);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn dissolve_ortho_edges_folds_the_sides_into_the_walls() {
        // The cube's top face: every side quad is perpendicular to the cap, so it is joined
        // into the wall below and the old top corners collapse into the cap's (V8 E12 F6, as
        // `mesh.extrude_manifold` before its translation).
        let me = frap_mesh::primitives::cube::cube(2.0, false, [1.0; 3]);
        let mut bm = BMesh::from_mesh(&me, &crate::FromMeshParams::default());
        bm.normals_update();
        let top = bm
            .face_ids()
            .into_iter()
            .find(|&f| bm.face(f).no[2] > 0.5)
            .unwrap();
        let geom = face_geom(&bm, top);
        let out = extrude_face_region(
            &mut bm,
            &ExtrudeFaceRegionIn {
                geom,
                use_dissolve_ortho_edges: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!((bm.verts_num(), bm.edges_num(), bm.faces_num()), (8, 12, 6));
        assert!(out.geom.iter().all(|&g| bm.elem_alive(g)));
        assert!(bm.validate().is_empty());
    }
}
