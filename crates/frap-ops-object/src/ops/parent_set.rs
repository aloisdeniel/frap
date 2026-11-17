// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.parent_set(type='OBJECT', xmirror=False, keep_transform=False)`, tier T0
//! (`tests/golden/cases/object/parent/*.py`). Port of `parent_set_exec`,
//! `parent_set_nonvertex_parent` and `ED_object_parent_set` (`editors/object/
//! object_relations.cc`) with `BKE_object_workob_calc_parent`
//! (`blenkernel/intern/object.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4:
//! - the active object becomes the parent of every other selected editable object; the
//!   selection and the active object stay; `matrix_parent_inverse` becomes the inverse of the
//!   parent's world matrix and the child's channels are kept, so a child that had no parent
//!   does not move, while one that had another parent jumps to its own channels unless
//!   `keep_transform=True` (then its channels first take its world matrix, as evaluated before
//!   the operator);
//! - a parent loop: `RuntimeError: Error: Loop in parents` (Blender stops at that child;
//!   frap rolls the whole operator back, D-44 candidate);
//! - `CURVE`, `LATTICE` and the `ARMATURE*` types parent like `OBJECT` (`parent_type` stays
//!   `'OBJECT'` when the parent is not a curve, lattice or armature) and also add a `Curve`,
//!   `Lattice` or `Armature` modifier pointing at the parent to every child that can have
//!   modifiers (a mesh), whatever the parent's type (`support::deform`); `BONE`/`BONE_RELATIVE`
//!   without an armature parent: `Error: No active bone`; `FOLLOW`/`PATH_CONST` without a
//!   curve parent: `CANCELLED`, nothing changes;
//! - poll `ED_operator_object_active_editable` (no active object: `poll() failed, context is
//!   incorrect`).
//!
//! Vertex parents (`VERTEX`, `VERTEX_TRI`) and armature/bone parenting arrive with Edit Mode
//! (M2) and armatures (M6).

use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::IdCode;
use frap_scene::id::transform::would_cycle;
use frap_spec_ops_object::object::{self, parent_set::Type};

use crate::support::deform;
use crate::support::xform::{self, Worlds};

#[operator(
    poll = poll::ACTIVE_EDITABLE,
    tier = T0,
    support = Partial(
        "type='VERTEX'/'VERTEX_TRI' (Edit Mode, M2) and armature or bone parents (M6) answer \
         NotImplementedError"
    ),
    notes = [
        "A parent loop (Error: Loop in parents) rolls the whole operator back; Blender keeps the \
         children parented before the loop."
    ]
)]
fn exec(cx: &mut OpCtx, p: object::ParentSet) -> OpResult {
    let ty = p.r#type.value;
    let keep = p.keep_transform.value;
    let (par, children) = {
        let c = cx.ctx();
        let Some(par) = c.active_object() else {
            return Err(OpError::Runtime("no active object".into()));
        };
        let children: Vec<_> = c
            .selected_editable_objects()
            .into_iter()
            .filter(|o| *o != par)
            .collect();
        (par, children)
    };
    if children.is_empty() {
        return Ok(OpStatus::Finished);
    }
    let par_code = cx
        .main()
        .objects
        .get(par)
        .and_then(|o| o.data)
        .map(|d| d.code);
    let par_is_armature = par_code == Some(IdCode::AR);
    match ty {
        Type::Vertex | Type::VertexTri => {
            return Err(OpError::NotImplemented(
                "frap: vertex parenting (type='VERTEX'/'VERTEX_TRI') arrives with Edit Mode in M2"
                    .into(),
            ));
        }
        Type::Bone | Type::BoneRelative if !par_is_armature => {
            return Err(OpError::Runtime("No active bone".into()));
        }
        Type::Follow | Type::PathConst => {
            // Needs a curve parent (frap has no curves): `parent_set` returns false silently.
            cx.report(
                ReportType::Warning,
                format!(
                    "frap: parent_set(type='{}') needs a curve parent; nothing changed",
                    ty.identifier()
                ),
            );
            return Ok(OpStatus::Cancelled);
        }
        Type::Bone
        | Type::BoneRelative
        | Type::Armature
        | Type::ArmatureName
        | Type::ArmatureAuto
        | Type::ArmatureEnvelope
            if par_is_armature =>
        {
            return Err(OpError::NotImplemented(
                "frap: parenting to an armature arrives with armatures in M6".into(),
            ));
        }
        _ => {}
    }
    let modifier = deform::parent_type_modifier(ty.identifier());
    let selected: Vec<frap_rna::IdAny> = cx
        .ctx()
        .selected_objects()
        .into_iter()
        .map(Into::into)
        .collect();
    let worlds = Worlds::of(cx.main());
    let (main, log) = cx.main_mut();
    let par_world = worlds.get(par);
    let inv = par_world.inverse();
    for ob in children {
        if would_cycle(&main.objects, ob, par) {
            return Err(OpError::Runtime("Loop in parents".into()));
        }
        let Some(o) = main.objects.get_mut(ob, log) else {
            continue;
        };
        if keep {
            xform::apply_mat4(o, &worlds.get(ob), false, None);
        }
        o.parent = Some(par);
        o.parent_type = "OBJECT".into();
        o.parent_bone = Default::default();
        o.matrix_parent_inverse = inv;
        if let Some(mt) = modifier {
            deform::add_parent_deform(o, mt, par.into(), &|t| selected.contains(&t));
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}
