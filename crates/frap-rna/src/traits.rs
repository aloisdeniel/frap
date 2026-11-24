// SPDX-License-Identifier: GPL-3.0-or-later
//! Traits implemented by data structs (ARCHITECTURE §9.4, CONTRACT frozen at M1 W0, amended by
//! RFC 0001 F-TR-1..8).
//!
//! Many RNA structs do not exist in memory (SoA element proxies, derived `MaterialSlot`,
//! computed `Context`), so the traits return **owned proxies** wherever a borrow is impossible.

use std::collections::BTreeSet;
use std::ops::Range;

use frap_math::EulerOrder;
use frap_math::glam::{Mat3, Mat4};
use smallvec::SmallVec;
/// Re-exported so `rna_struct!` expansions can name it.
pub use smol_str::SmolStr;

use crate::def::{ArrayShape, FloatStorage, PropDef, PropFlags, PropKind, StructDef};
use crate::error::{PyExc, RnaError, RnaErrorKind};
use crate::id::{EmbeddedSlot, IdAny, IdKind, IdRef};
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
/// instead of one `Value` each. Flat, in Blender's raw memory order (a `MATRIX` item is
/// column-major, [`PropDef::reorder_raw`]); the item length is `prop.array_len()`.
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
    /// Appends one item's value of `prop`, a matrix in Blender's raw (column-major) order
    /// ([`PropDef::reorder_raw`]).
    pub fn push_value(&mut self, prop: &PropDef, v: &Value) -> Result<(), RnaError> {
        match (self, v) {
            (Bulk::F32(b), Value::Float(f)) => b.push(*f),
            (Bulk::F32(b), Value::Double(f)) => b.push(*f as f32),
            (Bulk::F32(b), Value::FloatArray { data, .. }) => {
                let at = b.len();
                b.extend_from_slice(data);
                prop.reorder_raw(&mut b[at..], true);
            }
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
            Bulk::F32(b) => {
                let mut data = SmallVec::from_slice(&b[r]);
                prop.reorder_raw(&mut data, false);
                Value::FloatArray {
                    data,
                    shape: prop.array,
                    tag: MathTag::for_subtype(prop.subtype),
                }
            }
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
                Field::Value(v) => out.push_value(prop, &v)?,
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

/// `bpy_prop_collection[index]: index 9 out of range, size 3`.
pub fn index_out_of_range(i: usize, len: usize) -> RnaError {
    RnaError::new(
        RnaErrorKind::IndexRange,
        PyExc::IndexError,
        format!("bpy_prop_collection[index]: index {i} out of range, size {len}"),
    )
}

/// Error for a binding asked for a kind of access it does not provide (`struct_mut` on a
/// value property, …). The resolver never asks for those; this is an internal error.
pub fn wrong_access(srna: &'static StructDef, prop: &str, what: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::TypeError,
        format!("{}.{prop} is not {what}", srna.identifier),
    )
}

/// Mutable access to a collection that is served by a hook (`hook_coll`).
pub fn coll_needs_hook(srna: &'static StructDef, prop: &str) -> RnaError {
    RnaError::new(
        RnaErrorKind::NotImplemented,
        PyExc::RuntimeError,
        format!(
            "{}.{prop} is provided by an RNA hook, not by the struct binding",
            srna.identifier
        ),
    )
}

/// A struct type with one static RNA definition (implemented by `rna_struct!`).
pub trait RnaStruct {
    /// The `StructDef` this type is bound to.
    fn struct_def() -> &'static StructDef;
}

fn internal_value(prop: &PropDef, expected: &str, got: &Value) -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        format!(
            "frap internal: {} expects {expected}, got {got:?}",
            prop.identifier
        ),
    )
}

fn kind_name(prop: &PropDef) -> String {
    let shape = match prop.array {
        ArrayShape::Scalar => String::new(),
        ArrayShape::Fixed(n) => format!("[{n}]"),
        ArrayShape::Matrix(r, c) => format!("[{r}][{c}]"),
        ArrayShape::Dynamic => "[]".into(),
    };
    format!("{}{shape}", prop.type_str())
}

fn expect_kind(prop: &PropDef, ok: bool, rust: &str) -> Result<(), String> {
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{} is {} but is bound to a Rust {rust}",
            prop.identifier,
            kind_name(prop)
        ))
    }
}

fn is_scalar(prop: &PropDef) -> bool {
    matches!(prop.array, ArrayShape::Scalar)
}

/// Conversion of a bound Rust field to an RNA [`Value`] (used by `rna_struct!` getters).
pub trait ToRnaValue {
    /// Converts. `tag` is the math wrapper of float arrays (ignored otherwise).
    fn to_rna_value(&self, prop: &PropDef, tag: MathTag) -> Value;
    /// Checks that this Rust type can represent `prop` (type, array shape, enum items), for
    /// [`crate::testing::check_bindings`].
    fn check_prop(prop: &PropDef) -> Result<(), String>
    where
        Self: Sized;
}

/// Conversion of an already coerced [`Value`] back to a bound Rust field (`rna_struct!`
/// setters). Errors are internal: `coerce` has validated the value against the `PropDef`.
pub trait FromRnaValue: Sized {
    /// Converts.
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError>;
}

impl ToRnaValue for bool {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::Bool(*self)
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        expect_kind(
            prop,
            matches!(prop.kind, PropKind::Bool { .. }) && is_scalar(prop),
            "bool",
        )
    }
}

impl FromRnaValue for bool {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::Bool(b) => Ok(b),
            Value::Int(i @ (0 | 1)) => Ok(i == 1),
            other => Err(internal_value(prop, "a bool", &other)),
        }
    }
}

macro_rules! int_value {
    ($($t:ty),*) => {$(
        impl ToRnaValue for $t {
            fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
                Value::Int(*self as i64)
            }
            fn check_prop(prop: &PropDef) -> Result<(), String> {
                expect_kind(
                    prop,
                    matches!(prop.kind, PropKind::Int { .. }) && is_scalar(prop),
                    stringify!($t),
                )
            }
        }
        impl FromRnaValue for $t {
            fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
                match v {
                    Value::Int(i) => <$t>::try_from(i)
                        .map_err(|_| internal_value(prop, stringify!($t), &Value::Int(i))),
                    Value::Bool(b) => Ok(<$t>::from(b)),
                    other => Err(internal_value(prop, "an int", &other)),
                }
            }
        }
    )*};
}
int_value!(i8, i16, i32, i64, u8, u16, u32, u64, usize);

impl ToRnaValue for f32 {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        match prop.kind {
            PropKind::Float {
                storage: FloatStorage::F64,
                ..
            } => Value::Double(f64::from(*self)),
            _ => Value::Float(*self),
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        expect_kind(
            prop,
            matches!(prop.kind, PropKind::Float { .. }) && is_scalar(prop),
            "f32",
        )
    }
}

impl ToRnaValue for f64 {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        match prop.kind {
            PropKind::Float {
                storage: FloatStorage::F64,
                ..
            } => Value::Double(*self),
            _ => Value::Float(*self as f32),
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        expect_kind(
            prop,
            matches!(prop.kind, PropKind::Float { .. }) && is_scalar(prop),
            "f64",
        )
    }
}

impl FromRnaValue for f32 {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::Float(f) => Ok(f),
            Value::Double(d) => Ok(d as f32),
            Value::Int(i) => Ok(i as f32),
            other => Err(internal_value(prop, "a float", &other)),
        }
    }
}

impl FromRnaValue for f64 {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::Float(f) => Ok(f64::from(f)),
            Value::Double(d) => Ok(d),
            Value::Int(i) => Ok(i as f64),
            other => Err(internal_value(prop, "a float", &other)),
        }
    }
}

fn string_value(prop: &PropDef, s: &str) -> Value {
    match prop.kind {
        PropKind::Enum { .. } => Value::Enum(s.into()),
        _ => Value::Str(s.into()),
    }
}

fn check_string(prop: &PropDef, rust: &str) -> Result<(), String> {
    expect_kind(
        prop,
        is_scalar(prop)
            && (matches!(prop.kind, PropKind::String { .. })
                || (matches!(prop.kind, PropKind::Enum { .. })
                    && !prop.flags.contains(PropFlags::ENUM_FLAG))),
        rust,
    )
}

fn string_from(prop: &PropDef, v: Value) -> Result<SmolStr, RnaError> {
    match v {
        Value::Str(s) | Value::Enum(s) => Ok(s),
        other => Err(internal_value(prop, "a string", &other)),
    }
}

impl ToRnaValue for str {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        string_value(prop, self)
    }
}

impl ToRnaValue for SmolStr {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        string_value(prop, self)
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_string(prop, "SmolStr")
    }
}

impl FromRnaValue for SmolStr {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        string_from(prop, v)
    }
}

impl ToRnaValue for String {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        string_value(prop, self)
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_string(prop, "String")
    }
}

impl FromRnaValue for String {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        string_from(prop, v).map(|s| s.to_string())
    }
}

impl ToRnaValue for &'static str {
    fn to_rna_value(&self, prop: &PropDef, _: MathTag) -> Value {
        string_value(prop, self)
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_string(prop, "&str")
    }
}

impl ToRnaValue for BTreeSet<SmolStr> {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::EnumSet(self.clone())
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        expect_kind(
            prop,
            matches!(prop.kind, PropKind::Enum { .. }) && prop.flags.contains(PropFlags::ENUM_FLAG),
            "BTreeSet<SmolStr> (enum flag)",
        )
    }
}

impl FromRnaValue for BTreeSet<SmolStr> {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::EnumSet(s) => Ok(s),
            other => Err(internal_value(prop, "an enum set", &other)),
        }
    }
}

impl ToRnaValue for EulerOrder {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::Enum(self.as_str().into())
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_string(prop, "EulerOrder")?;
        if let PropKind::Enum { items, .. } = prop.kind {
            for o in EulerOrder::ALL {
                if !items.iter().any(|i| i.identifier == o.as_str()) {
                    return Err(format!(
                        "{}: enum item '{}' of EulerOrder is not in the RNA items",
                        prop.identifier,
                        o.as_str()
                    ));
                }
            }
        }
        Ok(())
    }
}

impl FromRnaValue for EulerOrder {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        let s = string_from(prop, v)?;
        EulerOrder::from_blender(&s)
            .ok_or_else(|| internal_value(prop, "an Euler order", &Value::Enum(s)))
    }
}

fn check_array(
    prop: &PropDef,
    float: bool,
    int: bool,
    n: Option<usize>,
    rust: &str,
) -> Result<(), String> {
    let kind_ok = match prop.kind {
        PropKind::Float { .. } => float,
        PropKind::Int { .. } => int,
        PropKind::Bool { .. } => !float && !int,
        _ => false,
    };
    let shape_ok = match (n, prop.array) {
        (_, ArrayShape::Scalar) => false,
        (None, _) => true,
        (Some(n), ArrayShape::Dynamic) => n > 0,
        (Some(n), _) => prop.array_len() == n,
    };
    expect_kind(prop, kind_ok && shape_ok, rust)
}

impl<const N: usize> ToRnaValue for [f32; N] {
    fn to_rna_value(&self, prop: &PropDef, tag: MathTag) -> Value {
        Value::FloatArray {
            data: SmallVec::from_slice(self),
            shape: prop.array,
            tag,
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, true, false, Some(N), &format!("[f32; {N}]"))
    }
}

impl<const N: usize> FromRnaValue for [f32; N] {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::FloatArray { data, .. } if data.len() == N => {
                let mut out = [0.0; N];
                out.copy_from_slice(data);
                Ok(out)
            }
            _ => Err(internal_value(prop, &format!("float[{N}]"), &v)),
        }
    }
}

impl<const N: usize> ToRnaValue for [i32; N] {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::IntArray(self.iter().map(|i| i64::from(*i)).collect())
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, false, true, Some(N), &format!("[i32; {N}]"))
    }
}

impl<const N: usize> FromRnaValue for [i32; N] {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::IntArray(data) if data.len() == N => {
                let mut out = [0; N];
                for (o, d) in out.iter_mut().zip(data) {
                    *o = i32::try_from(*d).map_err(|_| internal_value(prop, "int32", &v))?;
                }
                Ok(out)
            }
            _ => Err(internal_value(prop, &format!("int[{N}]"), &v)),
        }
    }
}

impl<const N: usize> ToRnaValue for [bool; N] {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::BoolArray(SmallVec::from_slice(self))
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, false, false, Some(N), &format!("[bool; {N}]"))
    }
}

impl<const N: usize> FromRnaValue for [bool; N] {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::BoolArray(data) if data.len() == N => {
                let mut out = [false; N];
                out.copy_from_slice(data);
                Ok(out)
            }
            _ => Err(internal_value(prop, &format!("bool[{N}]"), &v)),
        }
    }
}

impl ToRnaValue for Vec<f32> {
    fn to_rna_value(&self, prop: &PropDef, tag: MathTag) -> Value {
        Value::FloatArray {
            data: SmallVec::from_slice(self),
            shape: prop.array,
            tag,
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, true, false, None, "Vec<f32>")
    }
}

impl FromRnaValue for Vec<f32> {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::FloatArray { data, .. } => Ok(data.to_vec()),
            other => Err(internal_value(prop, "a float array", &other)),
        }
    }
}

impl ToRnaValue for Vec<i32> {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::IntArray(self.iter().map(|i| i64::from(*i)).collect())
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, false, true, None, "Vec<i32>")
    }
}

impl FromRnaValue for Vec<i32> {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::IntArray(data) => data
                .iter()
                .map(|d| i32::try_from(*d).map_err(|_| internal_value(prop, "int32", &v)))
                .collect(),
            other => Err(internal_value(prop, "an int array", other)),
        }
    }
}

impl ToRnaValue for Vec<bool> {
    fn to_rna_value(&self, _: &PropDef, _: MathTag) -> Value {
        Value::BoolArray(SmallVec::from_slice(self))
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, false, false, None, "Vec<bool>")
    }
}

impl FromRnaValue for Vec<bool> {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match v {
            Value::BoolArray(data) => Ok(data.to_vec()),
            other => Err(internal_value(prop, "a bool array", &other)),
        }
    }
}

/// `glam::Mat4` (column-major, like Blender's `float m[4][4]`) ↔ row-major `FloatArray`.
impl ToRnaValue for Mat4 {
    fn to_rna_value(&self, prop: &PropDef, tag: MathTag) -> Value {
        Value::FloatArray {
            data: SmallVec::from_slice(&self.transpose().to_cols_array()),
            shape: prop.array,
            tag,
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, true, false, Some(16), "Mat4")
    }
}

impl FromRnaValue for Mat4 {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::FloatArray { data, .. } if data.len() == 16 => {
                Ok(Mat4::from_cols_slice(data).transpose())
            }
            _ => Err(internal_value(prop, "float[16]", &v)),
        }
    }
}

/// `glam::Mat3` ↔ row-major `FloatArray`.
impl ToRnaValue for Mat3 {
    fn to_rna_value(&self, prop: &PropDef, tag: MathTag) -> Value {
        Value::FloatArray {
            data: SmallVec::from_slice(&self.transpose().to_cols_array()),
            shape: prop.array,
            tag,
        }
    }
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        check_array(prop, true, false, Some(9), "Mat3")
    }
}

impl FromRnaValue for Mat3 {
    fn from_rna_value(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match &v {
            Value::FloatArray { data, .. } if data.len() == 9 => {
                Ok(Mat3::from_cols_slice(data).transpose())
            }
            _ => Err(internal_value(prop, "float[9]", &v)),
        }
    }
}

/// The rotation order an Euler tag reads from the owner (`tag = Euler(rotation_mode)`):
/// an [`EulerOrder`] or Blender's `rotation_mode` string (`'QUATERNION'`/`'AXIS_ANGLE'` and
/// unknown strings give `XYZ`, which is how Blender wraps `rotation_euler` then).
pub trait AsEulerOrder {
    /// The order.
    fn euler_order(&self) -> EulerOrder;
}

impl AsEulerOrder for EulerOrder {
    fn euler_order(&self) -> EulerOrder {
        *self
    }
}
impl AsEulerOrder for str {
    fn euler_order(&self) -> EulerOrder {
        EulerOrder::from_blender(self).unwrap_or_default()
    }
}
impl AsEulerOrder for SmolStr {
    fn euler_order(&self) -> EulerOrder {
        self.as_str().euler_order()
    }
}
impl AsEulerOrder for String {
    fn euler_order(&self) -> EulerOrder {
        self.as_str().euler_order()
    }
}

/// A field holding an ID pointer (`id(field)` bindings, `id_coll` items).
pub trait IdField: Sized {
    /// The referenced ID.
    fn get_id(&self) -> Option<IdAny>;
    /// Builds the field from a coerced `Value::Id` (or `Value::None`).
    fn set_id(prop: &PropDef, v: Value) -> Result<Self, RnaError>;
    /// Checks the `PropDef` is an ID pointer.
    fn check_prop(prop: &PropDef) -> Result<(), String> {
        expect_kind(
            prop,
            matches!(prop.kind, PropKind::Pointer { is_id: true, .. }),
            "ID reference",
        )
    }
}

fn id_of(prop: &PropDef, v: Value) -> Result<Option<IdAny>, RnaError> {
    match v {
        Value::Id(a) => Ok(a),
        Value::None => Ok(None),
        other => Err(internal_value(prop, "an ID", &other)),
    }
}

fn wrong_id(prop: &PropDef, a: IdAny) -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        format!(
            "{} does not accept a datablock of bpy.data.{}",
            prop.identifier,
            a.code.collection()
        ),
    )
}

fn none_id(prop: &PropDef) -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        format!("{} does not support a 'None' assignment", prop.identifier),
    )
}

impl<K: IdKind> IdField for Option<IdRef<K>> {
    fn get_id(&self) -> Option<IdAny> {
        self.map(IdRef::any)
    }
    fn set_id(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match id_of(prop, v)? {
            None => Ok(None),
            Some(a) => IdRef::from_any(a)
                .map(Some)
                .ok_or_else(|| wrong_id(prop, a)),
        }
    }
}

impl<K: IdKind> IdField for IdRef<K> {
    fn get_id(&self) -> Option<IdAny> {
        Some(self.any())
    }
    fn set_id(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        match id_of(prop, v)? {
            None => Err(none_id(prop)),
            Some(a) => IdRef::from_any(a).ok_or_else(|| wrong_id(prop, a)),
        }
    }
}

impl IdField for Option<IdAny> {
    fn get_id(&self) -> Option<IdAny> {
        *self
    }
    fn set_id(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        id_of(prop, v)
    }
}

impl IdField for IdAny {
    fn get_id(&self) -> Option<IdAny> {
        Some(*self)
    }
    fn set_id(prop: &PropDef, v: Value) -> Result<Self, RnaError> {
        id_of(prop, v)?.ok_or_else(|| none_id(prop))
    }
}

/// Name of a struct item through its `name_property` (refined type first).
fn item_name(item: &dyn Rna) -> Option<SmolStr> {
    let srna = item.srna();
    let np = srna.name_property?;
    let prop = srna.find_prop(np)?;
    match item.get(prop).ok()? {
        Field::Value(Value::Str(s)) => Some(s),
        _ => None,
    }
}

/// An in-memory `Vec<T>` of structs as an RNA collection (`coll(field)` bindings): keys come
/// from the items' `name_property`.
pub struct VecColl<'a, T> {
    /// The items.
    pub items: &'a [T],
    /// Item struct type.
    pub item_type: &'static StructDef,
}

impl<'a, T> VecColl<'a, T> {
    /// New view.
    pub fn new(items: &'a [T], item_type: &'static StructDef) -> Self {
        VecColl { items, item_type }
    }
}

impl<T: Rna> RnaColl for VecColl<'_, T> {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        item_name(self.items.get(i)?)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.items.len()).find(|i| self.key_of(*i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|t| Item::Struct(StructRef::Borrowed(t)))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

/// Mutable counterpart of [`VecColl`].
pub struct VecCollMut<'a, T> {
    /// The items.
    pub items: &'a mut Vec<T>,
    /// Item struct type.
    pub item_type: &'static StructDef,
}

impl<'a, T> VecCollMut<'a, T> {
    /// New view.
    pub fn new(items: &'a mut Vec<T>, item_type: &'static StructDef) -> Self {
        VecCollMut { items, item_type }
    }
}

impl<T: Rna> RnaColl for VecCollMut<'_, T> {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        item_name(self.items.get(i)?)
    }
    fn find(&self, key: &str) -> Option<usize> {
        (0..self.items.len()).find(|i| self.key_of(*i).as_deref() == Some(key))
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|t| Item::Struct(StructRef::Borrowed(t)))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

impl<T: RnaMut> RnaCollMut for VecCollMut<'_, T> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.items.len();
        match self.items.get_mut(i) {
            Some(t) => Ok(Box::new(t)),
            None => Err(index_out_of_range(i, len)),
        }
    }
}

/// A `Vec` of ID pointers as an RNA collection (`id_coll(field)` bindings: `mesh.materials`,
/// `collection.objects`). Items are `Item::Id`; names are resolved through `IdNames` by the
/// resolver, so `key_of`/`find` answer `None` (F-TR-4).
pub struct IdVecColl<'a, X> {
    /// The items.
    pub items: &'a [X],
    /// Item struct type.
    pub item_type: &'static StructDef,
}

impl<'a, X> IdVecColl<'a, X> {
    /// New view.
    pub fn new(items: &'a [X], item_type: &'static StructDef) -> Self {
        IdVecColl { items, item_type }
    }
}

impl<X: IdField + Sync> RnaColl for IdVecColl<'_, X> {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|x| Item::Id(x.get_id()))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

/// Mutable counterpart of [`IdVecColl`] (items are re-rooted, never mutated in place).
pub struct IdVecCollMut<'a, X> {
    /// The items.
    pub items: &'a mut Vec<X>,
    /// Item struct type.
    pub item_type: &'static StructDef,
}

impl<'a, X> IdVecCollMut<'a, X> {
    /// New view.
    pub fn new(items: &'a mut Vec<X>, item_type: &'static StructDef) -> Self {
        IdVecCollMut { items, item_type }
    }
}

impl<X: IdField + Sync> RnaColl for IdVecCollMut<'_, X> {
    fn item_type(&self) -> &'static StructDef {
        self.item_type
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        self.items
            .get(i)
            .map(|x| Item::Id(x.get_id()))
            .ok_or_else(|| index_out_of_range(i, self.items.len()))
    }
}

impl<X: IdField + Sync> RnaCollMut for IdVecCollMut<'_, X> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        // NotImplemented: a bulk write falls back to item writes through each ID's own root.
        Err(RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::TypeError,
            format!(
                "{}[{i}] is a datablock: access it through its own root",
                self.item_type.identifier
            ),
        ))
    }
}

/// Item type of a `Vec<T>` bound with `coll(field)` and no explicit `item = …`.
pub fn vec_item_type<T: RnaStruct>(_: &[T]) -> &'static StructDef {
    T::struct_def()
}
