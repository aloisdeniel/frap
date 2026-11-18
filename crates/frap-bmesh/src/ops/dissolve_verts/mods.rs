// SPDX-License-Identifier: GPL-3.0-or-later
//! Kernel-level helpers the WP 2.7 operators share (delete, dissolve, merge): the pieces of
//! `bmesh_mods.cc`, `bmesh_query.cc`, `bmesh_inline.hh`, `bmesh_core.cc` and the BLI math
//! they call that the WP 2.1 kernel does not expose. Kept here (a submodule of an owned file,
//! ARCHITECTURE §22.1) so the frozen kernel API does not change.
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later): `bmesh/intern/bmesh_mods.cc`
//! (`BM_faces_join_pair`, `BM_vert_collapse_faces`, `BM_vert_collapse_edge`),
//! `bmesh/intern/bmesh_query.cc` (`BM_vert_is_edge_pair`, `BM_vert_edge_pair`,
//! `BM_edge_face_pair`, `BM_edge_loop_pair`, `BM_vert_calc_edge_angle`,
//! `BM_edge_calc_face_angle`), `bmesh/intern/bmesh_inline.hh` (`BM_elem_flag_merge_ex`),
//! `bmesh/intern/bmesh_core.cc` (`bmesh_face_swap_data`), `blenlib/intern/math_vector.cc`
//! (`angle_*`, `cos_v3v3v3`) and `blenkernel/intern/customdata.cc` (the "math" callbacks of
//! the loop layers: `multiply`, `add`, `initminmax`, `dominmax`, `copyvalue`).

use std::sync::Arc;

use frap_mesh::AttrData;

use crate::query::geom::{self, V3};
use crate::types::{BMElem, BMesh, EdgeId, ElemFlag, FaceId, LoopId, VertId};

// ==================================================================== queries

/// `BM_vert_is_edge_pair`: `v` has exactly two edges.
pub(crate) fn vert_is_edge_pair(bm: &BMesh, v: VertId) -> bool {
    vert_edge_pair(bm, v).is_some()
}

/// `BM_vert_edge_pair`: the two edges of a vertex with exactly two (`v->e` first).
pub(crate) fn vert_edge_pair(bm: &BMesh, v: VertId) -> Option<(EdgeId, EdgeId)> {
    let e_a = bm.vert(v).e?;
    let e_b = bm.disk_edge_next(e_a, v);
    (e_b != e_a && bm.disk_edge_next(e_b, v) == e_a).then_some((e_a, e_b))
}

/// `BM_edge_loop_pair`: the two loops of an edge used by exactly two faces.
pub(crate) fn edge_loop_pair(bm: &BMesh, e: EdgeId) -> Option<(LoopId, LoopId)> {
    let la = bm.edge(e).l?;
    let lb = bm.lp(la).radial_next;
    (la != lb && bm.lp(lb).radial_next == la).then_some((la, lb))
}

/// `BM_edge_face_pair`.
pub(crate) fn edge_face_pair(bm: &BMesh, e: EdgeId) -> Option<(FaceId, FaceId)> {
    edge_loop_pair(bm, e).map(|(a, b)| (bm.lp(a).f, bm.lp(b).f))
}

/// `BM_loop_is_adjacent`.
pub(crate) fn loop_is_adjacent(bm: &BMesh, a: LoopId, b: LoopId) -> bool {
    bm.lp(a).next == b || bm.lp(a).prev == b
}

/// `BM_edge_calc_length_squared`.
pub(crate) fn edge_calc_length_squared(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    frap_geom::normals::len_squared_v3v3(bm.vert(a).co, bm.vert(b).co)
}

/// `BM_vert_calc_edge_angle`: π minus the angle between the two edges of an edge-pair vertex,
/// 90° otherwise.
pub(crate) fn vert_calc_edge_angle(bm: &BMesh, v: VertId) -> f32 {
    match vert_edge_pair(bm, v) {
        Some((e1, e2)) => {
            let v1 = bm.edge_other_vert(e1, v);
            let v2 = bm.edge_other_vert(e2, v);
            core::f32::consts::PI - angle_v3v3v3(bm.vert(v1).co, bm.vert(v).co, bm.vert(v2).co)
        }
        None => deg2radf(90.0),
    }
}

/// `BM_edge_calc_face_angle`: the angle between the normals of a manifold edge's faces, 90°
/// otherwise.
pub(crate) fn edge_calc_face_angle(bm: &BMesh, e: EdgeId) -> f32 {
    if bm.edge_is_manifold(e) {
        let l1 = bm.edge(e).l.expect("manifold edge has loops");
        let l2 = bm.lp(l1).radial_next;
        angle_normalized_v3v3(bm.face(bm.lp(l1).f).no, bm.face(bm.lp(l2).f).no)
    } else {
        deg2radf(90.0)
    }
}

// ==================================================================== flags

/// `BM_elem_flag_merge_ex(a, b, hflag_and)`: both elements get the union of their flags,
/// except `hflag_and`, which survives only when both have it.
pub(crate) fn elem_flag_merge_ex(bm: &mut BMesh, a: BMElem, b: BMElem, hflag_and: ElemFlag) {
    let (mut fa, mut fb) = (bm.elem_flag(a), bm.elem_flag(b));
    if (fa & fb & hflag_and).is_empty() {
        fa.remove(hflag_and);
        fb.remove(hflag_and);
    }
    let u = fa | fb;
    *bm.elem_flag_mut(a) = u;
    *bm.elem_flag_mut(b) = u;
}

// ==================================================================== Euler helpers

/// `BM_faces_join_pair(l_a, l_b, do_del, nullptr)`: joins the faces of two loops of one
/// edge, reversing `l_b`'s face first when both loops start at the same vertex.
pub(crate) fn faces_join_pair(
    bm: &mut BMesh,
    l_a: LoopId,
    l_b: LoopId,
    do_del: bool,
) -> Option<FaceId> {
    debug_assert!(l_a != l_b && bm.lp(l_a).e == bm.lp(l_b).e);
    if bm.lp(l_a).v == bm.lp(l_b).v {
        let f = bm.lp(l_b).f;
        bm.loop_reverse(f);
    }
    let faces = [bm.lp(l_a).f, bm.lp(l_b).f];
    bm.faces_join(&faces, do_del, false).0
}

/// `BM_vert_collapse_edge(e_kill, v_kill, do_del, kill_degenerate, kill_duplicate)`
/// (`BM_vert_collapse_faces` with `fac = 1` and `join_faces = false`): removes the
/// edge-pair vertex `v_kill`, joining its two edges; the corners of `v_kill` take the data
/// of the corners at the other end of `e_kill`. Returns the surviving edge.
pub(crate) fn vert_collapse_edge(
    bm: &mut BMesh,
    e_kill: EdgeId,
    v_kill: VertId,
    do_del: bool,
    kill_degenerate_faces: bool,
    kill_duplicate_faces: bool,
) -> Option<EdgeId> {
    let fac = 1.0f32;
    let tv = bm.edge_other_vert(e_kill, v_kill);
    // First modify the face loop data.
    if let Some(first) = bm.edge(e_kill).l {
        let w = [1.0 - fac, fac];
        let mut l_iter = first;
        loop {
            let (lv, ln) = (bm.lp(l_iter).v, bm.lp(l_iter).next);
            if lv == tv && bm.lp(ln).v == v_kill {
                let (tvloop, kvloop) = (l_iter, ln);
                bm.ldata.interp(&[kvloop.0, tvloop.0], &w, kvloop.0);
            }
            l_iter = bm.lp(l_iter).radial_next;
            if l_iter == first {
                break;
            }
        }
    }
    // Now interpolate the vertex data.
    bm.data_interp_from_verts(v_kill, tv, v_kill, fac);
    bm.join_edge_kill_vert(
        e_kill,
        v_kill,
        do_del,
        true,
        kill_degenerate_faces,
        kill_duplicate_faces,
    )
}

/// `bmesh_face_swap_data(f_a, f_b)`: swaps the loops (and so the geometry), flags, normal and
/// material of two faces while each keeps its slot, custom data and index. frap also swaps the
/// tool flags of every layer (Blender swaps the `oflags` pointer when the caller asks).
pub(crate) fn face_swap_data(bm: &mut BMesh, f_a: FaceId, f_b: FaceId) {
    debug_assert_ne!(f_a, f_b);
    for l in bm.face_loops(f_a) {
        bm.lp_mut(l).f = f_b;
    }
    let first_b = bm.face(f_b).l_first;
    let mut l = first_b;
    loop {
        // The loops of `f_b` (still linked as a cycle) now belong to `f_a`.
        bm.lp_mut(l).f = f_a;
        l = bm.lp(l).next;
        if l == first_b {
            break;
        }
    }
    let a = bm.face(f_a).clone();
    let b = bm.face(f_b).clone();
    *bm.face_mut(f_a) = b;
    *bm.face_mut(f_b) = a;
    // `bm->act_face` is a pointer to the slot: it stays where it is.
    for layer in &mut bm.oflags.layers {
        let col = &mut layer[2];
        let need = f_a.slot().max(f_b.slot()) + 1;
        if col.len() < need {
            col.resize(need, 0);
        }
        col.swap(f_a.slot(), f_b.slot());
    }
}

/// `BM_select_history_merge_from_targetmap(bm, map, map, map, use_chain = true)`: every
/// history entry follows `map` to its final target; an entry is kept only when that target
/// was itself in the history before (first occurrence wins).
pub(crate) fn select_history_merge_from_targetmap(
    bm: &mut BMesh,
    map: &rustc_hash::FxHashMap<BMElem, BMElem>,
) {
    let original: Vec<BMElem> = bm.select_history.clone();
    let mut overlap: rustc_hash::FxHashSet<BMElem> = original.iter().copied().collect();
    let mut mapped = Vec::with_capacity(original.len());
    for &ele in &original {
        let mut dst = ele;
        while let Some(&next) = map.get(&dst) {
            dst = next;
            if dst == ele {
                break;
            }
        }
        mapped.push(dst);
    }
    bm.select_history = mapped.into_iter().filter(|e| overlap.remove(e)).collect();
}

// ==================================================================== math (BLI)

/// `DEG2RADF(x)`: `x * float(M_PI / 180.0)`.
pub(crate) fn deg2radf(x: f32) -> f32 {
    x * (core::f64::consts::PI / 180.0) as f32
}

/// `RAD2DEGF(x)`: `x * float(180.0 / M_PI)`.
pub(crate) fn rad2degf(x: f32) -> f32 {
    x * (180.0 / core::f64::consts::PI) as f32
}

/// `safe_asinf`.
fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -core::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        core::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `len_v3v3`.
fn len_v3v3(a: V3, b: V3) -> f32 {
    frap_geom::normals::len_v3v3(a, b)
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if geom::dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    core::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `angle_v3v3`.
pub(crate) fn angle_v3v3(a: V3, b: V3) -> f32 {
    angle_normalized_v3v3(geom::normalize_v3(a).0, geom::normalize_v3(b).0)
}

/// `angle_v3v3v3`: the angle at `b` of the corner `a`–`b`–`c`.
pub(crate) fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    let v1 = geom::normalize_v3(geom::sub(b, a)).0;
    let v2 = geom::normalize_v3(geom::sub(b, c)).0;
    angle_normalized_v3v3(v1, v2)
}

/// `cos_v3v3v3`.
pub(crate) fn cos_v3v3v3(p1: V3, p2: V3, p3: V3) -> f32 {
    let v1 = geom::normalize_v3(geom::sub(p2, p1)).0;
    let v2 = geom::normalize_v3(geom::sub(p2, p3)).0;
    geom::dot(v1, v2)
}

/// `project_plane_normalized_v3_v3v3`.
fn project_plane_normalized(p: V3, v_plane: V3) -> V3 {
    let mul = geom::dot(p, v_plane);
    let mut out = p;
    geom::madd_v3_v3fl(&mut out, v_plane, -mul);
    out
}

/// `angle_on_axis_v3v3v3_v3`: the angle at `v2` of `v1`–`v2`–`v3` projected on the plane of
/// `axis`.
pub(crate) fn angle_on_axis_v3v3v3_v3(v1: V3, v2: V3, v3: V3, axis: V3) -> f32 {
    let a = project_plane_normalized(geom::sub(v1, v2), axis);
    let b = project_plane_normalized(geom::sub(v3, v2), axis);
    angle_v3v3(a, b)
}

/// `interpf(target, origin, fac)`.
pub(crate) fn interpf(target: f32, origin: f32, fac: f32) -> f32 {
    (fac * target) + (1.0 - fac) * origin
}

// ==================================================================== loop-data math

/// The value of a loop layer with "math" callbacks (`CustomData_layer_has_math`: `FLOAT2`,
/// `BYTE_COLOR`, `FLOAT_COLOR`), in the representation the callbacks use.
#[derive(Copy, Clone, Debug, PartialEq)]
pub(crate) enum MathValue {
    /// `CD_PROP_FLOAT2`.
    Float2([f32; 2]),
    /// `CD_PROP_BYTE_COLOR` (`MLoopCol`).
    Byte([u8; 4]),
    /// `CD_PROP_COLOR` (`MPropCol`).
    Color([f32; 4]),
}

impl MathValue {
    /// `multiply(data, fac)`.
    pub(crate) fn multiply(&mut self, fac: f32) {
        match self {
            MathValue::Float2(v) => {
                v[0] *= fac;
                v[1] *= fac;
            }
            // `m->r = float(m->r) * fac` stores through an implicit float→uchar conversion.
            MathValue::Byte(c) => {
                for x in c.iter_mut() {
                    *x = (f32::from(*x) * fac) as u8;
                }
            }
            MathValue::Color(c) => {
                for x in c.iter_mut() {
                    *x *= fac;
                }
            }
        }
    }
    /// `add(data1, data2)`.
    pub(crate) fn add(&mut self, o: &MathValue) {
        match (self, o) {
            (MathValue::Float2(a), MathValue::Float2(b)) => {
                a[0] += b[0];
                a[1] += b[1];
            }
            // `m->r += m2->r` on `unsigned char` wraps.
            (MathValue::Byte(a), MathValue::Byte(b)) => {
                for k in 0..4 {
                    a[k] = a[k].wrapping_add(b[k]);
                }
            }
            (MathValue::Color(a), MathValue::Color(b)) => {
                for k in 0..4 {
                    a[k] += b[k];
                }
            }
            _ => {}
        }
    }
}

/// Positions of the loop layers with math callbacks, in layer order.
pub(crate) fn math_layers(bm: &BMesh) -> Vec<usize> {
    bm.ldata
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            matches!(
                l.data,
                AttrData::Float2(_) | AttrData::ByteColor(_) | AttrData::FloatColor(_)
            )
        })
        .map(|(i, _)| i)
        .collect()
}

/// The value of loop `l` in math layer `layer`.
pub(crate) fn math_get(bm: &BMesh, layer: usize, l: LoopId) -> MathValue {
    let s = l.slot();
    match &bm.ldata.layers[layer].data {
        AttrData::Float2(c) => MathValue::Float2(c[s]),
        AttrData::ByteColor(c) => MathValue::Byte(c[s]),
        AttrData::FloatColor(c) => MathValue::Color(c[s]),
        _ => unreachable!("not a math layer"),
    }
}

/// Writes `v` to loop `l` of math layer `layer` (`copyvalue` with `CDT_MIX_NOMIX`).
pub(crate) fn math_set(bm: &mut BMesh, layer: usize, l: LoopId, v: MathValue) {
    let s = l.slot();
    match (&mut bm.ldata.layers[layer].data, v) {
        (AttrData::Float2(c), MathValue::Float2(x)) => Arc::make_mut(c)[s] = x,
        (AttrData::ByteColor(c), MathValue::Byte(x)) => Arc::make_mut(c)[s] = x,
        (AttrData::FloatColor(c), MathValue::Color(x)) => Arc::make_mut(c)[s] = x,
        _ => unreachable!("not a math layer"),
    }
}

/// `equal(data1, data2)` (`layerEqual_propfloat2`, `layerEqual_mloopcol`,
/// `layerEqual_propcol`).
pub(crate) fn math_equal(a: &MathValue, b: &MathValue) -> bool {
    match (a, b) {
        (MathValue::Float2(a), MathValue::Float2(b)) => {
            let d = [a[0] - b[0], a[1] - b[1]];
            d[0] * d[0] + d[1] * d[1] < 0.00001
        }
        (MathValue::Byte(a), MathValue::Byte(b)) => {
            let d: Vec<f32> = (0..4).map(|k| f32::from(a[k]) - f32::from(b[k])).collect();
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3] < 0.001
        }
        (MathValue::Color(a), MathValue::Color(b)) => {
            let mut tot = 0.0f32;
            for k in 0..4 {
                let c = a[k] - b[k];
                tot += c * c;
            }
            tot < 0.001
        }
        _ => false,
    }
}

/// A running min/max of one math layer (`initminmax` / `dominmax`), then the bounding-box
/// centre (`multiply(min, 0.5)`, `multiply(max, 0.5)`, `add(min, max)`).
pub(crate) struct MinMax {
    min: MathValue,
    max: MathValue,
}

impl MinMax {
    /// `initminmax` for the type of `like`.
    pub(crate) fn new(like: &MathValue) -> MinMax {
        let (min, max) = match like {
            MathValue::Float2(_) => (
                MathValue::Float2([f32::MAX; 2]),
                MathValue::Float2([-f32::MAX; 2]),
            ),
            MathValue::Byte(_) => (MathValue::Byte([255; 4]), MathValue::Byte([0; 4])),
            // `copy_v4_fl(max->color, FLT_MIN)`: the smallest positive float, as in Blender.
            MathValue::Color(_) => (
                MathValue::Color([f32::MAX; 4]),
                MathValue::Color([f32::MIN_POSITIVE; 4]),
            ),
        };
        MinMax { min, max }
    }
    /// `dominmax`.
    pub(crate) fn add(&mut self, v: &MathValue) {
        match (&mut self.min, &mut self.max, v) {
            (MathValue::Float2(a), MathValue::Float2(b), MathValue::Float2(x)) => {
                for k in 0..2 {
                    a[k] = a[k].min(x[k]);
                    b[k] = b[k].max(x[k]);
                }
            }
            (MathValue::Byte(a), MathValue::Byte(b), MathValue::Byte(x)) => {
                for k in 0..4 {
                    a[k] = a[k].min(x[k]);
                    b[k] = b[k].max(x[k]);
                }
            }
            (MathValue::Color(a), MathValue::Color(b), MathValue::Color(x)) => {
                // `minmax_v4v4_v4`.
                for k in 0..4 {
                    if a[k] > x[k] {
                        a[k] = x[k];
                    }
                    if b[k] < x[k] {
                        b[k] = x[k];
                    }
                }
            }
            _ => {}
        }
    }
    /// The centre of the box.
    pub(crate) fn center(mut self) -> MathValue {
        self.min.multiply(0.5);
        self.max.multiply(0.5);
        self.min.add(&self.max);
        self.min
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    #[test]
    fn angles_match_blender_formulas() {
        let right = angle_v3v3v3([1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0]);
        assert!((right - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        let straight = angle_v3v3v3([1.0, 0.0, 0.0], [0.0; 3], [-1.0, 0.0, 0.0]);
        assert!((straight - core::f32::consts::PI).abs() < 1e-6);
        assert!((cos_v3v3v3([1.0, 0.0, 0.0], [0.0; 3], [-1.0, 0.0, 0.0]) + 1.0).abs() < 1e-6);
        // Projected on the XY plane the corner is a right angle.
        let a =
            angle_on_axis_v3v3v3_v3([1.0, 0.0, 5.0], [0.0; 3], [0.0, 1.0, -3.0], [0.0, 0.0, 1.0]);
        assert!((a - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert_eq!(interpf(10.0, 0.0, 0.25), 2.5);
        assert!((deg2radf(180.0) - core::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn flag_merge_keeps_hidden_only_when_both_are() {
        let mut bm = BMesh::new();
        let a = bm.vert_create([0.0; 3], None, CreateFlag::empty());
        let b = bm.vert_create([1.0; 3], None, CreateFlag::empty());
        bm.vert_mut(a).flag = ElemFlag::SELECT | ElemFlag::HIDDEN;
        bm.vert_mut(b).flag = ElemFlag::TAG;
        elem_flag_merge_ex(&mut bm, BMElem::Vert(a), BMElem::Vert(b), ElemFlag::HIDDEN);
        assert_eq!(bm.vert(a).flag, ElemFlag::SELECT | ElemFlag::TAG);
        assert_eq!(bm.vert(b).flag, ElemFlag::SELECT | ElemFlag::TAG);
    }

    #[test]
    fn history_follows_the_map_and_keeps_first_survivors() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = (0..4)
            .map(|i| bm.vert_create([i as f32, 0.0, 0.0], None, CreateFlag::empty()))
            .collect();
        bm.select_history = vec![BMElem::Vert(v[1]), BMElem::Vert(v[0]), BMElem::Vert(v[2])];
        let mut map = rustc_hash::FxHashMap::default();
        map.insert(BMElem::Vert(v[1]), BMElem::Vert(v[0]));
        map.insert(BMElem::Vert(v[2]), BMElem::Vert(v[3]));
        select_history_merge_from_targetmap(&mut bm, &map);
        // v1 -> v0 (in the history: kept once), v2 -> v3 (never in the history: dropped).
        assert_eq!(bm.select_history, vec![BMElem::Vert(v[0])]);
    }

    #[test]
    fn byte_colour_math_wraps_like_unsigned_char() {
        let mut a = MathValue::Byte([200, 10, 255, 1]);
        a.add(&MathValue::Byte([100, 10, 1, 1]));
        assert_eq!(a, MathValue::Byte([44, 20, 0, 2]));
        a.multiply(0.5);
        assert_eq!(a, MathValue::Byte([22, 10, 0, 1]));
        let mut mm = MinMax::new(&MathValue::Float2([0.0; 2]));
        mm.add(&MathValue::Float2([0.0, 1.0]));
        mm.add(&MathValue::Float2([2.0, -1.0]));
        assert_eq!(mm.center(), MathValue::Float2([1.0, 0.0]));
        assert!(math_equal(
            &MathValue::Float2([0.0, 0.0]),
            &MathValue::Float2([0.001, 0.0])
        ));
        assert!(!math_equal(
            &MathValue::Float2([0.0, 0.0]),
            &MathValue::Float2([0.01, 0.0])
        ));
    }
}
