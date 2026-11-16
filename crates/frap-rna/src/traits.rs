// SPDX-License-Identifier: GPL-3.0-or-later
//! Traits implemented by data structs (ARCHITECTURE §9.4, CONTRACT frozen at M1 W0, amended by
//! RFC 0001 F-TR-1..8).
//!
//! Many RNA structs do not exist in memory (SoA element proxies, derived `MaterialSlot`,
//! computed `Context`), so the traits return **owned proxies** wherever a borrow is impossible.

use std::ops::Range;

use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::{ArrayShape, PropDef, StructDef};
use crate::error::{PyExc, RnaError, RnaErrorKind};
use crate::id::{EmbeddedSlot, IdAny};
use crate::ptr::{RnaPtr, Seg};
use crate::value::{MathTag, Value};

/// Result of reading a property.
#[non_exhaustive]
pub enum Field<'a> {
    /// A plain value.
    Value(Value),
    /// A nested struct (in memory or proxy).
    Struct(StructRef<'a>),
    /// An ID pointer (the resolver re-roots at `Root::Id`).
    Id(Option<IdAny>),
    /// A collection.
    Coll(CollRef<'a>),
    /// An embedded ID: re-root at `Root::Embedded { owner: <current ID>, slot }` (F-TR-1).
    Embedded(EmbeddedSlot, StructRef<'a>),
    /// Re-root at a canonical pointer, resolved again from its root (F-TR-2:
    /// `bpy.context.tool_settings` is `bpy.data.scenes['Scene'].tool_settings`).
    Ptr(RnaPtr),
    /// The binding is `=> hook` / `hook_coll`: the resolver calls the registered get hook
    /// (`RNA_HOOKS`, F-TR-3).
    NeedsHook,
}

/// A struct reachable from a property: borrowed real memory or an owned proxy
/// (e.g. `MeshVertexRef { mesh: &Mesh, i }`).
pub enum StructRef<'a> {
    /// In-memory struct.
    Borrowed(&'a dyn Rna),
    /// Proxy.
    Owned(Box<dyn Rna + 'a>),
}

/// A collection reachable from a property.
pub enum CollRef<'a> {
    /// In-memory collection.
    Borrowed(&'a dyn RnaColl),
    /// Proxy.
    Owned(Box<dyn RnaColl + 'a>),
}

impl<'a> std::ops::Deref for StructRef<'a> {
    type Target = dyn Rna + 'a;
    fn deref(&self) -> &Self::Target {
        match self {
            StructRef::Borrowed(r) => *r,
            StructRef::Owned(b) => &**b,
        }
    }
}

impl<'a> std::ops::Deref for CollRef<'a> {
    type Target = dyn RnaColl + 'a;
    fn deref(&self) -> &Self::Target {
        match self {
            CollRef::Borrowed(r) => *r,
            CollRef::Owned(b) => &**b,
        }
    }
}

/// One collection item: a struct, or an ID (possibly `None`: `mesh.materials.append(None)` is
/// legal) that the resolver re-roots at (F-TR-4).
pub enum Item<'a> {
    /// A struct item.
    Struct(StructRef<'a>),
    /// An ID item (`bpy.data.*`, `collection.objects`, `mesh.materials`, …).
    Id(Option<IdAny>),
}

/// Mutable access is always a guard: a reborrow of real memory (`Box::new(&mut node)`, through
/// the blanket impls below, F-TR-6) or a proxy such as `MeshVertexMut { mesh: &mut Mesh, i }`,
/// which writes through `Mesh`'s `&mut` methods (§6.1).
pub type RnaMutGuard<'a> = Box<dyn RnaMut + 'a>;
/// Mutable collection guard.
pub type RnaCollMutGuard<'a> = Box<dyn RnaCollMut + 'a>;

/// Read access to an RNA struct.
pub trait Rna: Send + Sync {
    /// Refined struct type (e.g. `BevelModifier`).
    fn srna(&self) -> &'static StructDef;
    /// Reads a property.
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError>;
}

/// Write access to an RNA struct. `v` is already coerced against `prop` (§9.2).
pub trait RnaMut: Rna {
    /// Writes a property.
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError>;
    /// Mutable access to a nested struct.
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError>;
    /// Mutable access to a collection.
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError>;
}

/// Dense bulk buffer for `foreach_get`/`foreach_set` (§13.3, F-TR-7): 4 bytes per scalar
/// instead of one `Value` each. Flat, row-major; the item length is `prop.array_len()`.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Bulk {
    /// FLOAT props.
    F32(Vec<f32>),
    /// INT props.
    I32(Vec<i32>),
    /// BOOLEAN props.
    Bool(Vec<bool>),
}

impl Bulk {
    /// Number of scalars.
    pub fn len(&self) -> usize {
        match self {
            Bulk::F32(v) => v.len(),
            Bulk::I32(v) => v.len(),
            Bulk::Bool(v) => v.len(),
        }
    }
    /// True when empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn push_value(&mut self, v: &Value) -> Result<(), RnaError> {
        match (self, v) {
            (Bulk::F32(b), Value::Float(f)) => b.push(*f),
            (Bulk::F32(b), Value::Double(f)) => b.push(*f as f32),
            (Bulk::F32(b), Value::FloatArray { data, .. }) => b.extend_from_slice(data),
            (Bulk::I32(b), Value::Int(i)) => b.push(*i as i32),
            (Bulk::I32(b), Value::IntArray(a)) => b.extend(a.iter().map(|i| *i as i32)),
            (Bulk::Bool(b), Value::Bool(x)) => b.push(*x),
            (Bulk::Bool(b), Value::BoolArray(a)) => b.extend_from_slice(a),
            _ => return Err(bulk_type_error()),
        }
        Ok(())
    }
    fn item_value(&self, prop: &'static PropDef, i: usize) -> Value {
        let n = prop.array_len();
        let scalar = matches!(prop.array, ArrayShape::Scalar);
        let r = i * n..i * n + n;
        match self {
            Bulk::F32(b) if scalar => Value::Float(b[r.start]),
            Bulk::F32(b) => Value::FloatArray {
                data: SmallVec::from_slice(&b[r]),
                shape: prop.array,
                tag: MathTag::for_subtype(prop.subtype),
            },
            Bulk::I32(b) if scalar => Value::Int(b[r.start] as i64),
            Bulk::I32(b) => Value::IntArray(b[r].iter().map(|x| *x as i64).collect()),
            Bulk::Bool(b) if scalar => Value::Bool(b[r.start]),
            Bulk::Bool(b) => Value::BoolArray(SmallVec::from_slice(&b[r])),
        }
    }
}

fn bulk_type_error() -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        "foreach_get/foreach_set: property type does not match the buffer type",
    )
}

/// Read access to an RNA collection.
pub trait RnaColl {
    /// Item struct type.
    fn item_type(&self) -> &'static StructDef;
    /// Number of items.
    fn len(&self) -> usize;
    /// True when empty (F-TR-8: required by `clippy::len_without_is_empty`).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Name of item `i`; `None` for nameless items (vertices, loops).
    fn key_of(&self, i: usize) -> Option<SmolStr>;
    /// Index of the item with this key (node sockets: first enabled match). ID-item
    /// collections answer `None` and are searched by the resolver through `IdNames` (F-TR-4).
    fn find(&self, key: &str) -> Option<usize>;
    /// Item `i`: a struct or an ID (F-TR-4).
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError>;
    /// Canonical repr/path segment of item `i` (F-TR-5). Default: `Key(name)` when the item
    /// has a name, else `Index(i)`. Sockets and material slots override it to `Index(i)`.
    fn item_seg(&self, i: usize) -> Seg {
        match self.key_of(i) {
            Some(k) => Seg::Key(k),
            None => Seg::Index(i as i64),
        }
    }
    /// Bulk read of `prop` over `range` (`foreach_get` / slices, §13.3). Default: loop over
    /// `item()`; SoA collections override it.
    fn foreach_get(&self, prop: &'static PropDef, range: Range<usize>) -> Result<Bulk, RnaError> {
        let mut out = match prop.type_str() {
            "FLOAT" => Bulk::F32(Vec::new()),
            "INT" => Bulk::I32(Vec::new()),
            "BOOLEAN" => Bulk::Bool(Vec::new()),
            _ => return Err(bulk_type_error()),
        };
        for i in range {
            let Item::Struct(s) = self.item(i)? else {
                return Err(bulk_type_error());
            };
            match s.get(prop)? {
                Field::Value(v) => out.push_value(&v)?,
                _ => return Err(bulk_type_error()),
            }
        }
        Ok(out)
    }
}

/// Write access to an RNA collection.
pub trait RnaCollMut: RnaColl {
    /// Mutable item `i`.
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError>;
    /// Bulk write of `prop` for every item (`foreach_set`). A wrong length raises Blender's
    /// `RuntimeError: internal error setting the array` (verified, F-TR-7). Default: loop over
    /// `item_mut()`; SoA collections override it.
    fn foreach_set(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> {
        let n = self.len();
        if data.len() != n * prop.array_len() {
            return Err(RnaError::new(
                RnaErrorKind::Other,
                PyExc::RuntimeError,
                "internal error setting the array",
            ));
        }
        for i in 0..n {
            let v = data.item_value(prop, i);
            match self.item_mut(i)?.set(prop, v)? {
                SetOutcome::Done => {}
                SetOutcome::NeedsHook => {
                    return Err(RnaError::new(
                        RnaErrorKind::NotImplemented,
                        PyExc::RuntimeError,
                        format!("foreach_set: {} needs a hook", prop.identifier),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Result of writing a property.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SetOutcome {
    /// Written.
    Done,
    /// The property needs a global set hook (§9.5).
    NeedsHook,
}

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

/// `'Object' object has no attribute 'foo'` (helper for hand-written bindings).
pub fn no_attribute(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::NoAttribute,
        PyExc::AttributeError,
        format!("'{}' object has no attribute '{}'", srna.identifier, prop),
    )
}

/// `bpy_struct: attribute "type" from "Object" is read-only`.
pub fn read_only(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::ReadOnly,
        PyExc::AttributeError,
        format!(
            "bpy_struct: attribute \"{}\" from \"{}\" is read-only",
            prop, srna.identifier
        ),
    )
}
