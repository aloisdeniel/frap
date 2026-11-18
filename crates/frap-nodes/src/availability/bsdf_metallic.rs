// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeBsdfMetallic` socket availability.
//!
//! Verified on 5.2.4: `fresnel_type='F82'` enables `Base Color` and `Edge Tint`,
//! `PHYSICAL_CONDUCTOR` enables `IOR` and `Extinction` instead; `distribution` changes no
//! socket.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_bsdf_metallic.cc
// (`node_shader_update_metallic`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let physical = node.prop_str("fresnel_type") == "PHYSICAL_CONDUCTOR";
    node.set_input_enabled("Base Color", !physical);
    node.set_input_enabled("Edge Tint", !physical);
    node.set_input_enabled("IOR", physical);
    node.set_input_enabled("Extinction", physical);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static BSDF_METALLIC: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeBsdfMetallic",
    update,
};
