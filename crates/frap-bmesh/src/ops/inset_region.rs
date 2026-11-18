// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.inset_region` (Blender 5.2.4 `bmo_inset_region_exec`, `operators/bmo_inset.cc`),
//! implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.inset_region(bmesh, faces=[], faces_exclude=[], use_boundary=False, use_even_offset=False, use_interpolate=False, use_relative_offset=False, use_edge_rail=False, thickness=0.0, depth=0.0, use_outset=False)`.
//!
//! **Create/kill order** (verified on the factory cube [MESH §4.3, App. B.3]):
//! 1. Every edge between a tagged (input) and an untagged face, and with `use_boundary` every
//!    boundary edge of a tagged face, is "split", in edge iteration order: the tagged face's
//!    loop moves to a new copy of the edge (`bmesh_kernel_edge_separate`); for a boundary edge
//!    the new copy is created as the *outer* edge instead (`BM_edge_create`).
//! 2. The vertices of the split edges, edge by edge (`v1` then `v2`), are split into one
//!    vertex per face fan (`bmesh_kernel_vert_separate`: the original stays with the first fan,
//!    new vertices for the others); the inner ones move inward by `thickness`; with three or
//!    more fans the untagged fans are glued back together (`BM_vert_splice`).
//! 3. One quad (or triangle when a corner did not split) per split edge, in the same order,
//!    between the inner and outer edge (`BM_face_create_verts`, missing edges first), taking
//!    the loop data of the faces on both sides.
//! 4. With `depth`, the vertices of the input faces move along their (region) normals.
//!
//! The input faces keep their slots and become the inner faces; `faces.out` are the new ring
//! faces. Nothing is killed (glued vertices excepted). The original vertices keep their slots
//! and move with the inner faces; the new vertices take the outer positions.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH`.

use rustc_hash::FxHashMap;

use crate::ops::inset_individual::{
    CdBlocks, ELE_NEW, InterpFace, col_has_math, copy_col_value, edge_calc_face_tangent,
    edge_calc_length, shell_v3v3_mid_normalized_to_dist, shell_v3v3_normalized_to_dist,
    vert_calc_shell_factor,
};
use crate::query::geom::{self, V3};
use crate::types::{CreateFlag, ElemTypes, OpTypeFlag};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `inset_region` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsetRegionIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `faces_exclude`.
    pub faces_exclude: Vec<FaceId>,
    /// `use_boundary`.
    pub use_boundary: bool,
    /// `use_even_offset`.
    pub use_even_offset: bool,
    /// `use_interpolate`.
    pub use_interpolate: bool,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
    /// `use_edge_rail`.
    pub use_edge_rail: bool,
    /// `thickness`.
    pub thickness: f32,
    /// `depth`.
    pub depth: f32,
    /// `use_outset`.
    pub use_outset: bool,
}

/// Output slots of `inset_region`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct InsetRegionOut {
    /// The new faces as elements (same content as `faces`).
    pub geom: Vec<BMElem>,
    /// `faces.out`: the new ring faces, in iteration order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_inset_region_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC.union(OpTypeFlag::SELECT_FLUSH);

/// `SplitEdgeInfo`.
struct SplitEdgeInfo {
    no: V3,
    length: f32,
    e_old: EdgeId,
    e_new: EdgeId,
    l: LoopId,
}

fn tagged(bm: &BMesh, e: impl Into<BMElem>) -> bool {
    bm.elem_flag(e.into()).contains(ElemFlag::TAG)
}
fn set_tag(bm: &mut BMesh, e: impl Into<BMElem>, on: bool) {
    bm.elem_flag_mut(e.into()).set(ElemFlag::TAG, on);
}

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bm_edge_is_mixed_face_tag).
/// The tagged loop when the radial cycle of `l` has exactly one tagged face and at least one
/// untagged face.
fn edge_is_mixed_face_tag(bm: &BMesh, l: Option<LoopId>) -> Option<LoopId> {
    let l = l?;
    let mut tot_tag = 0;
    let mut tot_untag = 0;
    let mut l_tag = None;
    let mut it = l;
    loop {
        if tagged(bm, bm.lp(it).f) {
            // More than one tagged face: bail out early.
            if tot_tag == 1 {
                return None;
            }
            l_tag = Some(it);
            tot_tag += 1;
        } else {
            tot_untag += 1;
        }
        it = bm.lp(it).radial_next;
        if it == l {
            break;
        }
    }
    (tot_tag == 1 && tot_untag >= 1).then_some(l_tag).flatten()
}

/// `BM_elem_index_get` of an edge (the scratch index into the edge info array).
fn edge_index(bm: &BMesh, e: EdgeId) -> i32 {
    bm.index_get(BMElem::Edge(e))
}

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bm_edge_info_average_length).
fn edge_info_average_length(bm: &BMesh, v: VertId, info: &[SplitEdgeInfo]) -> f32 {
    let mut len = 0.0f32;
    let mut tot = 0;
    for e in bm.vert_edges(v) {
        let i = edge_index(bm, e);
        if i != -1 {
            len += info[i as usize].length;
            tot += 1;
        }
    }
    if tot != 0 { len / tot as f32 } else { -1.0 }
}

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc
// (bm_edge_info_average_length_fallback).
/// Fills the average lengths of interior vertices from the topological distance to the inset
/// boundary (computed once, lazily).
fn edge_info_average_length_fallback(
    bm: &mut BMesh,
    v_lookup: VertId,
    info: &[SplitEdgeInfo],
    vert_lengths: &mut Option<Vec<(f32, i32)>>,
) -> f32 {
    if vert_lengths.is_none() {
        bm.elem_index_ensure(ElemTypes::VERT);
        let totvert = bm.verts_num();
        let mut lengths: Vec<(f32, i32)> = vec![(0.0, 0); totvert];
        let mut stack: Vec<VertId> = Vec::with_capacity(totvert);
        let vi = |bm: &BMesh, v: VertId| bm.index_of(BMElem::Vert(v));
        for e in bm.edge_ids() {
            if edge_index(bm, e) != -1 {
                for v in bm.edge(e).v {
                    if tagged(bm, v) {
                        let i = vi(bm, v);
                        if lengths[i].1 == 0 {
                            stack.push(v);
                            lengths[i] = (edge_info_average_length(bm, v, info), 1);
                        }
                    }
                }
            }
        }
        // While there are vertices without their accumulated lengths divided by the count.
        while !stack.is_empty() {
            let mut stack_index = stack.len();
            while stack_index > 0 {
                stack_index -= 1;
                let v = stack[stack_index];
                // `STACK_REMOVE`: the last element fills the hole.
                stack.swap_remove(stack_index);
                let i = vi(bm, v);
                lengths[i].0 /= lengths[i].1 as f32;
                lengths[i].1 = -1;
                for e in bm.vert_edges(v) {
                    if !tagged(bm, e) {
                        continue;
                    }
                    let v_other = bm.edge_other_vert(e, v);
                    if !tagged(bm, v_other) {
                        continue;
                    }
                    let j = vi(bm, v_other);
                    if lengths[j].1 >= 0 {
                        if lengths[j].1 == 0 {
                            stack.push(v_other);
                        }
                        lengths[j].1 += 1;
                        lengths[j].0 += lengths[i].0;
                    }
                }
            }
        }
        *vert_lengths = Some(lengths);
    }
    let i = bm.index_of(BMElem::Vert(v_lookup));
    vert_lengths.as_ref().expect("computed above")[i].0
}

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bm_loop_customdata_merge).
/// Merges loop data (UVs, colours) that interpolating across the inset would make diverge at
/// the vertex shared by `l_a_inner` and `l_b_inner` (#41445).
fn loop_customdata_merge(
    bm: &mut BMesh,
    l_a_outer: LoopId,
    l_b_outer: LoopId,
    l_a_inner: LoopId,
    l_b_inner: LoopId,
) {
    let is_flip = bm.lp(l_a_inner).next == l_a_outer;
    let (e_a, e_b) = if is_flip {
        (bm.lp(bm.lp(l_a_inner).prev).e, bm.lp(l_b_inner).e)
    } else {
        (bm.lp(l_a_inner).e, bm.lp(bm.lp(l_b_inner).prev).e)
    };
    // The edges must be manifold for the "other" loop to make sense.
    if !bm.edge_is_manifold(e_a) || !bm.edge_is_manifold(e_b) {
        return;
    }
    let l_a_inner_inset = bm.edge_other_loop(e_a, l_a_inner);
    let l_b_inner_inset = bm.edge_other_loop(e_b, l_b_inner);
    // No chance of diverging.
    if bm.lp(l_a_inner_inset).f == bm.lp(l_b_inner_inset).f {
        return;
    }
    let share_edge = if is_flip {
        bm.lp(l_b_inner_inset).e == bm.lp(bm.lp(l_a_inner_inset).prev).e
    } else {
        bm.lp(l_a_inner_inset).e == bm.lp(bm.lp(l_b_inner_inset).prev).e
    };
    let fan: Vec<LoopId> = if share_edge {
        Vec::new()
    } else {
        let v = bm.lp(l_a_inner_inset).v;
        bm.vert_loops(v)
            .into_iter()
            .filter(|&l| {
                tagged(bm, bm.lp(l).f)
                    && ![l_a_inner, l_b_inner, l_a_inner_inset, l_b_inner_inset].contains(&l)
            })
            .collect()
    };
    for layer in 0..bm.ldata.layers.len() {
        let d = &bm.ldata.layers[layer].data;
        // Only layers with math, and only values that start merged.
        if !col_has_math(d) || !data_equals(d, l_a_outer.slot(), l_b_outer.slot()) {
            continue;
        }
        let (a_in, b_in, a_inset, b_inset) = (
            l_a_inner.slot(),
            l_b_inner.slot(),
            l_a_inner_inset.slot(),
            l_b_inner_inset.slot(),
        );
        // `CustomData_data_mix_value(a_inset → b_inset, MIX, 0.5)` then
        // `CustomData_data_copy_value(a_inset → b_inset)`: the copy wins, `b_inset = a_inset`
        // (Blender's comment intends `a = b = (a + b) / 2`).
        let col = &mut bm.ldata.layers[layer].data;
        copy_within(col, a_inset, b_inset);
        // `data_src` is `l_a_inner_inset`'s value.
        for &l in &fan {
            let d = l.slot();
            if data_equals(col, d, b_in) || data_equals(col, d, a_in) {
                copy_within(col, a_inset, d);
            }
        }
        copy_within(col, a_inset, b_in);
        copy_within(col, a_inset, a_in);
    }
}

/// `col[d] = col[s]`.
fn copy_within(col: &mut frap_mesh::AttrData, s: usize, d: usize) {
    if s == d {
        return;
    }
    let src = col.clone();
    copy_col_value(&src, s, col, d);
}

/// `CustomData_data_equals` for the math types (UVs within `1e-5` squared distance, colours
/// within `1e-3` squared distance; byte colours compared as floats).
fn data_equals(col: &frap_mesh::AttrData, a: usize, b: usize) -> bool {
    use frap_mesh::AttrData as D;
    match col {
        D::Float2(c) => {
            let (x, y) = (c[a], c[b]);
            let d = [x[0] - y[0], x[1] - y[1]];
            d[0] * d[0] + d[1] * d[1] < 0.00001
        }
        D::FloatColor(c) => {
            let mut tot = 0.0f32;
            for i in 0..4 {
                let d = c[a][i] - c[b][i];
                tot += d * d;
            }
            tot < 0.001
        }
        D::ByteColor(c) => {
            let d: Vec<f32> = (0..4).map(|i| c[a][i] as f32 - c[b][i] as f32).collect();
            d[0] * d[0] + d[1] * d[1] + d[2] * d[2] + d[3] * d[3] < 0.001
        }
        _ => false,
    }
}

/// Runs `inset_region` on `bm`.
pub fn inset_region(bm: &mut BMesh, i: &InsetRegionIn) -> Result<InsetRegionOut, BmError> {
    for f in i.faces.iter().chain(&i.faces_exclude) {
        if !bm.faces.contains(f.0) {
            return Err(BmError::Invalid(format!(
                "inset_region: dead face {f:?} in faces"
            )));
        }
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i)))
}

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bmo_inset_region_exec).
fn exec(bm: &mut BMesh, i: &InsetRegionIn) -> InsetRegionOut {
    let use_outset = i.use_outset;
    let use_boundary = i.use_boundary && !use_outset;
    let use_even_offset = i.use_even_offset;
    let use_even_boundary = use_even_offset; // Could be its own option.
    let use_relative_offset = i.use_relative_offset;
    let use_edge_rail = i.use_edge_rail;
    let use_interpolate = i.use_interpolate;
    let thickness = i.thickness;
    let depth = i.depth;
    let has_math_ldata =
        use_interpolate && bm.ldata.layers.iter().any(|l| col_has_math(&l.data));

    // Interpolation: the faces stored (`iface_array`, indexed by face), and their blocks.
    let mut blocks = use_interpolate.then(|| CdBlocks::save(bm));
    let mut ifaces: FxHashMap<FaceId, InterpFace> = FxHashMap::default();

    // Original vertex positions (`use_edge_rail`).
    let use_vert_coords_orig = use_edge_rail;
    let mut vert_coords: FxHashMap<VertId, V3> = FxHashMap::default();

    if !use_outset {
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
        for &f in &i.faces {
            set_tag(bm, f, true);
        }
    } else {
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.insert(ElemFlag::TAG);
        }
        for &f in i.faces.iter().chain(&i.faces_exclude) {
            set_tag(bm, f, false);
        }
    }

    // Count every inset edge to split; fill the edge indices and initialise the tags.
    let mut edge_info_len = 0i32;
    for e in bm.edge_ids() {
        let l = bm.edge(e).l;
        let boundary_tagged =
            use_boundary && bm.edge_is_boundary(e) && tagged(bm, bm.lp(l.expect("boundary")).f);
        let [v1, v2] = bm.edge(e).v;
        if boundary_tagged || edge_is_mixed_face_tag(bm, l).is_some() {
            set_tag(bm, v1, true);
            set_tag(bm, v2, true);
            set_tag(bm, e, true);
            bm.index_set(BMElem::Edge(e), edge_info_len);
            edge_info_len += 1;
        } else {
            set_tag(bm, v1, false);
            set_tag(bm, v2, false);
            set_tag(bm, e, false);
            bm.index_set(BMElem::Edge(e), -1);
        }
    }
    bm.index.dirty |= ElemTypes::EDGE;

    let mut edge_info: Vec<SplitEdgeInfo> = Vec::with_capacity(edge_info_len as usize);
    for e in bm.edge_ids() {
        if edge_index(bm, e) != -1 {
            edge_info.push(SplitEdgeInfo {
                no: [0.0; 3],
                length: edge_calc_length(bm, e),
                e_old: e,
                e_new: e,
                l: LoopId(u32::MAX),
            });
        }
    }

    for idx in 0..edge_info.len() {
        frap_math::budget::check();
        let e_old = edge_info[idx].e_old;
        let l = match edge_is_mixed_face_tag(bm, bm.edge(e_old).l) {
            Some(l) => l,
            // Must be a boundary.
            None => bm.edge(e_old).l.expect("inset edge has a face"),
        };
        // Run the separation.
        if !bm.edge_is_boundary(e_old) {
            bm.edge_separate(e_old, l, false);
        }
        let e_new = bm.lp(l).e;
        let no = edge_calc_face_tangent(bm, l);
        let mut e_old_now = e_old;
        if e_new == e_old {
            // Happens on boundary edges: this double edge *must* have its vertices replaced
            // later on.
            let [a, b] = bm.edge(e_new).v;
            e_old_now = bm.edge_create(a, b, Some(e_new), CreateFlag::empty());
        }
        // Store the index back to the original in the edge info.
        bm.index_set(BMElem::Edge(e_new), idx as i32);
        set_tag(bm, e_new, true);
        // Important to tag again here.
        let [a, b] = bm.edge(e_new).v;
        set_tag(bm, a, true);
        set_tag(bm, b, true);
        edge_info[idx] = SplitEdgeInfo {
            no,
            length: edge_info[idx].length,
            e_old: e_old_now,
            e_new,
            l,
        };
        // Initialise the interpolation of the faces around the edge's vertices (faces on the
        // other side of the inset too: redundant but harmless).
        if use_interpolate {
            for v in [a, b] {
                for f in bm.vert_faces(v) {
                    if let std::collections::hash_map::Entry::Vacant(slot) = ifaces.entry(f) {
                        slot.insert(InterpFace::store(bm, f, |v| v.0));
                    }
                }
            }
        }
    }

    // Split the vertices and position them. Loop over the edges, then their vertices (the
    // vertices are split off while iterating).
    for idx in 0..edge_info.len() {
        for j in 0..2 {
            let v = bm.edge(edge_info[idx].e_new).v[j];
            // Only split tagged vertices (used by separated edges).
            if !tagged(bm, v) {
                continue;
            }
            frap_math::budget::check();
            // Disable touching twice (this *will* happen if the flags are not disabled).
            set_tag(bm, v, false);
            let vout = bm.vert_separate(v, &[], false);
            // In some cases the edge does not split off.
            if vout.len() == 1 {
                if use_vert_coords_orig {
                    let co = bm.vert(vout[0]).co;
                    vert_coords.entry(vout[0]).or_insert(co);
                }
                continue;
            }
            let mut v_glue: Option<VertId> = None;
            for &v_split in &vout {
                if use_vert_coords_orig {
                    let co = bm.vert(v_split).co;
                    vert_coords.entry(v_split).or_insert(co);
                }
                // Find the adjacent inset edges.
                let mut vert_edge_tag_tot = 0;
                let mut vecpair = [0usize; 2];
                for e in bm.vert_edges(v_split) {
                    if tagged(bm, e)
                        && let Some(el) = bm.edge(e).l
                        && tagged(bm, bm.lp(el).f)
                    {
                        if vert_edge_tag_tot < 2 {
                            let ix = edge_index(bm, e);
                            debug_assert!(ix != -1);
                            vecpair[vert_edge_tag_tot] = ix.max(0) as usize;
                        }
                        vert_edge_tag_tot += 1;
                    }
                }

                if vert_edge_tag_tot != 0 {
                    let mut tvec: V3;
                    if vert_edge_tag_tot >= 2 {
                        // Two edge users, the common case.
                        let (ia, ib) = (vecpair[0], vecpair[1]);
                        let f_a = bm.lp(edge_info[ia].l).f;
                        let f_b = bm.lp(edge_info[ib].l).f;
                        // Set when not exactly between the two edge normals.
                        let mut is_mid = true;
                        tvec = geom::add(edge_info[ia].no, edge_info[ib].no);
                        if use_edge_rail && f_a != f_b {
                            let l_other_a = loop_other_vert_loop(bm, edge_info[ia].l, v_split);
                            let l_other_b = loop_other_vert_loop(bm, edge_info[ib].l, v_split);
                            if bm.lp(l_other_a).v == bm.lp(l_other_b).v {
                                // Both edges' faces are adjacent: having both vertices is
                                // enough. `l_other_a->v` may itself be inset (feedback), so
                                // use its original position.
                                let ov = bm.lp(l_other_a).v;
                                let co_other = if use_vert_coords_orig {
                                    vert_coords.get(&ov).copied().unwrap_or(bm.vert(ov).co)
                                } else {
                                    bm.vert(ov).co
                                };
                                tvec = geom::sub(co_other, bm.vert(v_split).co);
                                is_mid = false;
                            }
                        }
                        tvec = geom::normalize_v3(tvec).0;
                        // Scale by the edge angle.
                        if use_even_offset {
                            let (na, nb) = (edge_info[ia].no, edge_info[ib].no);
                            let s = if is_mid {
                                shell_v3v3_mid_normalized_to_dist(na, nb)
                            } else {
                                // Use the largest angle.
                                let pick = if frap_geom::normals::len_squared_v3v3(tvec, na)
                                    > frap_geom::normals::len_squared_v3v3(tvec, nb)
                                {
                                    na
                                } else {
                                    nb
                                };
                                shell_v3v3_normalized_to_dist(tvec, pick)
                            };
                            tvec = geom::scale(tvec, s);
                        }
                        // Scale relative to the edge lengths.
                        if use_relative_offset {
                            tvec = geom::scale(
                                tvec,
                                (edge_info[ia].length + edge_info[ib].length) / 2.0,
                            );
                        }
                    } else {
                        // One edge user: a boundary vertex, not so common.
                        let e_no_a = edge_info[vecpair[0]].no;
                        if use_even_boundary {
                            // Align the inset with the boundary edge instead of the normal of
                            // the interior edge (which would give an unsightly bump).
                            let l0 = bm
                                .edge(bm.vert(v_split).e.expect("split vertex has edges"))
                                .l
                                .expect("inset edge has a face");
                            let ll = bm.lp(l0);
                            let l = if bm.lp(ll.prev).v == v_split {
                                ll.prev
                            } else if bm.lp(ll.next).v == v_split {
                                ll.next
                            } else {
                                l0
                            };
                            // Find the edge which is *not* being split here.
                            let le = bm.lp(l).e;
                            let lpe = bm.lp(bm.lp(l).prev).e;
                            let e_other = if !tagged(bm, le) {
                                le
                            } else if !tagged(bm, lpe) {
                                lpe
                            } else {
                                debug_assert!(false, "inset_region: no untagged edge");
                                le
                            };
                            let v_other = bm.edge_other_vert(e_other, v_split);
                            tvec = geom::normalize_v3(geom::sub(
                                bm.vert(v_other).co,
                                bm.vert(v_split).co,
                            ))
                            .0;
                            if use_even_offset {
                                tvec = geom::scale(tvec, shell_v3v3_normalized_to_dist(e_no_a, tvec));
                            }
                        } else {
                            tvec = e_no_a;
                        }
                        // Scale relative to the edge length.
                        if use_relative_offset {
                            tvec = geom::scale(tvec, edge_info[vecpair[0]].length);
                        }
                    }
                    // Apply the offset.
                    let mut co = bm.vert(v_split).co;
                    geom::madd_v3_v3fl(&mut co, tvec, thickness);
                    bm.vert_mut(v_split).co = co;
                }

                // This saves an expensive glue check for the common cases.
                if vout.len() > 2 {
                    // Last step: glue the vertex unless it has a tagged face.
                    let ok = !bm.vert_faces(v_split).into_iter().any(|f| tagged(bm, f));
                    if ok {
                        match v_glue {
                            None => v_glue = Some(v_split),
                            Some(g) => {
                                if bm.vert_splice(g, v_split) && use_vert_coords_orig {
                                    vert_coords.remove(&v_split);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Re-interpolate the stored faces, in face order (vertex data shared by several faces is
    // written last by the last face).
    if let Some(blocks) = blocks.as_mut() {
        let mut order: Vec<FaceId> = ifaces.keys().copied().collect();
        order.sort_unstable();
        for f in order {
            ifaces[&f].interp(bm, blocks);
        }
    }

    // Create the faces.
    for es in &edge_info {
        frap_math::budget::check();
        // The vertices in the right order (`BM_edge_ordered_verts_ex(e_new, &varr[1],
        // &varr[0], l)`).
        let mut varr: Vec<VertId> = vec![bm.lp(bm.lp(es.l).next).v, bm.lp(es.l).v];
        let [nv1, nv2] = bm.edge(es.e_new).v;
        let [ov1, ov2] = bm.edge(es.e_old).v;
        // Slightly trickier check, since the vertices may not be split.
        if varr[0] == nv1 {
            if ov2 != nv2 {
                varr.push(ov2);
            }
            if ov1 != nv1 {
                varr.push(ov1);
            }
        } else {
            if ov1 != nv1 {
                varr.push(ov1);
            }
            if ov2 != nv2 {
                varr.push(ov2);
            }
        }
        if varr.len() == 2 {
            // Can't make a face.
            continue;
        }
        // No need to check doubles: there are none. The reversed face is correct here.
        let lf = bm.lp(es.l).f;
        let f = bm
            .face_create_verts(&varr, Some(lf), CreateFlag::empty(), true)
            .expect("distinct corners");
        bm.oflag_enable(f, ELE_NEW);

        // Copy the loop data (otherwise UVs and colours are no good): the two inner loops on
        // the edge between the new face and the original.
        let l_a = bm.face(f).l_first;
        let l_b = bm.lp(l_a).next;
        let ea = bm.lp(l_a).e;
        // This side has a radial_next because of the order of the created vertices.
        let l_a_other = bm.edge_other_loop(ea, l_a);
        let l_b_other = bm.edge_other_loop(ea, l_b);
        bm.loop_attrs_copy(l_a_other, l_a);
        bm.loop_attrs_copy(l_b_other, l_b);

        // Step around to the opposite side of the quad (this may have no other edges).
        let l_a = bm.lp(bm.lp(l_a).next).next;
        let l_b = bm.lp(l_a).next;

        // Swap a and b intentionally.
        if let (true, Some(blocks)) = (use_interpolate, blocks.as_ref()) {
            let iface = &ifaces[&lf];
            let i_a = iface.loop_index(l_a_other).expect("stored loop");
            let i_b = iface.loop_index(l_b_other).expect("stored loop");
            blocks.copy_loop(bm, iface.l_slots[i_a], l_b);
            blocks.copy_loop(bm, iface.l_slots[i_b], l_a);

            if has_math_ldata {
                // Connecting edge 'a'.
                let l_a_prev = bm.lp(l_a).prev;
                let e_connect = bm.lp(l_a_prev).e;
                if bm.edge_is_manifold(e_connect) {
                    let o1 = bm.edge_other_loop(e_connect, l_a);
                    let o2 = bm.edge_other_loop(e_connect, l_a_prev);
                    loop_customdata_merge(bm, l_a, o1, l_a_prev, o2);
                }
                // Connecting edge 'b' (argument order swapped to keep the winding).
                let e_connect = bm.lp(l_b).e;
                if bm.edge_is_manifold(e_connect) {
                    let l_b_next = bm.lp(l_b).next;
                    let o1 = bm.edge_other_loop(e_connect, l_b);
                    let o2 = bm.edge_other_loop(e_connect, l_b_next);
                    loop_customdata_merge(bm, l_b, o1, l_b_next, o2);
                }
            }
        } else {
            bm.loop_attrs_copy(l_a_other, l_b);
            bm.loop_attrs_copy(l_b_other, l_a);
        }
    }

    let faces: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.oflag_test(f, ELE_NEW))
        .collect();

    // A cheap feature to add depth to the inset.
    if depth != 0.0 {
        // Re-calculate the normals of the tagged vertices from the faces they inset from.
        for es in &edge_info {
            for v in bm.edge(es.e_new).v {
                bm.vert_mut(v).no = [0.0; 3];
            }
        }
        for es in &edge_info {
            let no = bm.face(bm.lp(es.l).f).no;
            for v in bm.edge(es.e_new).v {
                let n = bm.vert(v).no;
                bm.vert_mut(v).no = geom::add(n, no);
            }
        }
        for es in &edge_info {
            for v in bm.edge(es.e_new).v {
                let n = bm.vert(v).no;
                // Avoid normalising twice.
                if geom::len_squared_v3(n) != 1.0 {
                    bm.vert_mut(v).no = geom::normalize_v3(n).0;
                }
            }
        }
        // Untag the vertices, then tag the vertices and edges of the input faces.
        for v in bm.verts.slots.iter_mut().flatten() {
            v.flag.remove(ElemFlag::TAG);
        }
        for &f in &i.faces {
            for l in bm.face_loops(f) {
                let (v, e) = (bm.lp(l).v, bm.lp(l).e);
                set_tag(bm, v, true);
                set_tag(bm, e, true);
            }
        }
        // Two passes, so moving the vertices does not feed back into the face angles that
        // `BM_vert_calc_shell_factor` uses.
        let mut vert_lengths: Option<Vec<(f32, i32)>> = None;
        let mut moved: Vec<(VertId, V3)> = Vec::new();
        for v in bm.vert_ids() {
            if tagged(bm, v) {
                let rel = if use_relative_offset {
                    let len = edge_info_average_length(bm, v, &edge_info);
                    if len != -1.0 {
                        len
                    } else {
                        edge_info_average_length_fallback(bm, v, &edge_info, &mut vert_lengths)
                    }
                } else {
                    1.0
                };
                let shell = if use_even_boundary {
                    vert_calc_shell_factor(bm, v)
                } else {
                    1.0
                };
                let fac = depth * rel * shell;
                let mut co = bm.vert(v).co;
                geom::madd_v3_v3fl(&mut co, bm.vert(v).no, fac);
                moved.push((v, co));
            }
        }
        for (v, co) in moved {
            bm.vert_mut(v).co = co;
        }
    }

    InsetRegionOut {
        geom: faces.iter().copied().map(BMElem::Face).collect(),
        faces,
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_loop_other_vert_loop).
/// The loop of `l`'s face at `v`'s neighbour along the face, away from `l`'s edge.
fn loop_other_vert_loop(bm: &BMesh, l: LoopId, v: VertId) -> LoopId {
    let ll = bm.lp(l);
    let v_prev = bm.edge_other_vert(ll.e, v);
    if ll.v == v {
        if bm.lp(ll.prev).v == v_prev {
            return ll.next;
        }
        return ll.prev;
    }
    if bm.lp(ll.prev).v == v {
        return bm.lp(ll.prev).prev;
    }
    bm.lp(ll.next).next
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2 × 1 grid of quads (`[0,1,4,3]`, `[1,2,5,4]`), normals +Z.
    fn grid() -> (BMesh, [FaceId; 2]) {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
            [2.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
        let a = bm
            .face_create_verts(&[v[0], v[1], v[4], v[3]], None, CreateFlag::empty(), true)
            .unwrap();
        let b = bm
            .face_create_verts(&[v[1], v[2], v[5], v[4]], None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        (bm, [a, b])
    }

    #[test]
    fn interior_edge_inset_keeps_the_face() {
        let (mut bm, [a, _]) = grid();
        let out = inset_region(
            &mut bm,
            &InsetRegionIn {
                faces: vec![a],
                thickness: 0.1,
                use_even_offset: true,
                ..Default::default()
            },
        )
        .unwrap();
        // Only the shared edge is between a tagged and an untagged face (no use_boundary).
        assert_eq!(out.faces.len(), 1);
        assert!(bm.faces.contains(a.0));
        assert!(bm.validate().is_empty());
        // The shared edge's vertices of the inset face moved 0.1 into it.
        let xs: Vec<f32> = bm.face_verts(a).iter().map(|&v| bm.vert(v).co[0]).collect();
        assert!(xs.iter().any(|&x| (x - 0.9).abs() < 1e-6), "{xs:?}");
    }

    #[test]
    fn boundary_inset_rings_the_face() {
        let (mut bm, [a, _]) = grid();
        let out = inset_region(
            &mut bm,
            &InsetRegionIn {
                faces: vec![a],
                thickness: 0.1,
                use_boundary: true,
                use_even_offset: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.faces.len(), 4);
        assert_eq!(bm.faces_num(), 6);
        for c in bm.face_verts(a).iter().map(|&v| bm.vert(v).co) {
            assert!(c[0] > 0.05 && c[0] < 0.95 && c[1] > 0.05 && c[1] < 0.95, "{c:?}");
        }
        assert!(bm.validate().is_empty());
    }
}
