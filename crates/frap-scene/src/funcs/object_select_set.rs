// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.select_set(state, view_layer=None)`: selects or deselects the object's base in the view layer (see `frap_scene::id::layers::select_set` for the verified rules).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};

use crate::id::layers::select_set;
use crate::id::paths::{object_of, view_layer_arg};
use crate::{FuncArgs, Host};

#[rna_func(srna = "Object", func = "select_set", krate = crate)]
fn object_select_set(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    let ob = object_of(this)?;
    let state = args.get("state").and_then(Value::as_bool).unwrap_or(false);
    let win = (h.win().scene, h.win().view_layer.clone());
    let (scene, vl) = view_layer_arg(h.main(), args.get("view_layer"), (win.0, &win.1))?;
    let (main, log) = h.main_mut();
    select_set(main, log, scene, vl, ob, state)?;
    Ok(Value::None)
}
