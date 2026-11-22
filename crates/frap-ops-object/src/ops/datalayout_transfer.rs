// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.datalayout_transfer(modifier='', data_type='', use_delete=False,
//! layers_select_src='ACTIVE', layers_select_dst='ACTIVE')`, tier T2. Port of
//! `datalayout_transfer_exec` (`editors/object/object_data_transfer.cc`, Blender 5.2.4,
//! GPL-2.0-or-later) over `frap_modifiers::data_transfer::transfer_layout`
//! (`BKE_object_data_transfer_layout`, WP 8.3.13).
//!
//! With `modifier` naming a DATA_TRANSFER modifier of the active object ("Generate Data
//! Layers"): creates on the active object the layers that modifier needs from its source
//! object (its data types and layer selections; `CANCELLED` without a source). Otherwise:
//! creates on every other selected editable mesh object the layers of `data_type` of the active
//! object (`use_delete` also removes the destination layers the source does not have), each
//! mesh data once (the WARNING `Skipping object …, data … has already been processed with a
//! previous object`). An unset `data_type` creates nothing. As in Blender, the objects after the
//! active one in the selection order get nothing: the walk tags each selected object for an
//! update, and the tagged source has no evaluated mesh any more.
//!
//! Poll: an active mesh object (`edit_modifier_poll_generic` or `data_transfer_poll`).

use frap_modifiers::data_transfer::{modes, transfer_layout};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{IdRef, ObjectId};
use frap_scene::id::ModifierSettings;
use frap_spec_ops_object::object;

use super::data_transfer::{TRANSFER_POLL, destinations, layer_arrays, source, with_destination};

/// The data types and layer selections of the DATA_TRANSFER modifier `name` of `ob`, with its
/// source object (`edit_modifier_property_get(op, ob, eModifierType_DataTransfer)`).
#[allow(clippy::type_complexity)]
fn modifier_layout(
    cx: &OpCtx,
    ob: IdRef<ObjectId>,
    name: &str,
) -> Option<(
    Option<IdRef<ObjectId>>,
    u32,
    [i32; modes::DT_MULTILAYER_INDEX_MAX],
    [i32; modes::DT_MULTILAYER_INDEX_MAX],
)> {
    let o = cx.main().objects.get(ob)?;
    let m = o.modifiers.iter().find(|m| m.name == name)?;
    let ModifierSettings::DataTransfer(s) = &m.settings else {
        return None;
    };
    let types = frap_modifiers::types::data_transfer::data_types(s);
    let (src, dst) = frap_modifiers::types::data_transfer::layer_selects(s);
    Some((s.object.and_then(|o| o.typed::<ObjectId>()), types, src, dst))
}

#[operator(poll = TRANSFER_POLL, tier = T2)]
fn exec(cx: &mut OpCtx, p: object::DatalayoutTransfer) -> OpResult {
    let Some(ob_act) = cx.ctx().active_object() else {
        return Ok(OpStatus::Cancelled);
    };
    if let Some((src, types, layers_src, layers_dst)) =
        modifier_layout(cx, ob_act, &p.modifier.value)
    {
        let Some(ob_src) = src else {
            return Ok(OpStatus::Cancelled);
        };
        if super::data_transfer::mesh_of(cx, ob_src).is_none() {
            return Ok(OpStatus::Cancelled);
        }
        let src_eo = cx.evaluated(ob_src)?;
        let dst_eo = cx.evaluated(ob_act)?;
        let Some(source) = source(&cx.ctx(), ob_src, &src_eo) else {
            return Ok(OpStatus::Cancelled);
        };
        let m = dst_eo.matrix_world.to_cols_array_2d();
        with_destination(cx, ob_act, m, |d| {
            transfer_layout(types, false, &layers_src, &layers_dst, &source, d);
        })?;
        return Ok(OpStatus::Finished);
    }
    let data_type = if p.data_type.is_set {
        modes::data_type_bits(p.data_type.value.identifier())
    } else {
        0
    };
    let use_delete = p.use_delete.value;
    let layers_src = modes::layers_select_src(p.layers_select_src.value.identifier());
    let layers_dst = modes::layers_select_dst(p.layers_select_dst.value.identifier());
    let (fromlayers, tolayers) = layer_arrays(data_type, layers_src, layers_dst);
    if super::data_transfer::mesh_of(cx, ob_act).is_none() {
        return Ok(OpStatus::Cancelled);
    }
    let src_eo = cx.evaluated(ob_act)?;
    let objects = cx.ctx().selected_editable_objects();
    // Blender tags every selected object for an update while it walks them; once the source
    // is tagged, `BKE_object_get_evaluated_mesh` of its evaluated object is null and the
    // objects after it in the selection get no layers.
    let src_pos = objects
        .iter()
        .position(|&o| o == ob_act)
        .unwrap_or(objects.len());
    let (dsts, _) = destinations(cx, ob_act, &objects);
    for ob in dsts {
        if objects
            .iter()
            .position(|&o| o == ob)
            .is_some_and(|i| i > src_pos)
        {
            continue;
        }
        let dst_eo = cx.evaluated(ob)?;
        let Some(source) = source(&cx.ctx(), ob_act, &src_eo) else {
            continue;
        };
        let m = dst_eo.matrix_world.to_cols_array_2d();
        with_destination(cx, ob, m, |d| {
            transfer_layout(data_type, use_delete, &fromlayers, &tolayers, &source, d);
        })?;
    }
    Ok(OpStatus::Finished)
}
