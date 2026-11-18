// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.wireframe` (Blender 5.2.4 `bmo_wireframe_exec`, `operators/bmo_wireframe.cc`, and
//! `BM_mesh_wireframe`, `tools/bmesh_wireframe.cc`), implemented by WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.wireframe(bmesh, faces=[], thickness=0.0, offset=0.0, use_replace=False, use_boundary=False, use_even_offset=False, use_crease=False, crease_weight=0.0, use_relative_offset=False, material_offset=0)`.
//!
//! **Create/kill order.** (1) For every vertex of an input face, in vertex order, two copies
//! are created (`neg` then `pos`, offset along the vertex normal by the inner and outer shell
//! distances). (2) For every corner of every input face (face order, then loop order from
//! `l_first`) one inset vertex is created; with `use_boundary`, the first time a boundary
//! corner is met each of its edge's two vertices also gets a boundary vertex. (3) For every
//! corner of every input face, two quads (`l1 l2 neg2 neg1` and `l2 l1 pos1 pos2`, edges
//! created as `BM_face_create_verts` does), plus two boundary quads on boundary edges with
//! `use_boundary`. (4) With `use_replace` the original vertices made entirely into wire are
//! killed (`BM_vert_kill`, which kills their edges and faces), in vertex order. The new
//! faces copy the face they come from (`material_offset` added, clamped) and are the output.
//! `use_crease` writes `crease_weight` into the `crease_edge` attribute (created if missing)
//! of the edges running along the wire. As in Blender, the `material_offset` slot is not
//! read (`bmo_wireframe_exec` passes 0).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use std::sync::Arc;

use frap_mesh::AttrData;
use rustc_hash::FxHashMap;

use super::triangulate::math;
use crate::customdata::CdLayer;
use crate::types::{CreateFlag, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `wireframe` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WireframeIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `thickness`.
    pub thickness: f32,
    /// `offset`.
    pub offset: f32,
    /// `use_replace`.
    pub use_replace: bool,
    /// `use_boundary`.
    pub use_boundary: bool,
    /// `use_even_offset`.
    pub use_even_offset: bool,
    /// `use_crease`.
    pub use_crease: bool,
    /// `crease_weight`.
    pub crease_weight: f32,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
    /// `material_offset`.
    pub material_offset: i32,
}

/// Output slots of `wireframe`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct WireframeOut {
    /// `faces.out`, as elements.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the wire faces, face order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_wireframe_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// `MAXMAT`.
const MAXMAT: i32 = 32767;

fn tagged_f(bm: &BMesh, f: FaceId) -> bool {
    bm.face(f).flag.contains(ElemFlag::TAG)
}

/// `bm_edge_tag_faceloop`: the loop of `e` in a tagged face.
fn edge_tag_faceloop(bm: &BMesh, e: EdgeId) -> Option<LoopId> {
    bm.edge_loops(e)
        .into_iter()
        .find(|&l| tagged_f(bm, bm.lp(l).f))
}

/// `bm_loop_is_radial_boundary`: `l` is the only tagged face around its edge.
fn loop_is_radial_boundary(bm: &BMesh, l_first: LoopId) -> bool {
    let mut l = bm.lp(l_first).radial_next;
    if l == l_first {
        return true;
    }
    loop {
        if tagged_f(bm, bm.lp(l).f) {
            return false;
        }
        l = bm.lp(l).radial_next;
        if l == l_first {
            return true;
        }
    }
}

/// `bm_vert_boundary_tangent`: `(r_no, r_no_face, va_other, vb_other)`.
fn vert_boundary_tangent(
    bm: &BMesh,
    v: VertId,
) -> ([f32; 3], [f32; 3], Option<VertId>, Option<VertId>) {
    let mut e_a = None;
    let mut e_b = None;
    for e in bm.vert_edges(v) {
        if bm.edge(e).flag.contains(ElemFlag::TAG) {
            if e_a.is_none() {
                e_a = Some(e);
            } else {
                e_b = Some(e);
                break;
            }
        }
    }
    let vco = bm.vert(v).co;
    let (no_face, no_edge, tvec_a, va, vb);
    match (e_a, e_b) {
        (Some(e_a), Some(e_b)) => {
            let l_a = edge_tag_faceloop(bm, e_a).expect("invariant: a tagged face loop");
            let l_b = edge_tag_faceloop(bm, e_b).expect("invariant: a tagged face loop");
            no_face = math::normalize_v3(math::add(
                bm.face(bm.lp(l_a).f).no,
                bm.face(bm.lp(l_b).f).no,
            ))
            .0;
            let v_a = bm.edge_other_vert(e_a, v);
            let v_b = bm.edge_other_vert(e_b, v);
            let ta = math::normalize_v3(math::sub(vco, bm.vert(v_a).co)).0;
            let tb = math::normalize_v3(math::sub(bm.vert(v_b).co, vco)).0;
            no_edge = math::add(ta, tb);
            let t1 = math::edge_calc_face_tangent(bm, l_a);
            let t2 = math::edge_calc_face_tangent(bm, l_b);
            tvec_a = math::add(t1, t2);
            va = Some(v_a);
            vb = Some(v_b);
        }
        (Some(e_a), None) => {
            let l_a = edge_tag_faceloop(bm, e_a).expect("invariant: a tagged face loop");
            no_face = bm.face(bm.lp(l_a).f).no;
            let v_a = bm.edge_other_vert(e_a, v);
            no_edge = math::sub(vco, bm.vert(v_a).co);
            tvec_a = math::edge_calc_face_tangent(bm, l_a);
            va = None;
            vb = None;
        }
        _ => {
            // No tagged edge: cannot happen for a boundary corner.
            return ([0.0; 3], [0.0; 3], None, None);
        }
    }
    let mut r_no = math::normalize_v3(math::cross(no_edge, no_face)).0;
    if math::dot(r_no, tvec_a) > 0.0 {
        r_no = math::negate(r_no);
    }
    (r_no, no_face, va, vb)
}

fn set_crease(bm: &mut BMesh, layer: usize, a: VertId, b: VertId, w: f32) {
    let Some(e) = bm.edge_exists(a, b) else {
        return;
    };
    let slots = bm.edges.capacity();
    bm.edata.ensure_slots(slots);
    if let AttrData::Float(col) = &mut bm.edata.layers[layer].data {
        Arc::make_mut(col)[e.slot()] = w;
    }
}

fn create_quad(
    bm: &mut BMesh,
    v: [VertId; 4],
    f_src: FaceId,
    mat_offset: i32,
    mat_max: i32,
) -> (FaceId, LoopId) {
    let f_new = bm
        .face_create_verts(&v, Some(f_src), CreateFlag::empty(), true)
        .expect("invariant: a wire quad over distinct vertices");
    if mat_offset != 0 {
        let m = (bm.face(f_new).mat_nr as i32 + mat_offset).clamp(0, mat_max);
        bm.face_mut(f_new).mat_nr = m as i16;
    }
    bm.face_mut(f_new).flag.insert(ElemFlag::TAG);
    (f_new, bm.face(f_new).l_first)
}

/// Copies loop data onto the four corners of a new quad (`l_new`, `prev`, `next`,
/// `next.next` from `a`, `a`, `b`, `b`).
fn copy_quad_loops(bm: &mut BMesh, l_new: LoopId, a: LoopId, b: LoopId) {
    let prev = bm.lp(l_new).prev;
    let next = bm.lp(l_new).next;
    let next2 = bm.lp(next).next;
    bm.loop_attrs_copy(a, l_new);
    bm.loop_attrs_copy(a, prev);
    bm.loop_attrs_copy(b, next);
    bm.loop_attrs_copy(b, next2);
}

// Ported from blender/source/blender/bmesh/tools/bmesh_wireframe.cc (BM_mesh_wireframe, without
// vertex groups).
/// `BM_mesh_wireframe(bm, offset, offset_fac, 0, use_replace, use_boundary, use_even_offset,
/// use_relative_offset, use_crease, crease_weight, -1, false, mat_offset, mat_max, use_tag)`.
#[allow(clippy::too_many_arguments)]
pub fn mesh_wireframe(
    bm: &mut BMesh,
    offset: f32,
    offset_fac: f32,
    use_replace: bool,
    use_boundary: bool,
    use_even_offset: bool,
    use_relative_offset: bool,
    use_crease: bool,
    crease_weight: f32,
    mat_offset: i32,
    mat_max: i32,
    use_tag: bool,
) {
    let ofs_orig = -(((-offset_fac + 1.0) * 0.5) * offset);
    let ofs_new = offset + ofs_orig;
    let ofs_mid = (ofs_orig + ofs_new) / 2.0;
    let inset = offset / 2.0;

    let mut crease_layer = None;
    if use_crease {
        let slots = bm.edges.capacity();
        bm.edata.ensure_slots(slots);
        crease_layer = Some(match bm.edata.find("crease_edge") {
            Some(i) if matches!(bm.edata.layers[i].data, AttrData::Float(_)) => i,
            _ => bm.edata.add_layer(CdLayer {
                name: "crease_edge".into(),
                data: AttrData::Float(Arc::new(vec![0.0; slots])),
            }),
        });
    }

    let verts_src = bm.vert_ids();
    let totvert_orig = verts_src.len();
    for (i, &v) in verts_src.iter().enumerate() {
        bm.index_set(BMElem::Vert(v), i as i32);
        bm.vert_mut(v).flag.remove(ElemFlag::TAG);
    }
    bm.index.dirty.remove(crate::ElemTypes::VERT);

    // Set up tags: every face and vertex to duplicate is tagged.
    let mut verts_loop_tot = 0usize;
    for (i, f) in bm.face_ids().into_iter().enumerate() {
        bm.index_set(BMElem::Face(f), i as i32);
        if use_tag {
            if !tagged_f(bm, f) {
                continue;
            }
        } else {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
        }
        verts_loop_tot += bm.face(f).len as usize;
        for l in bm.face_loops(f) {
            let (v, e) = (bm.lp(l).v, bm.lp(l).e);
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
            let b = loop_is_radial_boundary(bm, l);
            bm.edge_mut(e).flag.set(ElemFlag::TAG, b);
        }
    }
    bm.index.dirty.remove(crate::ElemTypes::FACE);

    let mut verts_neg: Vec<Option<VertId>> = vec![None; totvert_orig];
    let mut verts_pos: Vec<Option<VertId>> = vec![None; totvert_orig];
    let mut verts_boundary: Vec<Option<VertId>> = vec![None; totvert_orig];
    let mut verts_relfac: Vec<f32> = vec![1.0; totvert_orig];

    // Duplicate tagged vertices.
    for (i, &v_src) in verts_src.iter().enumerate() {
        if bm.vert(v_src).flag.contains(ElemFlag::TAG) {
            frap_math::budget::check();
            let mut fac = 1.0f32;
            if use_relative_offset {
                verts_relfac[i] = math::vert_calc_median_tagged_edge_length(bm, v_src);
                fac *= verts_relfac[i];
            }
            let neg = bm.vert_create([0.0; 3], Some(v_src), CreateFlag::empty());
            let pos = bm.vert_create([0.0; 3], Some(v_src), CreateFlag::empty());
            let (co, no) = (bm.vert(v_src).co, bm.vert(v_src).no);
            if offset == 0.0 {
                bm.vert_mut(neg).co = math::madd(co, no, ofs_orig * fac);
                bm.vert_mut(pos).co = math::madd(co, no, ofs_new * fac);
            } else {
                let tvec = math::madd(co, no, ofs_mid * fac);
                bm.vert_mut(neg).co = math::madd(tvec, no, (ofs_orig - ofs_mid) * fac);
                bm.vert_mut(pos).co = math::madd(tvec, no, (ofs_new - ofs_mid) * fac);
            }
            verts_neg[i] = Some(neg);
            verts_pos[i] = Some(pos);
        }
        // Conflicts with `BM_vert_calc_median_tagged_edge_length`.
        if !use_relative_offset {
            bm.vert_mut(v_src).flag.remove(ElemFlag::TAG);
        }
    }
    if use_relative_offset {
        for v in bm.vert_ids() {
            bm.vert_mut(v).flag.remove(ElemFlag::TAG);
        }
    }

    let vidx = |bm: &BMesh, v: VertId| bm.index_get(BMElem::Vert(v)) as usize;
    let mut loop_index: FxHashMap<LoopId, usize> = FxHashMap::default();
    let mut verts_loop: Vec<VertId> = Vec::with_capacity(verts_loop_tot);

    // `BM_ITER_MESH` over faces: faces created in this loop would be visited too, but this
    // loop creates none.
    for f_src in bm.face_ids() {
        if use_tag && !tagged_f(bm, f_src) {
            continue;
        }
        for l in bm.face_loops(f_src) {
            frap_math::budget::check();
            loop_index.insert(l, verts_loop.len());
            let tangent = math::loop_calc_face_tangent(bm, l);
            let lv = bm.lp(l).v;
            let fac = verts_relfac[vidx(bm, lv)];
            let mut fac_shell = fac;
            if use_even_offset {
                fac_shell *= math::shell_angle_to_dist(
                    (std::f32::consts::PI - math::loop_calc_face_angle(bm, l)) * 0.5,
                );
            }
            let (lco, lno) = (bm.vert(lv).co, bm.vert(lv).no);
            let mut tvec = math::madd(lco, tangent, inset * fac_shell);
            if offset != 0.0 {
                tvec = math::madd(tvec, lno, ofs_mid * fac);
            }
            let vl = bm.vert_create(tvec, Some(lv), CreateFlag::empty());
            verts_loop.push(vl);

            if use_boundary && bm.edge(bm.lp(l).e).flag.contains(ElemFlag::TAG) {
                let v_pair = [lv, bm.lp(bm.lp(l).next).v];
                for v_boundary in v_pair {
                    if bm.vert(v_boundary).flag.contains(ElemFlag::TAG) {
                        continue;
                    }
                    let vbi = vidx(bm, v_boundary);
                    bm.vert_mut(v_boundary).flag.insert(ElemFlag::TAG);
                    let (tvec_b, no_face, va_other, vb_other) =
                        vert_boundary_tangent(bm, v_boundary);
                    let fac = verts_relfac[vbi];
                    let mut fac_shell = fac;
                    if use_even_offset && let (Some(va), Some(vb)) = (va_other, vb_other) {
                        fac_shell *= math::shell_angle_to_dist(
                            (std::f32::consts::PI
                                - math::angle_on_axis_v3v3v3_v3(
                                    bm.vert(va).co,
                                    bm.vert(v_boundary).co,
                                    bm.vert(vb).co,
                                    no_face,
                                ))
                                * 0.5,
                        );
                    }
                    let (bco, bno) = (bm.vert(v_boundary).co, bm.vert(v_boundary).no);
                    let mut t = math::madd(bco, tvec_b, inset * fac_shell);
                    if offset != 0.0 {
                        t = math::madd(t, bno, ofs_mid * fac);
                    }
                    verts_boundary[vbi] =
                        Some(bm.vert_create(t, Some(v_boundary), CreateFlag::empty()));
                }
            }
        }
    }
    bm.index.dirty |= crate::ElemTypes::LOOP;

    // Create the wire faces. Faces created here are visited by Blender's iterator but skipped
    // (index -1); walking the slots as they grow reproduces that.
    let mut s = 0u32;
    while (s as usize) < bm.faces.capacity() {
        let f_src = FaceId(s);
        s += 1;
        if !bm.faces.contains(f_src.0) {
            continue;
        }
        if bm.index_get(BMElem::Face(f_src)) == -1 {
            continue;
        }
        if use_tag && !tagged_f(bm, f_src) {
            continue;
        }
        bm.face_mut(f_src).flag.remove(ElemFlag::TAG);
        for l in bm.face_loops(f_src) {
            frap_math::budget::check();
            let l_next = bm.lp(l).next;
            let v_l1 = verts_loop[loop_index[&l]];
            let v_l2 = verts_loop[loop_index[&l_next]];
            let i_1 = vidx(bm, bm.lp(l).v);
            let i_2 = vidx(bm, bm.lp(l_next).v);
            let (Some(v_neg1), Some(v_neg2), Some(v_pos1), Some(v_pos2)) = (
                verts_neg[i_1],
                verts_neg[i_2],
                verts_pos[i_1],
                verts_pos[i_2],
            ) else {
                continue;
            };

            let (_, l_new) =
                create_quad(bm, [v_l1, v_l2, v_neg2, v_neg1], f_src, mat_offset, mat_max);
            copy_quad_loops(bm, l_new, l, l_next);
            let (_, l_new) =
                create_quad(bm, [v_l2, v_l1, v_pos1, v_pos2], f_src, mat_offset, mat_max);
            copy_quad_loops(bm, l_new, l_next, l);

            if use_boundary
                && bm.edge(bm.lp(l).e).flag.contains(ElemFlag::TAG)
                && let (Some(v_b1), Some(v_b2)) = (verts_boundary[i_1], verts_boundary[i_2])
            {
                let (_, l_new) =
                    create_quad(bm, [v_b2, v_b1, v_neg1, v_neg2], f_src, mat_offset, mat_max);
                copy_quad_loops(bm, l_new, l_next, l);
                let (_, l_new) =
                    create_quad(bm, [v_b1, v_b2, v_pos2, v_pos1], f_src, mat_offset, mat_max);
                copy_quad_loops(bm, l_new, l, l_next);
                if let Some(cl) = crease_layer {
                    set_crease(bm, cl, v_pos1, v_b1, crease_weight);
                    set_crease(bm, cl, v_pos2, v_b2, crease_weight);
                    set_crease(bm, cl, v_neg1, v_b1, crease_weight);
                    set_crease(bm, cl, v_neg2, v_b2, crease_weight);
                }
            }
            if let Some(cl) = crease_layer {
                set_crease(bm, cl, v_pos1, v_l1, crease_weight);
                set_crease(bm, cl, v_pos2, v_l2, crease_weight);
                set_crease(bm, cl, v_neg1, v_l1, crease_weight);
                set_crease(bm, cl, v_neg2, v_l2, crease_weight);
            }
        }
    }

    if use_replace {
        if use_tag {
            // Keep every vertex used by a face that wasn't entirely made into wire.
            for f_src in bm.face_ids() {
                if bm.index_get(BMElem::Face(f_src)) == -1 {
                    continue;
                }
                let verts = bm.face_verts(f_src);
                let mut mix_flag = 0;
                for &v in &verts {
                    mix_flag |= if verts_neg[vidx(bm, v)].is_some() {
                        1
                    } else {
                        2
                    };
                    if mix_flag == 3 {
                        break;
                    }
                }
                if mix_flag == 3 {
                    for &v in &verts {
                        verts_pos[vidx(bm, v)] = None;
                    }
                }
            }
            // Now remove the vertices made into wire by all their faces.
            for (i, &v_src) in verts_src.iter().enumerate() {
                if verts_pos[i].is_some() && bm.verts.contains(v_src.0) {
                    bm.vert_kill(v_src);
                }
            }
        } else {
            for &v_src in &verts_src {
                if bm.verts.contains(v_src.0) {
                    bm.vert_kill(v_src);
                }
            }
        }
    }
}

/// Runs `wireframe` on `bm`.
pub fn wireframe(bm: &mut BMesh, i: &WireframeIn) -> Result<WireframeOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "wireframe: dead face {dead:?} in faces"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| {
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.remove(ElemFlag::TAG);
        }
        for f in bm.face_ids() {
            bm.face_mut(f).flag.remove(ElemFlag::TAG);
        }
        for &f in &i.faces {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
        }
        mesh_wireframe(
            bm,
            i.thickness,
            i.offset,
            i.use_replace,
            i.use_boundary,
            i.use_even_offset,
            i.use_relative_offset,
            i.use_crease,
            i.crease_weight,
            // `bmo_wireframe_exec` passes 0: the `material_offset` slot is not read.
            0,
            MAXMAT,
            true,
        );
        let mut out = WireframeOut::default();
        for f in bm.face_ids() {
            if tagged_f(bm, f) {
                out.faces.push(f);
                out.geom.push(BMElem::Face(f));
            }
        }
        out
    }))
}
