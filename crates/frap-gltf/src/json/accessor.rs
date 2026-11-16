// SPDX-License-Identifier: GPL-3.0-or-later
//! `accessors[]`: typed views into buffer views (data access is in `crate::accessor`).

use serde::{Deserialize, Serialize};

use super::{Extensions, Extras, Index, gl_enum, str_enum};

gl_enum! {
    /// Component type of an accessor (5124 INT is not allowed by glTF).
    ComponentType {
        /// 5120 BYTE (i8)
        Byte = 5120,
        /// 5121 UNSIGNED_BYTE (u8)
        UnsignedByte = 5121,
        /// 5122 SHORT (i16)
        Short = 5122,
        /// 5123 UNSIGNED_SHORT (u16)
        UnsignedShort = 5123,
        /// 5125 UNSIGNED_INT (u32)
        UnsignedInt = 5125,
        /// 5126 FLOAT (f32)
        Float = 5126,
    }
}

impl ComponentType {
    /// Size of one component in bytes.
    pub fn size(self) -> usize {
        match self {
            ComponentType::Byte | ComponentType::UnsignedByte => 1,
            ComponentType::Short | ComponentType::UnsignedShort => 2,
            ComponentType::UnsignedInt | ComponentType::Float => 4,
        }
    }

    /// Whether `normalized` is allowed (every integer type except UNSIGNED_INT).
    pub fn can_normalize(self) -> bool {
        !matches!(self, ComponentType::UnsignedInt | ComponentType::Float)
    }

    /// Whether the type is an unsigned integer type (the index types).
    pub fn is_unsigned_int(self) -> bool {
        matches!(
            self,
            ComponentType::UnsignedByte | ComponentType::UnsignedShort | ComponentType::UnsignedInt
        )
    }
}

str_enum! {
    /// Element shape of an accessor.
    AccessorType {
        /// 1 component
        Scalar = "SCALAR",
        /// 2 components
        Vec2 = "VEC2",
        /// 3 components
        Vec3 = "VEC3",
        /// 4 components
        Vec4 = "VEC4",
        /// 2×2 column-major
        Mat2 = "MAT2",
        /// 3×3 column-major
        Mat3 = "MAT3",
        /// 4×4 column-major
        Mat4 = "MAT4",
    }
}

impl AccessorType {
    /// Components per element.
    pub fn components(self) -> usize {
        match self {
            AccessorType::Scalar => 1,
            AccessorType::Vec2 => 2,
            AccessorType::Vec3 => 3,
            AccessorType::Vec4 | AccessorType::Mat2 => 4,
            AccessorType::Mat3 => 9,
            AccessorType::Mat4 => 16,
        }
    }

    /// `(columns, rows)`; vectors and scalars are one column.
    pub fn columns_rows(self) -> (usize, usize) {
        match self {
            AccessorType::Mat2 => (2, 2),
            AccessorType::Mat3 => (3, 3),
            AccessorType::Mat4 => (4, 4),
            t => (1, t.components()),
        }
    }
}

/// A typed view into a buffer view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Accessor {
    /// Buffer view (absent: zeros, possibly patched by `sparse`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer_view: Option<Index>,
    /// Offset in the buffer view (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// Component type.
    pub component_type: ComponentType,
    /// Number of elements.
    pub count: u64,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Per-component maximum (raw component values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<Vec<f64>>,
    /// Per-component minimum (raw component values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<Vec<f64>>,
    /// Name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Integers map to `[0,1]` / `[-1,1]` (default false).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized: Option<bool>,
    /// Sparse substitution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sparse: Option<Sparse>,
    /// Element shape.
    #[serde(rename = "type")]
    pub type_: AccessorType,
}

impl Accessor {
    /// A tightly packed accessor with no min/max.
    pub fn new(
        buffer_view: Option<Index>,
        component_type: ComponentType,
        type_: AccessorType,
        count: u64,
    ) -> Self {
        Accessor {
            buffer_view,
            byte_offset: None,
            component_type,
            normalized: None,
            count,
            type_,
            max: None,
            min: None,
            sparse: None,
            name: None,
            extensions: Extensions::default(),
            extras: None,
        }
    }

    /// Effective offset in the buffer view.
    pub fn byte_offset_or_default(&self) -> u64 {
        self.byte_offset.unwrap_or(0)
    }

    /// Effective `normalized`.
    pub fn normalized_or_default(&self) -> bool {
        self.normalized.unwrap_or(false)
    }
}

/// Sparse storage: `count` (index, value) pairs substituted into the base data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sparse {
    /// Number of substituted elements.
    pub count: u64,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
    /// Strictly increasing element indices.
    pub indices: SparseIndices,
    /// Substituted values (same component type and shape as the accessor).
    pub values: SparseValues,
}

/// Indices of a sparse accessor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SparseIndices {
    /// Buffer view (no stride, no target).
    pub buffer_view: Index,
    /// Offset in the buffer view (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// UNSIGNED_BYTE, UNSIGNED_SHORT or UNSIGNED_INT.
    pub component_type: ComponentType,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
}

/// Values of a sparse accessor.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SparseValues {
    /// Buffer view (no stride, no target).
    pub buffer_view: Index,
    /// Offset in the buffer view (default 0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
    /// Extensions.
    #[serde(default, skip_serializing_if = "Extensions::is_empty")]
    pub extensions: Extensions,
    /// Application data.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Extras>,
}
