// SPDX-License-Identifier: GPL-3.0-or-later
//! Double-sidedness: `doubleSided = not Material.use_backface_culling`
//! (`__gather_double_sided`, `io_scene_gltf2/blender/exp/material/materials.py`) and back
//! (`set_double_sided`, `imp/material.py`; glTF-Blender-IO 5.2.41, Apache-2.0). The
//! `KHR_materials_volume` override (`doubleSided: false` whenever the volume extension is
//! written) is the exporter's (`frap-io-gltf` `export/materials/mod.rs`), which knows which
//! extensions it wrote.

use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    out.double_sided = !cx.mat.use_backface_culling;
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    cx.into.use_backface_culling = !lm.double_sided;
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "double_sided",
    order: 50,
    lower,
    raise: Some(raise),
};
