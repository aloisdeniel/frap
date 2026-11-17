// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeMapping` socket availability. Port of `node_shader_update_mapping`
//! (`source/blender/nodes/shader/nodes/node_shader_mapping.cc`, Blender 5.2,
//! GPL-2.0-or-later).
//!
//! Verified on 5.2.4: `Location` is enabled only for `vector_type` POINT and TEXTURE (a
//! direction or normal is not translated); `Vector`, `Rotation` and `Scale` are always on.

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let translates = matches!(node.prop_str("vector_type"), "POINT" | "TEXTURE");
    node.set_input_enabled("Location", translates);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static MAPPING: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeMapping",
    update,
};
