// SPDX-License-Identifier: GPL-3.0-or-later
//! `Scene.objects`: every object of the scene's collection hierarchy, each once, in
//! `FOREACH_SCENE_OBJECT` order: a collection's own objects first, then its children depth
//! first, starting at the master collection (verified on 5.2.4:
//! `['zz_master', 'Cube', 'Light', 'Camera', 'aa_coll', 'mm_child', 'dd_top2']`). Unlike the
//! view layer's bases, excluded collections are included.

use std::collections::HashSet;

use frap_rna::{CollRef, Field, IdRef, ObjectId, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::data::Main;
use crate::id::idlist::IdList;
use crate::id::paths::{internal, scene_of};
use crate::id::{Collection, Scene};

/// The objects of `scene` (see the module docs).
pub fn scene_objects(main: &Main, scene: &Scene) -> Vec<IdRef<ObjectId>> {
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
            if main.objects.contains(*o) && seen.insert(*o) {
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
    walk(main, &scene.collection, 0, &mut HashSet::new(), &mut out);
    out
}

#[rna_hook(srna = "Scene", prop = "objects", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let s = main
        .scenes
        .get(scene_of(this)?)
        .ok_or_else(|| internal("Scene.objects: the scene was removed"))?;
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(
        scene_objects(main, s),
    )))))
}
