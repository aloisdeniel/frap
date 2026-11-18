// SPDX-License-Identifier: GPL-3.0-or-later
//! Scenes → glTF `scenes[]` (M3, WP 3.10; created by M3 W0 with the M1 behaviour moved here
//! from `write.rs`): every Blender scene becomes a glTF scene with the active one as `scene`,
//! or only the active one with `use_active_scene=True` (io_scene_gltf2 5.2.41
//! `blender/exp/scenes.py`, `__gather_scenes`). M1 exports the context scene only.

use frap_gltf::json::{Index, Scene};

use crate::export::gather::Gather;
use crate::export::{ExportError, extras, nodes, vtree};

/// Gathers the scene(s) and their node trees into `g.doc`, setting `g.doc.scene`.
pub fn gather(g: &mut Gather) -> Result<(), ExportError> {
    let tree = vtree::build(g.main, g.ctx, g.settings);
    let mut roots: Vec<Index> = Vec::with_capacity(tree.roots.len());
    for &r in &tree.roots {
        roots.push(nodes::gather_node(g, &tree, r)?);
    }
    let ctx = g.ctx;
    let scene = ctx.scene();
    let name = if g.settings.collection.is_empty() {
        scene.header.name.to_string()
    } else {
        g.settings.collection.clone()
    };
    let extras = extras::scene(g, scene);
    g.doc.scenes.push(Scene {
        name: Some(name),
        nodes: roots,
        extras,
        ..Scene::default()
    });
    g.doc.scene = Some(0);
    Ok(())
}
