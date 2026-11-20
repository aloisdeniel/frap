// SPDX-License-Identifier: GPL-3.0-or-later
//! `id.animation_data_clear()` (`rna_ID_animation_data_free` → `BKE_animdata_free`, Blender
//! 5.2.4): removes the datablock's `AnimData` (its action, NLA tracks and settings); the action
//! loses that user. Verified on 5.2.4: `animation_data` reads `None` afterwards.

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::adt_slot_mut;

#[rna_func(srna = "ID", func = "animation_data_clear")]
fn i_d_animation_data_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    if !this.path.is_empty() {
        return Ok(Value::None);
    }
    let (main, log) = h.main_mut();
    if let Some(adt) = adt_slot_mut(main, log, this.root) {
        *adt = None;
    }
    Ok(Value::None)
}
