// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.convert(target='MESH', keep_original=False, merge_customdata=True,
//! thickness=5, faces=True, offset=0.01)`, tier T2 (the evaluated mesh). Port of
//! `object_convert_exec`, `object_convert_poll`, `convert_mesh_to_mesh`,
//! `get_object_for_conversion` and `duplibase_for_convert` (`editors/object/object_add.cc`,
//! Blender 5.2.4, GPL-2.0-or-later); `target='MESH'` only.
//!
//! Verified on 5.2.4: a mesh object's data is replaced by its evaluated mesh (every enabled
//! modifier applied, viewport settings) and its modifiers are cleared; two selected objects
//! sharing a mesh convert it once (the second only loses its modifiers);
//! `keep_original=True` converts a copy (`Cube.001` with its own data, linked to the same
//! collections, selected and made active instead of the original); cameras and lights are
//! incompatible (INFO `None of the objects are compatible with a conversion to "Mesh"`, or
//! `The selection included N object type(s) which do not support conversion to "Mesh"`), an
//! empty has no evaluated mesh (WARNING `Object 'Empty' has no evaluated Mesh data`); nothing
//! selected is `CANCELLED` with the INFO `No editable objects to convert`; a selected object
//! in Edit Mode: `RuntimeError: Error: Cannot convert selected objects while they are in edit
//! mode`. The poll needs the active object (if any) in Object Mode.
//!
//! Other targets (curves, point clouds, grease pencil) answer `OP_NOT_IMPLEMENTED`, and so does
//! a mesh whose stack holds a modifier frap cannot evaluate (no evaluator for its type, a node
//! group other than Smooth by Angle): converting it would drop the modifier without applying
//! it. Blender's own modifier errors (a modifier it skips too) are WARNING reports.

use std::collections::HashSet;
use std::sync::Arc;

use frap_eval::EvalCache;
use frap_mesh::Mesh;
use frap_modifiers::stack;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdAny, IdCode, IdRef, MeshId, ObjectId};
use frap_scene::id::layers;
use frap_scene::{
    BaseFlag, Context, EvalError, EvalMode, EvaluatedObject, Evaluator, ModifierStatus, ObjectMode,
};
use frap_spec_ops_object::object;

use super::modifier_apply::merge_customdata_for_apply_modifier;
use super::move_to_collection::common::{all_scene_collections, has_object, link_object};

/// `object_convert_poll`: an editable scene whose active object (if any) is in Object Mode.
fn convert_poll(c: &Context) -> Result<(), PollFail> {
    poll::scene_editable(c)?;
    match poll::active(c) {
        Some((_, ob)) if ob.mode != ObjectMode::Object => Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        )),
        _ => Ok(()),
    }
}

/// `object_convert_poll`.
const CONVERT_POLL: Poll = Poll {
    requires: &[PollReq::SceneEditable, PollReq::ModeObject],
    check: convert_poll,
};

/// `duplibase_for_convert` + the data copy of `get_object_for_conversion`: a copy of `ob`
/// with a copy of its mesh, linked where `ob` is, selected instead of `ob`.
fn duplicate_for_convert(cx: &mut OpCtx, ob: IdRef<ObjectId>) -> Option<IdRef<ObjectId>> {
    let (scene, layer) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        (c.win.scene, layer)
    };
    let (main, log) = cx.main_mut();
    let mut copy = main.objects.get(ob)?.clone();
    let name = copy.header.name.clone();
    copy.header.use_fake_user = false;
    copy.header.library = None;
    if let Some(me) = copy.data.and_then(|d| d.typed::<MeshId>())
        && let Some(mut m) = main.meshes.get(me).cloned()
    {
        let mname = m.header.name.clone();
        m.header.use_fake_user = false;
        m.header.library = None;
        copy.data = Some(main.meshes.insert(&mname, m, log).any());
    }
    let new = main.objects.insert(&name, copy, log);
    let mut linked = false;
    for c in all_scene_collections(main, scene) {
        if has_object(main, scene, c, ob) && link_object(main, log, scene, c, new) {
            linked = true;
        }
    }
    if !linked {
        link_object(main, log, scene, None, new);
    }
    layers::sync_all(main, log);
    if let Some(s) = main.scenes.get_mut(scene, log) {
        let vl = &mut s.view_layers[layer];
        if let Some(b) = vl.base_mut(new) {
            b.flag.insert(BaseFlag::SELECTED);
        }
        if let Some(b) = vl.base_mut(ob) {
            b.flag.remove(BaseFlag::SELECTED);
        }
    }
    log.context_changed = true;
    Some(new)
}

/// The evaluated mesh to write into the data: a modifier frap cannot evaluate (no evaluator,
/// a node group it does not support) answers `OP_NOT_IMPLEMENTED` rather than dropping it
/// (as `object.modifier_apply` does); the stack's other warnings (Blender's modifier errors,
/// which Blender also skips) are WARNING reports; an evaluation failure is an error.
fn evaluated_mesh(
    cx: &mut OpCtx,
    name: &str,
    evaluated: &Result<Arc<EvaluatedObject>, EvalError>,
) -> Result<Mesh, OpError> {
    let eo = evaluated
        .as_ref()
        .map_err(|e| OpError::Runtime(format!("Object '{name}': {e}")))?;
    if let Some((m, st)) = eo
        .modifier_status
        .iter()
        .find(|(_, st)| stack::status_is_frap_limitation(st))
    {
        let why = match st {
            ModifierStatus::Error(e) => e.clone(),
            _ => "its type has no frap evaluator".to_owned(),
        };
        return Err(OpError::NotImplemented(format!(
            "frap: object.convert cannot apply modifier '{m}' of object '{name}' ({why})"
        )));
    }
    for w in &eo.warnings {
        cx.report(w.level, w.message.clone());
    }
    eo.mesh
        .as_deref()
        .cloned()
        .ok_or_else(|| OpError::Runtime(format!("Object '{name}' has no evaluated Mesh data")))
}

#[operator(
    poll = CONVERT_POLL,
    tier = T2,
    support = Partial("target='MESH' only; other targets answer OP_NOT_IMPLEMENTED"),
)]
fn exec(cx: &mut OpCtx, p: object::Convert) -> OpResult {
    if p.target.value != object::convert::Target::Mesh {
        return Err(OpError::NotImplemented(format!(
            "frap: object.convert(target='{}') is not implemented (only 'MESH')",
            p.target.value.identifier()
        )));
    }
    let keep_original = p.keep_original.value;
    let (objects, active) = {
        let c = cx.ctx();
        (c.selected_editable_objects(), c.active_object())
    };
    if objects.is_empty() {
        cx.report(ReportType::Info, "No editable objects to convert");
        return Ok(OpStatus::Cancelled);
    }
    if objects.iter().any(|&ob| {
        cx.main()
            .objects
            .get(ob)
            .is_some_and(|o| o.mode == ObjectMode::Edit)
    }) {
        return Err(OpError::Runtime(
            "Cannot convert selected objects while they are in edit mode".into(),
        ));
    }
    // The evaluated meshes, before anything changes (`BKE_scene_graph_update_tagged`).
    let evaluated: Vec<Result<Arc<EvaluatedObject>, EvalError>> = {
        let (host, _) = cx.split_eval();
        let mut ev = EvalCache::new();
        objects
            .iter()
            .map(|&ob| ev.object(host, ob, EvalMode::Viewport))
            .collect()
    };
    let mut converted_data: HashSet<IdAny> = HashSet::new();
    let mut incompatible = 0usize;
    let mut new_active = None;
    for (k, &ob) in objects.iter().enumerate() {
        let Some(o) = cx.main().objects.get(ob) else {
            continue;
        };
        let (ty, data, o_name) = (o.object_type(), o.data, o.header.name.clone());
        if let Some(d) = data
            && !keep_original
            && converted_data.contains(&d)
        {
            // The data was converted with another object: only the modifiers go.
            if ty == "MESH" && !o.modifiers.is_empty() {
                cx.object_mut(ob)?.modifiers.clear();
            }
            continue;
        }
        match ty {
            "MESH" => {
                let target = if keep_original {
                    match duplicate_for_convert(cx, ob) {
                        Some(n) => {
                            if active == Some(ob) {
                                new_active = Some(n);
                            }
                            n
                        }
                        None => continue,
                    }
                } else {
                    ob
                };
                let Some(me) = cx
                    .main()
                    .objects
                    .get(target)
                    .and_then(|o| o.data)
                    .filter(|d| d.code == IdCode::ME)
                    .and_then(|d| d.typed::<MeshId>())
                else {
                    continue;
                };
                let mut new_mesh = evaluated_mesh(cx, &o_name, &evaluated[k])?;
                if p.merge_customdata.value {
                    merge_customdata_for_apply_modifier(&mut new_mesh);
                }
                let mesh = cx.mesh_mut(me)?;
                new_mesh.header = mesh.header.clone();
                // A shape key whose basis is not in the result is dropped.
                new_mesh.shape_keys = None;
                *mesh = new_mesh;
                if !cx.main().objects.get(target).is_some_and(|o| o.modifiers.is_empty()) {
                    cx.object_mut(target)?.modifiers.clear();
                }
                if let Some(d) = data
                    && !keep_original
                {
                    converted_data.insert(d);
                }
            }
            "EMPTY" => cx.report(
                ReportType::Warning,
                format!("Object '{}' has no evaluated Mesh data", o.header.name),
            ),
            _ => incompatible += 1,
        }
    }
    if let Some(n) = new_active {
        let (scene, vl_name) = {
            let c = cx.ctx();
            (c.win.scene, c.view_layer().name.clone())
        };
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log)
            && let Some(vl) = s.view_layers.iter_mut().find(|v| v.name == vl_name)
        {
            vl.active = Some(n);
        }
        log.context_changed = true;
    }
    if incompatible != 0 {
        if incompatible == objects.len() {
            cx.report(
                ReportType::Info,
                "None of the objects are compatible with a conversion to \"Mesh\"",
            );
        } else {
            cx.report(
                ReportType::Info,
                format!(
                    "The selection included {incompatible} object type(s) which do not support conversion to \"Mesh\""
                ),
            );
        }
    }
    Ok(OpStatus::Finished)
}
