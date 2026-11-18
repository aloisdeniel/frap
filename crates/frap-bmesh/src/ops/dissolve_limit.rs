// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_limit` (Blender 5.2.4 `bmo_dissolve_limit_exec`, `operators/
//! bmo_dissolve.cc`, and `BM_mesh_decimate_dissolve_ex`, `tools/bmesh_decimate_dissolve.cc`),
//! implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_limit(bmesh, angle_limit=0.0, use_dissolve_boundaries=False, verts=[], edges=[], delimit={})`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/tools/
//! bmesh_decimate_dissolve.cc): `angle_limit` is capped at 90°. The input edges go on a
//! min-heap (`BLI_heap`, ported in `dissolve_limit/heap.rs`) keyed by the negated cosine of
//! the angle between their faces (delimited or non-manifold edges never dissolve); while the
//! cheapest edge is flatter than the limit, its two faces are joined (`BM_faces_join_pair`:
//! the merged face is **created**, the two old ones freed) and the costs of the new face's
//! edges are updated. Edges that became wire are then killed from the **last** edge to the
//! first, with their vertices when left loose. Then the input vertices between exactly two
//! edges are dissolved (`BM_vert_collapse_edge`): all of them in input order with
//! `use_dissolve_boundaries`, otherwise cheapest first by the angle between their edges
//! (scaled by the face angle), skipping collapses that would flip or overlap a face corner.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

mod heap;

use frap_mesh::AttrData;
use rustc_hash::FxHashMap;

use crate::ops::dissolve_verts::mods;
use crate::query::geom::{self, V3};
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};
use heap::{Heap, NodeId};

/// Input slots of `dissolve_limit` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveLimitIn {
    /// `angle_limit`.
    pub angle_limit: f32,
    /// `use_dissolve_boundaries`.
    pub use_dissolve_boundaries: bool,
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `delimit` (enum-flag identifiers).
    pub delimit: Vec<&'static str>,
}

/// Output slots of `dissolve_limit`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveLimitOut {
    /// `region.out` as elements.
    pub geom: Vec<BMElem>,
    /// `region.out`: the faces created by joins, in face order.
    pub region: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_dissolve_limit_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const FACE_NEW: u16 = 4;
const COST_INVALID: f32 = f32::MAX;

/// `BMO_Delimit`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
struct Delimit {
    normal: bool,
    material: bool,
    seam: bool,
    sharp: bool,
    uv: bool,
}

impl Delimit {
    fn any(self) -> bool {
        self.normal || self.material || self.seam || self.sharp || self.uv
    }
}

/// Runs `dissolve_limit` on `bm`.
pub fn dissolve_limit(bm: &mut BMesh, i: &DissolveLimitIn) -> Result<DissolveLimitOut, BmError> {
    let mut d = Delimit::default();
    for &s in &i.delimit {
        match s {
            "NORMAL" => d.normal = true,
            "MATERIAL" => d.material = true,
            "SEAM" => d.seam = true,
            "SHARP" => d.sharp = true,
            "UV" => d.uv = true,
            other => {
                return Err(BmError::Invalid(format!(
                    "dissolve_limit: unknown delimit '{other}' (expected NORMAL, MATERIAL, \
                     SEAM, SHARP or UV)"
                )));
            }
        }
    }
    if let Some(v) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!("dissolve_limit: dead vertex {v:?}")));
    }
    if let Some(e) = i.edges.iter().find(|e| !bm.edges.contains(e.0)) {
        return Err(BmError::Invalid(format!("dissolve_limit: dead edge {e:?}")));
    }
    let region = bm.bmo_exec(TYPE_FLAG, |bm| {
        let angle_max = core::f32::consts::FRAC_PI_2;
        let angle_limit = angle_max.min(i.angle_limit);
        decimate_dissolve_ex(
            bm,
            angle_limit,
            i.use_dissolve_boundaries,
            d,
            &i.verts,
            &i.edges,
            FACE_NEW,
        );
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, FACE_NEW))
            .collect::<Vec<_>>()
    });
    Ok(DissolveLimitOut {
        geom: region.iter().map(|&f| BMElem::Face(f)).collect(),
        region,
    })
}

/// The loop layers the `UV` delimit compares (every `CD_PROP_FLOAT2` layer).
fn uv_layers(bm: &BMesh) -> Vec<usize> {
    bm.ldata
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| matches!(l.data, AttrData::Float2(_)))
        .map(|(i, _)| i)
        .collect()
}

/// `BM_edge_is_contiguous_loop_cd` for one `FLOAT2` layer.
fn edge_is_contiguous_loop_cd(bm: &BMesh, e: EdgeId, layer: usize) -> bool {
    let Some(l) = bm.edge(e).l else {
        return true;
    };
    if bm.lp(l).radial_next == l {
        return true;
    }
    let base_v1 = l;
    let base_v2 = bm.lp(l).next;
    let get = |x: LoopId| mods::math_get(bm, layer, x);
    let mut it = bm.lp(l).radial_next;
    loop {
        let li = bm.lp(it);
        let (i1, i2) = if li.v == bm.lp(base_v1).v {
            (it, li.next)
        } else {
            (li.next, it)
        };
        if !mods::math_equal(&get(base_v1), &get(i1)) || !mods::math_equal(&get(base_v2), &get(i2))
        {
            return false;
        }
        it = li.radial_next;
        if it == l {
            break;
        }
    }
    true
}

struct Ctx {
    delimit: Delimit,
    uv_layers: Vec<usize>,
}

impl Ctx {
    /// `bm_edge_is_delimiter` (the caller ensures a manifold edge).
    fn edge_is_delimiter(&self, bm: &BMesh, e: EdgeId) -> bool {
        let d = self.delimit;
        if !d.any() {
            return false;
        }
        let ed = bm.edge(e);
        if d.seam && ed.flag.contains(ElemFlag::SEAM) {
            return true;
        }
        if d.sharp && !ed.flag.contains(ElemFlag::SMOOTH) {
            return true;
        }
        if d.material {
            let l = ed.l.expect("manifold");
            let r = bm.lp(l).radial_next;
            if bm.face(bm.lp(l).f).mat_nr != bm.face(bm.lp(r).f).mat_nr {
                return true;
            }
        }
        if d.normal && !bm.edge_is_contiguous(e) {
            return true;
        }
        if d.uv
            && !self
                .uv_layers
                .iter()
                .all(|&layer| edge_is_contiguous_loop_cd(bm, e, layer))
        {
            return true;
        }
        false
    }

    /// `bm_vert_is_delimiter`.
    fn vert_is_delimiter(&self, bm: &BMesh, v: VertId) -> bool {
        self.delimit.any()
            && bm
                .vert_edges(v)
                .into_iter()
                .any(|e| bm.edge_is_manifold(e) && self.edge_is_delimiter(bm, e))
    }

    /// `bm_vert_edge_face_angle`.
    fn vert_edge_face_angle(&self, bm: &BMesh, v: VertId) -> f32 {
        let unit_to_angle = mods::deg2radf(90.0);
        let angle_to_unit = 1.0 / unit_to_angle;
        let angle = mods::vert_calc_edge_angle(bm, v);
        if let Some(ve) = bm.vert(v).e
            && bm.edge_is_manifold(ve)
            && !self.vert_is_delimiter(bm, v)
        {
            return ((angle * angle_to_unit) * (mods::edge_calc_face_angle(bm, ve) * angle_to_unit))
                * unit_to_angle;
        }
        angle
    }

    /// `bm_edge_calc_dissolve_error`.
    fn edge_dissolve_error(&self, bm: &BMesh, e: EdgeId) -> f32 {
        if bm.edge_is_manifold(e) && !self.edge_is_delimiter(bm, e) {
            let l = bm.edge(e).l.expect("manifold");
            let r = bm.lp(l).radial_next;
            let mut angle_cos_neg = geom::dot(bm.face(bm.lp(l).f).no, bm.face(bm.lp(r).f).no);
            if bm.edge_is_contiguous(e) {
                angle_cos_neg *= -1.0;
            }
            return angle_cos_neg;
        }
        COST_INVALID
    }
}

/// `mul_v2_m3v3_center`.
fn proj_center(rows: &[V3; 2], a: V3, center: V3) -> [f32; 2] {
    geom::project_v2(rows, geom::sub(a, center))
}

fn cross_tri_v2(v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> f32 {
    (v1[0] - v2[0]) * (v2[1] - v3[1]) + (v1[1] - v2[1]) * (v3[0] - v2[0])
}

fn signum_i(a: f32) -> i32 {
    if a > 0.0 {
        1
    } else if a < 0.0 {
        -1
    } else {
        0
    }
}

fn line_point_side_v2(l1: [f32; 2], l2: [f32; 2], pt: [f32; 2]) -> f32 {
    ((l1[0] - pt[0]) * (l2[1] - pt[1])) - ((l2[0] - pt[0]) * (l1[1] - pt[1]))
}

fn isect_point_tri_v2_cw(pt: [f32; 2], v1: [f32; 2], v2: [f32; 2], v3: [f32; 2]) -> bool {
    line_point_side_v2(v1, v2, pt) >= 0.0
        && line_point_side_v2(v2, v3, pt) >= 0.0
        && line_point_side_v2(v3, v1, pt) >= 0.0
}

/// `bm_loop_collapse_is_degenerate`.
fn loop_collapse_is_degenerate(bm: &BMesh, l_ear: LoopId) -> bool {
    let le = bm.lp(l_ear).clone();
    let center = bm.vert(le.v).co;
    let rows = geom::axis_dominant_v3_rows(bm.face(le.f).no);
    let co = |l: LoopId| bm.vert(bm.lp(l).v).co;
    let mut tri = [
        proj_center(&rows, co(le.prev), center),
        [0.0, 0.0],
        proj_center(&rows, co(le.next), center),
    ];
    // Check we're not flipping face corners before or after the ear.
    if !mods::vert_is_edge_pair(bm, bm.lp(le.prev).v) {
        let adjacent = proj_center(&rows, co(bm.lp(le.prev).prev), center);
        if signum_i(cross_tri_v2(adjacent, tri[0], tri[1]))
            != signum_i(cross_tri_v2(adjacent, tri[0], tri[2]))
        {
            return true;
        }
    }
    if !mods::vert_is_edge_pair(bm, bm.lp(le.next).v) {
        let adjacent = proj_center(&rows, co(bm.lp(le.next).next), center);
        if signum_i(cross_tri_v2(adjacent, tri[2], tri[1]))
            != signum_i(cross_tri_v2(adjacent, tri[2], tri[0]))
        {
            return true;
        }
    }
    // Check no existing verts are inside the triangle (flip a concave one to clockwise).
    if cross_tri_v2(tri[0], tri[1], tri[2]) < 0.0 {
        tri.swap(1, 2);
    }
    let l_first = le.prev;
    let mut l_iter = bm.lp(le.next).next;
    loop {
        let p = proj_center(&rows, co(l_iter), center);
        if isect_point_tri_v2_cw(p, tri[0], tri[1], tri[2]) {
            return true;
        }
        l_iter = bm.lp(l_iter).next;
        if l_iter == l_first {
            break;
        }
    }
    false
}

/// `bm_vert_collapse_is_degenerate`.
fn vert_collapse_is_degenerate(bm: &BMesh, v: VertId) -> bool {
    let Some((e0, e1)) = mods::vert_edge_pair(bm, v) else {
        return true;
    };
    // Allow wire edges.
    if bm.edge_is_wire(e0) || bm.edge_is_wire(e1) {
        return false;
    }
    let v0 = bm.edge_other_vert(e0, v);
    let v1 = bm.edge_other_vert(e1, v);
    if mods::cos_v3v3v3(bm.vert(v0).co, bm.vert(v).co, bm.vert(v1).co).abs()
        < (1.0 - f32::EPSILON)
    {
        for l in bm.edge_loops(e1) {
            if bm.face(bm.lp(l).f).len > 3 {
                let l_pivot = if bm.lp(l).v == v { l } else { bm.lp(l).next };
                if loop_collapse_is_degenerate(bm, l_pivot) {
                    return true;
                }
            }
        }
    }
    false
}

/// `BM_mesh_decimate_dissolve_ex`.
fn decimate_dissolve_ex(
    bm: &mut BMesh,
    angle_limit: f32,
    do_dissolve_boundaries: bool,
    mut delimit: Delimit,
    vinput: &[VertId],
    einput: &[EdgeId],
    oflag_out: u16,
) {
    let angle_limit_cos_neg = -libm::cosf(angle_limit);
    let uv = uv_layers(bm);
    if delimit.uv && uv.is_empty() {
        delimit.uv = false;
    }
    let cx = Ctx {
        delimit,
        uv_layers: uv,
    };
    let mut vinput: Vec<Option<VertId>> = vinput.iter().copied().map(Some).collect();

    // --- first edges ---
    {
        let mut eheap: Heap<EdgeId> = Heap::new(einput.len());
        let mut table: Vec<Option<NodeId>> = vec![None; einput.len()];
        // Wire -> tag; the edge index (`BM_elem_index_set`) is a scratch map here.
        let mut eindex: FxHashMap<EdgeId, usize> = FxHashMap::default();
        for e in bm.edge_ids() {
            let wire = bm.edge_is_wire(e);
            bm.edge_mut(e).flag.set(ElemFlag::TAG, wire);
        }
        for (i, &e) in einput.iter().enumerate() {
            let cost = cx.edge_dissolve_error(bm, e);
            table[i] = Some(eheap.insert(cost, e));
            eindex.insert(e, i);
        }
        while let Some(top) = eheap.top() {
            if eheap.value(top) >= angle_limit_cos_neg {
                break;
            }
            frap_math::budget::check();
            let e = eheap.ptr(top);
            let i = eindex[&e];
            let mut f_new = None;
            if bm.edge_is_manifold(e) {
                let l = bm.edge(e).l.expect("manifold");
                let r = bm.lp(l).radial_next;
                // `f_new` may be an existing face (#144383); it is still flagged as output.
                f_new = mods::faces_join_pair(bm, l, r, false);
                if let Some(f_new) = f_new {
                    eheap.remove(top);
                    table[i] = None;
                    bm.face_normal_update(f_new);
                    if oflag_out != 0 {
                        bm.oflag_enable(f_new, oflag_out);
                    }
                    // Re-calculate costs.
                    for l in bm.face_loops(f_new) {
                        let le = bm.lp(l).e;
                        if let Some(&j) = eindex.get(&le)
                            && let Some(n) = table[j]
                        {
                            let cost = cx.edge_dissolve_error(bm, le);
                            eheap.update(n, cost);
                        }
                    }
                }
            }
            if f_new.is_none() {
                eheap.update(top, COST_INVALID);
            }
        }

        // Prepare for cleanup.
        let mut vert_reverse_lookup: FxHashMap<VertId, usize> = FxHashMap::default();
        for (i, v) in vinput.iter().enumerate() {
            if let Some(v) = v {
                vert_reverse_lookup.insert(*v, i);
            }
        }
        // Remove all edges/verts left behind from dissolving, nulling the vertex array so we
        // don't re-use.
        let earray = bm.edge_ids();
        for &e in earray.iter().rev() {
            if !bm.edges.contains(e.0) {
                continue;
            }
            if bm.edge_is_wire(e) && !bm.edge(e).flag.contains(ElemFlag::TAG) {
                // Edge has become wire.
                let [v1, v2] = bm.edge(e).v;
                bm.edge_kill(e);
                for v in [v1, v2] {
                    if bm.verts.contains(v.0) && bm.vert(v).e.is_none() {
                        if let Some(&r) = vert_reverse_lookup.get(&v) {
                            vinput[r] = None;
                        }
                        bm.vert_kill(v);
                    }
                }
            }
        }
    }

    // --- second verts ---
    if do_dissolve_boundaries {
        // Simple version: dissolve all verts that use 2 edges.
        for v in vinput.iter().flatten() {
            let v = *v;
            if bm.verts.contains(v.0) && mods::vert_is_edge_pair(bm, v) {
                let e = bm.vert(v).e.expect("edge pair");
                mods::vert_collapse_edge(bm, e, v, true, true, true);
            }
        }
    } else {
        let mut vheap: Heap<VertId> = Heap::new(vinput.len());
        let mut table: Vec<Option<NodeId>> = vec![None; vinput.len()];
        let mut vindex: FxHashMap<VertId, usize> = FxHashMap::default();
        for (i, v) in vinput.iter().enumerate() {
            if let Some(v) = *v {
                let cost = cx.vert_edge_face_angle(bm, v);
                table[i] = Some(vheap.insert(cost, v));
                vindex.insert(v, i);
            }
        }
        while let Some(top) = vheap.top() {
            if vheap.value(top) >= angle_limit {
                break;
            }
            frap_math::budget::check();
            let v = vheap.ptr(top);
            let i = vindex[&v];
            let mut e_new = None;
            if !vert_collapse_is_degenerate(bm, v) {
                let ve = bm.vert(v).e.expect("edge pair");
                // Join edges.
                e_new = mods::vert_collapse_edge(bm, ve, v, true, true, true);
                if let Some(en) = e_new {
                    vheap.remove(top);
                    table[i] = None;
                    // Update normals.
                    for f in bm.edge_faces(en) {
                        bm.face_normal_update(f);
                    }
                    // Re-calculate costs.
                    for x in bm.edge(en).v {
                        if let Some(&j) = vindex.get(&x)
                            && let Some(n) = table[j]
                        {
                            let cost = cx.vert_edge_face_angle(bm, x);
                            vheap.update(n, cost);
                        }
                    }
                    // Dissolving a vertex may mean vertices we previously weren't able to
                    // dissolve can now be re-evaluated.
                    for l in bm.edge_loops(en) {
                        // Skip vertices part of this edge, evaluated above.
                        let l_cycle_first = bm.lp(l).prev;
                        let mut l_cycle = bm.lp(bm.lp(l).next).next;
                        loop {
                            let cv = bm.lp(l_cycle).v;
                            if let Some(&j) = vindex.get(&cv)
                                && let Some(n) = table[j]
                                && vheap.value(n) == COST_INVALID
                            {
                                let cost = cx.vert_edge_face_angle(bm, cv);
                                vheap.update(n, cost);
                            }
                            l_cycle = bm.lp(l_cycle).next;
                            if l_cycle == l_cycle_first {
                                break;
                            }
                        }
                    }
                }
            }
            if e_new.is_none() {
                vheap.update(top, COST_INVALID);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CreateFlag;

    /// A flat 3 x 1 strip of quads.
    fn strip() -> BMesh {
        let mut bm = BMesh::new();
        let mut v = Vec::new();
        for y in 0..2 {
            for x in 0..4 {
                v.push(bm.vert_create([x as f32, y as f32, 0.0], None, CreateFlag::empty()));
            }
        }
        for x in 0..3 {
            bm.face_create_verts(&[v[x], v[x + 1], v[x + 5], v[x + 4]], None, CreateFlag::empty(), true);
        }
        bm.normals_update();
        bm
    }

    #[test]
    fn flat_strip_becomes_one_quad() {
        let mut bm = strip();
        let i = DissolveLimitIn {
            angle_limit: mods::deg2radf(5.0),
            verts: bm.vert_ids(),
            edges: bm.edge_ids(),
            delimit: vec!["NORMAL"],
            ..Default::default()
        };
        let out = dissolve_limit(&mut bm, &i).unwrap();
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        assert_eq!(bm.faces_num(), 1);
        assert_eq!(bm.verts_num(), 4);
        assert_eq!(out.region.len(), 1);
    }

    #[test]
    fn unknown_delimit_is_refused() {
        let mut bm = strip();
        let i = DissolveLimitIn {
            delimit: vec!["FOO"],
            ..Default::default()
        };
        assert!(dissolve_limit(&mut bm, &i).is_err());
    }
}
