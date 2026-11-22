// SPDX-License-Identifier: GPL-3.0-or-later
//! `export_unused_textures` and `export_unused_images` (WP 8.12), both only with
//! `export_materials='EXPORT'`.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (`gather_material`: with `gltf_unused_textures`, every Image Texture node of the material,
//! `get_material_nodes` order, whose name is not in `nodes_used` is gathered as a texture from
//! its `Color` output and appended to `additional_texture_export`; an unlit material returns
//! before that; `BlenderMaterialIndentifier.__can_use_inline`: the original tree is read),
//! material/search_node_tree.py (`get_material_nodes`: the tree's non-muted nodes of the type,
//! then those of every non-muted group node with a node tree other than the legacy
//! "glTF Settings", recursively), exporter.py (`traverse_additional_textures`, then
//! `traverse_additional_images`: every image of `bpy.data.images` that is not a Viewer and
//! whose name no texture wrote (`exported_images`) becomes a glTF image) and material/image.py
//! (`get_gltf_image_from_blender_image`: the whole image, `__get_mime_type_of_image`),
//! io_scene_gltf2 5.2.41, Apache-2.0, The glTF-Blender-IO authors. Both run after the unused
//! skins (`__gather_gltf`), so their textures, samplers and images come last.
//!
//! The textures themselves are only referenced from the add-on's `additional_data` (offered to
//! user extensions), so what reaches the file is the `textures`, `samplers` and `images`
//! entries; a texture equal to one already written is shared (`__traverse` appends uniquely).

use frap_material::{ChannelSel, LoweredMaterial};
use frap_nodes::{Node, NodeTree};
use frap_rna::{IdRef, ImageId, MaterialId, NodeTreeId, Value};
use frap_scene::id::Image;

use crate::export::gather::Gather;
use crate::export::images::{self, Role, TexInput, TextureSlot};
use crate::export::{ExportError, ExportMaterials, ExportSettings};

/// The legacy settings group the add-on does not enter (`get_gltf_old_group_node_name`).
const OLD_GLTF_GROUP: &str = "glTF Settings";

/// Materials gathered with `export_materials='EXPORT'`, in gather order (the order of
/// `additional_texture_export`).
#[derive(Default)]
pub(crate) struct GatheredMaterials(pub(crate) Vec<IdRef<MaterialId>>);

/// Records a material lowered for export (called once per material by `materials::gather`).
pub(crate) fn material_gathered(g: &mut Gather, r: IdRef<MaterialId>, lm: &LoweredMaterial) {
    // `__export_unlit` returns before the unused textures are collected.
    if lm.unlit {
        return;
    }
    let s = g.state::<GatheredMaterials>();
    if !s.0.contains(&r) {
        s.0.push(r);
    }
}

/// `gltf_unused_textures`: the option, only with `export_materials='EXPORT'`.
pub fn unused_textures(settings: &ExportSettings) -> bool {
    settings.export_materials == ExportMaterials::Export
        && settings.props.export_unused_textures.value
}

/// `gltf_unused_images`.
fn unused_images(settings: &ExportSettings) -> bool {
    settings.export_materials == ExportMaterials::Export
        && settings.props.export_unused_images.value
}

/// `get_material_nodes(tree, [tree], ShaderNodeTexImage)`: each node with the tree it is in.
fn image_nodes<'m>(
    g: &Gather<'m>,
    tree: &'m NodeTree,
    depth: usize,
    out: &mut Vec<(&'m NodeTree, &'m Node)>,
) {
    for n in tree
        .nodes
        .iter()
        .filter(|n| n.bl_idname == "ShaderNodeTexImage" && !n.mute)
    {
        out.push((tree, n));
    }
    if depth > 64 {
        return;
    }
    let main = g.main;
    for n in tree
        .nodes
        .iter()
        .filter(|n| frap_nodes::is_group_node(n.bl_idname) && !n.mute)
    {
        let group = match n.prop("node_tree") {
            Some(Value::Id(Some(id))) => IdRef::<NodeTreeId>::from_any(*id),
            _ => None,
        };
        if let Some(gt) = group.and_then(|r| main.node_groups.get(r))
            && gt.header.name != OLD_GLTF_GROUP
        {
            image_nodes(g, gt, depth + 1, out);
        }
    }
}

/// `traverse_additional_textures` (with the gathering of `gather_material`).
fn additional_textures(g: &mut Gather) -> Result<(), ExportError> {
    if !unused_textures(g.settings) {
        return Ok(());
    }
    let main = g.main;
    let mats = std::mem::take(&mut g.state::<GatheredMaterials>().0);
    for r in mats {
        let Some(mat) = main.materials.get(r) else {
            continue;
        };
        let (_, used) = frap_material::lower_original_used(&mat.node_tree, mat, main);
        let mut nodes = Vec::new();
        image_nodes(g, &mat.node_tree, 0, &mut nodes);
        let mut k = 0usize;
        for (tree, n) in nodes {
            if used.contains(&n.name) {
                continue;
            }
            let cx = frap_material::LowerCx::new(tree, mat, main).with_original(true);
            let mut diags = Vec::new();
            let Some(t) = cx.tex_ref(n, ChannelSel::Rgb, &mut diags) else {
                continue;
            };
            let before = g.doc.textures.len();
            let inputs = [TexInput {
                tex: &t,
                role: Role::Full,
            }];
            let Some(i) = images::gather_texture_from(g, &inputs, (r, TextureSlot::Additional(k)))?
            else {
                continue;
            };
            k += 1;
            // `__append_unique_and_get_index`: an equal texture is not appended twice.
            if g.doc.textures.len() > before
                && g.doc.textures[..i as usize]
                    .iter()
                    .any(|x| *x == g.doc.textures[i as usize])
            {
                g.doc.textures.pop();
            }
        }
    }
    Ok(())
}

/// `__get_mime_type_of_image`: 4-channel images are PNG (WebP for a WebP file); others follow
/// `export_image_format` (`AUTO`: JPEG and WebP files keep their format).
fn image_mime(g: &Gather, im: &Image) -> &'static str {
    images::whole_image_mime(g.settings, im, images::image_channels(g, im) == 4)
}

/// `traverse_additional_images`.
fn additional_images(g: &mut Gather) -> Result<(), ExportError> {
    if !unused_images(g.settings) {
        return Ok(());
    }
    let main = g.main;
    let refs: Vec<IdRef<ImageId>> = main
        .images
        .iter()
        .filter(|(_, im)| im.source != "VIEWER")
        .map(|(r, _)| r)
        .collect();
    for r in refs {
        if g.images.was_exported(main, r) {
            continue;
        }
        let Some(im) = main.images.get(r) else {
            continue;
        };
        let mime = image_mime(g, im);
        images::write_whole_image(g, r, mime)?;
    }
    Ok(())
}

/// Both passes, after the unused skins.
pub fn finish(g: &mut Gather) -> Result<(), ExportError> {
    additional_textures(g)?;
    additional_images(g)
}
