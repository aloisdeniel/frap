// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.intersect_boolean(operation='DIFFERENCE', use_swap=False, use_self=False,
//! threshold=1e-06, solver='EXACT')` (Intersect (Boolean)), tier T3.
//!
//! Port of `edbm_intersect_boolean_exec` and `edbm_intersect_select`
//! (blender/source/blender/editors/mesh/editmesh_intersect.cc) and of the BMesh side of
//! `BM_mesh_boolean` (blender/source/blender/bmesh/tools/bmesh_boolean.cc:
//! `apply_mesh_output_to_bmesh`), Blender 5.2.4, GPL-2.0-or-later. For every mesh in Edit Mode
//! with selected faces, the unselected visible faces (side A) and the selected ones (side B;
//! swapped by `use_swap`) are combined by the boolean `operation`; hidden faces take no part
//! and are kept. The result is written back into the BMesh as Blender does: vertices and
//! faces that survive unchanged are kept, new faces copy the attributes and interpolate the
//! corner data (UVs) of the face they come from, new edges those of the edge they lie on,
//! and the edges along the intersection are tagged (`get_cdt_edge_orig`'s `is_intersect`:
//! between the two sides, on no edge of its own input face nor of a coplanar one, so a cutter
//! that only touches the mesh's faces tags nothing). When something changed, everything is
//! deselected and, in vertex or edge select mode, the intersection edges are selected. When
//! nothing changed on any mesh: WARNING `No intersections found`.
//!
//! The boolean is `frap_boolean`'s EXACT stand-in (manifold-rust's exact robust engine, see
//! `frap_boolean::manifold`): Blender's exact mesh arrangement is not ported (WP 8.6), so the
//! result has Blender's volume but not its face layout (T3), both sides must be closed
//! (an open side is `OP_NOT_IMPLEMENTED`), and `use_self` does not change the result (each
//! side is a solid by winding number). `solver='FLOAT'` (Blender's `BM_mesh_intersect`) runs
//! the same solver and adds a WARNING until M8; `threshold` is FLOAT's and unused.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use frap_spec_ops_mesh::mesh::intersect_boolean::{Operation, Solver};

#[operator(
    poll = poll::EDITMESH,
    tier = T3,
    support = Partial(
        "EXACT runs manifold-rust's exact robust boolean (Blender's exact mesh arrangement is \
         not ported, WP 8.6): Blender's volume, not its face layout; the selected and \
         unselected parts must each be closed (OP_NOT_IMPLEMENTED otherwise); use_self does \
         not change the result; FLOAT runs EXACT with a WARNING"
    ),
)]
fn exec(cx: &mut OpCtx, p: mesh::IntersectBoolean) -> OpResult {
    let op = match p.operation.value {
        Operation::Intersect => frap_boolean::BoolOperation::Intersect,
        Operation::Union => frap_boolean::BoolOperation::Union,
        Operation::Difference => frap_boolean::BoolOperation::Difference,
    };
    if p.solver.value == Solver::Float {
        cx.report(ReportType::Warning, common::FLOAT_WARNING);
    }
    let side = if p.use_swap.value {
        common::side_pair_swap
    } else {
        common::side_pair
    };
    let objects = helpers::iter_edit_meshes(cx);
    let mut isect_len = 0;
    for (_, me) in &objects {
        let bm = cx.bmesh_mut(*me)?;
        if bm.selected_counts().2 == 0 {
            continue;
        }
        let has_isect = common::boolean(bm, side, op)?;
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

/// The BMesh side of Blender's mesh boolean and knife, shared with `mesh.intersect`.
pub(crate) mod common {
    use frap_bmesh::{BMElem, BMesh, CreateFlag, EdgeId, ElemFlag, FaceId, SelectMode, VertId};
    use frap_boolean::{BoolMesh, BoolOperation, BoolOptions, Src};
    use frap_op::OpError;
    use rustc_hash::{FxHashMap, FxHashSet};
    use smallvec::SmallVec;

    /// The WARNING of `solver='FLOAT'`.
    pub(crate) const FLOAT_WARNING: &str = "frap: the FLOAT solver (BM_mesh_intersect) is not \
        ported until M8; the EXACT solver was used";

    /// `bm_face_isect_pair`: hidden faces take no part, selected ones are side 1, the others
    /// side 0.
    pub(crate) fn side_pair(bm: &BMesh, f: FaceId) -> i32 {
        let fl = bm.face(f).flag;
        if fl.contains(ElemFlag::HIDDEN) {
            -1
        } else if fl.contains(ElemFlag::SELECT) {
            1
        } else {
            0
        }
    }

    /// `bm_face_isect_pair_swap`.
    pub(crate) fn side_pair_swap(bm: &BMesh, f: FaceId) -> i32 {
        let fl = bm.face(f).flag;
        if fl.contains(ElemFlag::HIDDEN) {
            -1
        } else if fl.contains(ElemFlag::SELECT) {
            0
        } else {
            1
        }
    }

    /// `bm_face_isect_self`: the selected faces are the one side.
    pub(crate) fn side_self(bm: &BMesh, f: FaceId) -> i32 {
        if bm.face(f).flag.contains(ElemFlag::SELECT) {
            0
        } else {
            -1
        }
    }

    /// One side of the operation as plain arrays (local vertex indices).
    pub(crate) struct Shape {
        pub(crate) faces: Vec<FaceId>,
        pub(crate) verts: Vec<VertId>,
        positions: Vec<[f32; 3]>,
        face_offsets: Vec<u32>,
        corner_verts: Vec<u32>,
        tris: Vec<[u32; 3]>,
    }

    impl Shape {
        pub(crate) fn bool_mesh(&self) -> BoolMesh<'_> {
            BoolMesh::new(&self.positions, &self.face_offsets, &self.corner_verts)
                .with_corner_tris(&self.tris)
        }
    }

    /// The faces of each side (`0..nshapes`) in face order, with Blender's tessellation
    /// (`mesh_calc_tessellation`: quads split on the shorter-safe diagonal, n-gons by
    /// polyfill) for the solver. Also returns the faces taking no part.
    pub(crate) fn shapes(
        bm: &BMesh,
        side: fn(&BMesh, FaceId) -> i32,
        nshapes: usize,
    ) -> (Vec<Shape>, Vec<FaceId>) {
        let mut out: Vec<Shape> = (0..nshapes)
            .map(|_| Shape {
                faces: Vec::new(),
                verts: Vec::new(),
                positions: Vec::new(),
                face_offsets: vec![0],
                corner_verts: Vec::new(),
                tris: Vec::new(),
            })
            .collect();
        let mut local: Vec<FxHashMap<VertId, u32>> = vec![FxHashMap::default(); nshapes];
        let mut rest = Vec::new();
        for f in bm.face_ids() {
            let s = side(bm, f);
            if s < 0 || s as usize >= nshapes {
                rest.push(f);
                continue;
            }
            let s = s as usize;
            let sh = &mut out[s];
            let start = sh.corner_verts.len() as u32;
            for v in bm.face_verts(f) {
                let i = *local[s].entry(v).or_insert_with(|| {
                    sh.verts.push(v);
                    sh.positions.push(bm.vert(v).co);
                    sh.verts.len() as u32 - 1
                });
                sh.corner_verts.push(i);
            }
            let n = sh.corner_verts.len() as u32 - start;
            let fv: Vec<i32> = sh.corner_verts[start as usize..]
                .iter()
                .map(|&v| v as i32)
                .collect();
            let p = |i: usize| sh.positions[fv[i] as usize];
            match n {
                0..=2 => {}
                3 => sh.tris.push([start, start + 1, start + 2]),
                4 => {
                    if frap_geom::polyfill::is_quad_flip_v3_first_third_fast(p(0), p(1), p(2), p(3))
                    {
                        sh.tris.push([start, start + 1, start + 3]);
                        sh.tris.push([start + 1, start + 2, start + 3]);
                    } else {
                        sh.tris.push([start, start + 1, start + 2]);
                        sh.tris.push([start, start + 2, start + 3]);
                    }
                }
                _ => {
                    for t in frap_geom::polyfill::polyfill_mesh_face(&sh.positions, &fv) {
                        sh.tris.push([start + t[0], start + t[1], start + t[2]]);
                    }
                }
            }
            sh.faces.push(f);
            sh.face_offsets.push(sh.corner_verts.len() as u32);
        }
        (out, rest)
    }

    /// A result to write into the BMesh (`IMesh m_out`): every vertex with its original
    /// vertex, every face with its original face, and per face edge (`v[i]` to `v[i + 1]`)
    /// the original edge it lies on and whether it is on an intersection.
    #[derive(Default)]
    pub(crate) struct OutMesh {
        pub(crate) positions: Vec<[f32; 3]>,
        pub(crate) vert_orig: Vec<Option<VertId>>,
        pub(crate) faces: Vec<SmallVec<[u32; 8]>>,
        pub(crate) face_orig: Vec<Option<FaceId>>,
        pub(crate) edge_orig: Vec<SmallVec<[Option<EdgeId>; 8]>>,
        pub(crate) is_intersect: Vec<SmallVec<[bool; 8]>>,
    }

    /// Whether `p` lies on the segment `a`-`b` (relative tolerance).
    fn on_segment(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> bool {
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let r = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
        let dd = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
        if dd == 0.0 {
            return false;
        }
        let t = (r[0] * d[0] + r[1] * d[1] + r[2] * d[2]) / dd;
        if !(-1e-6..=1.0 + 1e-6).contains(&t) {
            return false;
        }
        let c = [r[0] - d[0] * t, r[1] - d[1] * t, r[2] - d[2] * t];
        c[0] * c[0] + c[1] * c[1] + c[2] * c[2] <= 1e-10 * dd
    }

    /// The original edge an output edge lies on (`face.edge_orig`): the edge between its
    /// original vertices, else the edge of its original face it runs along.
    fn edge_orig(
        bm: &BMesh,
        face: Option<FaceId>,
        (pa, va): ([f32; 3], Option<VertId>),
        (pb, vb): ([f32; 3], Option<VertId>),
    ) -> Option<EdgeId> {
        if let (Some(a), Some(b)) = (va, vb)
            && let Some(e) = bm.edge_exists(a, b)
        {
            return Some(e);
        }
        let f = face?;
        bm.face_edges(f).into_iter().find(|&e| {
            let [u, w] = bm.edge(e).v;
            let (cu, cw) = (bm.vert(u).co, bm.vert(w).co);
            on_segment(pa, cu, cw) && on_segment(pb, cu, cw)
        })
    }

    /// Builds the [`OutMesh`] of a solver result given as flat arrays.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn out_mesh(
        bm: &BMesh,
        shapes: &[Shape],
        positions: &[[f32; 3]],
        vert_src: &[Option<Src>],
        face_offsets: &[u32],
        corner_verts: &[u32],
        face_src: &[Src],
        is_isect: &dyn Fn(u32, u32) -> bool,
    ) -> OutMesh {
        let mut out = OutMesh {
            positions: positions.to_vec(),
            vert_orig: vert_src
                .iter()
                .map(|s| s.map(|(m, v)| shapes[m as usize].verts[v as usize]))
                .collect(),
            ..Default::default()
        };
        for (f, w) in face_offsets.windows(2).enumerate() {
            let fv: SmallVec<[u32; 8]> = corner_verts[w[0] as usize..w[1] as usize].into();
            let (m, lf) = face_src[f];
            let orig = shapes[m as usize].faces.get(lf as usize).copied();
            let k = fv.len();
            let mut eo = SmallVec::new();
            let mut ii = SmallVec::new();
            for i in 0..k {
                let (a, b) = (fv[i], fv[(i + 1) % k]);
                eo.push(edge_orig(
                    bm,
                    orig,
                    (positions[a as usize], out.vert_orig[a as usize]),
                    (positions[b as usize], out.vert_orig[b as usize]),
                ));
                ii.push(is_isect(a, b));
            }
            out.faces.push(fv);
            out.face_orig.push(orig);
            out.edge_orig.push(eo);
            out.is_intersect.push(ii);
        }
        out
    }

    /// `BM_vert_is_wire`.
    fn vert_is_wire(bm: &BMesh, v: VertId) -> bool {
        bm.vert(v).e.is_some() && bm.vert_edges(v).iter().all(|&e| bm.edge(e).l.is_none())
    }

    // Ported from blender/source/blender/bmesh/tools/bmesh_boolean.cc
    // (apply_mesh_output_to_bmesh), Blender 5.2.4, GPL-2.0-or-later.
    /// `apply_mesh_output_to_bmesh`: makes `bm` match `out`. Returns whether anything changed.
    /// `passthrough` faces (no side) are kept with their vertices.
    pub(crate) fn apply(
        bm: &mut BMesh,
        out: &OutMesh,
        keep_hidden: bool,
        passthrough: &[FaceId],
    ) -> bool {
        let mut any_change = false;
        let mut keep_v: FxHashSet<VertId> = FxHashSet::default();
        for v in bm.vert_ids() {
            let hidden = bm.vert(v).flag.contains(ElemFlag::HIDDEN)
                || bm
                    .vert_faces(v)
                    .iter()
                    .any(|&f| bm.face(f).flag.contains(ElemFlag::HIDDEN));
            if (keep_hidden && hidden) || vert_is_wire(bm, v) {
                keep_v.insert(v);
            }
        }
        let mut keep_f: FxHashSet<FaceId> = FxHashSet::default();
        for &f in passthrough {
            keep_f.insert(f);
            keep_v.extend(bm.face_verts(f));
        }
        if keep_hidden {
            for f in bm.face_ids() {
                if bm.face(f).flag.contains(ElemFlag::HIDDEN) {
                    keep_f.insert(f);
                }
            }
        }
        let mut new_v: Vec<VertId> = Vec::with_capacity(out.positions.len());
        for (i, o) in out.vert_orig.iter().enumerate() {
            let v = match o {
                Some(v) => *v,
                None => {
                    any_change = true;
                    bm.vert_create(out.positions[i], None, CreateFlag::empty())
                }
            };
            keep_v.insert(v);
            new_v.push(v);
        }
        for (fi, f) in out.faces.iter().enumerate() {
            let verts: Vec<VertId> = f.iter().map(|&v| new_v[v as usize]).collect();
            if let Some(ex) = bm.face_exists(&verts) {
                keep_f.insert(ex);
                continue;
            }
            let k = verts.len();
            let mut edges = Vec::with_capacity(k);
            for i in 0..k {
                let (a, b) = (verts[i], verts[(i + 1) % k]);
                let e = match bm.edge_exists(a, b) {
                    Some(e) => e,
                    None => {
                        let orig = out.edge_orig[fi][i].filter(|&e| bm.edges.contains(e.0));
                        let e = bm.edge_create(a, b, orig, CreateFlag::empty());
                        if let Some(o) = orig {
                            bm.elem_select_copy(BMElem::Edge(e), BMElem::Edge(o));
                        }
                        e
                    }
                };
                bm.edge_mut(e)
                    .flag
                    .set(ElemFlag::TAG, out.is_intersect[fi][i]);
                edges.push(e);
            }
            let orig = out.face_orig[fi].filter(|&f| bm.faces.contains(f.0));
            let Some(nf) = bm.face_create(&verts, &edges, orig, CreateFlag::empty()) else {
                continue;
            };
            if let Some(o) = orig {
                bm.elem_select_copy(BMElem::Face(nf), BMElem::Face(o));
                for l in bm.face_loops(nf) {
                    bm.loop_interp_from_face(l, o, false);
                }
            }
            keep_f.insert(nf);
            any_change = true;
        }
        for f in bm.face_ids() {
            if !keep_f.contains(&f) && bm.faces.contains(f.0) {
                bm.face_kill_loose(f);
                any_change = true;
            }
        }
        for v in bm.vert_ids() {
            if !keep_v.contains(&v) && bm.verts.contains(v.0) {
                bm.vert_kill(v);
                any_change = true;
            }
        }
        any_change
    }

    /// The `OP_NOT_IMPLEMENTED` of an open side.
    pub(crate) fn open_side_error() -> OpError {
        OpError::NotImplemented(
            "frap: the selected and unselected parts must each be closed (Blender's exact mesh \
             arrangement, which also cuts open surfaces, is not ported; WP 8.6)"
                .into(),
        )
    }

    /// `BM_mesh_boolean(bm, looptris, test_fn, nullptr, 2, use_self, keep_hidden = true,
    /// hole_tolerant = false, operation)`: returns whether the mesh changed.
    pub(crate) fn boolean(
        bm: &mut BMesh,
        side: fn(&BMesh, FaceId) -> i32,
        op: BoolOperation,
    ) -> Result<bool, OpError> {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        let (shapes, rest) = shapes(bm, side, 2);
        let mut opts = BoolOptions::default();
        opts.operation = op;
        opts.solver = frap_boolean::SolverKind::Exact;
        let ops: Vec<BoolMesh<'_>> = shapes.iter().map(Shape::bool_mesh).collect();
        let r = match frap_boolean::default_solver().run(&ops, &opts) {
            Ok(r) => r,
            Err(frap_boolean::BoolError::NonManifold(_)) => return Err(open_side_error()),
            Err(e) => return Err(OpError::Runtime(e.to_string())),
        };
        // Candidate intersection edges: between faces of different sides.
        let mut edge_side: FxHashMap<(u32, u32), u32> = FxHashMap::default();
        let mut isect: FxHashSet<(u32, u32)> = FxHashSet::default();
        for (f, w) in r.face_offsets.windows(2).enumerate() {
            let fv = &r.corner_verts[w[0] as usize..w[1] as usize];
            for i in 0..fv.len() {
                let (a, b) = (fv[i], fv[(i + 1) % fv.len()]);
                let k = (a.min(b), a.max(b));
                let s = r.face_src[f].0;
                match edge_side.get(&k) {
                    Some(&o) if o != s => {
                        isect.insert(k);
                    }
                    Some(_) => {}
                    None => {
                        edge_side.insert(k, s);
                    }
                }
            }
        }
        let mut out = out_mesh(
            bm,
            &shapes,
            &r.positions,
            &r.vert_src,
            &r.face_offsets,
            &r.corner_verts,
            &r.face_src,
            &|a, b| isect.contains(&(a.min(b), a.max(b))),
        );
        // `get_cdt_edge_orig`: an edge is an intersection only where it lies on no edge of
        // its own input face nor of an input face coplanar with it (Blender triangulates
        // coplanar faces together, and their edges keep their origin).
        let planes = Planes::new(&shapes);
        for (fi, w) in r.face_offsets.windows(2).enumerate() {
            let fv = &r.corner_verts[w[0] as usize..w[1] as usize];
            let (m, lf) = r.face_src[fi];
            for i in 0..fv.len() {
                if !out.is_intersect[fi][i] {
                    continue;
                }
                let (a, b) = (fv[i], fv[(i + 1) % fv.len()]);
                if out.edge_orig[fi][i].is_some()
                    || planes.on_coplanar_edge(
                        &shapes,
                        (m as usize, lf as usize),
                        r.positions[a as usize],
                        r.positions[b as usize],
                    )
                {
                    out.is_intersect[fi][i] = false;
                }
            }
        }
        Ok(apply(bm, &out, true, &rest))
    }

    /// The plane of every input face of every side (unit Newell normal and offset).
    struct Planes {
        planes: Vec<Vec<([f64; 3], f64)>>,
        tol: f64,
    }

    impl Planes {
        fn new(shapes: &[Shape]) -> Self {
            let mut scale = 0.0f64;
            let planes = shapes
                .iter()
                .map(|sh| {
                    for p in &sh.positions {
                        for c in p {
                            scale = scale.max(f64::from(c.abs()));
                        }
                    }
                    sh.face_offsets
                        .windows(2)
                        .map(|w| {
                            let fv = &sh.corner_verts[w[0] as usize..w[1] as usize];
                            let pt =
                                |i: usize| sh.positions[fv[i % fv.len()] as usize].map(f64::from);
                            let mut n = [0.0f64; 3];
                            for i in 0..fv.len() {
                                let (c, d) = (pt(i), pt(i + 1));
                                n[0] += (c[1] - d[1]) * (c[2] + d[2]);
                                n[1] += (c[2] - d[2]) * (c[0] + d[0]);
                                n[2] += (c[0] - d[0]) * (c[1] + d[1]);
                            }
                            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                            if len > 0.0 {
                                n = n.map(|x| x / len);
                            }
                            let p0 = pt(0);
                            (n, n[0] * p0[0] + n[1] * p0[1] + n[2] * p0[2])
                        })
                        .collect()
                })
                .collect();
            Planes {
                planes,
                tol: 1e-6 * scale.max(1.0),
            }
        }

        /// Whether `pa`-`pb` lies on an edge of an input face (other than `src`) in the plane
        /// of the input face `src` (side, face).
        fn on_coplanar_edge(
            &self,
            shapes: &[Shape],
            src: (usize, usize),
            pa: [f32; 3],
            pb: [f32; 3],
        ) -> bool {
            let (n, d) = self.planes[src.0][src.1];
            if n == [0.0; 3] {
                return false;
            }
            for (s, sh) in shapes.iter().enumerate() {
                for (f, &(m, e)) in self.planes[s].iter().enumerate() {
                    if (s, f) == src {
                        continue;
                    }
                    let cos = n[0] * m[0] + n[1] * m[1] + n[2] * m[2];
                    if cos.abs() < 1.0 - 1e-6 || (e - cos.signum() * d).abs() > self.tol {
                        continue;
                    }
                    let fv = &sh.corner_verts
                        [sh.face_offsets[f] as usize..sh.face_offsets[f + 1] as usize];
                    for i in 0..fv.len() {
                        let (u, w) = (
                            sh.positions[fv[i] as usize],
                            sh.positions[fv[(i + 1) % fv.len()] as usize],
                        );
                        if on_segment(pa, u, w) && on_segment(pb, u, w) {
                            return true;
                        }
                    }
                }
            }
            false
        }
    }

    /// `edbm_intersect_select`: after a change everything is deselected and, in vertex or
    /// edge select mode, the tagged (intersection) edges are selected and flushed; normals are
    /// recomputed (`EDBM_update`).
    pub(crate) fn intersect_select(bm: &mut BMesh, do_select: bool) {
        if do_select {
            bm.deselect_all();
            if bm
                .select_mode
                .intersects(SelectMode::VERT | SelectMode::EDGE)
            {
                for e in bm.edge_ids() {
                    if bm.edge(e).flag.contains(ElemFlag::TAG) {
                        bm.edge_select_set(e, true);
                    }
                }
                bm.select_flush_mode();
                // `EDBM_uvselect_clear` only invalidates Blender's UV-selection sync, which
                // frap does not keep.
            }
        }
        bm.normals_update();
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Edit-mode booleans against Blender 5.2.4 (`bpy.ops.mesh.primitive_cube_add` in Edit
    //! Mode, then the operator): element counts, the selection the operator leaves, and the
    //! result's volume and bounds.
    use frap_bmesh::{BMesh, CreateFlag, ElemFlag, SelectMode};
    use frap_boolean::BoolOperation;

    use super::common;

    /// Adds Blender's cube (`primitive_cube_add` order), selected or not.
    pub(crate) fn add_cube(bm: &mut BMesh, size: f32, c: [f32; 3], select: bool) {
        let h = size / 2.0;
        let mut v = Vec::new();
        for x in [-h, h] {
            for y in [-h, h] {
                for z in [-h, h] {
                    v.push(bm.vert_create(
                        [c[0] + x, c[1] + y, c[2] + z],
                        None,
                        CreateFlag::empty(),
                    ));
                }
            }
        }
        for f in [
            [0, 1, 3, 2],
            [2, 3, 7, 6],
            [6, 7, 5, 4],
            [4, 5, 1, 0],
            [2, 6, 4, 0],
            [7, 3, 1, 5],
        ] {
            let verts: Vec<_> = f.iter().map(|&i| v[i]).collect();
            let face = bm
                .face_create_verts(&verts, None, CreateFlag::empty(), true)
                .unwrap();
            if select {
                bm.face_select_set(face, true);
            }
        }
    }

    pub(crate) fn counts(bm: &BMesh) -> (usize, usize, usize) {
        (
            bm.vert_ids().len(),
            bm.edge_ids().len(),
            bm.face_ids().len(),
        )
    }

    /// Signed volume and bounds of the visible faces.
    pub(crate) fn volume_bounds(bm: &BMesh) -> (f64, [[f32; 3]; 2]) {
        let mut vol = 0.0f64;
        let mut b = [[f32::INFINITY; 3], [f32::NEG_INFINITY; 3]];
        for f in bm.face_ids() {
            if bm.face(f).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            let p: Vec<[f64; 3]> = bm
                .face_verts(f)
                .iter()
                .map(|&v| bm.vert(v).co.map(f64::from))
                .collect();
            for q in &p {
                for c in 0..3 {
                    b[0][c] = b[0][c].min(q[c] as f32);
                    b[1][c] = b[1][c].max(q[c] as f32);
                }
            }
            for i in 1..p.len() - 1 {
                let (a, u, w) = (p[0], p[i], p[i + 1]);
                vol += (a[0] * (u[1] * w[2] - u[2] * w[1]) - a[1] * (u[0] * w[2] - u[2] * w[0])
                    + a[2] * (u[0] * w[1] - u[1] * w[0]))
                    / 6.0;
            }
        }
        (vol, b)
    }

    fn run(cutter: (f32, [f32; 3]), op: BoolOperation, mode: SelectMode) -> BMesh {
        let mut bm = BMesh::new();
        bm.select_mode = mode;
        add_cube(&mut bm, 2.0, [0.0; 3], false);
        add_cube(&mut bm, cutter.0, cutter.1, true);
        let changed = common::boolean(&mut bm, common::side_pair, op).unwrap();
        common::intersect_select(&mut bm, changed);
        bm
    }

    #[test]
    fn coplanar_contact_is_not_an_intersection() {
        // Blender: V14 E21 F9, nothing selected (the notch's edges lie on the cutter's
        // coplanar faces' edges).
        let bm = run((1.0, [0.5; 3]), BoolOperation::Difference, SelectMode::VERT);
        assert_eq!(counts(&bm), (14, 21, 9));
        assert_eq!(bm.selected_counts(), (0, 0, 0));
        let (vol, b) = volume_bounds(&bm);
        assert!((vol - 7.0).abs() < 1e-5, "{vol}");
        assert_eq!(b, [[-1.0; 3], [1.0; 3]]);
    }

    #[test]
    fn coplanar_union_selects_nothing() {
        // Blender: V16 E28 F14 (its coplanar faces stay split: T3), nothing selected.
        let bm = run(
            (2.0, [1.0, 0.0, 0.0]),
            BoolOperation::Union,
            SelectMode::VERT,
        );
        assert_eq!(bm.selected_counts(), (0, 0, 0));
        let (vol, b) = volume_bounds(&bm);
        assert!((vol - 12.0).abs() < 1e-5, "{vol}");
        assert_eq!(b, [[-1.0; 3], [2.0, 1.0, 1.0]]);
    }

    #[test]
    fn poke_through_selects_the_cut() {
        // Blender: V16 E26 F12 with the 4 edges of the hole selected, in vertex and edge
        // select mode, for DIFFERENCE and UNION. The DIFFERENCE's floor is the cutter's own
        // face, which `apply_mesh_output_to_bmesh` reuses with its old winding
        // (`BM_face_exists` matches either winding): Blender's signed volume is 7.1333, not
        // 7.4.
        for (op, vol, hi) in [
            (BoolOperation::Difference, 7.133_333_33, [1.0, 1.0, 1.0]),
            (BoolOperation::Union, 8.4, [1.4, 1.0, 1.0]),
        ] {
            for mode in [SelectMode::VERT, SelectMode::EDGE] {
                let bm = run((1.0, [0.9, 0.2, 0.3]), op, mode);
                assert_eq!(counts(&bm), (16, 26, 12), "{op:?}");
                assert_eq!(bm.selected_counts(), (4, 4, 0), "{op:?} {mode:?}");
                let (v, b) = volume_bounds(&bm);
                assert!((v - vol).abs() < 1e-5, "{op:?}: {v}");
                assert!((0..3).all(|c| (b[1][c] - hi[c]).abs() < 1e-6), "{b:?}");
            }
        }
    }

    #[test]
    fn face_select_mode_selects_nothing() {
        // `edbm_intersect_select` selects the cut only in vertex and edge select mode.
        let bm = run(
            (1.0, [0.9, 0.2, 0.3]),
            BoolOperation::Difference,
            SelectMode::FACE,
        );
        assert_eq!(bm.selected_counts(), (0, 0, 0));
    }
}
