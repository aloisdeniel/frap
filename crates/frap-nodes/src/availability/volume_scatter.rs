// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShaderNodeVolumeScatter` socket availability: the phase-function rule of
//! `volume_coefficients.rs` (same sockets, verified on 5.2.4 for every `phase`).

// Ported from blender/source/blender/nodes/shader/nodes/node_shader_volume_scatter.cc
// (`node_shader_update_scatter`, Blender 5.2.4, GPL-2.0-or-later).

use crate::NodeAvailability;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(crate::NODE_AVAILABILITY)]
#[linkme(crate = frap_rna::linkme)]
static VOLUME_SCATTER: NodeAvailability = NodeAvailability {
    bl_idname: "ShaderNodeVolumeScatter",
    update: super::volume_coefficients::update_phase,
};
