// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.instance_type` and `Object.instance_collection` (WP 8.3.4; ported from
//! blender/source/blender/makesrna/intern/rna_object.cc, `rna_Object_instance_type_itemf` and
//! `rna_Object_dup_collection_set`, and `BKE_collection_has_object_recursive_instanced`,
//! blenkernel/intern/collection.cc, Blender 5.2.4, GPL-2.0-or-later). Verified on 5.2.4:
//!
//! - the items of `instance_type` follow the object type: `NONE`, `COLLECTION` for an empty,
//!   `NONE`, `VERTS`, `FACES` for every other type (frap has no text objects, whose items are
//!   `NONE`, `VERTS`); another value is `TypeError: bpy_struct: item.attr = val: enum "FACES"
//!   not found in ('NONE', 'COLLECTION')`;
//! - `instance_collection` takes a collection only on an empty and only when the empty is not
//!   in it (directly, in a child collection, or through a collection instanced inside it);
//!   otherwise Blender logs `Only empty objects support collection instances` or `Cannot set
//!   instance-collection as object belongs (directly or indirectly) in collection being
//!   instanced, thus causing a dependency cycle` and nothing changes (no exception);
//! - `None` clears it; `instance_type` is independent (an empty may keep `COLLECTION` without
//!   a collection, and `instance_type = 'NONE'` keeps the collection).

use frap_rna::{
    CollectionId, Field, IdRef, ObjectId, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value,
    rna_hook,
};

use crate::data::IdMap;
use crate::id::paths::{internal, object_of};
use crate::id::{Collection, Object};
use crate::{Host, HostRead};

/// `rna_Object_instance_type_itemf` for an object of `object_type`.
pub fn instance_type_items(object_type: &str) -> &'static [&'static str] {
    if object_type == "EMPTY" {
        &["NONE", "COLLECTION"]
    } else {
        &["NONE", "VERTS", "FACES"]
    }
}

/// `BKE_collection_has_object_recursive_instanced(collection, ob)`: `ob` is in `collection`,
/// in one of its child collections, or in a collection instanced by an object found there
/// (`collection_object_cache_fill` with instances).
pub fn has_object_recursive_instanced(
    collections: &IdMap<Collection>,
    objects: &IdMap<Object>,
    collection: IdRef<CollectionId>,
    ob: IdRef<ObjectId>,
) -> bool {
    fn go(
        collections: &IdMap<Collection>,
        objects: &IdMap<Object>,
        c: IdRef<CollectionId>,
        ob: IdRef<ObjectId>,
        seen_obs: &mut Vec<IdRef<ObjectId>>,
        depth: usize,
    ) -> bool {
        let Some(col) = collections.get(c) else {
            return false;
        };
        if depth > 256 {
            return false;
        }
        for &o in &col.objects {
            if o == ob {
                return true;
            }
            if seen_obs.contains(&o) {
                continue;
            }
            seen_obs.push(o);
            if let Some(ic) = objects.get(o).and_then(|x| x.instance_collection)
                && go(collections, objects, ic, ob, seen_obs, depth + 1)
            {
                return true;
            }
        }
        col.children
            .iter()
            .any(|&ch| go(collections, objects, ch, ob, seen_obs, depth + 1))
    }
    go(collections, objects, collection, ob, &mut Vec::new(), 0)
}

#[rna_hook(srna = "Object", prop = "instance_type", krate = crate)]
mod instance_type {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let Some(value) = v.as_str() else {
            return Ok(());
        };
        let (main, log) = h.main_mut();
        let Some(o) = main.objects.get(ob) else {
            return Ok(());
        };
        let items = instance_type_items(o.object_type());
        if !items.contains(&value) {
            let listed = items
                .iter()
                .map(|n| format!("'{n}'"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RnaError::new(
                RnaErrorKind::Enum,
                PyExc::TypeError,
                format!("bpy_struct: item.attr = val: enum \"{value}\" not found in ({listed})"),
            )
            .with_detail(
                "valid",
                items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            ));
        }
        if o.instance_type == value {
            return Ok(());
        }
        let value = value.to_owned();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.instance_type = value.into();
        }
        Ok(())
    }
}

#[rna_hook(srna = "Object", prop = "instance_collection", krate = crate)]
mod instance_collection {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let ob = object_of(this)?;
        let o = h
            .main()
            .objects
            .get(ob)
            .ok_or_else(|| internal("Object.instance_collection: the object was removed"))?;
        Ok(Field::Id(o.instance_collection.map(IdRef::any)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let target: Option<IdRef<CollectionId>> = match v {
            Value::Id(Some(a)) => Some(IdRef::from_any(a).ok_or_else(|| {
                RnaError::new(
                    RnaErrorKind::Type,
                    PyExc::TypeError,
                    "bpy_struct: item.attr = val: Object.instance_collection expected a Collection type",
                )
            })?),
            Value::Id(None) | Value::None => None,
            _ => {
                return Err(RnaError::new(
                    RnaErrorKind::NotImplemented,
                    PyExc::NotImplementedError,
                    "frap: Object.instance_collection takes a bpy.data.collections item (the scene's master collection cannot be instanced in frap)",
                ));
            }
        };
        let (main, log) = h.main_mut();
        let Some(o) = main.objects.get(ob) else {
            return Ok(());
        };
        // `rna_Object_dup_collection_set`: the cycle test first, then the type test; both only
        // log an error.
        if let Some(c) = target
            && (has_object_recursive_instanced(&main.collections, &main.objects, c, ob)
                || o.object_type() != "EMPTY")
        {
            return Ok(());
        }
        if o.instance_collection == target {
            return Ok(());
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.instance_collection = target;
        }
        Ok(())
    }
}
