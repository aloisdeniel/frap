// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.subdivide_edges` (Blender 5.2.4 `bmo_subdivide_edges_exec`) and
//! `BM_mesh_esubdivide` (the edit-mode wrapper with its selection modes), implemented by
//! WP 2.6 (Subdivide and loops).
//!
//! Ported from blender/source/blender/bmesh/operators/bmo_subdivide.cc (Blender 5.2.4,
//! GPL-2.0-or-later); `bmesh_subd_falloff_calc` from bmesh/intern/bmesh_query.cc and
//! `BM_vert_pair_share_face_by_len` from bmesh/intern/bmesh_query.cc.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.subdivide_edges(bmesh, edges=[], smooth=0.0, smooth_falloff='SMOOTH', fractal=0.0, along_normal=0.0, cuts=0, seed=0, custom_patterns={}, edge_percents={}, quad_corner_type='STRAIGHT_CUT', use_grid_fill=False, use_single_edge=False, use_only_quads=False, use_sphere=False, use_smooth_even=False)`.
//!
//! **Create/kill order** (nothing is killed; new elements take freed slots first, LIFO):
//! 1. Every input edge, in **edge iteration order** (the input buffer is rebuilt from the
//!    tagged edges), is cut `cuts` times from `v1` towards `v2` (`BM_edge_split` at `v1`, so
//!    each cut creates one vertex then one edge, and the vertices of an edge are created from
//!    `v1` to `v2`).
//! 2. The faces that match a pattern (or have exactly two cut edges and no pattern) are
//!    gathered in face iteration order onto a **stack** and filled in **reverse** order: the
//!    last matching face is filled first. Pattern fills create vertices (grid interiors, inner
//!    vertices), edges and faces through `BM_face_split` (one edge and one face per split); a
//!    whole cube with one cut therefore gets its face centres in reversed face order.
//!
//! Smooth, sphere and fractal displacements are written to a temporary shape layer while
//! cutting (so later cuts interpolate the undisplaced positions) and copied to the vertex
//! positions afterwards, as Blender does with its temporary `CD_SHAPEKEY` layer; the
//! difference is also applied to the other shape layers (Blender's loop skips the first layer
//! and includes the temporary one; ported as is). Vertex creases of new vertices are zeroed.
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE` (plus
//! `UNTAN_MULTIRES`, a no-op without multires).
//!
//! `custom_patterns` (internal in Blender, not reachable from Python) maps a face to an index
//! of [`PATTERN_TABLE`].

pub(crate) mod blimath;
mod noise;

pub(crate) use blimath as math;

use rustc_hash::FxHashMap;

use crate::types::{ElemFlag, LoopId, OpTypeFlag, ShapeLayer, VertId};
use crate::{BMElem, BMesh, BmError, EdgeId, FaceId};
use math::V3;

/// Input slots of `subdivide_edges` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct SubdivideEdgesIn {
    /// `edges`.
    pub edges: Vec<EdgeId>,
    /// `smooth`.
    pub smooth: f32,
    /// `smooth_falloff` (enum identifier).
    pub smooth_falloff: &'static str,
    /// `fractal`.
    pub fractal: f32,
    /// `along_normal`.
    pub along_normal: f32,
    /// `cuts`.
    pub cuts: i32,
    /// `seed`.
    pub seed: i32,
    /// `custom_patterns` (face -> pattern (unused by the UI)).
    pub custom_patterns: Vec<(FaceId, i32)>,
    /// `edge_percents` (edge -> cut factor).
    pub edge_percents: Vec<(EdgeId, f32)>,
    /// `quad_corner_type` (enum identifier).
    pub quad_corner_type: &'static str,
    /// `use_grid_fill`.
    pub use_grid_fill: bool,
    /// `use_single_edge`.
    pub use_single_edge: bool,
    /// `use_only_quads`.
    pub use_only_quads: bool,
    /// `use_sphere`.
    pub use_sphere: bool,
    /// `use_smooth_even`.
    pub use_smooth_even: bool,
}

impl Default for SubdivideEdgesIn {
    fn default() -> Self {
        SubdivideEdgesIn {
            edges: Vec::new(),
            smooth: 0.0,
            smooth_falloff: "SMOOTH",
            fractal: 0.0,
            along_normal: 0.0,
            cuts: 0,
            seed: 0,
            custom_patterns: Vec::new(),
            edge_percents: Vec::new(),
            quad_corner_type: "STRAIGHT_CUT",
            use_grid_fill: false,
            use_single_edge: false,
            use_only_quads: false,
            use_sphere: false,
            use_smooth_even: false,
        }
    }
}

/// Output slots of `subdivide_edges`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SubdivideEdgesOut {
    /// `geom.out`: every element tagged inner, split or input (vertices, then edges, then
    /// faces, each in iteration order).
    pub geom: Vec<BMElem>,
    /// `geom_inner.out`: the new vertices, the edges and faces created by the fills.
    pub geom_inner: Vec<BMElem>,
    /// `geom_split.out`: the cut vertices and the edge pieces of the cut edges.
    pub geom_split: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_subdivide_edges_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// Flags for all elements share a common bit-field space.
const SUBD_SPLIT: u16 = 1;
const EDGE_PERCENT: u16 = 2;
const FACE_CUSTOMFILL: u16 = 4;
const ELE_INNER: u16 = 8;
const ELE_SPLIT: u16 = 16;

/// A value of 0.00005 means face splits at a little under 1.0 degrees (#32665).
const FLT_FACE_SPLIT_EPSILON: f32 = 0.00005;

/// `SUBD_FALLOFF_*` (the bmesh enum `smooth_falloff`, also `loopcut.falloff` and the
/// edge-ring `profile_shape`, whose RNA values match).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Falloff {
    /// `SMOOTH`
    Smooth,
    /// `SPHERE`
    Sphere,
    /// `ROOT`
    Root,
    /// `SHARP`
    Sharp,
    /// `LINEAR`
    Linear,
    /// `INVERSE_SQUARE`
    InverseSquare,
}

impl Falloff {
    /// Parses Blender's enum identifier.
    pub fn from_identifier(s: &str) -> Option<Falloff> {
        Some(match s {
            "SMOOTH" => Falloff::Smooth,
            "SPHERE" => Falloff::Sphere,
            "ROOT" => Falloff::Root,
            "SHARP" => Falloff::Sharp,
            "LINEAR" => Falloff::Linear,
            "INVERSE_SQUARE" => Falloff::InverseSquare,
            _ => return None,
        })
    }

    // Ported from blender/source/blender/bmesh/intern/bmesh_query.cc (bmesh_subd_falloff_calc).
    /// `bmesh_subd_falloff_calc`.
    pub fn calc(self, val: f32) -> f32 {
        match self {
            Falloff::Smooth => 3.0 * val * val - 2.0 * val * val * val,
            Falloff::Sphere => libm::sqrtf(2.0 * val - val * val),
            Falloff::Root => libm::sqrtf(val),
            Falloff::Sharp => val * val,
            Falloff::Linear => val,
            Falloff::InverseSquare => val * (2.0 - val),
        }
    }
}

/// `SUBD_CORNER_*` (`quad_corner_type`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CornerType {
    /// `STRAIGHT_CUT`
    StraightCut,
    /// `INNER_VERT` (`INNERVERT` in `mesh.subdivide`)
    InnerVert,
    /// `PATH`
    Path,
    /// `FAN`
    Fan,
}

impl CornerType {
    /// Parses the bmesh identifier (`INNER_VERT`) or the operator one (`INNERVERT`).
    pub fn from_identifier(s: &str) -> Option<CornerType> {
        Some(match s {
            "STRAIGHT_CUT" => CornerType::StraightCut,
            "INNER_VERT" | "INNERVERT" => CornerType::InnerVert,
            "PATH" => CornerType::Path,
            "FAN" => CornerType::Fan,
            _ => return None,
        })
    }
}

/// The fill function of a pattern (`subd_pattern_fill_fp`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Fill {
    Quad1Edge,
    Quad2EdgePath,
    Quad2EdgeInnerVert,
    Quad2EdgeFan,
    Quad3Edge,
    Quad4Edge,
    Tri1Edge,
    Tri3Edge,
}

/// `SubDPattern`: the selected-edge mask (its length is the face length) and the fill.
#[derive(Debug, PartialEq)]
pub struct Pattern {
    seledges: &'static [bool],
    fill: Fill,
}

static QUAD_1EDGE: Pattern = Pattern {
    seledges: &[true, false, false, false],
    fill: Fill::Quad1Edge,
};
static QUAD_2EDGE_PATH: Pattern = Pattern {
    seledges: &[true, true, false, false],
    fill: Fill::Quad2EdgePath,
};
static QUAD_2EDGE_INNERVERT: Pattern = Pattern {
    seledges: &[true, true, false, false],
    fill: Fill::Quad2EdgeInnerVert,
};
static QUAD_2EDGE_FAN: Pattern = Pattern {
    seledges: &[true, true, false, false],
    fill: Fill::Quad2EdgeFan,
};
static QUAD_3EDGE: Pattern = Pattern {
    seledges: &[true, true, true, false],
    fill: Fill::Quad3Edge,
};
static QUAD_4EDGE: Pattern = Pattern {
    seledges: &[true, true, true, true],
    fill: Fill::Quad4Edge,
};
static TRI_1EDGE: Pattern = Pattern {
    seledges: &[true, false, false],
    fill: Fill::Tri1Edge,
};
static TRI_3EDGE: Pattern = Pattern {
    seledges: &[true, true, true],
    fill: Fill::Tri3Edge,
};

/// The patterns `custom_patterns` indices refer to: quad 1 edge, quad 2 edges (path, inner
/// vertex, fan), quad 3 edges, quad 4 edges, triangle 1 edge, triangle 3 edges.
pub static PATTERN_TABLE: [&Pattern; 8] = [
    &QUAD_1EDGE,
    &QUAD_2EDGE_PATH,
    &QUAD_2EDGE_INNERVERT,
    &QUAD_2EDGE_FAN,
    &QUAD_3EDGE,
    &QUAD_4EDGE,
    &TRI_1EDGE,
    &TRI_3EDGE,
];

/// A vertex position and normal as read by `alter_co` (`v_a`, `v_b`).
#[derive(Copy, Clone, Debug)]
struct VSnap {
    co: V3,
    no: V3,
}

/// `SubDParams`.
struct Params {
    numcuts: i32,
    smooth: f32,
    smooth_falloff: Falloff,
    fractal: f32,
    along_normal: f32,
    use_smooth: bool,
    use_smooth_even: bool,
    use_sphere: bool,
    use_fractal: bool,
    fractal_ofs: V3,
    edge_percents: FxHashMap<EdgeId, f32>,
    /// Index of the temporary shape layer (`shape_info.tmpkey`).
    tmpkey: usize,
    /// `shape_info.totlayer` (the temporary layer included).
    totlayer: usize,
}

impl BMesh {
    fn snap(&self, v: VertId) -> VSnap {
        let x = self.vert(v);
        VSnap { co: x.co, no: x.no }
    }
    fn tmp_co(&self, p: &Params, v: VertId) -> V3 {
        self.shape_layers[p.tmpkey].co[v.slot()]
    }
    fn set_tmp_co(&mut self, p: &Params, v: VertId, co: V3) {
        let col = &mut self.shape_layers[p.tmpkey].co;
        if col.len() <= v.slot() {
            col.resize(v.slot() + 1, [0.0; 3]);
        }
        col[v.slot()] = co;
    }
}

// Ported from blender/source/blender/bmesh/intern/bmesh_query.cc
// (BM_vert_pair_share_face_by_len).
/// `BM_vert_pair_share_face_by_len`: the smallest face using both vertices, with their loops.
pub(crate) fn vert_pair_share_face_by_len(
    bm: &BMesh,
    v_a: VertId,
    v_b: VertId,
    allow_adjacent: bool,
) -> Option<(FaceId, LoopId, LoopId)> {
    let mut cur: Option<(FaceId, LoopId, LoopId)> = None;
    if bm.vert(v_a).e.is_some() && bm.vert(v_b).e.is_some() {
        for l_a in bm.vert_loops(v_a) {
            let f = bm.lp(l_a).f;
            if cur.is_none_or(|(fc, _, _)| bm.face(f).len < bm.face(fc).len)
                && let Some(l_b) = bm.face_vert_share_loop(f, v_b)
                && (allow_adjacent || !loop_is_adjacent(bm, l_a, l_b))
            {
                cur = Some((f, l_a, l_b));
            }
        }
    }
    cur
}

/// `BM_loop_is_adjacent`.
fn loop_is_adjacent(bm: &BMesh, l_a: LoopId, l_b: LoopId) -> bool {
    bm.lp(l_a).next == l_b || bm.lp(l_a).prev == l_b
}

/// `connect_smallest_face`: splits the smallest face using both vertices; returns the new
/// edge and the new face.
fn connect_smallest_face(bm: &mut BMesh, v_a: VertId, v_b: VertId) -> (Option<EdgeId>, Option<FaceId>) {
    let Some((f, l_a, l_b)) = vert_pair_share_face_by_len(bm, v_a, v_b, true) else {
        return (None, None);
    };
    match bm.face_split(f, l_a, l_b, None, false) {
        Some((f_new, l_new)) => (Some(bm.lp(l_new).e), Some(f_new)),
        None => (None, None),
    }
}

/// `interp_slerp_co_no_v3`: specialised slerp over the sphere defined by both normals.
fn interp_slerp_co_no_v3(co_a: V3, no_a: V3, co_b: V3, no_b: V3, no_dir: V3, fac: f32) -> V3 {
    let mut center = None;
    let no_mid = math::normalize_v3(math::add(no_a, no_b)).0;
    let (no_ortho, len) = math::normalize_v3(math::cross(no_mid, no_dir));
    if len != 0.0 {
        let v_a_no_ortho = math::project_v3_plane(math::cross(no_ortho, no_a), no_ortho, [0.0; 3]);
        let v_b_no_ortho = math::project_v3_plane(math::cross(no_ortho, no_b), no_ortho, [0.0; 3]);
        let plane_a = math::plane_from_point_normal_v3(co_a, v_a_no_ortho);
        let plane_b = math::plane_from_point_normal_v3(co_b, v_b_no_ortho);
        let plane_c = math::plane_from_point_normal_v3(co_b, no_ortho);
        center = math::isect_plane_plane_plane_v3(plane_a, plane_b, plane_c);
    }
    let center = center.unwrap_or_else(|| math::mid_v3_v3v3(co_a, co_b));
    let (ofs_a, dist_a) = math::normalize_v3(math::sub(co_a, center));
    let (ofs_b, dist_b) = math::normalize_v3(math::sub(co_b, center));
    match math::interp_v3_v3v3_slerp(ofs_a, ofs_b, fac) {
        Some(ofs_slerp) => math::madd(center, ofs_slerp, math::interpf(dist_b, dist_a, fac)),
        None => math::interp_v3_v3v3(co_a, co_b, fac),
    }
}

/// `alter_co`: the smooth, sphere or fractal position of `v`, written to the temporary shape
/// layer (and the difference applied to the other shape layers).
fn alter_co(bm: &mut BMesh, v: VertId, params: &Params, perc: f32, v_a: VSnap, v_b: VSnap) {
    let v_co = bm.vert(v).co;
    let mut co = v_co;
    if params.use_sphere {
        co = math::normalize_v3_length(co, params.smooth).0;
    } else if params.use_smooth {
        // USE_SPHERE_DUAL_BLEND: calculating twice and blending gives smoother results.
        let eps_unit_vec = 1e-5f32;
        let no_dir = math::normalize_v3(math::sub(v_a.co, v_b.co)).0;

        // Sphere-a.
        let no_reflect = math::reflect_v3_v3v3(v_a.no, no_dir);
        let co_a = if math::len_squared_v3(math::sub(no_reflect, v_a.no)) < eps_unit_vec {
            math::interp_v3_v3v3(v_a.co, v_b.co, perc)
        } else {
            interp_slerp_co_no_v3(v_a.co, v_a.no, v_b.co, no_reflect, no_dir, perc)
        };
        // Sphere-b.
        let no_reflect = math::reflect_v3_v3v3(v_b.no, no_dir);
        let co_b = if math::len_squared_v3(math::sub(no_reflect, v_b.no)) < eps_unit_vec {
            math::interp_v3_v3v3(v_a.co, v_b.co, perc)
        } else {
            interp_slerp_co_no_v3(v_a.co, no_reflect, v_b.co, v_b.no, no_dir, perc)
        };
        // Blend both spheres.
        co = math::interp_v3_v3v3(co_a, co_b, perc);

        // Apply falloff.
        let mut smooth = if params.smooth_falloff == Falloff::Linear {
            1.0
        } else {
            let s = (1.0 - 2.0 * (0.5 - perc).abs()).abs();
            1.0 + params.smooth_falloff.calc(s)
        };
        if params.use_smooth_even {
            smooth *= math::shell_v3v3_mid_normalized_to_dist(v_a.no, v_b.no);
        }
        smooth *= params.smooth;
        if smooth != 1.0 {
            let co_flat = math::interp_v3_v3v3(v_a.co, v_b.co, perc);
            co = math::interp_v3_v3v3(co_flat, co, smooth);
        }
    }

    if params.use_fractal {
        let len = math::len_v3v3(v_a.co, v_b.co);
        let fac = params.fractal * len;
        let normal = math::mid_v3_v3v3(v_a.no, v_b.no);
        let (base1, base2) = frap_geom::polyfill::ortho_basis_v3v3_v3(normal);
        let co2 = math::scale(math::add(v_co, params.fractal_ofs), 10.0);
        let tvec = [
            fac * (noise::turbulence_new_perlin(1.0, co2[0], co2[1], co2[2], 15, false) - 0.5),
            fac * (noise::turbulence_new_perlin(1.0, co2[1], co2[0], co2[2], 15, false) - 0.5),
            fac * (noise::turbulence_new_perlin(1.0, co2[1], co2[2], co2[0], 15, false) - 0.5),
        ];
        co = math::madd(co, normal, tvec[0]);
        co = math::madd(co, base1, tvec[1] * (1.0 - params.along_normal));
        co = math::madd(co, base2, tvec[2] * (1.0 - params.along_normal));
    }
    bm.set_tmp_co(params, v, co);

    // Apply the new difference to the rest of the shape keys. Blender's loop starts at the
    // first layer, steps before each write and so writes layers 1..totlayer (the temporary
    // layer included, the first layer skipped).
    if params.totlayer > 1 {
        let tvec = math::sub(v_co, co);
        for li in 1..params.totlayer {
            let col = &mut bm.shape_layers[li].co;
            if col.len() <= v.slot() {
                col.resize(v.slot() + 1, [0.0; 3]);
            }
            col[v.slot()] = math::sub(col[v.slot()], tvec);
        }
    }
}

/// `bm_subdivide_edge_addvert`: splits `edge` at its first vertex and places the new vertex.
#[allow(clippy::too_many_arguments)]
fn subdivide_edge_addvert(
    bm: &mut BMesh,
    edge: EdgeId,
    params: &Params,
    factor_edge_split: f32,
    factor_subd: f32,
    v_a: VSnap,
    v_b: VSnap,
) -> (VertId, EdgeId) {
    let v1 = bm.edge(edge).v[0];
    let (v_new, e_new) = bm.edge_split(edge, v1, factor_edge_split);
    bm.oflag_enable(v_new, ELE_INNER);
    alter_co(bm, v_new, params, factor_subd, v_a, v_b);
    let no = math::normalize_v3(math::interp_v3_v3v3(v_a.no, v_b.no, factor_subd)).0;
    bm.vert_mut(v_new).no = no;
    (v_new, e_new)
}

/// `subdivide_edge_num`.
fn subdivide_edge_num(
    bm: &mut BMesh,
    edge: EdgeId,
    curpoint: i32,
    totpoint: i32,
    params: &Params,
    v_a: VSnap,
    v_b: VSnap,
) -> (VertId, EdgeId) {
    let (factor_edge_split, factor_subd) = match params.edge_percents.get(&edge) {
        Some(&pct) if bm.oflag_test(edge, EDGE_PERCENT) && totpoint == 1 => (pct, 0.0),
        _ => (
            1.0 / (totpoint + 1 - curpoint) as f32,
            (curpoint + 1) as f32 / (totpoint + 1) as f32,
        ),
    };
    subdivide_edge_addvert(bm, edge, params, factor_edge_split, factor_subd, v_a, v_b)
}

/// `bm_subdivide_multicut`.
fn subdivide_multicut(bm: &mut BMesh, edge: EdgeId, params: &Params) {
    let [v1, v2] = bm.edge(edge).v;
    let (v_a, v_b) = (bm.snap(v1), bm.snap(v2));
    let (v1_tmp, v2_tmp) = (v_a, v_b);
    for i in 0..params.numcuts {
        let (v, e_new) = subdivide_edge_num(bm, edge, i, params.numcuts, params, v_a, v_b);
        bm.oflag_enable(v, SUBD_SPLIT | ELE_SPLIT);
        bm.oflag_enable(edge, SUBD_SPLIT | ELE_SPLIT);
        bm.oflag_enable(e_new, SUBD_SPLIT | ELE_SPLIT);
    }
    alter_co(bm, v1, params, 0.0, v1_tmp, v2_tmp);
    alter_co(bm, v2, params, 1.0, v1_tmp, v2_tmp);
}

// ------------------------------------------------------------------------------- patterns
// The patterns are rotated to match the input geometry; `verts` starts at the first new
// vertex cut, not at the first vertex of the face (the pre-split state).

/// `quad_1edge_split`.
fn quad_1edge_split(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    // If it's odd, the middle face is a quad, otherwise it's a triangle.
    let mut add = 2usize;
    if numcuts.is_multiple_of(2) {
        for i in 0..numcuts {
            if i == numcuts / 2 {
                add -= 1;
            }
            connect_smallest_face(bm, verts[i], verts[numcuts + add]);
        }
    } else {
        for i in 0..numcuts {
            connect_smallest_face(bm, verts[i], verts[numcuts + add]);
            if i == numcuts / 2 {
                add -= 1;
                connect_smallest_face(bm, verts[i], verts[numcuts + add]);
            }
        }
    }
}

/// `quad_2edge_split_path`.
fn quad_2edge_split_path(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    for i in 0..numcuts {
        connect_smallest_face(bm, verts[i], verts[numcuts + (numcuts - i)]);
    }
    connect_smallest_face(bm, verts[numcuts * 2 + 3], verts[numcuts * 2 + 1]);
}

/// `quad_2edge_split_innervert`.
fn quad_2edge_split_innervert(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    let mut v_last = verts[numcuts];
    for i in (0..numcuts).rev() {
        let (e, _) = connect_smallest_face(bm, verts[i], verts[numcuts + (numcuts - i)]);
        // Blender dereferences the new edge unconditionally.
        let Some(e) = e else {
            continue;
        };
        let [ev1, ev2] = bm.edge(e).v;
        let (sa, sb) = (bm.snap(ev1), bm.snap(ev2));
        let (v, _) = subdivide_edge_addvert(bm, e, params, 0.5, 0.5, sa, sb);
        if i != numcuts - 1 {
            connect_smallest_face(bm, v_last, v);
        }
        v_last = v;
    }
    connect_smallest_face(bm, v_last, verts[numcuts * 2 + 2]);
}

/// `quad_2edge_split_fan`.
fn quad_2edge_split_fan(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    for i in 0..numcuts {
        connect_smallest_face(bm, verts[i], verts[numcuts * 2 + 2]);
        connect_smallest_face(bm, verts[numcuts + (numcuts - i)], verts[numcuts * 2 + 2]);
    }
}

/// `quad_3edge_split`.
fn quad_3edge_split(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    let mut add = 0usize;
    for i in 0..numcuts {
        if i == numcuts / 2 {
            if !numcuts.is_multiple_of(2) {
                connect_smallest_face(bm, verts[numcuts - i - 1 + add], verts[i + numcuts + 1]);
            }
            add = numcuts * 2 + 2;
        }
        connect_smallest_face(bm, verts[numcuts - i - 1 + add], verts[i + numcuts + 1]);
    }
    for i in 0..numcuts / 2 + 1 {
        connect_smallest_face(bm, verts[i], verts[(numcuts - i) + numcuts * 2 + 1]);
    }
}

/// `quad_4edge_subdivide`: an inner grid.
fn quad_4edge_subdivide(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    let s = numcuts + 2;
    let mut lines: Vec<Option<VertId>> = vec![None; s * s];
    // Build a 2-dimensional array of every vertex (and all new ones) in the face.
    // First line.
    for i in 0..numcuts + 2 {
        lines[i] = Some(verts[numcuts * 3 + 2 + (numcuts + 1 - i)]);
    }
    // Last line.
    for i in 0..numcuts + 2 {
        lines[(s - 1) * s + i] = Some(verts[numcuts + i]);
    }
    // First and last members of middle lines.
    for i in 0..numcuts {
        let a = i;
        let b = numcuts + 1 + numcuts + 1 + (numcuts - i - 1);
        let (e, f_new) = connect_smallest_face(bm, verts[a], verts[b]);
        let Some(e) = e else {
            continue;
        };
        bm.oflag_enable(e, ELE_INNER);
        if let Some(f) = f_new {
            bm.oflag_enable(f, ELE_INNER);
        }
        let v1 = verts[a];
        let v2 = verts[b];
        lines[(i + 1) * s] = Some(v1);
        lines[(i + 1) * s + s - 1] = Some(v2);
        let (s1, s2) = (bm.snap(v1), bm.snap(v2));
        for a in 0..numcuts {
            let (v, e_new) = subdivide_edge_num(bm, e, a as i32, numcuts as i32, params, s1, s2);
            bm.oflag_enable(e_new, ELE_INNER);
            lines[(i + 1) * s + a + 1] = Some(v);
        }
    }
    for i in 1..numcuts + 2 {
        for j in 1..=numcuts {
            frap_math::budget::check();
            let a = i * s + j;
            let b = (i - 1) * s + j;
            let (Some(la), Some(lb)) = (lines[a], lines[b]) else {
                continue;
            };
            let (e, f_new) = connect_smallest_face(bm, la, lb);
            let Some(e) = e else {
                continue;
            };
            bm.oflag_enable(e, ELE_INNER);
            if let Some(f) = f_new {
                bm.oflag_enable(f, ELE_INNER);
            }
        }
    }
}

/// `tri_1edge_split`.
fn tri_1edge_split(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    for i in 0..numcuts {
        connect_smallest_face(bm, verts[i], verts[numcuts + 1]);
    }
}

/// `tri_3edge_subdivide`: an inner triangle grid.
fn tri_3edge_subdivide(bm: &mut BMesh, verts: &[VertId], params: &Params) {
    let numcuts = params.numcuts as usize;
    // Rows 0..numcuts+1; row i has i + 1 vertices (the last row numcuts + 2).
    let mut lines: Vec<Vec<Option<VertId>>> = vec![Vec::new(); numcuts + 2];
    lines[0] = vec![Some(verts[numcuts * 2 + 1])];
    let mut last = vec![None; numcuts + 2];
    for i in 0..numcuts {
        last[i + 1] = Some(verts[i]);
    }
    last[0] = Some(verts[numcuts * 3 + 2]);
    last[numcuts + 1] = Some(verts[numcuts]);
    lines[numcuts + 1] = last;

    for i in 0..numcuts {
        lines[i + 1] = vec![None; 2 + i];
        let a = numcuts * 2 + 2 + i;
        let b = numcuts + numcuts - i;
        let (e, f_new) = connect_smallest_face(bm, verts[a], verts[b]);
        let Some(e) = e else {
            // `goto cleanup`.
            return;
        };
        bm.oflag_enable(e, ELE_INNER);
        if let Some(f) = f_new {
            bm.oflag_enable(f, ELE_INNER);
        }
        lines[i + 1][0] = Some(verts[a]);
        lines[i + 1][i + 1] = Some(verts[b]);
        let (sa, sb) = (bm.snap(verts[a]), bm.snap(verts[b]));
        for j in 0..i {
            let (v, e_new) = subdivide_edge_num(bm, e, j as i32, i as i32, params, sa, sb);
            lines[i + 1][j + 1] = Some(v);
            bm.oflag_enable(e_new, ELE_INNER);
        }
    }

    for i in 1..=numcuts {
        for j in 0..i {
            frap_math::budget::check();
            if let (Some(p), Some(q)) = (lines[i][j], lines[i + 1][j + 1]) {
                let (e, f_new) = connect_smallest_face(bm, p, q);
                if let Some(e) = e {
                    bm.oflag_enable(e, ELE_INNER);
                }
                if let Some(f) = f_new {
                    bm.oflag_enable(f, ELE_INNER);
                }
            }
            if let (Some(p), Some(q)) = (lines[i][j + 1], lines[i + 1][j + 1]) {
                let (e, f_new) = connect_smallest_face(bm, p, q);
                if let Some(e) = e {
                    bm.oflag_enable(e, ELE_INNER);
                }
                if let Some(f) = f_new {
                    bm.oflag_enable(f, ELE_INNER);
                }
            }
        }
    }
}

fn fill(bm: &mut BMesh, fill: Fill, verts: &[VertId], params: &Params) {
    match fill {
        Fill::Quad1Edge => quad_1edge_split(bm, verts, params),
        Fill::Quad2EdgePath => quad_2edge_split_path(bm, verts, params),
        Fill::Quad2EdgeInnerVert => quad_2edge_split_innervert(bm, verts, params),
        Fill::Quad2EdgeFan => quad_2edge_split_fan(bm, verts, params),
        Fill::Quad3Edge => quad_3edge_split(bm, verts, params),
        Fill::Quad4Edge => quad_4edge_subdivide(bm, verts, params),
        Fill::Tri1Edge => tri_1edge_split(bm, verts, params),
        Fill::Tri3Edge => tri_3edge_subdivide(bm, verts, params),
    }
}

/// `SubDFaceData`.
struct FaceData {
    start: Option<VertId>,
    pat: Option<&'static Pattern>,
    totedgesel: usize,
    face: FaceId,
}

/// Every element carrying any of `flag`, vertices then edges then faces
/// (`BMO_slot_buffer_from_enabled_flag(…, BM_ALL_NOLOOP, flag)`).
fn elems_with_flag(bm: &BMesh, flag: u16) -> Vec<BMElem> {
    let mut out: Vec<BMElem> = Vec::new();
    out.extend(
        bm.vert_ids()
            .into_iter()
            .filter(|&v| bm.oflag_test(v, flag))
            .map(BMElem::Vert),
    );
    out.extend(
        bm.edge_ids()
            .into_iter()
            .filter(|&e| bm.oflag_test(e, flag))
            .map(BMElem::Edge),
    );
    out.extend(
        bm.face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, flag))
            .map(BMElem::Face),
    );
    out
}

fn check_ids(bm: &BMesh, i: &SubdivideEdgesIn) -> Result<(), BmError> {
    for &e in i.edges.iter().chain(i.edge_percents.iter().map(|(e, _)| e)) {
        if !bm.edges.contains(e.0) {
            return Err(BmError::Invalid(format!("edges: no edge {}", e.0)));
        }
    }
    for &(f, p) in &i.custom_patterns {
        if !bm.faces.contains(f.0) {
            return Err(BmError::Invalid(format!("custom_patterns: no face {}", f.0)));
        }
        if usize::try_from(p).map_or(true, |p| p >= PATTERN_TABLE.len()) {
            return Err(BmError::Invalid(format!("custom_patterns: no pattern {p}")));
        }
    }
    Ok(())
}

/// Runs `subdivide_edges` on `bm`.
pub fn subdivide_edges(bm: &mut BMesh, i: &SubdivideEdgesIn) -> Result<SubdivideEdgesOut, BmError> {
    let smooth_falloff = Falloff::from_identifier(i.smooth_falloff).ok_or_else(|| {
        BmError::Invalid(format!("smooth_falloff: unknown '{}'", i.smooth_falloff))
    })?;
    let cornertype = CornerType::from_identifier(i.quad_corner_type).ok_or_else(|| {
        BmError::Invalid(format!("quad_corner_type: unknown '{}'", i.quad_corner_type))
    })?;
    check_ids(bm, i)?;
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| exec(bm, i, smooth_falloff, cornertype)))
}

fn exec(
    bm: &mut BMesh,
    i: &SubdivideEdgesIn,
    smooth_falloff: Falloff,
    cornertype: CornerType,
) -> SubdivideEdgesOut {
    for &e in &i.edges {
        bm.oflag_enable(e, SUBD_SPLIT);
    }
    let numcuts = i.cuts;

    // `patterns[]`: quad single edge, quad corner, tri single edge, quad grid, quad 3 edges,
    // tri grid.
    let patterns: [Option<&'static Pattern>; 6] = [
        i.use_single_edge.then_some(&QUAD_1EDGE),
        match cornertype {
            CornerType::Path => Some(&QUAD_2EDGE_PATH),
            CornerType::InnerVert => Some(&QUAD_2EDGE_INNERVERT),
            CornerType::Fan => Some(&QUAD_2EDGE_FAN),
            CornerType::StraightCut => None,
        },
        i.use_single_edge.then_some(&TRI_1EDGE),
        i.use_grid_fill.then_some(&QUAD_4EDGE),
        Some(&QUAD_3EDGE),
        i.use_grid_fill.then_some(&TRI_3EDGE),
    ];

    // Add a temporary shape-key layer to store displacements on current geometry.
    let mut tmp = ShapeLayer {
        name: "".into(),
        co: vec![[0.0; 3]; bm.verts.capacity()],
    };
    for (s, v) in bm.verts.iter() {
        tmp.co[s as usize] = v.co;
    }
    bm.shape_layers.push(tmp);
    let tmpkey = bm.shape_layers.len() - 1;

    // First go through and tag edges: the input becomes every tagged edge in iteration order.
    let edges_in: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| bm.oflag_test(e, SUBD_SPLIT))
        .collect();

    let mut params = Params {
        numcuts,
        smooth: i.smooth,
        smooth_falloff,
        fractal: i.fractal,
        along_normal: i.along_normal,
        use_smooth: i.smooth != 0.0,
        use_smooth_even: i.use_smooth_even,
        use_sphere: i.use_sphere,
        use_fractal: i.fractal != 0.0,
        fractal_ofs: [0.0; 3],
        edge_percents: i.edge_percents.iter().copied().collect(),
        tmpkey,
        totlayer: bm.shape_layers.len(),
    };
    if params.use_fractal {
        let mut rng = frap_math::rng::BliRng::new_srandom(i.seed as u32);
        params.fractal_ofs[0] = rng.get_float() * 200.0;
        params.fractal_ofs[1] = rng.get_float() * 200.0;
        params.fractal_ofs[2] = rng.get_float() * 200.0;
    }

    let custom: FxHashMap<FaceId, &'static Pattern> = i
        .custom_patterns
        .iter()
        .map(|&(f, p)| (f, PATTERN_TABLE[p as usize]))
        .collect();
    for &f in custom.keys() {
        bm.oflag_enable(f, FACE_CUSTOMFILL);
    }
    for &(e, _) in &i.edge_percents {
        bm.oflag_enable(e, EDGE_PERCENT);
    }

    let mut facedata: Vec<FaceData> = Vec::new();
    for face in bm.face_ids() {
        frap_math::budget::check();
        let flen = bm.face(face).len as usize;
        // Skip non-quads if requested.
        if i.use_only_quads && flen != 4 {
            continue;
        }
        // Figure out which pattern to use.
        let loops = bm.face_loops(face);
        let edges: Vec<EdgeId> = loops.iter().map(|&l| bm.lp(l).e).collect();
        let verts: Vec<VertId> = loops.iter().map(|&l| bm.lp(l).v).collect();
        let (mut e1, mut e2) = (None, None);
        let mut totesel = 0usize;
        for &e in &edges {
            if bm.oflag_test(e, SUBD_SPLIT) {
                if e1.is_none() {
                    e1 = Some(e);
                } else {
                    e2 = Some(e);
                }
                totesel += 1;
            }
        }
        // Make sure the two edges have a valid angle to each other.
        if totesel == 2
            && let (Some(e1), Some(e2)) = (e1, e2)
            && edge_share_vert_check(bm, e1, e2)
        {
            let d = |e: EdgeId| {
                let [a, b] = bm.edge(e).v;
                math::normalize_v3(math::sub(bm.vert(b).co, bm.vert(a).co)).0
            };
            if math::dot(d(e1), d(e2)).abs() > 1.0 - FLT_FACE_SPLIT_EPSILON {
                totesel = 0;
            }
        }

        let sel = |e: EdgeId| bm.oflag_test(e, SUBD_SPLIT);
        if bm.oflag_test(face, FACE_CUSTOMFILL) {
            let pat = custom[&face];
            let len = pat.seledges.len();
            for r in 0..len {
                // Blender indexes the face's edges with the pattern length.
                let matched = (0..len).all(|j| edges.get((j + r) % len).is_some_and(|&e| sel(e)) == pat.seledges[j]);
                if matched {
                    bm.oflag_enable(face, SUBD_SPLIT);
                    facedata.push(FaceData {
                        start: verts.get(r).copied(),
                        pat: Some(pat),
                        totedgesel: totesel,
                        face,
                    });
                    break;
                }
            }
            // Obviously don't test for other patterns matching.
            continue;
        }

        let mut matched = false;
        let mut found: Option<(&'static Pattern, usize)> = None;
        for pat in patterns.iter().flatten() {
            let len = pat.seledges.len();
            if len != flen {
                continue;
            }
            let mut rot = 0;
            for a in 0..len {
                matched = (0..len).all(|b| sel(edges[(b + a) % len]) == pat.seledges[b]);
                if matched {
                    rot = a;
                    break;
                }
            }
            if matched {
                found = Some((pat, rot));
                break;
            }
        }
        if let Some((pat, a)) = found {
            bm.oflag_enable(face, SUBD_SPLIT);
            facedata.push(FaceData {
                start: Some(verts[a]),
                pat: Some(pat),
                totedgesel: totesel,
                face,
            });
        } else if totesel != 0 {
            bm.oflag_enable(face, SUBD_SPLIT);
            facedata.push(FaceData {
                start: None,
                pat: None,
                totedgesel: totesel,
                face,
            });
        }
    }

    // Go through and split edges.
    for &edge in &edges_in {
        frap_math::budget::check();
        subdivide_multicut(bm, edge, &params);
    }

    // Copy original-geometry displacements to current coordinates.
    copy_tmp_to_co(bm, &params);

    // The face data is a stack: the last gathered face is filled first.
    while let Some(fd) = facedata.pop() {
        frap_math::budget::check();
        let face = fd.face;
        if !bm.faces.contains(face.0) {
            continue;
        }
        match fd.pat {
            None if fd.totedgesel == 2 => split_two_edges_no_pattern(bm, face, &params),
            None => {}
            Some(pat) => {
                let loops = bm.face_loops(face);
                let flen = loops.len();
                let mut a = 0usize;
                for (j, &l) in loops.iter().enumerate() {
                    if Some(bm.lp(l).v) == fd.start {
                        a = j + 1;
                        break;
                    }
                }
                let mut verts = vec![VertId(0); flen];
                for (j, &l) in loops.iter().enumerate() {
                    let b = (j + flen - a % flen) % flen;
                    verts[b] = bm.lp(l).v;
                }
                fill(bm, pat.fill, &verts, &params);
            }
        }
    }

    // Copy original-geometry displacements to current coordinates.
    copy_tmp_to_co(bm, &params);
    bm.shape_layers.remove(params.tmpkey);
    params.totlayer -= 1;

    // Vertex creases should not be interpolated when subdividing edges (#154814).
    if let Some(li) = bm.vdata.find("crease_vert") {
        let inner: Vec<VertId> = bm
            .vert_ids()
            .into_iter()
            .filter(|&v| bm.oflag_test(v, ELE_INNER))
            .collect();
        if let frap_mesh::AttrData::Float(col) = &mut bm.vdata.layers[li].data {
            let col = std::sync::Arc::make_mut(col);
            for v in inner {
                if let Some(x) = col.get_mut(v.slot()) {
                    *x = 0.0;
                }
            }
        }
    }

    SubdivideEdgesOut {
        geom_inner: elems_with_flag(bm, ELE_INNER),
        geom_split: elems_with_flag(bm, ELE_SPLIT),
        geom: elems_with_flag(bm, ELE_INNER | ELE_SPLIT | SUBD_SPLIT),
    }
}

/// `BM_edge_share_vert_check`.
fn edge_share_vert_check(bm: &BMesh, e1: EdgeId, e2: EdgeId) -> bool {
    let [a, b] = bm.edge(e1).v;
    let [c, d] = bm.edge(e2).v;
    a == c || a == d || b == c || b == d
}

/// Copies the temporary shape layer to the vertex positions.
fn copy_tmp_to_co(bm: &mut BMesh, params: &Params) {
    for v in bm.vert_ids() {
        let co = bm.tmp_co(params, v);
        bm.vert_mut(v).co = co;
    }
}

/// The no-pattern case of a face with two cut edges: connects the cut vertices of one edge
/// to those of the other (`bmo_subdivide_edges_exec`, "ok, no pattern. we still may be able
/// to do something").
fn split_two_edges_no_pattern(bm: &mut BMesh, face: FaceId, params: &Params) {
    let numcuts = params.numcuts as usize;
    let loops = bm.face_loops(face);
    let vlen = loops.len();
    let inner = |bm: &BMesh, i: usize| bm.oflag_test(bm.lp(loops[i % vlen]).v, ELE_INNER);

    // Find the boundary of one of the split edges.
    let mut a = 0usize;
    while a < vlen {
        if !inner(bm, if a != 0 { a - 1 } else { vlen - 1 }) && inner(bm, a) {
            break;
        }
        a += 1;
    }
    if a >= vlen {
        // Blender asserts here (an internal error).
        return;
    }
    let mut b;
    if inner(bm, (a + numcuts + 1) % vlen) {
        b = (a + numcuts + 1) % vlen;
    } else {
        // Find the boundary of the other edge.
        b = 0;
        for j in 0..vlen {
            b = (j + a + numcuts + 1) % vlen;
            if !inner(bm, if b == 0 { vlen - 1 } else { b - 1 }) && inner(bm, b) {
                break;
            }
        }
    }
    b += numcuts.saturating_sub(1);

    let mut loops_split: Vec<Option<(LoopId, LoopId)>> = Vec::with_capacity(numcuts);
    for _ in 0..numcuts {
        let (la, lb) = (loops[a % vlen], loops[b % vlen]);
        // Special case #32500: the vertex pair shares another face (not as an edge of it);
        // splitting is ambiguous, so the pair is skipped.
        let va = bm.lp(la).v;
        let vb = bm.lp(lb).v;
        let ok = !bm
            .vert_loops(va)
            .into_iter()
            .any(|other| bm.lp(other).f != face && bm.face_vert_share_loop(bm.lp(other).f, vb).is_some());
        loops_split.push(ok.then_some((la, lb)));
        b = (b + vlen - 1) % vlen;
        a = (a + 1) % vlen;
    }

    for (la, lb) in loops_split.into_iter().flatten() {
        if let Some((_, l_new)) = bm.face_split(face, la, lb, None, false) {
            let e = bm.lp(l_new).e;
            bm.oflag_enable(e, ELE_INNER);
        }
    }
}

// ----------------------------------------------------------------------- BM_mesh_esubdivide

/// `SUBDIV_SELECT_*`: how [`mesh_esubdivide`] changes the selection.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SubdivSelect {
    /// `SUBDIV_SELECT_NONE`
    None,
    /// `SUBDIV_SELECT_ORIG`: the new inner elements are selected too, then flushed from the
    /// vertices.
    Orig,
    /// `SUBDIV_SELECT_INNER`: the new inner vertices and edges are selected.
    Inner,
    /// `SUBDIV_SELECT_LOOPCUT`: everything is deselected, then the new inner edges selected.
    Loopcut,
}

/// The arguments of `BM_mesh_esubdivide` (the edges are the selected ones).
#[derive(Clone, Debug)]
pub struct ESubdivide {
    /// `smooth`.
    pub smooth: f32,
    /// `smooth_falloff` (enum identifier).
    pub smooth_falloff: &'static str,
    /// `use_smooth_even`.
    pub use_smooth_even: bool,
    /// `fractal`.
    pub fractal: f32,
    /// `along_normal`.
    pub along_normal: f32,
    /// `numcuts`.
    pub numcuts: i32,
    /// `seltype`.
    pub seltype: SubdivSelect,
    /// `cornertype` (enum identifier).
    pub cornertype: &'static str,
    /// `use_single_edge`.
    pub use_single_edge: bool,
    /// `use_grid_fill`.
    pub use_grid_fill: bool,
    /// `use_only_quads`.
    pub use_only_quads: bool,
    /// `seed`.
    pub seed: i32,
}

// Ported from blender/source/blender/bmesh/operators/bmo_subdivide.cc (BM_mesh_esubdivide)
// with `edge_hflag = BM_ELEM_SELECT`.
/// `BM_mesh_esubdivide(bm, BM_ELEM_SELECT, …)`: subdivides the selected edges and updates the
/// selection for `seltype`.
pub fn mesh_esubdivide(bm: &mut BMesh, a: &ESubdivide) -> Result<SubdivideEdgesOut, BmError> {
    let edges: Vec<EdgeId> = bm
        .edge_ids()
        .into_iter()
        .filter(|&e| {
            let f = bm.edge(e).flag;
            f.contains(ElemFlag::SELECT) && !f.contains(ElemFlag::HIDDEN)
        })
        .collect();
    let out = subdivide_edges(
        bm,
        &SubdivideEdgesIn {
            edges,
            smooth: a.smooth,
            smooth_falloff: a.smooth_falloff,
            use_smooth_even: a.use_smooth_even,
            fractal: a.fractal,
            along_normal: a.along_normal,
            cuts: a.numcuts,
            quad_corner_type: a.cornertype,
            use_single_edge: a.use_single_edge,
            use_grid_fill: a.use_grid_fill,
            use_only_quads: a.use_only_quads,
            seed: a.seed,
            ..SubdivideEdgesIn::default()
        },
    )?;
    let select = |bm: &mut BMesh, e: BMElem| {
        bm.select_set(e, true);
        if bm.elem_flag(e).contains(ElemFlag::HIDDEN) {
            return;
        }
        bm.elem_flag_mut(e).insert(ElemFlag::SELECT);
    };
    match a.seltype {
        SubdivSelect::None => {}
        SubdivSelect::Orig => {
            for &e in &out.geom_inner {
                select(bm, e);
            }
            bm.select_flush();
        }
        SubdivSelect::Inner => {
            for &e in &out.geom_inner {
                if matches!(e, BMElem::Vert(_) | BMElem::Edge(_)) {
                    select(bm, e);
                }
            }
        }
        SubdivSelect::Loopcut => {
            // Deselect input (`BM_mesh_elem_hflag_disable_all(…, BM_ELEM_SELECT, false)`).
            bm.select_history.clear();
            for v in bm.vert_ids() {
                bm.vert_mut(v).flag.remove(ElemFlag::SELECT);
            }
            for e in bm.edge_ids() {
                bm.edge_mut(e).flag.remove(ElemFlag::SELECT);
            }
            for f in bm.face_ids() {
                bm.face_mut(f).flag.remove(ElemFlag::SELECT);
            }
            for &e in &out.geom_inner {
                if matches!(e, BMElem::Edge(_)) {
                    select(bm, e);
                }
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::FromMeshParams;
    use frap_mesh::primitives::cube::{Cube, cube_mesh};
    use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
    use frap_mesh::primitives::grid::{Grid, grid};
    use frap_mesh::primitives::ico_sphere::{IcoSphere, ico_sphere};

    /// The default cube as an edit BMesh (everything selected).
    pub(crate) fn cube() -> BMesh {
        BMesh::from_mesh(&cube_mesh(&Cube::default()), &FromMeshParams::default())
    }
    /// A cylinder with `n` vertices per rim.
    pub(crate) fn cyl(n: i32) -> BMesh {
        BMesh::from_mesh(
            &cylinder(&Cylinder {
                vertices: n,
                ..Cylinder::default()
            }),
            &FromMeshParams::default(),
        )
    }
    /// A grid of `x` × `y` vertices.
    pub(crate) fn grid_bm(x: i32, y: i32) -> BMesh {
        BMesh::from_mesh(
            &grid(&Grid {
                x_subdivisions: x,
                y_subdivisions: y,
                ..Grid::default()
            }),
            &FromMeshParams::default(),
        )
    }
    pub(crate) fn counts(bm: &BMesh) -> (usize, usize, usize) {
        (bm.verts_num(), bm.edges_num(), bm.faces_num())
    }
    pub(crate) fn assert_valid(bm: &BMesh) {
        let e = bm.validate_strict();
        assert!(e.is_empty(), "{e:?}");
    }

    fn all_edges(bm: &mut BMesh, cuts: i32) -> SubdivideEdgesOut {
        let edges = bm.edge_ids();
        subdivide_edges(
            bm,
            &SubdivideEdgesIn {
                edges,
                cuts,
                use_grid_fill: true,
                ..SubdivideEdgesIn::default()
            },
        )
        .unwrap()
    }

    #[test]
    fn whole_cube_one_cut_reverses_face_centres() {
        let mut bm = cube();
        let out = all_edges(&mut bm, 1);
        assert_eq!(counts(&bm), (26, 48, 24));
        assert_valid(&bm);
        // Edge midpoints 8..19 in edge order, then the face centres in reversed face order.
        let co = |i: u32| bm.vert(VertId(i)).co;
        assert_eq!(co(20), [0.0, 0.0, 1.0]);
        assert_eq!(co(21), [0.0, 0.0, -1.0]);
        assert_eq!(co(25), [-1.0, 0.0, 0.0]);
        // 18 new vertices, 36 inner edges (12 split + 24 grid), 18 new faces.
        let n = |k: fn(&BMElem) -> bool| out.geom_inner.iter().filter(|e| k(e)).count();
        assert_eq!(n(|e| matches!(e, BMElem::Vert(_))), 18);
        assert_eq!(n(|e| matches!(e, BMElem::Face(_))), 18);
    }

    #[test]
    fn grids_and_triangles_stay_valid() {
        for cuts in 1..=4 {
            let mut bm = cube();
            all_edges(&mut bm, cuts);
            let c = cuts as usize;
            // Every quad becomes (cuts + 1)^2 quads.
            assert_eq!(bm.faces_num(), 6 * (c + 1) * (c + 1));
            assert_valid(&bm);
            let mut ico = BMesh::from_mesh(
                &ico_sphere(&IcoSphere {
                    subdivisions: 1,
                    ..IcoSphere::default()
                }),
                &FromMeshParams::default(),
            );
            all_edges(&mut ico, cuts);
            assert_eq!(ico.faces_num(), 20 * (c + 1) * (c + 1));
            assert_valid(&ico);
        }
    }

    #[test]
    fn edge_percent_places_a_single_cut() {
        let mut bm = cube();
        let e = bm.edge_ids()[0];
        let [v1, v2] = bm.edge(e).v;
        let (a, b) = (bm.vert(v1).co, bm.vert(v2).co);
        subdivide_edges(
            &mut bm,
            &SubdivideEdgesIn {
                edges: vec![e],
                cuts: 1,
                edge_percents: vec![(e, 0.25)],
                ..SubdivideEdgesIn::default()
            },
        )
        .unwrap();
        let v_new = VertId(8);
        let want = math::interp_v3_v3v3(a, b, 0.25);
        assert_eq!(bm.vert(v_new).co, want);
        // No pattern for one cut edge without `use_single_edge`: the faces gain a corner.
        assert_eq!(counts(&bm), (9, 13, 6));
        assert_valid(&bm);
    }

    #[test]
    fn single_edge_and_custom_patterns() {
        let mut bm = cube();
        let e = bm.edge_ids()[0];
        subdivide_edges(
            &mut bm,
            &SubdivideEdgesIn {
                edges: vec![e],
                cuts: 2,
                use_single_edge: true,
                ..SubdivideEdgesIn::default()
            },
        )
        .unwrap();
        // Both quads of the edge are fanned (quad_1edge with an even number of cuts).
        assert_eq!(counts(&bm), (10, 18, 10));
        assert_valid(&bm);

        let mut bm = cube();
        let f = bm.face_ids()[0];
        let edges = bm.face_edges(f);
        subdivide_edges(
            &mut bm,
            &SubdivideEdgesIn {
                edges,
                cuts: 1,
                custom_patterns: vec![(f, 5)],
                ..SubdivideEdgesIn::default()
            },
        )
        .unwrap();
        // The custom grid pattern fills face 0; its neighbours become 5-gons.
        assert_eq!(counts(&bm), (13, 20, 9));
        assert_valid(&bm);
        let bad = subdivide_edges(
            &mut cube(),
            &SubdivideEdgesIn {
                custom_patterns: vec![(f, 99)],
                ..SubdivideEdgesIn::default()
            },
        );
        assert!(matches!(bad, Err(BmError::Invalid(_))));
    }

    #[test]
    fn unknown_enums_are_invalid() {
        let mut bm = cube();
        for i in [
            SubdivideEdgesIn {
                smooth_falloff: "NOPE",
                ..SubdivideEdgesIn::default()
            },
            SubdivideEdgesIn {
                quad_corner_type: "NOPE",
                ..SubdivideEdgesIn::default()
            },
            SubdivideEdgesIn {
                edges: vec![EdgeId(999)],
                ..SubdivideEdgesIn::default()
            },
        ] {
            assert!(matches!(subdivide_edges(&mut bm, &i), Err(BmError::Invalid(_))));
        }
    }

    #[test]
    fn sphere_and_crease_rules() {
        let mut bm = cube();
        bm.vdata.add_layer(crate::CdLayer {
            name: "crease_vert".into(),
            data: frap_mesh::AttrData::Float(std::sync::Arc::new(vec![1.0; 8])),
        });
        let edges = bm.edge_ids();
        subdivide_edges(
            &mut bm,
            &SubdivideEdgesIn {
                edges,
                cuts: 1,
                smooth: 2.0,
                use_sphere: true,
                use_grid_fill: true,
                ..SubdivideEdgesIn::default()
            },
        )
        .unwrap();
        // `use_sphere` puts every vertex at distance `smooth` from the origin.
        for v in bm.vert_ids() {
            let d = math::len_v3(bm.vert(v).co);
            assert!((d - 2.0).abs() < 1e-5, "{d}");
        }
        // New vertices get no vertex crease; the originals keep theirs.
        let li = bm.vdata.find("crease_vert").unwrap();
        let frap_mesh::AttrData::Float(c) = &bm.vdata.layers[li].data else {
            unreachable!()
        };
        assert_eq!(c[0], 1.0);
        assert_eq!(c[8], 0.0);
        // The temporary shape layer is gone.
        assert!(bm.shape_layers.is_empty());
    }

    #[test]
    fn esubdivide_selection_modes() {
        // Loop cut selection: only the new inner edges.
        let mut bm = cyl(8);
        let ring: Vec<EdgeId> = [2u32, 3, 6, 9, 12, 15, 18, 21].iter().map(|&i| EdgeId(i)).collect();
        for e in bm.edge_ids() {
            bm.edge_select_set(e, false);
        }
        for v in bm.vert_ids() {
            bm.vert_select_set(v, false);
        }
        for f in bm.face_ids() {
            bm.face_mut(f).flag.remove(ElemFlag::SELECT);
        }
        for &e in &ring {
            bm.edge_select_set(e, true);
        }
        let args = ESubdivide {
            smooth: 0.0,
            smooth_falloff: "INVERSE_SQUARE",
            use_smooth_even: true,
            fractal: 0.0,
            along_normal: 0.0,
            numcuts: 1,
            seltype: SubdivSelect::Loopcut,
            cornertype: "PATH",
            use_single_edge: false,
            use_grid_fill: true,
            use_only_quads: false,
            seed: 0,
        };
        mesh_esubdivide(&mut bm, &args).unwrap();
        let sel: Vec<EdgeId> = bm
            .edge_ids()
            .into_iter()
            .filter(|&e| bm.edge(e).flag.contains(ElemFlag::SELECT))
            .collect();
        assert_eq!(sel.len(), 8);
        for e in sel {
            let [a, b] = bm.edge(e).v;
            assert!(bm.vert(a).co[2].abs() < 1e-6 && bm.vert(b).co[2].abs() < 1e-6);
        }
        assert_valid(&bm);

        // Orig selection: the new geometry joins the selection.
        let mut bm = grid_bm(2, 2);
        mesh_esubdivide(
            &mut bm,
            &ESubdivide {
                seltype: SubdivSelect::Orig,
                cornertype: "STRAIGHT_CUT",
                use_smooth_even: false,
                ..args
            },
        )
        .unwrap();
        assert_eq!(bm.selected_counts(), (25, 40, 16));
    }
}

#[cfg(test)]
mod falloff_tests {
    use super::*;

    #[test]
    fn falloff_values() {
        assert_eq!(Falloff::Linear.calc(0.25), 0.25);
        assert_eq!(Falloff::Sharp.calc(0.5), 0.25);
        assert_eq!(Falloff::InverseSquare.calc(0.5), 0.75);
        assert_eq!(Falloff::Smooth.calc(0.5), 0.5);
        assert_eq!(Falloff::Root.calc(0.25), 0.5);
        assert!((Falloff::Sphere.calc(0.5) - 0.75f32.sqrt()).abs() < 1e-7);
        assert_eq!(Falloff::from_identifier("INVERSE_SQUARE"), Some(Falloff::InverseSquare));
        assert_eq!(CornerType::from_identifier("INNERVERT"), Some(CornerType::InnerVert));
        assert_eq!(CornerType::from_identifier("INNER_VERT"), Some(CornerType::InnerVert));
    }
}
