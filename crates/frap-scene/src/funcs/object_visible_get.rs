// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.visible_get(view_layer=None)`: visible in the (virtual) viewport of the view layer: a base exists, it is not hidden, the object is not `hide_viewport`, and some collection path to it is neither a hidden collection nor a hidden layer collection (verified on 5.2.4). The `viewport` argument (local view) is ignored: frap has one virtual 3D View.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};

use crate::id::paths::{object_of, view_layer_arg};
use crate::{FuncArgs, Host};

#[rna_func(srna = "Object", func = "visible_get", krate = crate)]
fn object_visible_get(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let ob = object_of(this)?;
    let win = (h.win().scene, h.win().view_layer.clone());
    let (scene, vl) = view_layer_arg(h.main(), args.get("view_layer"), (win.0, &win.1))?;
    let main = h.main();
    let Some(s) = main.scenes.get(scene) else {
        return Ok(Value::Bool(false));
    };
    let layer = &s.view_layers[vl];
    let vis = crate::id::layers::visibility(s, layer, &main.collections, &main.objects);
    Ok(Value::Bool(vis.get(&ob).is_some_and(|v| v.visible)))
}
