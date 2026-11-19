// SPDX-License-Identifier: GPL-3.0-or-later
//! `KHR_materials_iridescence` export (WP 3.6), ported from
//! `io_scene_gltf2/blender/exp/material/extensions/iridescence.py` and `export.py`'s
//! `__check_iridescence` (glTF-Blender-IO 5.2.41, Apache-2.0):
//! - `iridescenceIor` when it differs from 1.3 by more than 1e-4, `iridescenceFactor` when not
//!   0, `iridescenceTexture`;
//! - with a thickness texture: `iridescenceThicknessMaximum` / `Minimum` when they differ from
//!   400 / 100 by more than 1e-4, and the texture; without one, only the maximum;
//! - the extension is dropped when the factor or the maximum thickness is 0 (written or
//!   default; frap writes no animation, so nothing keeps it). Its textures were gathered
//!   before, as in the add-on.

use frap_gltf::Extension;
use frap_gltf::ext::khr_materials_iridescence::KhrMaterialsIridescence;
use frap_gltf::json::Material as GMaterial;
use frap_material::LoweredMaterial;
use frap_material::patterns::iridescence::{THICKNESS_MAXIMUM, THICKNESS_MINIMUM};

use crate::export::ExportError;
use crate::export::materials::num;
use crate::material_ext::{ExtExportCx, MaterialExtExporter};

/// `abs(v - default) > 0.0001`, with the default as the Python float.
fn differs(v: f32, default: f64) -> bool {
    (f64::from(v) - default).abs() > 0.0001
}

fn run(
    cx: &mut ExtExportCx<'_, '_>,
    lm: &LoweredMaterial,
    m: &mut GMaterial,
) -> Result<(), ExportError> {
    // `__export_unlit` returns before `__gather_extensions`.
    if lm.unlit {
        return Ok(());
    }
    let Some(ir) = &lm.ext.iridescence else {
        return Ok(());
    };
    let mut e = KhrMaterialsIridescence::default();
    // `GLTF_IRIDESCENCE_IOR` is the Python float 1.3, not its f32 rounding.
    if differs(ir.ior, 1.3) {
        e.iridescence_ior = Some(num(ir.ior));
    }
    if ir.factor.factor != 0.0 {
        e.iridescence_factor = Some(num(ir.factor.factor));
    }
    if let Some(t) = &ir.factor.texture {
        e.iridescence_texture = cx.texture_info(t, "iridescenceTexture")?;
    }
    if differs(ir.thickness_max.factor, f64::from(THICKNESS_MAXIMUM)) {
        e.iridescence_thickness_maximum = Some(num(ir.thickness_max.factor));
    }
    if let Some(t) = &ir.thickness_max.texture {
        if differs(ir.thickness_min, f64::from(THICKNESS_MINIMUM)) {
            e.iridescence_thickness_minimum = Some(num(ir.thickness_min));
        }
        e.iridescence_thickness_texture = cx.texture_info(t, "iridescenceThicknessTexture")?;
    }
    // `__check_iridescence`.
    if e.iridescence_factor_or_default() == 0.0
        || e.iridescence_thickness_maximum_or_default() == 0.0
    {
        return Ok(());
    }
    m.extensions.set(Some(e));
    cx.use_extension(KhrMaterialsIridescence::NAME, false);
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::material_ext::MATERIAL_EXT_EXPORTERS)]
static EXPORTER: MaterialExtExporter = MaterialExtExporter {
    name: KhrMaterialsIridescence::NAME,
    order: 70,
    run,
};
