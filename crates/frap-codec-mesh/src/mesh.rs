// SPDX-License-Identifier: GPL-3.0-or-later
//! The mesh data a codec encodes and decodes (CONTRACT, M7 W0): triangle (or point) indices and
//! typed per-vertex attributes, in the layout of a glTF primitive's accessors.

/// Typed attribute values, flat (`components` values per vertex).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AttrData {
    /// FLOAT
    F32(Vec<f32>),
    /// BYTE
    I8(Vec<i8>),
    /// UNSIGNED_BYTE
    U8(Vec<u8>),
    /// SHORT
    I16(Vec<i16>),
    /// UNSIGNED_SHORT
    U16(Vec<u16>),
    /// UNSIGNED_INT
    U32(Vec<u32>),
}

impl AttrData {
    /// Number of scalar values.
    pub fn len(&self) -> usize {
        match self {
            AttrData::F32(v) => v.len(),
            AttrData::I8(v) => v.len(),
            AttrData::U8(v) => v.len(),
            AttrData::I16(v) => v.len(),
            AttrData::U16(v) => v.len(),
            AttrData::U32(v) => v.len(),
        }
    }

    /// No values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What an attribute means to the codec (Draco's `GeometryAttribute::Type`; meshopt ignores it).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttributeKind {
    /// `POSITION`
    Position,
    /// `NORMAL`
    Normal,
    /// `COLOR_n`
    Color,
    /// `TEXCOORD_n`
    TexCoord,
    /// Anything else (`TANGENT`, `JOINTS_n`, `WEIGHTS_n`, `_CUSTOM`, morph targets).
    Generic,
}

/// One per-vertex attribute.
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    /// Its meaning.
    pub kind: AttributeKind,
    /// Values per vertex (1–4).
    pub components: u8,
    /// Integer values are normalized (glTF `normalized`).
    pub normalized: bool,
    /// The values, `components * vertex_count` of them.
    pub data: AttrData,
}

impl Attribute {
    /// Number of vertices this attribute covers.
    pub fn vertex_count(&self) -> usize {
        self.data.len() / usize::from(self.components.max(1))
    }
}

/// An indexed mesh (TRIANGLES) or a point cloud (`indices` empty).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshData {
    /// Triangle corner indices (three per triangle); empty for a point cloud.
    pub indices: Vec<u32>,
    /// Attributes, every one with the same vertex count.
    pub attributes: Vec<Attribute>,
}

impl MeshData {
    /// Number of vertices (of the first attribute; 0 without attributes).
    pub fn vertex_count(&self) -> usize {
        self.attributes.first().map_or(0, Attribute::vertex_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        let m = MeshData {
            indices: vec![0, 1, 2],
            attributes: vec![Attribute {
                kind: AttributeKind::Position,
                components: 3,
                normalized: false,
                data: AttrData::F32(vec![0.0; 9]),
            }],
        };
        assert_eq!(m.vertex_count(), 3);
        assert!(!m.attributes[0].data.is_empty());
        assert_eq!(MeshData::default().vertex_count(), 0);
    }
}
