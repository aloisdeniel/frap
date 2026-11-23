// SPDX-License-Identifier: GPL-3.0-or-later
//! Custom attribute writer (`export_attributes`, WP 8.12): every mesh attribute whose name
//! starts with `_` or `KHR_` becomes a glTF attribute named after it in upper case, written as
//! Blender stores it (no Y-up conversion, no V flip).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`define_attributes`: `KeepAttribute`, EDGE attributes and types without a glTF component
//! type skipped, the `Attribute collision name` warning; `__get_layer_attribute`: CORNER as is,
//! POINT through each corner's vertex, FACE spread over the face's corners;
//! `create_dots_data_structure`: the fields join the dots before NORMAL, and only POINT ones
//! reach the loose-edge and loose-point dots; `primitive_creation_edges_and_points`),
//! blender/com/conversion.py (`get_component_type`, `get_data_type`, `get_numpy_type`: every
//! numeric type as `float32`, INT/INT8/BOOLEAN as FLOAT SCALAR, FLOAT4X4 as MAT4), blender/exp/
//! attribute_utils.py (`extract_attribute_data`: `color` of a BYTE_COLOR is linear) and
//! primitive_attributes.py (`__gather_attribute`: a BYTE_COLOR attribute, UNSIGNED_SHORT, is
//! clipped to [0, 1] and written as normalized `uint16(x * 65535 + 0.5)`), io_scene_gltf2 5.2.41,
//! Apache-2.0, The glTF-Blender-IO authors.

use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use smol_str::SmolStr;

use crate::export::ExportError;
use crate::export::mesh::convert::{byte_color_to_linear, key_word};
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter,
    PrimitiveDraft, SplitKey,
};

/// This writer's split-key column.
pub const NAME: &str = "custom_attributes";

/// Whether a glTF attribute of a draft is one of this writer's (`_…`, `KHR_…`). The add-on's
/// `primitive.attributes` dict starts with them (the dots' fields come first), so their
/// accessors are written before POSITION, NORMAL and TEXCOORD_n, and they come first among
/// a Draco primitive's attribute ids (verified on 5.2.4).
pub fn is_custom(semantic: &str) -> bool {
    semantic.starts_with('_') || semantic.starts_with("KHR_")
}

/// glTF shape of a kept attribute.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Shape {
    /// FLOAT SCALAR (FLOAT, INT, INT8, BOOLEAN)
    Scalar,
    /// FLOAT VEC2 (FLOAT2)
    Vec2,
    /// FLOAT VEC3 (FLOAT_VECTOR)
    Vec3,
    /// FLOAT VEC4 (FLOAT_COLOR, QUATERNION, FLOAT4)
    Vec4,
    /// UNSIGNED_SHORT VEC4, normalized (BYTE_COLOR)
    UShort4,
    /// FLOAT MAT4 (FLOAT4X4)
    Mat4,
}

impl Shape {
    /// `get_data_length`.
    fn len(self) -> usize {
        match self {
            Shape::Scalar => 1,
            Shape::Vec2 => 2,
            Shape::Vec3 => 3,
            Shape::Vec4 | Shape::UShort4 => 4,
            Shape::Mat4 => 16,
        }
    }
}

/// `get_component_type` / `get_data_type` of a Blender data type (`None`: not exportable, as
/// STRING, INT16_2D, INT32_2D).
fn shape(data: &AttrData) -> Option<Shape> {
    Some(match data {
        AttrData::Float(_) | AttrData::Int(_) | AttrData::Int8(_) | AttrData::Bool(_) => {
            Shape::Scalar
        }
        AttrData::Float2(_) => Shape::Vec2,
        AttrData::Float3(_) => Shape::Vec3,
        AttrData::Float4(_) | AttrData::FloatColor(_) | AttrData::Quaternion(_) => Shape::Vec4,
        AttrData::ByteColor(_) => Shape::UShort4,
        AttrData::Float4x4(_) => Shape::Mat4,
        _ => return None,
    })
}

/// One exported attribute.
struct Kept<'m> {
    attr: &'m Attribute,
    /// glTF name (the Blender name upper-cased).
    gltf: SmolStr,
    shape: Shape,
}

/// The attributes `define_attributes` keeps, in `mesh.attributes` order, and the collision
/// warnings.
fn plan<'m>(cx: &MeshExportCx, mesh: &'m Mesh) -> (Vec<Kept<'m>>, Vec<String>) {
    let mut kept: Vec<Kept> = Vec::new();
    let mut warnings = Vec::new();
    if !cx.settings.props.export_attributes.value {
        return (kept, warnings);
    }
    for attr in mesh.attributes().iter() {
        if attr.domain == Domain::Edge {
            continue;
        }
        let Some(shape) = shape(&attr.data) else {
            continue;
        };
        if !(attr.name.starts_with('_') || attr.name.starts_with("KHR_")) {
            continue;
        }
        // Python's `str.upper()` (full Unicode case mapping).
        let gltf = SmolStr::new(attr.name.to_uppercase());
        if kept.iter().any(|k| k.gltf == gltf) {
            warnings.push(format!(
                "Attribute collision name: {}, ignoring one of them",
                attr.name
            ));
            continue;
        }
        kept.push(Kept { attr, gltf, shape });
    }
    (kept, warnings)
}

/// The float32 components of element `i` of an attribute (`foreach_get` into `float32`).
fn element(data: &AttrData, i: usize, out: &mut Vec<f32>) {
    match data {
        AttrData::Float(v) => out.push(v[i]),
        AttrData::Int(v) => out.push(v[i] as f32),
        AttrData::Int8(v) => out.push(f32::from(v[i])),
        AttrData::Bool(v) => out.push(if v[i] { 1.0 } else { 0.0 }),
        AttrData::Float2(v) => out.extend_from_slice(&v[i]),
        AttrData::Float3(v) => out.extend_from_slice(&v[i]),
        AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            out.extend_from_slice(&v[i])
        }
        AttrData::ByteColor(v) => out.extend_from_slice(&byte_color_to_linear(v[i])),
        AttrData::Float4x4(v) => {
            for col in &v[i] {
                out.extend_from_slice(col);
            }
        }
        _ => {}
    }
}

/// Face of every corner.
fn corner_faces(mesh: &Mesh) -> Vec<u32> {
    let mut out = vec![0u32; mesh.corners_num()];
    for f in 0..mesh.faces_num() {
        for c in mesh.face(f) {
            out[c] = f as u32;
        }
    }
    out
}

/// The element of `attr` a corner reads (`None` for an EDGE attribute, never kept).
fn corner_element(attr: &Attribute, corner: usize, cv: &[i32], cf: &[u32]) -> Option<usize> {
    match attr.domain {
        Domain::Corner => Some(corner),
        Domain::Point => Some(cv[corner] as usize),
        Domain::Face => Some(cf[corner] as usize),
        _ => None,
    }
}

/// The dots fields: every kept attribute's components, per corner, in plan order (POINT ones
/// are a function of the vertex index, but their place in the record still orders the
/// deduplicated vertices).
fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    let (kept, warnings) = plan(cx, cx.mesh);
    for w in warnings {
        cx.warn(w);
    }
    if kept.is_empty() {
        return Ok(());
    }
    let stride: usize = kept.iter().map(|a| a.shape.len()).sum();
    let cv = cx.mesh.corner_verts();
    let cf = corner_faces(cx.mesh);
    let mut words = Vec::with_capacity(stride * cx.mesh.corners_num());
    let mut buf = Vec::with_capacity(16);
    for c in 0..cx.mesh.corners_num() {
        for a in &kept {
            buf.clear();
            if let Some(e) = corner_element(a.attr, c, cv, &cf) {
                element(&a.attr.data, e, &mut buf);
            }
            buf.resize(a.shape.len(), 0.0);
            words.extend(buf.iter().map(|&x| key_word(x)));
        }
    }
    k.push(NAME, stride, words);
    Ok(())
}

/// `uint16(clip(x, 0, 1) * 65535 + 0.5)`.
fn to_ushort(x: f32) -> u16 {
    (x.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16
}

fn run(cx: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    if k.column(NAME).is_none() {
        return Ok(());
    }
    let (kept, _) = plan(cx, cx.mesh);
    let loose = d.corners.contains(&u32::MAX);
    let cv = cx.mesh.corner_verts();
    let cf = corner_faces(cx.mesh);
    let mut buf = Vec::with_capacity(16);
    for a in &kept {
        // Loose edges and points only carry POINT attributes.
        if loose && a.attr.domain != Domain::Point {
            continue;
        }
        let n = a.shape.len();
        let mut flat: Vec<f32> = Vec::with_capacity(n * d.vertices.len());
        for (i, &c) in d.corners.iter().enumerate() {
            buf.clear();
            let e = if c == u32::MAX {
                Some(d.vertices[i] as usize)
            } else {
                corner_element(a.attr, c as usize, cv, &cf)
            };
            if let Some(e) = e {
                element(&a.attr.data, e, &mut buf);
            }
            buf.resize(n, 0.0);
            // The value as the dots hold it (`float32` words), so equal keys give equal data.
            flat.extend(buf.iter().map(|&x| f32::from_bits(key_word(x))));
        }
        let values = match a.shape {
            Shape::Scalar => AttrValues::F32(flat),
            Shape::Vec2 => AttrValues::F32x2(flat.chunks_exact(2).map(|c| [c[0], c[1]]).collect()),
            Shape::Vec3 => {
                AttrValues::F32x3(flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
            }
            Shape::Vec4 => AttrValues::F32x4(
                flat.chunks_exact(4)
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect(),
            ),
            Shape::UShort4 => AttrValues::U16x4(
                flat.chunks_exact(4)
                    .map(|c| {
                        [
                            to_ushort(c[0]),
                            to_ushort(c[1]),
                            to_ushort(c[2]),
                            to_ushort(c[3]),
                        ]
                    })
                    .collect(),
            ),
            Shape::Mat4 => AttrValues::F32x16(
                flat.chunks_exact(16)
                    .map(|c| {
                        let mut m = [0.0; 16];
                        m.copy_from_slice(c);
                        m
                    })
                    .collect(),
            ),
        };
        d.attributes.insert(
            a.gltf.clone(),
            AttrColumn {
                values,
                normalized: a.shape == Shape::UShort4,
            },
        );
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static CUSTOM_ATTRIBUTES: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    // Before NORMAL (100): the custom fields come first in the add-on's dots.
    order: 50,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_scene::Main;

    use super::*;
    use crate::export::ExportSettings;
    use crate::export::settings::default_props;

    fn settings(on: bool) -> ExportSettings {
        let mut p = default_props();
        p.export_attributes.value = on;
        ExportSettings {
            props: Arc::new(p),
            ..ExportSettings::default()
        }
    }

    fn quad() -> Mesh {
        let mut m = Mesh::new("Q");
        m.from_pydata(
            &[
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ],
            &[],
            &[vec![0, 1, 2, 3]],
            false,
        );
        m
    }

    #[test]
    fn keeps_underscore_and_khr_names_only() {
        let mut m = quad();
        let add = |m: &mut Mesh, name: &str, domain, data| {
            m.attributes_mut().insert(Attribute {
                name: name.into(),
                domain,
                data,
            })
        };
        add(
            &mut m,
            "_temp",
            Domain::Point,
            AttrData::Float(Arc::new(vec![1.0, 2.0, 3.0, 4.0])),
        );
        add(
            &mut m,
            "KHR_x",
            Domain::Face,
            AttrData::Int(Arc::new(vec![7])),
        );
        add(
            &mut m,
            "plain",
            Domain::Point,
            AttrData::Float(Arc::new(vec![0.0; 4])),
        );
        add(
            &mut m,
            "_edge",
            Domain::Edge,
            AttrData::Float(Arc::new(vec![0.0; 4])),
        );
        add(
            &mut m,
            "_TEMP",
            Domain::Point,
            AttrData::Float(Arc::new(vec![0.0; 4])),
        );
        let main = Main::default();
        let s = settings(true);
        let cx = MeshExportCx::new(&main, &m, None, &s);
        let (kept, warnings) = plan(&cx, &m);
        let names: Vec<_> = kept.iter().map(|k| k.gltf.as_str()).collect();
        assert_eq!(names, ["_TEMP", "KHR_X"]);
        assert_eq!(
            warnings,
            ["Attribute collision name: _TEMP, ignoring one of them"]
        );
        let off = settings(false);
        let cx = MeshExportCx::new(&main, &m, None, &off);
        assert!(plan(&cx, &m).0.is_empty());
    }

    #[test]
    fn byte_colours_become_normalized_ushorts() {
        assert_eq!(to_ushort(1.0), 65535);
        assert_eq!(to_ushort(0.5), 32768);
        assert_eq!(to_ushort(-1.0), 0);
        assert_eq!(to_ushort(2.0), 65535);
    }
}
