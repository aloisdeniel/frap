// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeMapRange` socket availability.
//!
//! Verified on 5.2.4 for every (`data_type`, `interpolation_type`) pair: the sockets of the
//! data type's socket type (FLOAT: the six float inputs and `Result`; FLOAT_VECTOR: the six
//! vector inputs and `Vector`) are on, except that type's `Steps` input (index 5 or 11) unless
//! `interpolation_type='STEPPED'`. `clamp` changes no socket.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_map_range.cc
// (`node_shader_update_map_range`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability, SocketType};

/// Index of the float `Steps` input.
const STEPS_FLOAT: usize = 5;
/// Index of the vector `Steps_FLOAT3` input.
const STEPS_VECTOR: usize = 11;

fn update(node: &mut Node) {
    let ty = if node.prop_str("data_type") == "FLOAT" {
        SocketType::Value
    } else {
        SocketType::Vector
    };
    let steps = if ty == SocketType::Value {
        STEPS_FLOAT
    } else {
        STEPS_VECTOR
    };
    let stepped = node.prop_str("interpolation_type") == "STEPPED";
    for (i, s) in node.inputs.iter_mut().enumerate() {
        s.enabled = s.ty == ty && (stepped || i != steps);
    }
    for s in &mut node.outputs {
        s.enabled = s.ty == ty;
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static MAP_RANGE: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeMapRange",
    update,
};
