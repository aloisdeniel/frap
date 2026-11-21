// SPDX-License-Identifier: GPL-3.0-or-later
//! The spec-complete writer (WP 7.4): what io_scene_gltf2 5.2.41 cannot write, behind the
//! frap-only options of `export_scene.gltf` (ARCHITECTURE §17.2), each off by default so the
//! output stays Blender's:
//!
//! - `frap_compat='SPEC'` ([`compat`]): drops the add-on's quirks. Optional extensions are no
//!   longer listed in `extensionsRequired` (the add-on's `Extension(required=True)` default
//!   marks `KHR_texture_transform`, `KHR_lights_punctual` and the primitive's
//!   `KHR_materials_variants` required); animation times follow Blender's clock
//!   (`frame × fps_base / fps`) instead of the add-on's `frame / (fps × fps_base)`; and
//!   primitive indices use UNSIGNED_BYTE when every index is below 255 ([`layout`]; the add-on
//!   writes UNSIGNED_SHORT at least).
//! - `frap_interleave` ([`layout`]): the vertex attributes of each primitive (and of each
//!   morph target) share one ARRAY_BUFFER view with a `byteStride`.
//! - `frap_node_visibility` ([`visibility`]): `KHR_node_visibility` (`visible: false`) on the
//!   node of every object with `hide_render`.
//! - `frap_diffuse_transmission`: `KHR_materials_diffuse_transmission`, written by
//!   `materials/ext/diffuse_transmission.rs` from the Principled BSDF's thin-walled subsurface
//!   (`frap_material::patterns::diffuse_transmission`).
//!
//! The output of every option passes the Khronos glTF-Validator (tests:
//! `tests/spec_complete.rs`, which runs it when `FRAP_GLTF_VALIDATOR` names its install).
//!
//! `write::finish` calls [`finish`] once, on the gathered document and buffer, before the
//! buffer-level meshopt compression (WP 7.1), the extension order and the buffer are fixed.

pub mod compat;
pub mod layout;
#[cfg(test)]
mod tests;
pub mod visibility;

use frap_spec_ops_io::export_scene::gltf::FrapCompat;

use crate::export::ExportError;
use crate::export::gather::Gather;

/// The `export_scene.gltf` props this module honours (the operator stops warning about them).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "frap_compat",
    "frap_interleave",
    "frap_node_visibility",
    "frap_diffuse_transmission",
];

/// Whether `frap_compat='SPEC'`.
pub fn spec(g: &Gather) -> bool {
    g.settings.props.frap_compat.value == FrapCompat::Spec
}

/// Rewrites the gathered document for the spec-complete options.
pub fn finish(g: &mut Gather) -> Result<(), ExportError> {
    let spec = spec(g);
    if g.settings.props.frap_node_visibility.value {
        visibility::apply(g);
    }
    if spec {
        compat::animation_times(g)?;
    }
    layout::apply(g, g.settings.props.frap_interleave.value, spec)?;
    if spec {
        compat::extensions_required(g);
    }
    Ok(())
}
