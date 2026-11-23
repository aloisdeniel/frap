// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.join_uvs()` ("Transfer UV Maps"), tier T0
//! (`tests/golden/cases/object/data_m8/join_uvs.py`). Port of the Python operator `JoinUVs`
//! (`scripts/startup/bl_operators/object.py`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Verified on 5.2.4:
//! - poll: the active object is a mesh (`failed, context is incorrect`);
//! - the active mesh's active UV map is copied, corner by corner, into the active UV map of
//!   every other selected editable mesh object whose mesh is another one with the same number
//!   of corners (a mesh shared by several targets is written once); a target without a UV map
//!   gets a new `UVMap` first;
//! - a target with another corner count: WARNING `Object: <ob>, Mesh: '<me>' has <n> loops
//!   (for <f> faces), expected <m>`; an active mesh without UV maps: WARNING `Object: <ob>,
//!   Mesh: '<me>' has no UVs`;
//! - in Edit Mode the operator leaves it (`object.mode_set(mode='OBJECT')`), copies, and enters
//!   it again; always `FINISHED`.

use frap_mesh::{AttrData, Mesh};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, Input, MeshId};
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;
use smol_str::SmolStr;

/// `JoinUVs.poll`: `obj and obj.type == 'MESH'`.
fn join_poll(c: &Context) -> Result<(), PollFail> {
    poll::active_object(c)?;
    match poll::active(c) {
        Some((_, ob)) if ob.object_type() == "MESH" => Ok(()),
        _ => Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make a mesh object active",
        )),
    }
}

/// `JoinUVs.poll`.
const JOIN_POLL: Poll = Poll {
    requires: &[PollReq::ActiveObject, PollReq::ActiveType("MESH")],
    check: join_poll,
};

/// `mesh.uv_layers.active`: the active UV map, else the first one.
fn active_uv(me: &Mesh) -> Option<SmolStr> {
    let maps = me.uv_maps();
    me.uv_active
        .as_ref()
        .filter(|a| maps.iter().any(|(n, _)| *n == a.as_str()))
        .cloned()
        .or_else(|| maps.first().map(|(n, _)| SmolStr::new(n)))
}

fn mode_set(cx: &mut OpCtx, mode: &str) -> Result<(), OpError> {
    let def = frap_op::lookup("object.mode_set").map_err(|e| OpError::Runtime(e.message))?;
    let (args, _, _) = frap_op::convert_args(
        def.spec,
        Vec::new(),
        vec![
            ("mode".into(), Input::Str(mode.into())),
            ("toggle".into(), Input::Bool(false)),
        ],
    )
    .map_err(|e| OpError::Runtime(e.message))?;
    cx.run_op("object.mode_set", args)?;
    Ok(())
}

fn copy_uvs(cx: &mut OpCtx, me: IdRef<MeshId>, uvs: &[[f32; 2]]) -> Result<(), OpError> {
    let m = cx.mesh_mut(me)?;
    let name = match active_uv(m) {
        Some(n) => n,
        // `uv_layers.new()`: `UVMap`, active when it is the first one.
        None => m
            .add_uv_map("", true)
            .ok_or_else(|| OpError::Runtime("Could not add a new UV map to the mesh".into()))?,
    };
    if let Some(AttrData::Float2(v)) = m.attributes_mut().get_mut(&name).map(|a| &mut a.data) {
        *std::sync::Arc::make_mut(v) = uvs.to_vec();
    }
    // `mesh_other.update()`.
    m.update(false);
    Ok(())
}

#[operator(poll = JOIN_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::JoinUvs) -> OpResult {
    let Some(act) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    let is_editmode = cx
        .main()
        .objects
        .get(act)
        .is_some_and(|o| o.mode == ObjectMode::Edit);
    if is_editmode {
        mode_set(cx, "OBJECT")?;
    }
    let main = cx.main();
    let Some(ob) = main.objects.get(act) else {
        return Ok(OpStatus::Cancelled);
    };
    let ob_name = ob.header.name.clone();
    let Some(me) = ob.data.and_then(|d| d.typed::<MeshId>()) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(mesh) = main.meshes.get(me) else {
        return Ok(OpStatus::Cancelled);
    };
    let uv = active_uv(mesh).and_then(|n| {
        mesh.uv_maps()
            .into_iter()
            .find(|(m, _)| *m == n.as_str())
            .map(|(_, v)| v.to_vec())
    });
    match uv {
        None => {
            let msg = format!("Object: {ob_name}, Mesh: '{}' has no UVs", mesh.header.name);
            cx.report(ReportType::Warning, msg);
        }
        Some(uvs) => {
            let nbr_loops = mesh.corners_num();
            let objects = cx.ctx().selected_editable_objects();
            let mut tagged: Vec<IdRef<MeshId>> = Vec::new();
            for other in objects {
                let main = cx.main();
                let Some(o) = main.objects.get(other) else {
                    continue;
                };
                if other == act || o.object_type() != "MESH" {
                    continue;
                }
                let Some(other_me) = o.data.and_then(|d| d.typed::<MeshId>()) else {
                    continue;
                };
                if other_me == me || tagged.contains(&other_me) {
                    continue;
                }
                tagged.push(other_me);
                let Some(m) = main.meshes.get(other_me) else {
                    continue;
                };
                if m.corners_num() != nbr_loops {
                    let msg = format!(
                        "Object: {}, Mesh: '{}' has {} loops (for {} faces), expected {}",
                        o.header.name,
                        m.header.name,
                        m.corners_num(),
                        m.faces_num(),
                        nbr_loops
                    );
                    cx.report(ReportType::Warning, msg);
                    continue;
                }
                copy_uvs(cx, other_me, &uvs)?;
            }
        }
    }
    if is_editmode {
        mode_set(cx, "EDIT")?;
    }
    Ok(OpStatus::Finished)
}
