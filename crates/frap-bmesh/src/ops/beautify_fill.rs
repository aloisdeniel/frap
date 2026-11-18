// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.beautify_fill` (Blender 5.2.4 `bmo_beautify_fill_exec`,
//! `operators/bmo_beautify.cc`) and `BM_mesh_beautify_fill` (`tools/bmesh_beautify.cc`),
//! implemented by WP 2.8 (Create and fill).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.beautify_fill(bmesh, faces=[], edges=[], use_restrict_tag=False, method='AREA')`.
//!
//! **Create/kill order**: edges between two input triangles are rotated one at a time, best
//! improvement first (a `BLI_heap` of negative costs, `heap.rs`); each rotation is
//! `BM_edge_rotate(CHECK_EXISTS)` (new edge, faces joined then split, see
//! `rotate_edges.rs`), then the four surrounding edges are re-scored. Each edge slot keeps the
//! set of states it has been in, so rotations never cycle.
//!
//! Also exports `BLI_polyfill_beautify_quad_rotate_calc_ex` and
//! `BLI_polyfill_edge_calc_rotate_beauty__area` (`blenlib/intern/polyfill_2d_beautify.cc`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

pub mod heap;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::ops::contextual_create::bli::{self, V3};
use crate::ops::rotate_edges;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, VertId};
use heap::{Heap, HeapNode};

/// Input slots of `beautify_fill` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct BeautifyFillIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `use_restrict_tag`.
    pub use_restrict_tag: bool,
    /// `method` (enum identifier).
    pub method: &'static str,
}

impl Default for BeautifyFillIn {
    fn default() -> Self {
        BeautifyFillIn {
            faces: Vec::new(),
            edges: Vec::new(),
            use_restrict_tag: false,
            method: "AREA",
        }
    }
}

/// Output slots of `beautify_fill`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BeautifyFillOut {
    /// `geom.out`: the rotated edges and their faces (edges, then faces; mesh order).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_beautify_fill_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// The `method` slot.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BeautifyMethod {
    /// `AREA` (0): area/perimeter ratio in the projected quad.
    Area,
    /// `ANGLE` (1): angle between the two triangle normals.
    Angle,
}

/// `VERT_RESTRICT_TAG`.
const VERT_RESTRICT_TAG: u8 = 1;
/// `EDGE_RESTRICT_DEGENERATE`.
const EDGE_RESTRICT_DEGENERATE: u8 = 2;

/// `cross_tri_v2`.
#[inline]
fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

/// `signum_i_ex`.
#[inline]
fn signum_i_ex(a: f32, eps: f32) -> i32 {
    if a > eps {
        1
    } else if a < -eps {
        -1
    } else {
        0
    }
}

// Ported from blender/source/blender/blenlib/intern/polyfill_2d_beautify.cc
// (BLI_polyfill_beautify_quad_rotate_calc_ex).
/// `BLI_polyfill_beautify_quad_rotate_calc_ex`: negative when the (1-3) diagonal of the quad
/// `v1 v2 v3 v4` beats the current (2-4) one; also returns the quad area (`r_area`).
pub fn quad_rotate_calc_ex(
    v1: [f32; 2],
    v2: [f32; 2],
    v3: [f32; 2],
    v4: [f32; 2],
    lock_degenerate: bool,
) -> (f32, f32) {
    let eps_zero_area = 1e-12f32;
    let area_2x_234 = cross_tri_v2(v2, v3, v4);
    let area_2x_241 = cross_tri_v2(v2, v4, v1);
    let area_2x_123 = cross_tri_v2(v1, v2, v3);
    let area_2x_134 = cross_tri_v2(v1, v3, v4);
    let r_area =
        (area_2x_234.abs() + area_2x_241.abs() + area_2x_123.abs() + area_2x_134.abs()) / 8.0;
    if (area_2x_123 >= 0.0) != (area_2x_134 >= 0.0) {
        return (f32::MAX, r_area);
    }
    if area_2x_123.abs() <= eps_zero_area || area_2x_134.abs() <= eps_zero_area {
        return (f32::MAX, r_area);
    }
    if (area_2x_234 >= 0.0) != (area_2x_241 >= 0.0) {
        if lock_degenerate {
            return (f32::MAX, r_area);
        }
        return (-f32::MAX, r_area);
    }
    if area_2x_234.abs() <= eps_zero_area || area_2x_241.abs() <= eps_zero_area {
        return (-f32::MAX, r_area);
    }
    let len_12 = bli::len_v2v2(v1, v2);
    let len_23 = bli::len_v2v2(v2, v3);
    let len_34 = bli::len_v2v2(v3, v4);
    let len_41 = bli::len_v2v2(v4, v1);
    let len_13 = bli::len_v2v2(v1, v3);
    let len_24 = bli::len_v2v2(v2, v4);
    let area_a = area_2x_234.abs();
    let area_b = area_2x_241.abs();
    let prim_a = len_23 + len_34 + len_24;
    let prim_b = len_41 + len_12 + len_24;
    let fac_24 = (area_a / prim_a) + (area_b / prim_b);
    let area_a = area_2x_123.abs();
    let area_b = area_2x_134.abs();
    let prim_a = len_12 + len_23 + len_13;
    let prim_b = len_34 + len_41 + len_13;
    let fac_13 = (area_a / prim_a) + (area_b / prim_b);
    (fac_24 - fac_13, r_area)
}

// Ported from blender/source/blender/blenlib/intern/polyfill_2d_beautify.cc
// (BLI_polyfill_edge_calc_rotate_beauty__area).
/// `BLI_polyfill_edge_calc_rotate_beauty__area` on 3D positions.
pub fn edge_calc_rotate_beauty_area(v1: V3, v2: V3, v3: V3, v4: V3, lock_degenerate: bool) -> f32 {
    let eps = 1e-5f32;
    let no_a = bli::cross_tri_v3(v2, v3, v4);
    let no_b = bli::cross_tri_v3(v2, v4, v1);
    let (no, no_scale) = bli::normalize_v3(bli::add(no_a, no_b));
    if no_scale == 0.0 {
        return f32::MAX;
    }
    let m = bli::axis_dominant_v3_to_m3(no);
    let v1 = bli::mul_v2_m3v3(&m, v1);
    let v2 = bli::mul_v2_m3v3(&m, v2);
    let v3 = bli::mul_v2_m3v3(&m, v3);
    let v4 = bli::mul_v2_m3v3(&m, v4);
    if signum_i_ex(cross_tri_v2(v2, v3, v4) / no_scale, eps)
        + signum_i_ex(cross_tri_v2(v2, v4, v1) / no_scale, eps)
        == 0
    {
        return f32::MAX;
    }
    quad_rotate_calc_ex(v1, v2, v3, v4, lock_degenerate).0
}

/// `bm_edge_calc_rotate_beauty__angle`.
fn rotate_beauty_angle(v1: V3, v2: V3, v3: V3, v4: V3) -> f32 {
    let no_a = bli::normal_tri_v3(v2, v3, v4).0;
    let no_b = bli::normal_tri_v3(v2, v4, v1).0;
    let angle_24 = bli::angle_normalized_v3v3(no_a, no_b);
    let (no_a, la) = bli::normal_tri_v3(v1, v2, v3);
    let (no_b, lb) = bli::normal_tri_v3(v1, v3, v4);
    if la == 0.0 || lb == 0.0 {
        return f32::MAX;
    }
    let angle_13 = bli::angle_normalized_v3v3(no_a, no_b);
    angle_13 - angle_24
}

// Ported from blender/source/blender/bmesh/tools/bmesh_beautify.cc (BM_verts_calc_rotate_beauty).
/// `BM_verts_calc_rotate_beauty`.
fn verts_calc_rotate_beauty(
    bm: &BMesh,
    v1: VertId,
    v2: VertId,
    v3: VertId,
    v4: VertId,
    flag: u8,
    method: BeautifyMethod,
) -> f32 {
    if flag & VERT_RESTRICT_TAG != 0
        && bm.vert(v1).flag.contains(ElemFlag::TAG) == bm.vert(v3).flag.contains(ElemFlag::TAG)
    {
        return f32::MAX;
    }
    if v1 == v3 {
        return f32::MAX;
    }
    let co = |v: VertId| bm.vert(v).co;
    match method {
        BeautifyMethod::Area => edge_calc_rotate_beauty_area(
            co(v1),
            co(v2),
            co(v3),
            co(v4),
            flag & EDGE_RESTRICT_DEGENERATE != 0,
        ),
        BeautifyMethod::Angle => rotate_beauty_angle(co(v1), co(v2), co(v3), co(v4)),
    }
}

/// `bm_edge_calc_rotate_beauty`.
fn edge_calc_rotate_beauty(bm: &BMesh, e: EdgeId, flag: u8, method: BeautifyMethod) -> f32 {
    let l = bm.edge(e).l.expect("manifold edge");
    let ll = bm.lp(l);
    let v1 = bm.lp(ll.prev).v;
    let v2 = ll.v;
    let lr = bm.lp(ll.radial_next);
    let v3 = bm.lp(lr.prev).v;
    let v4 = bm.lp(ll.next).v;
    verts_calc_rotate_beauty(bm, v1, v2, v3, v4, flag, method)
}

/// `EdRotState`: (edge verts, opposite verts), each ordered.
type EdRotState = ([u32; 2], [u32; 2]);

fn erot_state_ex(bm: &BMesh, e: EdgeId) -> ([u32; 2], [u32; 2]) {
    let [a, b] = bm.edge(e).v;
    let mut v = [a.0, b.0];
    if v[0] > v[1] {
        v.swap(0, 1);
    }
    let l = bm.edge(e).l.expect("manifold edge");
    let p0 = bm.lp(bm.lp(l).prev).v.0;
    let lr = bm.lp(l).radial_next;
    let p1 = bm.lp(bm.lp(lr).prev).v.0;
    let mut f = [p0, p1];
    if f[0] > f[1] {
        f.swap(0, 1);
    }
    (v, f)
}

struct Beauty<'a> {
    heap: Heap<EdgeId>,
    table: Vec<Option<HeapNode>>,
    states: Vec<FxHashSet<EdRotState>>,
    edge_array: &'a mut Vec<EdgeId>,
    index: FxHashMap<EdgeId, usize>,
    flag: u8,
    method: BeautifyMethod,
}

impl Beauty<'_> {
    fn in_array(&self, e: EdgeId) -> Option<usize> {
        let i = *self.index.get(&e)?;
        (i < self.edge_array.len() && self.edge_array[i] == e).then_some(i)
    }
    /// `bm_edge_update_beauty_cost_single`.
    fn update_single(&mut self, bm: &BMesh, e: EdgeId) {
        let Some(i) = self.in_array(e) else {
            return;
        };
        if let Some(n) = self.table[i].take() {
            self.heap.remove(n);
        }
        // `erot_state_alternate`: the swapped state.
        let (v, f) = erot_state_ex(bm, e);
        if self.states[i].contains(&(f, v)) {
            return;
        }
        let cost = edge_calc_rotate_beauty(bm, e, self.flag, self.method);
        self.table[i] = if cost < 0.0 {
            Some(self.heap.insert(cost, e))
        } else {
            None
        };
    }
}

// Ported from blender/source/blender/bmesh/tools/bmesh_beautify.cc (BM_mesh_beautify_fill).
/// `BM_mesh_beautify_fill(edge_array, flag, method, oflag_edge, oflag_face)`: rotates edges of
/// `edge_array` (updated in place with the new edges) until no rotation improves; returns the
/// rotated edges and their faces (`oflag_edge`/`oflag_face` set on the running operator's
/// layer when non-zero).
pub fn mesh_beautify_fill(
    bm: &mut BMesh,
    edge_array: &mut Vec<EdgeId>,
    use_restrict_tag: bool,
    method: BeautifyMethod,
    oflag_edge: u16,
    oflag_face: u16,
) {
    let flag = (if use_restrict_tag {
        VERT_RESTRICT_TAG
    } else {
        0
    }) | EDGE_RESTRICT_DEGENERATE;
    let n = edge_array.len();
    let mut b = Beauty {
        heap: Heap::new(),
        table: vec![None; n],
        states: vec![FxHashSet::default(); n],
        edge_array,
        index: FxHashMap::default(),
        flag,
        method,
    };
    for i in 0..n {
        let e = b.edge_array[i];
        let cost = edge_calc_rotate_beauty(bm, e, flag, method);
        b.table[i] = if cost < 0.0 {
            Some(b.heap.insert(cost, e))
        } else {
            None
        };
        b.index.insert(e, i);
    }
    while !b.heap.is_empty() {
        frap_math::budget::check();
        let e = b.heap.pop_min();
        let i = b.index[&e];
        b.table[i] = None;
        let Some(e_new) = rotate_edges::edge_rotate(bm, e, false, rotate_edges::CHECK_EXISTS)
        else {
            continue;
        };
        b.index.remove(&e);
        let st = erot_state_ex(bm, e_new);
        b.states[i].insert(st);
        b.edge_array[i] = e_new;
        b.index.insert(e_new, i);
        // `bm_edge_update_beauty_cost`.
        let l = bm.edge(e_new).l.expect("rotated edge has faces");
        let ll = bm.lp(l).clone();
        let lr = bm.lp(ll.radial_next).clone();
        let e_arr = [
            bm.lp(ll.next).e,
            bm.lp(ll.prev).e,
            bm.lp(lr.next).e,
            bm.lp(lr.prev).e,
        ];
        for ea in e_arr {
            b.update_single(bm, ea);
        }
        if oflag_edge != 0 {
            bm.oflag_enable(e_new, oflag_edge);
        }
        if oflag_face != 0 {
            let l = bm.edge(e_new).l.expect("rotated edge has faces");
            let f0 = bm.lp(l).f;
            let f1 = bm.lp(bm.lp(l).radial_next).f;
            bm.oflag_enable(f0, oflag_face);
            bm.oflag_enable(f1, oflag_face);
        }
    }
}

const ELE_NEW: u16 = 1;
const FACE_MARK: u16 = 2;

// Ported from blender/source/blender/bmesh/operators/bmo_beautify.cc (bmo_beautify_fill_exec).
/// The body of `bmesh.ops.beautify_fill` on the running operator's flag layer (callers inside
/// another operator push a layer first, as `BMO_op_exec` does); returns the faces of
/// `geom.out`.
pub fn beautify_fill_inner(
    bm: &mut BMesh,
    faces: &[FaceId],
    edges: &[EdgeId],
    use_restrict_tag: bool,
    method: BeautifyMethod,
) -> Vec<FaceId> {
    let caps = [
        bm.verts.capacity(),
        bm.edges.capacity(),
        bm.faces.capacity(),
    ];
    bm.oflags.push(caps);
    let (_, faces_out) = beautify_body(bm, faces, edges, use_restrict_tag, method);
    bm.oflags.pop();
    faces_out
}

/// The body of `bmo_beautify_fill_exec` on the running operator's flag layer: returns the
/// rotated edges and their faces (`geom.out`, mesh order).
pub fn beautify_body(
    bm: &mut BMesh,
    faces: &[FaceId],
    edges: &[EdgeId],
    use_restrict_tag: bool,
    method: BeautifyMethod,
) -> (Vec<EdgeId>, Vec<FaceId>) {
    for &f in faces {
        if bm.face(f).len == 3 {
            bm.oflag_enable(f, FACE_MARK);
        }
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::TAG);
    }
    let mut edge_array = Vec::new();
    for &e in edges {
        if rotate_edges::edge_rotate_check(bm, e) {
            let l = bm.edge(e).l.expect("manifold");
            let f0 = bm.lp(l).f;
            let f1 = bm.lp(bm.lp(l).radial_next).f;
            if bm.oflag_test(f0, FACE_MARK) && bm.oflag_test(f1, FACE_MARK) {
                edge_array.push(e);
            }
        }
    }
    mesh_beautify_fill(
        bm,
        &mut edge_array,
        use_restrict_tag,
        method,
        ELE_NEW,
        FACE_MARK | ELE_NEW,
    );
    let e_out: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, ELE_NEW))
        .collect();
    let f_out: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.oflag_test(f, ELE_NEW))
        .collect();
    (e_out, f_out)
}

/// Runs `beautify_fill` on `bm`.
pub fn beautify_fill(bm: &mut BMesh, i: &BeautifyFillIn) -> Result<BeautifyFillOut, BmError> {
    let method = match i.method {
        "AREA" => BeautifyMethod::Area,
        "ANGLE" => BeautifyMethod::Angle,
        m => {
            return Err(BmError::Invalid(format!(
                "beautify_fill: unknown method '{m}' (expected AREA or ANGLE)"
            )));
        }
    };
    if let Some(dead) = i.faces.iter().find(|&&f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "beautify_fill: dead face {dead:?}"
        )));
    }
    if let Some(dead) = i.edges.iter().find(|&&e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!(
            "beautify_fill: dead edge {dead:?}"
        )));
    }
    let (e_out, f_out) = bm.bmo_exec(TYPE_FLAG, |bm| {
        beautify_body(bm, &i.faces, &i.edges, i.use_restrict_tag, method)
    });
    let mut geom: Vec<BMElem> = e_out.into_iter().map(BMElem::Edge).collect();
    geom.extend(f_out.into_iter().map(BMElem::Face));
    Ok(BeautifyFillOut { geom })
}
