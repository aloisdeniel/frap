// SPDX-License-Identifier: GPL-3.0-or-later
//! Operator spec types shared by generated code and the framework (ARCHITECTURE §10.1,
//! CONTRACT frozen at M1 W0, amended by RFC 0001 F-OP-1..4).

use std::collections::BTreeSet;

use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::PropDef;
use crate::value::Value;

/// A typed operator property with Blender's is-set flag (`RNA_struct_property_is_set`).
#[derive(Clone, Debug, PartialEq)]
pub struct Prop<T> {
    /// Value (default when not set).
    pub value: T,
    /// Whether the caller passed it.
    pub is_set: bool,
}

impl<T> Prop<T> {
    /// Maps the value, keeping `is_set` (§10.4: `p.align.map(Into::into)`).
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prop<U> {
        Prop {
            value: f(self.value),
            is_set: self.is_set,
        }
    }
}

/// Small growable bit set (F-OP-1: no bit-set crate in §3.5; glTF export has 110 props).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct BitSet(SmallVec<[u64; 2]>);

impl BitSet {
    /// Empty set.
    pub fn new() -> Self {
        BitSet::default()
    }
    /// Sets bit `i`.
    pub fn insert(&mut self, i: usize) {
        let w = i / 64;
        if self.0.len() <= w {
            self.0.resize(w + 1, 0);
        }
        self.0[w] |= 1 << (i % 64);
    }
    /// Tests bit `i`.
    pub fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
    /// Set bits in ascending order.
    pub fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(w, bits)| {
            (0..64)
                .filter(move |b| bits & (1u64 << b) != 0)
                .map(move |b| w * 64 + b)
        })
    }
}

/// Output of argument conversion: one value per spec prop, in spec order, defaults filled in
/// for unset props.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct OpArgs {
    /// Values in spec order.
    pub values: IndexMap<&'static str, Value>,
    /// Indices (into `values`, i.e. spec order) of the props the caller set.
    pub is_set: BitSet,
    /// Macro sub-operator arguments, keyed by `bl_idname` (`"TRANSFORM_OT_translate"`).
    pub nested: IndexMap<&'static str, OpArgs>,
}

impl OpArgs {
    /// Value of a prop. Panics only on a spec/props-struct mismatch, which drift tests catch.
    pub fn get(&self, name: &str) -> &Value {
        self.values
            .get(name)
            .unwrap_or_else(|| panic!("invariant: operator prop {name} missing from OpArgs"))
    }
    /// Whether the caller set the prop named `name`.
    pub fn was_set(&self, name: &str) -> bool {
        self.values
            .get_index_of(name)
            .is_some_and(|i| self.is_set.contains(i))
    }
    /// Typed extraction used by the generated `from_args` (F-OP-2).
    pub fn prop<T: FromValue>(&self, index: usize, identifier: &'static str) -> Prop<T> {
        Prop {
            value: T::from_value(self.get(identifier)),
            is_set: self.is_set.contains(index),
        }
    }
}

/// Infallible conversion from an already-coerced `Value` to a typed props-struct field (F-OP-2).
pub trait FromValue: Sized {
    /// Converts; `coerce` has already validated `v` against the prop.
    fn from_value(v: &Value) -> Self;
}

impl FromValue for bool {
    fn from_value(v: &Value) -> Self {
        matches!(v, Value::Bool(true))
    }
}
impl FromValue for i32 {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::Int(i) => *i as i32,
            _ => 0,
        }
    }
}
impl FromValue for f32 {
    fn from_value(v: &Value) -> Self {
        v.as_f32().unwrap_or(0.0)
    }
}
impl FromValue for SmolStr {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::Str(s) | Value::Enum(s) => s.clone(),
            _ => SmolStr::default(),
        }
    }
}
impl<const N: usize> FromValue for [f32; N] {
    fn from_value(v: &Value) -> Self {
        let mut a = [0.0; N];
        if let Value::FloatArray { data, .. } = v {
            for (o, x) in a.iter_mut().zip(data.iter()) {
                *o = *x;
            }
        }
        a
    }
}
impl<const R: usize, const C: usize> FromValue for [[f32; C]; R] {
    fn from_value(v: &Value) -> Self {
        let mut a = [[0.0; C]; R];
        if let Value::FloatArray { data, .. } = v {
            for (i, x) in data.iter().enumerate().take(R * C) {
                a[i / C][i % C] = *x;
            }
        }
        a
    }
}
impl<const N: usize> FromValue for [bool; N] {
    fn from_value(v: &Value) -> Self {
        let mut a = [false; N];
        if let Value::BoolArray(d) = v {
            for (o, x) in a.iter_mut().zip(d.iter()) {
                *o = *x;
            }
        }
        a
    }
}
impl<const N: usize> FromValue for [i32; N] {
    fn from_value(v: &Value) -> Self {
        let mut a = [0; N];
        if let Value::IntArray(d) = v {
            for (o, x) in a.iter_mut().zip(d.iter()) {
                *o = *x as i32;
            }
        }
        a
    }
}
impl FromValue for BTreeSet<SmolStr> {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::EnumSet(s) => s.clone(),
            _ => BTreeSet::new(),
        }
    }
}
impl FromValue for Vec<IndexMap<SmolStr, Value>> {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::Structs(s) => s.clone(),
            _ => Vec::new(),
        }
    }
}
impl FromValue for Value {
    fn from_value(v: &Value) -> Self {
        v.clone()
    }
}

/// Implemented by the generated typed props struct of an operator (§10.4).
pub trait OpProps: Sized {
    /// "mesh.primitive_cube_add"
    const IDNAME: &'static str;
    /// "MESH_OT_primitive_cube_add"
    const BL_IDNAME: &'static str;
    /// The spec static.
    fn spec() -> &'static OperatorSpec;
    /// Field-by-field extraction from converted arguments.
    fn from_args(a: &OpArgs) -> Self;
}

bitflags::bitflags! {
    /// Operator options (`bl_options`); names equal Blender's.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
    pub struct OpOptions: u16 {
        /// REGISTER
        const REGISTER = 1 << 0;
        /// UNDO
        const UNDO = 1 << 1;
        /// UNDO_GROUPED
        const UNDO_GROUPED = 1 << 2;
        /// BLOCKING
        const BLOCKING = 1 << 3;
        /// MACRO
        const MACRO = 1 << 4;
        /// GRAB_CURSOR
        const GRAB_CURSOR = 1 << 5;
        /// GRAB_CURSOR_X
        const GRAB_CURSOR_X = 1 << 6;
        /// GRAB_CURSOR_Y
        const GRAB_CURSOR_Y = 1 << 7;
        /// DEPENDS_ON_CURSOR
        const DEPENDS_ON_CURSOR = 1 << 8;
        /// PRESET
        const PRESET = 1 << 9;
        /// INTERNAL
        const INTERNAL = 1 << 10;
        /// MODAL_PRIORITY
        const MODAL_PRIORITY = 1 << 11;
    }
}

impl OpOptions {
    /// Names of the set options, sorted like Blender's dump (`sorted(bl_options)`).
    pub fn names(self) -> Vec<&'static str> {
        let mut v: Vec<_> = self.iter_names().map(|(n, _)| n).collect();
        v.sort_unstable();
        v
    }
}

/// Result of `op.poll()` in one dumped context.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PollCell {
    /// Poll returned True.
    Y,
    /// Poll returned False.
    N,
    /// Poll raised.
    Raised,
}

/// Dumped poll results in contexts A–E, without and with a VIEW_3D override [OBJ §0].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PollMatrix {
    /// Contexts A–E, no override.
    pub plain: [PollCell; 5],
    /// Contexts A–E inside a VIEW_3D override.
    pub view3d: [PollCell; 5],
}

/// Dumped operator metadata. Statics are generated into `frap-spec-ops-<group>` or decoded by
/// `frap-spec-catalog` (pointer identity holds for the process either way).
#[derive(Debug)]
pub struct OperatorSpec {
    /// "mesh.primitive_cube_add"
    pub idname: &'static str,
    /// "MESH_OT_primitive_cube_add"
    pub bl_idname: &'static str,
    /// "Add Cube"
    pub label: &'static str,
    /// Description.
    pub description: &'static str,
    /// `bl_options`.
    pub options: OpOptions,
    /// Properties in RNA order.
    pub props: &'static [PropDef],
    /// Macro steps (`["MESH_OT_extrude_region", "TRANSFORM_OT_translate"]`); empty otherwise.
    pub macro_steps: &'static [&'static str],
    /// Dumped poll matrix; `None` for hand-written `frap.*` specs (F-OP-3).
    pub poll_matrix: Option<PollMatrix>,
}

impl OperatorSpec {
    /// Finds a prop by identifier.
    pub fn prop(&self, name: &str) -> Option<&'static PropDef> {
        self.props.iter().find(|p| p.identifier == name)
    }
    /// Index of a prop (its bit in `OpArgs::is_set`).
    pub fn prop_index(&self, name: &str) -> Option<usize> {
        self.props.iter().position(|p| p.identifier == name)
    }
}
