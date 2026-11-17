// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context.evaluated_depsgraph_get()` (`rna_Context_evaluated_depsgraph_get` →
//! `CTX_data_ensure_evaluated_depsgraph`): an evaluation boundary (§7.3: pending animation is
//! applied and the depsgraph-lite updated), then the depsgraph of the context's view layer.
//! The pointer is the view layer's `depsgraph` (verified on 5.2.4:
//! `C.view_layer.depsgraph == C.evaluated_depsgraph_get()`), canonically
//! `bpy.data.scenes['Scene'].view_layers["ViewLayer"].depsgraph`.

use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::id::paths::view_layer_ptr;
use frap_scene::{FuncArgs, Host};

#[rna_func(srna = "Context", func = "evaluated_depsgraph_get")]
fn context_evaluated_depsgraph_get(
    h: &mut dyn Host,
    _this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    h.apply_pending_animation()?;
    let (host, ev) = h.eval();
    ev.depsgraph_update(host);
    let win = h.win();
    let p = view_layer_ptr(win.scene, &win.view_layer)
        .child(Seg::Prop("depsgraph"), &frap_spec_rna::DEPSGRAPH);
    Ok(Value::Ptr(Box::new(p)))
}
