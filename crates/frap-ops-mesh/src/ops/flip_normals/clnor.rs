// SPDX-License-Identifier: GPL-3.0-or-later
//! Custom split normals of an edit BMesh: the `custom_normal` INT16_2D corner layer, its
//! per-fan normal spaces (`MLoopNorSpaceArray`), and the loop-normal edit data the custom
//! normal tools work on.
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc (Blender 5.2.4,
//! GPL-2.0-or-later): `bm_mesh_loops_calc_normals__single_threaded`,
//! `bm_mesh_loops_calc_normals_for_loop`, `BM_loop_check_cyclic_smooth_fan`,
//! `bm_edge_tag_from_smooth`, `BM_lnorspacearr_store` / `BM_lnorspace_update` (the custom
//! normal layer is ensured, zeros = automatic normals), `bm_loop_normal_mark_*` and
//! `BM_loop_normal_editdata_array_init_with_htype`; and from
//! blender/source/blender/blenkernel/intern/mesh_normals.cc: `corner_fan_space_define`
//! (`BKE_lnor_space_define`) and `corner_space_custom_normal_to_data`
//! (`BKE_lnor_space_custom_normal_to_data`). Decoding is
//! `frap_geom::normals::corner_space_custom_data_to_normal`.
//!
//! Blender keeps the space array on the BMesh and rebuilds it lazily; frap recomputes it every
//! time an operator needs it ([`update`]), which gives the same spaces (every operator that
//! changes them in Blender marks them dirty). Loop indices are face-iteration order
//! (`BM_mesh_elem_index_ensure(bm, BM_LOOP)`).

use std::sync::Arc;

use frap_bmesh::{BMesh, CdLayer, EdgeId, ElemFlag, FaceId, LoopId, SelectMode, VertId};
use frap_geom::normals::{
    CornerNormalSpace, V3, corner_space_custom_data_to_normal, cross, dot, normalize,
    normalize_v3, safe_acos_approx, scale, sub,
};
use frap_mesh::AttrData;

/// The custom normal layer's name.
pub(crate) const LAYER: &str = "custom_normal";

/// `LNOR_SPACE_TRIGO_THRESHOLD`.
const LNOR_SPACE_TRIGO_THRESHOLD: f32 = 1.0 - 1e-4;

/// Whether the edit BMesh has the INT16_2D `custom_normal` corner layer.
pub(crate) fn has_layer(bm: &BMesh) -> bool {
    matches!(bm.ldata.get(LAYER).map(|l| &l.data), Some(AttrData::Int16x2(_)))
}

/// `BM_data_layer_ensure_named(bm, &bm->ldata, CD_PROP_INT16_2D, "custom_normal")`: adds the
/// layer filled with zeros (automatic normals) when missing.
pub(crate) fn ensure_layer(bm: &mut BMesh) {
    if has_layer(bm) {
        return;
    }
    let n = bm.loops.capacity();
    bm.ldata.add_layer(CdLayer {
        name: LAYER.into(),
        data: AttrData::Int16x2(Arc::new(vec![[0; 2]; n])),
    });
}

/// The custom normal data of loop `l` (`[0, 0]` without the layer).
pub(crate) fn get(bm: &BMesh, l: LoopId) -> [i16; 2] {
    match bm.ldata.get(LAYER).map(|x| &x.data) {
        Some(AttrData::Int16x2(v)) => v.get(l.slot()).copied().unwrap_or([0; 2]),
        _ => [0; 2],
    }
}

/// Sets the custom normal data of loop `l` (the layer must exist).
pub(crate) fn set(bm: &mut BMesh, l: LoopId, value: [i16; 2]) {
    let Some(i) = bm.ldata.find(LAYER) else {
        return;
    };
    if let AttrData::Int16x2(v) = &mut bm.ldata.layers[i].data {
        let v = Arc::make_mut(v);
        if v.len() <= l.slot() {
            v.resize(l.slot() + 1, [0; 2]);
        }
        v[l.slot()] = value;
    }
}

/// One smooth fan's normal space (`MLoopNorSpace`).
#[derive(Clone, Debug)]
pub(crate) struct Space {
    /// The space.
    pub space: CornerNormalSpace,
    /// Its loops (`MLoopNorSpace.loops`).
    pub loops: Vec<LoopId>,
    /// `MLNOR_SPACE_IS_SINGLE`: a corner between two sharp edges.
    pub single: bool,
}

/// The normal spaces of every loop (`MLoopNorSpaceArray`), the loop normals and the loop
/// indices.
#[derive(Clone, Debug, Default)]
pub(crate) struct Spaces {
    /// The spaces.
    pub spaces: Vec<Space>,
    /// Space of each loop, by loop slot (`usize::MAX`: none).
    of_slot: Vec<usize>,
    /// Loop normals, by loop slot.
    pub lnors: Vec<V3>,
    /// Loop index (face iteration order) by loop slot.
    index: Vec<usize>,
    /// Loops by index.
    pub loops: Vec<LoopId>,
}

impl Spaces {
    /// The space of loop `l`.
    pub(crate) fn of(&self, l: LoopId) -> Option<&Space> {
        self.of_slot
            .get(l.slot())
            .and_then(|&i| self.spaces.get(i))
    }
    /// The loop index of `l` (face iteration order).
    pub(crate) fn index(&self, l: LoopId) -> usize {
        self.index[l.slot()]
    }
    /// The custom normal of `l` decoded in its space
    /// (`BKE_lnor_space_custom_data_to_normal`).
    pub(crate) fn decode(&self, bm: &BMesh, l: LoopId) -> V3 {
        self.of(l)
            .map_or([0.0; 3], |s| corner_space_custom_data_to_normal(&s.space, get(bm, l)))
    }
    /// `BKE_lnor_space_custom_normal_to_data` in the space of `l`.
    pub(crate) fn encode(&self, l: LoopId, n: V3) -> [i16; 2] {
        self.of(l)
            .map_or([0; 2], |s| custom_normal_to_data(&s.space, n))
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (corner_fan_space_define).
/// `BKE_lnor_space_define`.
pub(crate) fn fan_space_define(
    lnor: V3,
    vec_ref: V3,
    vec_other: V3,
    edge_vectors: &[V3],
) -> CornerNormalSpace {
    let mut space = CornerNormalSpace::default();
    let pi2 = std::f32::consts::PI * 2.0;
    let dtp_ref = dot(vec_ref, lnor);
    let dtp_other = dot(vec_other, lnor);
    if dtp_ref.abs() >= LNOR_SPACE_TRIGO_THRESHOLD || dtp_other.abs() >= LNOR_SPACE_TRIGO_THRESHOLD
    {
        return space;
    }
    space.vec_lnor = lnor;
    if !edge_vectors.is_empty() {
        let mut alpha = 0.0f32;
        for &v in edge_vectors {
            alpha += safe_acos_approx(dot(v, lnor));
        }
        space.ref_alpha = alpha / edge_vectors.len() as f32;
    } else {
        space.ref_alpha =
            (safe_acos_approx(dot(vec_ref, lnor)) + safe_acos_approx(dot(vec_other, lnor))) / 2.0;
    }
    space.vec_ref = normalize(sub(vec_ref, scale(lnor, dtp_ref)));
    space.vec_ortho = normalize(cross(lnor, space.vec_ref));
    let vec_other_proj = normalize(sub(vec_other, scale(lnor, dtp_other)));
    let dtp = dot(space.vec_ref, vec_other_proj);
    if dtp < LNOR_SPACE_TRIGO_THRESHOLD {
        let beta = safe_acos_approx(dtp);
        space.ref_beta = if dot(space.vec_ortho, vec_other_proj) < 0.0 {
            pi2 - beta
        } else {
            beta
        };
    } else {
        space.ref_beta = pi2;
    }
    space
}

/// `unit_float_to_short`.
fn unit_float_to_short(v: f32) -> i16 {
    (v * f32::from(i16::MAX) + 0.5).floor() as i16
}

/// `compare_v3v3(a, b, limit)`.
fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc
// (corner_space_custom_normal_to_data).
/// Encodes `custom` in `space` (zero vector or the automatic normal → `[0, 0]`).
pub(crate) fn custom_normal_to_data(space: &CornerNormalSpace, custom: V3) -> [i16; 2] {
    if custom == [0.0; 3] || compare_v3v3(space.vec_lnor, custom, 1e-4) {
        return [0, 0];
    }
    let pi2 = (std::f64::consts::PI * 2.0) as f32;
    let cos_alpha = dot(space.vec_lnor, custom);
    let alpha = safe_acos_approx(cos_alpha);
    let a = if alpha > space.ref_alpha {
        unit_float_to_short(-(pi2 - alpha) / (pi2 - space.ref_alpha))
    } else {
        unit_float_to_short(alpha / space.ref_alpha)
    };
    let l = scale(space.vec_lnor, -cos_alpha);
    let vec = normalize([l[0] + custom[0], l[1] + custom[1], l[2] + custom[2]]);
    let cos_beta = dot(space.vec_ref, vec);
    let b = if cos_beta < LNOR_SPACE_TRIGO_THRESHOLD {
        let mut beta = safe_acos_approx(cos_beta);
        if dot(space.vec_ortho, vec) < 0.0 {
            beta = pi2 - beta;
        }
        if beta > space.ref_beta {
            unit_float_to_short(-(pi2 - beta) / (pi2 - space.ref_beta))
        } else {
            unit_float_to_short(beta / space.ref_beta)
        }
    } else {
        0
    };
    [a, b]
}

/// `bm_edge_tag_from_smooth` with no split angle: an edge is smooth (TAG) when it is manifold,
/// its faces wind the same way, and it and both faces are smooth.
fn edge_tag_from_smooth(bm: &mut BMesh, e: EdgeId) {
    let Some(l_a) = bm.edge(e).l else {
        return;
    };
    let l_b = bm.lp(l_a).radial_next;
    let smooth = bm.lp(l_b).radial_next == l_a
        && bm.lp(l_a).v != bm.lp(l_b).v
        && bm.edge(e).flag.contains(ElemFlag::SMOOTH)
        && bm.face(bm.lp(l_a).f).flag.contains(ElemFlag::SMOOTH)
        && bm.face(bm.lp(l_b).f).flag.contains(ElemFlag::SMOOTH);
    bm.edge_mut(e).flag.set(ElemFlag::TAG, smooth);
}

fn etag(bm: &BMesh, e: EdgeId) -> bool {
    bm.edge(e).flag.contains(ElemFlag::TAG)
}
fn ltag(bm: &BMesh, l: LoopId) -> bool {
    bm.lp(l).flag.contains(ElemFlag::TAG)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
// (BM_loop_check_cyclic_smooth_fan).
fn check_cyclic_smooth_fan(bm: &mut BMesh, l_curr: LoopId) -> bool {
    let mut pivot = l_curr;
    let mut e_next = bm.lp(l_curr).e;
    bm.lp_mut(pivot).flag.insert(ElemFlag::TAG);
    loop {
        let next = bm.vert_step_fan_loop(pivot, &mut e_next);
        let Some(next) = next else {
            return false;
        };
        if !etag(bm, e_next) {
            return false;
        }
        if ltag(bm, next) {
            return next == l_curr;
        }
        bm.lp_mut(next).flag.insert(ElemFlag::TAG);
        pivot = next;
    }
}

fn co(bm: &BMesh, v: VertId) -> V3 {
    bm.vert(v).co
}

/// One call of `bm_mesh_loops_calc_normals_for_loop` with a space array and custom normals.
fn calc_for_loop(bm: &mut BMesh, out: &mut Spaces, has_clnors: bool, l_curr: LoopId) {
    let lc = bm.lp(l_curr).clone();
    if etag(bm, lc.e) && (ltag(bm, l_curr) || !check_cyclic_smooth_fan(bm, l_curr)) {
        return;
    }
    let prev_e = bm.lp(lc.prev).e;
    if !etag(bm, lc.e) && !etag(bm, prev_e) {
        let no = bm.face(lc.f).no;
        out.lnors[l_curr.slot()] = no;
        let pivot = co(bm, lc.v);
        let vec_curr = normalize_v3(sub(co(bm, bm.lp(lc.next).v), pivot)).0;
        let vec_prev = normalize_v3(sub(co(bm, bm.lp(lc.prev).v), pivot)).0;
        let space = fan_space_define(no, vec_curr, vec_prev, &[]);
        out.of_slot[l_curr.slot()] = out.spaces.len();
        out.spaces.push(Space {
            space,
            loops: vec![l_curr],
            single: true,
        });
        if has_clnors {
            out.lnors[l_curr.slot()] = corner_space_custom_data_to_normal(&space, get(bm, l_curr));
        }
        return;
    }
    let v_pivot = lc.v;
    let co_pivot = co(bm, v_pivot);
    let e_org = lc.e;
    let mut lnor = [0.0f32; 3];
    let mut clnors_avg = [0i32; 2];
    let mut clnor_ref: Option<[i16; 2]> = None;
    let mut clnors_count = 0i32;
    let mut clnors_invalid = false;
    let mut fan: Vec<LoopId> = Vec::new();
    let mut edge_vectors: Vec<V3> = Vec::new();
    let space_index = out.spaces.len();
    let mut pivot = l_curr;
    let mut e_next = e_org;
    let vec_org = normalize_v3(sub(co(bm, bm.lp(lc.next).v), co_pivot)).0;
    let mut vec_curr = vec_org;
    edge_vectors.push(vec_org);
    let mut vec_next;
    loop {
        frap_math::budget::check();
        let next = bm.vert_step_fan_loop(pivot, &mut e_next);
        if next.is_none() {
            let lp = bm.lp(pivot);
            e_next = if lp.e == e_next { bm.lp(lp.prev).e } else { lp.e };
        }
        let other = bm.edge_other_vert(e_next, v_pivot);
        vec_next = normalize_v3(sub(co(bm, other), co_pivot)).0;
        let f = bm.lp(pivot).f;
        let fac = safe_acos_approx(dot(vec_next, vec_curr));
        let no = bm.face(f).no;
        lnor[0] += no[0] * fac;
        lnor[1] += no[1] * fac;
        lnor[2] += no[2] * fac;
        if has_clnors {
            let c = get(bm, pivot);
            match clnor_ref {
                Some(r) => clnors_invalid |= r != c,
                None => clnor_ref = Some(c),
            }
            clnors_avg[0] += i32::from(c[0]);
            clnors_avg[1] += i32::from(c[1]);
            clnors_count += 1;
        }
        fan.push(pivot);
        out.of_slot[pivot.slot()] = space_index;
        if e_next != e_org {
            edge_vectors.push(vec_next);
        }
        if !etag(bm, e_next) || e_next == e_org {
            break;
        }
        vec_curr = vec_next;
        pivot = next.expect("a smooth edge is manifold");
    }
    let (mut lnor_n, mut lnor_len) = normalize_v3(lnor);
    if lnor_len == 0.0 {
        lnor_n = out.lnors[pivot.slot()];
        lnor_len = 1.0;
    }
    let space = fan_space_define(lnor_n, vec_org, vec_next, &edge_vectors);
    let mut fan_normal = lnor_n;
    if has_clnors {
        let mut r = clnor_ref.unwrap_or([0; 2]);
        if clnors_invalid {
            let avg = [
                (clnors_avg[0] / clnors_count) as i16,
                (clnors_avg[1] / clnors_count) as i16,
            ];
            for &l in &fan {
                set(bm, l, avg);
            }
            r = avg;
        }
        fan_normal = corner_space_custom_data_to_normal(&space, r);
    }
    if lnor_len != 0.0 {
        for &l in &fan {
            out.lnors[l.slot()] = fan_normal;
        }
    }
    // `BLI_linklist_prepend`: the space lists its loops last first.
    fan.reverse();
    out.spaces.push(Space {
        space,
        loops: fan,
        single: false,
    });
    bm.vert_mut(v_pivot).flag.insert(ElemFlag::TAG);
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
// (bm_mesh_loops_calc_normals__single_threaded with a space array, no split angle).
/// The loop normals and normal spaces of the BMesh, with its custom normals when the layer
/// exists (fans whose custom normal values differ are set to their average, as Blender does).
/// Uses and leaves the edge, loop and vertex `TAG` flags. Face normals must be current.
pub(crate) fn calc(bm: &mut BMesh) -> Spaces {
    let has_clnors = has_layer(bm);
    let n = bm.loops.capacity();
    let mut out = Spaces {
        spaces: Vec::new(),
        of_slot: vec![usize::MAX; n],
        lnors: vec![[0.0; 3]; n],
        index: vec![usize::MAX; n],
        loops: Vec::with_capacity(bm.loops_num()),
    };
    for f in bm.face_ids() {
        for l in bm.face_loops(f) {
            out.index[l.slot()] = out.loops.len();
            out.loops.push(l);
            bm.lp_mut(l).flag.remove(ElemFlag::TAG);
        }
    }
    for e in bm.edge_ids() {
        edge_tag_from_smooth(bm, e);
    }
    let loops = out.loops.clone();
    for l in loops {
        calc_for_loop(bm, &mut out, has_clnors, l);
    }
    out
}

/// `BM_lnorspace_update` from a fresh state (`bm_lnorspace_ensure_from_free_normals` →
/// `BM_lnorspacearr_store`): ensures the custom normal layer, then computes the spaces.
pub(crate) fn update(bm: &mut BMesh) -> Spaces {
    ensure_layer(bm);
    for f in bm.face_ids() {
        bm.face_normal_update(f);
    }
    calc(bm)
}

/// One entry of `BMLoopNorEditDataArray`.
#[derive(Clone, Debug)]
pub(crate) struct EditData {
    /// The loop.
    pub loop_: LoopId,
    /// Its index (face iteration order) when the array was made.
    pub loop_index: usize,
    /// The custom normal (`nloc`).
    pub nloc: V3,
}

/// `BMLoopNorEditDataArray`: the edit data in vertex order, and by loop index.
#[derive(Clone, Debug, Default)]
pub(crate) struct EditArray {
    /// Entries (`lnor_editdata`), loops of each vertex in vertex order.
    pub data: Vec<EditData>,
    /// Entry of each loop index (`lidx_to_lnor_editdata`).
    pub by_index: Vec<Option<usize>>,
}

/// Which elements choose the loops to edit (`htype_override`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    /// `BM_FACE`: loops of selected faces.
    Faces,
    /// No override: by select mode and selection history (`bm_loop_normal_mark_indiv`).
    Indiv,
}

/// `bm_loop_normal_mark_indiv_do_loop`.
fn mark_loop(
    bm: &BMesh,
    sp: &Spaces,
    marked: &mut [bool],
    count: &mut usize,
    l: Option<LoopId>,
    all_of_vert: bool,
) {
    let Some(l) = l else {
        return;
    };
    let i = sp.index(l);
    if marked[i] {
        return;
    }
    marked[i] = true;
    *count += 1;
    if all_of_vert {
        for lf in bm.vert_loops(bm.lp(l).v) {
            let j = sp.index(lf);
            if !marked[j] {
                marked[j] = true;
                *count += 1;
            }
        }
    } else if let Some(s) = sp.of(l)
        && !s.single
    {
        for &lf in &s.loops {
            let j = sp.index(lf);
            if !marked[j] {
                marked[j] = true;
                *count += 1;
            }
        }
    }
}

fn mark_faces(bm: &BMesh, sp: &Spaces, marked: &mut [bool], count: &mut usize, all: bool) {
    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::SELECT) {
            for l in bm.face_loops(f) {
                mark_loop(bm, sp, marked, count, Some(l), all);
            }
        }
    }
}

fn mark_edges(bm: &BMesh, sp: &Spaces, marked: &mut [bool], count: &mut usize, all: bool) {
    for e in bm.edge_ids() {
        if !bm.edge(e).flag.contains(ElemFlag::SELECT) {
            continue;
        }
        for l in bm.edge_loops(e) {
            mark_loop(bm, sp, marked, count, Some(l), all);
            let ll = bm.lp(l);
            let other = if bm.vert_in_edge(e, bm.lp(ll.next).v) {
                ll.next
            } else {
                ll.prev
            };
            mark_loop(bm, sp, marked, count, Some(other), all);
        }
    }
}

fn mark_verts(bm: &BMesh, sp: &Spaces, marked: &mut [bool], count: &mut usize, all: bool) {
    for v in bm.vert_ids() {
        if bm.vert(v).flag.contains(ElemFlag::SELECT) {
            for l in bm.vert_loops(v) {
                mark_loop(bm, sp, marked, count, Some(l), all);
            }
        }
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
// (bm_loop_normal_mark_indiv).
fn mark_indiv(bm: &BMesh, sp: &Spaces, marked: &mut [bool], all: bool) -> usize {
    let mut count = 0usize;
    let sel_verts = bm.select_mode.contains(SelectMode::VERT);
    let sel_edges = bm.select_mode.contains(SelectMode::EDGE);
    let sel_faces = bm.select_mode.contains(SelectMode::FACE);
    if sel_faces && (sel_edges || sel_verts) {
        let hist = &bm.select_history;
        for (k, &ese) in hist.iter().enumerate().rev() {
            let frap_bmesh::BMElem::Face(f) = ese else {
                continue;
            };
            for &prev in hist[..k].iter().rev() {
                match prev {
                    frap_bmesh::BMElem::Vert(v) => {
                        let l = bm.face_vert_share_loop(f, v);
                        mark_loop(bm, sp, marked, &mut count, l, all);
                    }
                    frap_bmesh::BMElem::Edge(e) => {
                        let [v1, v2] = bm.edge(e).v;
                        let l = bm.face_vert_share_loop(f, v1);
                        mark_loop(bm, sp, marked, &mut count, l, all);
                        let l = bm.face_vert_share_loop(f, v2);
                        mark_loop(bm, sp, marked, &mut count, l, all);
                    }
                    frap_bmesh::BMElem::Face(_) => {}
                }
            }
        }
    }
    if count == 0 {
        if sel_faces {
            mark_faces(bm, sp, marked, &mut count, all);
        }
        if sel_edges {
            mark_edges(bm, sp, marked, &mut count, all);
        }
        if sel_verts {
            mark_verts(bm, sp, marked, &mut count, all);
        }
    }
    count
}

// Ported from blender/source/blender/bmesh/intern/bmesh_mesh_normals.cc
// (BM_loop_normal_editdata_array_init_with_htype, loop_normal_editdata_init).
/// The loops to edit and their current custom normals (decoded in `sp`); ensures the layer.
pub(crate) fn editdata_array(
    bm: &mut BMesh,
    sp: &Spaces,
    all_of_vert: bool,
    mark: Mark,
) -> EditArray {
    ensure_layer(bm);
    let mut marked = vec![false; sp.loops.len()];
    let count = match mark {
        Mark::Faces => {
            let mut c = 0;
            mark_faces(bm, sp, &mut marked, &mut c, all_of_vert);
            c
        }
        Mark::Indiv => mark_indiv(bm, sp, &mut marked, all_of_vert),
    };
    let mut out = EditArray {
        data: Vec::with_capacity(count),
        by_index: vec![None; sp.loops.len()],
    };
    if count == 0 {
        return out;
    }
    for v in bm.vert_ids() {
        for l in bm.vert_loops(v) {
            let i = sp.index(l);
            if marked[i] {
                out.by_index[i] = Some(out.data.len());
                out.data.push(EditData {
                    loop_: l,
                    loop_index: i,
                    nloc: sp.decode(bm, l),
                });
            }
        }
    }
    out
}

/// Faces of the BMesh with `SELECT`, in iteration order.
pub(crate) fn selected_faces(bm: &BMesh) -> Vec<FaceId> {
    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
        .collect()
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (flip_custom_normals_init_data).
/// The custom normals of the loops around selected faces, before their winding changes;
/// `None` without the custom normal layer.
pub(crate) fn flip_init(bm: &mut BMesh) -> Option<EditArray> {
    if !has_layer(bm) {
        return None;
    }
    let sp = update(bm);
    Some(editdata_array(bm, &sp, true, Mark::Faces))
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc (flip_custom_normals).
/// Re-encodes the custom normals saved by [`flip_init`] after the selected faces were
/// reversed: loops of selected faces get the negated normal of the loop that had their index
/// position, the others keep theirs. Returns whether anything was re-encoded.
pub(crate) fn flip_apply(bm: &mut BMesh, old: Option<EditArray>) -> bool {
    let Some(mut old) = old else {
        return false;
    };
    if old.data.is_empty() {
        return false;
    }
    let sp = update(bm);
    let new_full = editdata_array(bm, &sp, true, Mark::Faces);
    for nf in &new_full.data {
        let Some(oi) = old.by_index[nf.loop_index] else {
            continue;
        };
        let v = sp.encode(nf.loop_, old.data[oi].nloc);
        set(bm, nf.loop_, v);
    }
    for f in selected_faces(bm) {
        let l_start = bm.face(f).l_first;
        let mut l = l_start;
        let mut old_index = sp.index(l);
        loop {
            let loop_index = sp.index(l);
            if let (Some(oi), Some(_)) = (
                old.by_index.get(old_index).copied().flatten(),
                new_full.by_index[loop_index],
            ) {
                let n = old.data[oi].nloc;
                let neg = [-n[0], -n[1], -n[2]];
                old.data[oi].nloc = neg;
                let v = sp.encode(l, neg);
                set(bm, l, v);
            }
            old_index += 1;
            l = bm.lp(l).prev;
            if l == l_start {
                break;
            }
        }
    }
    true
}
