// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.gltf2_material_to_variant()`, tier T0
//! (`tests/golden/cases/gltf/variants_ops/*.py`; M8 W0, decision log M8W0-6):
//! `SCENE_OT_gltf2_material_to_variant` of the glTF add-on (io_scene_gltf2 5.2.41
//! `blender/com/gltf2_blender_ui.py`, Apache-2.0), poll: the active object has a material slot and
//! Mesh.gltf2_variant_pointer is set. Registered only while `KHR_materials_variants_ui` is on (else
//! Blender's unknown-operator error). The body is `frap_scene::gltf_variants::material_to_variant`.

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
static GATE: OpGate = support::gate("scene.gltf2_material_to_variant");

/// `len(material_slots) > 0 and context.object.data.gltf2_variant_pointer != ""`
fn pointer_poll(c: &Context) -> Result<(), PollFail> {
    let ob = support::object_with_slots(c)?;
    if gltf_variants::pointer(c.main, ob).is_some_and(|p| !p.is_empty()) {
        Ok(())
    } else {
        Err(PollFail {
            message: None,
            unmet: Default::default(),
            hint: Some(
                "name a variant first: bpy.context.object.data.gltf2_variant_pointer = 'VariantName'"
                    .into(),
            ),
        })
    }
}

/// The poll passed before the statement, so the context still has the object.
fn poll_gone(f: PollFail) -> OpError {
    OpError::Runtime(f.hint.unwrap_or_default())
}

const POLL: Poll = Poll {
    requires: &[],
    check: pointer_poll,
};

#[operator(
    poll = POLL,
    spec = scene::OPSPEC_SCENE_OT_GLTF2_MATERIAL_TO_VARIANT,
    tier = T0,
    notes = ["glTF add-on operator, registered while KHR_materials_variants_ui is on; re-points the active entry of the active slot's mapping to the variant Mesh.gltf2_variant_pointer names (CANCELLED without a match)"]
)]
fn exec(cx: &mut OpCtx, _p: scene::Gltf2MaterialToVariant) -> OpResult {
    let ob = support::object_with_slots(&cx.ctx()).map_err(poll_gone)?;
    let (main, log) = cx.main_mut();
    if gltf_variants::material_to_variant(main, ob, log).map_err(support::exec_error)? {
        Ok(OpStatus::Finished)
    } else {
        Ok(OpStatus::Cancelled)
    }
}
