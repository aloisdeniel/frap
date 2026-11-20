// SPDX-License-Identifier: GPL-3.0-or-later
//! `id.animation_data_create()` (`rna_ID_animation_data_create` → `BKE_animdata_ensure_id`,
//! Blender 5.2.4): the datablock's `AnimData`, created when missing (no action, `use_nla`,
//! influence 1, REPLACE, HOLD; verified on 5.2.4). `None` for datablocks that cannot be
//! animated. Meshes are not animatable in frap yet (`NotImplementedError`).

use frap_rna::{IdCode, PyExc, RnaError, RnaErrorKind, RnaPtr, Root, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_action_api::adt_slot_mut;

#[rna_func(srna = "ID", func = "animation_data_create")]
fn i_d_animation_data_create(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    if !this.path.is_empty() {
        return Ok(Value::None);
    }
    let (main, log) = h.main_mut();
    match adt_slot_mut(main, log, this.root) {
        Some(adt) => {
            adt.get_or_insert_with(Default::default);
            Ok(Value::Ptr(Box::new(
                this.child(Seg::Prop("animation_data"), &frap_spec_rna::ANIM_DATA),
            )))
        }
        None if matches!(this.root, Root::Id(a) if a.code == IdCode::ME) => Err(RnaError::new(
            RnaErrorKind::NotImplemented,
            PyExc::NotImplementedError,
            "frap: mesh animation data is not implemented yet",
        )),
        // `id_can_have_animdata` is false: collections, libraries, actions.
        None => Ok(Value::None),
    }
}
