// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.convex_hull` (Blender 5.2.4 `bmo_convex_hull_exec`, `operators/bmo_hull.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.convex_hull(bmesh, input=[], use_existing_faces=False)`.
//!
//! The hull comes from a port of Bullet's `btConvexHullComputer` (`convex_hull/bullet.rs`, what
//! Blender calls through `plConvexHullCompute`): its planar polygons (two-sided for a planar
//! input) are fanned from their first vertex into hull triangles, exactly as
//! `hull_from_bullet` does. Everything after the hull is Blender's.
//!
//! **Create/kill order**: one face per hull triangle (`BM_face_create_verts`,
//! `BM_CREATE_NO_DOUBLE`, edges created in `(v0, v1), (v1, v2), (v2, v0)` order, attributes from
//! a neighbouring input face, loop data from shared edges), skipped with `use_existing_faces`
//! when an existing face already covers it. Nothing is killed here: `geom_unused.out` and
//! `geom_holes.out` list what the `mesh.convex_hull` operator deletes.
//!
//! Errors: `Requires at least three vertices` (`BMO_ERROR_CANCEL`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use rustc_hash::FxHashSet;

use crate::ops::contextual_create::construct;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, VertId};

/// Input slots of `convex_hull` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConvexHullIn {
    /// `input`.
    pub input: Vec<BMElem>,
    /// `use_existing_faces`.
    pub use_existing_faces: bool,
}

/// Output slots of `convex_hull`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct ConvexHullOut {
    /// `geom.out`: every hull vertex, edge and face (mesh order).
    pub geom: Vec<BMElem>,
    /// `geom_interior.out`: input elements inside the hull.
    pub geom_interior: Vec<BMElem>,
    /// `geom_unused.out`: interior input elements used by nothing outside the input.
    pub geom_unused: Vec<BMElem>,
    /// `geom_holes.out`: input faces/edges on the hull that the hull replaces.
    pub geom_holes: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_convex_hull_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const HULL_FLAG_INPUT: u16 = 1 << 0;
const HULL_FLAG_INTERIOR_ELE: u16 = 1 << 1;
const HULL_FLAG_OUTPUT_GEOM: u16 = 1 << 2;
const HULL_FLAG_DEL: u16 = 1 << 3;
const HULL_FLAG_HOLE: u16 = 1 << 4;

pub mod bullet;

/// Runs `convex_hull` on `bm`. `Requires at least three vertices` is a [`BmError::Cancel`].
pub fn convex_hull(bm: &mut BMesh, i: &ConvexHullIn) -> Result<ConvexHullOut, BmError> {
    if let Some(dead) = i.input.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!("convex_hull: dead element {dead:?}")));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i))
}

#[derive(Clone, Debug)]
struct HullTriangle {
    v: [VertId; 3],
    skip: bool,
}

/// `hull_find_example_face`.
fn find_example_face(bm: &BMesh, e: EdgeId) -> Option<FaceId> {
    bm.edge_faces(e).into_iter().find(|&f| {
        bm.oflag_test(f, HULL_FLAG_INPUT) || !bm.oflag_test(f, HULL_FLAG_OUTPUT_GEOM)
    })
}

fn ord(a: VertId, b: VertId) -> (VertId, VertId) {
    if a > b { (b, a) } else { (a, b) }
}

// Ported from blender/source/blender/bmesh/operators/bmo_hull.cc (bmo_convex_hull_exec and
// its helpers).
fn exec(bm: &mut BMesh, i: &ConvexHullIn) -> Result<ConvexHullOut, BmError> {
    let in_verts: Vec<VertId> = i
        .input
        .iter()
        .filter_map(|g| match g {
            BMElem::Vert(v) => Some(*v),
            _ => None,
        })
        .collect();
    if in_verts.len() < 3 {
        return Err(BmError::Cancel("Requires at least three vertices".into()));
    }
    for &g in &i.input {
        match g {
            BMElem::Vert(v) => bm.oflag_enable(v, HULL_FLAG_INPUT | HULL_FLAG_INTERIOR_ELE),
            _ => bm.oflag_enable(g, HULL_FLAG_INPUT),
        }
    }
    // `hull_from_bullet`: Bullet's polygons fanned from their first vertex.
    let coords: Vec<[f32; 3]> = in_verts.iter().map(|&v| bm.vert(v).co).collect();
    let hull = bullet::compute(&coords);
    let hull_verts: Vec<VertId> = hull.original_index.iter().map(|&k| in_verts[k]).collect();
    let mut tris: Vec<HullTriangle> = Vec::new();
    for fvi in &hull.faces {
        if fvi.len() > 2 {
            for j in 2..fvi.len() {
                let v = [hull_verts[fvi[0]], hull_verts[fvi[j - 1]], hull_verts[fvi[j]]];
                // `hull_add_triangle`.
                for &x in &v {
                    bm.oflag_disable(x, HULL_FLAG_INTERIOR_ELE);
                }
                tris.push(HullTriangle { v, skip: false });
            }
        }
    }
    // `hull_final_edges`.
    let mut final_edges: FxHashSet<(VertId, VertId)> = FxHashSet::default();
    for t in &tris {
        for k in 0..3 {
            final_edges.insert(ord(t.v[k], t.v[(k + 1) % 3]));
        }
    }
    let on_hull = |a: VertId, b: VertId| final_edges.contains(&ord(a, b));
    // `hull_mark_interior_elements`.
    for &g in &i.input {
        match g {
            BMElem::Edge(e) => {
                let [a, b] = bm.edge(e).v;
                if !on_hull(a, b) {
                    bm.oflag_enable(e, HULL_FLAG_INTERIOR_ELE);
                }
            }
            BMElem::Face(f) => bm.oflag_enable(f, HULL_FLAG_INTERIOR_ELE),
            BMElem::Vert(_) => {}
        }
    }
    if i.use_existing_faces {
        // `hull_remove_overlapping`.
        for t in &mut tris {
            for f in bm.vert_faces(t.v[0]) {
                let f_on_hull = bm.face_edges(f).into_iter().all(|e| {
                    let [a, b] = bm.edge(e).v;
                    on_hull(a, b)
                });
                if construct::vert_in_face(bm, t.v[1], f)
                    && construct::vert_in_face(bm, t.v[2], f)
                    && f_on_hull
                {
                    t.skip = true;
                    bm.oflag_disable(f, HULL_FLAG_INTERIOR_ELE);
                    bm.oflag_enable(f, HULL_FLAG_HOLE);
                }
            }
        }
        // `hull_tag_holes`.
        for &g in &i.input {
            if let BMElem::Face(f) = g
                && bm.oflag_test(f, HULL_FLAG_HOLE)
                && bm.face_edges(f).into_iter().any(|e| bm.edge_is_boundary(e))
            {
                bm.oflag_disable(f, HULL_FLAG_HOLE);
            }
        }
        for &g in &i.input {
            if let BMElem::Edge(e) = g {
                let fs = bm.edge_faces(e);
                if !fs.is_empty() && fs.iter().all(|&f| bm.oflag_test(f, HULL_FLAG_HOLE)) {
                    bm.oflag_enable(e, HULL_FLAG_HOLE);
                }
            }
        }
    }
    // `hull_output_triangles`.
    for t in &tris {
        if !t.skip {
            let edges = [
                bm.edge_create(t.v[0], t.v[1], None, CreateFlag::NO_DOUBLE),
                bm.edge_create(t.v[1], t.v[2], None, CreateFlag::NO_DOUBLE),
                bm.edge_create(t.v[2], t.v[0], None, CreateFlag::NO_DOUBLE),
            ];
            let f = match bm.face_exists(&t.v) {
                Some(f) => {
                    bm.oflag_enable(f, HULL_FLAG_OUTPUT_GEOM);
                    bm.oflag_disable(f, HULL_FLAG_HOLE);
                    bm.oflag_disable(f, HULL_FLAG_INTERIOR_ELE);
                    f
                }
                None => {
                    let mut example = None;
                    for &e in &edges {
                        if example.is_none() {
                            example = find_example_face(bm, e);
                        }
                    }
                    let f = bm
                        .face_create_verts(&t.v, example, CreateFlag::NO_DOUBLE, true)
                        .expect("hull triangle");
                    construct::face_copy_shared(bm, f, None);
                    f
                }
            };
            bm.oflag_enable(f, HULL_FLAG_OUTPUT_GEOM);
            bm.face_select_set(f, true);
            for &e in &edges {
                bm.oflag_enable(e, HULL_FLAG_OUTPUT_GEOM);
            }
        } else {
            for k in 0..3 {
                if let Some(e) = bm.edge_exists(t.v[k], t.v[(k + 1) % 3])
                    && bm.oflag_test(e, HULL_FLAG_INPUT)
                    && !bm.oflag_test(e, HULL_FLAG_HOLE)
                {
                    bm.oflag_enable(e, HULL_FLAG_OUTPUT_GEOM);
                }
            }
        }
        for &v in &t.v {
            bm.oflag_enable(v, HULL_FLAG_OUTPUT_GEOM);
        }
    }
    // `hull_tag_unused`.
    for &g in &i.input {
        match g {
            BMElem::Vert(v) => {
                if bm.oflag_test(v, HULL_FLAG_INTERIOR_ELE) {
                    let del = bm
                        .vert_edges(v)
                        .into_iter()
                        .all(|e| bm.oflag_test(e, HULL_FLAG_INPUT))
                        && bm
                            .vert_faces(v)
                            .into_iter()
                            .all(|f| bm.oflag_test(f, HULL_FLAG_INPUT));
                    if del {
                        bm.oflag_enable(v, HULL_FLAG_DEL);
                    }
                }
            }
            BMElem::Edge(e) => {
                if bm.oflag_test(e, HULL_FLAG_INTERIOR_ELE)
                    && bm
                        .edge_faces(e)
                        .into_iter()
                        .all(|f| bm.oflag_test(f, HULL_FLAG_INPUT))
                {
                    bm.oflag_enable(e, HULL_FLAG_DEL);
                }
            }
            BMElem::Face(f) => {
                if bm.oflag_test(f, HULL_FLAG_INTERIOR_ELE) {
                    bm.oflag_enable(f, HULL_FLAG_DEL);
                }
            }
        }
    }
    let collect = |bm: &BMesh, flag: u16| -> Vec<BMElem> {
        let mut out: Vec<BMElem> = Vec::new();
        out.extend(bm.vert_ids().into_iter().filter(|&v| bm.oflag_test(v, flag)).map(BMElem::Vert));
        out.extend(bm.edge_ids().into_iter().filter(|&e| bm.oflag_test(e, flag)).map(BMElem::Edge));
        out.extend(bm.face_ids().into_iter().filter(|&f| bm.oflag_test(f, flag)).map(BMElem::Face));
        out
    };
    Ok(ConvexHullOut {
        geom_interior: collect(bm, HULL_FLAG_INTERIOR_ELE),
        geom_unused: collect(bm, HULL_FLAG_DEL),
        geom_holes: collect(bm, HULL_FLAG_HOLE),
        geom: collect(bm, HULL_FLAG_OUTPUT_GEOM),
    })
}
