// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeVolumeCoefficients` socket availability.
//!
//! Verified on 5.2.4 for every `phase`: `IOR` and `Backscatter` only for FOURNIER_FORAND,
//! `Anisotropy` for HENYEY_GREENSTEIN and DRAINE, `Alpha` for DRAINE, `Diameter` for MIE.

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_volume_coefficients.cc
// (`node_shader_update_coefficients`, Blender 5.2.4, GPL-2.0-or-later).

use crate::{Node, NodeAvailability};

/// The phase-function rule shared with `volume_scatter.rs` (same socket names).
pub(crate) fn update_phase(node: &mut Node) {
    let phase = node.prop_str("phase").to_owned();
    let ff = phase == "FOURNIER_FORAND";
    node.set_input_enabled("IOR", ff);
    node.set_input_enabled("Backscatter", ff);
    node.set_input_enabled(
        "Anisotropy",
        matches!(phase.as_str(), "HENYEY_GREENSTEIN" | "DRAINE"),
    );
    node.set_input_enabled("Alpha", phase == "DRAINE");
    node.set_input_enabled("Diameter", phase == "MIE");
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static VOLUME_COEFFICIENTS: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeVolumeCoefficients",
    update: update_phase,
};
