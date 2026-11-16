// SPDX-License-Identifier: GPL-3.0-or-later
//! Socket availability extension point (ARCHITECTURE §16.1): `availability/<node>.rs`
//! registers one [`NodeAvailability`] per node type whose `socket.enabled` depends on a mode
//! property (Mix `data_type`, Math `operation`, …). [`Node::update_availability`] runs the
//! entry of the node's type after node creation and after every node property write, so new
//! node types only add a file (§22.1).

use crate::Node;

/// Recomputes `enabled` on the sockets of one node type.
#[derive(Debug)]
pub struct NodeAvailability {
    /// Node `bl_idname` ("ShaderNodeMix").
    pub bl_idname: &'static str,
    /// Sets `enabled` on `node.inputs` / `node.outputs` from `node.props`.
    pub update: fn(&mut Node),
}

/// Every socket-availability rule (one per file in `availability/`).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice]
#[linkme(crate = frap_rna::linkme)]
pub static NODE_AVAILABILITY: [NodeAvailability];

/// The availability rule of a node type, if it has one.
pub fn availability_of(bl_idname: &str) -> Option<&'static NodeAvailability> {
    NODE_AVAILABILITY.iter().find(|a| a.bl_idname == bl_idname)
}

impl Node {
    /// Recomputes `socket.enabled` from the node's mode properties (no-op for node types
    /// without an `availability/` rule).
    pub fn update_availability(&mut self) {
        if let Some(a) = availability_of(self.bl_idname) {
            (a.update)(self);
        }
    }

    /// Sets `enabled` on the input with this identifier (no-op if absent).
    pub fn set_input_enabled(&mut self, identifier: &str, enabled: bool) {
        if let Some(s) = self.inputs.iter_mut().find(|s| s.identifier == identifier) {
            s.enabled = enabled;
        }
    }

    /// Sets `enabled` on the output with this identifier (no-op if absent).
    pub fn set_output_enabled(&mut self, identifier: &str, enabled: bool) {
        if let Some(s) = self.outputs.iter_mut().find(|s| s.identifier == identifier) {
            s.enabled = enabled;
        }
    }
}
