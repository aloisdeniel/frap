// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.World` (§5.6). The field set is frozen at M1 W0 (decision log W0-21); WP 1.10
//! owns the bindings, WP 1.11 the default graph (Background → World Output).

use frap_anim::AnimData;
use frap_nodes::{NodeTree, NodeTreeType};
use frap_rna::*;

use crate::id::material::SHADER_NODETREE_NAME;

/// World.
#[derive(Clone, Debug)]
pub struct World {
    /// ID header.
    pub header: IdHeader,
    /// `World.node_tree` (embedded, `Root::Embedded { owner, slot: NodeTree }`).
    pub node_tree: NodeTree,
    /// `World.color` (viewport/Workbench background, linear RGB)
    pub color: [f32; 3],
    /// `World.use_nodes` (deprecated, True in 5.x)
    pub use_nodes: bool,
    /// `World.animation_data`
    pub animation_data: Option<AnimData>,
}

impl World {
    /// A world with an **empty** embedded tree (WP 1.11 adds the default graph) and the
    /// factory `color` (0.0509, verified on 5.2.4).
    pub fn new(name: &str) -> World {
        World {
            header: IdHeader::new(name),
            node_tree: NodeTree::new(SHADER_NODETREE_NAME, NodeTreeType::Shader, true),
            color: [0.050_876_09; 3],
            use_nodes: true,
            animation_data: None,
        }
    }
}

crate::id::id_rna! { World as frap_spec_rna::WORLD, "WORLD" {
    color => color,
    node_tree => embedded(node_tree, NodeTree),
    // Deprecated in 5.x: always True, writes are ignored (as `Material.use_nodes`).
    use_nodes => get(|w| w.use_nodes) set(|_w, _v: bool| ()),
}}

impl IdType for World {
    type Kind = WorldId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        self.node_tree.for_each_ref(f);
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        self.node_tree.remap_ref(from, to);
        crate::id::remap_action(&mut self.animation_data, from, to);
    }
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[EmbeddedSlot::NodeTree]
    }
}
