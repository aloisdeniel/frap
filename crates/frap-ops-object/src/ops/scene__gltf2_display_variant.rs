// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.gltf2_display_variant()`, tier T0 (`tests/golden/cases/gltf/variants_ops/*.py`;
//! M8 W0, decision log M8W0-6): `SCENE_OT_gltf2_display_variant` of the glTF add-on (io_scene_gltf2
//! 5.2.41 `blender/com/gltf2_blender_ui.py`, Apache-2.0), poll: bpy.data.scenes[0] has a variant.
//! Registered only while `KHR_materials_variants_ui` is on (else Blender's unknown-operator error).
//! The body is `frap_scene::gltf_variants::display_variant`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{OpGate, Poll};
use frap_scene::gltf_variants;
use frap_spec_ops_object::scene;

use crate::support::gltf_variants as support;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_op::linkme::distributed_slice(frap_op::OP_GATES)]
#[linkme(crate = frap_op::linkme)]
static GATE: OpGate = support::gate("scene.gltf2_display_variant");

const POLL: Poll = Poll {
    requires: &[],
    check: support::has_variants,
};

#[operator(
    poll = POLL,
    spec = scene::OPSPEC_SCENE_OT_GLTF2_DISPLAY_VARIANT,
    tier = T0,
    notes = ["glTF add-on operator, registered while KHR_materials_variants_ui is on; every mesh object's slots take the materials its mesh maps to bpy.data.scenes[0].gltf2_active_variant"]
)]
fn exec(cx: &mut OpCtx, _p: scene::Gltf2DisplayVariant) -> OpResult {
    let (main, log) = cx.main_mut();
    gltf_variants::display_variant(main, log).map_err(support::exec_error)?;
    Ok(OpStatus::Finished)
}
