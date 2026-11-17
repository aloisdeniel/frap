// SPDX-License-Identifier: GPL-3.0-or-later
//! `collection.children.link(child)` (`rna_Collection_children_link` →
//! `BKE_collection_child_add`), on a `bpy.data.collections` ID or a scene's master
//! collection: appends the child collection. Verified on 5.2.4, every refusal raises
//! `RuntimeError: Error: Collection 'K' already in collection 'P'`: a child that is already a
//! direct child, the collection itself, a child that would make a cycle (the parent is
//! reachable from the child), and a scene's master collection. View layers are resynced at
//! once (the child's layer collection and its objects' bases appear).

use frap_rna::{RnaError, RnaPtr, Root, Value, rna_func};
use frap_scene::id::layers::sync_all;
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{CollTarget, arg_id, arg_ptr, internal, reaches, report_error};

#[rna_func(srna = "CollectionChildren", func = "link")]
fn collection_children_link(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let target = CollTarget::of(this)?;
    let (main, log) = h.main_mut();
    let parent_name = target.get(main)?.header.name.clone();
    let refuse = |child: &str| {
        Err(report_error(format!(
            "Collection '{child}' already in collection '{parent_name}'"
        )))
    };
    let Some(child) = arg_id(args, "child").and_then(|a| a.typed()) else {
        // A scene's master collection (an embedded ID) can never be a child.
        return match arg_ptr(args, "child").map(|p| p.root) {
            Some(Root::Embedded { owner, slot }) => {
                let name = main
                    .embedded_header(owner, slot)
                    .map_or_else(SmolStr::default, |h| h.name.clone());
                refuse(&name)
            }
            _ => Err(internal("children.link(): no collection argument")),
        };
    };
    let child_name = main
        .collections
        .name_of(child)
        .map(SmolStr::new)
        .unwrap_or_default();
    let cycle = match target {
        CollTarget::Id(p) => reaches(main, child, p),
        CollTarget::Master(_) => false,
    };
    if cycle || target.get(main)?.children.contains(&child) {
        return refuse(&child_name);
    }
    target.get_mut(main, log)?.children.push(child);
    sync_all(main, log);
    Ok(Value::None)
}
