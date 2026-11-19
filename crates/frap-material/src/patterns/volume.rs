// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_volume`: the "glTF Material Output" group's `Thickness` and a Volume
//! Absorption node on the Material Output's `Volume` → `thicknessFactor`/`thicknessTexture`
//! (G), `attenuationDistance` (`1 / Density`) and `attenuationColor` (ARCHITECTURE §16.2,
//! WP 3.6).
//!
//! Lowering ports `export_volume` (`io_scene_gltf2/blender/exp/material/extensions/volume.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0):
//! - no extension without a group node that has a `Thickness` input, or when that input is
//!   unlinked and 0;
//! - `thicknessFactor`: the unlinked value, or the texture behind the input (packed into G,
//!   from G unless a Separate Color output names the channel) and its factor (1 without a
//!   constant multiplier); a socket linked to anything else gives no member (factor 0);
//! - the attenuation comes from the first non-muted Volume Absorption node linked to the
//!   active output: its `Color` and `1 / Density` (∞ for a density of 0), read from the
//!   sockets' own values even when they are linked. Without one the attenuation keeps the glTF
//!   defaults (white, ∞) and the exporter writes neither member ([`absorption_node`]).
//!
//! Raising ports `pbr_metallic_roughness()`'s node prelude and `volume()`
//! (`imp/pbrMetallicRoughness.py`). This pattern runs first (order 2), as the prelude does:
//! the settings node when anything needs it (`need_settings_node`: occlusion, a non-zero
//! thickness, dispersion or iridescence factor), and for a non-zero thickness factor the Volume
//! Absorption node (colour, density `1 / attenuationDistance`) on the output's `Volume`. The
//! thickness itself (`scalar_factor_and_texture` into the settings node's `Thickness`,
//! "Thickness", G) creates nodes whose names depend on their rank, so it is raised where the
//! importer raises it, after the dispersion (`raise_thickness`, called by `dispersion.rs`).

use frap_nodes::{Node, SocketValue};

use super::transmission::ext_common::{
    TexKind, default_f32, default_rgb, find_settings_node, konst_f32, needs_settings_node,
    node_linked_to_output, scalar_factor_and_texture, scalar_member, settings_input, settings_node,
    volume_node,
};
use crate::{Channel, ChannelSel, LowerCx, LoweredMaterial, MaterialPattern, RaiseCx, Volume};

/// The Volume Absorption node the exporter reads the attenuation from
/// (`get_socket(tree, 'Density', volume=True)`): the first non-muted one linked to the active
/// Material Output. The glTF exporter writes `attenuationColor` (and a finite
/// `attenuationDistance`) only when there is one.
pub fn absorption_node<'a>(cx: &LowerCx<'a>) -> Option<&'a Node> {
    node_linked_to_output(cx, "ShaderNodeVolumeAbsorption")
}

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some((g, ti)) = settings_input(cx, "Thickness") else {
        return;
    };
    if konst_f32(cx, cx.src_at(g, ti)) == Some(0.0) {
        return;
    }
    let thickness = scalar_member(
        cx,
        g,
        ti,
        TexKind::Packed(ChannelSel::G),
        "Thickness",
        &mut out.diagnostics,
    );
    let (mut color, mut distance) = ([1.0; 3], f32::INFINITY);
    if let Some(va) = absorption_node(cx) {
        if let Some(c) = va.input_index("Color").and_then(|i| default_rgb(va, i)) {
            color = c;
        }
        if let Some(d) = va.input_index("Density").and_then(|i| default_f32(va, i)) {
            distance = if d != 0.0 {
                (1.0 / f64::from(d)) as f32
            } else {
                f32::INFINITY
            };
        }
    }
    out.ext.volume = Some(Volume {
        thickness: Channel {
            factor: thickness.factor.unwrap_or(0.0),
            texture: thickness.texture,
        },
        attenuation_distance: distance,
        attenuation_color: color,
    });
}

/// The start of `pbr_metallic_roughness()`, right after the Principled BSDF and the Material
/// Output (so `tree.nodes` keeps the importer's order): the settings node when anything needs
/// it, then, for a non-zero thickness, the Volume Absorption node with its colour and density.
fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    if needs_settings_node(lm) {
        settings_node(cx);
    }
    let Some(v) = lm.ext.volume.as_ref().filter(|v| v.thickness.factor != 0.0) else {
        return;
    };
    let va = volume_node(cx, true);
    let [r, g, b] = v.attenuation_color;
    cx.set_input(va, "Color", SocketValue::Color([r, g, b, 1.0]));
    cx.set_input(
        va,
        "Density",
        SocketValue::Float((1.0 / f64::from(v.attenuation_distance)) as f32),
    );
}

/// `volume()`'s thickness, which the importer raises after `dispersion()` (called from
/// `dispersion.rs`, whose pattern runs at that point): `scalar_factor_and_texture` into the
/// settings node's `Thickness` ("Thickness", G), for a non-zero thickness only.
pub(crate) fn raise_thickness(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(v) = lm.ext.volume.as_ref().filter(|v| v.thickness.factor != 0.0) else {
        return;
    };
    let Some(settings) = find_settings_node(cx) else {
        return;
    };
    let socket = cx.input(settings, "Thickness");
    let loc = cx.location("volume_thickness");
    scalar_factor_and_texture(
        cx,
        loc,
        "Thickness",
        socket,
        v.thickness.factor,
        v.thickness.texture.as_ref(),
        1,
    );
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "volume",
    order: 2,
    lower,
    raise: Some(raise),
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::build::LAYOUT_BLOCKS)]
static LAYOUT: crate::build::LayoutBlock = crate::build::LayoutBlock {
    order: 160,
    key: "volume_thickness",
    takes_space: |lm| {
        lm.ext
            .volume
            .as_ref()
            .is_some_and(|v| v.thickness.texture.is_some())
    },
};
