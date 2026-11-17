// SPDX-License-Identifier: GPL-3.0-or-later
//! §9.1 Metadata (`frap-rna/src/def.rs`, CONTRACT).
//!
//! Types the contract names but does not define (`Defaults`, `EnumDefault`, `ArrayShape`,
//! `PropFlags`, `FuncFlags`, `FloatStorage`, `EnumItem`) are defined here with the minimum the
//! real dumps require (RFC 0001 F-DEF-*). `Subtype` and `Unit` are generated from the dump
//! (`rna/src/generated/enums.rs`), because Blender exposes the complete lists.

pub use crate::generated::enums::{Subtype, Unit};

#[derive(Debug)]
pub struct StructDef {
    pub identifier: &'static str,
    pub base: Option<&'static StructDef>,
    pub name: &'static str,
    pub description: &'static str,
    pub props: &'static [PropDef],
    pub funcs: &'static [FuncDef],
    pub name_property: Option<&'static str>,
    pub is_id: bool,
}

impl StructDef {
    /// Lookup walks the base chain (props are "own props only").
    pub fn find_prop(&'static self, identifier: &str) -> Option<&'static PropDef> {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(d) = s {
            if let Some(p) = d.props.iter().find(|p| p.identifier == identifier) {
                return Some(p);
            }
            s = d.base;
        }
        None
    }
    pub fn find_func(&'static self, identifier: &str) -> Option<&'static FuncDef> {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(d) = s {
            if let Some(f) = d.funcs.iter().find(|f| f.identifier == identifier) {
                return Some(f);
            }
            s = d.base;
        }
        None
    }
    pub fn is_a(&'static self, identifier: &str) -> bool {
        let mut s: Option<&'static StructDef> = Some(self);
        while let Some(d) = s {
            if d.identifier == identifier {
                return true;
            }
            s = d.base;
        }
        false
    }
}

#[derive(Debug)]
pub struct PropDef {
    pub identifier: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub kind: PropKind,
    pub subtype: Subtype,
    pub unit: Unit,
    pub array: ArrayShape,
    pub flags: PropFlags,
    /// RFC 0001 F-DEF-3: 5.2 exposes deprecation (`is_deprecated`, note, version, removal) and
    /// uses it for `Material.use_nodes`; frap must raise Blender's DeprecationWarning report.
    pub deprecated: Option<&'static Deprecation>,
}

#[derive(Debug)]
pub struct Deprecation {
    pub note: &'static str,
    pub version: [u16; 3],
    pub removal_version: [u16; 3],
}

#[derive(Debug)]
pub enum Defaults<T: 'static> {
    Scalar(T),
    Array(&'static [T]),
}

#[derive(Debug)]
pub enum EnumDefault {
    Single(&'static str),
    Flags(&'static [&'static str]),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ArrayShape {
    Scalar,
    Fixed(u16),
    Matrix(u16, u16),
    Dynamic,
}

impl ArrayShape {
    /// Number of scalars (None for scalars and dynamic arrays).
    pub const fn flat_len(self) -> Option<usize> {
        match self {
            ArrayShape::Scalar => None,
            ArrayShape::Fixed(n) => Some(n as usize),
            ArrayShape::Matrix(r, c) => Some(r as usize * c as usize),
            ArrayShape::Dynamic => None,
        }
    }
}

/// Not in the contract: Blender does not expose storage width through bpy, so this cannot come
/// from the dump (RFC 0001 F-DEF-2). The probe codegen always emits `F32`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FloatStorage {
    F32,
    F64,
}

#[derive(Debug)]
pub enum PropKind {
    Bool {
        default: Defaults<bool>,
    },
    Int {
        default: Defaults<i64>,
        hard: (i64, i64),
        soft: (i64, i64),
    },
    Float {
        default: Defaults<f64>,
        hard: (f64, f64),
        soft: (f64, f64),
        precision: u8,
        storage: FloatStorage,
    },
    String {
        default: &'static str,
        max_bytes: u32,
    },
    Enum {
        items: &'static [EnumItem],
        default: EnumDefault,
        dynamic: Option<&'static str>,
    },
    Pointer {
        fixed_type: &'static str,
        is_id: bool,
    },
    Collection {
        item_type: &'static str,
        api: Option<&'static str>,
    },
}

#[derive(Debug)]
pub struct EnumItem {
    pub identifier: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub value: i32,
}

#[derive(Debug)]
pub struct FuncDef {
    pub identifier: &'static str,
    pub description: &'static str,
    pub params: &'static [PropDef],
    pub returns: &'static [PropDef],
    /// Not dumpable: RNA functions carry no keyword-only marker (RFC 0001 F-DEF-4); only the
    /// Python-only `bpy_struct.keyframe_insert/delete` have one (overlay `python_only.toml`).
    pub keyword_only_from: Option<u8>,
    pub flags: FuncFlags,
}

bitflags::bitflags! {
    /// Contract set (§9.1) plus what the dump exposes and frap needs (RFC 0001 F-DEF-1).
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct PropFlags: u32 {
        const READONLY = 1 << 0;
        const HIDDEN = 1 << 1;
        const SKIP_SAVE = 1 << 2;
        const ENUM_FLAG = 1 << 3;
        const REQUIRED = 1 << 4;
        const OUTPUT = 1 << 5;
        const ANIMATABLE = 1 << 6;
        const PYTHON_ONLY = 1 << 7;
        const DYNAMIC_ENUM = 1 << 8;
        const NEVER_NULL = 1 << 9;
        // ---- additions (RFC 0001 F-DEF-1)
        /// FuncDef params: `is_argument_optional` (keyword may be omitted).
        const ARGUMENT_OPTIONAL = 1 << 10;
        /// `is_deprecated` (see `PropDef::deprecated`).
        const DEPRECATED = 1 << 11;
        /// String paths: `//` allowed (`is_path_supports_blend_relative`) — render/export filepath.
        const PATH_SUPPORTS_BLEND_RELATIVE = 1 << 12;
        /// `bpy.context` member that is not an RNA property of `Context` (object, selected_objects, …):
        /// write error wording differs (`Context property "object" is read-only`).
        const CONTEXT_MEMBER = 1 << 13;
        /// `is_skip_preset`; together with SKIP_SAVE drives "remember last value" (redo) semantics.
        const SKIP_PRESET = 1 << 14;
    }
}

bitflags::bitflags! {
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct FuncFlags: u8 {
        const NO_SELF = 1 << 0;
        const USE_SELF_ID = 1 << 1;
        /// Not dumpable through bpy (RFC 0001 F-DEF-4).
        const USE_CONTEXT = 1 << 2;
        const PYTHON_ONLY = 1 << 3;
        /// Dumped `use_self_type` (class-level functions); not in the contract (RFC 0001 F-DEF-4).
        const USE_SELF_TYPE = 1 << 4;
    }
}
