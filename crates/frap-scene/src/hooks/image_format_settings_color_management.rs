// SPDX-License-Identifier: GPL-3.0-or-later
//! `ImageFormatSettings.color_management` (`rna_ImageFormatSettings_color_management_set`):
//! switching a scene's output settings to `OVERRIDE` copies the scene's colour management into
//! them (`BKE_image_format_color_management_copy_from_scene`): `view_settings` (view, look,
//! exposure, gamma, curve mapping, white balance), `display_settings`, and
//! `linear_colorspace_settings` set to the scene-linear role. Setting the current value again
//! copies nothing.
//!
//! Verified on 5.2.4: scene exposure 0.7, look `AgX - Base Contrast`, gamma 1.3, white balance
//! 4000 K, then `OVERRIDE`: `image_settings.view_settings` reads AgX / `AgX - Base Contrast` /
//! 0.7 / 1.3 / 4000 K, `linear_colorspace_settings.name` `Linear Rec.709` (was `ACEScg`);
//! `OVERRIDE` again after scene exposure 2 keeps 0.7; `FOLLOW_SCENE` then `OVERRIDE` reads 2.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_scene.cc and
//! blender/source/blender/blenkernel/intern/image_format.cc, Blender 5.2.4, GPL-2.0-or-later.

use frap_rna::{PropDef, RnaError, RnaMut, RnaPtr, Root, Seg, Value, rna_hook};
use smol_str::SmolStr;

use crate::Host;
use crate::generated::plain::{
    ColorManagedDisplaySettings, ColorManagedViewSettings, ImageFormatSettings,
};
use crate::id::Scene;

/// `ImageFormatSettings` is a hand-written plain binding (`image_format.rs`, no `rna_struct!`),
/// so its hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ImageFormatSettings",
    prop: "color_management",
    coll: false,
};

/// The output settings of `scene` at `path` (below the scene).
fn settings_mut<'a>(scene: &'a mut Scene, path: &[Seg]) -> Option<&'a mut ImageFormatSettings> {
    match path {
        [Seg::Prop("render"), Seg::Prop("image_settings")] => {
            Some(&mut scene.render.image_settings)
        }
        _ => None,
    }
}

/// `BKE_image_format_color_management_copy_from_scene`, given the scene's settings.
pub fn copy_from_scene(
    imf: &mut ImageFormatSettings,
    view: ColorManagedViewSettings,
    display: ColorManagedDisplaySettings,
) {
    imf.display_settings = display;
    imf.view_settings = view;
    imf.linear_colorspace_settings.name =
        SmolStr::new_static(frap_color::spaces::ColorSpace::LinearRec709.name());
}

#[rna_hook(srna = "ImageFormatSettings", prop = "color_management", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (main, log) = h.main_mut();
    if let Root::Id(a) = this.root
        && let Some(sc) = frap_rna::IdRef::from_any(a)
        && let Some(scene) = main.scenes.get_mut(sc, log)
    {
        let (view, display) = (scene.view_settings.clone(), scene.display_settings.clone());
        if let Some(imf) = settings_mut(scene, &this.path) {
            let was = imf.color_management.clone();
            imf.set(prop, v)?;
            if imf.color_management != was && imf.color_management == "OVERRIDE" {
                copy_from_scene(imf, view, display);
            }
            return Ok(());
        }
    }
    crate::color_view::set_through(main, log, this, prop, v)
}
