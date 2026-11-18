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
//! (`transform_mode_mirror.cc`) and `transdata_elem_value` (`transform_mode_customdata.cc`).
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
use frap_scene::id::transform::world_matrix;
use glam::{Mat3, Mat4, Vec3};

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
    /// `orient_matrix_type`.
    pub orient_matrix_type: String,
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
            orient_matrix_type: "GLOBAL".into(),
            constraint_axis: None,
            has_constraint_prop: false,
            center_override: None,
            proportional: None,
            slide: super::slide::SlideParams::default(),
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
    /// A custom-data layer to create (`BM_data_layer_add_named`) before writing.
    pub add_layer: bool,
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
    if let Some(name) = cdata
        && bm.vdata.find(name).is_none()
    {
        tc.add_layer = true;
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
    let layer = cdata.and_then(|name| bm.vdata.get(name));
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
    let layer = bm.edata.get(name);
    tc.add_layer = layer.is_none();
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
            let axis = match con {
                Some(c) => {
                    let axes = if c.space == ConstraintSpace::PerElement {
                        t.tcs
                            .first()
                            .and_then(|tc| tc.data.first().map(|td| tc.mat3_unit * td.axismtx))
                            .unwrap_or(Mat3::IDENTITY)
                    } else {
                        spacemtx
                    };
                    con_axis_index(&c).map_or(Vec3::ZERO, |i| axes.col(i))
                }
                None => -spacemtx.col(t.req.orient_axis),
            };
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
        Mode::EdgeSlide | Mode::VertSlide => {}
    }
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
    let mut orient = Orientation::from_props(
        req.orient_type.as_deref(),
        req.orient_matrix,
        &req.orient_matrix_type,
        &src,
    );
    if orient.kind == OrientType::Normal {
        orient.matrix = c
            .active_object()
            .and_then(|a| {
                let ob = c.main.objects.get(a)?;
                let me = ob.data?.typed::<MeshId>()?;
                let em = edit.meshes.get(&me.key())?;
                Some(normal_orientation(
                    &em.bm,
                    &world_matrix(&c.main.objects, a),
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
    {
        let c = cx.ctx();
        let ts = c.tool_settings();
        if ts.use_mesh_automerge && req.mode.data_kind() == DataKind::Verts {
            return Err(OpError::NotImplemented(
                "frap: edit-mode transforms with tool_settings.use_mesh_automerge (Auto Merge \
                 Vertices) are not implemented yet"
                    .into(),
            ));
        }
    }
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
            let mat = world_matrix(&c.main.objects, *ob);
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
                add_layer: false,
            };
            match t.req.mode.data_kind() {
                DataKind::Edges(name) => create_edges(&mut tc, bm, &t.req, name),
                _ => create_verts(&mut tc, bm, &t.req, around),
            }
            if !tc.data.is_empty() {
                t.tcs.push(tc);
            }
        }
        (t, c.win.view3d.view_matrix)
    };
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

    match t.req.mode {
        Mode::EdgeSlide | Mode::VertSlide => return super::slide::run(cx, t),
        _ => apply(&mut t),
    }
    write_back(cx, &t)
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
                bm.normals_update();
            }
            DataKind::Edges(name) => {
                let col = float_layer(&mut bm.edata, name, bm.edges.slots.len(), tc.add_layer);
                for td in &tc.data {
                    if let Some(e) = td.e {
                        col[e.slot()] = td.val;
                    }
                }
            }
            DataKind::VertCData(name) => {
                let col = float_layer(&mut bm.vdata, name, bm.verts.slots.len(), tc.add_layer);
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

/// The float column named `name`, created (zeroed, `BM_data_layer_add_named`) when missing.
fn float_layer<'a>(
    cd: &'a mut frap_bmesh::CustomData,
    name: &str,
    slots: usize,
    add: bool,
) -> &'a mut Vec<f32> {
    if add || cd.find(name).is_none() {
        cd.add_layer(CdLayer {
            name: name.into(),
            data: AttrData::Float(Arc::new(vec![0.0; slots])),
        });
    }
    cd.ensure_slots(slots);
    let i = cd.find(name).expect("layer just ensured");
    match &mut cd.layers[i].data {
        AttrData::Float(c) => {
            let v = Arc::make_mut(c);
            if v.len() < slots {
                v.resize(slots, 0.0);
            }
            v
        }
        _ => unreachable!("crease and bevel weight layers are FLOAT"),
    }
}
