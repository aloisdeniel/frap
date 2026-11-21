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
//! Core features (WP 3.5): alpha mode and cutoff (`core_alpha`), emission and
//! `KHR_materials_emissive_strength` (`core_emissive`), the normal map (`core_normal`),
//! occlusion and the ORM image (`core_occlusion`), base colour, metallic and roughness
//! (`core_pbr`), `KHR_materials_unlit` (`core_unlit`); the `KHR_materials_*` extensions are
//! the `ext/` exporters (WP 3.6). Members are gathered in the add-on's traversal order
//! (alphabetical: `emissiveTexture`, `extensions`, `normalTexture`, `occlusionTexture`,
//! `pbrMetallicRoughness`), so texture, sampler and image arrays follow Blender's.
//! `doubleSided` is `false` when `KHR_materials_volume` is written (`__gather_double_sided`).

pub mod core_alpha;
pub mod core_emissive;
pub mod core_normal;
pub mod core_occlusion;
pub mod core_pbr;
pub mod core_unlit;
pub mod lower;

/// `KHR_materials_*` exporters, one file per extension (`ext/<name>.rs`, WP 3.6), each
/// registering a [`crate::material_ext::MaterialExtExporter`] (M3 W0 extension point).
pub mod ext {
    automod::dir!(pub "src/export/materials/ext");
}

use std::collections::HashMap;

use frap_gltf::json::{Index, Material as GMaterial, PbrMetallicRoughness};
use frap_material::{LoweredMaterial, TexRef};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};
use frap_scene::id::Material;

use crate::export::gather::Gather;
use crate::export::images::tex_coord;
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

/// Calls `f` on every texture of [`textures`], in the same order.
pub fn for_each_texture_mut(lm: &mut LoweredMaterial, mut f: impl FnMut(&mut TexRef)) {
    let mut visit = |t: Option<&mut TexRef>| {
        if let Some(t) = t {
            f(t);
        }
    };
    visit(lm.emissive.texture.as_mut());
    visit(lm.normal.as_mut().map(|n| &mut n.texture));
    visit(lm.occlusion.as_mut());
    visit(lm.base_color.texture.as_mut());
    visit(lm.metallic.texture.as_mut());
    visit(lm.roughness.texture.as_mut());
    let e = &mut lm.ext;
    if let Some(c) = e.clearcoat.as_mut() {
        visit(c.factor.texture.as_mut());
        visit(c.roughness.texture.as_mut());
        visit(c.normal.as_mut().map(|n| &mut n.texture));
    }
    visit(
        e.transmission
            .as_mut()
            .and_then(|t| t.factor.texture.as_mut()),
    );
    visit(e.volume.as_mut().and_then(|v| v.thickness.texture.as_mut()));
    if let Some(sp) = e.specular.as_mut() {
        visit(sp.factor.texture.as_mut());
        visit(sp.color.texture.as_mut());
    }
    if let Some(sh) = e.sheen.as_mut() {
        visit(sh.color.texture.as_mut());
        visit(sh.roughness.texture.as_mut());
    }
    if let Some(ir) = e.iridescence.as_mut() {
        visit(ir.factor.texture.as_mut());
        visit(ir.thickness_max.texture.as_mut());
    }
    visit(e.anisotropy.as_mut().and_then(|a| a.texture.as_mut()));
    if let Some(dt) = e.diffuse_transmission.as_mut() {
        visit(dt.factor.texture.as_mut());
        visit(dt.color.texture.as_mut());
    }
}

/// The extension objects `__gather_extensions` creates for `lm` beyond the always-created ones,
/// possibly empty (they declare the extension in the add-on's traversal order even when the
/// JSON drops them; `ext_order`): iridescence for a Principled material with the glTF Material
/// Output group's `Iridescence Factor` (`export_iridescence`), dispersion for one with a volume
/// and the group's `Dispersion` (`export_dispersion`).
fn created_empty(lm: &LoweredMaterial, m: &GMaterial) -> Vec<&'static str> {
    let mut out = Vec::new();
    if lm.unlit {
        return out;
    }
    if lm.ext.iridescence.is_some() {
        out.push("KHR_materials_iridescence");
    }
    if lm.ext.dispersion.is_some() && m.extensions.0.contains_key("KHR_materials_volume") {
        out.push("KHR_materials_dispersion");
    }
    out
}

/// `export_image_format='NONE'` (WP 7.7): `__gather_base_color_factor` and
/// `export_emission_factor` read the RGB of the Base Color and Emissive sockets themselves
/// (`get_const_from_default_value_socket`, linked or not), not their factor; the base colour's
/// colour attribute is not gathered (`vc_info['color']` stays unset, see
/// `mesh/writers/color.rs`), and an emission with a linked colour keeps that value (its
/// `[1, 1, 1]` for a texture without a factor does not apply).
pub fn image_none_factors(
    main: &frap_scene::Main,
    mat: &Material,
    settings: &crate::export::ExportSettings,
    lm: &mut LoweredMaterial,
) {
    use frap_nodes::{SocketType, SocketValue};
    let cx = frap_material::LowerCx::new(&mat.node_tree, mat, main)
        .with_original(lower::reads_original(settings, mat));
    let rgb = |i: frap_material::graph::In| match (i.socket().ty, &i.socket().default_value) {
        (SocketType::Rgba, SocketValue::Color(c)) => Some([c[0], c[1], c[2]]),
        _ => None,
    };
    if lm.unlit {
        return;
    }
    if let Some(s) = cx
        .get_socket("Base Color")
        .or_else(|| cx.get_socket("BaseColor"))
        .or_else(|| cx.gltf_output_input("BaseColorFactor"))
    {
        let c = rgb(s).unwrap_or([1.0; 3]);
        let a = lm.base_color.factor[3];
        lm.base_color.factor = [c[0], c[1], c[2], a];
    }
    if let Some(s) = cx
        .get_socket("Emissive")
        .or_else(|| cx.gltf_output_input("EmissiveFactor"))
        && let Some(c) = rgb(s)
    {
        // The strength is still read through `get_factor_from_socket`.
        let strength_name = match s.node.bl_idname {
            "ShaderNodeEmission" => "Strength",
            _ => "Emission Strength",
        };
        let strength =
            frap_material::graph::In::named(s.node, strength_name).and_then(|st| {
                match cx.get_factor(st) {
                    Some(frap_material::graph::Konst::Value(v)) => Some(f64::from(v)),
                    _ => None,
                }
            });
        let f = c.map(|x| strength.map_or(f64::from(x), |v| f64::from(x) * v) as f32);
        lm.emissive.factor = f;
        lm.emissive_strength = 1.0;
        lm.emissive.texture = None;
    }
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
            let settings = g.settings;
            let mut lm = g
                .materials
                .lowered
                .entry(r)
                .or_insert_with(|| lower::lowered_for(main, mat, settings))
                .clone();
            crate::export::tex_coord::resolve_attributes(&mut lm, mat, mesh, settings);
            if settings.export_image_format == crate::export::settings::ExportImageFormat::None {
                image_none_factors(main, mat, settings, &mut lm);
            }
            let coords: Vec<u32> = textures(&lm).iter().map(|t| tex_coord(t, mesh)).collect();
            let key = (r, coords);
            if let Some(i) = g.materials.written.get(&key) {
                return Ok(*i);
            }
            let m = build(g, r, mat, &lm, mesh)?;
            let created = created_empty(&lm, &m);
            g.doc.materials.push(m);
            let i = Some((g.doc.materials.len() - 1) as Index);
            if !created.is_empty() {
                let idx = (g.doc.materials.len() - 1) as Index;
                g.state::<crate::export::ext_order::CreatedEmpty>()
                    .0
                    .insert(idx, created);
            }
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

/// `gather_material` for `export_materials='EXPORT'`, members gathered in the add-on's
/// traversal order (module documentation).
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
    let mut m = GMaterial {
        name: Some(mat.header.name.to_string()),
        ..GMaterial::default()
    };
    m.extras = crate::export::extras::material(g, mat);
    core_alpha::apply(lm, &mut m);
    // `use_backface_culling` is read on the original material, as the add-on does.
    m.double_sided = (!mat.use_backface_culling).then_some(true);
    if lm.unlit {
        core_unlit::apply(g, r, lm, mesh, &mut m)?;
        return Ok(m);
    }
    core_emissive::apply(g, r, lm, mesh, &mut m)?;
    // `KHR_materials_*` (M3 W0 extension point; WP 3.6 registers the exporters).
    let mut cx = crate::material_ext::ExtExportCx {
        g: &mut *g,
        material: r,
        mesh,
    };
    for e in crate::material_ext::exporters() {
        (e.run)(&mut cx, lm, &mut m)?;
    }
    if m.extensions.contains("KHR_materials_volume") {
        m.double_sided = Some(false);
    }
    core_normal::apply(g, r, lm, mesh, &mut m)?;
    let orm = core_occlusion::orm_inputs(g, lm);
    core_occlusion::apply(g, r, lm, orm.as_deref(), mesh, &mut m)?;
    core_pbr::apply(g, r, lm, orm.as_deref(), mesh, &mut m)?;
    Ok(m)
}
