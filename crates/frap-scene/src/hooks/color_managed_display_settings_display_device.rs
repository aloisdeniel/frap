// SPDX-License-Identifier: GPL-3.0-or-later
//! `ColorManagedDisplaySettings.display_device` (`rna_ColorManagedDisplaySettings_display_device_update`):
//! changing a scene's own `display_settings` validates the scene's `view_settings` against the
//! new display (`IMB_colormanagement_validate_settings`): a view the display does not offer
//! becomes the display's view of the same family, else `Standard`, with the look remap of the
//! view setter (`frap_color::views::validate_for_display`). Other owners
//! (`render.image_settings.display_settings`) keep their view.
//!
//! Verified on 5.2.4: `Filmic` / `Medium High Contrast`, then `display_device = 'Display P3'`
//! reads `Standard` / `Medium High Contrast`; `Khronos PBR Neutral` → `Rec.2020` reads
//! `Standard`; `AgX` / `AgX - Punchy` → `Rec.2100-PQ` reads `AgX - HDR 1000 nits` /
//! `AgX - Punchy`; `ACES 2.0` → `Rec.2100-PQ` reads `ACES 2.0 - HDR 500 nits`; exposure is kept;
//! `render.image_settings` (`OVERRIDE`) on `Filmic`, display `Display P3`: the view is kept.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_color.cc and
//! blender/source/blender/imbuf/intern/colormanagement.cc, Blender 5.2.4, GPL-2.0-or-later.

use frap_rna::{PropDef, RnaError, RnaPtr, Root, Seg, Value, rna_hook};

use crate::Host;
use crate::color_view::{set_through, validate_for_display};

/// `ColorManagedDisplaySettings` is a generated plain binding without hooked props, so its
/// hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ColorManagedDisplaySettings",
    prop: "display_device",
    coll: false,
};

#[rna_hook(srna = "ColorManagedDisplaySettings", prop = "display_device", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let (main, log) = h.main_mut();
    set_through(main, log, this, prop, v)?;
    if let Root::Id(a) = this.root
        && matches!(this.path.as_slice(), [Seg::Prop("display_settings")])
        && let Some(sc) = frap_rna::IdRef::<frap_rna::SceneId>::from_any(a)
        && let Some(scene) = main.scenes.get(sc)
        && frap_color::views::validate_for_display(
            &scene.display_settings.display_device,
            &scene.view_settings.view_transform,
        )
        .is_some_and(|v| v != scene.view_settings.view_transform)
        && let Some(scene) = main.scenes.get_mut(sc, log)
    {
        let display = scene.display_settings.display_device.clone();
        validate_for_display(&mut scene.view_settings, &display);
    }
    Ok(())
}
