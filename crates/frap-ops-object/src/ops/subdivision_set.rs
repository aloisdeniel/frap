// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.subdivision_set(level=1, relative=False, ensure_modifier=True)`, tier T1.
//! Port of `SubdivisionSet.execute` (`scripts/startup/bl_operators/object.py`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4: for every selected editable object, the first SUBSURF modifier gets
//! `levels = level` (`+= level` with `relative`, clamped to 0–11 as the RNA property is);
//! without one, `modifiers.new("Subdivision", 'SUBSURF')` is added (before pinned modifiers)
//! with `levels = level`; an object that cannot take modifiers gets the WARNING
//! `Modifiers cannot be added to object: Camera` and the operator still finishes; no selected
//! editable object is `CANCELLED` with `No selected editable objects to operate on`;
//! `relative` with `level=0` is `CANCELLED`; `ensure_modifier=False` without any SUBSURF is
//! `CANCELLED` with `No Subdivision Surface modifiers found`.
//!
//! Multires (sculpt mode, `object.multires_subdivide`) is not implemented: a MULTIRES modifier
//! found first raises `OP_NOT_IMPLEMENTED`.

use frap_modifiers::{ModifierType, stack::edit};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::ModifierSettings;
use frap_spec_ops_object::object;

/// `SubsurfModifier.levels` hard range.
const LEVELS_MAX: i32 = 11;

#[operator(
    poll = poll::ALWAYS,
    tier = T1,
    support = Partial("MULTIRES (sculpt-mode subdivision) answers OP_NOT_IMPLEMENTED")
)]
fn exec(cx: &mut OpCtx, p: object::SubdivisionSet) -> OpResult {
    let level = p.level.value;
    let relative = p.relative.value;
    let objects = cx.ctx().selected_editable_objects();
    if objects.is_empty() {
        cx.report(
            ReportType::Warning,
            "No selected editable objects to operate on",
        );
        return Ok(OpStatus::Cancelled);
    }
    if relative && level == 0 {
        return Ok(OpStatus::Cancelled);
    }
    let first_relevant = |cx: &OpCtx, ob| {
        cx.main().objects.get(ob).and_then(|o| {
            o.modifiers.iter().position(|m| {
                matches!(
                    m.settings.type_identifier(),
                    "SUBSURF" | "MULTIRES"
                )
            })
        })
    };
    if !p.ensure_modifier.value
        && !objects.iter().any(|&ob| {
            cx.main().objects.get(ob).is_some_and(|o| {
                o.modifiers
                    .iter()
                    .any(|m| m.settings.type_identifier() == "SUBSURF")
            })
        })
    {
        cx.report(
            ReportType::Warning,
            "No Subdivision Surface modifiers found",
        );
        return Ok(OpStatus::Cancelled);
    }
    let level = if !relative && level < 0 { 0 } else { level };
    for ob in objects {
        match first_relevant(cx, ob) {
            Some(i) => {
                let o = cx.object_mut(ob)?;
                match &mut o.modifiers[i].settings {
                    ModifierSettings::Subsurf(s) => {
                        let v = if relative { s.levels + level } else { level };
                        s.levels = v.clamp(0, LEVELS_MAX);
                    }
                    _ => {
                        return Err(OpError::NotImplemented(
                            "frap: object.subdivision_set on a MULTIRES modifier \
                             (multires subdivision) is not implemented"
                                .into(),
                        ));
                    }
                }
            }
            None if p.ensure_modifier.value => {
                let Some(mut o) = cx.main().objects.get(ob).cloned() else {
                    continue;
                };
                match edit::add(&mut o, Some("Subdivision"), ModifierType::Subsurf) {
                    Ok(i) => {
                        if let ModifierSettings::Subsurf(s) = &mut o.modifiers[i].settings {
                            s.levels = level.clamp(0, LEVELS_MAX);
                        }
                        *cx.object_mut(ob)? = o;
                    }
                    Err(_) => cx.report(
                        ReportType::Warning,
                        format!("Modifiers cannot be added to object: {}", o.header.name),
                    ),
                }
            }
            None => {}
        }
    }
    Ok(OpStatus::Finished)
}
