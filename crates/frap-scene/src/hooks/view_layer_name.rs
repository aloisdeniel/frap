// SPDX-License-Identifier: GPL-3.0-or-later
//! `ViewLayer.name`: unique within the scene (sub-data name, 63 bytes, `BLI_uniquename`); a
//! window showing the view layer follows the rename (verified on 5.2.4: renaming to a taken
//! name gives `VL.001`; `window.view_layer` reprs the new name).

use frap_rna::naming::unique_subname;
use frap_rna::{PropDef, RnaError, RnaPtr, Value, rna_hook};
use smol_str::SmolStr;

use crate::Host;
use crate::id::paths::view_layer_of;

#[rna_hook(srna = "ViewLayer", prop = "name", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let requested = v.as_str().unwrap_or_default().to_owned();
    let (scene, i, _) = view_layer_of(h.main(), this)?;
    let (main, log) = h.main_mut();
    let Some(s) = main.scenes.get(scene) else {
        return Ok(());
    };
    let old = s.view_layers[i].name.clone();
    if old == requested {
        return Ok(());
    }
    let others: Vec<SmolStr> = s
        .view_layers
        .iter()
        .enumerate()
        .filter(|(j, _)| *j != i)
        .map(|(_, v)| v.name.clone())
        .collect();
    let name = unique_subname(
        &|n| others.iter().any(|o| o == n),
        &requested,
        63,
        "ViewLayer",
    );
    if let Some(s) = main.scenes.get_mut(scene, log) {
        s.view_layers[i].name = name.clone();
    }
    let win = h.win_mut();
    if win.scene == scene && win.view_layer == old {
        win.view_layer = name;
    }
    Ok(())
}
