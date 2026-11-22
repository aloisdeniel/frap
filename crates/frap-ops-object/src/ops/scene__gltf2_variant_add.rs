// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.scene.gltf2_variant_add()`, tier T0 (`tests/golden/cases/gltf/variants_ops/*.py`; M8
//! W0, decision log M8W0-6): `SCENE_OT_gltf2_variant_add` of the glTF add-on (io_scene_gltf2 5.2.41
//! `blender/com/gltf2_blender_ui.py`, Apache-2.0), poll: always (registered while the preference is
//! on). Registered only while `KHR_materials_variants_ui` is on (else Blender's unknown-operator
//! error). The body is `frap_scene::gltf_variants::variant_add`.

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
static GATE: OpGate = support::gate("scene.gltf2_variant_add");

/// `poll` returns `True`.
fn always(_c: &Context) -> Result<(), PollFail> {
    Ok(())
}

const POLL: Poll = Poll {
    requires: &[],
    check: always,
};

#[operator(
    poll = POLL,
    spec = scene::OPSPEC_SCENE_OT_GLTF2_VARIANT_ADD,
    tier = T0,
    notes = ["glTF add-on operator, registered while KHR_materials_variants_ui is on; appends a variant to bpy.data.scenes[0] (named VariantName, made unique) and makes it active"]
)]
fn exec(cx: &mut OpCtx, _p: scene::Gltf2VariantAdd) -> OpResult {
    let (main, log) = cx.main_mut();
    gltf_variants::variant_add(main, log);
    Ok(OpStatus::Finished)
}
