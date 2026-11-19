// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeVectorRotate` socket availability.
//!
//! Verified on 5.2.4: `Axis` only for AXIS_ANGLE, `Rotation` only for EULER_XYZ, `Angle` for
//! every type but EULER_XYZ; `invert` changes no socket.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_vector_rotate.cc
// (`node_shader_vector_rotate_update`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

fn update(node: &mut Node) {
    let ty = node.prop_str("rotation_type").to_owned();
    let euler = ty == "EULER_XYZ";
    node.set_input_enabled("Rotation", euler);
    node.set_input_enabled("Axis", ty == "AXIS_ANGLE");
    node.set_input_enabled("Angle", !euler);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static VECTOR_ROTATE: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeVectorRotate",
    update,
};
