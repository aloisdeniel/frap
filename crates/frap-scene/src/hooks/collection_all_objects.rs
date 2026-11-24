// SPDX-License-Identifier: GPL-3.0-or-later
//! `Collection.all_objects`: the collection's objects then its children's, recursively, each
//! once (verified on 5.2.4: `['Light', 'Camera', 'aa_coll', 'Cube', 'mm_child']`).

use std::collections::HashSet;

use frap_rna::{CollRef, Field, IdRef, ObjectId, PropDef, RnaError, RnaPtr, Root, rna_hook};

use crate::HostRead;
use crate::data::Main;
use crate::id::Collection;
use crate::id::idlist::IdList;
use crate::id::paths::internal;

/// Objects of `c` and of its descendants (pre-order, deduplicated).
pub fn all_objects(main: &Main, c: &Collection) -> Vec<IdRef<ObjectId>> {
    fn walk(
        main: &Main,
        c: &Collection,
        depth: usize,
        seen: &mut HashSet<IdRef<ObjectId>>,
        out: &mut Vec<IdRef<ObjectId>>,
    ) {
        if depth > 256 {
            return;
        }
        for o in &c.objects {
            if seen.insert(*o) {
                out.push(*o);
            }
        }
        for ch in &c.children {
            if let Some(cc) = main.collections.get(*ch) {
                walk(main, cc, depth + 1, seen, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(main, c, 0, &mut HashSet::new(), &mut out);
    out
}

#[rna_hook(srna = "Collection", prop = "all_objects", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let c = match this.root {
        Root::Id(a) => a.typed().and_then(|r| main.collections.get(r)),
        Root::Embedded { owner, .. } => owner
            .typed()
            .and_then(|s| main.scenes.get(s))
            .map(|s| &s.collection),
        _ => None,
    }
    .ok_or_else(|| internal("Collection.all_objects: the collection was removed"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(
        all_objects(main, c),
    )))))
}
