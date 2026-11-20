// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.modifier_apply_as_shapekey(keep_modifier=False, modifier='', report=False,
//! use_selected_objects=False)`, tier T2 (the result is the modifier's evaluator). Port of
//! `modifier_apply_as_shapekey_exec`, `modifier_apply_exec_ex`, `ed::object::modifier_apply`
//! and `modifier_apply_shape` (`editors/object/object_modifier.cc`), Blender 5.2.4,
//! GPL-2.0-or-later:
//!
//! the modifier is evaluated alone on the mesh **with its shape keys mixed in** (the
//! `ShapeKey` virtual modifier, `create_applied_mesh_for_modifier(…, use_virtual_modifiers =
//! true)`), and the result becomes a new shape key named after the modifier (a key with a
//! `Basis` block from the mesh is created first when the mesh has none). The modifier is
//! removed unless `keep_modifier`. Works on multi-user data. Errors (`RuntimeError`): `Modifier
//! is disabled, skipping apply`, `Only deforming modifiers can be applied to shapes`
//! (constructive types), `Modifiers cannot be applied in edit mode`. A modifier that is not
//! first adds the INFO `Applied modifier was not first, result may not be as expected`.
//! Poll as `modifier_apply`.

use frap_eval::EvalCache;
use frap_modifiers::stack::{self, edit};
use frap_modifiers::{ModKind, ModifierType, registry};
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::Key;
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::modifier_add::shared;

const OP: &str = "modifier_apply_as_shapekey";

/// `modifier_apply_poll`.
fn apply_poll(c: &Context) -> Result<(), PollFail> {
    shared::edit_modifier_poll(c, OP, false)?;
    match poll::active(c) {
        Some((_, ob)) if ob.object_type() == "EMPTY" => Err(PollFail {
            message: Some(shared::poll_message(
                OP,
                "Modifiers cannot be applied on empty object type",
            )),
            unmet: smallvec![PollReq::ActiveType("MESH")],
            hint: None,
        }),
        _ => Ok(()),
    }
}

/// `modifier_apply_poll`.
const APPLY_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ModeObject,
    ],
    check: apply_poll,
};

fn mesh_of(cx: &OpCtx, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    let me = cx.main().objects.get(ob)?.data?.typed::<MeshId>()?;
    cx.main().meshes.get(me).map(|_| me)
}

/// `modifier_apply_shape` for modifier `i` of `ob`.
fn apply_shape(cx: &mut OpCtx, ob: IdRef<ObjectId>, i: usize) -> Result<(), OpError> {
    let (ty, disabled, name) = {
        let o = cx
            .main()
            .objects
            .get(ob)
            .ok_or_else(|| OpError::Runtime("the object was removed".into()))?;
        let m = &o.modifiers[i];
        let ty = ModifierType::from_identifier(m.settings.type_identifier());
        let disabled = ty
            .and_then(registry::find)
            .and_then(|info| (info.is_disabled)(&m.settings))
            .is_some();
        (ty, disabled, m.name.clone())
    };
    if disabled {
        return Err(OpError::Runtime("Modifier is disabled, skipping apply".into()));
    }
    let Some(me) = mesh_of(cx, ob) else {
        return Err(OpError::Runtime(
            "Cannot apply modifier for this object type".into(),
        ));
    };
    let kind = ty.map(|t| registry::meta(t).kind);
    if !matches!(kind, Some(ModKind::OnlyDeform | ModKind::Nonconstructive)) {
        return Err(OpError::Runtime(
            "Only deforming modifiers can be applied to shapes".into(),
        ));
    }
    // The shape-key virtual modifier first: evaluate on the key mix.
    let mix = Key::evaluate_object(cx.main(), ob)
        .filter(|m| cx.main().meshes.get(me).is_some_and(|x| x.positions() != m.as_slice()));
    let saved = match mix {
        Some(m) => {
            let mesh = cx.mesh_mut(me)?;
            let old = mesh.positions().to_vec();
            mesh.set_positions(m);
            Some(old)
        }
        None => None,
    };
    let applied = {
        let (host, _) = cx.split_eval();
        EvalCache::new().apply_modifier(host, ob, i)
    };
    if let Some(old) = saved {
        cx.mesh_mut(me)?.set_positions(old);
    }
    let applied = match applied {
        Ok(a) => a,
        Err(frap_modifiers::ModError::Error(m)) if stack::is_limit_error(&m) => {
            return Err(OpError::Limit(m));
        }
        Err(frap_modifiers::ModError::Error(m)) if stack::is_frap_limitation(&m) => {
            return Err(OpError::NotImplemented(if m.starts_with("frap:") {
                m
            } else {
                format!("frap: {m}; it cannot be applied")
            }));
        }
        Err(e) => return Err(OpError::Runtime(e.to_string())),
    };
    for r in applied.reports {
        cx.report(r.level, r.message);
    }
    let Some(result) = applied.mesh else {
        return Err(OpError::Runtime(
            "Modifier is disabled or returned error, skipping apply".into(),
        ));
    };
    let positions = cx
        .main()
        .meshes
        .get(me)
        .map(|m| m.positions().to_vec())
        .unwrap_or_default();
    let (main, log) = cx.main_mut();
    let had_key = main
        .meshes
        .get(me)
        .and_then(|m| m.shape_keys)
        .is_some_and(|k| main.shape_keys.get(k).is_some());
    let k = Key::ensure_for_mesh(main, log, me)
        .ok_or_else(|| OpError::Runtime("the mesh was removed".into()))?;
    let key = main
        .shape_keys
        .get_mut(k, log)
        .ok_or_else(|| OpError::Runtime("the key was removed".into()))?;
    if !had_key {
        let b = key.add_block(None);
        key.key_blocks[b].data = positions;
    }
    let b = key.add_block(Some(&name));
    key.key_blocks[b].data = result.positions().to_vec();
    Ok(())
}

#[operator(
    poll = APPLY_POLL,
    tier = T2,
    support = Partial("meshes only; a type without a frap evaluator cannot be applied (NotImplementedError)")
)]
fn exec(cx: &mut OpCtx, p: object::ModifierApplyAsShapekey) -> OpResult {
    let name = p.modifier.value.clone();
    let objects = shared::edit_objects(&cx.ctx(), p.use_selected_objects.value);
    let mut changed = false;
    let mut found = false;
    let mut reported = 0usize;
    for &ob in &objects {
        let Some((i, in_edit)) = cx
            .main()
            .objects
            .get(ob)
            .and_then(|o| shared::find(o, &name).map(|i| (i, o.mode == ObjectMode::Edit)))
        else {
            continue;
        };
        found = true;
        if in_edit {
            return Err(OpError::Runtime(
                "Modifiers cannot be applied in edit mode".into(),
            ));
        }
        if i != 0 {
            cx.report(
                ReportType::Info,
                "Applied modifier was not first, result may not be as expected",
            );
            reported += 1;
        }
        apply_shape(cx, ob, i)?;
        if !p.keep_modifier.value {
            edit::remove(&mut cx.object_mut(ob)?.modifiers, i);
        }
        changed = true;
    }
    if !changed {
        if !found {
            shared::report_missing(cx, &objects, &name);
        }
        return Ok(OpStatus::Cancelled);
    }
    if p.report.value && reported == 0 {
        cx.report(ReportType::Info, format!("Applied modifier: {name}"));
    }
    Ok(OpStatus::Finished)
}
