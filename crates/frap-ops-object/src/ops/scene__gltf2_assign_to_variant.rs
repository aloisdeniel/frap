// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.gltf2_assign_to_variant()`, tier T0 (`tests/golden/cases/gltf/variants_ops/*.py`;
//! M8 W0, decision log M8W0-6): `SCENE_OT_gltf2_assign_to_variant` of the glTF add-on
//! (io_scene_gltf2 5.2.41 `blender/com/gltf2_blender_ui.py`, Apache-2.0), poll: a variant exists
//! and the active object is a mesh. Registered only while `KHR_materials_variants_ui` is on (else
//! Blender's unknown-operator error). The body is `frap_scene::gltf_variants::assign_to_variant`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{OpGate, Poll, PollFail};
use frap_scene::Context;
use frap_scene::gltf_variants;
use frap_spec_ops_object::scene;

use crate::support::gltf_variants as support;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_op::linkme::distributed_slice(frap_op::OP_GATES)]
#[linkme(crate = frap_op::linkme)]
static GATE: OpGate = support::gate("scene.gltf2_assign_to_variant");

/// `variants and bpy.context.object and bpy.context.object.type == "MESH"`
fn assign_poll(c: &Context) -> Result<(), PollFail> {
    support::has_variants(c)?;
    support::mesh_object(c).map(|_| ())
}

/// The poll passed before the statement, so the context still has the object.
fn poll_gone(f: PollFail) -> OpError {
    OpError::Runtime(f.hint.unwrap_or_default())
}

const POLL: Poll = Poll {
    requires: &[],
    check: assign_poll,
};

#[operator(
    poll = POLL,
    spec = scene::OPSPEC_SCENE_OT_GLTF2_ASSIGN_TO_VARIANT,
    tier = T0,
    notes = ["glTF add-on operator, registered while KHR_materials_variants_ui is on; records the active object's slot materials as the active variant's"]
)]
fn exec(cx: &mut OpCtx, _p: scene::Gltf2AssignToVariant) -> OpResult {
    let ob = support::mesh_object(&cx.ctx()).map_err(poll_gone)?;
    let (main, log) = cx.main_mut();
    gltf_variants::assign_to_variant(main, ob, log).map_err(support::exec_error)?;
    Ok(OpStatus::Finished)
}
