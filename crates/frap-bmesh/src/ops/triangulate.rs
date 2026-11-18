// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.triangulate` (Blender 5.2.4 `bmo_triangulate_exec`,
//! `operators/bmo_triangulate.cc`; `BM_mesh_triangulate`, `tools/bmesh_triangulate.cc`;
//! `BM_face_triangulate`, `intern/bmesh_polygon.cc`), implemented by WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.triangulate(bmesh, faces=[], quad_method='BEAUTY', ngon_method='BEAUTY')`.
//!
//! **Create/kill order.** Faces of 4+ corners are visited in face (slot) order. Each is split
//! into `len - 2` triangles created in tessellation order with `BM_face_create_verts` (the
//! diagonal edges are created on the fly, in `(v[n-1], v[0]), (v[0], v[1]), …` order, so new
//! edges and triangles take freed slots first, then append). The **last** triangle then
//! swaps its data with the original face (`bmesh_face_swap_data`: loops, `l_first`, flags,
//! normal, material move; the custom data and the slot stay) and is killed, so the original
//! face keeps its slot (with the last triangle's corners) and the freed slot is taken by the
//! next face's first triangle. A quad of the default cube therefore becomes "first triangle
//! appended, second triangle in the quad's slot". Triangles that duplicate an existing
//! triangle are listed in `face_map_double` (the mesh operator kills them).
//!
//! Quads: `FIXED` splits corners 0–2, `ALTERNATE` 1–3, `SHORT_EDGE`/`LONG_EDGE` compare the
//! diagonals, `BEAUTY` first rejects a diagonal that folds the quad (`is_quad_flip_v3`) and
//! otherwise rotates when `BM_verts_calc_rotate_beauty` is positive. N-gons: polyfill in the
//! plane of the face normal (`axis_dominant_v3_to_m3_negate`), plus `BLI_polyfill_beautify`
//! for `BEAUTY`.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

pub mod beautify;
pub(crate) mod heap;
pub(crate) mod math;
#[cfg(test)]
mod tests;

use crate::query::geom::V3;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId};

/// Input slots of `triangulate` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct TriangulateIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `quad_method` (enum identifier: `BEAUTY`, `FIXED`, `ALTERNATE`, `SHORT_EDGE`,
    /// `LONG_EDGE`).
    pub quad_method: &'static str,
    /// `ngon_method` (enum identifier: `BEAUTY`, `EAR_CLIP`).
    pub ngon_method: &'static str,
}

impl Default for TriangulateIn {
    fn default() -> Self {
        TriangulateIn {
            faces: Vec::new(),
            quad_method: "BEAUTY",
            ngon_method: "BEAUTY",
        }
    }
}

/// Output slots of `triangulate`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TriangulateOut {
    /// `edges.out` then `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `edges.out`: the new diagonal edges (and every edge that was tagged), edge order.
    pub edges: Vec<EdgeId>,
    /// `faces.out`: the triangles (including the original faces), face order.
    pub faces: Vec<FaceId>,
    /// `face_map.out`: `(triangle, original face)`, the original mapped to itself first.
    pub face_map: Vec<(FaceId, FaceId)>,
    /// `face_map_double.out`: `(duplicate triangle, original face)`.
    pub face_map_double: Vec<(FaceId, FaceId)>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_triangulate_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

/// `MOD_TRIANGULATE_QUAD_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum QuadMethod {
    /// `BEAUTY` (0).
    Beauty,
    /// `FIXED` (1): split corners 0–2.
    Fixed,
    /// `ALTERNATE` (2): split corners 1–3.
    Alternate,
    /// `SHORT_EDGE` (3).
    ShortEdge,
    /// `LONG_EDGE` (4).
    LongEdge,
}

impl QuadMethod {
    /// From the `bmesh.ops` identifier.
    pub fn from_identifier(s: &str) -> Option<QuadMethod> {
        Some(match s {
            "BEAUTY" => QuadMethod::Beauty,
            "FIXED" => QuadMethod::Fixed,
            "ALTERNATE" => QuadMethod::Alternate,
            "SHORT_EDGE" => QuadMethod::ShortEdge,
            "LONG_EDGE" => QuadMethod::LongEdge,
            _ => return None,
        })
    }
}

/// `MOD_TRIANGULATE_NGON_*`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum NgonMethod {
    /// `BEAUTY` (0).
    Beauty,
    /// `EAR_CLIP` (1).
    EarClip,
}

impl NgonMethod {
    /// From the `bmesh.ops` identifier.
    pub fn from_identifier(s: &str) -> Option<NgonMethod> {
        Some(match s {
            "BEAUTY" => NgonMethod::Beauty,
            "EAR_CLIP" => NgonMethod::EarClip,
            _ => return None,
        })
    }
}

// Ported from blender/source/blender/bmesh/tools/bmesh_beautify.cc (BM_verts_calc_rotate_beauty,
// flag 0, method 0).
/// `BM_verts_calc_rotate_beauty(v1, v2, v3, v4, 0, 0)`: area method, no restriction.
pub(crate) fn verts_calc_rotate_beauty(v1: V3, v2: V3, v3: V3, v4: V3, same_13: bool) -> f32 {
    if same_13 {
        return f32::MAX;
    }
    beautify::edge_calc_rotate_beauty_area(v1, v2, v3, v4, false)
}

/// The result of [`face_triangulate`].
#[derive(Clone, Debug, Default)]
pub struct FaceTriangulation {
    /// `r_faces_new`: every new triangle except the one swapped into the original face.
    pub faces_new: Vec<FaceId>,
    /// `r_edges_new`: the new edges, in creation order.
    pub edges_new: Vec<EdgeId>,
    /// `r_faces_double`: triangles duplicating an existing triangle, most recent first.
    pub faces_double: Vec<FaceId>,
}

/// `mul_v2_m3v3` with the transposed `axis_dominant_v3_to_m3_negate` matrix.
fn project_negate(m: &[V3; 3], a: V3) -> [f32; 2] {
    [
        m[0][0] * a[0] + m[1][0] * a[1] + m[2][0] * a[2],
        m[0][1] * a[0] + m[1][1] * a[1] + m[2][1] * a[2],
    ]
}

// Ported from blender/source/blender/bmesh/intern/bmesh_polygon.cc (BM_face_triangulate).
/// `BM_face_triangulate` on a face of 4+ corners. With `use_tag` the new triangles (except the
/// swapped last one) and the new edges get `BM_ELEM_TAG`.
pub fn face_triangulate(
    bm: &mut BMesh,
    f: FaceId,
    quad_method: QuadMethod,
    ngon_method: NgonMethod,
    use_tag: bool,
) -> FaceTriangulation {
    let mut out = FaceTriangulation::default();
    let len = bm.face(f).len as usize;
    debug_assert!(len > 3);
    let co = |bm: &BMesh, l: LoopId| bm.vert(bm.lp(l).v).co;
    let (loops, tris): (Vec<LoopId>, Vec<[u32; 3]>) = if len == 4 {
        let l_first = bm.face(f).l_first;
        let next = |l: LoopId| bm.lp(l).next;
        let prev = |l: LoopId| bm.lp(l).prev;
        let (l_v1, l_v2) = match quad_method {
            QuadMethod::Fixed => (l_first, next(next(l_first))),
            QuadMethod::Alternate => (next(l_first), prev(l_first)),
            _ => {
                let l_v1 = next(l_first);
                let l_v2 = next(next(l_first));
                let l_v3 = prev(l_first);
                let l_v4 = l_first;
                let (c1, c2, c3, c4) = (co(bm, l_v1), co(bm, l_v2), co(bm, l_v3), co(bm, l_v4));
                let split_24 = match quad_method {
                    QuadMethod::ShortEdge => {
                        let d1 = len_squared_v3v3(c4, c2);
                        let d2 = len_squared_v3v3(c1, c3);
                        (d2 - d1) > 0.0
                    }
                    QuadMethod::LongEdge => {
                        let d1 = len_squared_v3v3(c4, c2);
                        let d2 = len_squared_v3v3(c1, c3);
                        (d2 - d1) < 0.0
                    }
                    _ => {
                        let flip_flag = math::is_quad_flip_v3(c1, c2, c3, c4);
                        if flip_flag & 1 != 0 {
                            true
                        } else if flip_flag & 2 != 0 {
                            false
                        } else {
                            let same = bm.lp(l_v1).v == bm.lp(l_v3).v;
                            verts_calc_rotate_beauty(c1, c2, c3, c4, same) > 0.0
                        }
                    }
                };
                // Named confusingly, `l_v1` is in fact the second vertex.
                if split_24 { (l_v4, l_v2) } else { (l_v1, l_v3) }
            }
        };
        (
            vec![l_v1, next(l_v1), l_v2, next(l_v2)],
            vec![[0, 1, 2], [0, 2, 3]],
        )
    } else {
        let no = bm.face(f).no;
        let axis_mat = frap_geom::polyfill::axis_dominant_v3_to_m3_negate(no);
        let loops = bm.face_loops(f);
        let projverts: Vec<[f32; 2]> = loops
            .iter()
            .map(|&l| project_negate(&axis_mat, co(bm, l)))
            .collect();
        let mut tris = frap_geom::polyfill::polyfill_calc(&projverts, 1);
        if ngon_method == NgonMethod::Beauty {
            beautify::polyfill_beautify(&projverts, &mut tris);
        }
        (loops, tris)
    };

    let totfilltri = len - 2;
    let last_tri = len - 3;
    let mut f_new = f;
    for (i, tri) in tris.iter().enumerate().take(totfilltri) {
        let ltri = [
            loops[tri[0] as usize],
            loops[tri[1] as usize],
            loops[tri[2] as usize],
        ];
        let v_tri = [bm.lp(ltri[0]).v, bm.lp(ltri[1]).v, bm.lp(ltri[2]).v];
        f_new = bm
            .face_create_verts(&v_tri, Some(f), CreateFlag::empty(), true)
            .expect("invariant: a triangle over three distinct corners");
        let l_new = bm.face(f_new).l_first;

        // Check for duplicate.
        let first_radial = bm.lp(l_new).radial_next;
        if first_radial != l_new {
            let mut l_iter = first_radial;
            loop {
                let lf = bm.lp(l_iter).f;
                if bm.face(lf).len == 3 && bm.lp(bm.lp(l_new).prev).v == bm.lp(bm.lp(l_iter).prev).v
                {
                    // Check the last tri because we swap the last `f_new` with `f` at the end.
                    out.faces_double
                        .insert(0, if i != last_tri { f_new } else { f });
                    break;
                }
                l_iter = bm.lp(l_iter).radial_next;
                if l_iter == l_new {
                    break;
                }
            }
        }

        // Copy CD data.
        let l_next = bm.lp(l_new).next;
        let l_prev = bm.lp(l_new).prev;
        bm.loop_attrs_copy(ltri[0], l_new);
        bm.loop_attrs_copy(ltri[1], l_next);
        bm.loop_attrs_copy(ltri[2], l_prev);

        // Add all but the last face, which is swapped and removed (below).
        if i != last_tri {
            if use_tag {
                bm.face_mut(f_new).flag.insert(ElemFlag::TAG);
            }
            out.faces_new.push(f_new);
        }

        // New faces' loops: a loop alone on its edge is on a new edge.
        let mut l_iter = l_new;
        loop {
            let e = bm.lp(l_iter).e;
            if bm.lp(l_iter).radial_next == l_iter {
                if use_tag {
                    bm.edge_mut(e).flag.insert(ElemFlag::TAG);
                }
                out.edges_new.push(e);
            }
            l_iter = bm.lp(l_iter).next;
            if l_iter == l_new {
                break;
            }
        }
    }

    // We can't delete the real face, because some callers expect it to remain valid: swap
    // data and delete the last created triangle.
    math::face_swap_data(bm, f, f_new);
    bm.face_kill(f_new);
    bm.index.dirty |= crate::ElemTypes::FACE;
    out
}

fn len_squared_v3v3(a: V3, b: V3) -> f32 {
    let d = math::sub(a, b);
    math::dot(d, d)
}

// Ported from blender/source/blender/bmesh/tools/bmesh_triangulate.cc (BM_mesh_triangulate,
// the `slot_facemap_out` branch).
/// `BM_mesh_triangulate(bm, quad_method, ngon_method, min_vertices, tag_only, …)` with the
/// face maps; returns `(face_map, face_map_double)`.
#[allow(clippy::type_complexity)]
pub fn mesh_triangulate(
    bm: &mut BMesh,
    quad_method: QuadMethod,
    ngon_method: NgonMethod,
    min_vertices: usize,
    tag_only: bool,
) -> (Vec<(FaceId, FaceId)>, Vec<(FaceId, FaceId)>) {
    let mut face_map = Vec::new();
    let mut face_map_double = Vec::new();
    // `BM_ITER_MESH` keeps visiting faces created behind the cursor; they are triangles, so
    // they are skipped by the length test anyway.
    let mut s = 0u32;
    while (s as usize) < bm.faces.capacity() {
        if bm.faces.contains(s) {
            let face = FaceId(s);
            let fl = bm.face(face);
            if fl.len as usize >= min_vertices && (!tag_only || fl.flag.contains(ElemFlag::TAG)) {
                frap_math::budget::check();
                let t = face_triangulate(bm, face, quad_method, ngon_method, tag_only);
                face_map.push((face, face));
                for &fnew in &t.faces_new {
                    face_map.push((fnew, face));
                }
                for &fd in &t.faces_double {
                    face_map_double.push((fd, face));
                }
            }
        }
        s += 1;
    }
    (face_map, face_map_double)
}

/// Runs `triangulate` on `bm`.
pub fn triangulate(bm: &mut BMesh, i: &TriangulateIn) -> Result<TriangulateOut, BmError> {
    let quad = QuadMethod::from_identifier(i.quad_method).ok_or_else(|| {
        BmError::Invalid(format!(
            "triangulate: unknown quad_method '{}' (expected BEAUTY, FIXED, ALTERNATE, \
             SHORT_EDGE or LONG_EDGE)",
            i.quad_method
        ))
    })?;
    let ngon = NgonMethod::from_identifier(i.ngon_method).ok_or_else(|| {
        BmError::Invalid(format!(
            "triangulate: unknown ngon_method '{}' (expected BEAUTY or EAR_CLIP)",
            i.ngon_method
        ))
    })?;
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "triangulate: dead face {dead:?} in faces"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        // BM_mesh_elem_hflag_disable_all(bm, BM_FACE | BM_EDGE, BM_ELEM_TAG, false).
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        for f in bm.face_ids() {
            bm.face_mut(f).flag.remove(ElemFlag::TAG);
        }
        for &f in &i.faces {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
        }
        let (face_map, face_map_double) = mesh_triangulate(bm, quad, ngon, 4, true);
        let mut out = TriangulateOut {
            face_map,
            face_map_double,
            ..TriangulateOut::default()
        };
        for e in bm.edge_ids() {
            if bm.edge(e).flag.contains(ElemFlag::TAG) {
                out.edges.push(e);
                out.geom.push(BMElem::Edge(e));
            }
        }
        for f in bm.face_ids() {
            if bm.face(f).flag.contains(ElemFlag::TAG) {
                out.faces.push(f);
                out.geom.push(BMElem::Face(f));
            }
        }
        out
    }))
}
