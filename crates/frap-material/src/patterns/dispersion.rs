// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_dispersion`: the "glTF Material Output" group's `Dispersion` → `dispersion`
//! (ARCHITECTURE §16.2, WP 3.6).
//!
//! Lowering ports `export_dispersion` (`io_scene_gltf2/blender/exp/material/extensions/
//! dispersion.py`, glTF-Blender-IO 5.2.41, Apache-2.0): the value of the first group input
//! called `Dispersion` (its own value, even when linked); `None` without one. The exporter
//! writes it only with `KHR_materials_volume` and not at 0.
//!
//! Raising ports `dispersion()` (`imp/pbrMetallicRoughness.py`): the value goes into the
//! settings node's `Dispersion` (the `volume` pattern created the node for a non-zero value).
//! The importer raises the volume's thickness right after, so this pattern then calls
//! `volume::raise_thickness`.

use frap_nodes::SocketValue;

use super::transmission::ext_common::{default_f32, find_settings_node, settings_input};
use crate::{LowerCx, LoweredMaterial, MaterialPattern, RaiseCx};

fn lower(cx: &LowerCx, out: &mut LoweredMaterial) {
    out.ext.dispersion = settings_input(cx, "Dispersion").and_then(|(g, i)| default_f32(g, i));
}

fn raise(lm: &LoweredMaterial, cx: &mut RaiseCx) {
    if let Some(d) = lm.ext.dispersion
        && let Some(settings) = find_settings_node(cx)
    {
        cx.set_input(settings, "Dispersion", SocketValue::Float(d));
    }
    super::volume::raise_thickness(lm, cx);
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::MATERIAL_PATTERNS)]
static PATTERN: MaterialPattern = MaterialPattern {
    name: "dispersion",
    order: 320,
    lower,
    raise: Some(raise),
};
