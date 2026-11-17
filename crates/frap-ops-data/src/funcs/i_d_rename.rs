// SPDX-License-Identifier: GPL-3.0-or-later
//! `ID.rename(name, mode='NEVER')` (`rna_ID_rename` → `BKE_id_rename`): renames with Blender's
//! collision rules (`IdMap::rename`) and returns the status (`'UNCHANGED'`,
//! `'UNCHANGED_COLLISION'`, `'RENAMED_NO_COLLISION'`, `'RENAMED_COLLISION_ADJUSTED'`,
//! `'RENAMED_COLLISION_FORCED'`). Verified on 5.2.4: `''` gives the type's default name;
//! `ALWAYS` takes the name and renames the other ID; a linked ID can be renamed this way; an
//! embedded ID (`material.node_tree`, `scene.collection`) takes any name (no namespace,
//! `''` → `NodeTree`) and reports `'RENAMED_NO_COLLISION'`.

use frap_rna::naming::truncate_utf8;
use frap_rna::{EmbeddedSlot, IdCode, RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::{ChangeLog, FuncArgs, Host, Main, RenameMode, RenameResult};
use smol_str::SmolStr;

use super::_common::{arg_str, internal};

fn result_str(r: RenameResult) -> &'static str {
    match r {
        RenameResult::Unchanged => "UNCHANGED",
        RenameResult::UnchangedCollision => "UNCHANGED_COLLISION",
        RenameResult::RenamedNoCollision => "RENAMED_NO_COLLISION",
        RenameResult::RenamedCollisionAdjusted => "RENAMED_COLLISION_ADJUSTED",
        RenameResult::RenamedCollisionForced => "RENAMED_COLLISION_FORCED",
    }
}

/// Renames an embedded ID (no uniqueness: it is in no `bpy.data` collection).
fn rename_embedded(
    main: &mut Main,
    owner: frap_rna::IdAny,
    slot: EmbeddedSlot,
    name: &str,
    log: &mut ChangeLog,
) -> Result<RenameResult, RnaError> {
    let default = match slot {
        EmbeddedSlot::MasterCollection => "Collection",
        _ => "NodeTree",
    };
    let wanted = SmolStr::new(truncate_utf8(if name.is_empty() { default } else { name }, 255));
    let current = main
        .embedded_header(owner, slot)
        .ok_or_else(|| internal("ID.rename(): the datablock was removed"))?;
    if current.name == wanted {
        return Ok(RenameResult::Unchanged);
    }
    let header = match (owner.code, slot) {
        (IdCode::MA, EmbeddedSlot::NodeTree) => owner
            .typed()
            .and_then(|r| main.materials.get_mut(r, log))
            .map(|m| &mut m.node_tree.header),
        (IdCode::WO, EmbeddedSlot::NodeTree) => owner
            .typed()
            .and_then(|r| main.worlds.get_mut(r, log))
            .map(|w| &mut w.node_tree.header),
        (IdCode::SC, EmbeddedSlot::MasterCollection) => owner
            .typed()
            .and_then(|r| main.scenes.get_mut(r, log))
            .map(|s| &mut s.collection.header),
        _ => None,
    }
    .ok_or_else(|| internal("ID.rename(): unknown embedded ID"))?;
    header.name = wanted;
    Ok(RenameResult::RenamedNoCollision)
}

#[rna_func(srna = "ID", func = "rename")]
fn i_d_rename(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "rename")?;
    let name = arg_str(args, "name").to_owned();
    let mode = match arg_str(args, "mode") {
        "ALWAYS" => RenameMode::Always,
        "SAME_ROOT" => RenameMode::SameRoot,
        _ => RenameMode::Never,
    };
    let (main, log) = h.main_mut();
    let r = match this.root {
        Root::Id(a) => {
            if main.header(a).is_none() {
                return Err(internal("ID.rename(): the datablock was removed"));
            }
            main.rename(a, &name, mode, log)
        }
        Root::Embedded { owner, slot } => rename_embedded(main, owner, slot, &name, log)?,
        _ => return Err(internal("ID.rename(): not a datablock")),
    };
    Ok(Value::Enum(SmolStr::new_static(result_str(r))))
}
