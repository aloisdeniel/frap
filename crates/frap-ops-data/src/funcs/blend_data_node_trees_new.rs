// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.node_groups.new(name, type)` (`rna_Main_nodetree_new`, WP 3.1): an empty node
//! group (no node, no interface item), not embedded, no users, no fake user; the name made
//! unique (`G.001`). Verified on 5.2.4 for every type; `type` is required (`TypeError:
//! BlendDataNodeTrees.new(): required parameter "type" not specified`) and checked against
//! `('GeometryNodeTree', 'CompositorNodeTree', 'ShaderNodeTree', 'TextureNodeTree')`.
//!
//! frap has shader and geometry trees only: compositor and texture trees raise
//! `NotImplementedError`.

use frap_nodes::{NodeTree, NodeTreeType};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_str, not_implemented};

#[rna_func(srna = "BlendDataNodeTrees", func = "new")]
fn blend_data_node_trees_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let ty = match arg_str(args, "type") {
        "ShaderNodeTree" => NodeTreeType::Shader,
        "GeometryNodeTree" => NodeTreeType::Geometry,
        other => {
            return Err(not_implemented(format!(
                "{other} node groups are not implemented (frap has shader and geometry trees)"
            )));
        }
    };
    let (main, log) = h.main_mut();
    Ok(main
        .node_groups
        .insert(&name, NodeTree::new(&name, ty, false), log)
        .into())
}
