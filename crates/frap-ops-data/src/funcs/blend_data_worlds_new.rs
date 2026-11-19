// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.worlds.new(name)`: `BKE_world_add` + `node_tree_shader_default`, as `world.new`
//! (`ops/world__new.rs`): `color` 0.05 grey and the default graph in an embedded
//! tree named "World Nodetree" (Background, 0.05 grey, at (−200, 100) → World Output at
//! (200, 100); verified on 5.2.4: `nodes.keys() == ['Background', 'World Output']`), no users.

use frap_nodes::NodeTree;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::World;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataWorlds", func = "new")]
fn blend_data_worlds_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let mut w = World::new(&name);
    // `World.horr` of `BKE_world_add` (`World::new` has the factory World's 0.0509).
    w.color = [0.05; 3];
    w.node_tree = NodeTree::default_world();
    let (main, log) = h.main_mut();
    Ok(main.worlds.insert(&name, w, log).into())
}
