// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.world.new()`, tier T0 (`tests/golden/cases/nodes/world/*.py`, WP 3.1).
//!
//! Verified on 5.2.4 (`--factory-startup`, scripts and VIEW_3D overrides):
//! - with no `world` context member (only the Properties editor has one, scripts and the 3D
//!   View do not) it adds `World` (uniquified: `World.001`) through `BKE_world_add` +
//!   `node_tree_shader_default`: `color` 0.05 grey (not the factory World's 0.0509), an
//!   embedded tree "World Nodetree" with `Background` at (−200, 100) (its colour copied from
//!   the world colour, Strength 1) linked to `World Output` at (200, 100), the same graph as
//!   `bpy.data.worlds.new()`;
//! - the new world is **not** assigned to `scene.world` (the assignment goes through the
//!   Properties editor's `template_ID`), also when the scene has no world;
//! - with a `world` context member (Properties editor) it copies that world instead; frap has
//!   no such member, so that branch is unreachable from frap scripts;
//! - no poll: it always runs.
//!
//! Deviation (as `material.new`): Blender's new world has one dangling user, so it survives a
//! save; frap's has none, like `bpy.data.worlds.new`, and an unused one is dropped on save.

// Ported from blender/source/blender/editors/render/render_shading.cc (`new_world_exec`) and
// blender/source/blender/nodes/intern/node_tree_defaults.cc (`node_tree_shader_default`),
// Blender 5.2.4, GPL-2.0-or-later.

use frap_nodes::{NodeTree, SocketValue};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::World;
use frap_spec_ops_data::world;

/// `World.horr` of `BKE_world_add` (DNA default, verified: `bpy.data.worlds.new().color`).
const NEW_WORLD_COLOR: [f32; 3] = [0.05; 3];

/// `BKE_world_add(bmain, "World")` + `node_tree_shader_default`: the world and its default
/// graph, the Background colour copied from the world colour.
fn default_world(name: &str) -> World {
    let mut w = World::new(name);
    w.color = NEW_WORLD_COLOR;
    w.node_tree = NodeTree::default_world();
    let [r, g, b] = w.color;
    if let Some(bg) = w
        .node_tree
        .nodes
        .iter_mut()
        .find(|n| n.bl_idname == "ShaderNodeBackground")
        && let Some(c) = bg.input_mut("Color")
    {
        c.default_value = SocketValue::Color([r, g, b, 1.0]);
    }
    w
}

#[operator(
    poll = poll::ALWAYS,
    tier = T0,
    spec = world::OPSPEC_WORLD_OT_NEW,
    notes = [
        "does not assign the world to the scene (as Blender from scripts); use bpy.context.scene.world = bpy.data.worlds['World.001']",
    ]
)]
fn exec(cx: &mut OpCtx, _p: world::WorldNew) -> OpResult {
    let w = default_world("World");
    let (main, log) = cx.main_mut();
    let r = main.worlds.insert("World", w, log);
    cx.result("world", r.any());
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_world_matches_blender() {
        let w = default_world("World");
        assert_eq!(w.color, [0.05; 3]);
        assert_eq!(w.node_tree.header.name, "World Nodetree");
        let names: Vec<_> = w.node_tree.nodes.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(names, ["Background", "World Output"]);
        let bg = &w.node_tree.nodes[0];
        assert_eq!(bg.location, [-200.0, 100.0]);
        assert_eq!(
            bg.input("Color").map(|s| &s.default_value),
            Some(&SocketValue::Color([0.05, 0.05, 0.05, 1.0]))
        );
        assert_eq!(
            bg.input("Strength").map(|s| &s.default_value),
            Some(&SocketValue::Float(1.0))
        );
        assert_eq!(w.node_tree.nodes[1].location, [200.0, 100.0]);
        assert_eq!(w.node_tree.links.len(), 1);
    }
}
