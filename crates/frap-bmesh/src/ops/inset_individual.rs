// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.inset_individual` (Blender 5.2.4 `bmo_inset_individual_exec`,
//! `operators/bmo_inset.cc`), implemented by WP 2.5 (Extrude and inset).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.inset_individual(bmesh, faces=[], thickness=0.0, depth=0.0, use_even_offset=False, use_interpolate=False, use_relative_offset=False)`.
//!
//! **Create/kill order**: nothing is killed. For every input face, in slot order:
//! 1. each corner, in loop order from `l_first`, is split off its neighbours
//!    (`BM_face_loop_separate`): the face's two edges at the corner are separated from the
//!    other faces (new edges, `bmesh_kernel_edge_separate`) and the corner gets a new vertex;
//!    the original vertex stays with the neighbours (the new face corner position is moved
//!    inward later, the original keeps the outer position);
//! 2. per corner, in loop order: the outer edge between the two original vertices (taking the
//!    face edge as example; the existing one when the corner was not split), then the rim quad
//!    `[outer, outer next, inner next, inner]` (`BM_face_create_quad_tri`, missing edges first);
//! 3. the inner corners move by `thickness` (and `depth` along the face normal).
//!
//! The input face keeps its slot and loops and becomes the inner face. `faces.out` are the rim
//! quads. With `use_interpolate` the inner face's corner and vertex data are re-interpolated
//! from the face as it was (`BM_face_interp_from_face_ex`) and the rim takes the result.
//!
//! This file also holds the helpers both inset operators share (Blender's `bmo_inset.cc`
//! statics and the `BLI_math` functions they call).
//!
//! Operator type flags: `NORMALS_CALC` (the caller flushes the selection).

use std::sync::Arc;

use frap_mesh::AttrData;

use crate::customdata::CustomData;
use crate::query::geom::{self, V3};
use crate::types::{CreateFlag, OpTypeFlag, ShapeLayer};
use crate::{BMElem, BMesh, BmError, EdgeId, ElemFlag, FaceId, LoopId, VertId};

/// Input slots of `inset_individual` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InsetIndividualIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `thickness`.
    pub thickness: f32,
    /// `depth`.
    pub depth: f32,
    /// `use_even_offset`.
    pub use_even_offset: bool,
    /// `use_interpolate`.
    pub use_interpolate: bool,
    /// `use_relative_offset`.
    pub use_relative_offset: bool,
}

/// Output slots of `inset_individual`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct InsetIndividualOut {
    /// The new faces as elements (same content as `faces`).
    pub geom: Vec<BMElem>,
    /// `faces.out`: the new rim faces, in iteration order.
    pub faces: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_inset_individual_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

/// `ELE_NEW` of `bmo_inset.cc`.
pub(crate) const ELE_NEW: u16 = 1;

// ------------------------------------------------------------------------------ BLI math

/// `SMALL_NUMBER` of `math_geom_inline.cc`.
const SMALL_NUMBER: f32 = 1.0e-8;

/// `shell_v3v3_normalized_to_dist`.
pub(crate) fn shell_v3v3_normalized_to_dist(a: V3, b: V3) -> f32 {
    let angle_cos = geom::dot(a, b).abs();
    if angle_cos < SMALL_NUMBER {
        1.0
    } else {
        1.0 / angle_cos
    }
}

/// `shell_v3v3_mid_normalized_to_dist`.
pub(crate) fn shell_v3v3_mid_normalized_to_dist(a: V3, b: V3) -> f32 {
    let (ab, len) = geom::normalize_v3(geom::add(a, b));
    let angle_cos = if len != 0.0 {
        geom::dot(a, ab).abs()
    } else {
        0.0
    };
    if angle_cos < SMALL_NUMBER {
        1.0
    } else {
        1.0 / angle_cos
    }
}

/// `saasinf` / `safe_asinf`.
fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -std::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        std::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `angle_normalized_v3v3`.
pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
    if geom::dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(frap_geom::normals::len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    std::f32::consts::PI - 2.0 * safe_asinf(frap_geom::normals::len_v3v3(v1, v2_n) / 2.0)
}

/// `angle_v3v3v3(a, b, c)`: the angle at `b`.
pub(crate) fn angle_v3v3v3(a: V3, b: V3, c: V3) -> f32 {
    let (v1, _) = geom::normalize_v3(geom::sub(b, a));
    let (v2, _) = geom::normalize_v3(geom::sub(b, c));
    angle_normalized_v3v3(v1, v2)
}

// ------------------------------------------------------------------------------ BMesh queries

/// `BM_edge_calc_length`.
pub(crate) fn edge_calc_length(bm: &BMesh, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    frap_geom::normals::len_v3v3(bm.vert(a).co, bm.vert(b).co)
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_edge_calc_face_tangent).
/// `BM_edge_calc_face_tangent(e, e_loop)`: the unit vector in `e_loop`'s face, perpendicular
/// to the edge, pointing into the face.
pub(crate) fn edge_calc_face_tangent(bm: &BMesh, l: LoopId) -> V3 {
    let ll = bm.lp(l);
    let v1 = bm.vert(ll.v).co;
    let v2 = bm.vert(bm.lp(ll.next).v).co;
    let tvec = geom::sub(v1, v2);
    geom::normalize_v3(geom::cross(tvec, bm.face(ll.f).no)).0
}

/// `BM_loop_calc_face_angle`.
pub(crate) fn loop_calc_face_angle(bm: &BMesh, l: LoopId) -> f32 {
    let ll = bm.lp(l);
    angle_v3v3v3(
        bm.vert(bm.lp(ll.prev).v).co,
        bm.vert(ll.v).co,
        bm.vert(bm.lp(ll.next).v).co,
    )
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (BM_vert_calc_shell_factor).
/// `BM_vert_calc_shell_factor`: the angle-weighted mean of `1 / cos` between the vertex normal
/// and the normals of its faces.
pub(crate) fn vert_calc_shell_factor(bm: &BMesh, v: VertId) -> f32 {
    let mut accum_shell = 0.0f32;
    let mut accum_angle = 0.0f32;
    let no = bm.vert(v).no;
    for l in bm.vert_loops(v) {
        let face_angle = loop_calc_face_angle(bm, l);
        accum_shell += shell_v3v3_normalized_to_dist(no, bm.face(bm.lp(l).f).no) * face_angle;
        accum_angle += face_angle;
    }
    if accum_angle != 0.0 {
        accum_shell / accum_angle
    } else {
        1.0
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_core.cc
// (bmesh_kernel_unglue_region_make_vert, `BM_face_loop_separate`).
/// `BM_face_loop_separate(l_sep)`: detaches the corner `l_sep` from the faces around its
/// vertex (its two face edges are separated first); returns the vertex the corner now uses
/// (a new one, or the original when nothing was attached).
pub(crate) fn face_loop_separate(bm: &mut BMesh, l_sep: LoopId) -> VertId {
    let v_sep = bm.lp(l_sep).v;
    let e = bm.lp(l_sep).e;
    if !bm.edge_is_boundary(e) {
        bm.edge_separate(e, l_sep, false);
    }
    let l_prev = bm.lp(l_sep).prev;
    let e_prev = bm.lp(l_prev).e;
    if !bm.edge_is_boundary(e_prev) {
        bm.edge_separate(e_prev, l_prev, false);
    }
    let edges = [bm.lp(l_sep).e, bm.lp(bm.lp(l_sep).prev).e];
    // Search the disk for one of the corner's edges.
    let first = bm.vert(v_sep).e.expect("corner vertex has edges");
    let mut e_iter = first;
    while !edges.contains(&e_iter) {
        e_iter = bm.disk_edge_next(e_iter, v_sep);
        if e_iter == first {
            return v_sep;
        }
    }
    bm.vert_mut(v_sep).e = Some(edges[0]);
    let co = bm.vert(v_sep).co;
    let v_new = bm.vert_create(co, Some(v_sep), CreateFlag::empty());
    for e in edges {
        bm.edge_vert_swap(e, v_new, v_sep);
    }
    v_new
}

// ------------------------------------------------------------------------------ custom data

/// Whether a column type interpolates (`LayerTypeInfo::interp`, as `CustomData::interp`).
fn col_interpolates(d: &AttrData) -> bool {
    matches!(
        d,
        AttrData::Float(_)
            | AttrData::Int(_)
            | AttrData::Bool(_)
            | AttrData::Float2(_)
            | AttrData::Float3(_)
            | AttrData::Float4(_)
            | AttrData::FloatColor(_)
            | AttrData::ByteColor(_)
            | AttrData::Quaternion(_)
    )
}

/// Whether a column type has the math callbacks of `CustomData_layer_has_math` (UVs and
/// colours).
pub(crate) fn col_has_math(d: &AttrData) -> bool {
    matches!(
        d,
        AttrData::Float2(_) | AttrData::FloatColor(_) | AttrData::ByteColor(_)
    )
}

/// Copies the value at slot `s` of `src` to slot `d` of `dst` (same column type).
pub(crate) fn copy_col_value(src: &AttrData, s: usize, dst: &mut AttrData, d: usize) {
    macro_rules! cp {
        ($($v:ident),*) => {
            match (src, dst) {
                $((AttrData::$v(a), AttrData::$v(b)) => {
                    if let Some(x) = a.get(s) {
                        let b = Arc::make_mut(b);
                        #[allow(clippy::clone_on_copy)]
                        if b.len() <= d {
                            b.resize(d + 1, x.clone());
                        } else {
                            b[d] = x.clone();
                        }
                    }
                })*
                _ => debug_assert!(false, "copy_col_value: column types differ"),
            }
        };
    }
    cp!(
        Float, Int, Int8, Bool, Float2, Float3, Float4, FloatColor, ByteColor, Int16x2, Int32x2,
        Quaternion, Float4x4, String
    );
}

/// Copies slot `s` of `src` into slot `d` of `dst` for every layer (`interp_only`: only the
/// layers whose type interpolates; vertex groups follow `interp_only` too).
fn copy_cd(src: &CustomData, s: u32, dst: &mut CustomData, d: u32, interp_only: bool) {
    for (ls, ld) in src.layers.iter().zip(dst.layers.iter_mut()) {
        debug_assert_eq!(ls.name, ld.name);
        if interp_only && !col_interpolates(&ls.data) {
            continue;
        }
        copy_col_value(&ls.data, s as usize, &mut ld.data, d as usize);
    }
    if let (Some(a), Some(b)) = (&src.deform, &mut dst.deform)
        && let Some(x) = a.get(s as usize)
    {
        if b.len() <= d as usize {
            b.resize(d as usize + 1, Default::default());
        }
        b[d as usize] = x.clone();
    }
}

/// The custom data "blocks" an inset keeps for later (`CustomData_bmesh_copy_block` of the
/// faces it re-interpolates): a copy of the loop and vertex columns (and shape keys) taken
/// before the op changes anything. The columns are `Arc`-shared until the BMesh writes, so
/// taking it is cheap. Saved blocks are addressed by the element's slot at that time.
pub(crate) struct CdBlocks {
    ldata: CustomData,
    vdata: CustomData,
    shapes: Vec<ShapeLayer>,
    /// A scratch slot past every saved column (interpolation target before the copy).
    l_tmp: u32,
    v_tmp: u32,
}

impl CdBlocks {
    /// Saves the current loop and vertex data of `bm`.
    pub(crate) fn save(bm: &BMesh) -> CdBlocks {
        let len = |cd: &CustomData, cap: usize| {
            cd.layers
                .iter()
                .map(|l| l.data.len())
                .chain(cd.deform.as_ref().map(Vec::len))
                .fold(cap, usize::max) as u32
        };
        CdBlocks {
            ldata: bm.ldata.clone(),
            vdata: bm.vdata.clone(),
            shapes: bm.shape_layers.clone(),
            l_tmp: len(&bm.ldata, bm.loops.capacity()),
            v_tmp: len(&bm.vdata, bm.verts.capacity()),
        }
    }

    fn has_ldata(&self) -> bool {
        !self.ldata.layers.is_empty()
    }
    fn has_vdata(&self) -> bool {
        !self.vdata.layers.is_empty() || self.vdata.deform.is_some() || !self.shapes.is_empty()
    }

    /// `CustomData_bmesh_interp(&bm->ldata, blocks, w, n, l_dst)`.
    pub(crate) fn interp_loop(&mut self, bm: &mut BMesh, src: &[u32], w: &[f32], dst: LoopId) {
        if !self.has_ldata() {
            return;
        }
        self.ldata.interp(src, w, self.l_tmp);
        copy_cd(&self.ldata, self.l_tmp, &mut bm.ldata, dst.0, true);
    }

    /// `CustomData_bmesh_interp(&bm->vdata, blocks, w, n, v_dst)` (shape keys included, as
    /// `CD_SHAPEKEY` layers are part of Blender's vertex blocks).
    pub(crate) fn interp_vert(&mut self, bm: &mut BMesh, src: &[u32], w: &[f32], dst: VertId) {
        if !self.has_vdata() {
            return;
        }
        self.vdata.interp(src, w, self.v_tmp);
        copy_cd(&self.vdata, self.v_tmp, &mut bm.vdata, dst.0, true);
        for (saved, live) in self.shapes.iter().zip(bm.shape_layers.iter_mut()) {
            let mut co = [0.0f32; 3];
            for (&s, &wt) in src.iter().zip(w) {
                let c = saved.co.get(s as usize).copied().unwrap_or([0.0; 3]);
                geom::madd_v3_v3fl(&mut co, c, wt);
            }
            let d = dst.slot();
            if live.co.len() <= d {
                live.co.resize(d + 1, [0.0; 3]);
            }
            live.co[d] = co;
        }
    }

    /// `CustomData_bmesh_copy_block(bm->ldata, block, &l_dst->head.data)`: every loop layer
    /// of the saved slot `src`.
    pub(crate) fn copy_loop(&self, bm: &mut BMesh, src: u32, dst: LoopId) {
        copy_cd(&self.ldata, src, &mut bm.ldata, dst.0, false);
    }
}

/// `InterpFace`: what an inset keeps of a face to re-interpolate it once its corners moved
/// (`bm_interp_face_store`).
pub(crate) struct InterpFace {
    /// The face.
    pub(crate) f: FaceId,
    /// Its corners at store time, in loop order (their position is `BM_elem_index_get(l)`).
    pub(crate) loops: Vec<LoopId>,
    /// The saved loop and vertex blocks (slots in [`CdBlocks`]).
    pub(crate) l_slots: Vec<u32>,
    v_slots: Vec<u32>,
    cos_2d: Vec<[f32; 2]>,
    rows: [V3; 2],
}

impl InterpFace {
    // Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bm_interp_face_store).
    /// Stores `f`; `vert_block` gives the saved vertex block of each corner's vertex.
    pub(crate) fn store(bm: &BMesh, f: FaceId, vert_block: impl Fn(VertId) -> u32) -> InterpFace {
        let rows = geom::axis_dominant_v3_rows(bm.face(f).no);
        let loops = bm.face_loops(f);
        let mut it = InterpFace {
            f,
            l_slots: loops.iter().map(|l| l.0).collect(),
            v_slots: loops.iter().map(|&l| vert_block(bm.lp(l).v)).collect(),
            cos_2d: loops
                .iter()
                .map(|&l| geom::project_v2(&rows, bm.vert(bm.lp(l).v).co))
                .collect(),
            loops,
            rows,
        };
        it.loops.shrink_to_fit();
        it
    }

    /// The position of `l` in the stored face (`BM_elem_index_get(l)` after the store).
    pub(crate) fn loop_index(&self, l: LoopId) -> Option<usize> {
        self.loops.iter().position(|&x| x == l)
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_interp.cc
    // (BM_face_interp_from_face_ex).
    /// `BM_face_interp_from_face_ex(bm, f_dst = self.f, f_src = self.f, do_vertex = true, …)`:
    /// every corner (and its vertex) of the face re-interpolated from the stored face at the
    /// corner's current position.
    pub(crate) fn interp(&self, bm: &mut BMesh, blocks: &mut CdBlocks) {
        for l in bm.face_loops(self.f) {
            let v = bm.lp(l).v;
            let co = geom::project_v2(&self.rows, bm.vert(v).co);
            let w = geom::interp_weights_poly_v2(&self.cos_2d, co);
            blocks.interp_loop(bm, &self.l_slots, &w, l);
            blocks.interp_vert(bm, &self.v_slots, &w, v);
        }
    }
}

// ------------------------------------------------------------------------------ the op

// Ported from blender/source/blender/bmesh/operators/bmo_inset.cc (bmo_face_inset_individual).
#[allow(clippy::too_many_arguments)]
fn face_inset_individual(
    bm: &mut BMesh,
    f: FaceId,
    blocks: Option<&mut CdBlocks>,
    thickness: f32,
    depth: f32,
    use_even_offset: bool,
    use_relative_offset: bool,
    use_interpolate: bool,
) {
    let loops = bm.face_loops(f);
    let n = loops.len();
    // The vertices split away from the face (aligned with the face's corners).
    let mut verts: Vec<VertId> = Vec::with_capacity(n);
    // The edge normals (aligned with the face's loop edges).
    let mut edge_nors: Vec<V3> = Vec::with_capacity(n);
    // The original vertex of each corner (its saved vertex block).
    let mut orig: Vec<VertId> = Vec::with_capacity(n);

    // Split off all loops.
    for &l in &loops {
        let mut v_other = bm.lp(l).v;
        orig.push(v_other);
        let v_sep = face_loop_separate(bm, l);
        if v_sep == v_other {
            let v = bm.lp(l).v;
            let co = bm.vert(v).co;
            v_other = bm.vert_create(co, Some(v), CreateFlag::empty());
        }
        verts.push(v_other);
        // Unrelated to splitting, but calculated here.
        edge_nors.push(edge_calc_face_tangent(bm, l));
    }

    // Build the rim faces.
    for (i, &l) in loops.iter().enumerate() {
        let v_other = verts[i];
        let v_other_next = verts[(i + 1) % n];
        let e = bm.lp(l).e;
        bm.edge_create(v_other, v_other_next, Some(e), CreateFlag::NO_DOUBLE);
        let l_next = bm.lp(l).next;
        let quad = [v_other, v_other_next, bm.lp(l_next).v, bm.lp(l).v];
        let f_new_outer = bm
            .face_create_verts(&quad, Some(f), CreateFlag::empty(), true)
            .expect("four distinct corners");
        bm.oflag_enable(f_new_outer, ELE_NEW);
        // Copy the loop data.
        let l_other = bm.lp(l).radial_next;
        let lo_prev = bm.lp(l_other).prev;
        let lo_next2 = bm.lp(bm.lp(l_other).next).next;
        bm.loop_attrs_copy(l_next, lo_prev);
        bm.loop_attrs_copy(l, lo_next2);
        if !use_interpolate {
            let lo_next = bm.lp(l_other).next;
            bm.loop_attrs_copy(l_next, l_other);
            bm.loop_attrs_copy(l, lo_next);
        }
    }

    // Hold the interpolation values.
    let iface = use_interpolate.then(|| {
        let map: Vec<(VertId, u32)> = loops
            .iter()
            .zip(&orig)
            .map(|(&l, o)| (bm.lp(l).v, o.0))
            .collect();
        InterpFace::store(bm, f, |v| {
            map.iter()
                .find(|(x, _)| *x == v)
                .map(|&(_, s)| s)
                .unwrap_or(v.0)
        })
    });

    // Calculate the translation vectors of the new corners.
    let mut e_length_prev = if depth != 0.0 {
        edge_calc_length(bm, bm.lp(bm.lp(loops[0]).prev).e)
    } else {
        0.0
    };
    let fno = bm.face(f).no;
    let mut coords: Vec<V3> = Vec::with_capacity(n);
    for (i, &l) in loops.iter().enumerate() {
        let eno_prev = edge_nors[if i == 0 { n } else { i } - 1];
        let eno_next = edge_nors[i];
        let mut tvec = geom::normalize_v3(geom::add(eno_prev, eno_next)).0;
        let v = bm.lp(l).v;
        let mut v_new_co = bm.vert(v).co;
        if use_even_offset {
            tvec = geom::scale(tvec, shell_v3v3_mid_normalized_to_dist(eno_prev, eno_next));
        }
        // Modify the vertices and their normals.
        if use_relative_offset {
            let len =
                edge_calc_length(bm, bm.lp(l).e) + edge_calc_length(bm, bm.lp(bm.lp(l).prev).e);
            tvec = geom::scale(tvec, len / 2.0);
        }
        geom::madd_v3_v3fl(&mut v_new_co, tvec, thickness);
        // Set the normal, add the depth and write the new vertex position.
        bm.vert_mut(v).no = fno;
        if depth != 0.0 {
            let e_length = edge_calc_length(bm, bm.lp(l).e);
            let fac = depth
                * if use_relative_offset {
                    (e_length_prev + e_length) * 0.5
                } else {
                    1.0
                };
            e_length_prev = e_length;
            geom::madd_v3_v3fl(&mut v_new_co, fno, fac);
        }
        coords.push(v_new_co);
    }
    // Update the coordinates.
    for (&l, co) in loops.iter().zip(coords) {
        let v = bm.lp(l).v;
        bm.vert_mut(v).co = co;
    }

    if let (Some(iface), Some(blocks)) = (iface, blocks) {
        iface.interp(bm, blocks);
        // Rebuild the rim's inner loop data.
        for &l in &loops {
            let l_other = bm.lp(l).radial_next;
            let l_next = bm.lp(l).next;
            let lo_next = bm.lp(l_other).next;
            bm.loop_attrs_copy(l_next, l_other);
            bm.loop_attrs_copy(l, lo_next);
        }
    }
}

/// Runs `inset_individual` on `bm`.
pub fn inset_individual(
    bm: &mut BMesh,
    i: &InsetIndividualIn,
) -> Result<InsetIndividualOut, BmError> {
    if let Some(dead) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!(
            "inset_individual: dead face {dead:?} in faces"
        )));
    }
    let out = bm.bmo_exec(TYPE_FLAG, |bm| {
        // Ported from blender/source/blender/bmesh/operators/bmo_inset.cc
        // (bmo_inset_individual_exec). Only tag the faces in the slot.
        for f in bm.faces.slots.iter_mut().flatten() {
            f.flag.remove(ElemFlag::TAG);
        }
        for &f in &i.faces {
            bm.face_mut(f).flag.insert(ElemFlag::TAG);
        }
        let mut blocks = i.use_interpolate.then(|| CdBlocks::save(bm));
        for &f in &i.faces {
            frap_math::budget::check();
            face_inset_individual(
                bm,
                f,
                blocks.as_mut(),
                i.thickness,
                i.depth,
                i.use_even_offset,
                i.use_relative_offset,
                i.use_interpolate,
            );
        }
        let faces: Vec<FaceId> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, ELE_NEW))
            .collect();
        InsetIndividualOut {
            geom: faces.iter().copied().map(BMElem::Face).collect(),
            faces,
        }
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_factors() {
        let a = [1.0, 0.0, 0.0];
        assert_eq!(shell_v3v3_normalized_to_dist(a, a), 1.0);
        let b = geom::normalize_v3([1.0, 1.0, 0.0]).0;
        assert!((shell_v3v3_normalized_to_dist(a, b) - std::f32::consts::SQRT_2).abs() < 1e-6);
        // Perpendicular unit vectors: the mid vector is at 45 degrees.
        let c = [0.0, 1.0, 0.0];
        assert!((shell_v3v3_mid_normalized_to_dist(a, c) - std::f32::consts::SQRT_2).abs() < 1e-6);
        assert_eq!(shell_v3v3_mid_normalized_to_dist(a, [-1.0, 0.0, 0.0]), 1.0);
        assert!(
            (angle_v3v3v3([1.0, 0.0, 0.0], [0.0; 3], [0.0, 1.0, 0.0])
                - std::f32::consts::FRAC_PI_2)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn lone_quad_inset() {
        let mut bm = BMesh::new();
        let v: Vec<VertId> = [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ]
        .into_iter()
        .map(|co| bm.vert_create(co, None, CreateFlag::empty()))
        .collect();
        let f = bm
            .face_create_verts(&v, None, CreateFlag::empty(), true)
            .unwrap();
        bm.normals_update();
        let out = inset_individual(
            &mut bm,
            &InsetIndividualIn {
                faces: vec![f],
                thickness: 0.25,
                depth: 0.5,
                use_even_offset: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.faces.len(), 4);
        assert_eq!((bm.verts_num(), bm.edges_num(), bm.faces_num()), (8, 12, 5));
        for c in bm.face_verts(f).iter().map(|&x| bm.vert(x).co) {
            assert!(
                (c[0].abs() - 0.75).abs() < 1e-6 && (c[1].abs() - 0.75).abs() < 1e-6,
                "{c:?}"
            );
            assert!((c[2] - 0.5).abs() < 1e-6);
        }
        // The outer ring keeps the original positions.
        for &x in &v {
            assert_eq!(bm.vert(x).co[2], 0.0);
        }
        assert!(bm.validate().is_empty());
    }
}
