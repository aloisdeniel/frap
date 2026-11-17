// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.copy()` (`rna_ID_copy` → `BKE_id_copy_for_use_in_bmain`): a local copy in the same
//! `bpy.data` collection, named with the lowest free `.NNN` (`Material.002` when
//! `Material.001` exists), `use_fake_user` cleared, with no users. Verified on 5.2.4: an object
//! copy shares the data, keeps parent, transform, modifiers and slot overrides, is linked to
//! no collection, is unselected and in Object Mode; a mesh copy keeps its materials; a
//! material copy has its own node tree; a collection copy links the same objects; a copy of a
//! linked ID is local. Copies of scenes and libraries, and of embedded IDs (Blender returns an
//! unowned copy of `material.node_tree`), are not implemented.

use frap_rna::{RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{copy_id, internal, not_implemented};

#[rna_func(srna = "ID", func = "copy")]
fn i_d_copy(h: &mut dyn Host, this: &RnaPtr, _args: &mut FuncArgs) -> Result<Value, RnaError> {
    let a = match this.root {
        Root::Id(a) if this.path.is_empty() => a,
        Root::Embedded { .. } => {
            return Err(not_implemented(
                "ID.copy() of an embedded ID (material.node_tree, scene.collection) is not \
                 implemented; copy its owner instead",
            ));
        }
        _ => return Err(internal("ID.copy(): not a datablock")),
    };
    let (main, log) = h.main_mut();
    Ok(copy_id(main, a, log)?.into())
}
