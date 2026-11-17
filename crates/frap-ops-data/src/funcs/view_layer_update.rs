// SPDX-License-Identifier: GPL-3.0-or-later
//! `view_layer.update()` (`rna_ViewLayer_update_tagged`): an evaluation boundary (§7.3):
//! pending animation (a `frame_current` change) is applied to the originals and the
//! depsgraph-lite is updated. Derived reads are always fresh in frap (D-04), so this is the
//! only visible effect.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

#[rna_func(srna = "ViewLayer", func = "update")]
fn view_layer_update(
    h: &mut dyn Host,
    _this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    h.apply_pending_animation()?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    Ok(Value::None)
}
