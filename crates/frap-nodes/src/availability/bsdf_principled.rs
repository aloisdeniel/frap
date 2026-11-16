// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeBsdfPrincipled` socket availability. Port of `node_shader_update_principled`
//! (`source/blender/nodes/shader/nodes/node_shader_bsdf_principled.cc`, Blender 5.2,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4: `Subsurface IOR` is enabled only for
//! `subsurface_method='RANDOM_WALK_SKIN'`; the internal `Weight` input is always disabled.

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let skin = node.prop_str("subsurface_method") == "RANDOM_WALK_SKIN";
    node.set_input_enabled("Subsurface IOR", skin);
    node.set_input_enabled("Weight", false);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static BSDF_PRINCIPLED: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeBsdfPrincipled",
    update,
};
