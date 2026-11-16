// SPDX-License-Identifier: GPL-3.0-or-later
//! `view_layer.objects.selected` (`LayerObjects.selected`): the selected bases of the view
//! layer in base order (`rna_ViewLayer_objects_selected_skip` tests only `BASE_SELECTED`).
//! Verified on 5.2.4: it differs from `context.selected_objects` for an object under a layer
//! collection with `hide_viewport`, which stays listed here.

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::idlist::IdList;
use crate::id::paths::view_layer;

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "LayerObjects",
    prop: "selected",
    coll: true,
};

#[rna_hook(srna = "LayerObjects", prop = "selected", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let (_, _, vl) = view_layer(main, this)?;
    let sel = vl
        .bases
        .iter()
        .filter(|b| b.selected())
        .map(|b| b.object)
        .collect();
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(sel)))))
}
