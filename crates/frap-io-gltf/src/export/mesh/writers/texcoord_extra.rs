// SPDX-License-Identifier: GPL-3.0-or-later
//! Extra TEXCOORD_n writer: generic attributes that a material reads as UV coordinates (an
//! Attribute node of type GEOMETRY, through an optional Mapping node, into the Vector of an
//! exported Image Texture) and that are not UV maps become `TEXCOORD_<n>` after the regular
//! UV maps (`n` from the number of exported UV maps, per primitive in name order), V flipped.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/primitive_extract.py
//! (`manage_material_info`: `uvmap_attribute_list` per material, the extra `dots` fields
//! `<attr>0`/`<attr>1`, CORNER or POINT, FLOAT2 or the xy of FLOAT_VECTOR, other types left at
//! zero; `primitive_creation_not_shared`: `TEXCOORD_<tex_coord_max + k>`) and
//! blender/exp/material/texture_info.py (`get_texture_transform_and_uvmap_info`: the `'Attribute'`
//! UV type), io_scene_gltf2 5.2.41, Apache-2.0, The glTF-Blender-IO authors.
//!
//! Only materials exported with `export_materials='EXPORT'` read UVs. The fields join the
//! split key after the tangents and before the colours, for every corner, as the add-on's dots
//! do. The add-on lists a material's attributes through a Python `set` (unordered); frap sorts
//! them by name. The material side (`texCoord` of the texture) is `materials/`' business.

use std::collections::BTreeSet;

use frap_material::LoweredMaterial;
use frap_mesh::{AttrData, Domain, Mesh};
use frap_nodes::{Node, NodeTree};
use frap_rna::{IdRef, ImageId, MaterialId, Value};
use frap_scene::id::Material;
use smol_str::SmolStr;

use crate::export::ExportError;
use crate::export::ExportMaterials;
use crate::export::mesh::convert::key_word;
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter,
    PrimitiveDraft, SplitKey,
};
use crate::export::mesh::gather::{material_for_slot, slot_materials};
use crate::export::mesh::primitives::NO_MATERIAL_SLOT;
use crate::export::mesh::writers::texcoord::flip_v;

/// This writer's split-key column.
pub const NAME: &str = "texcoord_extra";

/// The images the lowered material exports as textures.
fn exported_images(lm: &LoweredMaterial) -> BTreeSet<IdRef<ImageId>> {
    crate::export::materials::textures(lm)
        .into_iter()
        .map(|t| t.image)
        .collect()
}

fn node(tree: &NodeTree, uid: u32) -> Option<&Node> {
    tree.nodes.iter().find(|n| n.uid == uid)
}

/// The node linked into input `name` of `to`.
fn from_input<'t>(tree: &'t NodeTree, to: &Node, name: &str) -> Option<&'t Node> {
    let sock = to.inputs.iter().find(|s| s.name == name)?;
    let l = tree
        .links
        .iter()
        .find(|l| l.to_node == to.uid && l.to_socket == sock.identifier)?;
    node(tree, l.from_node)
}

fn prop_str<'n>(n: &'n Node, name: &str) -> Option<&'n str> {
    match n.props.values.get(name) {
        Some(Value::Enum(s) | Value::Str(s)) => Some(s.as_str()),
        _ => None,
    }
}

/// The attribute names the material's exported Image Textures read as UVs (`'Attribute'`
/// entries of `uv_info`).
pub fn attribute_uvs(mat: &Material, lm: &LoweredMaterial) -> BTreeSet<SmolStr> {
    let images = exported_images(lm);
    let tree = &mat.node_tree;
    let mut out = BTreeSet::new();
    for n in tree
        .nodes
        .iter()
        .filter(|n| n.bl_idname == "ShaderNodeTexImage")
    {
        let image = match n.props.values.get("image") {
            Some(Value::Id(Some(id))) => id.typed::<ImageId>(),
            _ => None,
        };
        if !image.is_some_and(|i| images.contains(&i)) {
            continue;
        }
        let mut src = from_input(tree, n, "Vector");
        if let Some(m) = src.filter(|m| m.bl_idname == "ShaderNodeMapping") {
            src = from_input(tree, m, "Vector");
        }
        if let Some(a) = src.filter(|a| a.bl_idname == "ShaderNodeAttribute")
            && prop_str(a, "attribute_type") == Some("GEOMETRY")
            && let Some(name) = prop_str(a, "attribute_name").filter(|s| !s.is_empty())
        {
            out.insert(SmolStr::new(name));
        }
    }
    out
}

/// Per material bucket, the attribute UVs it exports (`uvmap_attribute_lists`), in bucket
/// order, and their union in field order.
pub fn plan(cx: &MeshExportCx) -> (Vec<(usize, Vec<SmolStr>)>, Vec<SmolStr>) {
    if cx.settings.export_materials != ExportMaterials::Export {
        return (Vec::new(), Vec::new());
    }
    let mats: Vec<Option<IdRef<MaterialId>>> = match cx.object.and_then(|o| cx.main.objects.get(o))
    {
        Some(ob) => slot_materials(ob, cx.mesh),
        None => cx.mesh.materials.clone(),
    };
    let uv_names: Vec<&str> = cx.mesh.uv_maps().into_iter().map(|(n, _)| n).collect();
    let mi = cx
        .mesh
        .attributes()
        .get("material_index")
        .and_then(|a| match (&a.domain, &a.data) {
            (Domain::Face, AttrData::Int(v)) => Some(v.clone()),
            _ => None,
        });
    let mut buckets: Vec<usize> = (0..cx.mesh.faces_num())
        .filter(|&f| cx.mesh.face(f).len() >= 3)
        .map(|f| {
            mi.as_ref()
                .and_then(|v| v.get(f).copied())
                .map_or(0, |i| i as u32 as usize)
        })
        .collect();
    buckets.sort_unstable();
    buckets.dedup();
    let mut per = Vec::new();
    let mut fields: Vec<SmolStr> = Vec::new();
    for b in buckets {
        let names: Vec<SmolStr> = material_for_slot(cx.settings.export_materials, b, &mats)
            .and_then(|m| cx.main.materials.get(m))
            .map(|mat| {
                let lm = crate::export::materials::lower::lowered(cx.main, mat);
                attribute_uvs(mat, &lm)
                    .into_iter()
                    .filter(|n| !uv_names.contains(&n.as_str()))
                    .filter(|n| cx.mesh.attributes().get(n).is_some())
                    .collect()
            })
            .unwrap_or_default();
        for n in &names {
            if !fields.contains(n) {
                fields.push(n.clone());
            }
        }
        per.push((b, names));
    }
    (per, fields)
}

/// An attribute's per-corner glTF UVs (zeros for unsupported types or domains).
fn corner_uvs(mesh: &Mesh, name: &str) -> Vec<[f32; 2]> {
    let n = mesh.corners_num();
    let Some(a) = mesh.attributes().get(name) else {
        return vec![[0.0; 2]; n];
    };
    let per: Vec<[f32; 2]> = match &a.data {
        AttrData::Float2(v) => v.to_vec(),
        AttrData::Float3(v) => v.iter().map(|p| [p[0], p[1]]).collect(),
        _ => return vec![[0.0; 2]; n],
    };
    let uv = match a.domain {
        Domain::Corner => per,
        Domain::Point => mesh
            .corner_verts()
            .iter()
            .map(|&v| per[v as usize])
            .collect(),
        _ => return vec![[0.0; 2]; n],
    };
    uv.into_iter().map(flip_v).collect()
}

fn key(cx: &MeshExportCx, k: &mut SplitKey) -> Result<(), ExportError> {
    let (_, fields) = plan(cx);
    if fields.is_empty() {
        return Ok(());
    }
    let n = cx.mesh.corners_num();
    let cols: Vec<Vec<[f32; 2]>> = fields.iter().map(|f| corner_uvs(cx.mesh, f)).collect();
    let mut words = Vec::with_capacity(2 * fields.len() * n);
    for c in 0..n {
        for col in &cols {
            words.push(key_word(col[c][0]));
            words.push(key_word(col[c][1]));
        }
    }
    k.push(NAME, 2 * fields.len(), words);
    Ok(())
}

fn run(cx: &MeshExportCx, k: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(col) = k.column(NAME) else {
        return Ok(());
    };
    if d.corners.contains(&u32::MAX) || d.material_slot == NO_MATERIAL_SLOT {
        return Ok(());
    }
    let (per, fields) = plan(cx);
    let Some((_, names)) = per.iter().find(|(b, _)| *b == d.material_slot) else {
        return Ok(());
    };
    let base = if cx.settings.export_texcoords {
        cx.mesh.uv_maps().len()
    } else {
        0
    };
    for (k, name) in names.iter().enumerate() {
        let Some(f) = fields.iter().position(|x| x == name) else {
            continue;
        };
        let values = d
            .corners
            .iter()
            .map(|&c| {
                let at = c as usize * col.stride + 2 * f;
                [
                    f32::from_bits(col.words[at]),
                    f32::from_bits(col.words[at + 1]),
                ]
            })
            .collect();
        d.attributes.insert(
            format!("TEXCOORD_{}", base + k).into(),
            AttrColumn {
                values: AttrValues::F32x2(values),
                normalized: false,
            },
        );
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static TEXCOORD_EXTRA: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 350,
    key: Some(key),
    run,
};

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use frap_mesh::Attribute;

    use super::*;

    #[test]
    fn corner_and_point_attributes_as_flipped_uvs() {
        let mut m = Mesh::new("T");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2]],
            true,
        );
        m.attributes_mut().insert(Attribute {
            name: "P".into(),
            domain: Domain::Point,
            data: AttrData::Float3(Arc::new(vec![[0.1, 0.2, 9.0], [0.3, 0.4, 9.0], [0.5, 0.75, 9.0]])),
        });
        m.attributes_mut().insert(Attribute {
            name: "C".into(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(vec![[1.0, 1.0], [2.0, 0.0], [0.0, 0.5]])),
        });
        m.attributes_mut().insert(Attribute {
            name: "I".into(),
            domain: Domain::Corner,
            data: AttrData::Int(Arc::new(vec![1, 2, 3])),
        });
        assert_eq!(corner_uvs(&m, "P")[2], [0.5, 0.25]);
        assert_eq!(corner_uvs(&m, "C"), [[1.0, 0.0], [2.0, 1.0], [0.0, 0.5]]);
        // Unsupported types stay at zero (not flipped).
        assert_eq!(corner_uvs(&m, "I"), [[0.0, 0.0]; 3]);
    }

    /// Blender 5.2.4: the factory cube with a FLOAT2 point attribute "MyUV" (`(0.1 i,
    /// 0.5 + 0.05 i)` at vertex i; a FLOAT2 *corner* attribute would be a UV map) read by an
    /// Attribute → Mapping → Image Texture → Base Color chain exports `TEXCOORD_1` with the
    /// flipped values (vertex 1: `(0.1, 0.45)`), 24 vertices.
    #[test]
    fn attribute_read_by_a_material_becomes_texcoord_1() {
        use frap_nodes::Link;
        use frap_scene::id::Image;
        let (mut main, _win) = frap_scene::startup::factory();
        let mut log = frap_scene::ChangeLog::default();
        let img = main.images.insert("g", Image::new("g", 4, 4), &mut log);
        let ma = main.materials.find("Material").unwrap();
        let mat = main.materials.get_mut(ma, &mut log).unwrap();
        let t = &mut mat.node_tree;
        let (ti, ai, mi) = (
            t.new_node("ShaderNodeTexImage").unwrap(),
            t.new_node("ShaderNodeAttribute").unwrap(),
            t.new_node("ShaderNodeMapping").unwrap(),
        );
        t.nodes[ti]
            .props
            .values
            .insert("image", Value::Id(Some(img.any())));
        t.nodes[ai]
            .props
            .values
            .insert("attribute_name", Value::Str("MyUV".into()));
        let bsdf = t
            .nodes
            .iter()
            .find(|n| n.bl_idname == "ShaderNodeBsdfPrincipled")
            .unwrap()
            .uid;
        let (tu, au, mu) = (t.nodes[ti].uid, t.nodes[ai].uid, t.nodes[mi].uid);
        for (f, fs, to, ts) in [
            (au, "Vector", mu, "Vector"),
            (mu, "Vector", tu, "Vector"),
            (tu, "Color", bsdf, "Base Color"),
        ] {
            t.links.push(Link {
                from_node: f,
                from_socket: fs.into(),
                to_node: to,
                to_socket: ts.into(),
            });
        }
        let me = main.meshes.find("Cube").unwrap();
        let mesh = main.meshes.get_mut(me, &mut log).unwrap();
        let n = mesh.verts_num();
        mesh.attributes_mut().insert(Attribute {
            name: "MyUV".into(),
            domain: Domain::Point,
            data: AttrData::Float2(Arc::new(
                (0..n).map(|i| [i as f32 * 0.1, 0.5 + i as f32 * 0.05]).collect(),
            )),
        });
        let mesh = main.meshes.get(me).unwrap();
        let s = crate::export::ExportSettings::default();
        let cx = MeshExportCx::new(&main, mesh, None, &s);
        assert_eq!(plan(&cx).1, [SmolStr::new("MyUV")]);
        let d = crate::export::mesh::primitives::extract(&cx).unwrap();
        assert_eq!(d[0].positions.len(), 24);
        let AttrValues::F32x2(v) = &d[0].attributes["TEXCOORD_1"].values else {
            panic!("TEXCOORD_1 is FLOAT VEC2")
        };
        let v1 = d[0].vertices.iter().position(|&x| x == 1).unwrap();
        assert!((v[v1][0] - 0.1).abs() < 1e-6 && (v[v1][1] - 0.45).abs() < 1e-6);
        // TEXCOORD_0 is the regular UV map.
        assert_eq!(d[0].texcoords.len(), 1);
        // Without materials exported, no extra UVs.
        let none = crate::export::ExportSettings {
            export_materials: ExportMaterials::Placeholder,
            ..crate::export::ExportSettings::default()
        };
        let cx = MeshExportCx::new(&main, mesh, None, &none);
        assert!(plan(&cx).1.is_empty());
    }

    #[test]
    fn nothing_without_materials() {
        let m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let main = frap_scene::Main::default();
        let s = crate::export::ExportSettings::default();
        let cx = MeshExportCx::new(&main, &m, None, &s);
        assert!(plan(&cx).1.is_empty());
    }
}
