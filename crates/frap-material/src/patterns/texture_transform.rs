// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_texture_transform` of a texture: the Mapping node in front of an Image Texture
//! (ARCHITECTURE §16.2). The exporter writes the extension as **required** (the add-on's
//! `Extension` default), so a file using it lists it in `extensionsRequired`.
//!
//! A per-texture pattern: `LowerCx::tex_ref` reads the Mapping node with [`from_mapping`] and
//! `RaiseCx::texture` builds it with [`raise_mapping`]; the material-wide entry registered
//! here has nothing to do on its own.
//!
//! Lowering ports `get_texture_transform_from_mapping_node` (`exp/material/search_node_tree.py`)
//! and `texture_transform_blender_to_gltf` / `inverted_trs_mapping_node`
//! (`blender/com/conversion.py`, glTF-Blender-IO 5.2.41, Apache-2.0): `vector_type` POINT,
//! VECTOR (no offset) or TEXTURE (the inverse TRS, when it is one); X/Y rotations cannot be
//! exported (WARNING, no transform); computed in f64 like Python.
//!
//! Raising ports the Mapping part of `texture()` (`imp/texture.py`) with
//! `texture_transform_gltf_to_blender`.

use frap_nodes::{Node, SocketAddr, SocketValue};
use frap_rna::Report;

use crate::graph::{In, unrecognised};
use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, TexTransform};

/// `get_texture_transform_from_mapping_node` + `texture_transform_blender_to_gltf`: the glTF
/// transform of Mapping node `n` (`None` when it cannot be exported or changes nothing).
///
/// The add-on reads `inputs[…].default_value` of the inlined tree: a linked input that the
/// inliner constant-folds (a Combine XYZ of constants, …) gives the folded value; one that
/// stays linked gives the socket's own value, as Blender does, with a WARNING.
pub fn from_mapping<'a>(
    cx: &LowerCx<'a>,
    n: &'a Node,
    diags: &mut Vec<Report>,
) -> Option<TexTransform> {
    let vector_type = n.prop_str("vector_type");
    if !matches!(vector_type, "POINT" | "VECTOR" | "TEXTURE") {
        unrecognised(
            diags,
            format!(
                "Mapping \"{}\": vector_type {vector_type} is not supported; texture transform skipped",
                n.name
            ),
        );
        return None;
    }
    let mut v3 = |name: &str| {
        let Some(i) = In::named(n, name) else {
            return [0.0; 3];
        };
        if let Some(v) = cx.inline_value(i) {
            return v.v().map(f64::from);
        }
        unrecognised(
            diags,
            format!(
                "Mapping \"{}\": input {name} is linked to a value that is not constant; its own value is exported",
                n.name
            ),
        );
        match &i.socket().default_value {
            SocketValue::Vector(v) => [v[0], v[1], v[2]].map(f64::from),
            _ => [0.0; 3],
        }
    };
    let (loc, rot, scale) = (v3("Location"), v3("Rotation"), v3("Scale"));
    if rot[0] != 0.0 || rot[1] != 0.0 {
        unrecognised(
            diags,
            format!(
                "Mapping \"{}\": only a Z rotation can be exported; texture transform skipped",
                n.name
            ),
        );
        return None;
    }
    let mut offset = [loc[0], loc[1]];
    let mut rotation = rot[2];
    let mut sc = [scale[0], scale[1]];
    match vector_type {
        "TEXTURE" => {
            // `inverted_trs_mapping_node`: the inverse of a TRS is a TRS only in these cases.
            if (rotation.abs() > 1e-5 && (sc[0] - sc[1]).abs() > 1e-5)
                || sc[0].abs() < 1e-5
                || sc[1].abs() < 1e-5
            {
                unrecognised(
                    diags,
                    format!(
                        "Mapping \"{}\": a TEXTURE mapping that is not a TRS cannot be exported; texture transform skipped",
                        n.name
                    ),
                );
                return None;
            }
            // Matrix.Rotation(-rotation, 3, 'Z') @ (-offset.x, -offset.y, 1)
            let (s, c) = (-rotation).sin_cos();
            let (ox, oy) = (-offset[0], -offset[1]);
            offset = [(c * ox - s * oy) / sc[0], (s * ox + c * oy) / sc[1]];
            rotation = -rotation;
            sc = [1.0 / sc[0], 1.0 / sc[1]];
        }
        "VECTOR" => offset = [0.0, 0.0],
        _ => {}
    }
    Some(blender_to_gltf_f64(offset, rotation, sc)).filter(|t| !is_identity(t))
}

fn blender_to_gltf_f64(offset: [f64; 2], rotation: f64, scale: [f64; 2]) -> TexTransform {
    TexTransform {
        offset: [
            (offset[0] - scale[1] * rotation.sin()) as f32,
            (1.0 - offset[1] - scale[1] * rotation.cos()) as f32,
        ],
        rotation: rotation as f32,
        scale: [scale[0] as f32, scale[1] as f32],
    }
}

/// `texture_transform_blender_to_gltf` (computed in f64 like Python).
pub fn blender_to_gltf_transform(offset: [f32; 2], rotation: f32, scale: [f32; 2]) -> TexTransform {
    blender_to_gltf_f64(
        offset.map(f64::from),
        f64::from(rotation),
        scale.map(f64::from),
    )
}

/// `texture_transform_gltf_to_blender` (computed in f64 like Python): Mapping node
/// (Location XY, Rotation Z, Scale XY) of a glTF texture transform.
pub fn gltf_to_blender_transform(t: &TexTransform) -> ([f32; 2], f32, [f32; 2]) {
    let (o0, o1, r, s1) = (
        f64::from(t.offset[0]),
        f64::from(t.offset[1]),
        f64::from(t.rotation),
        f64::from(t.scale[1]),
    );
    (
        [(o0 + s1 * r.sin()) as f32, (1.0 - o1 - s1 * r.cos()) as f32],
        t.rotation,
        t.scale,
    )
}

/// A glTF texture transform that changes nothing (the exporter omits it).
pub fn is_identity(t: &TexTransform) -> bool {
    t.offset == [0.0, 0.0] && t.rotation == 0.0 && t.scale == [1.0, 1.0]
}

/// The Mapping part of the importer's `texture()`: a POINT Mapping node at `(x − 160,
/// y + 30)` linked into `uv_socket`; returns its `Vector` input and the new `x`.
pub(crate) fn raise_mapping(
    cx: &mut RaiseCx,
    t: &TexTransform,
    uv_socket: SocketAddr,
    x: f32,
    y: f32,
) -> (SocketAddr, f32) {
    let mapping = cx.add_node("ShaderNodeMapping", [x - 160.0, y + 30.0]);
    cx.set_enum(mapping, "vector_type", "POINT");
    let o = cx.output(mapping, "Vector");
    cx.link(uv_socket, o);
    let next = cx.input(mapping, "Vector");
    let (loc, rot, scale) = gltf_to_blender_transform(t);
    cx.set_input(
        mapping,
        "Location",
        SocketValue::Vector([loc[0], loc[1], 0.0, 0.0]),
    );
    cx.set_input(
        mapping,
        "Rotation",
        SocketValue::Vector([0.0, 0.0, rot, 0.0]),
    );
    cx.set_input(
        mapping,
        "Scale",
        SocketValue::Vector([scale[0], scale[1], 1.0, 0.0]),
    );
    (next, x - 260.0)
}

/// Per-texture pattern: nothing material-wide to lower (see the module documentation).
fn lower(_: &LowerCx, _: &mut LoweredMaterial) {}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "texture_transform",
    order: 4,
    lower,
    raise: None,
};

#[cfg(test)]
mod tests {
    use super::*;

    use crate::graph::testkit::*;
    use frap_scene::id::Material;

    fn mapping(vector_type: &str, loc: [f32; 2], rot: f32, scale: [f32; 2]) -> (Material, u32) {
        let mut m = mat("M");
        let uid = add(&mut m, "ShaderNodeMapping");
        let n = m.node_tree.node_mut(uid).unwrap();
        // Location before the mode change: VECTOR and NORMAL disable it.
        n.input_mut("Location").unwrap().default_value =
            SocketValue::Vector([loc[0], loc[1], 0.0, 0.0]);
        assert!(n.set_prop("vector_type", frap_rna::Value::Enum(vector_type.into())));
        if let Some(r) = n.input_mut("Rotation") {
            r.default_value = SocketValue::Vector([0.0, 0.0, rot, 0.0]);
        }
        n.input_mut("Scale").unwrap().default_value =
            SocketValue::Vector([scale[0], scale[1], 1.0, 0.0]);
        (m, uid)
    }

    fn run(m: &(Material, u32), d: &mut Vec<Report>) -> Option<TexTransform> {
        let images = Images::default();
        let cx = LowerCx::new(&m.0.node_tree, &m.0, &images);
        from_mapping(&cx, m.0.node_tree.node(m.1).unwrap(), d)
    }

    #[test]
    fn point_vector_and_texture_mappings() {
        let mut d = Vec::new();
        // Research run: Location (0.1, 0.2), Scale 2 → offset [0.1, -1.2], scale [2, 2].
        let t = run(&mapping("POINT", [0.1, 0.2], 0.0, [2.0, 2.0]), &mut d).unwrap();
        assert!((t.offset[0] - 0.1).abs() < 1e-6 && (t.offset[1] + 1.2).abs() < 1e-6);
        assert_eq!(t.scale, [2.0, 2.0]);
        // VECTOR drops the location.
        let t = run(&mapping("VECTOR", [0.1, 0.2], 0.0, [2.0, 2.0]), &mut d).unwrap();
        assert!((t.offset[0]).abs() < 1e-6 && (t.offset[1] + 1.0).abs() < 1e-6);
        // TEXTURE inverts: scale 2 → 0.5, location (0.1, 0.2) → (-0.05, -0.1).
        let t = run(&mapping("TEXTURE", [0.1, 0.2], 0.0, [2.0, 2.0]), &mut d).unwrap();
        assert_eq!(t.scale, [0.5, 0.5]);
        assert!((t.offset[0] + 0.05).abs() < 1e-6);
        assert!((t.offset[1] - (1.0 + 0.1 - 0.5)).abs() < 1e-6, "{t:?}");
        // Identity is omitted.
        assert!(run(&mapping("POINT", [0.0, 0.0], 0.0, [1.0, 1.0]), &mut d).is_none());
        assert!(d.is_empty());
        // NORMAL is not exported.
        assert!(run(&mapping("NORMAL", [0.0, 0.0], 0.0, [1.0, 1.0]), &mut d).is_none());
        assert_eq!(d.len(), 1);
        // Round trip with the importer's conversion.
        let t = blender_to_gltf_transform([0.3, -0.2], 0.5, [2.0, 3.0]);
        let (loc, rot, scale) = gltf_to_blender_transform(&t);
        assert!((loc[0] - 0.3).abs() < 1e-6 && (loc[1] + 0.2).abs() < 1e-6);
        assert_eq!((rot, scale), (0.5, [2.0, 3.0]));
    }

    #[test]
    fn linked_mapping_inputs() {
        // Verified on 5.2.4 (probe tt.py): a Location linked from a Combine XYZ of constants
        // (X 0.5) is folded by the inliner → offset [0.5, 0].
        let (mut m, map) = mapping("POINT", [0.0, 0.0], 0.0, [1.0, 1.0]);
        let xyz = add(&mut m, "ShaderNodeCombineXYZ");
        set_in(&mut m, xyz, "X", float(0.5));
        link(&mut m, (xyz, "Vector"), (map, "Location"));
        let mut d = Vec::new();
        let t = run(&(m.clone(), map), &mut d).unwrap();
        assert_eq!(
            (t.offset, t.scale, t.rotation),
            ([0.5, 0.0], [1.0, 1.0], 0.0)
        );
        assert!(d.is_empty(), "{d:?}");
        // Linked to something that is not constant: the socket's own value, with a WARNING.
        let geo = add(&mut m, "ShaderNodeNewGeometry");
        link(&mut m, (geo, "Position"), (xyz, "X"));
        assert!(run(&(m, map), &mut d).is_none());
        assert!(d[0].message.contains("Location"), "{d:?}");
    }
}
