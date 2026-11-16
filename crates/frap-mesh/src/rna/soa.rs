// SPDX-License-Identifier: GPL-3.0-or-later
//! Generic struct-of-arrays proxies (ARCHITECTURE §9.4).
//!
//! `MeshVertex`, `MeshEdge`, attribute `data[i]` and the other element structs do not exist in
//! memory: an item is `(mesh, context, index)`. One [`SoaKind`] per element struct says how to
//! read and write item `i` of a column set; the four generic proxies below turn that into the
//! frozen `Rna`/`RnaMut`/`RnaColl`/`RnaCollMut` traits:
//!
//! | proxy | holds | implements |
//! |---|---|---|
//! | [`ItemRef`] | `&Mesh`, ctx, `i` | `Rna` |
//! | [`ItemMut`] | `&mut Mesh`, ctx, `i` | `Rna`, `RnaMut` |
//! | [`Coll`] | `&Mesh`, ctx | `RnaColl` |
//! | [`CollMut`] | `&mut Mesh`, ctx | `RnaColl`, `RnaCollMut` |
//!
//! The context is what a nested collection needs on top of the mesh: `()` for
//! `mesh.vertices`, the attribute's position for `attributes[n].data`, the vertex for
//! `vertices[v].groups`. Every write goes through `Mesh`'s `&mut` methods, which clear the
//! runtime caches (§6.1).

use std::marker::PhantomData;
use std::ops::Range;

use frap_rna::traits::{SmolStr, index_out_of_range, no_attribute, wrong_access};
use frap_rna::{
    Bulk, Field, Item, PropDef, PyExc, Rna, RnaColl, RnaCollMut, RnaCollMutGuard, RnaError,
    RnaErrorKind, RnaMut, RnaMutGuard, SetOutcome, StructDef, StructRef, Value,
};

use super::bulk;
use crate::mesh::Mesh;

/// One element struct over struct-of-arrays columns.
pub(crate) trait SoaKind: 'static {
    /// What the collection needs besides the mesh (see the module docs).
    type Ctx: Copy + Send + Sync + 'static;

    /// Item struct of the collection (`RnaColl::item_type`; attribute values: the struct of
    /// the attribute's data type, so `data.foreach_get("vector")` finds its property).
    fn item_type(m: &Mesh, ctx: Self::Ctx) -> &'static StructDef;
    /// Refined struct of item `i` (attributes: `FloatAttribute`, …). Default: the item type.
    fn item_srna(m: &Mesh, ctx: Self::Ctx, i: usize) -> &'static StructDef {
        let _ = i;
        Self::item_type(m, ctx)
    }
    /// Number of items.
    fn len(m: &Mesh, ctx: Self::Ctx) -> usize;
    /// Name of item `i` (named layers); `None` for nameless elements.
    fn key_of(m: &Mesh, ctx: Self::Ctx, i: usize) -> Option<SmolStr> {
        let _ = (m, ctx, i);
        None
    }
    /// Index of the item named `key`.
    fn find(m: &Mesh, ctx: Self::Ctx, key: &str) -> Option<usize> {
        (0..Self::len(m, ctx)).find(|&i| Self::key_of(m, ctx, i).as_deref() == Some(key))
    }
    /// Reads `prop` of item `i` (`i < len`).
    fn get<'a>(
        m: &'a Mesh,
        ctx: Self::Ctx,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError>;
    /// Writes `prop` of item `i` (`v` is already coerced against `prop`).
    fn set(
        m: &mut Mesh,
        ctx: Self::Ctx,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError>;
    /// Mutable nested collection of item `i` (`attributes[n].data`).
    fn coll_mut<'a>(
        m: &'a mut Mesh,
        ctx: Self::Ctx,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<RnaCollMutGuard<'a>, RnaError> {
        Err(wrong_access(
            Self::item_srna(m, ctx, i),
            prop.identifier,
            "a collection",
        ))
    }
    /// Bulk read (`foreach_get`). Default: item by item through [`SoaKind::get`].
    fn foreach_get(
        m: &Mesh,
        ctx: Self::Ctx,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::generic_get::<Self>(m, ctx, prop, range)
    }
    /// Bulk write (`foreach_set`). Default: item by item through [`SoaKind::set`].
    fn foreach_set(
        m: &mut Mesh,
        ctx: Self::Ctx,
        prop: &'static PropDef,
        data: &Bulk,
    ) -> Result<(), RnaError> {
        bulk::generic_set::<Self>(m, ctx, prop, data)
    }
}

/// Error for a property item `i` does not bind: `RNA_NOT_IMPLEMENTED` when the item's struct
/// defines it in Blender 5.2 (see [`unbound`]), else `'MeshVertex' object has no attribute 'foo'`.
pub(crate) fn no_attr<K: SoaKind>(m: &Mesh, ctx: K::Ctx, i: usize, prop: &PropDef) -> RnaError {
    unbound(K::item_srna(m, ctx, i), prop.identifier)
}

/// Error for a property frap does not bind on `srna`. A property that exists in Blender's
/// `StructDef` gives `NotImplementedError` (`RNA_NOT_IMPLEMENTED`, §13.4), so an agent is never
/// told that a real Blender property does not exist; any other name gives Blender's
/// `AttributeError`.
pub(crate) fn unbound(srna: &'static StructDef, prop: &str) -> RnaError {
    if srna.find_prop(prop).is_some() {
        RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::NotImplementedError,
            format!(
                "frap: {}.{prop} exists in Blender but is not implemented by frap yet",
                srna.identifier
            ),
        )
    } else {
        no_attribute(srna, prop)
    }
}

/// `bpy_struct: attribute "index" from "MeshVertex" is read-only` for item `i`.
pub(crate) fn read_only<K: SoaKind>(m: &Mesh, ctx: K::Ctx, i: usize, prop: &PropDef) -> RnaError {
    frap_rna::traits::read_only(K::item_srna(m, ctx, i), prop.identifier)
}

/// Read proxy of one item.
pub(crate) struct ItemRef<'a, K: SoaKind> {
    pub(crate) m: &'a Mesh,
    pub(crate) ctx: K::Ctx,
    pub(crate) i: usize,
    _k: PhantomData<fn() -> K>,
}

impl<'a, K: SoaKind> ItemRef<'a, K> {
    pub(crate) fn new(m: &'a Mesh, ctx: K::Ctx, i: usize) -> Self {
        ItemRef {
            m,
            ctx,
            i,
            _k: PhantomData,
        }
    }
}

impl<K: SoaKind> Rna for ItemRef<'_, K> {
    fn srna(&self) -> &'static StructDef {
        K::item_srna(self.m, self.ctx, self.i)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        K::get(self.m, self.ctx, self.i, prop)
    }
}

/// Write proxy of one item: writes through `Mesh`'s `&mut` methods.
pub(crate) struct ItemMut<'a, K: SoaKind> {
    pub(crate) m: &'a mut Mesh,
    pub(crate) ctx: K::Ctx,
    pub(crate) i: usize,
    _k: PhantomData<fn() -> K>,
}

impl<'a, K: SoaKind> ItemMut<'a, K> {
    pub(crate) fn new(m: &'a mut Mesh, ctx: K::Ctx, i: usize) -> Self {
        ItemMut {
            m,
            ctx,
            i,
            _k: PhantomData,
        }
    }
}

impl<K: SoaKind> Rna for ItemMut<'_, K> {
    fn srna(&self) -> &'static StructDef {
        K::item_srna(self.m, self.ctx, self.i)
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        K::get(self.m, self.ctx, self.i, prop)
    }
}

impl<K: SoaKind> RnaMut for ItemMut<'_, K> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        K::set(self.m, self.ctx, self.i, prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(wrong_access(
            K::item_srna(self.m, self.ctx, self.i),
            prop.identifier,
            "a struct",
        ))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        K::coll_mut(self.m, self.ctx, self.i, prop)
    }
}

/// Read view of a collection.
pub(crate) struct Coll<'a, K: SoaKind> {
    pub(crate) m: &'a Mesh,
    pub(crate) ctx: K::Ctx,
    _k: PhantomData<fn() -> K>,
}

impl<'a, K: SoaKind> Coll<'a, K> {
    pub(crate) fn new(m: &'a Mesh, ctx: K::Ctx) -> Self {
        Coll {
            m,
            ctx,
            _k: PhantomData,
        }
    }
}

/// `Field::Coll` of an owned [`Coll`].
pub(crate) fn coll_field<K: SoaKind>(m: &Mesh, ctx: K::Ctx) -> Field<'_> {
    Field::Coll(frap_rna::CollRef::Owned(Box::new(Coll::<K>::new(m, ctx))))
}

/// A boxed [`CollMut`].
pub(crate) fn coll_mut_guard<K: SoaKind>(m: &mut Mesh, ctx: K::Ctx) -> RnaCollMutGuard<'_> {
    Box::new(CollMut::<K>::new(m, ctx))
}

fn item<'a, K: SoaKind>(m: &'a Mesh, ctx: K::Ctx, i: usize) -> Result<Item<'a>, RnaError> {
    let n = K::len(m, ctx);
    if i >= n {
        return Err(index_out_of_range(i, n));
    }
    Ok(Item::Struct(StructRef::Owned(Box::new(ItemRef::<K>::new(
        m, ctx, i,
    )))))
}

impl<K: SoaKind> RnaColl for Coll<'_, K> {
    fn item_type(&self) -> &'static StructDef {
        K::item_type(self.m, self.ctx)
    }
    fn len(&self) -> usize {
        K::len(self.m, self.ctx)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        K::key_of(self.m, self.ctx, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        K::find(self.m, self.ctx, key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        item::<K>(self.m, self.ctx, i)
    }
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
        K::foreach_get(self.m, self.ctx, prop, range)
    }
}

/// Mutable view of a collection.
pub(crate) struct CollMut<'a, K: SoaKind> {
    pub(crate) m: &'a mut Mesh,
    pub(crate) ctx: K::Ctx,
    _k: PhantomData<fn() -> K>,
}

impl<'a, K: SoaKind> CollMut<'a, K> {
    pub(crate) fn new(m: &'a mut Mesh, ctx: K::Ctx) -> Self {
        CollMut {
            m,
            ctx,
            _k: PhantomData,
        }
    }
}

impl<K: SoaKind> RnaColl for CollMut<'_, K> {
    fn item_type(&self) -> &'static StructDef {
        K::item_type(self.m, self.ctx)
    }
    fn len(&self) -> usize {
        K::len(self.m, self.ctx)
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        K::key_of(self.m, self.ctx, i)
    }
    fn find(&self, key: &str) -> Option<usize> {
        K::find(self.m, self.ctx, key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        item::<K>(self.m, self.ctx, i)
    }
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
        K::foreach_get(self.m, self.ctx, prop, range)
    }
}

impl<K: SoaKind> RnaCollMut for CollMut<'_, K> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let n = K::len(self.m, self.ctx);
        if i >= n {
            return Err(index_out_of_range(i, n));
        }
        Ok(Box::new(ItemMut::<K>::new(self.m, self.ctx, i)))
    }
    fn foreach_set(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> {
        K::foreach_set(self.m, self.ctx, prop, data)
    }
}
