// SPDX-License-Identifier: GPL-3.0-or-later
//! Path resolution (ARCHITECTURE §9.3, research [RNA §1.1]): every path form —
//! `bpy.data.<coll>`, `bpy.context.<member>` and their `D`/`C`/`data`/`context` aliases,
//! `.attr`, `['Name']`, `["Name"]`, `['Name', lib]`, `[i]` (negative allowed), `[a:b]`, array
//! elements (`location[1]`, `location.x`, `matrix_world[0][1]`), RNA and collection functions —
//! with Blender's error classes and messages (verified on 5.2.4).
//!
//! Resolution walks read-only, one segment at a time, producing an owned [`Target`]: a
//! canonical [`RnaPtr`] that re-roots at every ID it reaches (`Root::Id`) or embedded ID
//! (`Root::Embedded`), so a path never borrows across datablocks. Reading and writing walk the
//! canonical pointer again from its root (`access`, `write`).

mod access;
pub mod compat;
mod context;
mod write;

use frap_lang::PathSeg;
use frap_rna::{
    ArrayShape, Bulk, Detached, EulerOrder, Field, FrapError, FuncDef, IdAny, IdNames, Item,
    MathTag, PropDef, PropFlags, PropKind, PyExc, RnaPtr, Root, Seg, StructDef, Value,
};
use frap_scene::HostRead;
use serde_json::{Value as J, json};
use smol_str::SmolStr;

pub use access::{CollView, get_field, id_srna, not_bound, stale, with_coll, with_struct};
pub use context::Member;
pub use write::{set_prop, with_coll_mut, with_struct_mut};

use crate::errors::*;
use crate::response::{f32_json, rna_json, value_json};

/// A resolved path (owned; reads and writes walk `ptr` again from its root).
#[derive(Clone, Debug)]
pub enum Target {
    /// `bpy.data`
    Data,
    /// `bpy.context`
    Context,
    /// A struct or an ID.
    Struct(RnaPtr),
    /// Python `None` (an unset pointer such as `scene.camera`).
    None,
    /// A non-collection property, whole or one part of an array.
    Prop(PropPlace),
    /// A collection property, or a slice of one.
    Coll(CollPlace),
    /// A Python list or tuple of targets (`bpy.context.selected_objects`, `Object.children`).
    List {
        /// `list` or `tuple` (error messages).
        what: &'static str,
        /// The items.
        items: Vec<Target>,
    },
    /// A function bound to a struct or collection.
    Func(FuncTarget),
    /// A plain Python value produced by a call (`find()` → `int`).
    Value(Value),
}

/// A property of a struct.
#[derive(Clone, Debug)]
pub struct PropPlace {
    /// Owner (`bpy.data` / `bpy.context` for their own properties).
    pub ptr: RnaPtr,
    /// The property.
    pub prop: &'static PropDef,
    /// Which part of an array.
    pub elem: Elem,
}

/// Part of an array property. Indices are kept as written and checked on access, because
/// Blender's message differs between a read and a write.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Elem {
    /// The whole value.
    Whole,
    /// `[i]`
    Index(i64),
    /// `.x .y .z .w .r .g .b` (index into the array).
    Component(char, usize),
    /// `[a:b]`
    Slice(Option<i64>, Option<i64>),
    /// `matrix[r]` (a row, a `Vector`).
    Row(usize),
    /// `matrix[r][c]`
    RowCol(usize, i64),
}

/// A collection property or a slice of one.
#[derive(Clone, Debug)]
pub struct CollPlace {
    /// Owner.
    pub ptr: RnaPtr,
    /// The collection property.
    pub prop: &'static PropDef,
    /// `coll[a:b]`: a Python list of items `a..b` (already clamped).
    pub range: Option<(usize, usize)>,
}

/// Python-level collection methods of `bpy_prop_collection` (implemented by the session).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CollFunc {
    /// `get(key, default=None)`
    Get,
    /// `find(key)`
    Find,
    /// `keys()`
    Keys,
    /// `values()`
    Values,
    /// `items()`
    Items,
    /// `foreach_get(attr, seq?)`
    ForeachGet,
    /// `foreach_set(attr, seq)`
    ForeachSet,
}

impl CollFunc {
    fn parse(s: &str) -> Option<CollFunc> {
        Some(match s {
            "get" => CollFunc::Get,
            "find" => CollFunc::Find,
            "keys" => CollFunc::Keys,
            "values" => CollFunc::Values,
            "items" => CollFunc::Items,
            "foreach_get" => CollFunc::ForeachGet,
            "foreach_set" => CollFunc::ForeachSet,
            _ => return None,
        })
    }
    /// Python name.
    pub fn name(self) -> &'static str {
        match self {
            CollFunc::Get => "get",
            CollFunc::Find => "find",
            CollFunc::Keys => "keys",
            CollFunc::Values => "values",
            CollFunc::Items => "items",
            CollFunc::ForeachGet => "foreach_get",
            CollFunc::ForeachSet => "foreach_set",
        }
    }
}

/// Python-level `bpy_struct` methods implemented by the session.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StructFunc {
    /// `path_from_id(property="")`
    PathFromId,
    /// `path_resolve(path, coerce=True)`
    PathResolve,
}

/// A callable.
#[derive(Clone, Debug)]
pub enum FuncTarget {
    /// An RNA function (`Object.select_set`, `BlendDataObjects.new`) or a Python-only
    /// `bpy_struct` method (`keyframe_insert`), implemented through `RNA_FUNCS`.
    Rna {
        /// `self` (for collection functions: the collection's API struct).
        this: RnaPtr,
        /// The definition.
        def: &'static FuncDef,
    },
    /// A `bpy_prop_collection` method.
    Coll {
        /// The collection.
        coll: CollPlace,
        /// Which method.
        which: CollFunc,
    },
    /// A `bpy_struct` method.
    Struct {
        /// `self`.
        this: RnaPtr,
        /// Which method.
        which: StructFunc,
    },
    /// A `mathutils` method bound to a value read from RNA (`ob.matrix_world.to_euler`,
    /// `ob.location.normalized`); the call runs on a copy, as in Blender.
    Math {
        /// The receiver (a float array whose shape and tag give its `mathutils` type).
        recv: Value,
        /// The property it was read from, when it was (its subtype decides the wrapper).
        prop: Option<&'static PropDef>,
        /// Method name.
        name: &'static str,
    },
}

impl FuncTarget {
    /// Python name of the function.
    pub fn name(&self) -> &'static str {
        match self {
            FuncTarget::Rna { def, .. } => def.identifier,
            FuncTarget::Coll { which, .. } => which.name(),
            FuncTarget::Struct { which, .. } => match which {
                StructFunc::PathFromId => "path_from_id",
                StructFunc::PathResolve => "path_resolve",
            },
            FuncTarget::Math { name, .. } => name,
        }
    }
    /// Python repr of the owner, plus the function name (`bpy.data.objects['Cube'].select_set`).
    pub fn repr(&self, names: &dyn IdNames) -> String {
        let owner = match self {
            FuncTarget::Rna { this, .. } | FuncTarget::Struct { this, .. } => this.repr(names),
            FuncTarget::Coll { coll, .. } => coll_repr(coll, names),
            FuncTarget::Math { recv, prop, .. } => match prop {
                Some(p) => ArrayKind::of(p).py_name().to_owned(),
                None => value_py_name(recv).to_owned(),
            },
        };
        format!("{owner}.{}", self.name())
    }
}

/// Output caps of value encoding (`--max-items`, `--max-array`, §13.3).
#[derive(Copy, Clone, Debug)]
pub struct Caps {
    /// Names listed in a `$coll`.
    pub max_items: usize,
    /// Scalars in a `$array`.
    pub max_array: usize,
}

// ------------------------------------------------------------------ errors

fn attr_error(msg: String) -> FrapError {
    FrapError::new(PyExc::AttributeError, RNA_NO_ATTRIBUTE, msg)
}

fn type_error(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::TypeError, RNA_TYPE, msg)
}

fn index_error(msg: impl Into<String>) -> FrapError {
    FrapError::new(PyExc::IndexError, RNA_INDEX_RANGE, msg)
}

/// `'Object' object has no attribute 'foo'` + `did_you_mean`, or the removed-4.x answer
/// (`RNA_REMOVED_IN_5X` with the replacement as hint).
pub fn no_attr(srna: &'static StructDef, attr: &str) -> FrapError {
    if let Some(r) = compat::lookup(srna, attr) {
        return compat::error(srna.identifier, r);
    }
    let mut cands: Vec<&str> = srna.all_props().iter().map(|p| p.identifier).collect();
    let mut s = Some(srna);
    while let Some(d) = s {
        cands.extend(d.funcs.iter().map(|f| f.identifier));
        s = d.base;
    }
    let sugg = frap_op::suggest(attr, cands.iter().copied());
    attr_error(format!(
        "'{}' object has no attribute '{attr}'",
        srna.identifier
    ))
    .with_detail(
        "did_you_mean",
        sugg.into_iter().map(J::from).collect::<Vec<_>>(),
    )
}

fn not_impl(what: impl std::fmt::Display) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!("frap: {what} is not implemented yet"),
    )
}

// ------------------------------------------------------------------ python-level types

/// How Python wraps an array property (`bpy_rna` + `mathutils`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArrayKind {
    /// `mathutils.Vector` of n components.
    Vector(usize),
    /// `mathutils.Euler`
    Euler,
    /// `mathutils.Quaternion`
    Quaternion,
    /// 3-component `mathutils.Color`
    Color,
    /// `mathutils.Matrix` (rows, cols)
    Matrix(usize, usize),
    /// Plain `bpy_prop_array`
    PropArray,
}

impl ArrayKind {
    /// The wrapper of a (non-scalar) property.
    pub fn of(prop: &PropDef) -> ArrayKind {
        let float = matches!(prop.kind, PropKind::Float { .. });
        let n = prop.array_len();
        if !float {
            return ArrayKind::PropArray;
        }
        match (MathTag::for_subtype(prop.subtype), prop.array) {
            (MathTag::Matrix, ArrayShape::Matrix(r, c)) => {
                ArrayKind::Matrix(r as usize, c as usize)
            }
            (MathTag::Matrix, ArrayShape::Fixed(16)) => ArrayKind::Matrix(4, 4),
            (MathTag::Matrix, ArrayShape::Fixed(9)) => ArrayKind::Matrix(3, 3),
            (_, ArrayShape::Matrix(..) | ArrayShape::Dynamic) => ArrayKind::PropArray,
            (MathTag::Vector, _) if (2..=4).contains(&n) => ArrayKind::Vector(n),
            (MathTag::Euler(_), _) if n == 3 => ArrayKind::Euler,
            (MathTag::Quaternion, _) if n == 4 => ArrayKind::Quaternion,
            (MathTag::Color, _) if n == 3 => ArrayKind::Color,
            _ => ArrayKind::PropArray,
        }
    }
    /// The wrapper of a free-standing float array value (a call result, a matrix row).
    pub fn of_value(data_len: usize, shape: ArrayShape, tag: MathTag) -> ArrayKind {
        match (tag, shape) {
            (MathTag::Matrix, ArrayShape::Matrix(r, c)) => {
                ArrayKind::Matrix(r as usize, c as usize)
            }
            (MathTag::Vector, _) => ArrayKind::Vector(data_len),
            (MathTag::Euler(_), _) => ArrayKind::Euler,
            (MathTag::Quaternion, _) => ArrayKind::Quaternion,
            (MathTag::Color, _) if data_len == 3 => ArrayKind::Color,
            _ => ArrayKind::PropArray,
        }
    }
    /// Python type name.
    pub fn py_name(self) -> &'static str {
        match self {
            ArrayKind::Vector(_) => "Vector",
            ArrayKind::Euler => "Euler",
            ArrayKind::Quaternion => "Quaternion",
            ArrayKind::Color => "Color",
            ArrayKind::Matrix(..) => "Matrix",
            ArrayKind::PropArray => "bpy_prop_array",
        }
    }
    fn lower(self) -> &'static str {
        match self {
            ArrayKind::Vector(_) => "vector",
            ArrayKind::Euler => "euler",
            ArrayKind::Quaternion => "quaternion",
            ArrayKind::Color => "color",
            ArrayKind::Matrix(..) => "matrix",
            ArrayKind::PropArray => "bpy_prop_array",
        }
    }
    /// Blender's read-index error.
    pub fn read_index_error(self, i: i64, len: usize) -> FrapError {
        index_error(match self {
            ArrayKind::Vector(_) => "vector[index]: out of range".to_owned(),
            ArrayKind::PropArray => {
                let adj = if i < 0 { i + len as i64 } else { i };
                format!("bpy_prop_array[index]: index {adj} out of range")
            }
            k => format!("{}[attribute]: array index out of range", k.lower()),
        })
    }
    /// Blender's write-index error.
    pub fn write_index_error(self) -> FrapError {
        index_error(match self {
            ArrayKind::Vector(_) => "vector[index] = x: assignment index out of range".to_owned(),
            ArrayKind::Matrix(..) => "matrix[attribute] = x: bad row".to_owned(),
            ArrayKind::PropArray => "bpy_prop_array[index] = value: index out of range".to_owned(),
            k => format!(
                "{}[attribute] = x: array assignment index out of range",
                k.lower()
            ),
        })
    }
    /// Index of a component accessor (`.x`, `.w`, `.r`).
    fn component(self, c: &str) -> Result<Option<usize>, FrapError> {
        let i = match (self, c) {
            (ArrayKind::Vector(n), "x" | "y" | "z" | "w") => {
                let i = "xyzw".find(c).expect("invariant: xyzw");
                if i >= n {
                    return Err(attr_error(format!(
                        "Vector.{c}: unavailable on {n}d vector"
                    )));
                }
                i
            }
            (ArrayKind::Euler, "x" | "y" | "z") => "xyz".find(c).expect("invariant: xyz"),
            (ArrayKind::Quaternion, "w" | "x" | "y" | "z") => {
                "wxyz".find(c).expect("invariant: wxyz")
            }
            (ArrayKind::Color, "r" | "g" | "b") => "rgb".find(c).expect("invariant: rgb"),
            _ => return Ok(None),
        };
        Ok(Some(i))
    }
}

/// Python type name of a scalar property value.
pub fn scalar_py_name(prop: &PropDef) -> &'static str {
    match prop.kind {
        PropKind::Bool { .. } => "bool",
        PropKind::Int { .. } => "int",
        PropKind::Float { .. } => "float",
        PropKind::String { .. } => "str",
        PropKind::Enum { .. } if prop.flags.contains(PropFlags::ENUM_FLAG) => "set",
        PropKind::Enum { .. } => "str",
        PropKind::Pointer { .. } => "NoneType",
        PropKind::Collection { .. } => "bpy_prop_collection",
        _ => "object",
    }
}

fn is_array(prop: &PropDef) -> bool {
    !matches!(prop.array, ArrayShape::Scalar)
        && matches!(
            prop.kind,
            PropKind::Float { .. } | PropKind::Int { .. } | PropKind::Bool { .. }
        )
}

/// Python type name of a resolved target (for `'<type>' object has no attribute` messages).
pub fn py_type_name(t: &Target) -> &'static str {
    match t {
        Target::Data => "BlendData",
        Target::Context => "Context",
        Target::Struct(p) => p.srna.identifier,
        Target::None => "NoneType",
        Target::Prop(p) if p.elem == Elem::Whole && is_array(p.prop) => {
            ArrayKind::of(p.prop).py_name()
        }
        Target::Prop(p) => match p.elem {
            Elem::Slice(..) => "tuple",
            Elem::Row(_) => "Vector",
            _ => {
                if is_array(p.prop) {
                    match p.prop.kind {
                        PropKind::Float { .. } => "float",
                        PropKind::Int { .. } => "int",
                        _ => "bool",
                    }
                } else {
                    scalar_py_name(p.prop)
                }
            }
        },
        Target::Coll(c) if c.range.is_some() => "list",
        Target::Coll(_) => "bpy_prop_collection",
        Target::List { what, .. } => what,
        Target::Func(_) => "bpy_func",
        Target::Value(v) => value_py_name(v),
    }
}

fn value_py_name(v: &Value) -> &'static str {
    match v {
        Value::None => "NoneType",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) | Value::Double(_) => "float",
        Value::Str(_) | Value::Enum(_) => "str",
        Value::EnumSet(_) => "set",
        Value::Tuple(_) => "tuple",
        Value::FloatArray { data, shape, tag } => {
            ArrayKind::of_value(data.len(), *shape, *tag).py_name()
        }
        _ => "list",
    }
}

// ------------------------------------------------------------------ resolution

/// The target of a path root.
pub fn root(r: frap_lang::PathRoot) -> Target {
    match r {
        frap_lang::PathRoot::BpyData => Target::Data,
        frap_lang::PathRoot::BpyContext => Target::Context,
    }
}

fn data_ptr() -> RnaPtr {
    RnaPtr::new(Root::Data, &frap_spec_rna::BLEND_DATA)
}

fn context_ptr() -> RnaPtr {
    RnaPtr::new(Root::Context, &frap_spec_rna::CONTEXT)
}

/// Resolves a path without calls (`PathSeg::Call` is the executor's job).
pub fn resolve(host: &dyn HostRead, path: &frap_lang::PathExpr) -> Result<Target, FrapError> {
    let mut t = root(path.root);
    for seg in &path.segs {
        t = step(host, t, seg)?;
    }
    Ok(t)
}

/// One segment. Everything reached from a [`Detached`] pointer (an `evaluated_get()` ID, a
/// `to_mesh()` result) is detached too, so writes through it are refused (`write.rs`); IDs it
/// reaches read `is_evaluated == False` as in Blender (`Detached::Temporary`). `ID.original`
/// leads back to the ordinary pointer, and `ID.is_evaluated` reads True on an evaluated ID
/// (WP 1.22 contract change).
pub fn step(host: &dyn HostRead, cur: Target, seg: &PathSeg) -> Result<Target, FrapError> {
    let id_root = match &cur {
        Target::Struct(p) if p.srna.is_id && p.path.is_empty() => Some(p.root),
        _ => None,
    };
    if let (Some(root), PathSeg::Attr(a)) = (id_root, seg) {
        match a.as_str() {
            // `ID.original`: the ID itself for an original, the original for a copy.
            "original" if matches!(root, Root::Id(_) | Root::Embedded { .. }) => {
                let Target::Struct(p) = cur else {
                    unreachable!("id_root is only set on structs")
                };
                return Ok(Target::Struct(RnaPtr {
                    detached: None,
                    ..p
                }));
            }
            "is_evaluated" if matches!(root, Root::Id(_) | Root::Embedded { .. }) => {
                let d = target_detached(&cur);
                return Ok(Target::Value(Value::Bool(d == Some(Detached::Evaluated))));
            }
            _ => {}
        }
    }
    let Some(d) = target_detached(&cur) else {
        return step_inner(host, cur, seg);
    };
    let parent_root = match &cur {
        Target::Struct(p) => Some(p.root),
        Target::Prop(p) => Some(p.ptr.root),
        Target::Coll(c) => Some(c.ptr.root),
        _ => None,
    };
    let mut t = step_inner(host, cur, seg)?;
    mark_detached(&mut t, d, parent_root);
    Ok(t)
}

fn target_detached(t: &Target) -> Option<Detached> {
    match t {
        Target::Struct(p) => p.detached,
        Target::Prop(p) => p.ptr.detached,
        Target::Coll(c) => c.ptr.detached,
        Target::Func(FuncTarget::Rna { this, .. } | FuncTarget::Struct { this, .. }) => {
            this.detached
        }
        Target::Func(FuncTarget::Coll { coll, .. }) => coll.ptr.detached,
        Target::List { items, .. } => items.iter().find_map(target_detached),
        _ => None,
    }
}

fn mark_detached(t: &mut Target, d: Detached, parent_root: Option<Root>) {
    let set = |p: &mut RnaPtr| {
        if p.detached.is_none() {
            // A re-rooted pointer is another ID of the copy, which Blender reports as not
            // evaluated (`ob.evaluated_get(dg).data.is_evaluated == False`, verified on 5.2.4).
            p.detached = Some(if Some(p.root) == parent_root {
                d
            } else {
                Detached::Temporary
            });
        }
    };
    match t {
        Target::Struct(p) => set(p),
        Target::Prop(p) => set(&mut p.ptr),
        Target::Coll(c) => set(&mut c.ptr),
        Target::Func(FuncTarget::Rna { this, .. } | FuncTarget::Struct { this, .. }) => set(this),
        Target::Func(FuncTarget::Coll { coll, .. }) => set(&mut coll.ptr),
        Target::Func(FuncTarget::Math { .. }) => {}
        Target::List { items, .. } => {
            for i in items {
                mark_detached(i, d, parent_root);
            }
        }
        Target::Data | Target::Context | Target::None | Target::Value(_) => {}
    }
}

fn step_inner(host: &dyn HostRead, cur: Target, seg: &PathSeg) -> Result<Target, FrapError> {
    match cur {
        Target::Data => step_struct(host, data_ptr(), seg),
        Target::Context => step_struct(host, context_ptr(), seg),
        Target::Struct(p) => step_struct(host, p, seg),
        Target::None => step_python(Target::None, seg),
        Target::Prop(p) => step_prop(host, p, seg),
        Target::Coll(c) => step_coll(host, c, seg),
        Target::List { what, items } => step_list(what, items, seg),
        Target::Value(v) => step_value(v, seg),
        t @ Target::Func(_) => step_python(t, seg),
    }
}

/// Segments on a plain Python value: `tuple` and `str` indexing and slicing, `mathutils`
/// components (`path_resolve('location').x`, `matrix[0:2][1]`).
fn step_value(v: Value, seg: &PathSeg) -> Result<Target, FrapError> {
    match (v, seg) {
        (Value::Tuple(items), PathSeg::Index(i)) => {
            let j = norm_index(*i, items.len())
                .ok_or_else(|| index_error("tuple index out of range"))?;
            Ok(Target::Value(
                items.into_iter().nth(j).expect("invariant: index checked"),
            ))
        }
        (Value::Tuple(items), PathSeg::Slice(a, b)) => {
            let (s, e) = norm_slice(*a, *b, items.len());
            Ok(Target::Value(Value::Tuple(
                items.into_iter().skip(s).take(e - s).collect(),
            )))
        }
        (Value::Tuple(_), PathSeg::Key(_) | PathSeg::KeyLib(..)) => Err(type_error(
            "tuple indices must be integers or slices, not str",
        )),
        (Value::Str(s) | Value::Enum(s), PathSeg::Index(i)) => {
            let chars: Vec<char> = s.chars().collect();
            let j = norm_index(*i, chars.len())
                .ok_or_else(|| index_error("string index out of range"))?;
            Ok(Target::Value(Value::Str(chars[j].to_string().into())))
        }
        (Value::Str(s) | Value::Enum(s), PathSeg::Slice(a, b)) => {
            let chars: Vec<char> = s.chars().collect();
            let (st, e) = norm_slice(*a, *b, chars.len());
            Ok(Target::Value(Value::Str(
                chars[st..e].iter().collect::<String>().into(),
            )))
        }
        (Value::Str(_) | Value::Enum(_), PathSeg::Key(_) | PathSeg::KeyLib(..)) => {
            Err(type_error("string indices must be integers, not 'str'"))
        }
        (Value::FloatArray { data, shape, tag }, seg) => {
            let kind = ArrayKind::of_value(data.len(), shape, tag);
            let row = |r: usize, c: usize| Value::FloatArray {
                data: data[r * c..(r + 1) * c].iter().copied().collect(),
                shape: ArrayShape::Fixed(c as u16),
                tag: MathTag::Vector,
            };
            match (kind, seg) {
                (ArrayKind::Matrix(r, c), PathSeg::Index(i)) => {
                    let j = norm_index(*i, r).ok_or_else(|| kind.read_index_error(*i, r))?;
                    Ok(Target::Value(row(j, c)))
                }
                (ArrayKind::Matrix(r, c), PathSeg::Slice(a, b)) => {
                    let (s, e) = norm_slice(*a, *b, r);
                    Ok(Target::Value(Value::Tuple(
                        (s..e).map(|j| row(j, c)).collect(),
                    )))
                }
                (_, PathSeg::Index(i)) => {
                    let j = norm_index(*i, data.len())
                        .ok_or_else(|| kind.read_index_error(*i, data.len()))?;
                    Ok(Target::Value(Value::Float(data[j])))
                }
                (_, PathSeg::Slice(a, b)) => {
                    let (s, e) = norm_slice(*a, *b, data.len());
                    Ok(Target::Value(Value::Tuple(
                        data[s..e].iter().map(|x| Value::Float(*x)).collect(),
                    )))
                }
                (_, PathSeg::Attr(a)) => {
                    if let Some(i) = kind.component(a)? {
                        return Ok(Target::Value(Value::Float(data[i])));
                    }
                    if let Some(name) = math_method(kind, a) {
                        return Ok(Target::Func(FuncTarget::Math {
                            recv: Value::FloatArray { data, shape, tag },
                            prop: None,
                            name,
                        }));
                    }
                    if let Some(v) = math_attr(kind, &data, a)? {
                        return Ok(Target::Value(v));
                    }
                    let sq = || dot_vn_vn(&data);
                    match (kind, a.as_str()) {
                        (ArrayKind::Euler, "order") => {
                            Ok(Target::Value(Value::Enum(SmolStr::new(match tag {
                                MathTag::Euler(o) => o.as_str(),
                                _ => EulerOrder::XYZ.as_str(),
                            }))))
                        }
                        (ArrayKind::Vector(_), "length" | "magnitude") => {
                            Ok(Target::Value(Value::Double(sq().sqrt())))
                        }
                        (ArrayKind::Vector(_), "length_squared") => {
                            Ok(Target::Value(Value::Double(sq())))
                        }
                        _ => Err(attr_error(format!(
                            "'{}' object has no attribute '{a}'",
                            kind.py_name()
                        ))),
                    }
                }
                (k, PathSeg::Key(_) | PathSeg::KeyLib(..)) => Err(type_error(format!(
                    "{} indices must be integers, not str",
                    k.lower()
                ))),
                _ => step_python(Target::Value(Value::FloatArray { data, shape, tag }), seg),
            }
        }
        (v, seg) => step_python(Target::Value(v), seg),
    }
}

/// Segments on a Python object without RNA members.
fn step_python(t: Target, seg: &PathSeg) -> Result<Target, FrapError> {
    let ty = py_type_name(&t);
    match seg {
        PathSeg::Attr(a) => Err(attr_error(format!("'{ty}' object has no attribute '{a}'"))),
        PathSeg::Call(_) => Err(type_error(format!("'{ty}' object is not callable"))),
        _ => Err(type_error(format!("'{ty}' object is not subscriptable"))),
    }
}

fn step_struct(_host: &dyn HostRead, ptr: RnaPtr, seg: &PathSeg) -> Result<Target, FrapError> {
    let srna = ptr.srna;
    match seg {
        PathSeg::Attr(a) => {
            let context = ptr.root == Root::Context && ptr.path.is_empty();
            if context && context::ABSENT.contains(&a.as_str()) {
                return Err(attr_error(format!(
                    "'Context' object has no attribute '{a}'"
                )));
            }
            if let Some(prop) = srna.find_prop(a) {
                // `bpy.context` members are computed (lists, re-rooted pointers), never RNA
                // collections; Python-only collection members (`Object.children`,
                // `users_collection`, …) are tuples in Blender, served by their get hook.
                let python_only = prop.flags.contains(PropFlags::PYTHON_ONLY);
                return Ok(match prop.kind {
                    PropKind::Collection { .. } if !context && !python_only => {
                        Target::Coll(CollPlace {
                            ptr,
                            prop,
                            range: None,
                        })
                    }
                    _ => Target::Prop(PropPlace {
                        ptr,
                        prop,
                        elem: Elem::Whole,
                    }),
                });
            }
            if let Some(def) = srna.find_func(a) {
                return Ok(Target::Func(FuncTarget::Rna { this: ptr, def }));
            }
            if !matches!(ptr.root, Root::Data | Root::Context) || !ptr.path.is_empty() {
                // `path_from_id`/`path_resolve` are `bpy_struct` methods implemented here;
                // `keyframe_insert`/`keyframe_delete` go through `RNA_FUNCS` (M6).
                let which = match a.as_str() {
                    "path_from_id" => Some(StructFunc::PathFromId),
                    "path_resolve" => Some(StructFunc::PathResolve),
                    _ => None,
                };
                if let Some(which) = which {
                    return Ok(Target::Func(FuncTarget::Struct { this: ptr, which }));
                }
                if let Some(def) = frap_spec_rna::BPY_STRUCT_FUNCS
                    .iter()
                    .find(|f| f.identifier == a.as_str())
                {
                    return Ok(Target::Func(FuncTarget::Rna { this: ptr, def }));
                }
            }
            Err(no_attr(srna, a))
        }
        PathSeg::Key(k) | PathSeg::KeyLib(k, _) => {
            if srna.is_id && matches!(ptr.root, Root::Id(_)) && ptr.path.is_empty() {
                // ID properties: frap stores none (custom properties arrive with M8).
                Err(FrapError::new(
                    PyExc::KeyError,
                    RNA_KEY_NOT_FOUND,
                    format!("'bpy_struct[key]: key \"{k}\" not found'"),
                ))
            } else {
                Err(type_error("this type doesn't support IDProperties"))
            }
        }
        PathSeg::Index(_) | PathSeg::Slice(..) => Err(type_error(format!(
            "'{}' object is not subscriptable",
            srna.identifier
        ))),
        PathSeg::Call(_) => Err(type_error(format!(
            "'{}' object is not callable",
            srna.identifier
        ))),
        _ => Err(not_impl("this path segment")),
    }
}

/// Dereferences a pointer property (or reads a context member) into a target.
pub fn deref(host: &dyn HostRead, p: &PropPlace) -> Result<Target, FrapError> {
    match read_prop(host, p)? {
        Read::Target(t) => Ok(t),
        Read::Value(v) => Ok(value_target(host, v)?),
    }
}

fn value_target(host: &dyn HostRead, v: Value) -> Result<Target, FrapError> {
    Ok(match v {
        Value::Id(Some(id)) => Target::Struct(RnaPtr::id(id, id_srna(host, id)?)),
        Value::Id(None) | Value::None => Target::None,
        Value::Ptr(p) => Target::Struct(*p),
        other => Target::Value(other),
    })
}

fn step_prop(host: &dyn HostRead, p: PropPlace, seg: &PathSeg) -> Result<Target, FrapError> {
    let prop = p.prop;
    if p.elem == Elem::Whole && !is_array(prop) {
        // Pointers, context members and Python-only collection members (tuples/lists).
        let derefs = matches!(
            prop.kind,
            PropKind::Pointer { .. } | PropKind::Collection { .. }
        ) || matches!(p.ptr.root, Root::Context);
        if derefs {
            let t = deref(host, &p)?;
            return step(host, t, seg);
        }
        // A plain value (`name[0]` indexes the `str`).
        return step_value(read_value(host, &p)?, seg);
    }
    let kind = ArrayKind::of(prop);
    match (p.elem, seg) {
        (Elem::Whole, PathSeg::Index(i))
            if kind == ArrayKind::PropArray
                && matches!(prop.kind, PropKind::Float { .. })
                && let ArrayShape::Matrix(r, c) = prop.array =>
        {
            // A multi-dimensional `bpy_prop_array` (`Object.bound_box`): an index reads one
            // row, itself a `bpy_prop_array` (verified on 5.2.4).
            let (r, c) = (r as usize, c as usize);
            let j = norm_index(*i, r).ok_or_else(|| kind.read_index_error(*i, r))?;
            let Value::FloatArray { data, .. } = read_value(host, &p)? else {
                return Err(stale("multi-dimensional array is not a float array"));
            };
            Ok(Target::Value(Value::FloatArray {
                data: data[j * c..(j + 1) * c].iter().copied().collect(),
                shape: ArrayShape::Fixed(c as u16),
                tag: MathTag::None,
            }))
        }
        (Elem::Whole, PathSeg::Slice(a, b))
            if kind == ArrayKind::PropArray
                && matches!(prop.kind, PropKind::Float { .. })
                && let ArrayShape::Matrix(r, c) = prop.array =>
        {
            // A slice of a multi-dimensional `bpy_prop_array`: a tuple of rows.
            let (r, c) = (r as usize, c as usize);
            let (st, e) = norm_slice(*a, *b, r);
            let Value::FloatArray { data, .. } = read_value(host, &p)? else {
                return Err(stale("multi-dimensional array is not a float array"));
            };
            Ok(Target::Value(Value::Tuple(
                (st..e)
                    .map(|j| Value::FloatArray {
                        data: data[j * c..(j + 1) * c].iter().copied().collect(),
                        shape: ArrayShape::Fixed(c as u16),
                        tag: MathTag::None,
                    })
                    .collect(),
            )))
        }
        (Elem::Whole, PathSeg::Index(i)) => {
            if let ArrayKind::Matrix(r, _) = kind
                && let Some(row) = norm_index(*i, r)
            {
                return Ok(Target::Prop(PropPlace {
                    elem: Elem::Row(row),
                    ..p
                }));
            }
            // An out-of-range matrix row stays an `Index`: reading it raises the read message,
            // assigning it the write message (`matrix[attribute] = x: bad row`).
            Ok(Target::Prop(PropPlace {
                elem: Elem::Index(*i),
                ..p
            }))
        }
        (Elem::Whole, PathSeg::Slice(a, b)) => Ok(Target::Prop(PropPlace {
            elem: Elem::Slice(*a, *b),
            ..p
        })),
        (Elem::Whole, PathSeg::Attr(a)) => match kind.component(a)? {
            Some(i) => Ok(Target::Prop(PropPlace {
                elem: Elem::Component(a.chars().next().unwrap_or('x'), i),
                ..p
            })),
            None => match (kind, a.as_str()) {
                (ArrayKind::Euler, "order")
                | (ArrayKind::Vector(_), "length" | "length_squared" | "magnitude") => {
                    read_math_attr(host, &p, a)
                }
                (k, a) if let Some(name) = math_method(k, a) => {
                    Ok(Target::Func(FuncTarget::Math {
                        recv: read_value(host, &p)?,
                        prop: Some(p.prop),
                        name,
                    }))
                }
                (k, a) if k != ArrayKind::PropArray => {
                    let v = read_value(host, &p)?;
                    let Value::FloatArray { data, .. } = &v else {
                        return Err(stale("math property is not a float array"));
                    };
                    match math_attr(k, data, a)? {
                        Some(v) => Ok(Target::Value(v)),
                        None => Err(attr_error(format!(
                            "'{}' object has no attribute '{a}'",
                            k.py_name()
                        ))),
                    }
                }
                _ => Err(attr_error(format!(
                    "'{}' object has no attribute '{a}'",
                    kind.py_name()
                ))),
            },
        },
        (Elem::Whole, PathSeg::Key(_) | PathSeg::KeyLib(..)) => Err(match kind {
            ArrayKind::PropArray => {
                type_error("bpy_prop_array[key]: invalid key, key must be an int")
            }
            k => type_error(format!("{} indices must be integers, not str", k.lower())),
        }),
        (Elem::Row(r), PathSeg::Index(j)) => Ok(Target::Prop(PropPlace {
            elem: Elem::RowCol(r, *j),
            ..p
        })),
        (Elem::Row(r), PathSeg::Attr(a)) => {
            let ArrayKind::Matrix(_, c) = kind else {
                return Err(stale("row of a non-matrix"));
            };
            match ArrayKind::Vector(c).component(a)? {
                // `matrix[r].x` stays a place, so it can be assigned.
                Some(i) => Ok(Target::Prop(PropPlace {
                    elem: Elem::RowCol(r, i as i64),
                    ..p
                })),
                None => step_value(read_value(host, &p)?, seg),
            }
        }
        (Elem::Row(_) | Elem::Slice(..), _) => {
            // A row is a `Vector`; a slice is a tuple of numbers (of row vectors for a matrix).
            step_value(read_value(host, &p)?, seg)
        }
        (Elem::Index(_) | Elem::Component(..) | Elem::RowCol(..), _) => {
            let v = read_value(host, &p)?; // checks the index (Blender's read message)
            step_value(v, seg)
        }
        (Elem::Whole, PathSeg::Call(_)) => Err(type_error(format!(
            "'{}' object is not callable",
            kind.py_name()
        ))),
        _ => Err(not_impl("this path segment")),
    }
}

fn is_swizzle(s: &str) -> bool {
    (2..=4).contains(&s.len()) && s.chars().all(|c| "xyzw".contains(c))
}

/// `mathutils` methods callable on a value read from RNA, per wrapper type (the ones
/// `session::eval::method` implements).
fn math_method(kind: ArrayKind, a: &str) -> Option<&'static str> {
    const VECTOR: &[&str] = &[
        "copy",
        "normalized",
        "to_tuple",
        "to_2d",
        "to_3d",
        "to_4d",
        "dot",
        "cross",
        "angle",
        "lerp",
    ];
    const EULER: &[&str] = &["copy", "to_matrix", "to_quaternion"];
    const QUAT: &[&str] = &[
        "copy",
        "to_matrix",
        "to_euler",
        "normalized",
        "inverted",
        "conjugated",
        "to_axis_angle",
    ];
    const MATRIX: &[&str] = &[
        "copy",
        "to_euler",
        "to_quaternion",
        "to_3x3",
        "to_4x4",
        "transposed",
        "inverted",
        "inverted_safe",
        "to_translation",
        "to_scale",
        "decompose",
    ];
    const COLOR: &[&str] = &["copy"];
    let list = match kind {
        ArrayKind::Vector(_) => VECTOR,
        ArrayKind::Euler => EULER,
        ArrayKind::Quaternion => QUAT,
        ArrayKind::Matrix(..) => MATRIX,
        ArrayKind::Color => COLOR,
        ArrayKind::PropArray => return None,
    };
    list.iter().copied().find(|m| *m == a)
}

/// Read-only `mathutils` attributes computed from the data (row-major for matrices):
/// `Matrix.translation`/`is_negative`/`median_scale`, `Quaternion.angle`/`axis`/`magnitude`,
/// `Vector` swizzles (`xy`, `zyx`, …). `None` when `a` is not one of them.
fn math_attr(kind: ArrayKind, data: &[f32], a: &str) -> Result<Option<Value>, FrapError> {
    let vector = |d: Vec<f32>| Value::FloatArray {
        shape: ArrayShape::Fixed(d.len() as u16),
        data: d.into_iter().collect(),
        tag: MathTag::Vector,
    };
    let m3 = |c: usize| -> [[f32; 3]; 3] {
        std::array::from_fn(|r| std::array::from_fn(|k| data[r * c + k]))
    };
    Ok(Some(match (kind, a) {
        (ArrayKind::Matrix(r, c), "translation") => {
            if r != 4 || c != 4 {
                return Err(attr_error(
                    "Matrix.translation: inappropriate matrix size, must be 4x4".to_owned(),
                ));
            }
            vector(vec![data[3], data[c + 3], data[2 * c + 3]])
        }
        (ArrayKind::Matrix(r, c), "is_negative") if r >= 3 && c >= 3 => {
            let m = m3(c);
            let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
                - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
                + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
            Value::Bool(det < 0.0)
        }
        (ArrayKind::Matrix(r, c), "median_scale") if r >= 3 && c >= 3 => {
            // `mat3_to_scale`: the length of the matrix applied to (√⅓, √⅓, √⅓), in floats.
            let m = m3(c);
            let u = 0.577_350_26_f32; // (float)M_SQRT1_3
            let v: [f32; 3] = std::array::from_fn(|i| m[i][0] * u + m[i][1] * u + m[i][2] * u);
            Value::Double((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt() as f64)
        }
        (ArrayKind::Quaternion, "magnitude") => {
            // `sqrt(dot_qtqt(q, q))`: a single-precision dot product.
            let d = data[0] * data[0] + data[1] * data[1] + data[2] * data[2] + data[3] * data[3];
            Value::Double((d as f64).sqrt())
        }
        (ArrayKind::Quaternion, "angle" | "axis") => {
            // `Quaternion_angle_get` / `_axis_get`: of the normalized quaternion.
            let (q, _) = frap_math::quat::normalize_qt([data[0], data[1], data[2], data[3]]);
            let (axis, angle) = frap_math::quat::quat_to_axis_angle(q);
            if a == "angle" {
                Value::Double(angle as f64)
            } else {
                vector(axis.to_vec())
            }
        }
        (ArrayKind::Vector(n), s) if is_swizzle(s) => {
            let mut out = Vec::with_capacity(s.len());
            for ch in s.chars() {
                let i = "xyzw".find(ch).expect("invariant: swizzle chars");
                if i >= n {
                    return Err(attr_error(
                        "Vector swizzle: specified axis not present".to_owned(),
                    ));
                }
                out.push(data[i]);
            }
            vector(out)
        }
        _ => return Ok(None),
    }))
}

fn read_math_attr(host: &dyn HostRead, p: &PropPlace, a: &str) -> Result<Target, FrapError> {
    let v = read_value(host, p)?;
    let Value::FloatArray { data, tag, .. } = v else {
        return Err(stale("math property is not a float array"));
    };
    Ok(Target::Value(match a {
        "order" => match tag {
            MathTag::Euler(o) => Value::Enum(SmolStr::new(o.as_str())),
            _ => Value::Enum(SmolStr::new(EulerOrder::XYZ.as_str())),
        },
        "length_squared" => Value::Double(dot_vn_vn(&data)),
        _ => Value::Double(dot_vn_vn(&data).sqrt()),
    }))
}

/// `dot_vn_vn(v, v)` as `mathutils` computes `Vector.length`/`length_squared`: single-precision
/// products summed in double precision, last component first.
fn dot_vn_vn(data: &[f32]) -> f64 {
    data.iter().rev().fold(0.0f64, |d, x| d + (x * x) as f64)
}

/// Python index normalisation: `Some(i)` within `0..len`.
pub fn norm_index(i: i64, len: usize) -> Option<usize> {
    let n = len as i64;
    let j = if i < 0 { i + n } else { i };
    (0..n).contains(&j).then_some(j as usize)
}

/// Python slice clamping (step 1).
pub fn norm_slice(a: Option<i64>, b: Option<i64>, len: usize) -> (usize, usize) {
    let n = len as i64;
    let clamp = |x: i64| {
        let x = if x < 0 { x + n } else { x };
        x.clamp(0, n) as usize
    };
    let s = a.map_or(0, clamp);
    let e = b.map_or(len, clamp);
    (s, e.max(s))
}

fn coll_len(host: &dyn HostRead, c: &CollPlace) -> Result<usize, FrapError> {
    if let Some((a, b)) = c.range {
        return Ok(b - a);
    }
    with_coll(host, &c.ptr, c.prop, &mut |v| Ok(v.len()))
}

fn coll_owner_name(c: &CollPlace) -> String {
    format!("{}.{}", c.ptr.srna.identifier, c.prop.identifier)
}

fn item_target(host: &dyn HostRead, c: &CollPlace, i: usize) -> Result<Target, FrapError> {
    with_coll(
        host,
        &c.ptr,
        c.prop,
        &mut |v| match v.item(i).map_err(access::rna_err)? {
            Item::Id(Some(id)) => Ok(Target::Struct(RnaPtr::id(id, id_srna(host, id)?))),
            Item::Id(None) => Ok(Target::None),
            Item::Struct(s) => Ok(Target::Struct(
                c.ptr
                    .child(Seg::Prop(c.prop.identifier), s.srna())
                    .child(v.item_seg(i), s.srna()),
            )),
        },
    )
}

fn step_coll(host: &dyn HostRead, c: CollPlace, seg: &PathSeg) -> Result<Target, FrapError> {
    if let Some((start, end)) = c.range {
        // A slice is a Python list; frap also lets bulk access page through it (§13.3).
        return match seg {
            PathSeg::Index(i) => {
                let j = norm_index(*i, end - start)
                    .ok_or_else(|| index_error("list index out of range"))?;
                item_target(host, &c, start + j)
            }
            PathSeg::Slice(a, b) => {
                let (s, e) = norm_slice(*a, *b, end - start);
                Ok(Target::Coll(CollPlace {
                    range: Some((start + s, start + e)),
                    ..c
                }))
            }
            PathSeg::Attr(a) if matches!(a.as_str(), "foreach_get" | "foreach_set") => {
                let which = CollFunc::parse(a).expect("invariant: foreach");
                Ok(Target::Func(FuncTarget::Coll { coll: c, which }))
            }
            PathSeg::Key(_) | PathSeg::KeyLib(..) => Err(type_error(
                "list indices must be integers or slices, not str",
            )),
            _ => step_python(Target::Coll(c), seg),
        };
    }
    match seg {
        PathSeg::Index(i) => {
            let len = coll_len(host, &c)?;
            match norm_index(*i, len) {
                Some(j) => item_target(host, &c, j),
                None if *i >= 0 => Err(index_error(format!(
                    "bpy_prop_collection[index]: index {i} out of range, size {len}"
                ))),
                None => Err(index_error(format!(
                    "bpy_prop_collection[{i}]: out of range."
                ))),
            }
        }
        PathSeg::Slice(a, b) => {
            let len = coll_len(host, &c)?;
            let (s, e) = norm_slice(*a, *b, len);
            Ok(Target::Coll(CollPlace {
                range: Some((s, e)),
                ..c
            }))
        }
        PathSeg::Key(k) => {
            let found = with_coll(host, &c.ptr, c.prop, &mut |v| {
                if !v.named() {
                    return Err(type_error(format!(
                        "bpy_prop_collection[key]: {} does not support string lookups",
                        coll_owner_name(&c)
                    )));
                }
                Ok(v.find(k))
            })?;
            match found {
                Some(i) => item_target(host, &c, i),
                None => Err(key_not_found(host, &c, k)),
            }
        }
        PathSeg::KeyLib(name, lib) => step_key_lib(host, c, name, lib.as_deref()),
        PathSeg::Attr(a) => {
            if let Some(which) = CollFunc::parse(a) {
                return Ok(Target::Func(FuncTarget::Coll { coll: c, which }));
            }
            if let PropKind::Collection { api: Some(api), .. } = c.prop.kind
                && let Some(api) = frap_spec_rna::find_struct(api)
            {
                let this = c.ptr.child(Seg::Prop(c.prop.identifier), api);
                if let Some(def) = api.find_func(a) {
                    return Ok(Target::Func(FuncTarget::Rna { this, def }));
                }
                if let Some(prop) = api.find_prop(a) {
                    return Ok(match prop.kind {
                        PropKind::Collection { .. } => Target::Coll(CollPlace {
                            ptr: this,
                            prop,
                            range: None,
                        }),
                        _ => Target::Prop(PropPlace {
                            ptr: this,
                            prop,
                            elem: Elem::Whole,
                        }),
                    });
                }
            }
            Err(attr_error(format!(
                "bpy_prop_collection: attribute \"{a}\" not found"
            )))
        }
        PathSeg::Call(_) => Err(type_error("'bpy_prop_collection' object is not callable")),
        _ => Err(not_impl("this path segment")),
    }
}

fn key_not_found(host: &dyn HostRead, c: &CollPlace, k: &str) -> FrapError {
    let names: Vec<SmolStr> = with_coll(host, &c.ptr, c.prop, &mut |v| {
        Ok((0..v.len()).filter_map(|i| v.key_of(i)).collect())
    })
    .unwrap_or_default();
    let sugg = frap_op::suggest(k, names.iter().map(|s| s.as_str()));
    let mut e = FrapError::new(
        PyExc::KeyError,
        RNA_KEY_NOT_FOUND,
        format!("'bpy_prop_collection[key]: key \"{k}\" not found'"),
    )
    .with_detail(
        "did_you_mean",
        sugg.into_iter().map(J::from).collect::<Vec<_>>(),
    );
    if names.len() <= 100 {
        e = e.with_detail(
            "valid",
            names
                .iter()
                .map(|s| J::from(s.as_str()))
                .collect::<Vec<_>>(),
        );
    }
    e
}

/// `bpy.data.<coll>['Name', lib]`: `lib` is a library filepath or `None` (local) (§5.2).
fn step_key_lib(
    host: &dyn HostRead,
    c: CollPlace,
    name: &str,
    lib: Option<&str>,
) -> Result<Target, FrapError> {
    let main = host.main();
    if !(c.ptr.root == Root::Data && c.ptr.path.is_empty()) {
        return Err(type_error(
            "bpy_prop_collection[key]: invalid key, must be a string or an int, not tuple",
        ));
    }
    if frap_scene::Main::code_of(c.prop.identifier).is_none() {
        return Err(not_bound(&frap_spec_rna::BLEND_DATA, c.prop));
    }
    // WP 1.10's lookup (first library with that filepath, local before linked); Python's
    // `str(KeyError)` quotes the message.
    match main.find_key_lib(c.prop.identifier, name, lib) {
        Ok(Some(id)) => Ok(Target::Struct(RnaPtr::id(id, id_srna(host, id)?))),
        Ok(None) => Err(not_bound(&frap_spec_rna::BLEND_DATA, c.prop)),
        Err(e) => {
            let mut e = FrapError::from(e);
            e.message = frap_rna::ptr::py_str_repr(&e.message);
            Err(e)
        }
    }
}

fn step_list(what: &'static str, items: Vec<Target>, seg: &PathSeg) -> Result<Target, FrapError> {
    match seg {
        PathSeg::Index(i) => {
            let j = norm_index(*i, items.len())
                .ok_or_else(|| index_error(format!("{what} index out of range")))?;
            Ok(items.into_iter().nth(j).expect("invariant: index checked"))
        }
        PathSeg::Slice(a, b) => {
            let (s, e) = norm_slice(*a, *b, items.len());
            Ok(Target::List {
                what,
                items: items.into_iter().skip(s).take(e - s).collect(),
            })
        }
        PathSeg::Key(_) | PathSeg::KeyLib(..) => Err(type_error(format!(
            "{what} indices must be integers or slices, not str"
        ))),
        _ => step_python(Target::List { what, items }, seg),
    }
}

// ------------------------------------------------------------------ reading

/// What reading a property gives.
pub enum Read {
    /// A plain value.
    Value(Value),
    /// A pointer, struct, collection or list, as a target.
    Target(Target),
}

/// Reads a property (whole), dereferencing context members and BlendData properties.
pub fn read_prop(host: &dyn HostRead, p: &PropPlace) -> Result<Read, FrapError> {
    match p.ptr.root {
        Root::Context if p.ptr.path.is_empty() => {
            return Ok(match context::member(host, p.prop)? {
                Member::Target(t) => Read::Target(t),
                Member::Value(v) => Read::Value(v),
            });
        }
        Root::Data if p.ptr.path.is_empty() => {
            let main = host.main();
            return Ok(Read::Value(match p.prop.identifier {
                "filepath" => Value::Str(main.filepath.as_deref().unwrap_or("").into()),
                "is_dirty" => Value::Bool(main.is_dirty),
                "is_saved" => Value::Bool(main.filepath.is_some()),
                _ => return Err(not_bound(&frap_spec_rna::BLEND_DATA, p.prop)),
            }));
        }
        _ => {}
    }
    if is_api_ptr(&p.ptr) {
        // A property of a collection's API struct (`view_layer.objects.active`): no struct
        // exists in memory, the property is served by a hook.
        let f = access::call_get_hook(host, &p.ptr, p.prop)?;
        return field_read(host, &p.ptr, p.prop, f);
    }
    with_struct(host, &p.ptr, &mut |s| {
        let f = get_field(host, s, &p.ptr, p.prop)?;
        field_read(host, &p.ptr, p.prop, f)
    })
}

/// True when `ptr` points at a collection's API struct (`….vertex_groups` as
/// `VertexGroups`): such structs have no memory of their own, their members are hooks and
/// RNA functions.
pub fn is_api_ptr(ptr: &RnaPtr) -> bool {
    static API: std::sync::OnceLock<std::collections::BTreeSet<&'static str>> =
        std::sync::OnceLock::new();
    let api = API.get_or_init(|| {
        frap_spec_rna::ALL_STRUCTS
            .iter()
            .flat_map(|s| s.props.iter())
            .filter_map(|p| match p.kind {
                PropKind::Collection { api: Some(a), .. } => Some(a),
                _ => None,
            })
            .collect()
    });
    matches!(ptr.path.last(), Some(Seg::Prop(_))) && api.contains(ptr.srna.identifier)
}

fn field_read(
    host: &dyn HostRead,
    owner: &RnaPtr,
    prop: &'static PropDef,
    f: Field<'_>,
) -> Result<Read, FrapError> {
    Ok(match f {
        Field::Value(Value::Id(Some(id))) | Field::Id(Some(id)) => {
            Read::Target(Target::Struct(RnaPtr::id(id, id_srna(host, id)?)))
        }
        Field::Value(Value::Id(None)) | Field::Id(None) => Read::Target(Target::None),
        Field::Value(Value::Ptr(p)) => Read::Target(Target::Struct(*p)),
        Field::Ptr(p) => Read::Target(Target::Struct(p)),
        Field::Value(Value::Tuple(items))
            if matches!(
                prop.kind,
                PropKind::Collection { .. } | PropKind::Pointer { .. }
            ) =>
        {
            Read::Target(Target::List {
                what: "tuple",
                items: items
                    .into_iter()
                    .map(|v| value_target(host, v))
                    .collect::<Result<_, _>>()?,
            })
        }
        Field::Value(v) => Read::Value(v),
        Field::Struct(s) => Read::Target(Target::Struct(
            owner.child(Seg::Prop(prop.identifier), s.srna()),
        )),
        Field::Embedded(slot, s) => match owner.root {
            Root::Id(id) if owner.path.is_empty() => Read::Target(Target::Struct(RnaPtr::new(
                Root::Embedded { owner: id, slot },
                s.srna(),
            ))),
            _ => return Err(stale("an embedded ID below a struct")),
        },
        Field::Coll(_) => Read::Target(Target::Coll(CollPlace {
            ptr: owner.clone(),
            prop,
            range: None,
        })),
        Field::NeedsHook => {
            let f = access::call_get_hook(host, owner, prop)?;
            return field_read(host, owner, prop, f);
        }
        _ => return Err(not_impl("this kind of field")),
    })
}

/// Reads a property place as a value (applying the element selector).
pub fn read_value(host: &dyn HostRead, p: &PropPlace) -> Result<Value, FrapError> {
    let whole = match read_prop(host, p)? {
        Read::Value(v) => v,
        Read::Target(_) if p.elem == Elem::Whole => {
            return Err(stale("read_value on a pointer"));
        }
        Read::Target(_) => return Err(stale("element of a pointer")),
    };
    element(p.prop, whole, p.elem)
}

fn float_at(v: &Value, i: usize) -> Value {
    match v {
        Value::FloatArray { data, .. } => Value::Float(data[i]),
        Value::IntArray(a) => Value::Int(a[i]),
        Value::BoolArray(a) => Value::Bool(a[i]),
        _ => Value::None,
    }
}

fn array_len_of(v: &Value) -> usize {
    match v {
        Value::FloatArray { data, .. } => data.len(),
        Value::IntArray(a) => a.len(),
        Value::BoolArray(a) => a.len(),
        _ => 0,
    }
}

/// Applies an element selector to a whole array value (Blender's read messages).
pub fn element(prop: &PropDef, whole: Value, elem: Elem) -> Result<Value, FrapError> {
    let kind = ArrayKind::of(prop);
    let len = array_len_of(&whole);
    match elem {
        Elem::Whole => Ok(whole),
        Elem::Index(i) => {
            if let ArrayKind::Matrix(r, _) = kind {
                // In-range rows resolve to `Elem::Row`.
                return Err(kind.read_index_error(i, r));
            }
            let j = norm_index(i, len).ok_or_else(|| kind.read_index_error(i, len))?;
            Ok(float_at(&whole, j))
        }
        Elem::Component(_, j) => Ok(float_at(&whole, j)),
        Elem::Slice(a, b) if let ArrayKind::Matrix(r, _) = kind => {
            // `mathutils.Matrix` slices rows: a tuple of row vectors (verified on 5.2.4).
            let (s, e) = norm_slice(a, b, r);
            (s..e)
                .map(|row| element(prop, whole.clone(), Elem::Row(row)))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Tuple)
        }
        Elem::Slice(a, b) => {
            let (s, e) = norm_slice(a, b, len);
            Ok(Value::Tuple((s..e).map(|j| float_at(&whole, j)).collect()))
        }
        Elem::Row(r) => {
            let ArrayKind::Matrix(_, c) = kind else {
                return Err(stale("row of a non-matrix"));
            };
            let Value::FloatArray { data, .. } = &whole else {
                return Err(stale("matrix is not a float array"));
            };
            Ok(Value::FloatArray {
                data: data[r * c..(r + 1) * c].iter().copied().collect(),
                shape: ArrayShape::Fixed(c as u16),
                tag: MathTag::Vector,
            })
        }
        Elem::RowCol(r, j) => {
            let ArrayKind::Matrix(_, c) = kind else {
                return Err(stale("row of a non-matrix"));
            };
            let k = norm_index(j, c).ok_or_else(|| ArrayKind::Vector(c).read_index_error(j, c))?;
            Ok(float_at(&whole, r * c + k))
        }
    }
}

// ------------------------------------------------------------------ repr

fn coll_repr(c: &CollPlace, names: &dyn IdNames) -> String {
    let base = match (c.ptr.root, c.ptr.path.is_empty()) {
        (Root::Data, true) => format!("bpy.data.{}", c.prop.identifier),
        (Root::Context, true) => format!("bpy.context.{}", c.prop.identifier),
        _ => format!("{}.{}", c.ptr.repr(names), c.prop.identifier),
    };
    match c.range {
        Some((a, b)) => format!("{base}[{a}:{b}]"),
        None => base,
    }
}

fn prop_repr(p: &PropPlace, names: &dyn IdNames) -> String {
    let base = match (p.ptr.root, p.ptr.path.is_empty()) {
        (Root::Data, true) => format!("bpy.data.{}", p.prop.identifier),
        (Root::Context, true) => format!("bpy.context.{}", p.prop.identifier),
        _ => format!("{}.{}", p.ptr.repr(names), p.prop.identifier),
    };
    let fmt_opt = |x: Option<i64>| x.map(|v| v.to_string()).unwrap_or_default();
    match p.elem {
        Elem::Whole => base,
        Elem::Index(i) => format!("{base}[{i}]"),
        Elem::Component(c, _) => format!("{base}.{c}"),
        Elem::Slice(a, b) => format!("{base}[{}:{}]", fmt_opt(a), fmt_opt(b)),
        Elem::Row(r) => format!("{base}[{r}]"),
        Elem::RowCol(r, c) => format!("{base}[{r}][{c}]"),
    }
}

/// The canonical repr of a target (`path` of a response).
pub fn repr(t: &Target, names: &dyn IdNames) -> Option<String> {
    Some(match t {
        Target::Data => "bpy.data".into(),
        Target::Context => "bpy.context".into(),
        Target::Struct(p) => p.repr(names),
        Target::Prop(p) => prop_repr(p, names),
        Target::Coll(c) => coll_repr(c, names),
        Target::Func(f) => f.repr(names),
        Target::None | Target::List { .. } | Target::Value(_) => return None,
    })
}

// ------------------------------------------------------------------ encoding (§13.3)

/// JSON value and RNA type tag of a target, as a `get` response shows them.
pub fn encode(host: &dyn HostRead, t: &Target, caps: Caps) -> Result<(J, Option<J>), FrapError> {
    let main = host.main();
    Ok(match t {
        Target::Data => (json!({"$ptr": "bpy.data", "type": "BlendData"}), None),
        Target::Context => (json!({"$ptr": "bpy.context", "type": "Context"}), None),
        Target::None => (J::Null, None),
        Target::Struct(p) => (struct_json(p, main), None),
        Target::Value(v) => (value_json(v, main), None),
        Target::List { items, .. } => (
            J::Array(
                items
                    .iter()
                    .map(|i| encode(host, i, caps).map(|(j, _)| j))
                    .collect::<Result<_, _>>()?,
            ),
            None,
        ),
        Target::Func(f) => (json!({"$func": f.repr(main)}), None),
        Target::Coll(c) => (coll_json(host, c, caps)?, Some(rna_tag(c.prop, None))),
        Target::Prop(p) => match read_prop(host, p)? {
            Read::Target(inner) if p.elem == Elem::Whole => {
                let (j, _) = encode(host, &inner, caps)?;
                (j, Some(rna_tag(p.prop, None)))
            }
            Read::Target(_) => return Err(stale("element of a pointer")),
            Read::Value(whole) => {
                let v = element(p.prop, whole, p.elem)?;
                let rna = if p.elem == Elem::Whole {
                    rna_tag(p.prop, Some(&v))
                } else {
                    json!({"type": p.prop.type_str(), "subtype": p.prop.subtype.as_str()})
                };
                (value_json(&v, main), Some(rna))
            }
        },
    })
}

/// `rna` tag (§13.2): `type`, `subtype`, `array`, `math` and, for Eulers, `order`.
pub fn rna_tag(prop: &PropDef, v: Option<&Value>) -> J {
    let mut j = match v {
        Some(v) => rna_json(prop, v),
        None => json!({"type": prop.type_str(), "subtype": prop.subtype.as_str()}),
    };
    if let (
        Some(Value::FloatArray {
            tag: MathTag::Euler(o),
            ..
        }),
        J::Object(m),
    ) = (v, &mut j)
    {
        m.insert("order".into(), J::from(o.as_str()));
    }
    j
}

fn struct_json(p: &RnaPtr, names: &dyn IdNames) -> J {
    if matches!(p.root, Root::Id(_)) && p.path.is_empty() {
        json!({"$ref": p.repr(names)})
    } else {
        json!({"$ptr": p.repr(names), "type": p.srna.identifier})
    }
}

/// `$coll` (named or nameless), or the items of a slice (§13.3).
fn coll_json(host: &dyn HostRead, c: &CollPlace, caps: Caps) -> Result<J, FrapError> {
    let main = host.main();
    if let Some((a, b)) = c.range {
        // Attribute data slices read in bulk as their value field.
        let item_type = with_coll(host, &c.ptr, c.prop, &mut |v| Ok(v.item_type()))?;
        if item_type.identifier.ends_with("AttributeValue")
            && let Some(prop) = ["vector", "value", "color"]
                .iter()
                .find_map(|n| item_type.find_prop(n))
        {
            let bulk = with_coll(host, &c.ptr, c.prop, &mut |v| {
                v.foreach_get(prop, a..b).map_err(access::rna_err)
            })?;
            return Ok(bulk_json(&bulk, prop, a, caps.max_array));
        }
        let n = (b - a).min(caps.max_items);
        let mut items = Vec::with_capacity(n);
        for i in a..a + n {
            let t = item_target(host, c, i)?;
            items.push(encode(host, &t, caps)?.0);
        }
        return Ok(J::Array(items));
    }
    with_coll(host, &c.ptr, c.prop, &mut |v| {
        let len = v.len();
        let mut m = serde_json::Map::new();
        m.insert("$coll".into(), J::from(coll_repr(c, main)));
        m.insert("item_type".into(), J::from(v.item_type().identifier));
        m.insert("len".into(), J::from(len));
        if v.named() {
            let n = len.min(caps.max_items);
            m.insert(
                "items".into(),
                J::Array(
                    (0..n)
                        .map(|i| v.key_of(i).map_or(J::Null, |k| J::from(k.as_str())))
                        .collect(),
                ),
            );
            if n < len {
                m.insert("truncated".into(), J::from(true));
            }
        }
        Ok(J::Object(m))
    })
}

/// `{"$array": [...], "shape", "dtype", "len", "range", "truncated"}`: flat row-major, capped
/// at `max_array` scalars (whole items only).
pub fn bulk_json(bulk: &Bulk, prop: &PropDef, start: usize, max_array: usize) -> J {
    // A variable-length array (`MeshPolygon.vertices`, dumped as `[3]`) answers its
    // concatenated values; when they do not split into whole items, it is answered flat
    // instead of being cut to `[n, 3]` (WP 1.19: a plane's quad read as one triangle).
    let flat = !bulk.len().is_multiple_of(prop.array_len().max(1));
    let k = if flat { 1 } else { prop.array_len().max(1) };
    let total_items = bulk.len() / k;
    // At least one whole item, so a page always advances (even with `--max-array` below the
    // item width).
    let kept = total_items.min((max_array / k).max(1));
    let scalars = kept * k;
    let (data, dtype): (Vec<J>, &str) = match bulk {
        Bulk::F32(v) => (
            v[..scalars].iter().map(|x| f32_json(*x)).collect(),
            "float32",
        ),
        Bulk::I32(v) => (v[..scalars].iter().map(|x| J::from(*x)).collect(), "int32"),
        Bulk::Bool(v) => (v[..scalars].iter().map(|x| J::from(*x)).collect(), "bool"),
        _ => (Vec::new(), "unknown"),
    };
    let shape = if flat || matches!(prop.array, ArrayShape::Scalar) {
        json!([kept])
    } else {
        json!([kept, k])
    };
    json!({
        "$array": data, "shape": shape, "dtype": dtype, "len": kept,
        "range": [start, start + kept], "truncated": kept < total_items,
    })
}

/// The ID repr of an `IdAny` (`bpy.data.objects['Cube']`).
pub fn id_repr(names: &dyn IdNames, id: IdAny) -> String {
    match names.id_name(id) {
        Some((n, lib)) => frap_rna::ptr::id_repr(id.code, &n, lib.as_deref()),
        None => frap_rna::ptr::id_repr(id.code, "<removed>", None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matrix() -> Value {
        Value::FloatArray {
            data: (1..=16).map(|x| x as f32).collect(),
            shape: ArrayShape::Matrix(4, 4),
            tag: MathTag::Matrix,
        }
    }

    fn rows(v: &Value) -> Vec<Vec<f32>> {
        let Value::Tuple(items) = v else {
            panic!("not a tuple: {v:?}")
        };
        items
            .iter()
            .map(|r| match r {
                Value::FloatArray {
                    data,
                    tag: MathTag::Vector,
                    ..
                } => data.to_vec(),
                other => panic!("not a row vector: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn matrix_slices_are_tuples_of_row_vectors() {
        let prop = frap_spec_rna::VIEW3_D_CURSOR
            .find_prop("matrix")
            .expect("cursor matrix");
        let m = matrix();
        let v = element(prop, m.clone(), Elem::Slice(Some(0), Some(2))).unwrap();
        assert_eq!(
            rows(&v),
            vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]]
        );
        let v = element(prop, m.clone(), Elem::Slice(Some(-1), None)).unwrap();
        assert_eq!(rows(&v), vec![vec![13.0, 14.0, 15.0, 16.0]]);
        let v = element(prop, m.clone(), Elem::Slice(Some(5), Some(9))).unwrap();
        assert!(rows(&v).is_empty());
        // An out-of-range row index is Blender's read error.
        let e = element(prop, m.clone(), Elem::Index(4)).unwrap_err();
        assert_eq!(e.message, "matrix[attribute]: array index out of range");
        // A free-standing matrix value (a call result) indexes and slices the same way.
        let Target::Value(row) = step_value(m.clone(), &PathSeg::Index(1)).unwrap() else {
            panic!("row")
        };
        let Target::Value(Value::Float(y)) = step_value(row, &PathSeg::Attr("y".into())).unwrap()
        else {
            panic!("component")
        };
        assert_eq!(y, 6.0);
        let Target::Value(v) = step_value(m, &PathSeg::Slice(Some(2), None)).unwrap() else {
            panic!("slice")
        };
        assert_eq!(
            rows(&v),
            vec![vec![9.0, 10.0, 11.0, 12.0], vec![13.0, 14.0, 15.0, 16.0]]
        );
    }

    #[test]
    fn bulk_pages_hold_at_least_one_item() {
        let prop = frap_spec_rna::MESH_VERTEX.find_prop("co").expect("co");
        let j = bulk_json(&Bulk::F32(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), prop, 0, 2);
        assert_eq!(j["len"], 1);
        assert_eq!(j["$array"], json!([1, 2, 3]));
        assert_eq!(j["truncated"], true);
        assert_eq!(j["range"], json!([0, 1]));
    }

    #[test]
    fn variable_length_bulk_is_flat() {
        // A plane's one quad: `polygons.foreach_get('vertices')` gives 4 values, not `[1, 3]`.
        let prop = frap_spec_rna::MESH_POLYGON
            .find_prop("vertices")
            .expect("vertices");
        let j = bulk_json(&Bulk::I32(vec![0, 1, 3, 2]), prop, 0, 1000);
        assert_eq!(j["$array"], json!([0, 1, 3, 2]));
        assert_eq!(j["shape"], json!([4]));
    }
}
