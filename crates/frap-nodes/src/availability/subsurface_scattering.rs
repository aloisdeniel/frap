// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeSubsurfaceScattering` socket availability.
//!
//! Verified on 5.2.4: `IOR` and `Anisotropy` are off for `falloff='BURLEY'`; `Roughness` is on
//! only for RANDOM_WALK and RANDOM_WALK_LEGACY.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_subsurface_scattering.cc
// (`node_shader_update_subsurface_scattering`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let falloff = node.prop_str("falloff").to_owned();
    let random_walk = matches!(falloff.as_str(), "RANDOM_WALK" | "RANDOM_WALK_LEGACY");
    node.set_input_enabled("IOR", falloff != "BURLEY");
    node.set_input_enabled("Anisotropy", falloff != "BURLEY");
    node.set_input_enabled("Roughness", random_walk);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static SUBSURFACE_SCATTERING: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeSubsurfaceScattering",
    update,
};
