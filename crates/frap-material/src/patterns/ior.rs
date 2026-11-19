// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_ior`: Principled BSDF `IOR` → `ior` (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_ior` (`io_scene_gltf2/blender/exp/material/extensions/ior.py`,
//! glTF-Blender-IO 5.2.41, Apache-2.0): the value of an unlinked `IOR` socket (a linked one is
//! not exported). Whether it is written (only with `KHR_materials_transmission`,
//! `KHR_materials_volume` or `KHR_materials_specular`, and not at 1.5) is the exporter's
//! decision [GLTF §4.5].
//!
//! Raising ports the end of `pbr_metallic_roughness()` (`imp/pbrMetallicRoughness.py`): `IOR`
//! = the extension's value, else 1.5 (the Principled BSDF's default, so nothing to do).

use frap_nodes::SocketValue;

use super::transmission::ext_common::konst_f32;
use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

/// `GLTF_IOR` (`io/com/constants.py`).
pub const GLTF_IOR: f32 = 1.5;

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    let Some(p) = cx.principled() else { return };
    out.ext.ior = p
        .input_index("IOR")
        .and_then(|i| konst_f32(cx, cx.src_at(p, i)));
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    let Some(bsdf) = cx.bsdf else { return };
    let ior = lm.ext.ior.unwrap_or(GLTF_IOR);
    cx.set_input(bsdf, "IOR", SocketValue::Float(ior));
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "ior",
    order: 380,
    lower,
    raise: Some(raise),
};
