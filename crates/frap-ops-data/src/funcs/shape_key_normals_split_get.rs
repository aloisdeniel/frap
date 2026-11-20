// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShapeKey.normals_split_get()` (`rna_KeyBlock_normals_loop_calc`, WP 6.3): the corner
//! normals of the key's mesh with the block's coordinates (sharp edges, sharp faces and custom
//! normals of the mesh honoured), flattened.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::shape_key_normals_vertex_get::{Normals, block_normals};

#[rna_func(srna = "ShapeKey", func = "normals_split_get")]
fn shape_key_normals_split_get(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    block_normals(h, this, Normals::Split)
}
