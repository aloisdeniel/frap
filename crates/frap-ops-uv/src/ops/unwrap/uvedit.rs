// SPDX-License-Identifier: GPL-3.0-or-later
//! What the `uv.*` operators share: the operator-pipeline glue of `uvedit_unwrap_ops.cc`,
//! `uvedit_islands.cc` and `uvedit_select.cc` on the edit BMesh, feeding `frap-uv`.
//!
//! Ported from blender/source/blender/editors/uvedit/{uvedit_unwrap_ops.cc,uvedit_islands.cc,
//! uvedit_select.cc,uvedit_ops.cc}, blender/source/blender/editors/mesh/mesh_data.cc
//! (`ED_mesh_uv_add`, `mesh_uv_reset_array`) and blender/source/blender/bmesh/intern/
//! (`BM_mesh_calc_face_groups`, `BM_loop_uv_share_*_check`, `BM_mesh_active_face_get`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! - **UV data.** The active UV map is `CustomData_get_active_layer(ldata, CD_PROP_FLOAT2)`:
//!   the BMesh's `uv_active` layer, else its first `FLOAT2` corner layer. Operators copy the
//!   column (indexed by loop slot), work on the copy and write it back ([`write_uvs`]).
//! - **Selection** ([`Sel`]) follows `uvedit_*_select_test_ex` with `bm->uv_select_sync_valid`
//!   false: frap has no operator that makes the synced UV selection valid (the UV-editor
//!   selection operators are M8), and the factory `uv_sticky_select_mode` (`SHARED_LOCATION`)
//!   and `uv_select_mode` (`VERTEX`), which frap does not store.
//! - **Tool settings.** `unwrap_options_get`'s DNA fields (`unwrapper`, `uvcalc_flag`,
//!   `uvcalc_margin`, …) are `ToolSettings` fields without an RNA property (M3 W2).
//! - **Aspect** (`correct_aspect`): the image of the material's active texture node
//!   (`node_get_active_texture`). frap stores no `NODE_ACTIVE_TEXTURE` flag, so the node is the
//!   one Blender picks when no node carries it (the first texture node, looking into groups);
//!   `nodes.new()` never sets the flag.

use std::sync::Arc;

use frap_bmesh::{BMElem, BMesh, CdLayer, ElemFlag, FaceId, LoopId, SelectMode, VertId};
use frap_mesh::AttrData;
use frap_op::OpCtx;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::PathBase;
use frap_scene::{Context, Main};
use frap_uv::param::bli;
use frap_uv::{ConstructOpts, PackIsland, PackParams, ParamFace, ParamMesh, PinMethod};
use smol_str::SmolStr;

/// A UV coordinate.
pub(crate) type Uv = [f32; 2];

// `eTool_UvCalcFlag` bits read by the operators.
/// `UVCALC_FILLHOLES`
pub(crate) const UVCALC_FILLHOLES: i32 = 1 << 0;
/// `UVCALC_NO_ASPECT_CORRECT`
pub(crate) const UVCALC_NO_ASPECT_CORRECT: i32 = 1 << 1;
/// `UVCALC_USESUBSURF`
pub(crate) const UVCALC_USESUBSURF: i32 = 1 << 3;
/// `UVCALC_UNWRAP_NO_FLIP`
pub(crate) const UVCALC_UNWRAP_NO_FLIP: i32 = 1 << 6;
/// `UVCALC_UNWRAP_USE_WEIGHTS`
pub(crate) const UVCALC_UNWRAP_USE_WEIGHTS: i32 = 1 << 7;

/// `IMG_SIZE_FALLBACK`: the size of an image without pixels.
const IMG_SIZE_FALLBACK: i32 = 256;

// ------------------------------------------------------------------------------- objects

/// `BKE_view_layer_array_from_objects_in_edit_mode_unique_data`: the meshes in Edit Mode
/// (unique data, active first).
pub(crate) fn edit_objects(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    helpers::iter_edit_meshes(cx)
}

/// The edit BMesh of `me` (shared, read-only).
pub(crate) fn edit_bm(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<Arc<BMesh>> {
    let (_, edit) = cx.split_edit();
    edit.meshes.get(&me.key()).map(|em| em.bm.clone())
}

/// `..._unique_data_with_uvs`: the meshes in Edit Mode with a UV map.
pub(crate) fn edit_objects_with_uvs(cx: &mut OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    edit_objects(cx)
        .into_iter()
        .filter(|&(_, me)| edit_bm(cx, me).is_some_and(|bm| active_uv(&bm).is_some()))
        .collect()
}

/// The poll failure Blender raises (`Operator bpy.ops.<op>.poll() failed, context is
/// incorrect`), for the parts of `ED_operator_uvedit` / `ED_operator_uvmap` the declarative
/// poll cannot see (the edit BMesh has faces, a UV map).
pub(crate) fn poll_failed(cx: &OpCtx, hint: &str) -> OpError {
    let msg = format!(
        "Operator bpy.ops.{}.poll() failed, context is incorrect",
        cx.op().idname
    );
    OpError::Frap(Box::new(
        frap_rna::FrapError::new(
            frap_rna::PyExc::RuntimeError,
            frap_op::errors::OP_POLL_FAILED,
            msg,
        )
        .with_detail("operator", cx.op().idname)
        .with_hint(hint),
    ))
}

/// The edit object of the context (`CTX_data_edit_object`) and its mesh.
fn edit_object(cx: &OpCtx) -> Option<(IdRef<ObjectId>, IdRef<MeshId>)> {
    let c = cx.ctx();
    let (ob, o) = poll::edit_object(&c)?;
    Some((ob, o.data?.typed::<MeshId>()?))
}

/// `ED_operator_uvmap`: the edit object is a mesh with faces.
pub(crate) fn check_uvmap(cx: &mut OpCtx) -> Result<(), OpError> {
    let ok = edit_object(cx)
        .and_then(|(_, me)| edit_bm(cx, me))
        .is_some_and(|bm| bm.faces_num() > 0);
    if ok {
        Ok(())
    } else {
        Err(poll_failed(
            cx,
            "the mesh in Edit Mode has no faces: add some, e.g. bpy.ops.mesh.primitive_cube_add()",
        ))
    }
}

/// `ED_operator_uvedit` (`ED_space_image_show_uvedit` without an Image Editor):
/// `EDBM_uv_check`, the edit object is a mesh with faces and a UV map.
pub(crate) fn check_uvedit(cx: &mut OpCtx) -> Result<(), OpError> {
    let ok = edit_object(cx)
        .and_then(|(_, me)| edit_bm(cx, me))
        .is_some_and(|bm| bm.faces_num() > 0 && active_uv(&bm).is_some());
    if ok {
        Ok(())
    } else {
        Err(poll_failed(
            cx,
            "the mesh in Edit Mode needs faces and a UV map: bpy.ops.mesh.uv_texture_add() or \
             bpy.ops.uv.smart_project()",
        ))
    }
}

/// The world matrix of `ob` in Blender's `float[4][4]` layout (`m[col][row]`).
pub(crate) fn object_to_world(main: &Main, ob: IdRef<ObjectId>) -> [[f32; 4]; 4] {
    frap_scene::id::transform::world_matrix_in(main, ob).to_cols_array_2d()
}

/// The image path base of the session (for image sizes).
pub(crate) fn path_base(cx: &mut OpCtx) -> PathBase {
    let (h, _) = cx.split_eval();
    PathBase::of(h)
}

// ------------------------------------------------------------------------------- layers

/// `CustomData_get_active_layer(ldata, CD_PROP_FLOAT2)`: the position of the active UV map.
pub(crate) fn active_uv(bm: &BMesh) -> Option<usize> {
    let float2 = |l: &CdLayer| matches!(l.data, AttrData::Float2(_)) && !l.name.starts_with('.');
    if let Some(a) = &bm.ldata.uv_active
        && let Some(i) = bm.ldata.find(a)
        && float2(&bm.ldata.layers[i])
    {
        return Some(i);
    }
    bm.ldata.layers.iter().position(float2)
}

/// The name of the UV map at layer `uv`.
pub(crate) fn uv_name(bm: &BMesh, uv: usize) -> SmolStr {
    bm.ldata.layers[uv].name.clone()
}

/// `BKE_uv_map_pin_name_get`.
pub(crate) fn pin_name(uv: &str) -> SmolStr {
    frap_bmesh::convert::uv_pin_name(uv)
}

/// The UV column of layer `uv` (by loop slot).
pub(crate) fn read_uvs(bm: &BMesh, uv: usize) -> Vec<Uv> {
    match &bm.ldata.layers[uv].data {
        AttrData::Float2(v) => {
            let mut v = v.as_ref().clone();
            v.resize(bm.loops.capacity().max(v.len()), [0.0; 2]);
            v
        }
        _ => vec![[0.0; 2]; bm.loops.capacity()],
    }
}

/// Writes the UV column of the layer named `name`.
pub(crate) fn write_uvs(bm: &mut BMesh, name: &str, uvs: Vec<Uv>) {
    if let Some(i) = bm.ldata.find(name)
        && let AttrData::Float2(v) = &mut bm.ldata.layers[i].data
    {
        *v = Arc::new(uvs);
    }
}

/// The pin flags (`.pn.<uv>`) of the active UV map by loop slot (all false without the layer).
pub(crate) fn read_pins(bm: &BMesh, uv: usize) -> Vec<bool> {
    let n = bm.loops.capacity();
    match bm.ldata.get(&pin_name(&uv_name(bm, uv))).map(|l| &l.data) {
        Some(AttrData::Bool(v)) => {
            let mut v = v.as_ref().clone();
            v.resize(n.max(v.len()), false);
            v
        }
        _ => vec![false; n],
    }
}

/// Every face in iteration order (`BM_FACES_OF_MESH`).
pub(crate) fn faces(bm: &BMesh) -> Vec<FaceId> {
    bm.face_ids()
}

/// `BM_elem_index_get(v)` after `BM_mesh_elem_index_ensure(bm, BM_VERT)`, by vertex slot.
pub(crate) fn vert_indices(bm: &BMesh, offset: u32) -> Vec<u32> {
    let mut idx = vec![u32::MAX; bm.verts.capacity()];
    for (i, v) in bm.vert_ids().into_iter().enumerate() {
        idx[v.slot()] = offset + i as u32;
    }
    idx
}

/// The coordinate of the vertex of loop `l`.
pub(crate) fn loop_co(bm: &BMesh, l: LoopId) -> [f32; 3] {
    bm.vert(bm.lp(l).v).co
}

/// `BM_face_calc_area`.
pub(crate) fn face_area(bm: &BMesh, f: FaceId) -> f32 {
    let mut n = [0.0f32; 3];
    for l in bm.face_loops(f) {
        bli::add_newell_cross(&mut n, loop_co(bm, l), loop_co(bm, bm.lp(l).next));
    }
    bli::len3(n) * 0.5
}

// ------------------------------------------------------------------------------- selection

/// The tool settings the UV selection tests read.
#[derive(Copy, Clone, Debug)]
pub(crate) struct UvTs {
    /// `uv_flag & UV_FLAG_SELECT_SYNC` (`use_uv_select_sync`).
    pub sync: bool,
    /// `ts->selectmode` (`mesh_select_mode`).
    pub selectmode: SelectMode,
}

impl UvTs {
    /// The context scene's.
    pub(crate) fn of(c: &Context) -> UvTs {
        let ts = c.tool_settings();
        let mut m = SelectMode::empty();
        for (on, bit) in
            ts.mesh_select_mode
                .iter()
                .zip([SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE])
        {
            if *on {
                m |= bit;
            }
        }
        UvTs {
            sync: ts.use_uv_select_sync,
            selectmode: m,
        }
    }
}

/// The UV selection of one BMesh (`uvedit_*_select_test_ex`).
pub(crate) struct Sel<'a> {
    /// Tool settings.
    pub ts: UvTs,
    /// The edit mesh.
    pub bm: &'a BMesh,
    /// The active UV map's column (by loop slot); empty when the mesh has none.
    pub uvs: &'a [Uv],
}

impl Sel<'_> {
    fn fflag(&self, f: FaceId, flag: ElemFlag) -> bool {
        self.bm.face(f).flag.contains(flag)
    }
    fn lflag(&self, l: LoopId, flag: ElemFlag) -> bool {
        self.bm.lp(l).flag.contains(flag)
    }
    fn uv(&self, l: LoopId) -> Uv {
        self.uvs.get(l.slot()).copied().unwrap_or([0.0; 2])
    }

    /// `uvedit_face_visible_test_ex`.
    pub(crate) fn face_visible(&self, f: FaceId) -> bool {
        if self.ts.sync {
            return !self.fflag(f, ElemFlag::HIDDEN);
        }
        !self.fflag(f, ElemFlag::HIDDEN) && self.fflag(f, ElemFlag::SELECT)
    }

    /// `uvedit_face_select_test_ex`.
    pub(crate) fn face_select(&self, f: FaceId) -> bool {
        if self.ts.sync {
            return self.fflag(f, ElemFlag::SELECT);
        }
        // `uv_selectmode == UV_SELECT_VERT` (factory).
        self.bm
            .face_loops(f)
            .into_iter()
            .all(|l| self.lflag(l, ElemFlag::SELECT_UV))
    }

    /// `uvedit_edge_select_test_ex`.
    pub(crate) fn edge_select(&self, l: LoopId) -> bool {
        let bm = self.bm;
        if self.ts.sync {
            let mode = self.ts.selectmode;
            if mode == SelectMode::FACE {
                // `UV_STICKY_LOCATION`.
                return self.fflag(bm.lp(l).f, ElemFlag::SELECT)
                    || self.edge_is_face_select_any_other(l);
            }
            if mode.contains(SelectMode::FACE) {
                return self.fflag(bm.lp(l).f, ElemFlag::SELECT);
            }
            if mode - SelectMode::FACE == SelectMode::EDGE {
                return bm.edge(bm.lp(l).e).flag.contains(ElemFlag::SELECT);
            }
            return bm.vert(bm.lp(l).v).flag.contains(ElemFlag::SELECT)
                && bm
                    .vert(bm.lp(bm.lp(l).next).v)
                    .flag
                    .contains(ElemFlag::SELECT);
        }
        self.lflag(l, ElemFlag::SELECT_UV) && self.lflag(bm.lp(l).next, ElemFlag::SELECT_UV)
    }

    /// `uvedit_uv_select_test_ex`.
    pub(crate) fn uv_select(&self, l: LoopId) -> bool {
        let bm = self.bm;
        if self.ts.sync {
            let mode = self.ts.selectmode;
            if mode == SelectMode::FACE {
                return self.fflag(bm.lp(l).f, ElemFlag::SELECT)
                    || self.vert_is_face_select_any_other(l);
            }
            if mode - SelectMode::FACE == SelectMode::EDGE {
                return self.fflag(bm.lp(l).f, ElemFlag::SELECT)
                    || self.vert_is_edge_select_any_other(l);
            }
            if mode.contains(SelectMode::FACE) {
                return self.fflag(bm.lp(l).f, ElemFlag::SELECT);
            }
            return bm.vert(bm.lp(l).v).flag.contains(ElemFlag::SELECT);
        }
        self.lflag(l, ElemFlag::SELECT_UV)
    }

    /// `BM_loop_uv_share_edge_check`.
    pub(crate) fn share_edge(&self, la: LoopId, lb: LoopId) -> bool {
        share_edge(self.bm, self.uvs, la, lb)
    }

    /// `BM_loop_uv_share_vert_check`.
    fn share_vert(&self, la: LoopId, lb: LoopId) -> bool {
        self.uv(la) == self.uv(lb)
    }

    /// `uvedit_edge_is_face_select_any_other`.
    fn edge_is_face_select_any_other(&self, l: LoopId) -> bool {
        let bm = self.bm;
        let mut r = bm.lp(l).radial_next;
        if r == l {
            return false;
        }
        loop {
            let f = bm.lp(r).f;
            if self.face_visible(f) && self.share_edge(l, r) && self.face_select(f) {
                return true;
            }
            r = bm.lp(r).radial_next;
            if r == l {
                return false;
            }
        }
    }

    /// `uvedit_vert_is_face_select_any_other`.
    fn vert_is_face_select_any_other(&self, l: LoopId) -> bool {
        let bm = self.bm;
        let f0 = bm.lp(l).f;
        bm.vert_loops(bm.lp(l).v).into_iter().any(|li| {
            let f = bm.lp(li).f;
            self.face_visible(f) && f != f0 && self.share_vert(l, li) && self.face_select(f)
        })
    }

    /// `uvedit_vert_is_edge_select_any_other`.
    fn vert_is_edge_select_any_other(&self, l: LoopId) -> bool {
        let bm = self.bm;
        let v = bm.lp(l).v;
        let e0 = bm.lp(l).e;
        let mut e = e0;
        loop {
            if let Some(first) = bm.edge(e).l {
                let mut r = first;
                loop {
                    if self.face_visible(bm.lp(r).f) {
                        let other = if bm.lp(r).v != v { bm.lp(r).next } else { r };
                        if self.share_vert(l, other) && self.edge_select(r) {
                            return true;
                        }
                    }
                    r = bm.lp(r).radial_next;
                    if r == first {
                        break;
                    }
                }
            }
            e = bm.disk_edge_next(e, v);
            if e == e0 {
                return false;
            }
        }
    }
}

/// `BM_loop_uv_share_edge_check`: `la` and `lb` (on the same edge) have the same UVs at both
/// ends.
pub(crate) fn share_edge(bm: &BMesh, uvs: &[Uv], la: LoopId, lb: LoopId) -> bool {
    let g = |l: LoopId| uvs.get(l.slot()).copied().unwrap_or([0.0; 2]);
    let (mut b_curr, mut b_next) = (g(lb), g(bm.lp(lb).next));
    if bm.lp(la).v != bm.lp(lb).v {
        core::mem::swap(&mut b_curr, &mut b_next);
    }
    g(la) == b_curr && g(bm.lp(la).next) == b_next
}

// ------------------------------------------------------------------------------- UV maps

/// `uvedit_ensure_uvs`: adds a UV map to a mesh with faces and none (`ED_mesh_uv_add(mesh,
/// nullptr, true, true)`: named `UVMap`, made unique; the selected faces get the default
/// layout, the others (0, 0); the pin layers are ensured), then UV-selects every face, loop
/// and loop edge. Returns `false` for a mesh without faces.
pub(crate) fn ensure_uvs(cx: &mut OpCtx, me: IdRef<MeshId>) -> Result<bool, OpError> {
    let Some(bm) = edit_bm(cx, me) else {
        return Ok(false);
    };
    if active_uv(&bm).is_some() {
        return Ok(true);
    }
    if bm.faces_num() == 0 {
        return Ok(false);
    }
    let vgroups = cx
        .main()
        .meshes
        .get(me)
        .map(|m| m.vertex_group_names.clone())
        .unwrap_or_default();
    let name = {
        let taken = |n: &str| {
            frap_bmesh::convert::stored_in_bmesh_builtin(n)
                || [&bm.vdata, &bm.edata, &bm.ldata, &bm.pdata]
                    .iter()
                    .any(|cd| cd.find(n).is_some())
                || vgroups.iter().any(|g| g == n)
        };
        frap_rna::naming::unique_subname(&taken, "UVMap", 64, "Attribute")
    };
    let bm = cx.bmesh_mut(me)?;
    let n = bm.loops.capacity();
    bm.ldata.add_layer(CdLayer {
        name: name.clone(),
        data: AttrData::Float2(Arc::new(vec![[0.0; 2]; n])),
    });
    // `BM_uv_map_attr_pin_ensure_for_all_layers`.
    for uv in bm.ldata.float2_names() {
        if uv.starts_with('.') {
            continue;
        }
        let pin = pin_name(&uv);
        if bm.ldata.find(&pin).is_none() {
            bm.ldata.add_layer(CdLayer {
                name: pin,
                data: AttrData::Bool(Arc::new(vec![false; n])),
            });
        }
    }
    // `reset_uv_map`: the selected faces.
    let mut uvs = vec![[0.0f32; 2]; n];
    for f in bm.face_ids() {
        if !bm.face(f).flag.contains(ElemFlag::SELECT) {
            continue;
        }
        let loops = bm.face_loops(f);
        for (l, uv) in loops.iter().zip(reset_face_uvs(loops.len())) {
            uvs[l.slot()] = uv;
        }
    }
    write_uvs(bm, &name, uvs);
    // `uv_maps_active_set`: the new map is active (and the render map when there is none).
    bm.ldata.uv_active = Some(name.clone());
    let render_ok = bm
        .ldata
        .uv_render
        .as_ref()
        .is_some_and(|r| bm.ldata.find(r).is_some());
    if !render_ok {
        bm.ldata.uv_render = Some(name.clone());
    }
    // Select the new UVs.
    for f in bm.face_ids() {
        bm.face_mut(f).flag.insert(ElemFlag::SELECT_UV);
        for l in bm.face_loops(f) {
            bm.lp_mut(l)
                .flag
                .insert(ElemFlag::SELECT_UV | ElemFlag::SELECT_UV_EDGE);
        }
    }
    let render = bm.ldata.uv_render.clone();
    let m = cx.mesh_mut(me)?;
    m.uv_active = Some(name);
    m.uv_active_render = render;
    Ok(true)
}

/// `mesh_uv_reset_array`: the default UVs of a face with `len` corners.
pub(crate) fn reset_face_uvs(len: usize) -> Vec<Uv> {
    match len {
        3 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        4 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        n if n > 2 => {
            let mut fac = 0.0f32;
            let dfac = (1.0f32 / n as f32) * (core::f32::consts::PI * 2.0);
            (0..n)
                .map(|_| {
                    let uv = [0.5 * libm::sinf(fac) + 0.5, 0.5 * libm::cosf(fac) + 0.5];
                    fac += dfac;
                    uv
                })
                .collect()
        }
        n => vec![[0.0, 0.0]; n],
    }
}

// ------------------------------------------------------------------------------- aspect

/// `ED_uvedit_get_aspect_from_material`: `(aspx, aspy)` of the image of the active texture
/// node of material slot `material_index` (1, 1 without one).
pub(crate) fn aspect_from_material(
    main: &Main,
    base: &PathBase,
    ob: IdRef<ObjectId>,
    material_index: i32,
) -> (f32, f32) {
    let Some(o) = main.objects.get(ob) else {
        return (1.0, 1.0);
    };
    let totcol = frap_scene::id::slots::slot_count(main, o);
    if material_index < 0 || material_index as usize >= totcol {
        return (1.0, 1.0);
    }
    let ma = frap_scene::id::slots::slot(main, o, material_index as usize)
        .1
        .and_then(|m| main.materials.get(m));
    let Some(ma) = ma else {
        return (1.0, 1.0);
    };
    // `bke::node_get_active_texture` without `NODE_ACTIVE`/`NODE_ACTIVE_TEXTURE` flags (frap
    // stores none): the first texture-class node of the tree, else (depth first) of the trees
    // of its group nodes.
    let is_texture = |bl: &str| bl.starts_with("ShaderNodeTex");
    let is_group = |bl: &str| bl == "ShaderNodeGroup";
    let mut stack = vec![(&ma.node_tree, 0usize)];
    let mut node = None;
    while let Some((t, i)) = stack.pop() {
        if i == 0
            && let Some(n) = t.nodes.iter().find(|n| is_texture(n.bl_idname))
        {
            node = Some(n);
            break;
        }
        let next = t
            .nodes
            .iter()
            .enumerate()
            .skip(i)
            .find(|(_, n)| is_group(n.bl_idname));
        if let Some((j, n)) = next {
            stack.push((t, j + 1));
            let sub = match n.props.values.get("node_tree") {
                Some(Value::Id(Some(id))) => id
                    .typed::<frap_rna::NodeTreeId>()
                    .and_then(|g| main.node_groups.get(g)),
                _ => None,
            };
            if let Some(sub) = sub
                && stack.len() < 32
            {
                stack.push((sub, 0));
            }
        }
    }
    let image = node
        .filter(|n| {
            matches!(
                n.bl_idname,
                "ShaderNodeTexImage" | "ShaderNodeTexEnvironment"
            )
        })
        .and_then(|n| match n.props.values.get("image") {
            Some(Value::Id(Some(id))) => id.typed::<frap_rna::ImageId>(),
            _ => None,
        })
        .and_then(|i| main.images.get(i));
    let Some(image) = image else {
        return (1.0, 1.0);
    };
    // `ED_image_get_uv_aspect`: `BKE_image_get_aspect` (frap images have no display aspect:
    // 1) times `BKE_image_get_size`.
    let (w, h) = match image.info(base) {
        Some(i) if i.x > 0 && i.y > 0 => (i.x as i32, i.y as i32),
        _ => (IMG_SIZE_FALLBACK, IMG_SIZE_FALLBACK),
    };
    (w as f32, h as f32)
}

/// `BM_mesh_active_face_get(bm, is_sloppy, is_selected)`.
pub(crate) fn active_face(bm: &BMesh, is_sloppy: bool, is_selected: bool) -> Option<FaceId> {
    if let Some(f) = bm.act_face
        && bm.elem_alive(BMElem::Face(f))
        && (!is_selected || bm.face(f).flag.contains(ElemFlag::SELECT))
    {
        return Some(f);
    }
    if !is_sloppy {
        return None;
    }
    let mut found = None;
    for e in bm.select_history.iter().rev() {
        if let BMElem::Face(f) = *e {
            let fl = bm.face(f).flag;
            if fl.contains(ElemFlag::HIDDEN) || (is_selected && !fl.contains(ElemFlag::SELECT)) {
                found = None;
            } else {
                found = Some(f);
                break;
            }
        }
    }
    found.or_else(|| {
        bm.face_ids()
            .into_iter()
            .find(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
    })
}

/// `ED_uvedit_get_aspect_y`: the aspect of the material of the active face.
pub(crate) fn aspect_y(main: &Main, base: &PathBase, ob: IdRef<ObjectId>, bm: &BMesh) -> f32 {
    let (x, y) = match active_face(bm, true, false) {
        Some(f) => aspect_from_material(main, base, ob, i32::from(bm.face(f).mat_nr)),
        None => (1.0, 1.0),
    };
    x / y
}

// ------------------------------------------------------------------------------- islands

/// `bm_mesh_calc_uv_islands`: the UV islands (faces in `BM_mesh_calc_face_groups` order) of
/// the faces passing `uvedit_is_face_affected_for_calc_uv_islands`, split where the corners'
/// UVs differ and, with `use_seams`, at seams.
pub(crate) fn calc_uv_islands(
    sel: &Sel,
    only_selected_faces: bool,
    only_selected_uvs: bool,
    use_seams: bool,
) -> Vec<Vec<FaceId>> {
    let bm = sel.bm;
    let faces = faces(bm);
    let mut tag = vec![true; bm.faces.capacity()];
    let mut tot_faces = 0usize;
    for &f in &faces {
        let fl = bm.face(f).flag;
        let affected = !fl.contains(ElemFlag::HIDDEN)
            && (!only_selected_faces
                || (fl.contains(ElemFlag::SELECT) && (!only_selected_uvs || sel.face_select(f))));
        if affected {
            tag[f.slot()] = false;
            tot_faces += 1;
        }
    }
    let shared = |la: LoopId, lb: LoopId| -> bool {
        if use_seams && bm.edge(bm.lp(la).e).flag.contains(ElemFlag::SEAM) {
            return false;
        }
        sel.share_edge(la, lb)
    };
    let mut groups = Vec::new();
    let mut touched = 0usize;
    let mut next = 0usize;
    while touched != tot_faces {
        let mut stack = Vec::new();
        while next < faces.len() {
            let f = faces[next];
            next += 1;
            if !tag[f.slot()] {
                tag[f.slot()] = true;
                stack.push(f);
                break;
            }
        }
        if stack.is_empty() {
            break;
        }
        let mut group = Vec::new();
        while let Some(f) = stack.pop() {
            frap_math::budget::check();
            group.push(f);
            touched += 1;
            for l in bm.face_loops(f) {
                let mut r = bm.lp(l).radial_next;
                if r == l {
                    continue;
                }
                while r != l {
                    if shared(l, r) {
                        let fo = bm.lp(r).f;
                        if !tag[fo.slot()] {
                            tag[fo.slot()] = true;
                            stack.push(fo);
                        }
                    }
                    r = bm.lp(r).radial_next;
                }
            }
        }
        groups.push(group);
    }
    groups
}

/// One object of a multi-object operation: its edit mesh, the active UV map's column and
/// the aspect of its active face's material.
pub(crate) struct UvOb {
    /// The object.
    pub ob: IdRef<ObjectId>,
    /// Its mesh.
    pub me: IdRef<MeshId>,
    /// The edit BMesh (shared snapshot).
    pub bm: Arc<BMesh>,
    /// The active UV map's name.
    pub uv_name: SmolStr,
    /// The UV column (by loop slot), edited in place and written back by [`UvOb::commit`].
    pub uvs: Vec<Uv>,
    /// The pins (by loop slot).
    pub pins: Vec<bool>,
}

impl UvOb {
    /// The objects of `obs` with a UV map, with their columns.
    pub(crate) fn gather(cx: &mut OpCtx, obs: &[(IdRef<ObjectId>, IdRef<MeshId>)]) -> Vec<UvOb> {
        let mut out = Vec::new();
        for &(ob, me) in obs {
            let Some(bm) = edit_bm(cx, me) else { continue };
            let Some(uv) = active_uv(&bm) else { continue };
            out.push(UvOb {
                ob,
                me,
                uv_name: uv_name(&bm, uv),
                uvs: read_uvs(&bm, uv),
                pins: read_pins(&bm, uv),
                bm,
            });
        }
        out
    }

    /// The selection view of this object.
    pub(crate) fn sel(&self, ts: UvTs) -> Sel<'_> {
        Sel {
            ts,
            bm: &self.bm,
            uvs: &self.uvs,
        }
    }

    /// Writes the UV column back to the edit BMesh.
    pub(crate) fn commit(self, cx: &mut OpCtx) -> Result<(), OpError> {
        let bm = cx.bmesh_mut(self.me)?;
        write_uvs(bm, &self.uv_name, self.uvs);
        Ok(())
    }
}

/// `island_has_pins`.
fn island_has_pins(o: &UvOb, sel: &Sel, faces: &[FaceId], params: &PackParams) -> bool {
    for &f in faces {
        if params.pin_unselected
            && params.only_selected_faces
            && !o.bm.face(f).flag.contains(ElemFlag::SELECT)
        {
            return true;
        }
        for l in o.bm.face_loops(f) {
            if o.pins[l.slot()] {
                return true;
            }
            if params.pin_unselected && !sel.uv_select(l) {
                return true;
            }
        }
    }
    false
}

/// What `uvedit_pack_islands_multi` packs to besides the unit square.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub(crate) enum PackTarget {
    /// The UDIM base offset of `params` (headless: tile 1001).
    #[default]
    Udim,
    /// `original_selection`: the bounds of the selected UVs.
    OriginalAabb,
    /// `custom_region` (`xmin, ymin, xmax, ymax`).
    Region([f32; 4]),
}

/// `uvedit_pack_islands_multi` (headless: no Image Editor, so no closest-UDIM source).
pub(crate) fn pack_islands_multi(
    obs: &mut [UvOb],
    ts: UvTs,
    aspects: &[f32],
    target: PackTarget,
    params: &mut PackParams,
) {
    // (object, faces, pinned)
    let mut islands: Vec<(usize, Vec<FaceId>, bool)> = Vec::new();
    for (oi, o) in obs.iter().enumerate() {
        let sel = o.sel(ts);
        let mut only_selected_faces = params.only_selected_faces;
        let mut only_selected_uvs = params.only_selected_uvs;
        let ignore_pinned = params.pin_method == PinMethod::Ignore;
        if ignore_pinned && params.pin_unselected {
            only_selected_faces = false;
            only_selected_uvs = false;
        }
        for faces in calc_uv_islands(
            &sel,
            only_selected_faces,
            only_selected_uvs,
            params.use_seams,
        ) {
            let pinned = island_has_pins(o, &sel, &faces, params);
            if ignore_pinned && pinned {
                continue;
            }
            islands.push((oi, faces, pinned));
        }
    }
    if islands.is_empty() {
        return;
    }
    // The bounds of the selected UVs (`BM_face_uv_minmax`).
    let mut min = [f32::MAX; 2];
    let mut max = [-f32::MAX; 2];
    for (oi, faces, _) in &islands {
        let o = &obs[*oi];
        for &f in faces {
            for l in o.bm.face_loops(f) {
                let uv = o.uvs[l.slot()];
                for k in 0..2 {
                    if uv[k] < min[k] {
                        min[k] = uv[k];
                    }
                    if uv[k] > max[k] {
                        max[k] = uv[k];
                    }
                }
            }
        }
    }
    match target {
        PackTarget::OriginalAabb => {
            if (max[0] - min[0]) * (max[1] - min[1]) > 1e-40 {
                params.udim_base_offset = min;
                params.target_extent = max[1] - min[1];
                params.target_aspect_y = (max[0] - min[0]) / (max[1] - min[1]);
            }
        }
        PackTarget::Region(r) => {
            let (sx, sy) = (r[2] - r[0], r[3] - r[1]);
            // `BLI_rctf_is_empty`.
            if !(r[2] <= r[0] || r[3] <= r[1]) {
                params.udim_base_offset = [r[0], r[1]];
                params.target_extent = sy;
                params.target_aspect_y = sx / sy;
            }
        }
        PackTarget::Udim => {}
    }
    let mut packs: Vec<PackIsland> = islands
        .iter()
        .enumerate()
        .map(|(i, (oi, faces, pinned))| {
            let o = &obs[*oi];
            let mut p = PackIsland::new();
            p.caller_index = i as i32;
            p.aspect_y = if params.correct_aspect {
                aspects[*oi]
            } else {
                1.0
            };
            p.pinned = *pinned;
            for &f in faces {
                let poly: Vec<Uv> =
                    o.bm.face_loops(f)
                        .into_iter()
                        .map(|l| o.uvs[l.slot()])
                        .collect();
                if poly.iter().all(|uv| uv[0].is_finite() && uv[1].is_finite()) {
                    p.add_polygon(&poly);
                } else {
                    // Non-finite UVs (an unwrap of huge coordinates): Blender packs them as
                    // they are and they stay non-finite; frap's polyfill cannot triangulate
                    // them, so the polygon is added as a fan (the result is the same NaN).
                    for k in 1..poly.len().saturating_sub(1) {
                        p.add_triangle(poly[0], poly[k], poly[k + 1]);
                    }
                }
            }
            p
        })
        .collect();
    let scale = frap_uv::pack::pack_islands(&mut packs, params);
    for p in &packs {
        let (oi, faces, _) = &islands[p.caller_index as usize];
        let o = &mut obs[*oi];
        for &f in faces {
            let loops = o.bm.face_loops(f);
            let mut poly: Vec<Uv> = loops.iter().map(|l| o.uvs[l.slot()]).collect();
            frap_uv::pack::apply_to_uvs(p, scale, params, &mut poly);
            for (l, uv) in loops.iter().zip(poly) {
                o.uvs[l.slot()] = uv;
            }
        }
    }
}

// ------------------------------------------------------------------------------- unwrapping

/// `UnwrapOptions`.
#[derive(Clone, Debug)]
pub(crate) struct UnwrapOptions {
    /// Connectivity from UVs instead of seams.
    pub topology_from_uvs: bool,
    /// With `topology_from_uvs`, seams split too.
    pub topology_from_uvs_use_seams: bool,
    /// Only the selected faces.
    pub only_selected_faces: bool,
    /// Only faces with a selected UV.
    pub only_selected_uvs: bool,
    /// `fill_holes`
    pub fill_holes: bool,
    /// `correct_aspect`
    pub correct_aspect: bool,
    /// Unselected UVs count as pinned.
    pub pin_unselected: bool,
    /// ABF++ (`ANGLE_BASED`).
    pub use_abf: bool,
    /// `use_subsurf_data`
    pub use_subsurf: bool,
    /// `use_original_bounds`
    pub use_original_bounds: bool,
    /// `weight_group`
    pub weight_group: SmolStr,
}

impl UnwrapOptions {
    /// `unwrap_options_get(nullptr, ob, ts)`: the tool settings' options.
    pub(crate) fn from_ts(ts: &frap_scene::generated::plain::ToolSettings) -> UnwrapOptions {
        let f = ts.uvcalc_flag;
        let method = ts.unwrapper.as_str();
        let use_slim = method == "MINIMUM_STRETCH";
        UnwrapOptions {
            topology_from_uvs: false,
            topology_from_uvs_use_seams: false,
            only_selected_faces: false,
            only_selected_uvs: false,
            fill_holes: f & UVCALC_FILLHOLES != 0 || use_slim,
            correct_aspect: f & UVCALC_NO_ASPECT_CORRECT == 0,
            pin_unselected: false,
            use_abf: method == "ANGLE_BASED",
            use_subsurf: f & UVCALC_USESUBSURF != 0,
            use_original_bounds: false,
            weight_group: ts.uvcalc_weight_group.clone(),
        }
    }
}

/// A `ParamHandle` over one or more edit meshes; UV slots index the concatenated columns of
/// the objects (`bases[i]` + loop slot).
pub(crate) struct Handle {
    /// The parametrizer.
    pub pm: ParamMesh,
    /// Charts that failed to build (`r_count_failed` of `construct_end`).
    pub failed: u32,
    /// The concatenated UV columns.
    pub uvs: Vec<Uv>,
    /// Where each object's column starts in `uvs`.
    pub bases: Vec<usize>,
}

impl Handle {
    /// Copies the flushed UVs back into the objects' columns.
    pub(crate) fn scatter(&self, obs: &mut [UvOb]) {
        for (o, &b) in obs.iter_mut().zip(&self.bases) {
            let n = o.uvs.len();
            o.uvs.copy_from_slice(&self.uvs[b..b + n]);
        }
    }
}

/// `uvedit_is_face_affected`.
fn face_affected(sel: &Sel, f: FaceId, options: &UnwrapOptions) -> bool {
    let fl = sel.bm.face(f).flag;
    if fl.contains(ElemFlag::HIDDEN) {
        return false;
    }
    if options.only_selected_faces && !fl.contains(ElemFlag::SELECT) {
        return false;
    }
    if options.only_selected_uvs {
        return sel.bm.face_loops(f).into_iter().any(|l| sel.uv_select(l));
    }
    true
}

/// `construct_param_handle_multi` (and `construct_param_handle` for one object, `aspect_y`
/// from the first object when `correct_aspect`).
pub(crate) fn construct_param_handle(
    obs: &[UvOb],
    ts: UvTs,
    aspect_first: f32,
    weight_index: &[Option<u32>],
    options: &UnwrapOptions,
) -> Handle {
    let mut pm = ParamMesh::new();
    if options.correct_aspect {
        pm.set_aspect_ratio(aspect_first);
    }
    let mut uvs = Vec::new();
    let mut bases = Vec::new();
    for o in obs {
        bases.push(uvs.len());
        uvs.extend_from_slice(&o.uvs);
    }
    let mut vert_offset = 0u32;
    let mut face_offset = 0u64;
    for (oi, o) in obs.iter().enumerate() {
        let bm = &*o.bm;
        let sel = o.sel(ts);
        let base = bases[oi];
        let vidx = vert_indices(bm, vert_offset);
        let faces = faces(bm);
        let vkey = |v: VertId| vidx[v.slot()] as i32;
        let pin_of = |l: LoopId| {
            let mut pin = o.pins[l.slot()];
            if options.pin_unselected && !pin {
                pin = !sel.uv_select(l);
            }
            pin
        };
        // `uvedit_prepare_pinned_indices`.
        for &f in &faces {
            if !face_affected(&sel, f, options) {
                continue;
            }
            for l in bm.face_loops(f) {
                if pin_of(l) {
                    pm.prepare_pin_index(vkey(bm.lp(l).v), o.uvs[l.slot()]);
                }
            }
        }
        // `construct_param_handle_face_add`.
        let wgroup = weight_index[oi];
        for (i, &f) in faces.iter().enumerate() {
            if !face_affected(&sel, f, options) {
                continue;
            }
            let loops = bm.face_loops(f);
            let vkeys: Vec<u64> = loops
                .iter()
                .map(|&l| pm.find_pin_index(vkey(bm.lp(l).v), o.uvs[l.slot()]))
                .collect();
            let co: Vec<[f32; 3]> = loops.iter().map(|&l| loop_co(bm, l)).collect();
            let uvi: Vec<u32> = loops.iter().map(|l| (base + l.slot()) as u32).collect();
            let uv_init: Vec<Uv> = loops.iter().map(|l| o.uvs[l.slot()]).collect();
            let select: Vec<bool> = loops.iter().map(|&l| sel.uv_select(l)).collect();
            let pin: Vec<bool> = loops
                .iter()
                .zip(&select)
                .map(|(l, &s)| o.pins[l.slot()] || (options.pin_unselected && !s))
                .collect();
            let weight: Vec<f32> = loops
                .iter()
                .map(|&l| match (wgroup, &bm.vdata.deform) {
                    (Some(g), Some(dv)) => dv
                        .get(bm.lp(l).v.slot())
                        .and_then(|d| d.weights.iter().find(|(gi, _)| *gi == g))
                        .map_or(0.0, |(_, w)| *w),
                    _ => 1.0,
                })
                .collect();
            let mut face = ParamFace::new(
                face_offset + i as u64,
                &vkeys,
                &co,
                &uvi,
                &uv_init,
                &pin,
                &select,
            );
            face.weight = Some(&weight);
            pm.face_add(&face);
        }
        // `construct_param_edge_set_seams`.
        if !options.topology_from_uvs || options.topology_from_uvs_use_seams {
            for e in bm.edge_ids() {
                if !bm.edge(e).flag.contains(ElemFlag::SEAM) {
                    continue;
                }
                for l in bm.edge_loops(e) {
                    let ln = bm.lp(l).next;
                    let k0 = pm.find_pin_index(vkey(bm.lp(l).v), o.uvs[l.slot()]);
                    let k1 = pm.find_pin_index(vkey(bm.lp(ln).v), o.uvs[ln.slot()]);
                    pm.edge_set_seam([k0, k1]);
                }
            }
        }
        vert_offset += bm.verts_num() as u32;
        face_offset += bm.faces_num() as u64;
    }
    let failed = pm.construct_end(ConstructOpts::new(
        options.fill_holes,
        options.topology_from_uvs,
    ));
    Handle {
        pm,
        failed,
        uvs,
        bases,
    }
}

/// `BKE_object_defgroup_name_index(ob, name)` for the mesh's vertex groups.
pub(crate) fn defgroup_index(main: &Main, me: IdRef<MeshId>, name: &str) -> Option<u32> {
    main.meshes
        .get(me)?
        .vertex_group_names
        .iter()
        .position(|g| g == name)
        .map(|i| i as u32)
}

// ------------------------------------------------------------------------------- projections

/// `uv_map_transform_center` with a 3D View (frap's virtual one, D-10): the centre for
/// `tool_settings.transform_pivot_point`, and the bounds of the selected faces.
pub(crate) fn transform_center(
    c: &Context,
    ob: IdRef<ObjectId>,
    bm: &BMesh,
) -> ([f32; 3], [[f32; 3]; 2]) {
    let mut min = [f32::MAX; 3];
    let mut max = [-f32::MAX; 3];
    let selected: Vec<FaceId> = bm
        .face_ids()
        .into_iter()
        .filter(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
        .collect();
    for &f in &selected {
        for l in bm.face_loops(f) {
            let co = loop_co(bm, l);
            for k in 0..3 {
                min[k] = min[k].min(co[k]);
                max[k] = max[k].max(co[k]);
            }
        }
    }
    let mid = |a: [f32; 3], b: [f32; 3]| {
        [
            (a[0] + b[0]) * 0.5,
            (a[1] + b[1]) * 0.5,
            (a[2] + b[2]) * 0.5,
        ]
    };
    let center = match c.tool_settings().transform_pivot_point.as_str() {
        "BOUNDING_BOX_CENTER" => mid(min, max),
        "MEDIAN_POINT" => {
            let mut r = [0.0f32; 3];
            for &f in &selected {
                let m = bm.face_calc_center_median(f);
                for k in 0..3 {
                    r[k] += m[k];
                }
            }
            let inv = 1.0 / selected.len() as f32;
            [r[0] * inv, r[1] * inv, r[2] * inv]
        }
        "CURSOR" => {
            let m = frap_scene::id::transform::world_matrix_in(c.main, ob).inverse();
            m.transform_point3(glam::Vec3::from(c.cursor().location))
                .to_array()
        }
        // `BM_select_history_active_get`: the last selected element, the active face instead
        // of a last face, else the active face.
        "ACTIVE_ELEMENT" => match active_element(bm) {
            Some(BMElem::Vert(v)) => bm.vert(v).co,
            Some(BMElem::Edge(e)) => {
                let [a, b] = bm.edge(e).v;
                mid(bm.vert(a).co, bm.vert(b).co)
            }
            Some(BMElem::Face(f)) => bm.face_calc_center_median(f),
            None => [0.0; 3],
        },
        _ => [0.0; 3],
    };
    (center, [min, max])
}

/// `BM_select_history_active_get`.
fn active_element(bm: &BMesh) -> Option<BMElem> {
    let efa = active_face(bm, false, true).map(BMElem::Face);
    match bm.select_history.last().copied() {
        Some(BMElem::Face(f)) => efa.or(Some(BMElem::Face(f))),
        Some(e) => Some(e),
        None => efa,
    }
}

/// `shrink_loop_uv_by_aspect_ratio` on one face.
fn shrink_face(bm: &BMesh, f: FaceId, uvs: &mut [Uv], aspect_y: f32) {
    for l in bm.face_loops(f) {
        let uv = &mut uvs[l.slot()];
        if aspect_y > 1.0 {
            uv[0] = uv[0] / aspect_y + (0.5 - 0.5 / aspect_y);
        } else {
            uv[1] = uv[1] * aspect_y + (0.5 - 0.5 * aspect_y);
        }
    }
}

/// The `uv_map_clip_correct` properties.
#[derive(Copy, Clone, Debug)]
pub(crate) struct ClipCorrect {
    /// `correct_aspect`
    pub correct_aspect: bool,
    /// `clip_to_bounds` (false when the operator has none).
    pub clip_to_bounds: bool,
    /// `scale_to_bounds`
    pub scale_to_bounds: bool,
}

/// `uv_map_clip_correct`.
pub(crate) fn clip_correct(
    main: &Main,
    base: &PathBase,
    obs: &mut [UvOb],
    ts: UvTs,
    p: ClipCorrect,
    per_face_aspect: bool,
    only_selected_uvs: bool,
) {
    let mut min = [f32::MAX; 2];
    let mut max = [-f32::MAX; 2];
    for o in obs.iter_mut() {
        let bm = o.bm.clone();
        let faces = faces(&bm);
        if p.correct_aspect {
            if per_face_aspect {
                // `correct_uv_aspect_per_face`.
                let totcol = main
                    .objects
                    .get(o.ob)
                    .map_or(0, |ob| frap_scene::id::slots::slot_count(main, ob));
                if totcol > 0 {
                    let mut cache: Vec<Option<f32>> = vec![None; totcol];
                    for &f in &faces {
                        if !bm.face(f).flag.contains(ElemFlag::SELECT) {
                            continue;
                        }
                        let mi = i32::from(bm.face(f).mat_nr);
                        if mi < 0 || mi as usize >= totcol {
                            continue;
                        }
                        let a = *cache[mi as usize].get_or_insert_with(|| {
                            let (x, y) = aspect_from_material(main, base, o.ob, mi);
                            x / y
                        });
                        if a != 1.0 {
                            shrink_face(&bm, f, &mut o.uvs, a);
                        }
                    }
                }
            } else {
                // `correct_uv_aspect`.
                let a = aspect_y(main, base, o.ob, &bm);
                if a != 1.0 {
                    for &f in &faces {
                        if bm.face(f).flag.contains(ElemFlag::SELECT) {
                            shrink_face(&bm, f, &mut o.uvs, a);
                        }
                    }
                }
            }
        }
        let sel_ok = |o: &UvOb, f: FaceId| {
            bm.face(f).flag.contains(ElemFlag::SELECT)
                && (!only_selected_uvs || o.sel(ts).face_select(f))
        };
        if p.scale_to_bounds {
            for &f in &faces {
                if !sel_ok(o, f) {
                    continue;
                }
                for l in bm.face_loops(f) {
                    let uv = o.uvs[l.slot()];
                    for k in 0..2 {
                        min[k] = min[k].min(uv[k]);
                        max[k] = max[k].max(uv[k]);
                    }
                }
            }
        } else if p.clip_to_bounds {
            for &f in &faces {
                if !sel_ok(o, f) {
                    continue;
                }
                for l in bm.face_loops(f) {
                    let uv = &mut o.uvs[l.slot()];
                    uv[0] = uv[0].clamp(0.0, 1.0);
                    uv[1] = uv[1].clamp(0.0, 1.0);
                }
            }
        }
    }
    if !p.scale_to_bounds {
        return;
    }
    let mut dx = max[0] - min[0];
    let mut dy = max[1] - min[1];
    if dx > 0.0 {
        dx = 1.0 / dx;
    }
    if dy > 0.0 {
        dy = 1.0 / dy;
    }
    if dx == 1.0 && dy == 1.0 && min[0] == 0.0 && min[1] == 0.0 {
        return;
    }
    for o in obs.iter_mut() {
        let bm = o.bm.clone();
        for f in faces(&bm) {
            if !bm.face(f).flag.contains(ElemFlag::SELECT)
                || (only_selected_uvs && !o.sel(ts).face_select(f))
            {
                continue;
            }
            for l in bm.face_loops(f) {
                let uv = &mut o.uvs[l.slot()];
                uv[0] = (uv[0] - min[0]) * dx;
                uv[1] = (uv[1] - min[1]) * dy;
            }
        }
    }
}

/// The values of a `uv_map_clip_correct` operator's properties.
pub(crate) fn clip_props(correct_aspect: bool, clip: bool, scale: bool) -> ClipCorrect {
    ClipCorrect {
        correct_aspect,
        clip_to_bounds: clip,
        scale_to_bounds: scale,
    }
}

/// `em->bm->totfacesel != 0`.
pub(crate) fn has_face_selection(bm: &BMesh) -> bool {
    bm.faces
        .iter()
        .any(|(_, f)| f.flag.contains(ElemFlag::SELECT))
}

/// `uvedit_have_selection`: a visible face (with `only_selected_uvs`, one with a selected UV);
/// a mesh without a UV map needs a selected face.
pub(crate) fn have_selection(bm: &BMesh, ts: UvTs, only_selected_uvs: bool) -> bool {
    let Some(uv) = active_uv(bm) else {
        return has_face_selection(bm);
    };
    let uvs = read_uvs(bm, uv);
    let sel = Sel { ts, bm, uvs: &uvs };
    bm.face_ids().into_iter().any(|f| {
        let fl = bm.face(f).flag;
        let visible = if ts.sync {
            !fl.contains(ElemFlag::HIDDEN)
        } else {
            fl.contains(ElemFlag::SELECT)
        };
        visible && (!only_selected_uvs || bm.face_loops(f).into_iter().any(|l| sel.uv_select(l)))
    })
}

/// `uv_map_rotation_matrix_ex(result, rv3d, ob, upangledeg, sideangledeg, radius, offset)`
/// with an `offset` (`uv.project_from_view(orthographic=True)`); `view` is the virtual 3D
/// View's `viewmat`, `obmat` the object's world matrix (`m[col][row]`).
pub(crate) fn rotation_matrix_ex(
    view: &[[f32; 4]; 4],
    obmat: &[[f32; 4]; 4],
    upangledeg: f32,
    sideangledeg: f32,
    radius: f32,
    offset: [f32; 4],
) -> [[f32; 4]; 4] {
    let mut viewmatrix = *view;
    viewmatrix[3][0] = 0.0;
    viewmatrix[3][1] = 0.0;
    viewmatrix[3][2] = 0.0;
    let mut rotobj = *obmat;
    rotobj[3] = [0.0, 0.0, 0.0, rotobj[3][3]];
    rotobj[3][0] += offset[0];
    rotobj[3][1] += offset[1];
    rotobj[3][2] += offset[2];
    rotobj[3][3] += offset[3];
    rotobj[3][3] = 0.0;
    let pi = core::f32::consts::PI;
    let sideangle = pi * (sideangledeg + 180.0) / 180.0;
    let mut rotside = [[0.0f32; 4]; 4];
    rotside[0][0] = libm::cosf(sideangle);
    rotside[0][1] = -libm::sinf(sideangle);
    rotside[1][0] = libm::sinf(sideangle);
    rotside[1][1] = libm::cosf(sideangle);
    rotside[2][2] = 1.0;
    let upangle = pi * upangledeg / 180.0;
    let mut rotup = [[0.0f32; 4]; 4];
    rotup[1][1] = libm::cosf(upangle) / radius;
    rotup[1][2] = -libm::sinf(upangle) / radius;
    rotup[2][1] = libm::sinf(upangle) / radius;
    rotup[2][2] = libm::cosf(upangle) / radius;
    rotup[0][0] = 1.0 / radius;
    let s = mul_m4_m4m4(&rotup, &rotside);
    let t = mul_m4_m4m4(&s, &viewmatrix);
    mul_m4_m4m4(&t, &rotobj)
}

/// `mul_m4_m4m4` (SSE2 summation order: `(b0·a0 + b1·a1) + (b2·a2 + b3·a3)`).
pub(crate) fn mul_m4_m4m4(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut r = [[0.0f32; 4]; 4];
    for i in 0..4 {
        for k in 0..4 {
            r[i][k] =
                (b[i][0] * a[0][k] + b[i][1] * a[1][k]) + (b[i][2] * a[2][k] + b[i][3] * a[3][k]);
        }
    }
    r
}

/// `frap_uv::project::ProjectMesh` over the selected-face walk of an edit BMesh: corners are
/// renumbered face by face (`corner_loops`), edges are their slots.
pub(crate) struct BmProject<'a> {
    /// The mesh.
    pub bm: &'a BMesh,
    /// Faces in iteration order.
    pub faces: Vec<FaceId>,
    /// Corner offsets per face (`faces.len() + 1`).
    pub offsets: Vec<usize>,
    /// The loop of each corner.
    pub corner_loops: Vec<LoopId>,
    /// Face index by face slot.
    face_index: Vec<usize>,
}

impl<'a> BmProject<'a> {
    /// The walk over `bm`.
    pub(crate) fn new(bm: &'a BMesh) -> BmProject<'a> {
        let faces = bm.face_ids();
        let mut offsets = vec![0];
        let mut corner_loops = Vec::new();
        let mut face_index = vec![usize::MAX; bm.faces.capacity()];
        for (i, &f) in faces.iter().enumerate() {
            face_index[f.slot()] = i;
            corner_loops.extend(bm.face_loops(f));
            offsets.push(corner_loops.len());
        }
        BmProject {
            bm,
            faces,
            offsets,
            corner_loops,
            face_index,
        }
    }
    /// The corner UVs gathered from a loop-slot column.
    pub(crate) fn gather(&self, uvs: &[Uv]) -> Vec<Uv> {
        self.corner_loops.iter().map(|l| uvs[l.slot()]).collect()
    }
    /// Writes corner UVs back into a loop-slot column.
    pub(crate) fn scatter(&self, corners: &[Uv], uvs: &mut [Uv]) {
        for (l, uv) in self.corner_loops.iter().zip(corners) {
            uvs[l.slot()] = *uv;
        }
    }
}

impl frap_uv::project::ProjectMesh for BmProject<'_> {
    fn face_count(&self) -> usize {
        self.faces.len()
    }
    fn face_corners(&self, f: usize) -> core::ops::Range<usize> {
        self.offsets[f]..self.offsets[f + 1]
    }
    fn corner_co(&self, c: usize) -> [f32; 3] {
        loop_co(self.bm, self.corner_loops[c])
    }
    fn corner_edge(&self, c: usize) -> usize {
        self.bm.lp(self.corner_loops[c]).e.slot()
    }
    fn edge_is_seam(&self, e: usize) -> bool {
        self.bm
            .edge(frap_bmesh::EdgeId(e as u32))
            .flag
            .contains(ElemFlag::SEAM)
    }
    fn edge_faces(&self, e: usize, out: &mut Vec<usize>) {
        // `BM_FACES_OF_EDGE`: the radial cycle from `e->l`.
        let bm = self.bm;
        if let Some(first) = bm.edge(frap_bmesh::EdgeId(e as u32)).l {
            let mut l = first;
            loop {
                out.push(self.face_index[bm.lp(l).f.slot()]);
                l = bm.lp(l).radial_next;
                if l == first {
                    break;
                }
            }
        }
    }
    fn face_selected(&self, f: usize) -> bool {
        self.bm.face(self.faces[f]).flag.contains(ElemFlag::SELECT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The factory cube as an edit BMesh (its `UVMap` is the cross layout), all selected.
    fn cube() -> BMesh {
        let me = frap_mesh::primitives::cube::cube(2.0, true, [1.0; 3]);
        let mut bm = BMesh::from_mesh(&me, &frap_bmesh::FromMeshParams::default());
        for f in bm.face_ids() {
            bm.face_select_set(f, true);
        }
        bm
    }

    fn ts(sync: bool, mode: SelectMode) -> UvTs {
        UvTs {
            sync,
            selectmode: mode,
        }
    }

    #[test]
    fn active_uv_and_pins() {
        let bm = cube();
        let uv = active_uv(&bm).expect("the cube has a UV map");
        assert_eq!(uv_name(&bm, uv), "UVMap");
        assert_eq!(pin_name("UVMap"), ".pn.UVMap");
        assert_eq!(read_pins(&bm, uv).len(), bm.loops.capacity());
        assert!(read_pins(&bm, uv).iter().all(|p| !p));
    }

    #[test]
    fn islands_follow_uv_connectivity_and_seams() {
        let mut bm = cube();
        let uv = active_uv(&bm).unwrap();
        let uvs = read_uvs(&bm, uv);
        let sel = Sel {
            ts: ts(true, SelectMode::VERT),
            bm: &bm,
            uvs: &uvs,
        };
        // The cross layout is one island when no seam is marked.
        assert_eq!(calc_uv_islands(&sel, true, false, true).len(), 1);
        // Every face apart in UV space: one island per face.
        let mut apart = uvs.clone();
        for (k, f) in bm.face_ids().into_iter().enumerate() {
            for l in bm.face_loops(f) {
                apart[l.slot()][0] += 10.0 * k as f32;
            }
        }
        let sel = Sel {
            ts: ts(true, SelectMode::VERT),
            bm: &bm,
            uvs: &apart,
        };
        assert_eq!(calc_uv_islands(&sel, true, false, true).len(), 6);
        // Seams split UV-connected faces with `use_seams`.
        for e in bm.edge_ids() {
            bm.edge_mut(e).flag.insert(ElemFlag::SEAM);
        }
        let sel = Sel {
            ts: ts(true, SelectMode::VERT),
            bm: &bm,
            uvs: &uvs,
        };
        assert_eq!(calc_uv_islands(&sel, true, false, true).len(), 6);
        assert_eq!(calc_uv_islands(&sel, true, false, false).len(), 1);
    }

    #[test]
    fn sync_selection_follows_the_mesh_selection_mode() {
        let mut bm = cube();
        let uv = active_uv(&bm).unwrap();
        let uvs = read_uvs(&bm, uv);
        bm.deselect_all();
        let f0 = bm.face_ids()[0];
        bm.face_select_set(f0, true);
        let l0 = bm.face_loops(f0)[0];
        for mode in [SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE] {
            let sel = Sel {
                ts: ts(true, mode),
                bm: &bm,
                uvs: &uvs,
            };
            assert!(sel.face_select(f0));
            assert!(sel.uv_select(l0));
            assert!(sel.edge_select(l0));
        }
        // Another face's corner on a vertex of the selected face with its UV elsewhere: selected
        // in vertex mode, not in face mode (sticky location).
        let v0 = bm.lp(l0).v;
        let other = bm
            .vert_loops(v0)
            .into_iter()
            .find(|&l| bm.lp(l).f != f0 && uvs[l.slot()] != uvs[l0.slot()]);
        if let Some(other) = other {
            let vert_mode = Sel {
                ts: ts(true, SelectMode::VERT),
                bm: &bm,
                uvs: &uvs,
            };
            assert!(vert_mode.uv_select(other));
            let face_mode = Sel {
                ts: ts(true, SelectMode::FACE),
                bm: &bm,
                uvs: &uvs,
            };
            assert!(!face_mode.uv_select(other));
        }
        // Without sync, only selected faces are visible.
        let no_sync = Sel {
            ts: ts(false, SelectMode::VERT),
            bm: &bm,
            uvs: &uvs,
        };
        assert!(!no_sync.face_visible(bm.face_ids()[1]));
        assert!(no_sync.face_visible(f0));
    }

    #[test]
    fn have_selection_and_projection_walk() {
        use frap_uv::project::ProjectMesh;
        let mut bm = cube();
        let ts_sync = ts(true, SelectMode::VERT);
        assert!(have_selection(&bm, ts_sync, true));
        bm.deselect_all();
        assert!(have_selection(&bm, ts_sync, false));
        assert!(!have_selection(&bm, ts_sync, true));
        let walk = BmProject::new(&bm);
        assert_eq!(walk.face_count(), 6);
        assert_eq!(walk.face_corners(5), 20..24);
        let mut faces = Vec::new();
        walk.edge_faces(0, &mut faces);
        assert_eq!(faces.len(), 2);
    }

    #[test]
    fn rotation_matrix_with_offset_matches_frap_uv() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let ours = rotation_matrix_ex(&id, &id, 90.0, 0.0, 1.0, [0.0; 4]);
        let theirs = frap_uv::project::uv_map_rotation_matrix(Some(&id), &id, 90.0, 0.0, 1.0);
        for c in 0..3 {
            for r in 0..3 {
                assert!((ours[c][r] - theirs[c][r]).abs() < 1e-7);
            }
        }
        let shifted = rotation_matrix_ex(&id, &id, 90.0, 0.0, 1.0, [1.0, 2.0, 3.0, -1.0]);
        assert_eq!(shifted[3][3], 0.0);
    }

    #[test]
    fn options_from_factory_and_new_scene_tool_settings() {
        let factory = frap_scene::generated::plain::ToolSettings::default();
        let o = UnwrapOptions::from_ts(&factory);
        assert!(o.use_abf && o.fill_holes && o.correct_aspect);
        let mut new_scene = factory.clone();
        new_scene.unwrapper = "CONFORMAL".into();
        new_scene.uvcalc_flag = 0;
        let o = UnwrapOptions::from_ts(&new_scene);
        assert!(!o.use_abf && !o.fill_holes);
        assert_eq!(factory.uvcalc_margin, 0.001);
    }
}
