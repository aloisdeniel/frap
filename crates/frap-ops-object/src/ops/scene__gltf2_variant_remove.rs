// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.gltf2_variant_remove()`, tier T0 (`tests/golden/cases/gltf/variants_ops/*.py`; M8
//! W0, decision log M8W0-6): `SCENE_OT_gltf2_variant_remove` of the glTF add-on (io_scene_gltf2
//! 5.2.41 `blender/com/gltf2_blender_ui.py`, Apache-2.0), poll: bpy.data.scenes[0] has a variant.
//! Registered only while `KHR_materials_variants_ui` is on (else Blender's unknown-operator error).
//! The body is `frap_scene::gltf_variants::variant_remove`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{OpGate, Poll};
use frap_scene::gltf_variants;
use frap_spec_ops_object::scene;

use crate::support::gltf_variants as support;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_op::linkme::distributed_slice(frap_op::OP_GATES)]
#[linkme(crate = frap_op::linkme)]
static GATE: OpGate = support::gate("scene.gltf2_variant_remove");

const POLL: Poll = Poll {
    requires: &[],
    check: support::has_variants,
};

#[operator(
    poll = POLL,
    spec = scene::OPSPEC_SCENE_OT_GLTF2_VARIANT_REMOVE,
    tier = T0,
    notes = ["glTF add-on operator, registered while KHR_materials_variants_ui is on; removes bpy.data.scenes[0]'s active variant and the mesh mappings that use it (the add-on's index handling)"]
)]
fn exec(cx: &mut OpCtx, _p: scene::Gltf2VariantRemove) -> OpResult {
    let (main, log) = cx.main_mut();
    gltf_variants::variant_remove(main, log);
    Ok(OpStatus::Finished)
}
