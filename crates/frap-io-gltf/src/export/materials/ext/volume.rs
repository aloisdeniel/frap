// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_volume` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/volume.py` and the volume rules of
//! `materials.py`/`export.py` (glTF-Blender-IO 5.2.41, Apache-2.0):
//! - a material with a volume (even one dropped below) is written `doubleSided: false`:
//!   `__gather_double_sided` tests the gathered extensions before `__check_volume` runs;
//! - `attenuationColor` and a finite `attenuationDistance` come from the Volume Absorption node
//!   and are written only when there is one; `thicknessFactor` with a texture, or when not 0;
//!   `thicknessTexture`;
//! - the extension is put on the material here but declared in `extensionsUsed` only by the
//!   final checks of `ior.rs` (`__check_ior` still sees it; `__check_volume` then drops it
//!   without `KHR_materials_transmission`).

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_volume::KhrMaterialsVolume;
use frap_gltf::json::Material as GMaterial;
use frap_material::patterns::volume::absorption_node;
use frap_material::{LowerCx, LoweredMaterial};

use crate::export::ExportError;
use crate::export::materials::num;
use crate::material_ext::{ExtExportCx, MaterialExtExporter};

fn run(
    cx: &mut ExtExportCx<'_, '_>,
    lm: &LoweredMaterial,
    m: &mut GMaterial,
) -> Result<(), ExportError> {
    // `__export_unlit` returns before `__gather_extensions`.
    if lm.unlit {
        return Ok(());
    }
    let Some(v) = &lm.ext.volume else {
        return Ok(());
    };
    m.double_sided = Some(false);
    let main = cx.g.main;
    let absorption = main
        .materials
        .get(cx.material)
        .is_some_and(|mat| absorption_node(&LowerCx::new(&mat.node_tree, mat, main)).is_some());
    let mut e = KhrMaterialsVolume::default();
    if absorption {
        e.attenuation_color = Some(v.attenuation_color.map(num));
        e.attenuation_distance = v
            .attenuation_distance
            .is_finite()
            .then(|| num(v.attenuation_distance));
    }
    if v.thickness.texture.is_some() || v.thickness.factor != 0.0 {
        e.thickness_factor = Some(num(v.thickness.factor));
    }
    if let Some(t) = &v.thickness.texture {
        e.thickness_texture = cx.texture_info(t, "thicknessTexture")?;
    }
    if e != KhrMaterialsVolume::default() {
        m.extensions.set(Some(e));
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsVolume::NAME,
    order: 30,
    run,
};
