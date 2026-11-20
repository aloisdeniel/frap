// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.bevel` (Blender 5.2.4 `bmo_bevel_exec`, `operators/bmo_bevel.cc`, and
//! `BM_mesh_bevel`, `tools/bmesh_bevel.cc`), implemented by WP 2.11 (Bevel). A port of
//! Blender's bevel: Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc
//! (GPL-2.0-or-later), split into `bevel/{types,math,util,construct,boundary,profile,vmesh,
//! build,uv}.rs`.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.bevel(bmesh, geom=[], offset=0.0, offset_type='OFFSET', profile_type='SUPERELLIPSE', segments=0, profile=0.0, affect='VERTICES', clamp_overlap=False, material=0, loop_slide=False, mark_seam=False, mark_sharp=False, harden_normals=False, face_strength_mode='NONE', miter_outer='SHARP', miter_inner='SHARP', spread=0.0, custom_profile=None, vmesh_method='ADJ')`.
//!
//! **Create/kill order** (`BM_mesh_bevel`): nothing happens unless `offset > 0` (so also for NaN). Otherwise
//! the input vertices (and both vertices of every manifold input edge) are tagged; then, in
//! vertex iteration order, each tagged vertex gets its boundary (no elements created). Then,
//! per beveled vertex in iteration order (`build_vmesh`): the boundary BMVerts `(i, 0, 0)`
//! in boundary order, the profile BMVerts, then the vertex-mesh BMVerts and faces of its
//! pattern (poly, tri-fan, ADJ rings + center ngon, or cutoff). Then, per input edge in
//! iteration order, the `segments` edge faces from the `v1` end. Then, per beveled vertex,
//! every face around it is rebuilt (a new face per original face, in `BM_FACES_OF_VERT`
//! order) and wires are reattached. Every new face creates its missing edges in
//! `(v[n-1], v[0]), (v[0], v[1]), …` order. Finally the original faces are killed in the order
//! they were rebuilt, then the beveled vertices (with their edges) in iteration order. New
//! elements take freed slots first (LIFO), then append.
//!
//! Outputs: `faces.out` (the new bevel faces, not the rebuilt ones), `edges.out` and
//! `verts.out` (the new edges/vertices of bevel faces, plus elements that copied a tag), in
//! iteration order. Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.
//!
//! Not supported: `profile_type='CUSTOM'` (a CurveProfile; [`bevel`] answers
//! [`BmError::Invalid`]). `harden_normals` writes the `custom_normal` layer as Blender does
//! (`bevel_harden_normals`, ported in the M5+M6 W0 as an M3+M4 leftover).

mod boundary;
mod build;
mod construct;
mod math;
mod profile;
mod types;
mod util;
mod uv;
mod vmesh;

use rustc_hash::{FxHashMap, FxHashSet};

use self::types::{Affect, Bevel, FaceStrength, Miter, OffsetType, VmeshMethod};
use self::types::{PRO_CIRCLE_R, PRO_LINE_R, PRO_SQUARE_IN_R, PRO_SQUARE_R};
use crate::types::{EdgeId, ElemFlag, FaceId, OpTypeFlag, VertId};
use crate::{BMElem, BMesh, BmError};

/// Input slots of `bevel` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, PartialEq)]
pub struct BevelIn {
    /// `geom`.
    pub geom: Vec<BMElem>,
    /// `offset`.
    pub offset: f32,
    /// `offset_type` (enum identifier).
    pub offset_type: &'static str,
    /// `profile_type` (enum identifier).
    pub profile_type: &'static str,
    /// `segments`.
    pub segments: i32,
    /// `profile`.
    pub profile: f32,
    /// `affect` (enum identifier).
    pub affect: &'static str,
    /// `clamp_overlap`.
    pub clamp_overlap: bool,
    /// `material`.
    pub material: i32,
    /// `loop_slide`.
    pub loop_slide: bool,
    /// `mark_seam`.
    pub mark_seam: bool,
    /// `mark_sharp`.
    pub mark_sharp: bool,
    /// `harden_normals`.
    pub harden_normals: bool,
    /// `face_strength_mode` (enum identifier).
    pub face_strength_mode: &'static str,
    /// `miter_outer` (enum identifier).
    pub miter_outer: &'static str,
    /// `miter_inner` (enum identifier).
    pub miter_inner: &'static str,
    /// `spread`.
    pub spread: f32,
    /// `custom_profile` (custom profile curve (unsupported)).
    pub custom_profile: Option<()>,
    /// `vmesh_method` (enum identifier).
    pub vmesh_method: &'static str,
}

impl Default for BevelIn {
    fn default() -> Self {
        BevelIn {
            geom: Vec::new(),
            offset: 0.0,
            offset_type: "OFFSET",
            profile_type: "SUPERELLIPSE",
            segments: 0,
            profile: 0.0,
            affect: "VERTICES",
            clamp_overlap: false,
            material: 0,
            loop_slide: false,
            mark_seam: false,
            mark_sharp: false,
            harden_normals: false,
            face_strength_mode: "NONE",
            miter_outer: "SHARP",
            miter_inner: "SHARP",
            spread: 0.0,
            custom_profile: None,
            vmesh_method: "ADJ",
        }
    }
}

/// Output slots of `bevel`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct BevelOut {
    /// Every element of the output slots: `verts.out`, then `edges.out`, then `faces.out`.
    pub geom: Vec<BMElem>,
    /// `faces.out`: the new bevel faces, in iteration order.
    pub faces: Vec<FaceId>,
    /// `edges.out`.
    pub edges: Vec<EdgeId>,
    /// `verts.out`.
    pub verts: Vec<VertId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_bevel_def` (`UNTAN_MULTIRES` has no effect in frap).
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

/// Parsed enum slots.
struct Enums {
    offset_type: OffsetType,
    affect: Affect,
    face_strength: FaceStrength,
    miter_outer: Miter,
    miter_inner: Miter,
    vmesh_method: VmeshMethod,
}

fn bad(slot: &str, value: &str, items: &str) -> BmError {
    BmError::Invalid(format!(
        "bevel: unknown {slot} '{value}' (expected {items})"
    ))
}

fn parse(i: &BevelIn) -> Result<Enums, BmError> {
    let offset_type = match i.offset_type {
        "OFFSET" => OffsetType::Offset,
        "WIDTH" => OffsetType::Width,
        "DEPTH" => OffsetType::Depth,
        "PERCENT" => OffsetType::Percent,
        "ABSOLUTE" => OffsetType::Absolute,
        o => {
            return Err(bad(
                "offset_type",
                o,
                "OFFSET, WIDTH, DEPTH, PERCENT or ABSOLUTE",
            ));
        }
    };
    match i.profile_type {
        "SUPERELLIPSE" => {}
        "CUSTOM" => {
            return Err(BmError::Invalid(
                "bevel: profile_type='CUSTOM' (a custom profile curve) is not supported; use \
                 'SUPERELLIPSE' with profile"
                    .into(),
            ));
        }
        o => return Err(bad("profile_type", o, "SUPERELLIPSE or CUSTOM")),
    }
    let affect = match i.affect {
        "VERTICES" => Affect::Vertices,
        "EDGES" => Affect::Edges,
        o => return Err(bad("affect", o, "VERTICES or EDGES")),
    };
    let face_strength = match i.face_strength_mode {
        "NONE" => FaceStrength::None,
        "NEW" => FaceStrength::New,
        "AFFECTED" => FaceStrength::Affected,
        "ALL" => FaceStrength::All,
        o => return Err(bad("face_strength_mode", o, "NONE, NEW, AFFECTED or ALL")),
    };
    let miter = |slot: &str, s: &str| match s {
        "SHARP" => Ok(Miter::Sharp),
        "PATCH" => Ok(Miter::Patch),
        "ARC" => Ok(Miter::Arc),
        o => Err(bad(slot, o, "SHARP, PATCH or ARC")),
    };
    let vmesh_method = match i.vmesh_method {
        "ADJ" => VmeshMethod::Adj,
        "CUTOFF" => VmeshMethod::Cutoff,
        o => return Err(bad("vmesh_method", o, "ADJ or CUTOFF")),
    };
    Ok(Enums {
        offset_type,
        affect,
        face_strength,
        miter_outer: miter("miter_outer", i.miter_outer)?,
        miter_inner: miter("miter_inner", i.miter_inner)?,
        vmesh_method,
    })
}

/// Runs `bevel` on `bm`.
pub fn bevel(bm: &mut BMesh, i: &BevelIn) -> Result<BevelOut, BmError> {
    let en = parse(i)?;
    if let Some(dead) = i.geom.iter().find(|&&g| !bm.elem_alive(g)) {
        return Err(BmError::Invalid(format!(
            "bevel: dead element {dead:?} in geom"
        )));
    }
    Ok(bm.bmo_exec(TYPE_FLAG, |bm| bmo_bevel_exec(bm, i, &en)))
}

/// `bmo_bevel_exec`.
fn bmo_bevel_exec(bm: &mut BMesh, i: &BevelIn, en: &Enums) -> BevelOut {
    let mut out = BevelOut::default();
    // `if (offset > 0)`: a NaN offset bevels nothing either.
    if i.offset.is_nan() || i.offset <= 0.0 {
        return out;
    }
    // First flush 'geom' into flags; faces are cleared so new faces can be collected.
    for v in bm.verts.slots.iter_mut().flatten() {
        v.flag.remove(ElemFlag::TAG);
    }
    for e in bm.edges.slots.iter_mut().flatten() {
        e.flag.remove(ElemFlag::TAG);
    }
    for f in bm.faces.slots.iter_mut().flatten() {
        f.flag.remove(ElemFlag::TAG);
    }
    for &g in &i.geom {
        if let BMElem::Vert(v) = g {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
    }
    for &g in &i.geom {
        if let BMElem::Edge(e) = g
            && bm.edge_is_manifold(e)
        {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
            // In case the verts were not also in the geom.
            let [a, b] = bm.edge(e).v;
            bm.vert_mut(a).flag.insert(ElemFlag::TAG);
            bm.vert_mut(b).flag.insert(ElemFlag::TAG);
        }
    }

    let weights = MeshBevelWeights {
        use_weights: false,
        bweight_vert: bm.vdata.find("bevel_weight_vert"),
        bweight_edge: bm.edata.find("bevel_weight_edge"),
        vertex_group_weights: None,
    };
    mesh_bevel(bm, i, en, i.clamp_overlap, weights);

    for f in bm.face_ids() {
        if bm.face(f).flag.contains(ElemFlag::TAG) {
            out.faces.push(f);
        }
    }
    for e in bm.edge_ids() {
        if bm.edge(e).flag.contains(ElemFlag::TAG) {
            out.edges.push(e);
        }
    }
    for v in bm.vert_ids() {
        if bm.vert(v).flag.contains(ElemFlag::TAG) {
            out.verts.push(v);
        }
    }
    out.geom = out
        .verts
        .iter()
        .map(|&v| BMElem::Vert(v))
        .chain(out.edges.iter().map(|&e| BMElem::Edge(e)))
        .chain(out.faces.iter().map(|&f| BMElem::Face(f)))
        .collect();
    out
}

/// The weight arguments of `BM_mesh_bevel` (`use_weights`, `dvert`/`vertex_group`,
/// `bweight_offset_vert`/`bweight_offset_edge`): what the BEVEL modifier passes besides the
/// `bmesh.ops.bevel` slots (WP 4.5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshBevelWeights {
    /// `use_weights`: scale the offsets by the bevel weight attributes.
    pub use_weights: bool,
    /// Position in `bm.vdata` of the FLOAT vertex bevel-weight layer (`bweight_offset_vert`).
    pub bweight_vert: Option<usize>,
    /// Position in `bm.edata` of the FLOAT edge bevel-weight layer (`bweight_offset_edge`).
    pub bweight_edge: Option<usize>,
    /// `dvert` + `vertex_group`: the group weight of every vertex by slot; it scales the
    /// offset of a vertex bevel (and takes precedence over `use_weights` there).
    pub vertex_group_weights: Option<Vec<f32>>,
}

// Ported from blender/source/blender/bmesh/tools/bmesh_bevel.cc (BM_mesh_bevel).
/// `BM_mesh_bevel` on the vertices and edges the caller tagged `BM_ELEM_TAG` (the BEVEL
/// modifier tags beveled edges together with both their vertices), as the modifier calls it:
/// `i.geom` and `i.clamp_overlap` are ignored, `limit_offset` is the clamp. `profile_type='CUSTOM'`
/// answers [`BmError::Invalid`].
pub fn mesh_bevel_tagged(
    bm: &mut BMesh,
    i: &BevelIn,
    limit_offset: bool,
    weights: MeshBevelWeights,
) -> Result<(), BmError> {
    let en = parse(i)?;
    mesh_bevel(bm, i, &en, limit_offset, weights);
    Ok(())
}

/// `BM_mesh_bevel` with the given clamp and weights (`bmo_bevel_exec` passes no bevel weights,
/// no vertex group and `limit_offset = clamp_overlap`).
fn mesh_bevel(
    bm: &mut BMesh,
    i: &BevelIn,
    en: &Enums,
    limit_offset: bool,
    weights: MeshBevelWeights,
) {
    let harden_normals = i.harden_normals;
    let seg = i.segments.max(1);
    let profile = i.profile;
    let mut bp = Bevel {
        vert_hash: FxHashMap::default(),
        face_hash: FxHashMap::default(),
        uv_face_hash: FxHashMap::default(),
        uv_vert_maps: Vec::new(),
        pro_spacing: Default::default(),
        pro_spacing_miter: Default::default(),
        math_layer_info: Default::default(),
        offset: i.offset,
        offset_type: en.offset_type,
        affect_type: en.affect,
        affect_vertices_odd: en.affect == Affect::Vertices && seg % 2 == 1,
        seg,
        profile,
        // Convert to the superellipse exponent.
        pro_super_r: -libm::logf(2.0) / libm::logf(profile.sqrt()),
        use_weights: weights.use_weights,
        bweight_vert: weights.bweight_vert,
        bweight_edge: weights.bweight_edge,
        vertex_group_weights: weights.vertex_group_weights,
        loop_slide: i.loop_slide,
        limit_offset,
        offset_adjust: en.affect != Affect::Vertices
            && !matches!(en.offset_type, OffsetType::Percent | OffsetType::Absolute),
        mark_seam: i.mark_seam,
        mark_sharp: i.mark_sharp,
        mat_nr: i.material,
        face_strength_mode: en.face_strength,
        miter_outer: en.miter_outer,
        miter_inner: en.miter_inner,
        vmesh_method: en.vmesh_method,
        spread: i.spread,
        ehs: Vec::new(),
        bnds: Vec::new(),
        vms: Vec::new(),
        bevs: Vec::new(),
        edge_tag: Vec::new(),
    };
    // `if (bp.offset > 0)` in Blender: a NaN offset bevels nothing too.
    if bp.offset.is_nan() || bp.offset <= 0.0 {
        return;
    }
    // Disable the miters with the cutoff vertex mesh method.
    if bp.vmesh_method == VmeshMethod::Cutoff {
        bp.miter_outer = Miter::Sharp;
        bp.miter_inner = Miter::Sharp;
    }
    if profile >= 0.950 {
        // r ~ 692, so PRO_SQUARE_R is 1e4.
        bp.pro_super_r = PRO_SQUARE_R;
    } else if f64::from((bp.pro_super_r - PRO_CIRCLE_R).abs()) < 1e-4 {
        bp.pro_super_r = PRO_CIRCLE_R;
    } else if f64::from((bp.pro_super_r - PRO_LINE_R).abs()) < 1e-4 {
        bp.pro_super_r = PRO_LINE_R;
    } else if f64::from(bp.pro_super_r) < 1e-4 {
        bp.pro_super_r = PRO_SQUARE_IN_R;
    }

    // The 2D profile point locations of the superellipse.
    bp.set_profile_spacing();
    // The 'fullness' of the profile for the ADJ vertex mesh method.
    if bp.seg > 1 {
        bp.pro_spacing.fullness = bp.find_profile_fullness();
    }

    bp.math_layer_info_init(bm);
    bp.uv_vert_map_init(bm);

    // Analyze input vertices, sorting edges and assigning initial new vertex positions.
    let verts = bm.vert_ids();
    for &v in &verts {
        frap_math::budget::check();
        if bm.vert(v).flag.contains(ElemFlag::TAG)
            && let Some(bv) = bp.bevel_vert_construct(bm, v)
            && !bp.limit_offset
        {
            bp.build_boundary(bm, bv, true);
            bp.determine_uv_vert_connectivity(bm, v);
        }
    }

    // Perhaps clamp the offset to avoid geometry collisions.
    if bp.limit_offset {
        bp.bevel_limit_offset(bm);
        // Assign initial new vertex positions.
        for &v in &verts {
            if bm.vert(v).flag.contains(ElemFlag::TAG)
                && let Some(bv) = bp.find_bevvert(v)
            {
                bp.build_boundary(bm, bv, true);
                bp.determine_uv_vert_connectivity(bm, v);
            }
        }
    }

    // Perhaps do a pass to try to even out widths.
    if bp.offset_adjust {
        bp.adjust_offsets(bm);
    }

    // Build the meshes around vertices, now that positions are final.
    for &v in &verts {
        frap_math::budget::check();
        if bm.vert(v).flag.contains(ElemFlag::TAG)
            && let Some(bv) = bp.find_bevvert(v)
        {
            bp.build_vmesh(bm, bv);
        }
    }

    // Build polygons for edges.
    if bp.affect_type != Affect::Vertices {
        for e in bm.edge_ids() {
            frap_math::budget::check();
            if bm.edges.contains(e.0) && bm.edge(e).flag.contains(ElemFlag::TAG) {
                bp.bevel_build_edge_polygons(bm, e);
            }
        }
    }

    // Extend edge data like sharp edges.
    for &v in &verts {
        if bm.vert(v).flag.contains(ElemFlag::TAG)
            && let Some(bv) = bp.find_bevvert(v)
        {
            bp.bevel_extend_edge_data(bm, bv);
        }
    }

    // Rebuild face polygons around affected vertices.
    let mut rebuilt: Vec<FaceId> = Vec::new();
    let mut rebuilt_set: FxHashSet<FaceId> = FxHashSet::default();
    for &v in &verts {
        frap_math::budget::check();
        if bm.vert(v).flag.contains(ElemFlag::TAG) {
            bp.bevel_rebuild_existing_polygons(bm, v, &mut rebuilt, &mut rebuilt_set);
            bp.bevel_reattach_wires(bm, v);
        }
    }
    // Blender kills these in the hash-set order of their pointers; frap in rebuild order.
    for f in rebuilt {
        bm.face_kill(f);
    }
    for &v in &verts {
        if bm.verts.contains(v.0) && bm.vert(v).flag.contains(ElemFlag::TAG) {
            debug_assert!(bp.find_bevvert(v).is_some());
            bp.uv_vert_map_pop(v);
            bm.vert_kill(v);
        }
    }

    bp.bevel_merge_uvs(bm);

    if harden_normals {
        bp.bevel_harden_normals(bm);
    }
    if bp.face_strength_mode != FaceStrength::None {
        bp.bevel_set_weighted_normal_face_strength(bm);
    }

    // Transfer the tool flags to BM_ELEM_TAG.
    for v in bm.vert_ids() {
        if bm.oflag_test(v, types::VERT_OUT) {
            bm.vert_mut(v).flag.insert(ElemFlag::TAG);
        }
    }
    for e in bm.edge_ids() {
        if bm.oflag_test(e, types::EDGE_OUT) {
            bm.edge_mut(e).flag.insert(ElemFlag::TAG);
        }
    }
    // Clear the long tags, set only on some loops of edge faces.
    bp.clear_long_tags(bm);
}

impl Bevel {
    /// `geometry_collide_offset`: the offset at which the clone of edge `eb` collapses (or
    /// slides into a neighbour).
    fn geometry_collide_offset(&self, bm: &BMesh, eb: types::Eh) -> f32 {
        use self::math as m;
        use self::util::{co, edge_calc_length};
        let no_collide_offset = self.offset + 1e6;
        let mut limit = no_collide_offset;
        if self.offset == 0.0 {
            return no_collide_offset;
        }
        let mut kb = self.eh(eb).offset_l_spec;
        let ea = self.enext(eb); // NOTE: this is in direction b --> a.
        let ka = self.eh(ea).offset_r_spec;
        let [e1, e2] = bm.edge(self.ehe(eb)).v;
        let (vb, mut vc) = if self.eh(eb).is_rev {
            (e2, e1)
        } else {
            (e1, e2)
        };
        let [a1, a2] = bm.edge(self.ehe(ea)).v;
        let va = if self.eh(ea).is_rev { a1 } else { a2 };
        let (ebother, bvc) = self.find_other_end_edge_half(bm, eb);
        if matches!(self.offset_type, OffsetType::Percent | OffsetType::Absolute) {
            if self.eh(ea).is_bev && ebother.is_some_and(|o| self.eh(self.eprev(o)).is_bev) {
                if self.offset_type == OffsetType::Percent {
                    return 50.0;
                }
                // This is only right sometimes.
                let blen = edge_calc_length(bm, self.ehe(eb));
                return if self.offset > blen / 2.0 {
                    blen / 2.0
                } else {
                    blen
                };
            }
            return no_collide_offset;
        }
        let ec;
        let vd;
        let kc;
        if let Some(ebother) = ebother {
            let e = self.eprev(ebother); // NOTE: this is in direction c --> d.
            ec = Some(e);
            vc = self.bev(bvc.expect("bevvert")).v;
            kc = self.eh(e).offset_l_spec;
            let [c1, c2] = bm.edge(self.ehe(e)).v;
            vd = if self.eh(e).is_rev { c1 } else { c2 };
        } else {
            // No bevvert for c, so C can't be beveled.
            kc = 0.0;
            ec = None;
            // An edge from c with the same face.
            let Some(fnext) = self.eh(eb).fnext else {
                return no_collide_offset;
            };
            let Some(lb) = bm.face_edge_share_loop(fnext, self.ehe(eb)) else {
                return no_collide_offset;
            };
            let lbn = bm.lp(lb).next;
            if bm.lp(lbn).v == vc {
                vd = bm.lp(bm.lp(lbn).next).v;
            } else if bm.lp(lb).v == vc {
                vd = bm.lp(bm.lp(lb).prev).v;
            } else {
                return no_collide_offset;
            }
        }
        if self.ehe(ea) == self.ehe(eb) || ec.is_some_and(|c| self.ehe(c) == self.ehe(eb)) {
            return no_collide_offset;
        }
        let th1 = m::angle_v3v3v3(co(bm, va), co(bm, vb), co(bm, vc));
        let th2 = m::angle_v3v3v3(co(bm, vb), co(bm, vc), co(bm, vd));
        // The offset at which edge B collapses: advancing clones of A, B, C all meet.
        let sin1 = libm::sinf(th1);
        let sin2 = libm::sinf(th2);
        let cos1 = libm::cosf(th1);
        let cos2 = libm::cosf(th2);
        let mut offsets_projected_on_b =
            m::safe_divide(ka + cos1 * kb, sin1) + m::safe_divide(kc + cos2 * kb, sin2);
        if offsets_projected_on_b > types::BEVEL_EPSILON {
            offsets_projected_on_b =
                self.offset * (m::len_v3v3(co(bm, vb), co(bm, vc)) / offsets_projected_on_b);
            if offsets_projected_on_b > types::BEVEL_EPSILON {
                limit = offsets_projected_on_b;
            }
        }
        // Edge slide cases: side edges in line with B and not beveled.
        if kb > f32::EPSILON && (ka == 0.0 || kc == 0.0) {
            // Use bevel weight offsets and not the full offset where weights are used.
            kb = self.offset / kb;
            let fnext = self.eh(eb).fnext;
            if ka == 0.0
                && let Some(mut la) = fnext.and_then(|f| bm.face_edge_share_loop(f, self.ehe(ea)))
            {
                let mut a_side_slide = 0.0f32;
                let mut exterior_angle = 0.0f32;
                let mut first = true;
                while exterior_angle < 0.0001 {
                    if first {
                        exterior_angle = m::PI_F - th1;
                        first = false;
                    } else {
                        la = bm.lp(la).prev;
                        let ln = bm.lp(la).next;
                        let lnn = bm.lp(ln).next;
                        exterior_angle += m::PI_F
                            - m::angle_v3v3v3(
                                co(bm, bm.lp(la).v),
                                co(bm, bm.lp(ln).v),
                                co(bm, bm.lp(lnn).v),
                            );
                    }
                    a_side_slide += edge_calc_length(bm, bm.lp(la).e) * libm::sinf(exterior_angle);
                }
                limit = (a_side_slide * kb).min(limit);
            }
            if kc == 0.0
                && let Some(lc0) = fnext.and_then(|f| bm.face_edge_share_loop(f, self.ehe(eb)))
            {
                let mut lc = bm.lp(lc0).next;
                let mut c_side_slide = 0.0f32;
                let mut exterior_angle = 0.0f32;
                let mut first = true;
                while exterior_angle < 0.0001 {
                    if first {
                        exterior_angle = m::PI_F - th2;
                        first = false;
                    } else {
                        lc = bm.lp(lc).next;
                        let lp = bm.lp(lc).prev;
                        let ln = bm.lp(lc).next;
                        exterior_angle += m::PI_F
                            - m::angle_v3v3v3(
                                co(bm, bm.lp(lp).v),
                                co(bm, bm.lp(lc).v),
                                co(bm, bm.lp(ln).v),
                            );
                    }
                    c_side_slide += edge_calc_length(bm, bm.lp(lc).e) * libm::sinf(exterior_angle);
                }
                limit = (c_side_slide * kb).min(limit);
            }
        }
        limit
    }

    /// `vertex_collide_offset`: the offset at which the new vertices sliding along `ea` from
    /// both ends meet.
    fn vertex_collide_offset(&self, bm: &BMesh, ea: types::Eh) -> f32 {
        let no_collide_offset = self.offset + 1e6;
        if self.offset == 0.0 {
            return no_collide_offset;
        }
        let ka = self.eh(ea).offset_l_spec / self.offset;
        let (eb, _) = self.find_other_end_edge_half(bm, ea);
        let kb = eb.map_or(0.0, |eb| self.eh(eb).offset_l_spec / self.offset);
        let kab = ka + kb;
        let la = util::edge_calc_length(bm, self.ehe(ea));
        if kab <= 0.0 {
            return no_collide_offset;
        }
        la / kab
    }

    /// `bevel_limit_offset`: clamp `offset` to the largest one without geometry collisions,
    /// scaling the edge offset specs accordingly.
    fn bevel_limit_offset(&mut self, bm: &BMesh) {
        let mut limited_offset = self.offset;
        let verts = bm.vert_ids();
        for &bmv in &verts {
            if !bm.vert(bmv).flag.contains(ElemFlag::TAG) {
                continue;
            }
            let Some(bv) = self.find_bevvert(bmv) else {
                continue;
            };
            for i in 0..self.bev(bv).edgecount {
                let eh = self.bev(bv).edge(i);
                let collision_offset = if self.affect_type == Affect::Vertices {
                    self.vertex_collide_offset(bm, eh)
                } else {
                    self.geometry_collide_offset(bm, eh)
                };
                limited_offset = collision_offset.min(limited_offset);
            }
        }
        if limited_offset < self.offset {
            // Every offset spec is some number times offset: scale them all.
            let offset_factor = limited_offset / self.offset;
            for &bmv in &verts {
                if !bm.vert(bmv).flag.contains(ElemFlag::TAG) {
                    continue;
                }
                let Some(bv) = self.find_bevvert(bmv) else {
                    continue;
                };
                for i in 0..self.bev(bv).edgecount {
                    let e = self.bev(bv).edge(i);
                    let eh = self.eh_mut(e);
                    eh.offset_l_spec *= offset_factor;
                    eh.offset_r_spec *= offset_factor;
                    eh.offset_l *= offset_factor;
                    eh.offset_r *= offset_factor;
                }
            }
            self.offset = limited_offset;
        }
    }
}

#[cfg(test)]
mod tests {
    use frap_mesh::primitives::cube::{Cube, cube_mesh};

    use super::*;
    use crate::FromMeshParams;

    fn cube() -> BMesh {
        BMesh::from_mesh(&cube_mesh(&Cube::default()), &FromMeshParams::default())
    }

    fn counts(bm: &BMesh) -> (usize, usize, usize) {
        (bm.verts_num(), bm.edges_num(), bm.faces_num())
    }

    fn edge_bevel(bm: &mut BMesh, edges: &[EdgeId], segments: i32) -> BevelOut {
        let mut geom: Vec<BMElem> = Vec::new();
        for &e in edges {
            let [a, b] = bm.edge(e).v;
            geom.extend([BMElem::Vert(a), BMElem::Vert(b)]);
        }
        geom.extend(edges.iter().map(|&e| BMElem::Edge(e)));
        let i = BevelIn {
            geom,
            offset: 0.2,
            segments,
            profile: 0.5,
            affect: "EDGES",
            loop_slide: true,
            material: -1,
            ..BevelIn::default()
        };
        bevel(bm, &i).unwrap()
    }

    #[test]
    fn one_edge_counts() {
        let mut bm = cube();
        let out = edge_bevel(&mut bm, &[EdgeId(0)], 1);
        assert_eq!(counts(&bm), (10, 15, 7));
        assert_eq!(out.faces.len(), 1);
        assert!(bm.validate().is_empty(), "{:?}", bm.validate());
        let mut bm = cube();
        edge_bevel(&mut bm, &[EdgeId(0)], 3);
        assert_eq!(counts(&bm), (14, 21, 9));
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn all_edges_counts() {
        let mut bm = cube();
        let edges = bm.edge_ids();
        edge_bevel(&mut bm, &edges, 1);
        assert_eq!(counts(&bm), (24, 48, 26));
        let mut bm = cube();
        let out = edge_bevel(&mut bm, &edges, 2);
        assert_eq!(counts(&bm), (56, 108, 54));
        // 12 edges x 2 strips + 8 corners x 3 quads.
        assert_eq!(out.faces.len(), 48);
        assert!(bm.validate().is_empty());
    }

    #[test]
    fn vertex_bevel_counts() {
        let mut bm = cube();
        let i = BevelIn {
            geom: vec![BMElem::Vert(VertId(0))],
            offset: 0.2,
            segments: 1,
            profile: 0.5,
            ..BevelIn::default()
        };
        let out = bevel(&mut bm, &i).unwrap();
        assert_eq!(counts(&bm), (10, 15, 7));
        assert_eq!(
            (out.verts.len(), out.edges.len(), out.faces.len()),
            (3, 3, 1)
        );
        // The new vertices are 0.2 from the old corner along each edge.
        for &v in &out.verts {
            let c = bm.vert(v).co;
            let d: f32 = c.iter().map(|x| (x.abs() - 1.0).abs()).sum();
            assert!((d - 0.2).abs() < 1e-6, "{c:?}");
        }
    }

    #[test]
    fn zero_offset_and_errors() {
        let mut bm = cube();
        let before = counts(&bm);
        let zero = BevelIn {
            geom: vec![BMElem::Edge(EdgeId(0))],
            affect: "EDGES",
            ..BevelIn::default()
        };
        let out = bevel(&mut bm, &zero).unwrap();
        assert_eq!(counts(&bm), before);
        assert!(out.geom.is_empty());
        // `if (offset > 0)`: a NaN offset bevels nothing either (no NaN vertices).
        let all: Vec<BMElem> = bm.vert_ids().into_iter().map(BMElem::Vert).collect();
        let co_before: Vec<_> = bm.vert_ids().iter().map(|&v| bm.vert(v).co).collect();
        for offset_type in ["OFFSET", "PERCENT"] {
            let nan = BevelIn {
                geom: all.clone(),
                offset: f32::NAN,
                offset_type,
                segments: 2,
                ..BevelIn::default()
            };
            let out = bevel(&mut bm, &nan).unwrap();
            assert_eq!(counts(&bm), before);
            assert!(out.geom.is_empty());
            let co: Vec<_> = bm.vert_ids().iter().map(|&v| bm.vert(v).co).collect();
            assert_eq!(co, co_before);
        }
        let bad = BevelIn {
            offset: 0.1,
            offset_type: "WIDE",
            ..BevelIn::default()
        };
        assert!(matches!(bevel(&mut bm, &bad), Err(BmError::Invalid(_))));
        let custom = BevelIn {
            offset: 0.1,
            profile_type: "CUSTOM",
            ..BevelIn::default()
        };
        assert!(
            matches!(bevel(&mut bm, &custom), Err(BmError::Invalid(m)) if m.contains("CUSTOM"))
        );
    }

    /// Every vertex coordinate and every UV of a live face corner is finite.
    fn assert_finite(bm: &BMesh, what: &str) {
        for v in bm.vert_ids() {
            let co = bm.vert(v).co;
            assert!(
                co.iter().all(|x| x.is_finite()),
                "{what}: vertex {v:?} at {co:?}"
            );
        }
        for (layer, l) in bm.ldata.layers.iter().enumerate() {
            if !matches!(l.data, frap_mesh::AttrData::Float2(_)) {
                continue;
            }
            for f in bm.face_ids() {
                for lp in bm.face_loops(f) {
                    let uv = bm.ldata.float2(layer, lp.0).expect("uv");
                    assert!(
                        uv.iter().all(|x| x.is_finite()),
                        "{what}: UV {uv:?} at loop {lp:?} of vertex {:?}",
                        bm.lp(lp).v
                    );
                }
            }
        }
    }

    fn all_verts_and_edges(bm: &BMesh) -> Vec<BMElem> {
        let mut geom: Vec<BMElem> = bm.vert_ids().into_iter().map(BMElem::Vert).collect();
        geom.extend(bm.edge_ids().into_iter().map(BMElem::Edge));
        geom
    }

    #[test]
    fn zero_length_snap_edge_gives_the_end_not_nan() {
        // `closest_to_ray_v3`: a zero direction gives the origin (lambda 0), not 0/0.
        let p = [1.0, 2.0, 3.0];
        let a = [0.5, -0.25, 1.0];
        assert_eq!(math::closest_to_line_v3(p, a, a), (a, 0.0));
        assert_eq!(math::closest_to_line_segment_v3(p, a, a), a);
    }

    /// A clamped vertex bevel of Suzanne leaves zero-length edges; beveling all of it again
    /// snaps corners to them for the UV interpolation (Blender: (0.5, 0.074485) at loop 6121,
    /// no NaN UV).
    #[test]
    fn second_bevel_of_a_clamped_suzanne_has_finite_uvs() {
        use frap_mesh::primitives::monkey::{Monkey, monkey};
        for (seg1, second) in [
            (1, ("WIDTH", 0.01f32, 2, "SHARP", "ADJ")),
            (3, ("OFFSET", 0.01, 3, "ARC", "CUTOFF")),
            (3, ("WIDTH", 0.01, 2, "SHARP", "CUTOFF")),
        ] {
            let mut bm = BMesh::from_mesh(&monkey(&Monkey::default()), &FromMeshParams::default());
            let first = BevelIn {
                geom: bm.vert_ids().into_iter().map(BMElem::Vert).collect(),
                offset: 0.05,
                segments: seg1,
                profile: 0.5,
                clamp_overlap: true,
                material: -1,
                ..BevelIn::default()
            };
            bevel(&mut bm, &first).unwrap();
            assert_finite(&bm, "first bevel");
            let (offset_type, offset, segments, miter_outer, vmesh_method) = second;
            let again = BevelIn {
                geom: all_verts_and_edges(&bm),
                offset,
                offset_type,
                segments,
                profile: 0.5,
                affect: "EDGES",
                material: -1,
                miter_outer,
                vmesh_method,
                ..BevelIn::default()
            };
            bevel(&mut bm, &again).unwrap();
            assert!(bm.validate().is_empty(), "{:?}", bm.validate());
            assert_finite(&bm, &format!("second bevel {second:?}"));
        }
    }

    /// Random selections and parameters on several meshes: no panic and a valid BMesh (golden
    /// cases check the geometry against Blender).
    #[test]
    fn random_bevels_keep_the_mesh_valid() {
        use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
        use frap_mesh::primitives::grid::{Grid, grid};
        use frap_mesh::primitives::monkey::{Monkey, monkey};
        let meshes = [
            cube_mesh(&Cube::default()),
            monkey(&Monkey::default()),
            cylinder(&Cylinder::default()),
            grid(&Grid {
                x_subdivisions: 4,
                y_subdivisions: 3,
                ..Grid::default()
            }),
        ];
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut rnd = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        const OFFSET_TYPES: [&str; 5] = ["OFFSET", "WIDTH", "DEPTH", "PERCENT", "ABSOLUTE"];
        const MITERS: [&str; 3] = ["SHARP", "PATCH", "ARC"];
        for iter in 0..120 {
            let me = &meshes[iter % meshes.len()];
            let mut bm = BMesh::from_mesh(me, &FromMeshParams::default());
            let vertices = rnd(4) == 0;
            let mut geom = Vec::new();
            if vertices {
                for v in bm.vert_ids() {
                    if rnd(5) == 0 {
                        geom.push(BMElem::Vert(v));
                    }
                }
            } else {
                for e in bm.edge_ids() {
                    if rnd(4) == 0 {
                        let [a, b] = bm.edge(e).v;
                        geom.extend([BMElem::Vert(a), BMElem::Vert(b), BMElem::Edge(e)]);
                    }
                }
            }
            let offset_type = OFFSET_TYPES[rnd(5) as usize];
            let i = BevelIn {
                geom,
                offset: if offset_type == "PERCENT" {
                    5.0 + rnd(60) as f32
                } else {
                    0.01 + rnd(30) as f32 / 100.0
                },
                offset_type,
                segments: 1 + rnd(6) as i32,
                profile: [0.0, 0.15, 0.5, 0.8, 1.0][rnd(5) as usize],
                affect: if vertices { "VERTICES" } else { "EDGES" },
                clamp_overlap: rnd(2) == 0,
                material: -1,
                loop_slide: rnd(3) != 0,
                mark_seam: rnd(2) == 0,
                mark_sharp: rnd(2) == 0,
                miter_outer: MITERS[rnd(3) as usize],
                miter_inner: ["SHARP", "ARC"][rnd(2) as usize],
                spread: 0.05,
                vmesh_method: ["ADJ", "CUTOFF"][rnd(2) as usize],
                ..BevelIn::default()
            };
            let out = bevel(&mut bm, &i).unwrap_or_else(|e| panic!("iteration {iter}: {e}"));
            let problems = bm.validate();
            assert!(
                problems.is_empty(),
                "iteration {iter} ({i:?}): {problems:?}"
            );
            assert_finite(&bm, &format!("iteration {iter} ({i:?})"));
            // Every output element is alive and tagged as Blender's output slots are.
            for g in out.geom {
                assert!(bm.elem_alive(g) && bm.elem_flag(g).contains(ElemFlag::TAG));
            }
            // A second bevel of everything, in the same BMesh (reused slots, beveled and
            // possibly clamped-to-zero-length geometry).
            if iter % 3 == 0 {
                let again = BevelIn {
                    geom: all_verts_and_edges(&bm),
                    offset: i.offset * 0.25,
                    affect: ["VERTICES", "EDGES"][rnd(2) as usize],
                    segments: 1 + rnd(3) as i32,
                    clamp_overlap: rnd(2) == 0,
                    ..i.clone()
                };
                bevel(&mut bm, &again)
                    .unwrap_or_else(|e| panic!("iteration {iter}, second bevel: {e}"));
                let problems = bm.validate();
                assert!(
                    problems.is_empty(),
                    "iteration {iter} again ({again:?}): {problems:?}"
                );
                assert_finite(&bm, &format!("iteration {iter}, second bevel ({again:?})"));
            }
        }
    }
}
