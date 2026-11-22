// SPDX-License-Identifier: GPL-3.0-or-later
//! The edit-mesh transform engine: one non-interactive transform run the way Blender's
//! `transform_exec` performs it (`T_INPUT_IS_VALUES_FINAL`), over every mesh in Edit Mode
//! (multi-object editing, one container per mesh).
//!
//! Ported from Blender 5.2.4 (GPL-2.0-or-later), `editors/transform/`:
//! `createTransEditVerts`, `VertsToTransData` (`transform_convert_mesh.cc`),
//! `createTransEdge` (`transform_convert_mesh_edge.cc`), `createTransMeshVertCData`
//! (`transform_convert_mesh_vert_cdata.cc`), `init_TransDataContainers`, `set_prop_dist`,
//! `prop_dist_loc_get` (`transform_convert.cc`), `initTransInfo`, `calculateCenter*`,
//! `calculatePropRatio` (`transform_generics.cc`), the constraint callbacks
//! (`transform_constraints.cc`), `ElementRotation_ex`, `ElementResize`
//! (`transform_mode.cc`), `transdata_elem_translate` (`transform_mode_translate.cc`),
//! `transdata_elem_rotate` (`transform_mode_rotate.cc`), `applyResize`
//! (`transform_mode_resize.cc`), `transdata_elem_shrink_fatten`
//! (`transform_mode_shrink_fatten.cc`), `to_sphere_radius_update`,
//! `transdata_elem_to_sphere` (`transform_mode_tosphere.cc`), `transdata_elem_push_pull`
//! (`transform_mode_push_pull.cc`), `ElementMirror`, `applyMirror`
//! (`transform_mode_mirror.cc`) and `transdata_elem_value` (`transform_mode_customdata.cc`);
//! for WP 8.3.5 `apply_shear_value`, `transdata_elem_shear` (`transform_mode_shear.cc`) and
//! `applyNormalRotation`, `initNormalRotation` (`transform_mode_edge_rotate_normal.cc`).
//!
//! World-space semantics follow Blender: the pivot is computed in world space from the
//! selection of every edit mesh, values (`value`, the constraint, the orientation) are world
//! vectors converted to each object's space with the pseudo-inverse of its 3×3 world matrix;
//! `shrink_fatten` and `push_pull` distances and the custom-data values are object-space
//! amounts, exactly as in Blender.

use std::sync::Arc;

use frap_bmesh::{BMesh, CdLayer, EdgeId, ElemFlag, SelectMode, VertId};
use frap_math::BliRng;
use frap_mesh::AttrData;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::transform::world_matrix_in;
use glam::{Mat3, Mat4, Vec3};
use smol_str::SmolStr;

use super::bmq::{self, co, has, vno};
use super::islands::{connectivity_distance, islands_calc};
use super::mathx::{
    create_space_normal, invert_m3, invert_m4_fallback, len, mid, normalize, normalize_length,
    normalize_m3, project, pseudoinverse_m3, reflect,
};
use super::orient::normal_orientation;
use crate::core::constraint::{Constraint, ConstraintSpace};
use crate::core::math::axis_angle_normalized_to_mat3;
use crate::core::orient::{OrientType, Orientation};
use crate::core::pivot::{self, PivotPoint};
use crate::core::proportional::{Falloff, RANDOM_SEED};

/// The transform mode (`TFM_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `TFM_TRANSLATION`
    Translate,
    /// `TFM_ROTATION`
    Rotate,
    /// `TFM_RESIZE`
    Resize,
    /// `TFM_SHRINKFATTEN` (`use_even_offset`)
    ShrinkFatten {
        /// `use_even_offset`
        even: bool,
    },
    /// `TFM_TOSPHERE`
    ToSphere,
    /// `TFM_PUSHPULL`
    PushPull,
    /// `TFM_MIRROR`
    Mirror,
    /// `TFM_EDGE_CREASE`
    EdgeCrease,
    /// `TFM_VERT_CREASE`
    VertCrease,
    /// `TFM_BWEIGHT` (edges)
    BevelWeight,
    /// `TFM_EDGE_SLIDE`
    EdgeSlide,
    /// `TFM_VERT_SLIDE`
    VertSlide,
    /// `TFM_SHEAR` (WP 8.3.5): `values[0]` is `tan(angle)`, `Request::orient_axis` the shear
    /// axis and `ortho` its `orient_axis_ortho`.
    Shear {
        /// `t->orient_axis_ortho`
        ortho: usize,
    },
    /// `TFM_NORMAL_ROTATION` (WP 8.3.5, `transform.rotate_normal`): rotates the custom normals
    /// of the selection by `values[0]`.
    NormalRotation,
}

/// What the transform data are made from (the `TransConvertType`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DataKind {
    /// `TransConvertType_Mesh`: vertices.
    Verts,
    /// `TransConvertType_MeshEdge` (`CTX_EDGE_DATA`): an edge float layer.
    Edges(&'static str),
    /// `TransConvertType_MeshVertCData`: a vertex float layer.
    VertCData(&'static str),
}

impl Mode {
    /// The data the mode transforms.
    pub fn data_kind(self) -> DataKind {
        match self {
            Mode::EdgeCrease => DataKind::Edges("crease_edge"),
            Mode::BevelWeight => DataKind::Edges("bevel_weight_edge"),
            Mode::VertCrease => DataKind::VertCData("crease_vert"),
            _ => DataKind::Verts,
        }
    }
}

/// Proportional editing of one call (`use_proportional_edit=True`; the scene's settings are
/// only used by modal transforms).
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PropEdit {
    /// `proportional_edit_falloff`
    pub falloff: Falloff,
    /// `proportional_size` (reset to 1 at or below 0.00001, as Blender does)
    pub size: f32,
    /// `use_proportional_connected`
    pub connected: bool,
    /// `use_proportional_projected`
    pub projected: bool,
}

/// The decoded operator call.
#[derive(Clone, Debug)]
pub struct Request {
    /// Mode.
    pub mode: Mode,
    /// `value` (zero when unset).
    pub values: [f32; 4],
    /// `value` was set and is an array (`t_values_set_is_array`).
    pub values_is_array: bool,
    /// `orient_axis` (0..2).
    pub orient_axis: usize,
    /// `orient_type`, when set.
    pub orient_type: Option<String>,
    /// `orient_matrix`, when set.
    pub orient_matrix: Option<Mat3>,
    /// `orient_matrix_type`, when set.
    pub orient_matrix_type: Option<String>,
    /// `constraint_axis` when the operator has it and it was set (`None` otherwise).
    pub constraint_axis: Option<[bool; 3]>,
    /// Whether the operator has a `constraint_axis` property.
    pub has_constraint_prop: bool,
    /// `center_override`, when set.
    pub center_override: Option<Vec3>,
    /// Proportional editing.
    pub proportional: Option<PropEdit>,
    /// Slide options (edge/vertex slide).
    pub slide: super::slide::SlideParams,
    /// `use_automerge_and_split`, when the operator has it and it was set (M2 leftover wired
    /// by M4 W0: `T_AUTOMERGE | T_AUTOSPLIT` when true, neither when false; `None` follows
    /// `tool_settings.use_mesh_automerge[_and_split]`).
    pub use_automerge_and_split: Option<bool>,
}

impl Request {
    /// A request with every option at Blender's unset default.
    pub fn new(mode: Mode) -> Request {
        Request {
            mode,
            values: [0.0; 4],
            values_is_array: false,
            orient_axis: 2,
            orient_type: None,
            orient_matrix: None,
            orient_matrix_type: None,
            constraint_axis: None,
            has_constraint_prop: false,
            center_override: None,
            proportional: None,
            slide: super::slide::SlideParams::default(),
            use_automerge_and_split: None,
        }
    }
}

/// One `TransData`.
#[derive(Clone, Debug)]
pub struct Td {
    /// The vertex (vertex data) or `None`.
    pub v: Option<VertId>,
    /// The edge (edge data) or `None`.
    pub e: Option<EdgeId>,
    /// `td->iloc` (object space; zero for custom-data transforms, as in Blender).
    pub iloc: Vec3,
    /// `td->loc` (the result).
    pub loc: Vec3,
    /// `td->center` (object space).
    pub center: Vec3,
    /// `td->axismtx` (object space).
    pub axismtx: Mat3,
    /// `TD_SELECTED`
    pub selected: bool,
    /// `td->dist` (connected distance).
    pub dist: f32,
    /// `td->rdist` (nearest-selected distance).
    pub rdist: f32,
    /// `td->factor`
    pub factor: f32,
    /// `td->ival` (custom-data value).
    pub ival: f32,
    /// `td->val` result.
    pub val: f32,
    /// `td_ext->iscale[0]`: the shell factor of shrink/fatten.
    pub shell: f32,
}

impl Td {
    fn new() -> Td {
        Td {
            v: None,
            e: None,
            iloc: Vec3::ZERO,
            loc: Vec3::ZERO,
            center: Vec3::ZERO,
            axismtx: Mat3::ZERO,
            selected: false,
            dist: 0.0,
            rdist: 0.0,
            factor: 1.0,
            ival: 0.0,
            val: 0.0,
            shell: 0.0,
        }
    }
}

/// One `TransDataContainer` (an object in Edit Mode).
#[derive(Clone, Debug)]
pub struct Tc {
    /// The object.
    pub ob: IdRef<ObjectId>,
    /// Its mesh.
    pub me: IdRef<MeshId>,
    /// `tc->mat` (world matrix).
    pub mat: Mat4,
    /// `tc->imat` (`invert_m4_m4_fallback`).
    pub imat: Mat4,
    /// `tc->mat3_unit`
    pub mat3_unit: Mat3,
    /// `td->mtx` (the 3×3 world matrix).
    pub mtx: Mat3,
    /// `td->smtx` (its pseudo-inverse).
    pub smtx: Mat3,
    /// `tc->center_local`
    pub center_local: Vec3,
    /// The transform data.
    pub data: Vec<Td>,
    /// The float layer of a custom-data mode, once resolved (the container passed the
    /// early-outs of `createTransEdge` / `createTransMeshVertCData`).
    pub cd: Option<CdTarget>,
}

/// The float layer a custom-data mode reads and writes in one edit mesh, resolved as Blender
/// does: `CustomData_has_layer_named(CD_PROP_FLOAT, name)`, else `BM_data_layer_add_named`
/// (whose `CustomData_add_layer_named` makes the name unique within the domain), then
/// `CustomData_get_offset_named(CD_PROP_FLOAT, name)`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CdTarget {
    /// A zero-filled FLOAT layer to add, under this domain-unique name.
    pub add: Option<SmolStr>,
    /// The FLOAT layer the values are read from and written to; `None` when the name is
    /// taken by another attribute, see [`cd_target`].
    pub write: Option<SmolStr>,
}

/// The attribute (custom-data layer) name limit (`MAX_CUSTOMDATA_LAYER_NAME` minus the NUL).
const MAX_LAYER_NAME: usize = 67;

/// Resolves the float layer `name` of the vertex (`vert`) or edge domain of `bm`.
///
/// - A FLOAT layer of that name in the domain is used.
/// - Otherwise a FLOAT layer is added; when another layer of the domain already has the name
///   (another type), the new layer is named `name.001` and Blender's offset lookup fails:
///   vertex data is then skipped (`createTransMeshVertCData` continues), edge values are not
///   stored (Blender writes them through an invalid offset; its layer stays zero).
/// - An edge layer named like a vertex attribute never reaches the mesh: `BM_mesh_bm_to_me`
///   keeps the first attribute of a name in domain order (points, edges, faces, corners), so
///   Blender drops the edge layer and its values on leaving Edit Mode; frap does not add it.
fn cd_target(bm: &BMesh, vert: bool, name: &str) -> CdTarget {
    if !vert && bm.vdata.find(name).is_some() {
        return CdTarget::default();
    }
    let cd = if vert { &bm.vdata } else { &bm.edata };
    if cd
        .get(name)
        .is_some_and(|l| matches!(l.data, AttrData::Float(_)))
    {
        return CdTarget {
            add: None,
            write: Some(name.into()),
        };
    }
    let taken = |n: &str| cd.find(n).is_some();
    let unique = frap_rna::naming::unique_subname(&taken, name, MAX_LAYER_NAME, name);
    CdTarget {
        write: (unique == name).then(|| unique.clone()),
        add: Some(unique),
    }
}

/// Applies a resolved [`CdTarget`]: adds its layer and, when the values are written, drops the
/// layers of the later domains sharing the name (Blender drops them on leaving Edit Mode, see
/// [`cd_target`]).
fn apply_cd_target(bm: &mut BMesh, vert: bool, name: &str, t: &CdTarget) {
    if let Some(n) = &t.add {
        let slots = if vert {
            bm.verts.slots.len()
        } else {
            bm.edges.slots.len()
        };
        let cd = if vert { &mut bm.vdata } else { &mut bm.edata };
        cd.ensure_slots(slots);
        cd.add_layer(CdLayer {
            name: n.clone(),
            data: AttrData::Float(Arc::new(vec![0.0; slots])),
        });
    }
    if t.write.is_none() {
        return;
    }
    let later: Vec<&mut frap_bmesh::CustomData> = if vert {
        vec![&mut bm.edata, &mut bm.pdata, &mut bm.ldata]
    } else {
        vec![&mut bm.pdata, &mut bm.ldata]
    };
    for cd in later {
        while cd.remove_layer(name).is_some() {}
        let first_uv = cd.float2_names().first().cloned();
        for slot in [&mut cd.uv_active, &mut cd.uv_render] {
            if slot.as_deref() == Some(name) {
                *slot = first_uv.clone();
            }
        }
    }
}

/// The transform state (`TransInfo`).
pub struct TransInfo {
    /// The request.
    pub req: Request,
    /// `t->around`
    pub around: PivotPoint,
    /// Containers with data.
    pub tcs: Vec<Tc>,
    /// `t->center_global`
    pub center_global: Vec3,
    /// The orientation (`t->spacemtx`, its type).
    pub orient: Orientation,
    /// The constraint, when applied.
    pub con: Option<Constraint>,
}

/// `transdata_check_local_center` / `transdata_check_local_islands` for meshes.
fn is_local(around: PivotPoint) -> bool {
    around == PivotPoint::IndividualOrigins
}

/// The visible vertices (or edges) count and the selected ones.
fn counts(bm: &BMesh) -> (usize, usize) {
    let mut vis = 0;
    let mut sel = 0;
    for v in bm.vert_ids() {
        if !has(bm, v, ElemFlag::HIDDEN) {
            vis += 1;
            if has(bm, v, ElemFlag::SELECT) {
                sel += 1;
            }
        }
    }
    (vis, sel)
}

/// `createTransEditVerts` / `createTransMeshVertCData` for one container.
fn create_verts(tc: &mut Tc, bm: &BMesh, req: &Request, around: PivotPoint) {
    let prop = req.proportional;
    let connected = prop.is_some_and(|p| p.connected);
    let totvertsel = bm.selected_counts().0;
    if (prop.is_none() || connected) && totvertsel == 0 {
        return;
    }
    let cdata = match req.mode.data_kind() {
        DataKind::VertCData(name) => Some(name),
        _ => None,
    };
    if let Some(name) = cdata {
        let target = cd_target(bm, true, name);
        let skip = target.write.is_none();
        tc.cd = Some(target);
        if skip {
            return;
        }
    }
    let data_len = if prop.is_some() {
        counts(bm).0
    } else {
        totvertsel
    };
    if data_len == 0 {
        return;
    }
    let (verts, vidx) = bmq::vert_index(bm);
    let is_island_center = is_local(around);
    let islands = if is_island_center {
        let calc_single =
            connected && is_local(around) && bm.select_mode.contains(SelectMode::VERT);
        let shrink = matches!(req.mode, Mode::ShrinkFatten { .. });
        islands_calc(
            bm,
            &vidx,
            calc_single,
            cdata.is_none(),
            cdata.is_none() && !shrink,
        )
    } else {
        super::islands::Islands::default()
    };
    let (dists, dists_index) = if connected {
        let (d, i) = connectivity_distance(bm, &vidx, &tc.mtx, is_island_center);
        (Some(d), i)
    } else {
        (None, None)
    };
    let layer = tc
        .cd
        .as_ref()
        .and_then(|t| t.write.as_deref())
        .and_then(|n| bm.vdata.get(n));
    for (a, v) in verts.iter().enumerate() {
        let v = *v;
        if has(bm, v, ElemFlag::HIDDEN) {
            continue;
        }
        let selected = has(bm, v, ElemFlag::SELECT);
        if prop.is_none() && !selected {
            continue;
        }
        let island_index = if islands.vert_map.is_empty() {
            -1
        } else {
            let ci = dists_index
                .as_ref()
                .map(|di| di[a])
                .filter(|i| *i != -1)
                .unwrap_or(a as i32);
            islands.vert_map[ci as usize]
        };
        let mut td = Td::new();
        td.v = Some(v);
        td.selected = selected;
        let vco = co(bm, v);
        let island_center = || {
            islands
                .center
                .as_ref()
                .filter(|_| island_index != -1)
                .map(|c| c[island_index as usize])
        };
        let mut no = vno(bm, v);
        if cdata.is_some() {
            // `mesh_cdata_transdata_create`: no `iloc`, the center is the island's or the
            // vertex (custom-data islands carry no centers).
            td.center = island_center().unwrap_or(vco);
            td.ival = layer.map_or(0.0, |l| float_at(&l.data, v.slot()));
            td.val = td.ival;
            td.axismtx = if is_local(around) {
                create_space_normal(no).unwrap_or(Mat3::ZERO)
            } else {
                Mat3::from_cols(Vec3::ZERO, Vec3::ZERO, no)
            };
        } else {
            td.iloc = vco;
            td.loc = vco;
            if matches!(req.mode, Mode::ShrinkFatten { .. })
                && bm.select_mode.contains(SelectMode::FACE)
                && selected
                && let Some(n) = bmq::vert_calc_normal_selected(bm, v)
            {
                no = n;
            }
            td.center = island_center().unwrap_or(vco);
            td.axismtx = match islands.axismtx.as_ref().filter(|_| island_index != -1) {
                Some(ax) => ax[island_index as usize],
                None if is_local(around) => create_space_normal(no).unwrap_or(Mat3::ZERO),
                None => Mat3::from_cols(Vec3::ZERO, Vec3::ZERO, no),
            };
            if matches!(req.mode, Mode::ShrinkFatten { .. }) {
                td.shell = bmq::vert_calc_shell_factor_selected(bm, v, no);
            }
        }
        if prop.is_some() {
            td.dist = dists.as_ref().map_or(f32::MAX, |d| d[a]);
        }
        tc.data.push(td);
    }
}

fn float_at(d: &AttrData, slot: usize) -> f32 {
    match d {
        AttrData::Float(c) => c.get(slot).copied().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// `createTransEdge` for one container.
fn create_edges(tc: &mut Tc, bm: &BMesh, req: &Request, name: &str) {
    let prop = req.proportional;
    let mut count = 0;
    let mut countsel = 0;
    for e in bm.edge_ids() {
        if !has(bm, e, ElemFlag::HIDDEN) {
            if has(bm, e, ElemFlag::SELECT) {
                countsel += 1;
            }
            if prop.is_some() {
                count += 1;
            }
        }
    }
    let connected = prop.is_some_and(|p| p.connected);
    if (if prop.is_some() && !connected {
        count
    } else {
        countsel
    }) == 0
    {
        return;
    }
    let target = cd_target(bm, false, name);
    let layer = target.write.as_deref().and_then(|n| bm.edata.get(n));
    tc.cd = Some(target);
    for e in bm.edge_ids() {
        if has(bm, e, ElemFlag::HIDDEN) {
            continue;
        }
        let selected = has(bm, e, ElemFlag::SELECT);
        if !selected && prop.is_none() {
            continue;
        }
        let ed = bm.edge(e);
        let mut td = Td::new();
        td.e = Some(e);
        td.selected = selected;
        td.center = mid(co(bm, ed.v[0]), co(bm, ed.v[1]));
        td.ival = layer.map_or(0.0, |l| float_at(&l.data, e.slot()));
        td.val = td.ival;
        tc.data.push(td);
    }
}

/// `prop_dist_loc_get` (edit containers use their local matrix).
fn prop_loc(tc: &Tc, td: &Td, use_island: bool, proj: Option<Vec3>) -> Vec3 {
    let p = if use_island { td.iloc } else { td.center };
    let mut vec = tc.mat.transform_point3(p);
    if let Some(pv) = proj {
        vec -= project(vec, pv);
    }
    vec
}

/// `set_prop_dist`: the distance of every unselected element to the nearest selected one (a
/// kd-tree of the selection, inserted in container and data order).
fn set_prop_dist(t: &mut TransInfo, with_dist: bool, view_matrix: &Mat4) {
    let use_island = is_local(t.around);
    let proj = t.req.proportional.filter(|p| p.projected).map(|_| {
        let viewinv = view_matrix.inverse();
        normalize(viewinv.z_axis.truncate()).0
    });
    let mut table: Vec<(usize, usize)> = Vec::new();
    let mut tree = frap_geom::kdtree::KdTree::new(0);
    for (ci, tc) in t.tcs.iter_mut().enumerate() {
        for td in tc.data.iter_mut() {
            if td.selected {
                td.rdist = 0.0;
            }
        }
        for (di, td) in tc.data.iter().enumerate() {
            if td.selected {
                let vec = prop_loc(tc, td, use_island, proj);
                // A non-finite point is never the nearest one (every comparison with a NaN or
                // infinite distance fails in Blender's search); left out, it cannot break the
                // kd-tree's partitioning either.
                if !vec.is_finite() {
                    continue;
                }
                tree.insert(table.len() as i32, vec.to_array());
                table.push((ci, di));
            }
        }
    }
    tree.balance();
    type Update = (usize, usize, f32, Option<(Vec3, Mat3)>);
    let mut updates: Vec<Update> = Vec::new();
    for (ci, tc) in t.tcs.iter().enumerate() {
        for (di, td) in tc.data.iter().enumerate() {
            if td.selected {
                continue;
            }
            let vec = prop_loc(tc, td, use_island, proj);
            let mut rdist = -1.0;
            let mut copy = None;
            if let Some(n) = tree.find_nearest(vec.to_array()) {
                rdist = n.dist;
                if use_island {
                    let (sc, sd) = table[n.index as usize];
                    let s = &t.tcs[sc].data[sd];
                    copy = Some((s.center, s.axismtx));
                }
            }
            updates.push((ci, di, rdist, copy));
        }
    }
    for (ci, di, rdist, copy) in updates {
        let td = &mut t.tcs[ci].data[di];
        td.rdist = rdist;
        if let Some((c, m)) = copy {
            td.center = c;
            td.axismtx = m;
        }
        if with_dist {
            td.dist = rdist;
        }
    }
}

/// `calculatePropRatio`.
fn calc_prop_ratio(t: &mut TransInfo) {
    let Some(p) = t.req.proportional else {
        return;
    };
    let mut rng: Option<BliRng> = None;
    for tc in &mut t.tcs {
        for td in &mut tc.data {
            if td.selected {
                td.factor = 1.0;
                continue;
            }
            let d = if p.connected { td.dist } else { td.rdist };
            if d > p.size {
                td.factor = 0.0;
                continue;
            }
            let dist = ((p.size - d) / p.size).max(0.0);
            td.factor = match p.falloff {
                Falloff::Sharp => dist * dist,
                Falloff::Smooth => (3.0 * dist * dist - 2.0 * dist * dist * dist).min(1.0),
                Falloff::Root => dist.sqrt(),
                Falloff::Linear => dist,
                Falloff::Constant => 1.0,
                Falloff::Sphere => (2.0 * dist - dist * dist).sqrt(),
                Falloff::Random => {
                    rng.get_or_insert_with(|| BliRng::new(RANDOM_SEED))
                        .get_float()
                        * dist
                }
                Falloff::InverseSquare => dist * (2.0 - dist),
            };
        }
    }
}

/// `transdata_center_global_get` over every selected element.
fn global_centers(t: &TransInfo) -> Vec<Vec3> {
    let mut out = Vec::new();
    for tc in &t.tcs {
        for td in &tc.data {
            if td.selected {
                out.push(tc.mat.transform_point3(td.center));
            }
        }
    }
    out
}

/// `calculateCenter`: the global center and each container's local one.
fn calc_center(t: &mut TransInfo, cursor: Vec3, active_center: Option<Vec3>) {
    let centers = global_centers(t);
    t.center_global = pivot::center(
        t.around,
        &centers,
        cursor,
        active_center,
        t.req.center_override,
    );
    for tc in &mut t.tcs {
        tc.center_local = tc.imat.transform_point3(t.center_global);
    }
}

/// The per-element constraint axes (`applyObjectConstraint*` in Edit Mode).
fn per_element(t: &TransInfo) -> bool {
    t.con
        .is_some_and(|c| c.space == ConstraintSpace::PerElement)
}

fn con_axis_index(c: &Constraint) -> Option<usize> {
    match c.axes {
        [true, false, false] | [false, true, true] => Some(0),
        [false, true, false] | [true, false, true] => Some(1),
        [false, false, true] | [true, true, false] => Some(2),
        _ => None,
    }
}

fn reset_unconstrained(c: &Constraint, mut m: Mat3) -> Mat3 {
    for i in 0..3 {
        if !c.axes[i] {
            m.col_mut(i)[i] = 1.0;
        }
    }
    m
}

/// `ElementRotation_ex` for points.
fn rotate_point(mtx: &Mat3, smtx: &Mat3, td: &mut Td, mat: &Mat3, center: Vec3) {
    let totmat = *mat * *mtx;
    let smat = *smtx * totmat;
    let vec = smat * (td.iloc - center);
    td.loc = vec + center;
}

/// Applies the mode to every element (`transformApply` of one non-interactive call).
fn apply(t: &mut TransInfo) {
    let values = Vec3::new(t.req.values[0], t.req.values[1], t.req.values[2]);
    let spacemtx = t.orient.matrix;
    let spacemtx_inv = t.orient.inverse();
    let local = is_local(t.around);
    let per_elem = per_element(t);
    let con = t.con;
    let prop = t.req.proportional.is_some();
    match t.req.mode {
        Mode::Translate => {
            // `T_INPUT_IS_VALUES_FINAL`: `global_dir = spacemtx · values`.
            let global_dir = if t.req.values_is_array {
                spacemtx * values
            } else {
                values
            };
            for tc in &mut t.tcs {
                for td in &mut tc.data {
                    let mut tvec = global_dir;
                    if per_elem {
                        tvec = tc.mat3_unit * (td.axismtx * (spacemtx_inv * tvec));
                    }
                    tvec = tc.smtx * tvec;
                    tvec *= td.factor;
                    td.loc = td.iloc + tvec;
                }
            }
        }
        Mode::Rotate => {
            let angle = t.req.values[0];
            let axis = rotation_axis(t, true);
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                let (mtx, smtx, mat3_unit) = (tc.mtx, tc.smtx, tc.mat3_unit);
                for td in &mut tc.data {
                    let mut axis_final = axis;
                    let mut angle_final = angle;
                    if let Some(c) = con {
                        if c.space == ConstraintSpace::PerElement {
                            let m = mat3_unit * td.axismtx;
                            axis_final = con_axis_index(&c).map_or(axis, |i| m.col(i));
                        } else {
                            axis_final = con_axis_index(&c).map_or(axis, |i| spacemtx.col(i));
                        }
                        angle_final = angle * td.factor;
                    } else if prop {
                        angle_final = angle * td.factor;
                    }
                    let mat = axis_angle_normalized_to_mat3(axis_final, angle_final);
                    let center = if local { td.center } else { center_local };
                    rotate_point(&mtx, &smtx, td, &mat, center);
                }
            }
        }
        Mode::Resize => {
            let mut mat = Mat3::from_diagonal(values);
            if let Some(c) = con
                && c.space == ConstraintSpace::Orientation
            {
                mat = spacemtx * (reset_unconstrained(&c, mat) * spacemtx_inv);
            }
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                for td in &mut tc.data {
                    let smat = mat * tc.mtx;
                    let mut tmat = tc.smtx * smat;
                    if let Some(c) = con
                        && per_elem
                    {
                        // `applyObjectConstraintSize`: Blender multiplies by `mat3_unit` and then
                        // overwrites the result, so only the element axes count.
                        let imat = invert_m3(&td.axismtx).unwrap_or(Mat3::ZERO);
                        tmat = td.axismtx * (reset_unconstrained(&c, tmat) * imat);
                    }
                    let center = if local { td.center } else { center_local };
                    let mut vec = tmat * (td.iloc - center);
                    vec += center;
                    vec -= td.iloc;
                    vec *= td.factor;
                    td.loc = td.iloc + vec;
                }
            }
        }
        Mode::ShrinkFatten { even } => {
            let distance = t.req.values[0];
            for tc in &mut t.tcs {
                for td in &mut tc.data {
                    let mut tdistance = distance * td.factor;
                    if even {
                        tdistance *= td.shell;
                    }
                    td.loc = super::mathx::madd(td.iloc, td.axismtx.z_axis, tdistance);
                }
            }
        }
        Mode::ToSphere => {
            let radius = to_sphere_radius(t);
            let ratio = t.req.values[0].clamp(0.0, 1.0);
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                for td in &mut tc.data {
                    let center = if local { td.center } else { center_local };
                    let (vec, r) = normalize(td.iloc - center);
                    let tratio = ratio * td.factor;
                    let vec = vec * (r * (1.0 - tratio) + radius * tratio);
                    td.loc = vec + center;
                }
            }
        }
        Mode::PushPull => {
            let distance = t.req.values[0];
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                for td in &mut tc.data {
                    let mut vec = center_local - td.center;
                    if let Some(c) = con {
                        // Only reachable through `transform.transform`; a full XYZ constraint
                        // has no axis (Blender reads an uninitialized one).
                        let axes = if per_elem {
                            tc.mat3_unit * td.axismtx
                        } else {
                            spacemtx
                        };
                        let axis = con_axis_index(&c).map_or(Vec3::ZERO, |i| axes.col(i));
                        let axis = tc.smtx * axis;
                        let lock = c.axes.iter().filter(|a| **a).count() == 2;
                        if lock {
                            vec -= project(vec, axis);
                        } else {
                            vec = project(vec, axis);
                        }
                    }
                    let (v, _) = normalize_length(vec, distance * td.factor);
                    td.loc = td.iloc + v;
                }
            }
        }
        Mode::Mirror => {
            let Some(c) = con else {
                // No axis: `ElementMirror(-1, false)` leaves every element in place.
                for tc in &mut t.tcs {
                    for td in &mut tc.data {
                        td.loc = td.iloc;
                    }
                }
                return;
            };
            let bits = c.axes.iter().filter(|a| **a).count();
            let special = if bits != 0 && bits != 3 {
                let want = bits != 2;
                c.axes.iter().position(|a| *a == want)
            } else {
                None
            };
            let flip = bits >= 2;
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                for td in &mut tc.data {
                    let center = if local { td.center } else { center_local };
                    let mut vec = td.iloc - center;
                    if let Some(axis) = special {
                        vec = tc.mtx * vec;
                        vec = reflect(vec, spacemtx.col(axis));
                        vec = tc.smtx * vec;
                    }
                    if flip {
                        vec = -vec;
                    }
                    vec += center;
                    vec -= td.iloc;
                    td.loc = td.iloc + vec;
                }
            }
        }
        Mode::EdgeCrease | Mode::VertCrease | Mode::BevelWeight => {
            let value = t.req.values[0].min(1.0);
            for tc in &mut t.tcs {
                for td in &mut tc.data {
                    td.val = (td.ival + value * td.factor).clamp(0.0, 1.0);
                }
            }
        }
        Mode::Shear { ortho } => {
            let mat_final = shear_matrix(&spacemtx, t.req.orient_axis, ortho, t.req.values[0]);
            for tc in &mut t.tcs {
                let center_local = tc.center_local;
                let tmat = tc.smtx * (mat_final * tc.mtx);
                for td in &mut tc.data {
                    let center = if local { td.center } else { center_local };
                    let mut vec = tmat * (td.iloc - center);
                    vec += center;
                    vec -= td.iloc;
                    vec *= td.factor;
                    td.loc = td.iloc + vec;
                }
            }
        }
        Mode::EdgeSlide | Mode::VertSlide | Mode::NormalRotation => {}
    }
}

/// `apply_shear_value`'s `mat_final`: the shear `x += value · y` in the frame whose X is the
/// `ortho` axis of `spacemtx`, whose Z is its `axis` axis and whose Y is `X × Z`.
pub fn shear_matrix(spacemtx: &Mat3, axis: usize, ortho: usize, value: f32) -> Mat3 {
    let mut smat = Mat3::IDENTITY;
    // `smat[1][0] = value`: column 1, row 0.
    smat.y_axis.x = value;
    let x = spacemtx.col(ortho);
    let z = spacemtx.col(axis);
    let axismat_inv = Mat3::from_cols(x, super::mathx::cross(x, z), z);
    let axismat = invert_m3(&axismat_inv).unwrap_or(Mat3::ZERO);
    axismat_inv * (smat * axismat)
}

/// The rotation axis of `rotate`/`rotate_normal` (`transform_mode_rotation_axis_get` /
/// `applyAxisConstraintRot` / `applyObjectConstraintRot` without an element): the constrained
/// axis, else the orientation's `orient_axis`, negated for `TFM_ROTATION`.
fn rotation_axis(t: &TransInfo, negate: bool) -> Vec3 {
    match t.con {
        Some(c) => {
            let axes = if c.space == ConstraintSpace::PerElement {
                t.tcs
                    .first()
                    .and_then(|tc| tc.data.first().map(|td| tc.mat3_unit * td.axismtx))
                    .unwrap_or(Mat3::IDENTITY)
            } else {
                t.orient.matrix
            };
            con_axis_index(&c).map_or(Vec3::ZERO, |i| axes.col(i))
        }
        None if negate => -t.orient.matrix.col(t.req.orient_axis),
        None => t.orient.matrix.col(t.req.orient_axis),
    }
}

// Ported from blender/source/blender/editors/transform/transform_mode_edge_rotate_normal.cc
// (initNormalRotation, storeCustomLNorValue, applyNormalRotation), Blender 5.2.4,
// GPL-2.0-or-later.
/// `transform.rotate_normal`: every container's custom normals (`BKE_editmesh_lnorspace_update`
/// adds the `custom_normal` layer when missing) of the loops chosen by the selection
/// (`BM_loop_normal_editdata_array_init(bm, false)`) are turned by the angle about the axis
/// and encoded in their fan spaces. The normals are object-space values turned by the
/// world-space rotation, as in Blender.
fn rotate_normals(cx: &mut OpCtx, t: &TransInfo) -> OpResult {
    use frap_bmesh::clnor;
    let axis = rotation_axis(t, false);
    let mat = axis_angle_normalized_to_mat3(axis, t.req.values[0]);
    for tc in &t.tcs {
        let bm = cx.bmesh_mut(tc.me)?;
        let sp = clnor::update(bm);
        let arr = clnor::editdata_array(bm, &sp, false, clnor::Mark::Indiv);
        for ed in &arr.data {
            let n = mat * Vec3::from_array(ed.nloc);
            let v = sp.encode(ed.loop_, n.to_array());
            clnor::set(bm, ed.loop_, v);
        }
    }
    Ok(OpStatus::Finished)
}

/// `to_sphere_radius_update`.
fn to_sphere_radius(t: &TransInfo) -> f32 {
    let local = is_local(t.around);
    let mut radius = 0.0f32;
    if t.req.proportional.is_some() {
        // Blender accumulates the factors in an `int` (truncating at every step).
        let mut factor_accum: i32 = 0;
        for tc in &t.tcs {
            for td in &tc.data {
                if td.factor == 0.0 {
                    continue;
                }
                let center = if local { td.center } else { tc.center_local };
                radius += td.factor * len(td.iloc - center);
                factor_accum = (factor_accum as f32 + td.factor) as i32;
            }
        }
        if factor_accum != 0 {
            radius /= factor_accum as f32;
        }
    } else {
        let mut n = 0usize;
        for tc in &t.tcs {
            for td in &tc.data {
                let center = if local { td.center } else { tc.center_local };
                radius += len(td.iloc - center);
                n += 1;
            }
        }
        radius /= n as f32;
    }
    radius
}

/// The orientation of an edit-mode call (`transform_orientation_matrix_get`): the core
/// orientation, with NORMAL built from the active edit mesh's selection and GIMBAL of a
/// non-Euler active object falling through to its parent's axes.
pub fn orientation(
    c: &frap_scene::Context,
    edit: &frap_scene::EditState,
    req: &Request,
    around: PivotPoint,
) -> Orientation {
    let src = crate::object::orient_source(c);
    let (orient_type, orient_matrix, orient_matrix_type) = orient_props(req);
    let mut orient = Orientation::from_props(orient_type, orient_matrix, orient_matrix_type, &src);
    if orient.kind == OrientType::Normal {
        orient.matrix = c
            .active_object()
            .and_then(|a| {
                let ob = c.main.objects.get(a)?;
                let me = ob.data?.typed::<MeshId>()?;
                let em = edit.meshes.get(&me.key())?;
                Some(normal_orientation(
                    &em.bm,
                    &world_matrix_in(c.main, a),
                    around == PivotPoint::ActiveElement,
                ))
            })
            .unwrap_or(Mat3::IDENTITY);
    } else if orient.kind == OrientType::Gimbal && src.active_gimbal.is_none() {
        // Not a gimbal rotation: Blender falls through to the PARENT orientation.
        orient.matrix = src
            .active_parent_world
            .map_or(Mat3::IDENTITY, |m| crate::core::math::axes_of(&m));
    }
    orient
}

/// The `orient_type`, `orient_matrix` and `orient_matrix_type` handed to
/// [`Orientation::from_props`] so that it resolves them as `initTransInfo` does
/// (`transform_generics.cc`): a set `orient_matrix` is used when `orient_matrix_type` is set and
/// equals the orientation (an unset `orient_type` then takes `orient_matrix_type`), or when
/// neither type is set (`V3D_ORIENT_CUSTOM_MATRIX`); with `orient_type` set and
/// `orient_matrix_type` unset, the matrix is ignored.
fn orient_props(req: &Request) -> (Option<&str>, Option<Mat3>, &str) {
    let ot = req.orient_type.as_deref();
    match (req.orient_matrix, req.orient_matrix_type.as_deref()) {
        (None, _) => (ot, None, "GLOBAL"),
        (Some(m), Some(mt)) => (Some(ot.unwrap_or(mt)), Some(m), mt),
        (Some(m), None) if ot.is_none() => (Some("GLOBAL"), Some(m), "GLOBAL"),
        (Some(_), None) => (ot, None, "GLOBAL"),
    }
}

/// `transform.translate(cursor_transform=True)` in Edit Mode: moves the 3D cursor by the
/// value in the edit-mode orientation.
pub fn translate_cursor(cx: &mut OpCtx, req: &Request) -> OpResult {
    let (scene, vec) = {
        let (c, edit) = cx.split_edit();
        let around = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
        let orient = orientation(&c, edit, req, around);
        let values = Vec3::new(req.values[0], req.values[1], req.values[2]);
        let mut vec = orient.matrix * values;
        if let Some(con) =
            Constraint::from_props(req.values_is_array, req.constraint_axis, &orient, 1)
        {
            vec = con.apply_vec(&orient, vec);
        }
        (c.win.scene, vec)
    };
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let l = Vec3::from_array(s.cursor.location) + vec;
        s.cursor.location = l.to_array();
        crate::object::sync_cursor_matrix(&mut s.cursor);
    }
    Ok(OpStatus::Finished)
}

/// Runs one edit-mode transform. `CANCELLED` (with a warning) when nothing can be
/// transformed, as Blender's `initTransform` fails then.
pub fn run(cx: &mut OpCtx, req: Request) -> OpResult {
    let mut req = req;
    if let Some(p) = &mut req.proportional
        && p.size <= 0.00001
    {
        p.size = 1.0;
    }
    let meshes = helpers::iter_edit_meshes(cx);
    let mut cd_targets: Vec<(IdRef<MeshId>, CdTarget)> = Vec::new();
    // `T_AUTOMERGE` / `T_AUTOSPLIT` (`initTransInfo`): the operator's `use_automerge_and_split`
    // when set (true: both; false: neither), else `tool_settings.use_mesh_automerge` and
    // `use_mesh_automerge_and_split`. Only the vertex conversions run
    // `special_aftertrans_update__mesh`; Auto Merge with split is `automerge.rs` (WP 4.6).
    let automerge = {
        let ts = cx.ctx().tool_settings();
        let (merge, split) = match req.use_automerge_and_split {
            Some(v) => (v, v),
            None => (
                ts.use_mesh_automerge,
                ts.use_mesh_automerge && ts.use_mesh_automerge_and_split,
            ),
        };
        (merge && req.mode.data_kind() == DataKind::Verts).then_some((ts.double_threshold, split))
    };
    let (mut t, view_matrix) = {
        let (c, edit) = cx.split_edit();
        let around = PivotPoint::from_identifier(&c.tool_settings().transform_pivot_point);
        let mut t = TransInfo {
            req,
            around,
            tcs: Vec::new(),
            center_global: Vec3::ZERO,
            orient: Orientation::GLOBAL,
            con: None,
        };
        for (ob, me) in &meshes {
            let Some(em) = edit.meshes.get(&me.key()) else {
                continue;
            };
            let bm = &em.bm;
            let mat = world_matrix_in(c.main, *ob);
            let mtx = Mat3::from_mat4(mat);
            let mut tc = Tc {
                ob: *ob,
                me: *me,
                mat,
                imat: invert_m4_fallback(&mat),
                mat3_unit: normalize_m3(&mtx),
                mtx,
                smtx: pseudoinverse_m3(&mtx),
                center_local: Vec3::ZERO,
                data: Vec::new(),
                cd: None,
            };
            match t.req.mode.data_kind() {
                DataKind::Edges(name) => create_edges(&mut tc, bm, &t.req, name),
                _ => create_verts(&mut tc, bm, &t.req, around),
            }
            if let Some(target) = &tc.cd {
                cd_targets.push((tc.me, target.clone()));
            }
            if !tc.data.is_empty() {
                t.tcs.push(tc);
            }
        }
        (t, c.win.view3d.view_matrix)
    };
    // `BM_data_layer_add_named` happens while the data is created, whatever comes next.
    match t.req.mode.data_kind() {
        DataKind::Verts => {}
        DataKind::Edges(name) | DataKind::VertCData(name) => {
            let vert = matches!(t.req.mode.data_kind(), DataKind::VertCData(_));
            for (me, target) in &cd_targets {
                apply_cd_target(cx.bmesh_mut(*me)?, vert, name, target);
                if target.write.is_none() {
                    let taken_by = if vert || target.add.is_some() {
                        "another attribute type of the same domain"
                    } else {
                        "a vertex attribute"
                    };
                    let skipped = if vert {
                        "the mesh is skipped"
                    } else {
                        "the values are not stored"
                    };
                    cx.report(
                        ReportType::Warning,
                        format!(
                            "frap: the attribute name '{name}' is taken by {taken_by}: {skipped}, \
                             as in Blender"
                        ),
                    );
                }
            }
        }
    }
    let nothing = t.tcs.is_empty()
        || (t.req.proportional.is_some()
            && !t.tcs.iter().any(|tc| tc.data.iter().any(|td| td.selected)));
    if nothing {
        cx.report(
            ReportType::Warning,
            "frap: nothing to transform: no visible element of the edit meshes is selected",
        );
        return Ok(OpStatus::Cancelled);
    }

    // `init_proportional_edit`.
    if let Some(p) = t.req.proportional {
        match t.req.mode.data_kind() {
            DataKind::Edges(_) => set_prop_dist(&mut t, true, &view_matrix),
            _ if p.connected => {}
            _ => set_prop_dist(&mut t, false, &view_matrix),
        }
        calc_prop_ratio(&mut t);
    }

    // Orientation, centers and the constraint.
    {
        let (c, edit) = cx.split_edit();
        let cursor = Vec3::from_array(c.cursor().location);
        let active_center = {
            let tc = &t.tcs[0];
            edit.meshes.get(&tc.me.key()).and_then(|em| {
                bmq::select_history_active_get(&em.bm).map(|ese| {
                    tc.mat
                        .transform_point3(bmq::editselection_center(&em.bm, ese))
                })
            })
        };
        calc_center(&mut t, cursor, active_center);

        let orient = orientation(&c, edit, &t.req, t.around);
        t.orient = orient;
        if t.req.has_constraint_prop {
            let n = t.tcs.iter().map(|tc| tc.data.len()).sum();
            t.con =
                Constraint::from_props(t.req.values_is_array, t.req.constraint_axis, &orient, n)
                    .map(|mut con| {
                        con.space = if orient.kind == OrientType::Normal && is_local(t.around) {
                            ConstraintSpace::PerElement
                        } else {
                            ConstraintSpace::Orientation
                        };
                        con
                    });
        }
    }

    let mes: Vec<IdRef<MeshId>> = t.tcs.iter().map(|tc| tc.me).collect();
    let status = match t.req.mode {
        Mode::EdgeSlide | Mode::VertSlide => super::slide::run(cx, t)?,
        Mode::NormalRotation => rotate_normals(cx, &t)?,
        _ => {
            apply(&mut t);
            write_back(cx, &t)?
        }
    };
    if let (Some((dist, split)), OpStatus::Finished) = (automerge, &status) {
        for me in mes {
            let bm = cx.bmesh_mut(me)?;
            if split {
                super::automerge::automerge_and_split(bm, dist)?;
            } else {
                auto_merge(bm, dist)?;
            }
        }
    }
    Ok(status)
}

// Ported from blender/source/blender/editors/transform/transform_convert_mesh.cc
// (special_aftertrans_update__mesh, without mesh symmetry: frap does not bind
// `Mesh.use_mirror_x/y/z`) and blender/source/blender/editors/mesh/editmesh_automerge.cc
// (EDBM_automerge → edbm_automerge_impl), Blender 5.2.4, GPL-2.0-or-later.
/// Auto Merge Vertices after a transform: `find_doubles verts=<all> keep_verts=<unselected>
/// dist=double_threshold`, then `weld_verts` (selected vertices merge into unselected ones and
/// into each other); in face select mode the selection is flushed from the edges again so the
/// faces re-select.
fn auto_merge(bm: &mut BMesh, dist: f32) -> Result<(), OpError> {
    use frap_bmesh::ops::find_doubles::{FindDoublesIn, find_doubles};
    use frap_bmesh::ops::weld_verts::{WeldVertsIn, weld_verts};
    let has_face_sel = bm.selected_counts().2 != 0;
    let err = |e: frap_bmesh::BmError| OpError::Runtime(e.to_string());
    // `%av`: every vertex; `%Hv` with `BMO_FLAG_RESPECT_HIDE`: the visible unselected ones.
    let verts: Vec<VertId> = bm.vert_ids();
    let keep_verts: Vec<VertId> = bm
        .verts
        .iter()
        .filter(|(_, v)| !v.flag.contains(ElemFlag::HIDDEN) && !v.flag.contains(ElemFlag::SELECT))
        .map(|(i, _)| VertId(i))
        .collect();
    let found = find_doubles(
        bm,
        &FindDoublesIn {
            verts,
            keep_verts,
            use_connected: false,
            dist,
        },
    )
    .map_err(err)?;
    weld_verts(
        bm,
        &WeldVertsIn {
            targetmap: found.targetmap,
            use_centroid: false,
            average_vert_data: false,
        },
    )
    .map_err(err)?;
    if has_face_sel && bm.select_mode == SelectMode::FACE {
        bm.select_flush_mode_ex(SelectMode::EDGE, false);
    }
    Ok(())
}

/// Writes the results into the edit BMeshes (`recalcData_mesh`: positions, then normals; the
/// custom-data modes write their layer).
pub fn write_back(cx: &mut OpCtx, t: &TransInfo) -> OpResult {
    for tc in &t.tcs {
        let bm = cx.bmesh_mut(tc.me)?;
        match t.req.mode.data_kind() {
            DataKind::Verts => {
                for td in &tc.data {
                    if let Some(v) = td.v {
                        bm.vert_mut(v).co = td.loc.to_array();
                    }
                }
                normals_update(bm, t, tc);
            }
            DataKind::Edges(_) => {
                let slots = bm.edges.slots.len();
                let Some(col) = float_col(&mut bm.edata, tc.cd.as_ref(), slots) else {
                    continue;
                };
                for td in &tc.data {
                    if let Some(e) = td.e {
                        col[e.slot()] = td.val;
                    }
                }
            }
            DataKind::VertCData(_) => {
                let slots = bm.verts.slots.len();
                let Some(col) = float_col(&mut bm.vdata, tc.cd.as_ref(), slots) else {
                    continue;
                };
                for td in &tc.data {
                    if let Some(v) = td.v {
                        col[v.slot()] = td.val;
                    }
                }
            }
        }
    }
    Ok(OpStatus::Finished)
}

/// `mesh_partial_types_calc` (no snapping, no projection): whether the tessellation and the
/// normals need the `PARTIAL_TYPE_ALL` update (`PARTIAL_TYPE_GROUP` otherwise).
fn partial_types(t: &TransInfo) -> (bool, bool) {
    match t.req.mode {
        Mode::Translate => (false, false),
        Mode::Rotate => (false, true),
        Mode::Resize => {
            // Ensure zero (or small) scale calculates normals (Blender #159460).
            let v = &t.req.values;
            let all = t.con.is_some()
                || v[..3].iter().any(|x| *x <= 1e-6)
                || v[0] != v[1]
                || v[0] != v[2];
            (false, all)
        }
        _ => (true, true),
    }
}

/// `mesh_partial_update` of one non-interactive run (`recalcData_mesh`): a full normals update
/// when everything is updated and selected, else Blender's partial update — face normals of
/// the faces of the partial (`BM_mesh_partial_create_from_verts*`), then the vertex normals of
/// its vertices.
///
/// Ported from blender/source/blender/editors/transform/transform_convert_mesh.cc
/// (`mesh_partial_update`, `mesh_partial_ensure`) and
/// blender/source/blender/bmesh/intern/bmesh_mesh_partial_update.cc. As in Blender 5.2.4, a
/// loose vertex (in no face) keeps its normal under a `PARTIAL_TYPE_ALL` update (it is never
/// part of that partial) and under a proportional `PARTIAL_TYPE_GROUP` one (`mesh_partial_ensure`
/// passes a zero group count, which disables the loose-vertex pass).
fn normals_update(bm: &mut BMesh, t: &TransInfo, tc: &Tc) {
    let (looptris_all, normals_all) = partial_types(t);
    let (totvert, totvertsel) = (bm.vert_ids().len(), bm.selected_counts().0);
    if looptris_all && normals_all && totvert == totvertsel {
        bm.normals_update();
        return;
    }
    let (verts, vidx) = bmq::vert_index(bm);
    // `td->factor != 0` marks a vertex (`verts_mask`), `verts_group` is 1 at full factor and
    // -1 below it.
    let mut group = vec![0i32; verts.len()];
    let mut mask_count = 0usize;
    for td in &tc.data {
        let Some(v) = td.v else { continue };
        if td.factor == 0.0 {
            continue;
        }
        let i = vidx[v.slot()] as usize;
        group[i] = if td.factor < 1.0 { -1 } else { 1 };
        mask_count += 1;
    }
    let mut faces: Vec<frap_bmesh::FaceId> = Vec::new();
    let mut face_tag = rustc_hash::FxHashSet::default();
    let gi = |v: VertId| group[vidx[v.slot()] as usize];
    let mut loose_pass = false;
    if normals_all {
        // `BM_mesh_partial_create_from_verts`: every face of a marked vertex.
        for (i, &v) in verts.iter().enumerate() {
            if group[i] == 0 {
                continue;
            }
            for f in bm.vert_faces(v) {
                if face_tag.insert(f) {
                    faces.push(f);
                }
            }
        }
    } else {
        let multi = t.req.proportional.is_some();
        for f in bm.face_ids() {
            let ls = bm.face_loops(f);
            let mixed = if multi {
                // `BM_mesh_partial_create_from_verts_group_multi`.
                let first = gi(bm.lp(bm.lp(ls[0]).prev).v);
                ls.iter().any(|&l| {
                    let g = gi(bm.lp(l).v);
                    g != first || g == -1
                })
            } else {
                // `BM_mesh_partial_create_from_verts_group_single`.
                let mut a = false;
                let mut b = false;
                ls.iter().any(|&l| {
                    if gi(bm.lp(l).v) != 0 {
                        a = true;
                    } else {
                        b = true;
                    }
                    a && b
                })
            };
            if mixed && face_tag.insert(f) {
                faces.push(f);
            }
        }
        // The group-multi call gets `verts_group_count == 0` from `mesh_partial_ensure`.
        loose_pass = !multi;
    }
    let mut part_verts: Vec<VertId> = Vec::new();
    let mut vert_tag = vec![false; verts.len()];
    for &f in &faces {
        for l in bm.face_loops(f) {
            let v = bm.lp(l).v;
            let i = vidx[v.slot()] as usize;
            if !vert_tag[i] {
                vert_tag[i] = true;
                part_verts.push(v);
            }
        }
    }
    if loose_pass && part_verts.len() < mask_count {
        for (i, &v) in verts.iter().enumerate() {
            if group[i] != 0 && !vert_tag[i] && bm.vert_face_count(v) == 0 {
                vert_tag[i] = true;
                part_verts.push(v);
            }
        }
    }
    // The face normals of the partial come from the tessellation pass or the normals pass
    // (both cover the same faces or a subset of the normals pass's).
    for &f in &faces {
        bm.face_normal_update(f);
    }
    for &v in &part_verts {
        bm.vert_normal_update(v);
    }
}

/// The FLOAT column a custom-data mode writes (`None` when the values are not stored, see
/// [`cd_target`]).
fn float_col<'a>(
    cd: &'a mut frap_bmesh::CustomData,
    target: Option<&CdTarget>,
    slots: usize,
) -> Option<&'a mut Vec<f32>> {
    let name = target?.write.as_deref()?;
    cd.ensure_slots(slots);
    let layer = cd.layers.iter_mut().find(|l| l.name == name)?;
    match &mut layer.data {
        AttrData::Float(c) => Some(Arc::make_mut(c)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_bmesh::FromMeshParams;
    use frap_rna::IdKey;

    fn cube() -> BMesh {
        BMesh::from_mesh(
            &frap_mesh::primitives::cube(2.0, true, [1.0; 3]),
            &FromMeshParams::default(),
        )
    }

    fn add(cd: &mut frap_bmesh::CustomData, name: &str, data: AttrData) {
        cd.add_layer(CdLayer {
            name: name.into(),
            data,
        });
    }

    fn some(n: &str) -> Option<SmolStr> {
        Some(n.into())
    }

    #[test]
    fn a_layer_of_another_type_gets_a_unique_float_layer_and_no_values() {
        // Verifier repro: an INT `crease_edge` edge attribute, then `edge_crease`. Blender adds
        // `crease_edge.001` (FLOAT, zero) and leaves the INT layer alone.
        let mut bm = cube();
        let ne = bm.edges.slots.len();
        add(
            &mut bm.edata,
            "crease_edge",
            AttrData::Int(Arc::new(vec![3; ne])),
        );
        let t = cd_target(&bm, false, "crease_edge");
        assert_eq!(
            t,
            CdTarget {
                add: some("crease_edge.001"),
                write: None
            }
        );
        apply_cd_target(&mut bm, false, "crease_edge", &t);
        assert!(float_col(&mut bm.edata, Some(&t), ne).is_none());
        let names: Vec<_> = bm.edata.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["crease_edge.001", "crease_edge"]);
        assert!(matches!(
            &bm.edata.get("crease_edge").expect("INT layer").data,
            AttrData::Int(v) if v.iter().all(|x| *x == 3)
        ));
        // A second call finds no FLOAT `crease_edge` either.
        assert_eq!(
            cd_target(&bm, false, "crease_edge").add,
            some("crease_edge.002")
        );
        // The same for the vertex crease: the mesh is skipped.
        let nv = bm.verts.slots.len();
        add(
            &mut bm.vdata,
            "crease_vert",
            AttrData::Bool(Arc::new(vec![false; nv])),
        );
        let t = cd_target(&bm, true, "crease_vert");
        assert_eq!(t.add, some("crease_vert.001"));
        assert_eq!(t.write, None);
    }

    #[test]
    fn float_layers_are_reused_and_other_domains_give_way() {
        let mut bm = cube();
        let (nv, ne) = (bm.verts.slots.len(), bm.edges.slots.len());
        let nl = bm.loops.slots.len();
        add(
            &mut bm.edata,
            "crease_edge",
            AttrData::Float(Arc::new(vec![0.25; ne])),
        );
        let t = cd_target(&bm, false, "crease_edge");
        assert_eq!(
            t,
            CdTarget {
                add: None,
                write: some("crease_edge")
            }
        );
        assert_eq!(
            float_col(&mut bm.edata, Some(&t), ne).expect("FLOAT")[0],
            0.25
        );
        // An edge layer named like a vertex attribute never reaches the mesh: not added.
        add(
            &mut bm.vdata,
            "bevel_weight_edge",
            AttrData::Float(Arc::new(vec![0.0; nv])),
        );
        assert_eq!(
            cd_target(&bm, false, "bevel_weight_edge"),
            CdTarget::default()
        );
        // The vertex crease wins over same-named edge, face and corner layers (a UV map too).
        add(
            &mut bm.edata,
            "crease_vert",
            AttrData::Float(Arc::new(vec![0.0; ne])),
        );
        add(
            &mut bm.ldata,
            "crease_vert",
            AttrData::Float2(Arc::new(vec![[0.0; 2]; nl])),
        );
        bm.ldata.uv_active = some("crease_vert");
        let t = cd_target(&bm, true, "crease_vert");
        assert_eq!(
            t,
            CdTarget {
                add: some("crease_vert"),
                write: some("crease_vert")
            }
        );
        apply_cd_target(&mut bm, true, "crease_vert", &t);
        assert!(bm.edata.get("crease_vert").is_none());
        assert!(bm.ldata.get("crease_vert").is_none());
        assert_eq!(bm.ldata.uv_active, some("UVMap"));
        float_col(&mut bm.vdata, Some(&t), nv).expect("FLOAT")[1] = 0.5;
        assert!(matches!(
            &bm.vdata.get("crease_vert").expect("added").data,
            AttrData::Float(v) if v[1] == 0.5
        ));
    }

    fn info(points: &[(Vec3, bool)]) -> TransInfo {
        let mut req = Request::new(Mode::Translate);
        req.proportional = Some(PropEdit {
            falloff: Falloff::Smooth,
            size: 1e12,
            connected: false,
            projected: false,
        });
        let data = points
            .iter()
            .map(|&(c, selected)| {
                let mut td = Td::new();
                td.center = c;
                td.iloc = c;
                td.loc = c;
                td.selected = selected;
                td
            })
            .collect();
        TransInfo {
            req,
            around: PivotPoint::MedianPoint,
            tcs: vec![Tc {
                ob: IdRef::from_key(IdKey::next()),
                me: IdRef::from_key(IdKey::next()),
                mat: Mat4::IDENTITY,
                imat: Mat4::IDENTITY,
                mat3_unit: Mat3::IDENTITY,
                mtx: Mat3::IDENTITY,
                smtx: Mat3::IDENTITY,
                center_local: Vec3::ZERO,
                data,
                cd: None,
            }],
            center_global: Vec3::ZERO,
            orient: Orientation::GLOBAL,
            con: None,
        }
    }

    #[test]
    fn proportional_distances_skip_non_finite_points() {
        // Verifier repro (overflowing values): NaN and infinite selected coordinates used to
        // send the kd-tree partition out of bounds.
        let nan = Vec3::splat(f32::NAN);
        let inf = Vec3::new(f32::INFINITY, 0.0, 0.0);
        let mut pts = vec![(nan, true), (inf, true), (Vec3::new(1.0, 0.0, 0.0), true)];
        for i in 0..6 {
            pts.push((Vec3::new(f32::NAN, i as f32, 0.0), true));
            pts.push((Vec3::new(-f32::INFINITY, i as f32, 0.0), true));
        }
        pts.push((Vec3::new(4.0, 0.0, 0.0), false));
        pts.push((nan, false));
        let mut t = info(&pts);
        set_prop_dist(&mut t, false, &Mat4::IDENTITY);
        let d = &t.tcs[0].data;
        assert_eq!(d[d.len() - 2].rdist, 3.0);
        // A NaN query finds nothing (every comparison fails).
        assert_eq!(d[d.len() - 1].rdist, -1.0);
        assert!(d.iter().filter(|td| td.selected).all(|td| td.rdist == 0.0));
        calc_prop_ratio(&mut t);
    }

    #[test]
    fn orient_matrix_type_resolution() {
        let m = Mat3::from_cols(Vec3::Y, -Vec3::X, Vec3::Z);
        let mut r = Request::new(Mode::Rotate);
        assert_eq!(orient_props(&r), (None, None, "GLOBAL"));
        r.orient_matrix = Some(m);
        // Neither type set: V3D_ORIENT_CUSTOM_MATRIX.
        assert_eq!(orient_props(&r), (Some("GLOBAL"), Some(m), "GLOBAL"));
        // Only orient_matrix_type: orient_type takes it, so the matrix is used.
        r.orient_matrix_type = Some("LOCAL".into());
        assert_eq!(orient_props(&r), (Some("LOCAL"), Some(m), "LOCAL"));
        r.orient_type = Some("GLOBAL".into());
        assert_eq!(orient_props(&r), (Some("GLOBAL"), Some(m), "LOCAL"));
        // orient_type without orient_matrix_type: the matrix is ignored.
        r.orient_matrix_type = None;
        assert_eq!(orient_props(&r), (Some("GLOBAL"), None, "GLOBAL"));
    }
}
