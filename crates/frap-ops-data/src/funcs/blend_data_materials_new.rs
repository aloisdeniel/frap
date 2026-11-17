// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.data.materials.new(name)`: the fresh values of `Material::new` and the default graph
//! (Principled BSDF at (−200, 100) → Material Output at (200, 100), linked BSDF → Surface, in
//! the embedded tree "Shader Nodetree"; verified on 5.2.4: `nodes.keys() == ['Principled
//! BSDF', 'Material Output']`, `use_nodes` True, no users).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Material;
use frap_scene::{FuncArgs, Host};

use super::_common::arg_str;

#[rna_func(srna = "BlendDataMaterials", func = "new")]
fn blend_data_materials_new(
    h: &mut dyn Host,
    _this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let name = arg_str(args, "name").to_owned();
    let mut m = Material::new(&name);
    m.node_tree.add_default_material_graph();
    let (main, log) = h.main_mut();
    Ok(main.materials.insert(&name, m, log).into())
}
