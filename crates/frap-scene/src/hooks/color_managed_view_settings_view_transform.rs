// SPDX-License-Identifier: GPL-3.0-or-later
//! `ColorManagedViewSettings.view_transform` (`rna_ColorManagedViewSettings_view_transform_itemf`
//! / `_set`): the items are the views of the sibling `display_settings.display_device`
//! (`frap_color::views::display_views`), so a view that display does not offer is Blender's enum
//! `TypeError` listing the display's views; an offered view is written through the struct
//! binding (`color_view.rs`: the look remap).
//!
//! Verified on 5.2.4: on `Display P3`, `view_transform = 'Filmic'` raises
//! `TypeError: bpy_struct: item.attr = val: enum "Filmic" not found in ('Standard', 'ACES 1.3',
//! 'ACES 2.0', 'AgX', 'False Color', 'Raw')`, for `scene.view_settings` and for
//! `render.image_settings.view_settings` (its own `display_settings`); on `Rec.2100-PQ`,
//! `'AgX - HDR 1000 nits'` is accepted.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_color.cc, Blender 5.2.4,
//! GPL-2.0-or-later.

use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::color_view::{check_view, set_through, sibling_display};

/// `ColorManagedViewSettings` has hand-written `Rna`/`RnaMut` impls (`color_view.rs`), so its
/// hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ColorManagedViewSettings",
    prop: "view_transform",
    coll: false,
};

#[rna_hook(srna = "ColorManagedViewSettings", prop = "view_transform", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    if let Value::Enum(name) | Value::Str(name) = &v {
        let display = sibling_display(h.main(), this);
        check_view(display.as_deref(), name)?;
    }
    let (main, log) = h.main_mut();
    set_through(main, log, this, prop, v)
}
