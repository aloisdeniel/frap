// SPDX-License-Identifier: GPL-3.0-or-later
//! `occlusionTexture` and the ORM image.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/material/materials.py
//! (glTF-Blender-IO 5.2.41, Apache-2.0, The glTF-Blender-IO authors):
//! - `__gather_orm_texture`: when the occlusion socket has a texture and metallic and/or
//!   roughness have one, and all those images have the same size (`check_same_size_images`,
//!   `Image.size`), the sockets `(occlusion, roughness, metallic)` (or `(occlusion,
//!   roughness)`, `(occlusion, metallic)`) make one image: R occlusion, G roughness,
//!   B metallic. `occlusionTexture` and `metallicRoughnessTexture` then share that texture
//!   (its sampler is the occlusion Image Texture's; each texture info keeps its own
//!   `texCoord` and transform). Otherwise they are written separately (the add-on logs an
//!   info message, no warning).
//! - `__gather_occlusion_texture`: `strength` from the Mix with white in front of the
//!   socket (`occlusion.rs`), written whenever that Mix exists (`1` included), omitted
//!   otherwise; the ORM sockets are used only when their texture can be written
//!   (`__gather_orm_texture`'s double check, `images::texture_writable`, WP 8.12).

use frap_gltf::json::{Material, OcclusionTextureInfo};
use frap_material::LoweredMaterial;
use frap_mesh::Mesh;
use frap_rna::{IdRef, MaterialId};

use super::num;
use crate::export::ExportError;
use crate::export::gather::Gather;
use crate::export::images::{Role, TexInput, TextureSlot, gather_texture_info_from};

/// The ORM sockets (`__gather_orm_texture`), `None` when occlusion is written alone.
pub fn orm_inputs<'l>(g: &Gather, lm: &'l LoweredMaterial) -> Option<Vec<TexInput<'l>>> {
    let occ = lm.occlusion.as_ref()?;
    let mut v = vec![TexInput {
        tex: occ,
        role: Role::Occlusion,
    }];
    let (m, r) = (lm.metallic.texture.as_ref(), lm.roughness.texture.as_ref());
    if m.is_none() && r.is_none() {
        return None;
    }
    if let Some(tex) = r {
        v.push(TexInput {
            tex,
            role: Role::Roughness,
        });
    }
    if let Some(tex) = m {
        v.push(TexInput {
            tex,
            role: Role::Metallic,
        });
    }
    // `check_same_size_images`
    let base = frap_scene::id::PathBase {
        blend: g.main.filepath.clone(),
        cwd: g.settings.cwd.clone(),
    };
    let mut sizes = v.iter().map(|i| {
        g.main
            .images
            .get(i.tex.image)
            .and_then(|im| im.info(&base))
            .map_or((0, 0), |s| (s.x, s.y))
    });
    let first = sizes.next()?;
    (first != (0, 0) && sizes.all(|s| s == first)).then_some(v)
}

/// Writes `occlusionTexture` (`orm`: the shared ORM sockets).
pub fn apply(
    g: &mut Gather,
    r: IdRef<MaterialId>,
    lm: &LoweredMaterial,
    orm: Option<&[TexInput]>,
    mesh: &Mesh,
    m: &mut Material,
) -> Result<(), ExportError> {
    let Some(occ) = &lm.occlusion else {
        return Ok(());
    };
    let alone = [TexInput {
        tex: occ,
        role: Role::Occlusion,
    }];
    let (inputs, slot) = match orm {
        Some(o) => (o, TextureSlot::MetallicRoughness),
        None => (&alone[..], TextureSlot::Occlusion),
    };
    if let Some(info) = gather_texture_info_from(g, occ, inputs, (r, slot), mesh)? {
        m.occlusion_texture = Some(OcclusionTextureInfo {
            index: info.index,
            // Written whenever a Mix gave it, 1 included (WP 8.12).
            strength: (lm.ext.occlusion_strength_set || lm.occlusion_strength != 1.0)
                .then(|| num(lm.occlusion_strength)),
            tex_coord: info.tex_coord,
            extensions: info.extensions,
            extras: None,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use frap_nodes::SocketValue;
    use frap_rna::Value;
    use frap_scene::{ChangeLog, Context, Preferences, startup};

    use crate::export::{ExportSettings, export};

    /// WP 8.12: factors of 1 the add-on writes anyway, in the written JSON: the occlusion
    /// strength of a Mix with white at factor 1 (`"strength": 1`) and the white factor of a
    /// Specular Tint behind a Multiply (`specularColorFactor: [1, 1, 1]`).
    #[test]
    fn factors_of_one_are_written() {
        let (mut main, win) = startup::factory();
        let mut log = ChangeLog::default();
        let mut g = frap_nodes::builtin_groups::gltf_material_output::new_group();
        g.header.name = "glTF Material Output".into();
        let group = main.node_groups.insert("glTF Material Output", g, &mut log);
        let ao = main
            .images
            .insert("AO", frap_scene::id::Image::new("AO", 4, 4), &mut log);
        let tint = main
            .images
            .insert("Tint", frap_scene::id::Image::new("Tint", 4, 4), &mut log);
        let mr = main.materials.find("Material").unwrap();
        let m = main.materials.get_mut(mr, &mut log).unwrap();
        let t = &mut m.node_tree;
        let add = |t: &mut frap_nodes::NodeTree, ty: &str| {
            let i = t.new_node(ty).unwrap();
            t.nodes[i].uid
        };
        let gn = add(t, "ShaderNodeGroup");
        t.node_mut(gn)
            .unwrap()
            .set_prop("node_tree", Value::Id(Some(group.any())));
        let iface = main_iface();
        t.node_mut(gn).unwrap().sync_group_sockets(Some(&iface));
        let img = |t: &mut frap_nodes::NodeTree, im: frap_rna::IdRef<frap_rna::ImageId>| {
            let n = add(t, "ShaderNodeTexImage");
            t.node_mut(n)
                .unwrap()
                .set_prop("image", Value::Id(Some(im.any())));
            n
        };
        let mix = |t: &mut frap_nodes::NodeTree, blend: &str| {
            let n = add(t, "ShaderNodeMix");
            let node = t.node_mut(n).unwrap();
            node.set_prop("data_type", Value::Enum("RGBA".into()));
            node.set_prop("blend_type", Value::Enum(blend.into()));
            node.input_mut("Factor").unwrap().default_value = SocketValue::Float(1.0);
            node.input_mut("A").unwrap().default_value = SocketValue::Color([1.0; 4]);
            n
        };
        let ia = img(t, ao);
        let ma = mix(t, "MIX");
        t.link_by_name((ia, "Color"), (ma, "B")).unwrap();
        t.link_by_name((ma, "Result"), (gn, "Occlusion")).unwrap();
        let it = img(t, tint);
        let mt = mix(t, "MULTIPLY");
        t.link_by_name((it, "Color"), (mt, "B")).unwrap();
        let bsdf = t.first_of_type("ShaderNodeBsdfPrincipled").unwrap().uid;
        t.link_by_name((mt, "Result"), (bsdf, "Specular Tint")).unwrap();
        let prefs = Preferences::default();
        let ctx = Context {
            main: &main,
            win: &win,
            prefs: &prefs,
        };
        let out = export(&main, &ctx, &ExportSettings::default()).unwrap();
        let gm = &out.document.materials[0];
        assert_eq!(gm.occlusion_texture.as_ref().unwrap().strength, Some(1.0));
        let spec: frap_gltf::ext::khr_materials_specular::KhrMaterialsSpecular =
            gm.extensions.get().unwrap().unwrap();
        assert_eq!(spec.specular_color_factor, Some([1.0; 3]));
    }

    /// The interface of the add-on's "glTF Material Output" group.
    fn main_iface() -> Vec<frap_nodes::InterfaceSocket> {
        frap_nodes::builtin_groups::gltf_material_output::new_group().interface
    }
}
