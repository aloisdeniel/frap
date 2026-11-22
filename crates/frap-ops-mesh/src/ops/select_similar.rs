// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_similar(type='VERT_NORMAL', compare='EQUAL', threshold=0.0)`, tier T1
//! (`tests/golden/cases/edit/select_m8/similar_*.py`). Port of `edbm_select_similar_exec`,
//! `similar_vert_select_exec`, `similar_edge_select_exec`, `similar_face_select_exec` and
//! `select_similar_type_itemf` (`editors/mesh/editmesh_select_similar.cc`, Blender 5.2.4,
//! GPL-2.0-or-later); the kd-trees are in `kdtree.rs`.
//!
//! The selected elements of every mesh in Edit Mode (one per mesh, active first) give the
//! reference values, then each unselected visible element whose value compares `EQUAL` (within
//! `threshold`), `GREATER` or `LESS` against them is selected and the selection flushed:
//! - vertex types (`VERT_*`): world-space normal (angle within `threshold * π + FLT_EPSILON`),
//!   number of faces or edges, shared vertex groups (by name, weight > 0), `crease_vert`;
//! - edge types (`EDGE_*`): world-space squared length, direction (either way round), number
//!   of faces, face angle (always `EQUAL`), `crease_edge`, `bevel_weight_edge`, seam, sharp,
//!   `freestyle_edge`;
//! - face types (`FACE_*`): material (the mesh's material at the face's index, objects with
//!   material slots only), area and perimeter (in the object's 3×3 world matrix), sides,
//!   world-space normal, plane (`COPLANAR`), smooth flag, `freestyle_face`.
//!
//! The kind of `type` decides what is compared, whatever the select mode: an unset `type` is
//! `VERT_NORMAL`, so a face-mode call without `type` selects vertices (their faces only follow
//! when the select mode flushes them, as in Blender). A `type` outside the select mode's list
//! (`select_similar_type_itemf`) is Blender's `TypeError`. `threshold` defaults to the tool
//! settings' `select_thresh` (0.01) and is stored there when given (shared with
//! `uv.select_similar`). Blender quirks kept: `LESS` on a kd-tree value finds no node (its
//! query is `FLT_MAX`, whose squared distance overflows) and selects nothing; with a mix of
//! seam (sharp, Freestyle) and non-seam edges selected every edge is selected.
//! Without a selected element of the compared kind the operator is `CANCELLED` with Blender's
//! error (`No vertex selected`, `No edge selected`, `No face selected`).

pub(crate) mod kdtree;
pub(crate) mod math;

use std::collections::BTreeSet;
use std::sync::Arc;

use frap_bmesh::{BMesh, EdgeId, ElemFlag, FaceId};
use frap_mesh::AttrData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MaterialId, MeshId, ObjectId};
use frap_scene::Context;
use frap_scene::id::transform::world_matrix_in;
use frap_spec_ops_mesh::mesh;
use frap_spec_ops_mesh::mesh::select_similar::{Compare, Type};
use smol_str::SmolStr;

use self::kdtree::{KdTree, SimilarCmp};
use self::math::{M3, M4, V3};
use super::select_more::common;

/// `FLT_EPSILON`.
const FLT_EPSILON: f32 = f32::EPSILON;
/// `float(M_PI)`.
const PI: f32 = std::f32::consts::PI;

/// The `prop_similar_types` value of a `type` (`SIMVERT_*` < 100 ≤ `SIMEDGE_*` < 200 ≤
/// `SIMFACE_*`).
fn type_value(t: Type) -> i32 {
    mesh::enums::E44
        .iter()
        .find(|i| i.identifier == t.identifier())
        .map_or(0, |i| i.value)
}

/// `select_similar_type_itemf`: the types of the edit object's select mode (vertex, else edge,
/// else face).
fn type_items(c: &Context) -> Vec<frap_op::EnumItemOwned> {
    let mode = c.tool_settings().mesh_select_mode;
    let range = if mode[0] {
        0..100
    } else if mode[1] {
        100..200
    } else if mode[2] {
        200..300
    } else {
        0..0
    };
    mesh::enums::E44
        .iter()
        .filter(|i| range.contains(&i.value))
        .map(|i| frap_op::EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// The `TypeError` of a `type` outside the dynamic items.
fn check_type(cx: &OpCtx, t: &frap_rna::Prop<Type>) -> Result<(), OpError> {
    if !t.is_set {
        return Ok(());
    }
    let items = type_items(&cx.ctx());
    if items.iter().any(|i| i.identifier == t.value.identifier()) {
        return Ok(());
    }
    let list = items
        .iter()
        .map(|i| format!("'{}'", i.identifier))
        .collect::<Vec<_>>()
        .join(", ");
    Err(OpError::Rna(frap_rna::RnaError::new(
        frap_rna::RnaErrorKind::Enum,
        frap_rna::PyExc::TypeError,
        format!(
            "Converting py args to operator properties: enum \"{}\" not found in ({list})",
            t.value.identifier()
        ),
    )))
}

/// `mesh_select_similar_compare_int`.
fn compare_int(delta: i64, compare: SimilarCmp) -> bool {
    match compare {
        SimilarCmp::Eq => delta == 0,
        SimilarCmp::Gt => delta > 0,
        SimilarCmp::Lt => delta < 0,
    }
}

/// `SIM*_DATA_TRUE` / `SIM*_DATA_FALSE`.
const DATA_TRUE: u8 = 1;
const DATA_FALSE: u8 = 2;
const DATA_ALL: u8 = DATA_TRUE | DATA_FALSE;

/// One mesh in Edit Mode with its object's matrices.
struct Ob {
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
    /// `object_to_world`.
    obmat: M4,
    /// `world_to_object` (`invert_m4_m4` of `object_to_world`).
    imat: M4,
}

fn objects(cx: &OpCtx) -> Vec<Ob> {
    let main = cx.main();
    common::edit_meshes(cx)
        .into_iter()
        .map(|(ob, me)| {
            let obmat = math::m4(&world_matrix_in(main, ob));
            Ob {
                ob,
                me,
                obmat,
                imat: math::invert_m4(&obmat),
            }
        })
        .collect()
}

/// A read-only copy of the edit BMesh of `me`.
fn bm_of(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Arc<BMesh>> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| em.bm.clone())
}

fn flag(bm: &BMesh, e: impl Into<frap_bmesh::BMElem>, f: ElemFlag) -> bool {
    bm.elem_flag(e.into()).contains(f)
}

/// The float layer `name` of `cd` (`CustomData_get_offset_named(…, CD_PROP_FLOAT, name)`).
fn float_layer<'a>(cd: &'a frap_bmesh::CustomData, name: &str) -> Option<&'a [f32]> {
    match cd.get(name).map(|l| &l.data) {
        Some(AttrData::Float(v)) => Some(v.as_slice()),
        _ => None,
    }
}

/// The bool layer `name` of `cd` (`CD_PROP_BOOL`).
fn bool_layer<'a>(cd: &'a frap_bmesh::CustomData, name: &str) -> Option<&'a [bool]> {
    match cd.get(name).map(|l| &l.data) {
        Some(AttrData::Bool(v)) => Some(v.as_slice()),
        _ => None,
    }
}

fn at<T: Copy + Default>(v: &[T], slot: usize) -> T {
    v.get(slot).copied().unwrap_or_default()
}

/// Selects `elems` on `me` and flushes (`EDBM_selectmode_flush`) when anything changed.
fn apply(cx: &mut OpCtx, me: IdRef<MeshId>, elems: Vec<frap_bmesh::BMElem>) -> OpResult {
    if elems.is_empty() {
        return Ok(OpStatus::Finished);
    }
    let bm = cx.bmesh_mut(me)?;
    for e in elems {
        bm.select_set(e, true);
    }
    common::selectmode_flush(bm);
    Ok(OpStatus::Finished)
}

/// The `face_select_all` / `edge_select_all` jump: selects every unselected element of every
/// mesh.
fn select_all_of(cx: &mut OpCtx, obs: &[Ob], faces: bool) -> OpResult {
    for o in obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        let elems: Vec<frap_bmesh::BMElem> = if faces {
            bm.face_ids()
                .into_iter()
                .filter(|&f| !flag(&bm, f, ElemFlag::SELECT))
                .map(Into::into)
                .collect()
        } else {
            bm.edge_ids()
                .into_iter()
                .filter(|&e| !flag(&bm, e, ElemFlag::SELECT))
                .map(Into::into)
                .collect()
        };
        let bm = cx.bmesh_mut(o.me)?;
        for e in elems {
            bm.select_set(e, true);
        }
        common::selectmode_flush(bm);
    }
    Ok(OpStatus::Finished)
}

// ------------------------------------------------------------------------------- vertices

/// `similar_vert_select_exec`.
fn similar_vert(cx: &mut OpCtx, ty: Type, thresh: f32, compare: SimilarCmp) -> OpResult {
    let thresh_radians = thresh * PI + FLT_EPSILON;
    let obs = objects(cx);
    let mut tot = 0usize;
    for o in &obs {
        tot += bm_of(cx, o.me).map_or(0, |bm| bm.selected_counts().0);
    }
    if tot == 0 {
        return Err(OpError::Runtime("No vertex selected".into()));
    }
    let mut tree_3d: KdTree<3> = KdTree::new();
    let mut tree_1d: KdTree<1> = KdTree::new();
    let mut groups: BTreeSet<SmolStr> = BTreeSet::new();
    let mut counts: BTreeSet<i64> = BTreeSet::new();
    let group_names = |cx: &OpCtx, me: IdRef<MeshId>| -> Vec<SmolStr> {
        cx.main()
            .meshes
            .get(me)
            .map(|m| m.vertex_group_names.clone())
            .unwrap_or_default()
    };
    for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        if bm.selected_counts().0 == 0 {
            continue;
        }
        let names = group_names(cx, o.me);
        let mut defbase_selected = vec![false; names.len()];
        let crease = float_layer(&bm.vdata, "crease_vert");
        match ty {
            Type::VertGroups => {
                if bm.vdata.deform.is_none() || names.is_empty() {
                    continue;
                }
            }
            Type::VertCrease if crease.is_none() => {
                tree_1d.insert([0.0]);
                continue;
            }
            _ => {}
        }
        for v in bm.vert_ids() {
            if !flag(&bm, v, ElemFlag::SELECT) {
                continue;
            }
            match ty {
                Type::VertFaces => {
                    counts.insert(bm.vert_face_count(v) as i64);
                }
                Type::VertEdges => {
                    counts.insert(bm.vert_edge_count(v) as i64);
                }
                Type::VertNormal => {
                    tree_3d.insert(math::normal_world(&o.imat, bm.vert(v).no));
                }
                Type::VertGroups => {
                    if let Some(dv) = bm.vdata.deform.as_ref().and_then(|d| d.get(v.slot())) {
                        for &(g, w) in &dv.weights {
                            if w > 0.0 && (g as usize) < defbase_selected.len() {
                                defbase_selected[g as usize] = true;
                            }
                        }
                    }
                }
                Type::VertCrease => {
                    tree_1d.insert([at(crease.unwrap_or(&[]), v.slot())]);
                }
                _ => {}
            }
        }
        if ty == Type::VertGroups {
            for (i, n) in names.iter().enumerate() {
                if defbase_selected[i] {
                    groups.insert(n.clone());
                }
            }
        }
    }
    if ty == Type::VertGroups && groups.is_empty() {
        cx.report(
            ReportType::Info,
            "No vertex group among the selected vertices",
        );
    }
    tree_1d.deduplicate();
    tree_1d.balance();
    tree_3d.deduplicate();
    tree_3d.balance();

    for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        let names = group_names(cx, o.me);
        let mut defbase_selected = vec![false; names.len()];
        let crease = float_layer(&bm.vdata, "crease_vert");
        match ty {
            Type::VertGroups => {
                if bm.vdata.deform.is_none() || names.is_empty() {
                    continue;
                }
                let mut found_any = false;
                for name in &groups {
                    // `BKE_defgroup_name_index`: the first group with that name.
                    if let Some(i) = names.iter().position(|n| n == name) {
                        defbase_selected[i] = true;
                        found_any = true;
                    }
                }
                if !found_any {
                    continue;
                }
            }
            // Without a layer every crease is 0: all vertices or none.
            Type::VertCrease if crease.is_none() && !tree_1d.compare(0.0, thresh, compare) => {
                continue;
            }
            _ => {}
        }
        let mut sel = Vec::new();
        for v in bm.vert_ids() {
            if flag(&bm, v, ElemFlag::SELECT) || flag(&bm, v, ElemFlag::HIDDEN) {
                continue;
            }
            let select = match ty {
                Type::VertEdges => {
                    let n = bm.vert_edge_count(v) as i64;
                    counts.iter().any(|&c| compare_int(n - c, compare))
                }
                Type::VertFaces => {
                    let n = bm.vert_face_count(v) as i64;
                    counts.iter().any(|&c| compare_int(n - c, compare))
                }
                Type::VertNormal => {
                    let normal = math::normal_world(&o.imat, bm.vert(v).no);
                    tree_3d.find_nearest(&normal).is_some_and(|n| {
                        math::angle_normalized_v3v3(normal, n.co) <= thresh_radians
                    })
                }
                Type::VertGroups => bm
                    .vdata
                    .deform
                    .as_ref()
                    .and_then(|d| d.get(v.slot()))
                    .is_some_and(|dv| {
                        dv.weights.iter().any(|&(g, w)| {
                            w > 0.0
                                && (g as usize) < defbase_selected.len()
                                && defbase_selected[g as usize]
                        })
                    }),
                Type::VertCrease => match crease {
                    None => true,
                    Some(c) => tree_1d.compare(at(c, v.slot()), thresh, compare),
                },
                _ => false,
            };
            if select {
                sel.push(frap_bmesh::BMElem::Vert(v));
            }
        }
        apply(cx, o.me, sel)?;
    }
    Ok(OpStatus::Finished)
}

// ---------------------------------------------------------------------------------- edges

/// `edge_pos_direction_worldspace_get`.
fn edge_dir_world(bm: &BMesh, obmat: &M4, e: EdgeId) -> V3 {
    let [a, b] = bm.edge(e).v;
    let v1 = math::mul_m4_v3(obmat, bm.vert(a).co);
    let v2 = math::mul_m4_v3(obmat, bm.vert(b).co);
    math::normalize_v3([v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]]).0
}

/// `edge_length_squared_worldspace_get`.
fn edge_len_sq_world(bm: &BMesh, obmat: &M4, e: EdgeId) -> f32 {
    let [a, b] = bm.edge(e).v;
    let v1 = math::mul_mat3_m4_v3(obmat, bm.vert(a).co);
    let v2 = math::mul_mat3_m4_v3(obmat, bm.vert(b).co);
    frap_bmesh::ops::contextual_create::bli::len_squared_v3v3(v1, v2)
}

/// The custom-data layer of an edge type (`crease_edge`, `bevel_weight_edge`).
fn edge_float_layer(ty: Type) -> Option<&'static str> {
    match ty {
        Type::EdgeCrease => Some("crease_edge"),
        Type::EdgeBevel => Some("bevel_weight_edge"),
        _ => None,
    }
}

/// `similar_edge_select_exec`.
fn similar_edge(cx: &mut OpCtx, ty: Type, thresh: f32, compare: SimilarCmp) -> OpResult {
    let thresh_radians = thresh * PI + FLT_EPSILON;
    let obs = objects(cx);
    let mut tot = 0usize;
    for o in &obs {
        tot += bm_of(cx, o.me).map_or(0, |bm| bm.selected_counts().1);
    }
    if tot == 0 {
        return Err(OpError::Runtime("No edge selected".into()));
    }
    let mut tree_1d: KdTree<1> = KdTree::new();
    let mut tree_3d: KdTree<3> = KdTree::new();
    let mut face_counts: BTreeSet<i64> = BTreeSet::new();
    let mut data_value = 0u8;
    let mut select_all = false;
    'objects: for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        if bm.selected_counts().1 == 0 {
            continue;
        }
        let freestyle = bool_layer(&bm.edata, "freestyle_edge");
        let layer = edge_float_layer(ty).and_then(|n| float_layer(&bm.edata, n));
        match ty {
            Type::EdgeFreestyle if freestyle.is_none() => {
                data_value |= DATA_FALSE;
                continue;
            }
            Type::EdgeCrease | Type::EdgeBevel if layer.is_none() => {
                tree_1d.insert([0.0]);
                continue;
            }
            _ => {}
        }
        let imat3 = math::invert_m3(&math::m3_of(&o.obmat));
        for e in bm.edge_ids() {
            if !flag(&bm, e, ElemFlag::SELECT) {
                continue;
            }
            match ty {
                Type::EdgeFaces => {
                    face_counts.insert(bm.edge_face_count(e) as i64);
                }
                Type::EdgeDir => {
                    let dir = edge_dir_world(&bm, &o.obmat, e);
                    tree_3d.insert(dir);
                    tree_3d.insert([-dir[0], -dir[1], -dir[2]]);
                }
                Type::EdgeLength => tree_1d.insert([edge_len_sq_world(&bm, &o.obmat, e)]),
                Type::EdgeFaceAngle => {
                    if bm.edge_face_count(e) >= 2 {
                        tree_1d.insert([math::edge_face_angle_with_imat3(&bm, e, &imat3)]);
                    }
                }
                Type::EdgeSeam | Type::EdgeSharp => {
                    let hflag = if ty == Type::EdgeSeam {
                        ElemFlag::SEAM
                    } else {
                        ElemFlag::SMOOTH
                    };
                    data_value |= if flag(&bm, e, hflag) {
                        DATA_TRUE
                    } else {
                        DATA_FALSE
                    };
                    if data_value == DATA_ALL {
                        select_all = true;
                        break 'objects;
                    }
                }
                Type::EdgeFreestyle => {
                    data_value |= if freestyle.is_some_and(|l| at(l, e.slot())) {
                        DATA_TRUE
                    } else {
                        DATA_FALSE
                    };
                    if data_value == DATA_ALL {
                        select_all = true;
                        break 'objects;
                    }
                }
                Type::EdgeCrease | Type::EdgeBevel => {
                    tree_1d.insert([at(layer.unwrap_or(&[]), e.slot())]);
                }
                _ => {}
            }
        }
    }
    if select_all {
        return select_all_of(cx, &obs, false);
    }
    tree_1d.deduplicate();
    tree_1d.balance();
    tree_3d.deduplicate();
    tree_3d.balance();

    for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        let freestyle = bool_layer(&bm.edata, "freestyle_edge");
        let layer = edge_float_layer(ty).and_then(|n| float_layer(&bm.edata, n));
        match ty {
            Type::EdgeFreestyle if data_value == DATA_TRUE && freestyle.is_none() => continue,
            Type::EdgeCrease | Type::EdgeBevel
                if layer.is_none() && !tree_1d.compare(0.0, thresh, compare) =>
            {
                continue;
            }
            _ => {}
        }
        let imat3 = math::invert_m3(&math::m3_of(&o.obmat));
        let mut sel = Vec::new();
        for e in bm.edge_ids() {
            if flag(&bm, e, ElemFlag::SELECT) || flag(&bm, e, ElemFlag::HIDDEN) {
                continue;
            }
            let select = match ty {
                Type::EdgeFaces => {
                    let n = bm.edge_face_count(e) as i64;
                    face_counts.iter().any(|&c| compare_int(n - c, compare))
                }
                Type::EdgeDir => {
                    let dir = edge_dir_world(&bm, &o.obmat, e);
                    tree_3d.find_nearest(&dir).is_some_and(|n| {
                        math::angle_normalized_v3v3(dir, n.co) <= thresh_radians
                    })
                }
                Type::EdgeLength => {
                    tree_1d.compare(edge_len_sq_world(&bm, &o.obmat, e), thresh, compare)
                }
                Type::EdgeFaceAngle => {
                    bm.edge_face_count(e) >= 2
                        && tree_1d.compare(
                            math::edge_face_angle_with_imat3(&bm, e, &imat3),
                            thresh,
                            SimilarCmp::Eq,
                        )
                }
                Type::EdgeSeam => {
                    flag(&bm, e, ElemFlag::SEAM) == (data_value & DATA_TRUE != 0)
                }
                Type::EdgeSharp => {
                    flag(&bm, e, ElemFlag::SMOOTH) == (data_value & DATA_TRUE != 0)
                }
                Type::EdgeFreestyle => match freestyle {
                    None => true,
                    Some(l) => at(l, e.slot()) == (data_value & DATA_TRUE != 0),
                },
                Type::EdgeCrease | Type::EdgeBevel => match layer {
                    None => true,
                    Some(l) => tree_1d.compare(at(l, e.slot()), thresh, compare),
                },
                _ => false,
            };
            if select {
                sel.push(frap_bmesh::BMElem::Edge(e));
            }
        }
        apply(cx, o.me, sel)?;
    }
    Ok(OpStatus::Finished)
}

// ---------------------------------------------------------------------------------- faces

/// `face_to_plane`: the world-space plane of a face (normalised world normal through its
/// first corner).
fn face_to_plane(bm: &BMesh, o: &Ob, f: FaceId) -> [f32; 4] {
    let normal = math::normal_world(&o.imat, bm.face(f).no);
    let first = bm.lp(bm.face(f).l_first).v;
    let co = math::mul_m4_v3(&o.obmat, bm.vert(first).co);
    math::plane_from_point_normal(co, normal)
}

/// The mesh's material at a face's index (`(*BKE_object_material_array_p(ob))[mat_nr]`), or
/// `None` for an object without material slots (`ob->totcol == 0`).
fn materials_of(cx: &OpCtx, o: &Ob) -> Option<Vec<Option<IdRef<MaterialId>>>> {
    let main = cx.main();
    let ob = main.objects.get(o.ob)?;
    if frap_scene::id::slots::slot_count(main, ob) == 0 {
        return None;
    }
    Some(main.meshes.get(o.me)?.materials.clone())
}

/// `similar_face_select_exec`.
fn similar_face(cx: &mut OpCtx, ty: Type, thresh: f32, compare: SimilarCmp) -> OpResult {
    let thresh_radians = thresh * PI;
    let obs = objects(cx);
    let mut tot = 0usize;
    for o in &obs {
        tot += bm_of(cx, o.me).map_or(0, |bm| bm.selected_counts().2);
    }
    if tot == 0 {
        return Err(OpError::Runtime("No face selected".into()));
    }
    let mut tree_1d: KdTree<1> = KdTree::new();
    let mut tree_3d: KdTree<3> = KdTree::new();
    let mut tree_4d: KdTree<4> = KdTree::new();
    let mut sides: BTreeSet<i64> = BTreeSet::new();
    let mut materials: BTreeSet<IdRef<MaterialId>> = BTreeSet::new();
    let mut data_value = 0u8;
    let mut select_all = false;
    'objects: for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        if bm.selected_counts().2 == 0 {
            continue;
        }
        let ob_m3: M3 = math::m3_of(&o.obmat);
        let mats = materials_of(cx, o);
        let freestyle = bool_layer(&bm.pdata, "freestyle_face");
        match ty {
            Type::FaceMaterial if mats.is_none() => continue,
            Type::FaceFreestyle if freestyle.is_none() => {
                data_value |= DATA_FALSE;
                continue;
            }
            _ => {}
        }
        for f in bm.face_ids() {
            if !flag(&bm, f, ElemFlag::SELECT) {
                continue;
            }
            match ty {
                Type::FaceSides => {
                    sides.insert(i64::from(bm.face(f).len));
                }
                Type::FaceMaterial => {
                    let nr = bm.face(f).mat_nr.max(0) as usize;
                    if let Some(Some(m)) = mats.as_ref().and_then(|v| v.get(nr)) {
                        materials.insert(*m);
                    }
                }
                Type::FaceArea => tree_1d.insert([math::face_area_with_mat3(&bm, f, &ob_m3)]),
                Type::FacePerimeter => {
                    tree_1d.insert([math::face_perimeter_with_mat3(&bm, f, &ob_m3)]);
                }
                Type::FaceNormal => tree_3d.insert(math::normal_world(&o.imat, bm.face(f).no)),
                Type::FaceCoplanar => tree_4d.insert(face_to_plane(&bm, o, f)),
                Type::FaceSmooth => {
                    data_value |= if flag(&bm, f, ElemFlag::SMOOTH) {
                        DATA_TRUE
                    } else {
                        DATA_FALSE
                    };
                    if data_value == DATA_ALL {
                        select_all = true;
                        break 'objects;
                    }
                }
                Type::FaceFreestyle => {
                    data_value |= if freestyle.is_some_and(|l| at(l, f.slot())) {
                        DATA_TRUE
                    } else {
                        DATA_FALSE
                    };
                    if data_value == DATA_ALL {
                        select_all = true;
                        break 'objects;
                    }
                }
                _ => {}
            }
        }
    }
    if select_all {
        return select_all_of(cx, &obs, true);
    }
    tree_1d.deduplicate();
    tree_1d.balance();
    tree_3d.deduplicate();
    tree_3d.balance();
    tree_4d.deduplicate();
    tree_4d.balance();

    for o in &obs {
        let Some(bm) = bm_of(cx, o.me) else { continue };
        let ob_m3: M3 = math::m3_of(&o.obmat);
        let mats = materials_of(cx, o);
        let freestyle = bool_layer(&bm.pdata, "freestyle_face");
        match ty {
            Type::FaceMaterial if mats.is_none() => continue,
            Type::FaceFreestyle if data_value == DATA_TRUE && freestyle.is_none() => continue,
            _ => {}
        }
        let mut sel = Vec::new();
        for f in bm.face_ids() {
            if flag(&bm, f, ElemFlag::SELECT) || flag(&bm, f, ElemFlag::HIDDEN) {
                continue;
            }
            let select = match ty {
                Type::FaceSides => {
                    let n = i64::from(bm.face(f).len);
                    sides.iter().any(|&c| compare_int(n - c, compare))
                }
                Type::FaceMaterial => {
                    let nr = bm.face(f).mat_nr.max(0) as usize;
                    match mats.as_ref().and_then(|v| v.get(nr)).copied().flatten() {
                        Some(m) => materials.contains(&m),
                        None => continue,
                    }
                }
                Type::FaceArea => {
                    tree_1d.compare(math::face_area_with_mat3(&bm, f, &ob_m3), thresh, compare)
                }
                Type::FacePerimeter => tree_1d.compare(
                    math::face_perimeter_with_mat3(&bm, f, &ob_m3),
                    thresh,
                    compare,
                ),
                Type::FaceNormal => {
                    let normal = math::normal_world(&o.imat, bm.face(f).no);
                    tree_3d.find_nearest(&normal).is_some_and(|n| {
                        math::angle_normalized_v3v3(normal, n.co) <= thresh_radians
                    })
                }
                Type::FaceCoplanar => {
                    let plane = face_to_plane(&bm, o, f);
                    tree_4d.find_nearest(&plane).is_some_and(|n| {
                        n.dist <= thresh
                            && (plane[3] - n.co[3]).abs() <= thresh
                            && math::angle_v3v3(
                                [plane[0], plane[1], plane[2]],
                                [n.co[0], n.co[1], n.co[2]],
                            ) <= thresh_radians
                    })
                }
                Type::FaceSmooth => {
                    flag(&bm, f, ElemFlag::SMOOTH) == (data_value & DATA_TRUE != 0)
                }
                Type::FaceFreestyle => match freestyle {
                    None => true,
                    Some(l) => at(l, f.slot()) == (data_value & DATA_TRUE != 0),
                },
                _ => false,
            };
            if select {
                sel.push(frap_bmesh::BMElem::Face(f));
            }
        }
        apply(cx, o.me, sel)?;
    }
    Ok(OpStatus::Finished)
}

/// Writes the context scene's tool settings.
pub(crate) fn set_select_thresh(cx: &mut OpCtx, value: f32) {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.tool_settings.select_thresh = value;
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    dyn_enums = [("type", type_items)],
    notes = [
        "the kind of type decides what is compared, whatever the select mode: an unset type is VERT_NORMAL, which in face mode selects vertices as in Blender",
        "threshold defaults to the tool settings' select_thresh (0.01) and is stored there when given (shared with uv.select_similar)",
        "Blender's LESS on area, perimeter, length, crease and bevel weight finds no node in its kd-tree and selects nothing (reproduced)",
    ]
)]
fn exec(cx: &mut OpCtx, p: mesh::SelectSimilar) -> OpResult {
    check_type(cx, &p.r#type)?;
    let thresh = if p.threshold.is_set {
        set_select_thresh(cx, p.threshold.value);
        p.threshold.value
    } else {
        cx.ctx().tool_settings().select_thresh
    };
    let compare = match p.compare.value {
        Compare::Equal => SimilarCmp::Eq,
        Compare::Greater => SimilarCmp::Gt,
        Compare::Less => SimilarCmp::Lt,
    };
    let ty = p.r#type.value;
    let value = type_value(ty);
    if value < 100 {
        similar_vert(cx, ty, thresh, compare)
    } else if value < 200 {
        similar_edge(cx, ty, thresh, compare)
    } else {
        similar_face(cx, ty, thresh, compare)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_values_split_by_kind() {
        assert_eq!(type_value(Type::VertNormal), 0);
        assert_eq!(type_value(Type::VertCrease), 4);
        assert_eq!(type_value(Type::EdgeLength), 101);
        assert_eq!(type_value(Type::EdgeFreestyle), 109);
        assert_eq!(type_value(Type::FaceMaterial), 201);
        assert_eq!(type_value(Type::FaceFreestyle), 208);
    }

    #[test]
    fn int_compare() {
        assert!(compare_int(0, SimilarCmp::Eq));
        assert!(compare_int(1, SimilarCmp::Gt) && !compare_int(0, SimilarCmp::Gt));
        assert!(compare_int(-1, SimilarCmp::Lt) && !compare_int(1, SimilarCmp::Lt));
    }
}
