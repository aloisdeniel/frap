// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.select_get(view_layer=None)`: the base's selection flag; False for an object without a base in the view layer (verified, no error). The flag is kept while a layer collection hides the object.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};

use crate::id::paths::{object_of, view_layer_arg};
use crate::{FuncArgs, Host};

#[rna_func(srna = "Object", func = "select_get", krate = crate)]
fn object_select_get(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    let ob = object_of(this)?;
    let win = (h.win().scene, h.win().view_layer.clone());
    let (scene, vl) = view_layer_arg(h.main(), args.get("view_layer"), (win.0, &win.1))?;
    let main = h.main();
    let Some(s) = main.scenes.get(scene) else {
        return Ok(Value::Bool(false));
    };
    let layer = &s.view_layers[vl];
    Ok(Value::Bool(layer.base(ob).is_some_and(|b| b.selected())))
}
