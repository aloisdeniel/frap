// SPDX-License-Identifier: GPL-3.0-or-later
//! Materials (ARCHITECTURE §17.2): one file per glTF material feature, each consuming the
//! [`LoweredMaterial`] of `frap-material` (§16.2).
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (`gather_material`: member order, `doubleSided` from `use_backface_culling`;
//! `get_final_material`: a material whose textures resolve to other `texCoord`s on another
//! mesh is written again, so the cache key is the material plus its resolved texture
//! coordinates) and material_viewport.py (`export_materials='VIEWPORT'`).
//!
//! M1 covers the base colour (factor and Image Texture), metallic and roughness factors,
//! emission (factor and `KHR_materials_emissive_strength`), alpha mode and cutoff, the normal
//! map and a metallic-roughness texture used as is. The `KHR_materials_*` extensions are
//! written by the exporters of [`ext`] (WP 3.6). Occlusion (ORM packing) and unlit are WP 3.5:
//! until it lands a material using them is exported without them and a WARNING names the
//! feature.

pub mod core_alpha;
pub mod core_emissive;
pub mod core_pbr;
pub mod lower;

/// `KHR_materials_*` exporters, one file per extension (`ext/<name>.rs`, WP 3.6), each
/// registering a [`crate::material_ext::MaterialExtExporter`] (M3 W0 extension point).
pub mod ext {
    automod::dir!(pub "src/export/materials/ext");
}

use std::collections::HashMap;

use frap_gltf::json::{Index, Material as GMaterial, NormalTextureInfo, PbrMetallicRoughness};
use frap_material::{LoweredMaterial, TexRef};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};
use frap_scene::id::Material;

use crate::export::gather::Gather;
use crate::export::images::{TextureSlot, gather_texture_info, tex_coord};
use crate::export::{ExportError, ExportMaterials};

/// Materials gathered so far.
#[derive(Default)]
pub struct MaterialCache {
    lowered: HashMap<IdRef<MaterialId>, LoweredMaterial>,
    written: HashMap<(IdRef<MaterialId>, Vec<u32>), Option<Index>>,
}

/// The textures of a lowered material: the core ones in the add-on's traversal order
/// (emissive, normal, occlusion, base colour, metallic-roughness), then every
/// `KHR_materials_*` texture. All of them take part in the cache key, as every texture the
/// add-on gathers records its `texCoord` for `get_final_material`.
pub fn textures(lm: &LoweredMaterial) -> Vec<&TexRef> {
    let e = &lm.ext;
    let cc = e.clearcoat.as_ref();
    let sh = e.sheen.as_ref();
    let sp = e.specular.as_ref();
    let ir = e.iridescence.as_ref();
    let dt = e.diffuse_transmission.as_ref();
    [
        lm.emissive.texture.as_ref(),
        lm.normal.as_ref().map(|n| &n.texture),
        lm.occlusion.as_ref(),
        lm.base_color.texture.as_ref(),
        lm.metallic.texture.as_ref(),
        lm.roughness.texture.as_ref(),
        cc.and_then(|c| c.factor.texture.as_ref()),
        cc.and_then(|c| c.roughness.texture.as_ref()),
        cc.and_then(|c| c.normal.as_ref()).map(|n| &n.texture),
        e.transmission
            .as_ref()
            .and_then(|t| t.factor.texture.as_ref()),
        e.volume.as_ref().and_then(|v| v.thickness.texture.as_ref()),
        sp.and_then(|s| s.factor.texture.as_ref()),
        sp.and_then(|s| s.color.texture.as_ref()),
        sh.and_then(|s| s.color.texture.as_ref()),
        sh.and_then(|s| s.roughness.texture.as_ref()),
        ir.and_then(|i| i.factor.texture.as_ref()),
        ir.and_then(|i| i.thickness_max.texture.as_ref()),
        e.anisotropy.as_ref().and_then(|a| a.texture.as_ref()),
        dt.and_then(|d| d.factor.texture.as_ref()),
        dt.and_then(|d| d.color.texture.as_ref()),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// f32 → JSON number as the add-on writes it (the float32 value widened to double).
pub fn num(x: f32) -> f64 {
    f64::from(x)
}

/// The glTF material for `r` as used by `mesh` (`None` when nothing is exported).
pub fn gather(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    mesh: &Mesh,
) -> Result<Option<Index>, ExportError> {
    let main = g.main;
    let Some(mat) = main.materials.get(r) else {
        return Ok(None);
    };
    match g.settings.export_materials {
        ExportMaterials::Viewport => {
            let key = (r, Vec::new());
            if let Some(i) = g.materials.written.get(&key) {
                return Ok(*i);
            }
            g.doc.materials.push(viewport(mat));
            let i = Some((g.doc.materials.len() - 1) as Index);
            g.materials.written.insert(key, i);
            Ok(i)
        }
        ExportMaterials::Export => {
            let lm = g
                .materials
                .lowered
                .entry(r)
                .or_insert_with(|| lower::lowered(main, mat))
                .clone();
            let coords: Vec<u32> = textures(&lm).iter().map(|t| tex_coord(t, mesh)).collect();
            let key = (r, coords);
            if let Some(i) = g.materials.written.get(&key) {
                return Ok(*i);
            }
            let m = build(g, r, mat, &lm, mesh)?;
            g.doc.materials.push(m);
            let i = Some((g.doc.materials.len() - 1) as Index);
            g.materials.written.insert(key, i);
            Ok(i)
        }
        ExportMaterials::Placeholder | ExportMaterials::None => Ok(None),
    }
}

/// `export_viewport_material`: viewport display values, every factor written.
pub fn viewport(mat: &Material) -> GMaterial {
    GMaterial {
        name: Some(mat.header.name.to_string()),
        pbr_metallic_roughness: Some(PbrMetallicRoughness {
            base_color_factor: Some(mat.diffuse_color.map(num)),
            metallic_factor: Some(num(mat.metallic)),
            roughness_factor: Some(num(mat.roughness)),
            ..PbrMetallicRoughness::default()
        }),
        ..GMaterial::default()
    }
}

/// `gather_material` for `export_materials='EXPORT'`, textures gathered in the add-on's
/// member order (`emissiveTexture`, `normalTexture`, `occlusionTexture`,
/// `pbrMetallicRoughness`).
fn build(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    mat: &Material,
    lm: &LoweredMaterial,
    mesh: &Mesh,
) -> Result<GMaterial, ExportError> {
    for d in &lm.diagnostics {
        if !g.reports.contains(d) {
            g.reports.push(d.clone());
        }
    }
    let name = mat.header.name.to_string();
    let mut m = GMaterial {
        name: Some(name.clone()),
        ..GMaterial::default()
    };
    m.extras = crate::export::extras::material(g, mat);
    core_alpha::apply(lm, &mut m);
    // `use_backface_culling` is read on the original material, as the add-on does.
    m.double_sided = (!mat.use_backface_culling).then_some(true);
    core_emissive::apply(g, r, lm, mesh, &mut m)?;
    if let Some(n) = &lm.normal
        && let Some(info) = gather_texture_info(g, &n.texture, (r, TextureSlot::Normal), mesh)?
    {
        m.normal_texture = Some(NormalTextureInfo {
            index: info.index,
            scale: (n.scale != 1.0).then(|| num(n.scale)),
            tex_coord: info.tex_coord,
            extensions: info.extensions,
            extras: None,
        });
    }
    if lm.occlusion.is_some() {
        g.warn(format!(
            "material '{name}': occlusion (ORM packing) is not exported yet (M3)"
        ));
    }
    if lm.unlit {
        g.warn(format!(
            "material '{name}': KHR_materials_unlit is not exported yet (M3)"
        ));
    }
    core_pbr::apply(g, r, lm, mesh, &mut m)?;
    // `KHR_materials_*` (M3 W0 extension point; WP 3.6 registers the exporters).
    let mut cx = crate::material_ext::ExtExportCx {
        g,
        material: r,
        mesh,
    };
    for e in crate::material_ext::exporters() {
        (e.run)(&mut cx, lm, &mut m)?;
    }
    Ok(m)
}
