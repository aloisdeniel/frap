// SPDX-License-Identifier: GPL-3.0-or-later
//! `Collection.hide_viewport`: hidden in every viewport: its objects become invisible and unselectable, so they are deselected (verified on 5.2.4); the view layers are resynced.

use frap_rna::{PropDef, RnaError, RnaPtr, Root, SceneId, Value, rna_hook};

use crate::Host;
use crate::id::layers::sync_all;

#[rna_hook(srna = "Collection", prop = "hide_viewport", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let Some(b) = v.as_bool() else {
        return Ok(());
    };
    let (main, log) = h.main_mut();
    match this.root {
        Root::Id(a) => {
            if let Some(c) = a.typed().and_then(|r| main.collections.get_mut(r, log)) {
                c.hide_viewport = b;
            }
        }
        Root::Embedded { owner, .. } => {
            if let Some(s) = owner
                .typed::<SceneId>()
                .and_then(|r| main.scenes.get_mut(r, log))
            {
                s.collection.hide_viewport = b;
            }
        }
        _ => {}
    }
    sync_all(main, log);
    Ok(())
}
