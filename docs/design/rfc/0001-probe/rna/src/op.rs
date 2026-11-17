// SPDX-License-Identifier: GPL-3.0-or-later
//! §10.1 Definitions shared by generated code and the framework (`frap-rna/src/op.rs`, CONTRACT).

use indexmap::IndexMap;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::def::PropDef;
use crate::value::Value;

#[derive(Clone, Debug, PartialEq)]
pub struct Prop<T> {
    pub value: T,
    pub is_set: bool,
}

impl<T> Prop<T> {
    /// Used by the §10.4 worked example (`p.align.map(Into::into)`) but not in the contract
    /// (RFC 0001 F-OP-4).
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Prop<U> {
        Prop {
            value: f(self.value),
            is_set: self.is_set,
        }
    }
}

/// `OpArgs::is_set: BitSet` names a type no listed dependency provides (§3.5 has no bit-set
/// crate; glTF export has 110 props, so `u64` is not enough). RFC 0001 F-OP-1.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BitSet(SmallVec<[u64; 2]>);

impl BitSet {
    pub fn insert(&mut self, i: usize) {
        let w = i / 64;
        if self.0.len() <= w {
            self.0.resize(w + 1, 0);
        }
        self.0[w] |= 1 << (i % 64);
    }
    pub fn contains(&self, i: usize) -> bool {
        self.0.get(i / 64).is_some_and(|w| w & (1 << (i % 64)) != 0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct OpArgs {
    pub values: IndexMap<&'static str, Value>,
    pub is_set: BitSet,
    pub nested: IndexMap<&'static str, OpArgs>,
}

impl OpArgs {
    /// Extraction helper the generated `from_args` uses (RFC 0001 F-OP-2: the contract says
    /// "generated field-by-field extraction" but `Value` -> field type needs a shared trait).
    pub fn prop<T: FromValue>(&self, index: usize, identifier: &'static str) -> Prop<T> {
        let v = self
            .values
            .get(identifier)
            .expect("invariant: OpArgs filled with defaults for every prop");
        Prop {
            value: T::from_value(v),
            is_set: self.is_set.contains(index),
        }
    }
}

/// RFC 0001 F-OP-2. Infallible by design: `coerce` already validated every value.
pub trait FromValue: Sized {
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
            _ => panic!("invariant: int"),
        }
    }
}
impl FromValue for f32 {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::Float(f) => *f,
            Value::Double(f) => *f as f32,
            _ => panic!("invariant: float"),
        }
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
            a.copy_from_slice(&data[..N]);
        }
        a
    }
}
impl<const R: usize, const C: usize> FromValue for [[f32; C]; R] {
    fn from_value(v: &Value) -> Self {
        let mut a = [[0.0; C]; R];
        if let Value::FloatArray { data, .. } = v {
            for r in 0..R {
                a[r].copy_from_slice(&data[r * C..r * C + C]);
            }
        }
        a
    }
}
impl<const N: usize> FromValue for [bool; N] {
    fn from_value(v: &Value) -> Self {
        let mut a = [false; N];
        if let Value::BoolArray(d) = v {
            a.copy_from_slice(&d[..N]);
        }
        a
    }
}
impl<const N: usize> FromValue for [i32; N] {
    fn from_value(v: &Value) -> Self {
        let mut a = [0; N];
        if let Value::IntArray(d) = v {
            for (o, i) in a.iter_mut().zip(d.iter()) {
                *o = *i as i32;
            }
        }
        a
    }
}
/// Enum-flag props: the set of identifiers.
impl FromValue for std::collections::BTreeSet<SmolStr> {
    fn from_value(v: &Value) -> Self {
        match v {
            Value::EnumSet(s) => s.clone(),
            _ => Default::default(),
        }
    }
}

pub trait OpProps: Sized {
    const IDNAME: &'static str;
    const BL_IDNAME: &'static str;
    fn spec() -> &'static OperatorSpec;
    fn from_args(a: &OpArgs) -> Self;
}

bitflags::bitflags! {
    /// Union of `bl_options` observed in the mesh/object dumps (+ the remaining Blender flags).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct OpOptions: u16 {
        const REGISTER = 1 << 0;
        const UNDO = 1 << 1;
        const UNDO_GROUPED = 1 << 2;
        const BLOCKING = 1 << 3;
        const MACRO = 1 << 4;
        const GRAB_CURSOR = 1 << 5;
        const GRAB_CURSOR_X = 1 << 6;
        const GRAB_CURSOR_Y = 1 << 7;
        const DEPENDS_ON_CURSOR = 1 << 8;
        const PRESET = 1 << 9;
        const INTERNAL = 1 << 10;
        const MODAL_PRIORITY = 1 << 11;
    }
}

/// Result of `op.poll()` in one dumped context.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PollCell {
    Y,
    N,
    Raised,
}

/// Contexts A–E without / with a VIEW_3D override (`tools/blender/dump_ops.py`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PollMatrix {
    pub plain: [PollCell; 5],
    pub view3d: [PollCell; 5],
}

#[derive(Debug)]
pub struct OperatorSpec {
    pub idname: &'static str,
    pub bl_idname: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub options: OpOptions,
    pub props: &'static [PropDef],
    pub macro_steps: &'static [&'static str],
    pub poll_matrix: PollMatrix,
}

impl OperatorSpec {
    pub fn prop_index(&self, identifier: &str) -> Option<usize> {
        self.props.iter().position(|p| p.identifier == identifier)
    }
}
