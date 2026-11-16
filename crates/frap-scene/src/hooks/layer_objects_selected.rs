// SPDX-License-Identifier: GPL-3.0-or-later
//! `view_layer.objects.selected` (`LayerObjects.selected`): the selected, visible objects of
//! the view layer in base order (verified: equal to `context.selected_objects`).

use frap_rna::{CollRef, Field, PropDef, RnaError, RnaPtr, rna_hook};

use crate::HostRead;
use crate::id::idlist::IdList;
use crate::id::layers::visibility;
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
    let (scene, _, vl) = view_layer(main, this)?;
    let Some(s) = main.scenes.get(scene) else {
        return Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(Vec::new())))));
    };
    let vis = visibility(s, vl, &main.collections, &main.objects);
    let sel = vl
        .bases
        .iter()
        .filter(|b| b.selected() && vis.get(&b.object).is_some_and(|v| v.visible))
        .map(|b| b.object)
        .collect();
    Ok(Field::Coll(CollRef::Owned(Box::new(IdList::objects(sel)))))
}
