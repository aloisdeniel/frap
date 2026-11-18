// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.triangle_fill` (Blender 5.2.4 `bmo_triangle_fill_exec`,
//! `operators/bmo_triangulate.cc`), implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.triangle_fill(bmesh, use_beauty=False, use_dissolve=False, edges=[], normal=Vector())`.
//!
//! **Create/kill order**: the input edges go to `BLI_scanfill` (`triangle_fill/scanfill.rs`,
//! holes, several polygons and loose edges handled) projected along `normal` (or the sorted,
//! sign-aligned sum of the corner normals of vertices with exactly two input edges); the
//! triangles are created in scan-fill order with `BM_face_create_quad_tri`
//! (`BM_CREATE_NO_DOUBLE`, edges created in `(v3, v1), (v1, v2), (v2, v3)` order), flipped
//! as a whole when the boundary edges of attached faces vote so. With `use_beauty` the new
//! triangles go through `beautify_fill` (AREA), with `use_dissolve` the new interior edges
//! are dissolved (`BM_faces_join_pair`, then the edge is killed) in edge order.
//!
//! Operator type flags: `UNTAN_MULTIRES | NORMALS_CALC | SELECT_FLUSH`.

pub mod scanfill;

use rustc_hash::FxHashMap;

use crate::ops::beautify_fill::{self, BeautifyMethod};
use crate::ops::contextual_create::bli;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId, VertId};
use scanfill::ScanFill;

/// Input slots of `triangle_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TriangleFillIn {
    /// `use_beauty`.
    pub use_beauty: bool,
    /// `use_dissolve`.
    pub use_dissolve: bool,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `normal`.
    pub normal: [f32; 3],
}

/// Output slots of `triangle_fill`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct TriangleFillOut {
    /// `geom.out`: the new edges then the new faces (mesh order).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_triangle_fill_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

const ELE_NEW: u16 = 1;
const EDGE_MARK: u16 = 4;

/// Runs `triangle_fill` on `bm`.
pub fn triangle_fill(bm: &mut BMesh, i: &TriangleFillIn) -> Result<TriangleFillOut, BmError> {
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("triangle_fill: dead edge {dead:?}")));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

/// `BM_face_create_quad_tri` for a triangle (`BM_face_create_verts` with edge creation).
fn face_create_tri(bm: &mut BMesh, v: [VertId; 3]) -> FaceId {
    bm.face_create_verts(&v, None, CreateFlag::NO_DOUBLE, true)
        .expect("triangle face")
}

// Ported from blender/source/blender/bmesh/operators/bmo_triangulate.cc
// (bmo_triangle_fill_exec).
fn exec(bm: &mut BMesh, i: &TriangleFillIn) -> TriangleFillOut {
    let scanfill_flag = scanfill::CALC_HOLES | scanfill::CALC_POLYS | scanfill::CALC_LOOSE;
    let mut normal = i.normal;
    let mut calc_winding = false;
    let mut sf = ScanFill::new();
    let mut map: FxHashMap<VertId, usize> = FxHashMap::default();
    let mut sf_to_v: Vec<VertId> = Vec::new();
    for &e in &i.edges {
        bm.oflag_enable(e, EDGE_MARK);
        calc_winding = calc_winding || bm.edge_is_boundary(e);
        let mut sv = [0usize; 2];
        for (k, v) in bm.edge(e).v.into_iter().enumerate() {
            sv[k] = *map.entry(v).or_insert_with(|| {
                sf_to_v.push(v);
                sf.vert_add(bm.vert(v).co, sf_to_v.len() - 1)
            });
        }
        sf.edge_add(sv[0], sv[1]);
    }
    if bli::is_zero_v3(normal) {
        let mut is_degenerate = true;
        let mut nors: Vec<(f32, [f32; 3])> = Vec::with_capacity(sf_to_v.len());
        for sv in sf.vert_list() {
            let v = sf_to_v[sf.verts[sv].user];
            let mut item = (-1.0f32, [0.0f32; 3]);
            normal = bli::add(normal, bm.vert(v).no);
            let mut e_pair = Vec::with_capacity(2);
            for e in bm.vert_edges(v) {
                if bm.oflag_test(e, EDGE_MARK) {
                    if e_pair.len() == 2 {
                        e_pair.clear();
                        break;
                    }
                    e_pair.push(e);
                }
            }
            if e_pair.len() == 2 {
                is_degenerate = false;
                let co = bm.vert(v).co;
                let dir_a = bli::sub(co, bm.vert(bm.edge_other_vert(e_pair[0], v)).co);
                let dir_b = bli::sub(co, bm.vert(bm.edge_other_vert(e_pair[1], v)).co);
                item.1 = bli::cross(dir_a, dir_b);
                item.0 = bli::len_squared_v3(item.1);
                if bli::len_squared_v3(dir_a) > bli::len_squared_v3(dir_b) {
                    item.1 = bli::negate(item.1);
                }
            }
            nors.push(item);
        }
        if !is_degenerate {
            // `qsort(..., BLI_sortutil_cmp_float_reverse)`.
            bli::c_qsort(&mut nors, &bli::cmp_float_reverse);
            normal = nors[0].1;
            for item in &nors {
                if item.0 == -1.0 {
                    break;
                }
                let mut no = item.1;
                if bli::dot(normal, no) < 0.0 {
                    no = bli::negate(no);
                }
                normal = bli::add(normal, no);
            }
            normal = bli::normalize_v3(normal).0;
        }
    } else {
        calc_winding = false;
    }
    let (n, l) = bli::normalize_v3(normal);
    normal = if l == 0.0 { [0.0, 0.0, 1.0] } else { n };
    sf.calc_ex(scanfill_flag, Some(normal));

    let mut tris: Vec<[VertId; 3]> = sf
        .faces
        .iter()
        .map(|t| t.map(|sv| sf_to_v[sf.verts[sv].user]))
        .collect();
    if calc_winding {
        let mut votes = 0i32;
        for t in &tris {
            let mut i_prev = 2;
            for k in 0..3 {
                if let Some(e) = bm.edge_exists(t[k], t[i_prev])
                    && bm.edge_is_boundary(e)
                    && bm.oflag_test(e, EDGE_MARK)
                {
                    let l = bm.edge(e).l.expect("boundary edge");
                    votes += if bm.lp(l).v == t[k] { 1 } else { -1 };
                }
                i_prev = k;
            }
        }
        if votes < 0 {
            for t in &mut tris {
                t.swap(1, 2);
            }
        }
    }
    for t in tris {
        let f = face_create_tri(bm, t);
        bm.oflag_enable(f, ELE_NEW);
        for e in bm.face_edges(f) {
            if !bm.oflag_test(e, EDGE_MARK) {
                bm.oflag_enable(e, ELE_NEW);
            }
        }
    }
    if i.use_beauty {
        let faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, ELE_NEW))
            .collect();
        // `edges=%Fe`: every edge *without* EDGE_MARK.
        let edges: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| !bm.oflag_test(e, EDGE_MARK))
            .collect();
        let caps = [
            bm.verts.capacity(),
            bm.edges.capacity(),
            bm.faces.capacity(),
        ];
        bm.oflags.push(caps);
        let (e_out, f_out) =
            beautify_fill::beautify_body(bm, &faces, &edges, false, BeautifyMethod::Area);
        bm.oflags.pop();
        for e in e_out {
            bm.oflag_enable(e, ELE_NEW);
        }
        for f in f_out {
            bm.oflag_enable(f, ELE_NEW);
        }
    }
    if i.use_dissolve {
        for e in bm.edge_ids() {
            if !bm.edges.contains(e.0) || !bm.oflag_test(e, ELE_NEW) {
                continue;
            }
            let pair = bm.edge(e).l.and_then(|la| {
                let lb = bm.lp(la).radial_next;
                (la != lb && bm.lp(lb).radial_next == la).then_some((la, lb))
            });
            match pair {
                Some((la, lb))
                    if bm.oflag_test(bm.lp(la).f, ELE_NEW) && bm.oflag_test(bm.lp(lb).f, ELE_NEW) =>
                {
                    // `BM_faces_join_pair(e->l, e->l->radial_next, false)`.
                    if bm.lp(la).v == bm.lp(lb).v {
                        bm.loop_reverse(bm.lp(lb).f);
                    }
                    let faces = [bm.lp(la).f, bm.lp(lb).f];
                    let (f_new, f_double) = bm.faces_join(&faces, false, true);
                    if f_double.is_some() {
                        if let Some(f) = f_new {
                            bm.face_kill(f);
                        }
                        bm.edge_kill(e);
                    } else if let Some(f) = f_new {
                        bm.oflag_enable(f, ELE_NEW);
                        bm.edge_kill(e);
                    }
                }
                _ => {
                    if bm.edge(e).l.is_none() {
                        bm.edge_kill(e);
                    }
                }
            }
        }
    }
    let mut geom: Vec<BMElem> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, ELE_NEW))
        .map(BMElem::Edge)
        .collect();
    geom.extend(
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, ELE_NEW))
            .map(BMElem::Face),
    );
    TriangleFillOut { geom }
}
