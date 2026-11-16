// SPDX-License-Identifier: GPL-3.0-or-later
//! A computed list of IDs as an RNA collection (`bpy.context.selected_objects`,
//! `view_layer.objects`, `scene.objects`, `Object.children`, …). Items are `Item::Id`; the
//! resolver names them through `IdNames` (F-TR-4), so `key_of`/`find` answer `None`.

use frap_rna::traits::index_out_of_range;
use frap_rna::{IdAny, IdKind, IdRef, Item, ObjectId, RnaColl, RnaError, StructDef};
use smol_str::SmolStr;

/// An owned list of IDs.
#[derive(Clone, Debug)]
pub struct IdList {
    /// The IDs, in collection order.
    pub ids: Vec<IdAny>,
    /// Item struct (`Object`, `Collection`, `Scene`, …).
    pub item_type: &'static StructDef,
}

impl IdList {
    /// A list of typed IDs.
    pub fn new<K: IdKind>(ids: Vec<IdRef<K>>, item_type: &'static StructDef) -> IdList {
        IdList {
            ids: ids.into_iter().map(IdRef::any).collect(),
            item_type,
        }
    }
    /// A list of objects.
    pub fn objects(ids: Vec<IdRef<ObjectId>>) -> IdList {
        IdList::new(ids, &frap_spec_rna::OBJECT)
    }
    /// An empty list of `item_type`.
    pub fn empty(item_type: &'static StructDef) -> IdList {
        IdList {
            ids: Vec::new(),
            item_type,
        }
    }
}

impl RnaColl for IdList {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.ids
            .get(i)
            .map(|a| Item::Id(Some(*a)))
            .ok_or_else(|| index_out_of_range(i, self.ids.len()))
    }
}
