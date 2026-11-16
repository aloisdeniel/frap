// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Material` (§5.6). The field set is frozen at M1 W0 (decision log W0-21): the
//! shading lives in the embedded `node_tree` (Principled BSDF inputs, lowered by
//! `frap-material`, §16); `diffuse_color`/`metallic`/`roughness` are the material's own
//! **viewport display** properties, not the Principled inputs. WP 1.10 owns the non-node
//! bindings, WP 1.11 the node tree.

use frap_anim::AnimData;
use frap_nodes::{NodeTree, NodeTreeType};
use frap_rna::*;
use smol_str::SmolStr;

/// Name of the embedded shader tree of materials and worlds (verified).
pub const SHADER_NODETREE_NAME: &str = "Shader Nodetree";

/// Material.
#[derive(Clone, Debug)]
pub struct Material {
    /// ID header.
    pub header: IdHeader,
    /// `Material.node_tree` (embedded, always present, `is_embedded_data == True`), reached
    /// as `Root::Embedded { owner, slot: NodeTree }` and mutated through the material.
    pub node_tree: NodeTree,
    /// `Material.surface_render_method` ("DITHERED", "BLENDED")
    pub surface_render_method: SmolStr,
    /// `Material.blend_method` (deprecated mirror; fresh "HASHED")
    pub blend_method: SmolStr,
    /// `Material.use_backface_culling`
    pub use_backface_culling: bool,
    /// `Material.diffuse_color` (viewport display colour, linear RGBA)
    pub diffuse_color: [f32; 4],
    /// `Material.metallic` (viewport display)
    pub metallic: f32,
    /// `Material.roughness` (viewport display)
    pub roughness: f32,
    /// `Material.alpha_threshold`
    pub alpha_threshold: f32,
    /// `Material.pass_index`
    pub pass_index: i32,
    /// `Material.use_nodes` (deprecated, always True in 5.x)
    pub use_nodes: bool,
    /// `Material.animation_data` (node-tree channels go to `node_tree.animation_data`, §18)
    pub animation_data: Option<AnimData>,
}

impl Material {
    /// A material with the fresh values of `bpy.data.materials.new(name)` (verified on 5.2.4)
    /// and an **empty** embedded tree; the default graph (Principled BSDF + Material Output)
    /// is added by WP 1.11's `materials.new`.
    pub fn new(name: &str) -> Material {
        Material {
            header: IdHeader::new(name),
            node_tree: NodeTree::new(SHADER_NODETREE_NAME, NodeTreeType::Shader, true),
            surface_render_method: "DITHERED".into(),
            blend_method: "HASHED".into(),
            use_backface_culling: false,
            diffuse_color: [0.8, 0.8, 0.8, 1.0],
            metallic: 0.0,
            roughness: 0.4,
            alpha_threshold: 0.5,
            pass_index: 0,
            use_nodes: true,
            animation_data: None,
        }
    }
}

crate::id::name_only_rna!(Material, frap_spec_rna::MATERIAL, MaterialId);

impl IdType for Material {
    type Kind = MaterialId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        // The embedded tree's references (images, its action) count as the material's (§5.1).
        self.node_tree.for_each_ref(f);
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.any());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        self.node_tree.remap_ref(from, to);
    }
    fn embedded(&self) -> &'static [EmbeddedSlot] {
        &[EmbeddedSlot::NodeTree]
    }
}
