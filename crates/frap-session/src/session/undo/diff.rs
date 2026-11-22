// SPDX-License-Identifier: GPL-3.0-or-later
//! What an undo statement brought back: the change set between the state before and after it
//! (the response's `changes`, ARCHITECTURE §13.2). A datablock is *added* when it exists only
//! afterwards, *removed* when it existed only before (with its last name), *modified* when its
//! generation (bumped on every mutable access, §5.3) or its edit data (edit BMesh generation,
//! edit armature) differs, and *renamed* when its name differs.

use std::collections::BTreeMap;
use std::sync::Arc;

use frap_rna::{IdAny, IdKey};
use frap_scene::ChangeLog;

use crate::session::State;

/// The change set from `before` to `after`.
pub(crate) fn changes(before: &State, after: &State) -> ChangeLog {
    let mut log = ChangeLog::default();
    let index = |s: &State| -> BTreeMap<IdKey, (IdAny, u64, smol_str::SmolStr)> {
        s.main
            .all_ids()
            .into_iter()
            .filter_map(|a| {
                let h = s.main.header(a)?;
                Some((a.key, (a, h.generation, h.name.clone())))
            })
            .collect()
    };
    let (b, a) = (index(before), index(after));
    for (key, (id, generation, name)) in &a {
        match b.get(key) {
            None => log.added.push(*id),
            Some((_, g0, n0)) => {
                if g0 != generation {
                    log.modified.insert(*id);
                }
                if n0 != name {
                    log.renamed.push((*id, n0.clone()));
                }
            }
        }
    }
    for (key, (id, _, name)) in &b {
        if !a.contains_key(key) {
            log.removed.push((*id, name.clone()));
        }
    }
    // Edit data lives outside `bpy.data`: a mesh whose edit BMesh came back, went away or
    // changed is modified, and so is an armature's edit data.
    let mut touch = |key: IdKey| {
        if let Some((id, _, _)) = a.get(&key)
            && !log.added.contains(id)
        {
            log.modified.insert(*id);
        }
    };
    let (em0, em1) = (&before.edit.meshes, &after.edit.meshes);
    for k in em0.keys().chain(em1.keys()) {
        let same = match (em0.get(k), em1.get(k)) {
            (Some(x), Some(y)) => x.bm_gen == y.bm_gen && Arc::ptr_eq(&x.bm, &y.bm),
            (None, None) => true,
            _ => false,
        };
        if !same {
            touch(*k);
        }
    }
    let (ea0, ea1) = (&before.edit.armatures, &after.edit.armatures);
    for k in ea0.keys().chain(ea1.keys()) {
        let same = match (ea0.get(k), ea1.get(k)) {
            (Some(x), Some(y)) => Arc::ptr_eq(x, y),
            (None, None) => true,
            _ => false,
        };
        if !same {
            touch(*k);
        }
    }
    log.context_changed = before.win.scene != after.win.scene
        || before.win.view_layer != after.win.view_layer
        || !log.added.is_empty()
        || !log.removed.is_empty()
        || !log.modified.is_empty();
    log
}
