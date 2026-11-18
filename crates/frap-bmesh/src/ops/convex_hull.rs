// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.convex_hull` (Blender 5.2.4 `bmo_convex_hull_exec`, `operators/bmo_hull.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.convex_hull(bmesh, input=[], use_existing_faces=False)`.
//!
//! The hull itself comes from an incremental 3D convex hull in `f64` (`hull3d`, in the input
//! vertex order) instead of Bullet's `btConvexHullComputer`: for points in general position
//! the hull triangles are the same; where four or more hull points are coplanar Blender fans
//! Bullet's polygon from its first vertex while frap keeps its own triangulation of that
//! facet (deviation recorded in `docs/deviations.md`). Everything after the hull is Blender's.
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

use rustc_hash::{FxHashMap, FxHashSet};

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

type P = [f64; 3];

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: P, b: P) -> P {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Incremental convex hull: outward-wound triangles over `pts` indices; `None` when the points
/// do not span a volume.
pub fn hull3d(pts: &[P]) -> Option<Vec<[usize; 3]>> {
    let n = pts.len();
    if n < 4 {
        return None;
    }
    let mut scale = 0.0f64;
    for p in pts {
        for c in p {
            scale = scale.max(c.abs());
        }
    }
    let eps = 1e-9 * scale.max(1.0);
    // Initial tetrahedron: first point, farthest from it, farthest from that line, farthest
    // from that plane.
    let i0 = 0usize;
    let i1 = (0..n).max_by(|&a, &b| {
        let da = dot(sub(pts[a], pts[i0]), sub(pts[a], pts[i0]));
        let db = dot(sub(pts[b], pts[i0]), sub(pts[b], pts[i0]));
        da.total_cmp(&db).then(b.cmp(&a))
    })?;
    let line = sub(pts[i1], pts[i0]);
    let i2 = (0..n).max_by(|&a, &b| {
        let ca = cross(line, sub(pts[a], pts[i0]));
        let cb = cross(line, sub(pts[b], pts[i0]));
        dot(ca, ca).total_cmp(&dot(cb, cb)).then(b.cmp(&a))
    })?;
    let nrm = cross(line, sub(pts[i2], pts[i0]));
    if dot(nrm, nrm).sqrt() <= eps * eps.max(1e-300) {
        return None;
    }
    let i3 = (0..n).max_by(|&a, &b| {
        let da = dot(nrm, sub(pts[a], pts[i0])).abs();
        let db = dot(nrm, sub(pts[b], pts[i0])).abs();
        da.total_cmp(&db).then(b.cmp(&a))
    })?;
    let vol = dot(nrm, sub(pts[i3], pts[i0]));
    let nl = dot(nrm, nrm).sqrt();
    if (vol / nl).abs() <= eps {
        return None;
    }
    let mut faces: Vec<[usize; 3]> = if vol < 0.0 {
        vec![[i0, i1, i2], [i0, i3, i1], [i1, i3, i2], [i2, i3, i0]]
    } else {
        vec![[i0, i2, i1], [i0, i1, i3], [i1, i2, i3], [i2, i0, i3]]
    };
    let outside = |f: &[usize; 3], p: P| -> f64 {
        let nn = cross(sub(pts[f[1]], pts[f[0]]), sub(pts[f[2]], pts[f[0]]));
        let l = dot(nn, nn).sqrt();
        if l == 0.0 {
            return 0.0;
        }
        dot(nn, sub(p, pts[f[0]])) / l
    };
    for (pi, &p) in pts.iter().enumerate() {
        if [i0, i1, i2, i3].contains(&pi) {
            continue;
        }
        let visible: Vec<bool> = faces.iter().map(|f| outside(f, p) > eps).collect();
        if !visible.iter().any(|&v| v) {
            continue;
        }
        // Horizon: directed edges of visible faces whose reverse belongs to a hidden face.
        let mut edge_owner: FxHashMap<(usize, usize), usize> = FxHashMap::default();
        for (fi, f) in faces.iter().enumerate() {
            for k in 0..3 {
                edge_owner.insert((f[k], f[(k + 1) % 3]), fi);
            }
        }
        let mut new_faces: Vec<[usize; 3]> = Vec::new();
        for (fi, f) in faces.iter().enumerate() {
            if !visible[fi] {
                new_faces.push(*f);
            }
        }
        for (fi, f) in faces.iter().enumerate() {
            if !visible[fi] {
                continue;
            }
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                let other = edge_owner.get(&(b, a)).copied();
                if other.is_none_or(|o| !visible[o]) {
                    new_faces.push([a, b, pi]);
                }
            }
        }
        faces = new_faces;
    }
    Some(faces)
}

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
    // `hull_from_bullet`.
    let pts: Vec<P> = in_verts
        .iter()
        .map(|&v| bm.vert(v).co.map(f64::from))
        .collect();
    let mut tris: Vec<HullTriangle> = Vec::new();
    if let Some(faces) = hull3d(&pts) {
        for f in faces {
            let v = f.map(|k| in_verts[k]);
            for &x in &v {
                bm.oflag_disable(x, HULL_FLAG_INTERIOR_ELE);
            }
            tris.push(HullTriangle { v, skip: false });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_points_give_twelve_triangles() {
        let mut pts = Vec::new();
        for x in [-1.0, 1.0] {
            for y in [-1.0, 1.0] {
                for z in [-1.0, 1.0] {
                    pts.push([x, y, z]);
                }
            }
        }
        pts.push([0.0, 0.0, 0.0]);
        let f = hull3d(&pts).unwrap();
        assert_eq!(f.len(), 12);
        assert!(f.iter().all(|t| !t.contains(&8)));
    }
}
