// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_diffuse_transmission` (frap option `frap_diffuse_transmission`, WP 7.4):
//! the Principled BSDF in thin-walled mode with subsurface scattering.
//!
//! io_scene_gltf2 5.2.41 has no mapping for the extension (it neither writes nor reads it),
//! so this one is frap's. Blender 5.2's Principled BSDF has no diffuse-transmission input;
//! with `Thin Wall` on, the surface is an infinitely thin slab and its subsurface lobe becomes
//! light diffusely transmitted through it (Subsurface Radius, Scale and Method no longer
//! matter), which is what the extension describes for a material without
//! `KHR_materials_volume`. So:
//!
//! - **Lowering.** A Principled BSDF whose `Thin Wall` is on (unlinked) and whose `Subsurface
//!   Weight` is linked or not 0 gives `diffuseTransmissionFactor` = `Subsurface Weight` (an
//!   unlinked value, or the factor of the image behind it, whose Alpha output must be the
//!   one used, since the glTF texture is read in A; another channel gives the factor alone
//!   and a WARNING). Blender's subsurface takes the Base Color, so the transmission colour is
//!   the base colour: `diffuseTransmissionColor{Factor,Texture}` repeat the `Base Color`
//!   socket's value, or its factor and image (copied whole, RGB).
//! - **Raising** (frap's importer; Blender ignores the extension): `Thin Wall` on and
//!   `Subsurface Weight` from the factor and the texture's Alpha (the importer's
//!   `scalar_factor_and_texture`, an Image Texture labelled "DIFFUSE TRANSMISSION"). The
//!   transmission colour has no socket of its own; the importer warns when it is not the
//!   base colour.

use frap_nodes::SocketValue;

use super::transmission::ext_common::{
    TexKind, factor_f32, factor_rgb, image_from_socket, konst_f32, konst_rgb,
    scalar_factor_and_texture, texture_behind,
};
use crate::graph::{Src, unrecognised};
use crate::{
    Channel, ChannelSel, DiffuseTransmission, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx,
};

/// The glTF texture member this pattern lays out (`calc_locations` block).
const BLOCK: &str = "diffuseTransmissionTexture";

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    let (Some(ti), Some(wi), Some(ci)) = (
        p.input_index("Thin Wall"),
        p.input_index("Subsurface Weight"),
        p.input_index("Base Color"),
    ) else {
        return;
    };
    if !matches!(cx.src_at(p, ti), Src::Const(SocketValue::Bool(true))) {
        return;
    }
    let src = cx.src_at(p, wi);
    let factor = match konst_f32(cx, src) {
        Some(0.0) => return,
        Some(f) => Channel {
            factor: f,
            texture: None,
        },
        None => {
            let texture = match image_from_socket(cx, p, wi) {
                Some(found) if found.channel == Some(ChannelSel::A) => {
                    cx.tex_ref(found.node, ChannelSel::A, &mut out.diagnostics)
                }
                Some(found) => {
                    unrecognised(
                        &mut out.diagnostics,
                        format!(
                            "Subsurface Weight: diffuseTransmissionTexture is read in A; \"{}\" feeds another channel (texture not exported)",
                            found.node.name
                        ),
                    );
                    None
                }
                None => {
                    if let Src::Linked(n, _) = src {
                        unrecognised(
                            &mut out.diagnostics,
                            format!(
                                "Subsurface Weight: unsupported node setup at \"{}\" ({}); not exported",
                                n.name, n.bl_idname
                            ),
                        );
                    }
                    None
                }
            };
            Channel {
                factor: factor_f32(cx, src).unwrap_or(1.0),
                texture,
            }
        }
    };
    let csrc = cx.src_at(p, ci);
    let color = match konst_rgb(cx, csrc) {
        Some(c) => Channel {
            factor: c,
            texture: None,
        },
        None => Channel {
            factor: factor_rgb(cx, csrc).unwrap_or([1.0; 3]),
            texture: texture_behind(
                cx,
                p,
                ci,
                TexKind::Full(ChannelSel::Rgb),
                &mut out.diagnostics,
            ),
        },
    };
    out.ext.diffuse_transmission = Some(DiffuseTransmission { factor, color });
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let (Some(d), Some(bsdf)) = (&lm.ext.diffuse_transmission, cx.bsdf) else {
        return;
    };
    cx.set_input(bsdf, "Thin Wall", SocketValue::Bool(true));
    let socket = cx.input(bsdf, "Subsurface Weight");
    let loc = cx.location(BLOCK);
    scalar_factor_and_texture(
        cx,
        loc,
        "Diffuse Transmission",
        socket,
        d.factor.factor,
        d.factor.texture.as_ref(),
        4,
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "diffuse_transmission",
    order: 390,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 170,
    key: BLOCK,
    takes_space: |lm| {
        lm.ext
            .diffuse_transmission
            .as_ref()
            .is_some_and(|d| d.factor.texture.is_some())
    },
};

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use frap_nodes::{NodeTree, SocketValue};
    use frap_rna::{IdKey, IdRef, ImageId, Value};
    use frap_scene::id::{Image, Material};

    use crate::*;

    #[derive(Default)]
    struct Images(HashMap<IdRef<ImageId>, Image>);

    impl ImageLookup for Images {
        fn image(&self, im: IdRef<ImageId>) -> Option<&Image> {
            self.0.get(&im)
        }
    }

    fn mat() -> Material {
        let mut m = Material::new("M");
        m.node_tree = NodeTree::default_material();
        m
    }

    const BSDF: u32 = 0;

    fn set(m: &mut Material, name: &str, v: SocketValue) {
        m.node_tree
            .node_mut(BSDF)
            .unwrap()
            .input_mut(name)
            .unwrap()
            .default_value = v;
    }

    fn input(m: &Material, name: &str) -> SocketValue {
        m.node_tree
            .first_of_type("ShaderNodeBsdfPrincipled")
            .unwrap()
            .input(name)
            .unwrap()
            .default_value
            .clone()
    }

    fn raised(lm: &LoweredMaterial) -> Material {
        let mut m = Material::new("M");
        raise(lm, &mut m, &mut NameAlloc::default());
        m
    }

    fn image_node(m: &mut Material, img: IdRef<ImageId>) -> u32 {
        let i = m.node_tree.new_node("ShaderNodeTexImage").unwrap();
        let tex = m.node_tree.nodes[i].uid;
        m.node_tree
            .node_mut(tex)
            .unwrap()
            .set_prop("image", Value::Id(Some(img.any())));
        tex
    }

    #[test]
    fn needs_thin_wall_and_a_weight() {
        let images = Images::default();
        let mut m = mat();
        // Factory material: no thin wall.
        assert!(
            lower(&m.node_tree, &m, &images)
                .ext
                .diffuse_transmission
                .is_none()
        );
        set(&mut m, "Subsurface Weight", SocketValue::Float(0.4));
        assert!(
            lower(&m.node_tree, &m, &images)
                .ext
                .diffuse_transmission
                .is_none()
        );
        set(&mut m, "Thin Wall", SocketValue::Bool(true));
        set(
            &mut m,
            "Base Color",
            SocketValue::Color([0.2, 0.5, 0.1, 1.0]),
        );
        let lm = lower(&m.node_tree, &m, &images);
        let d = lm.ext.diffuse_transmission.clone().unwrap();
        assert_eq!(d.factor.factor, 0.4);
        assert!(d.factor.texture.is_none());
        assert_eq!(d.color.factor, [0.2, 0.5, 0.1]);
        // A weight of 0 is no extension.
        set(&mut m, "Subsurface Weight", SocketValue::Float(0.0));
        assert!(
            lower(&m.node_tree, &m, &images)
                .ext
                .diffuse_transmission
                .is_none()
        );
        // Raise: thin wall on, subsurface weight = factor; lowering gives it back.
        let r = raised(&lm);
        assert_eq!(input(&r, "Thin Wall"), SocketValue::Bool(true));
        assert_eq!(input(&r, "Subsurface Weight"), SocketValue::Float(0.4));
        let again = lower(&r.node_tree, &r, &images);
        assert_eq!(again.ext.diffuse_transmission, lm.ext.diffuse_transmission);
    }

    #[test]
    fn weight_texture_is_read_in_alpha() {
        let mut images = Images::default();
        let img = IdRef::from_key(IdKey::next());
        images.0.insert(img, Image::new("mask", 1, 1));
        let mut m = mat();
        set(&mut m, "Thin Wall", SocketValue::Bool(true));
        let tex = image_node(&mut m, img);
        m.node_tree
            .link_by_name((tex, "Alpha"), (BSDF, "Subsurface Weight"))
            .unwrap();
        let lm = lower(&m.node_tree, &m, &images);
        let d = lm.ext.diffuse_transmission.clone().unwrap();
        assert_eq!(d.factor.factor, 1.0);
        let t = d.factor.texture.clone().unwrap();
        assert_eq!((t.image, t.channel), (img, ChannelSel::A));
        // Raised: the texture's Alpha drives Subsurface Weight; the law holds.
        let r = raised(&lm);
        let again = lower(&r.node_tree, &r, &images);
        assert_eq!(again.ext.diffuse_transmission, lm.ext.diffuse_transmission);
        // The Color output is another channel: factor only, with a WARNING.
        let mut m2 = mat();
        set(&mut m2, "Thin Wall", SocketValue::Bool(true));
        let tex = image_node(&mut m2, img);
        m2.node_tree
            .link_by_name((tex, "Color"), (BSDF, "Subsurface Weight"))
            .unwrap();
        let lm = lower(&m2.node_tree, &m2, &images);
        let d = lm.ext.diffuse_transmission.unwrap();
        assert!(d.factor.texture.is_none());
        assert!(
            lm.diagnostics.iter().any(|r| r
                .message
                .contains("diffuseTransmissionTexture is read in A")),
            "{:?}",
            lm.diagnostics
        );
    }
}
