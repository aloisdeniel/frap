// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeTexSky` socket availability.
//!
//! Verified on 5.2.4: `Vector` is off when `sky_type` is SINGLE_SCATTERING or
//! MULTIPLE_SCATTERING with `sun_disc` on (a fresh Sky Texture: MULTIPLE_SCATTERING, sun disc
//! on, so its `Vector` input starts disabled).

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_tex_sky.cc
// (`node_shader_update_sky`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let scattering = matches!(
        node.prop_str("sky_type"),
        "SINGLE_SCATTERING" | "MULTIPLE_SCATTERING"
    );
    let off = scattering && node.prop_bool("sun_disc");
    node.set_input_enabled("Vector", !off);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static TEX_SKY: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeTexSky",
    update,
};
