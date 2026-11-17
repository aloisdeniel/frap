// SPDX-License-Identifier: GPL-3.0-or-later
//! §9.4 Traits implemented by data structs (`frap-rna/src/traits.rs`, CONTRACT).
//!
//! The contract items are kept verbatim. Amendments found by the prototypes:
//! * `Field::Embedded`  (F-TR-1): `material.node_tree` is an ID pointer with no `IdKey`, so it
//!   can be neither `Field::Id` nor a plain `Field::Struct` (the resolver must re-root).
//! * `Field::Ptr`       (F-TR-2): context members / derived pointers that point *into* another
//!   ID (`bpy.context.tool_settings` is `bpy.data.scenes['Scene'].tool_settings`, verified).
//! * `Field::NeedsHook` (F-TR-3): a `=> hook` getter (`Object.material_slots`) has no way to say
//!   "ask RNA_HOOKS" (`SetOutcome::NeedsHook` exists only for setters).
//! * `Item`             (F-TR-4): `RnaColl::item` returns `StructRef`, which cannot express an ID
//!   item (re-root) nor a `None` item (`mesh.materials` may contain `None`, verified).
//! * `RnaColl::item_seg` (F-TR-5): the canonical path segment of item `i` is per collection:
//!   `nodes["Principled BSDF"]` by name but `inputs[0]` / `material_slots[0]` by index even when
//!   looked up by name (verified), `vertices[3]` (nameless).
//! * blanket `impl RnaMut for &mut T` (F-TR-6): needed to box a reborrow of real memory as
//!   `RnaMutGuard`; otherwise every in-memory struct needs a wrapper type.
//! * `Bulk` for `foreach_get`/`foreach_set` (F-TR-7): `&[Value]` costs `size_of::<Value>()` bytes
//!   per scalar (see `tests::sizes`).

use core::ops::Range;

use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::{PropDef, StructDef};
use crate::error::{PyExc, RnaError, codes};
use crate::id::{EmbeddedSlot, IdAny};
use crate::ptr::{RnaPtr, Seg};
use crate::value::Value;

pub enum Field<'a> {
    Value(Value),
    Struct(StructRef<'a>),
    Id(Option<IdAny>),
    Coll(CollRef<'a>),
    // ---- RFC 0001 additions
    /// F-TR-1: embedded ID (re-root at `Root::Embedded { owner: <current ID>, slot }`).
    Embedded(EmbeddedSlot, StructRef<'a>),
    /// F-TR-2: re-root at an arbitrary canonical pointer (resolved again from its root).
    Ptr(RnaPtr),
    /// F-TR-3: the binding is `=> hook`; the resolver must call the registered get hook.
    NeedsHook,
}

pub enum StructRef<'a> {
    Borrowed(&'a dyn Rna),
    Owned(Box<dyn Rna + 'a>),
}
pub enum CollRef<'a> {
    Borrowed(&'a dyn RnaColl),
    Owned(Box<dyn RnaColl + 'a>),
}

impl<'a> core::ops::Deref for StructRef<'a> {
    type Target = dyn Rna + 'a;
    fn deref(&self) -> &Self::Target {
        match self {
            StructRef::Borrowed(r) => *r,
            StructRef::Owned(b) => &**b,
        }
    }
}
impl<'a> core::ops::Deref for CollRef<'a> {
    type Target = dyn RnaColl + 'a;
    fn deref(&self) -> &Self::Target {
        match self {
            CollRef::Borrowed(r) => *r,
            CollRef::Owned(b) => &**b,
        }
    }
}

/// F-TR-4: one collection item.
pub enum Item<'a> {
    Struct(StructRef<'a>),
    Id(Option<IdAny>),
}

pub type RnaMutGuard<'a> = Box<dyn RnaMut + 'a>;
pub type RnaCollMutGuard<'a> = Box<dyn RnaCollMut + 'a>;

pub trait Rna: Send + Sync {
    fn srna(&self) -> &'static StructDef;
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError>;
}

pub trait RnaMut: Rna {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError>;
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError>;
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError>;
}

/// Dense bulk buffer for `foreach_get`/`foreach_set` (F-TR-7). One allocation, 4 bytes/scalar.
#[derive(Clone, Debug, PartialEq)]
pub enum Bulk {
    F32(Vec<f32>),
    I32(Vec<i32>),
    Bool(Vec<bool>),
}

impl Bulk {
    pub fn len(&self) -> usize {
        match self {
            Bulk::F32(v) => v.len(),
            Bulk::I32(v) => v.len(),
            Bulk::Bool(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait RnaColl {
    fn item_type(&self) -> &'static StructDef;
    fn len(&self) -> usize;
    /// Not in §9.4: without it `clippy::len_without_is_empty` fails `-D warnings` (§20.5) on
    /// the contract trait itself (RFC 0001 F-TR-8).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn key_of(&self, i: usize) -> Option<SmolStr>;
    fn find(&self, key: &str) -> Option<usize>;
    /// Contract form; the probe resolver uses `item_any` (F-TR-4). Default for ID collections.
    fn item(&self, i: usize) -> Result<StructRef<'_>, RnaError>;
    /// Bulk access (§13.3). Contract default: loop over item() (here: unsupported, returns error).
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Value, RnaError> {
        let _ = (prop, range);
        Err(RnaError::new(PyExc::RuntimeError, codes::NOT_IMPLEMENTED, "foreach_get"))
    }
    // ---- RFC 0001 additions (all with defaults, so additive)
    /// F-TR-4: item that may be an ID or `None`.
    fn item_any(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.item(i).map(Item::Struct)
    }
    /// F-TR-5: canonical repr/path segment of item `i`.
    fn item_seg(&self, i: usize) -> Seg {
        match self.key_of(i) {
            Some(k) => Seg::Key(k),
            None => Seg::Index(i as i64),
        }
    }
    /// F-TR-7: dense form.
    fn foreach_get_bulk(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
        let _ = (prop, range);
        Err(RnaError::new(PyExc::RuntimeError, codes::NOT_IMPLEMENTED, "foreach_get_bulk"))
    }
}

pub trait RnaCollMut: RnaColl {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError>;
    fn foreach_set(&mut self, prop: &'static PropDef, flat: &[Value]) -> Result<(), RnaError> {
        let _ = (prop, flat);
        Err(RnaError::new(PyExc::RuntimeError, codes::NOT_IMPLEMENTED, "foreach_set"))
    }
    /// F-TR-7: dense form.
    fn foreach_set_bulk(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> {
        let _ = (prop, data);
        Err(RnaError::new(PyExc::RuntimeError, codes::NOT_IMPLEMENTED, "foreach_set_bulk"))
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SetOutcome {
    Done,
    NeedsHook,
}

// ---- F-TR-6: blanket impls so `Box::new(&mut in_memory_struct)` is an `RnaMutGuard`.
impl<T: Rna + ?Sized> Rna for &mut T {
    fn srna(&self) -> &'static StructDef {
        (**self).srna()
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        (**self).get(prop)
    }
}
impl<T: RnaMut + ?Sized> RnaMut for &mut T {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        (**self).set(prop, v)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        (**self).struct_mut(prop)
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        (**self).coll_mut(prop)
    }
}

// ---- helpers shared by hand-written bindings (what `rna_struct!` would generate)

pub fn no_attr(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        PyExc::AttributeError,
        codes::RNA_NO_ATTRIBUTE,
        format!("'{}' object has no attribute '{}'", srna.identifier, prop),
    )
}
pub fn readonly(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        PyExc::AttributeError,
        codes::RNA_READONLY,
        format!("bpy_struct: attribute \"{}\" from \"{}\" is read-only", prop, srna.identifier),
    )
}
pub fn unbound(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        PyExc::NotImplementedError,
        codes::NOT_IMPLEMENTED,
        format!("frap: {}.{} is not implemented", srna.identifier, prop),
    )
}
pub fn bad_value(prop: &str) -> RnaError {
    RnaError::new(
        PyExc::TypeError,
        codes::RNA_TYPE,
        format!("probe: unexpected value kind for {prop}"),
    )
}

pub fn f32_array<const N: usize>(v: &Value) -> Result<[f32; N], RnaError> {
    match v {
        Value::FloatArray { data, .. } if data.len() == N => {
            let mut a = [0.0; N];
            a.copy_from_slice(data);
            Ok(a)
        }
        _ => Err(bad_value("float array")),
    }
}

pub fn small_f32(v: &[f32]) -> SmallVec<[f32; 16]> {
    SmallVec::from_slice(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Numbers recorded in RFC 0001 (F-TR-7 / F-VAL-1).
    #[test]
    fn sizes() {
        let v = core::mem::size_of::<Value>();
        let f = core::mem::size_of::<Field<'static>>();
        let p = core::mem::size_of::<RnaPtr>();
        eprintln!(
            "size_of Value={v} Field={f} RnaPtr={p} PropDef={} SmallVec<[f32;16]>={} Box<RnaPtr>={}",
            core::mem::size_of::<PropDef>(),
            core::mem::size_of::<SmallVec<[f32; 16]>>(),
            core::mem::size_of::<Box<RnaPtr>>()
        );
        // A 1M-vertex `foreach_set("co", …)` through `&[Value]` needs 3M * size_of::<Value>() bytes.
        assert!(v >= 64, "Value is large: {v} bytes per scalar in foreach_set(&[Value])");
    }
}
