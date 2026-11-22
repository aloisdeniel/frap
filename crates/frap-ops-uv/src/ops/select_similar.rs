// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.select_similar(type='PIN', compare='EQUAL', threshold=0.0)`, tier T1
//! (`tests/golden/cases/uv/select/similar*.py`). Port of `uv_select_similar_exec`, its
//! vertex/edge/face/island variants, the `get_uv_*_needle` functions and
//! `uv_select_similar_type_itemf` (`editors/uvedit/uvedit_select.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit_space_image`: *headless-extended* (frap runs it as in an Image
//! Editor in UV mode). The UV select mode (with sync selection, the mesh one;
//! `use_uv_select_island`: islands) decides what is compared and which `type`s exist: vertex
//! `PIN`; edge `LENGTH`, `LENGTH_3D`, `PIN`; face `AREA`, `AREA_3D`, `MATERIAL`, `OBJECT`,
//! `SIDES`, `WINDING`; island `AREA`, `AREA_3D`, `FACE` (another one is Blender's `TypeError`).
//! The unselected visible elements of every mesh in Edit Mode with a UV map whose value
//! compares `EQUAL` (within `threshold`), `GREATER` or `LESS` against the values of the
//! selected ones (a one-dimensional kd-tree, see `select/kdtree.rs`) get selected.
//! `threshold` defaults to the tool settings' `select_thresh` (0.01) and is stored there when
//! given (`ts->select_thresh` is shared with `mesh.select_similar`, which must read and write
//! it with the same is-set rule). Without `type` the comparison is `PIN` in every mode (the enum's first item, as in
//! Blender 5.2.4, whose itemf leaves the default out of the face and island lists: there it
//! counts pinned corners / compares 0). Always finishes.

use frap_bmesh::{BMesh, FaceId, LoopId};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, MaterialId, ObjectId};
use frap_scene::Context;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::select_similar::Type;
use frap_uv::param::bli;

use super::select_by_winding::face_area_uv_signed;
use super::unwrap::uvedit;
use crate::select::kdtree::{KdTree1, SimilarCmp};
use crate::select::{Ts, UV_SELECT_EDGE, UV_SELECT_FACE, UvSel, op};

/// `eUVSelectSimilar`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Kind {
    Pin,
    Length,
    Length3d,
    Area,
    Area3d,
    Material,
    Object,
    Sides,
    Winding,
    Face,
}

fn kind_of(t: Type) -> Kind {
    match t {
        Type::Pin => Kind::Pin,
        Type::Length => Kind::Length,
        Type::Length3d => Kind::Length3d,
        Type::Area => Kind::Area,
        Type::Area3d => Kind::Area3d,
        Type::Material => Kind::Material,
        Type::Object => Kind::Object,
        Type::Sides => Kind::Sides,
        Type::Winding => Kind::Winding,
        Type::Face => Kind::Face,
    }
}

/// What the operator compares (`uv_select_similar_exec`'s dispatch).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Domain {
    Vert,
    Edge,
    Face,
    Island,
}

fn domain(ts: &Ts) -> Domain {
    let selectmode = if ts.sync {
        ts.selectmode_bits()
    } else {
        ts.uv_selectmode
    };
    if ts.select_island_check() {
        Domain::Island
    } else if selectmode & UV_SELECT_FACE != 0 {
        Domain::Face
    } else if selectmode & UV_SELECT_EDGE != 0 {
        Domain::Edge
    } else {
        Domain::Vert
    }
}

/// `uv_select_similar_type_itemf`.
fn type_items(c: &Context) -> Vec<frap_op::EnumItemOwned> {
    let ids: &[&str] = match domain(&Ts::of(c)) {
        Domain::Island => &["AREA", "AREA_3D", "FACE"],
        Domain::Face => &["AREA", "AREA_3D", "MATERIAL", "OBJECT", "SIDES", "WINDING"],
        Domain::Edge => &["LENGTH", "LENGTH_3D", "PIN"],
        Domain::Vert => &["PIN"],
    };
    uv::enums::E28
        .iter()
        .filter(|i| ids.contains(&i.identifier))
        .map(|i| frap_op::EnumItemOwned {
            identifier: i.identifier.into(),
            name: i.name.into(),
            description: i.description.into(),
        })
        .collect()
}

/// One object's data for the needles.
struct ObData {
    index: usize,
    m3: [[f32; 3]; 3],
    material_remap: Vec<usize>,
    pins: Vec<bool>,
}

/// `BM_face_calc_area_uv`.
fn area_uv(s: &UvSel, f: FaceId) -> f32 {
    face_area_uv_signed(s, f).abs()
}

/// `BM_face_calc_area_with_mat3`.
fn area_3d(bm: &BMesh, f: FaceId, m3: &[[f32; 3]; 3]) -> f32 {
    let loops = bm.face_loops(f);
    let mut n = [0.0f32; 3];
    let co = |l: LoopId| bli::mul_m3_v3(m3, bm.vert(bm.lp(l).v).co);
    let mut c = co(loops[0]);
    for &l in &loops {
        let c_next = co(bm.lp(l).next);
        bli::add_newell_cross(&mut n, c, c_next);
        c = c_next;
    }
    bli::len3(n) * 0.5
}

fn pin(o: &ObData, l: LoopId) -> bool {
    o.pins.get(l.slot()).copied().unwrap_or(false)
}

/// `get_uv_vert_needle`.
fn vert_needle(kind: Kind, s: &UvSel, o: &ObData, l: LoopId) -> f32 {
    let v = s.lv(l);
    match kind {
        Kind::Area => s
            .bm
            .vert_faces(v)
            .into_iter()
            .fold(0.0, |a, f| a + area_uv(s, f)),
        Kind::Area3d => s
            .bm
            .vert_faces(v)
            .into_iter()
            .fold(0.0, |a, f| a + area_3d(s.bm, f, &o.m3)),
        Kind::Sides => s.bm.vert_edges(v).len() as f32,
        Kind::Pin if pin(o, l) => 1.0,
        _ => 0.0,
    }
}

/// `get_uv_edge_needle` (`loop_b` is `l->next`).
fn edge_needle(kind: Kind, s: &UvSel, o: &ObData, l: LoopId) -> f32 {
    let e = s.le(l);
    let lb = s.next(l);
    match kind {
        Kind::Area => s
            .bm
            .edge_faces(e)
            .into_iter()
            .fold(0.0, |a, f| a + area_uv(s, f)),
        Kind::Area3d => s
            .bm
            .edge_faces(e)
            .into_iter()
            .fold(0.0, |a, f| a + area_3d(s.bm, f, &o.m3)),
        Kind::Length => {
            let (a, b) = (s.uv(l), s.uv(lb));
            let d = [b[0] - a[0], b[1] - a[1]];
            (d[0] * d[0] + d[1] * d[1]).sqrt()
        }
        Kind::Length3d => {
            let [v1, v2] = s.bm.edge(e).v;
            bli::len_v3v3(s.bm.vert(v1).co, s.bm.vert(v2).co)
        }
        Kind::Sides => s.bm.edge_faces(e).len() as f32,
        Kind::Pin => {
            let mut r = 0.0;
            if pin(o, l) {
                r += 1.0;
            }
            if pin(o, lb) {
                r += 1.0;
            }
            r
        }
        _ => 0.0,
    }
}

/// `get_uv_face_needle`.
fn face_needle(kind: Kind, s: &UvSel, o: &ObData, f: FaceId) -> f32 {
    match kind {
        Kind::Area => area_uv(s, f),
        Kind::Area3d => area_3d(s.bm, f, &o.m3),
        Kind::Sides => s.bm.face(f).len as f32,
        Kind::Object => o.index as f32,
        Kind::Pin => s
            .face_loops(f)
            .into_iter()
            .fold(0.0, |a, l| if pin(o, l) { a + 1.0 } else { a }),
        Kind::Material => {
            let mat_nr = s.bm.face(f).mat_nr;
            let i = if mat_nr >= 0 && (mat_nr as usize) < o.material_remap.len() {
                mat_nr as usize
            } else {
                0
            };
            o.material_remap.get(i).copied().unwrap_or(0) as f32
        }
        Kind::Winding => {
            let a = face_area_uv_signed(s, f);
            if a > 0.0 {
                1.0
            } else if a < 0.0 {
                -1.0
            } else {
                0.0
            }
        }
        _ => 0.0,
    }
}

/// `get_uv_island_needle`.
fn island_needle(kind: Kind, s: &UvSel, o: &ObData, faces: &[FaceId]) -> f32 {
    match kind {
        Kind::Area => faces.iter().fold(0.0, |a, &f| a + area_uv(s, f)),
        Kind::Area3d => faces
            .iter()
            .fold(0.0, |a, &f| a + area_3d(s.bm, f, &o.m3)),
        Kind::Face => faces.len() as f32,
        _ => 0.0,
    }
}

/// `copy_m3_m4(ob_m3, ob->object_to_world())`.
fn ob_m3(main: &frap_scene::Main, ob: IdRef<ObjectId>) -> [[f32; 3]; 3] {
    let m = uvedit::object_to_world(main, ob);
    [
        [m[0][0], m[0][1], m[0][2]],
        [m[1][0], m[1][1], m[1][2]],
        [m[2][0], m[2][1], m[2][2]],
    ]
}

/// The per-object data, with the shared material-to-index map (`MATERIAL` only).
fn object_data(
    cx: &mut OpCtx,
    objects: &[(IdRef<ObjectId>, IdRef<frap_rna::MeshId>)],
    kind: Kind,
) -> Vec<ObData> {
    let mut material_map: Vec<Option<IdRef<MaterialId>>> = Vec::new();
    let mut out = Vec::new();
    for (index, &(ob, me)) in objects.iter().enumerate() {
        let main = cx.ctx().main;
        let m3 = ob_m3(main, ob);
        let mut material_remap = Vec::new();
        if kind == Kind::Material
            && let Some(o) = main.objects.get(ob)
        {
            let mats = frap_scene::id::slots::slot_materials(main, o);
            for i in 0..mats.len().max(1) {
                let ma = mats.get(i).copied().flatten();
                let k = match material_map.iter().position(|&x| x == ma) {
                    Some(k) => k,
                    None => {
                        material_map.push(ma);
                        material_map.len() - 1
                    }
                };
                material_remap.push(k);
            }
        }
        let pins = uvedit::edit_bm(cx, me)
            .and_then(|bm| Some(uvedit::read_pins(&bm, uvedit::active_uv(&bm)?)))
            .unwrap_or_default();
        out.push(ObData {
            index,
            m3,
            material_remap,
            pins,
        });
    }
    out
}

/// After selecting: the flush of every variant.
fn flush(s: &mut UvSel) {
    if s.ts.sync {
        if s.sync_valid {
            s.bm_flush_from_loop_verts_only_select();
            s.bm_sync_to_mesh();
        } else {
            s.bm.select_flush();
        }
    } else {
        s.select_flush_from_verts(true);
    }
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    dyn_enums = [("type", type_items)]
)]
fn exec(cx: &mut OpCtx, p: uv::SelectSimilar) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let ts = op::ts(cx);
    let dom = domain(&ts);
    // Unset, `type` is its first item (`PIN`) in every mode, as Blender's enum default.
    let kind = kind_of(p.r#type.value);
    if p.r#type.is_set {
        let items = type_items(&cx.ctx());
        if !items
            .iter()
            .any(|i| i.identifier == p.r#type.value.identifier())
        {
            let list = items
                .iter()
                .map(|i| format!("'{}'", i.identifier))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(OpError::Rna(frap_rna::RnaError::new(
                frap_rna::RnaErrorKind::Enum,
                frap_rna::PyExc::TypeError,
                format!(
                    "Converting py args to operator properties: enum \"{}\" not found in ({list})",
                    p.r#type.value.identifier()
                ),
            )));
        }
    }
    let threshold = if p.threshold.is_set {
        op::set_tool_settings(cx, |t| t.select_thresh = p.threshold.value);
        p.threshold.value
    } else {
        cx.ctx().tool_settings().select_thresh
    };
    let compare = SimilarCmp::of(p.compare.value.identifier());
    let objects = op::objects(cx);
    let data = object_data(cx, &objects, kind);
    let totvertsel = |cx: &mut OpCtx, me| {
        uvedit::edit_bm(cx, me).map_or(0, |bm| bm.selected_counts().0)
    };
    let mut tree = KdTree1::new();
    if dom == Domain::Island {
        let face_selected = !ts.sync;
        let mut islands: Vec<Vec<Vec<FaceId>>> = Vec::new();
        for (i, &(_, me)) in objects.iter().enumerate() {
            let isl = op::read(cx, me, ts, |s| {
                let sel = uvedit::Sel {
                    ts: uvedit::UvTs {
                        sync: s.ts.sync,
                        selectmode: s.ts.selectmode,
                    },
                    bm: s.bm,
                    uvs: &s.uv,
                };
                let islands = uvedit::calc_uv_islands(&sel, face_selected, false, false);
                for faces in &islands {
                    if s.face_select_test(faces[0]) {
                        tree.insert(island_needle(kind, s, &data[i], faces));
                    }
                }
                islands
            });
            islands.push(isl.unwrap_or_default());
        }
        tree.deduplicate();
        tree.balance();
        for (i, &(_, me)) in objects.iter().enumerate() {
            op::edit(cx, me, ts, |s| {
                if s.ts.sync {
                    s.prepare_sync_select();
                }
                let mut changed = false;
                for faces in &islands[i] {
                    if s.face_select_test(faces[0]) {
                        continue;
                    }
                    let needle = island_needle(kind, s, &data[i], faces);
                    if !tree.compare(needle, threshold, compare) {
                        continue;
                    }
                    for &f in faces {
                        s.face_select_set(f, true);
                    }
                    changed = true;
                }
                if changed {
                    flush(s);
                }
            })?;
        }
        return Ok(OpStatus::Finished);
    }
    for (i, &(_, me)) in objects.iter().enumerate() {
        if totvertsel(cx, me) == 0 {
            continue;
        }
        op::read(cx, me, ts, |s| {
            for f in s.faces() {
                if !s.face_visible(f) {
                    continue;
                }
                match dom {
                    Domain::Face => {
                        if s.face_select_test(f) {
                            tree.insert(face_needle(kind, s, &data[i], f));
                        }
                    }
                    Domain::Edge => {
                        for l in s.face_loops(f) {
                            if s.edge_select_test(l) {
                                tree.insert(edge_needle(kind, s, &data[i], l));
                            }
                        }
                    }
                    _ => {
                        for l in s.face_loops(f) {
                            if s.uv_select_test(l) {
                                tree.insert(vert_needle(kind, s, &data[i], l));
                            }
                        }
                    }
                }
            }
        });
    }
    tree.deduplicate();
    tree.balance();
    for (i, &(_, me)) in objects.iter().enumerate() {
        if totvertsel(cx, me) == 0 && !ts.sync {
            continue;
        }
        op::edit(cx, me, ts, |s| {
            if s.ts.sync {
                s.prepare_sync_select();
            }
            let mut changed = false;
            for f in s.faces() {
                if !s.face_visible(f) {
                    continue;
                }
                match dom {
                    Domain::Face => {
                        if s.face_select_test(f) {
                            continue;
                        }
                        let needle = face_needle(kind, s, &data[i], f);
                        if tree.compare(needle, threshold, compare) {
                            s.face_select_set(f, true);
                            changed = true;
                        }
                    }
                    Domain::Edge => {
                        for l in s.face_loops(f) {
                            if s.edge_select_test(l) {
                                continue;
                            }
                            let needle = edge_needle(kind, s, &data[i], l);
                            if tree.compare(needle, threshold, compare) {
                                s.edge_select_set(l, true);
                                changed = true;
                            }
                        }
                    }
                    _ => {
                        for l in s.face_loops(f) {
                            if s.uv_select_test(l) {
                                continue;
                            }
                            let needle = vert_needle(kind, s, &data[i], l);
                            if tree.compare(needle, threshold, compare) {
                                s.uv_select_set(l, true);
                                changed = true;
                            }
                        }
                    }
                }
            }
            if changed {
                flush(s);
            }
        })?;
    }
    Ok(OpStatus::Finished)
}
