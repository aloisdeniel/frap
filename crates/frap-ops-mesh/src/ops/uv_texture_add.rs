// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.uv_texture_add()`, tier T1 (`tests/golden/cases/edit/normals/uv_texture_*.py`).
//! Port of `mesh_uv_texture_add_exec` and `ED_mesh_uv_add(mesh, nullptr, true, true)` /
//! `reset_uv_map` / `mesh_uv_reset_array` (`editors/mesh/mesh_data.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll: the active object is an editable mesh (Object or Edit Mode). Adds a UV map named
//! `UVMap` (made unique: `UVMap.001`, …) that **copies the active UV map** and becomes the
//! active one ([MESH §4.10]); a mesh without UV maps gets the default layout (each face reset:
//! triangle `(0,0) (1,0) (1,1)`, quad `(0,0) (1,0) (1,1) (0,1)`, n-gon on a circle) on every
//! face in Object Mode, on the selected faces only in Edit Mode (the others stay at `(0,0)`).
//! The render UV map is set when none is. A ninth UV map is refused: WARNING `Cannot add more
//! than 8 UV maps`, `CANCELLED`. In Edit Mode the pin layers (`.pn.<uv>`) are ensured.

use std::sync::Arc;

use frap_bmesh::{BMesh, ElemFlag};
use frap_mesh::{AttrData, Domain, Mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh;
use smol_str::SmolStr;

use super::geometry__attribute_add::{attrs, common};

/// `mesh_uv_reset_array`: the default UVs of a face with `len` corners.
pub(crate) fn reset_face_uvs(len: usize) -> Vec<[f32; 2]> {
    match len {
        3 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        4 => vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
        n if n > 2 => {
            let mut fac = 0.0f32;
            let dfac = (1.0f32 / n as f32) * (std::f32::consts::PI * 2.0);
            (0..n)
                .map(|_| {
                    let uv = [0.5 * fac.sin() + 0.5, 0.5 * fac.cos() + 0.5];
                    fac += dfac;
                    uv
                })
                .collect()
        }
        n => vec![[0.0, 0.0]; n],
    }
}

/// Object Mode `ED_mesh_uv_add`.
fn add_object(me: &mut Mesh, names: &mut attrs::Names, vgroups: &[SmolStr]) -> Option<SmolStr> {
    let existing = me.uv_map_count();
    if existing >= frap_mesh::MAX_UV_MAPS {
        return None;
    }
    let name = attrs::unique_name(me, vgroups, "UVMap");
    let active = names
        .uv_active
        .as_deref()
        .filter(|n| !n.is_empty())
        .map(|n| match me.attributes().get(n) {
            Some(a) if a.domain == Domain::Corner => match &a.data {
                AttrData::Float2(v) => v.as_ref().clone(),
                _ => vec![[0.0; 2]; me.corners_num()],
            },
            _ => vec![[0.0; 2]; me.corners_num()],
        });
    let uvs = match active {
        Some(v) => v,
        None => {
            let mut out = Vec::with_capacity(me.corners_num());
            for f in 0..me.faces_num() {
                out.extend(reset_face_uvs(me.face(f).len()));
            }
            out
        }
    };
    attrs::Store::insert(
        me,
        name.clone(),
        Domain::Corner,
        AttrData::Float2(Arc::new(uvs)),
    );
    Some(name)
}

/// Edit Mode `ED_mesh_uv_add`.
fn add_edit(bm: &mut BMesh, names: &mut attrs::Names, vgroups: &[SmolStr]) -> Option<SmolStr> {
    let existing = bm.ldata.float2_names();
    if existing.len() >= frap_mesh::MAX_UV_MAPS {
        return None;
    }
    let name = attrs::unique_name(bm, vgroups, "UVMap");
    let n = bm.loops.capacity();
    attrs::Store::insert(
        bm,
        name.clone(),
        Domain::Corner,
        AttrData::Float2(Arc::new(vec![[0.0; 2]; n])),
    );
    attrs::pin_ensure_all(bm);
    if !existing.is_empty() {
        let src = names
            .uv_active
            .clone()
            .filter(|a| existing.contains(a))
            .unwrap_or_else(|| existing[0].clone());
        if let Some((_, data)) = attrs::Store::column(bm, &src) {
            attrs::Store::replace(bm, &name, data);
        }
    } else {
        let i = bm.ldata.find(&name).expect("added above");
        let faces = bm.face_ids();
        let mut writes = Vec::new();
        for f in faces {
            if !bm.face(f).flag.contains(ElemFlag::SELECT) {
                continue;
            }
            let loops = bm.face_loops(f);
            for (l, uv) in loops.iter().zip(reset_face_uvs(loops.len())) {
                writes.push((l.slot(), uv));
            }
        }
        if let AttrData::Float2(v) = &mut bm.ldata.layers[i].data {
            let v = Arc::make_mut(v);
            for (s, uv) in writes {
                v[s] = uv;
            }
        }
    }
    Some(name)
}

#[operator(poll = poll::ACTIVE_EDITABLE_MESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::UvTextureAdd) -> OpResult {
    let Some((_, me)) = common::context_mesh(&cx.ctx()) else {
        return Err(OpError::Runtime("Context missing active object".into()));
    };
    let edit = common::in_edit(cx, me);
    let added = if edit {
        let mut names = common::read(cx, me, |_, n| n.clone()).unwrap_or_default();
        let vgroups = cx
            .main()
            .meshes
            .get(me)
            .map(|m| m.vertex_group_names.clone())
            .unwrap_or_default();
        let bm = cx.bmesh_mut(me)?;
        let name = add_edit(bm, &mut names, &vgroups);
        if let Some(n) = &name {
            names.uv_active = Some(n.clone());
            let uvs = bm.ldata.float2_names();
            if !names.uv_render.as_ref().is_some_and(|r| uvs.contains(r)) {
                names.uv_render = Some(n.clone());
            }
            names.write_bm(bm);
            names.write(cx.mesh_mut(me)?);
        }
        name
    } else {
        let m = cx.mesh_mut(me)?;
        let mut names = attrs::Names::of(m);
        let vgroups = m.vertex_group_names.clone();
        let name = add_object(m, &mut names, &vgroups);
        if let Some(n) = &name {
            names.uv_active = Some(n.clone());
            let has_render = names
                .uv_render
                .as_deref()
                .is_some_and(|r| m.uv_maps().iter().any(|(u, _)| *u == r));
            if !has_render {
                names.uv_render = Some(n.clone());
            }
            names.write(m);
        }
        name
    };
    if added.is_none() {
        cx.report(
            ReportType::Warning,
            format!("Cannot add more than {} UV maps", frap_mesh::MAX_UV_MAPS),
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
