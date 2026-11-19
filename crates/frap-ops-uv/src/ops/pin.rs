// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.pin(clear=False, invert=False)`, tier T1 (`tests/golden/cases/uv/pin/*.py`).
//! Port of `uv_pin_exec` (`editors/uvedit/uvedit_ops.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvedit`. On every mesh in Edit Mode with a UV map and a selected vertex,
//! the selected UVs of visible faces are pinned in the active UV map's pin layer
//! (`.pn.<uv map>`, `uv_layers[...].pin[i].value`; created when missing): `clear` unpins them,
//! `invert` toggles them. With UV sync selection (the default) a corner counts as selected
//! when its vertex is (vertex select mode), when an edge through it at the same UV location
//! is (edge mode), or when a face using it at the same UV location is (face mode). Pinned UVs
//! stay in place in `uv.unwrap` and lock islands in `uv.pack_islands(pin=True)`. Always
//! finishes.

use std::sync::Arc;

use frap_bmesh::CdLayer;
use frap_mesh::AttrData;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;

use super::unwrap::uvedit::{self, UvOb, UvTs};

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: uv::Pin) -> OpResult {
    uvedit::check_uvedit(cx)?;
    let clear = p.clear.value;
    let invert = p.invert.value;
    let ts = UvTs::of(&cx.ctx());
    let objects = uvedit::edit_objects_with_uvs(cx);
    for o in UvOb::gather(cx, &objects) {
        let bm = o.bm.clone();
        if bm.selected_counts().0 == 0 {
            continue;
        }
        let pin_name = uvedit::pin_name(&o.uv_name);
        if clear && bm.ldata.find(&pin_name).is_none() {
            continue;
        }
        let sel = o.sel(ts);
        let mut pins = o.pins.clone();
        let mut changed = false;
        for f in uvedit::faces(&bm) {
            if !sel.face_visible(f) {
                continue;
            }
            for l in bm.face_loops(f) {
                if sel.uv_select(l) {
                    changed = true;
                    let v = &mut pins[l.slot()];
                    *v = if invert { !*v } else { !clear };
                }
            }
        }
        // `BM_uv_map_attr_pin_ensure_named`, then the values.
        let bm = cx.bmesh_mut(o.me)?;
        match bm.ldata.find(&pin_name) {
            Some(i) => {
                if changed {
                    bm.ldata.layers[i].data = AttrData::Bool(Arc::new(pins));
                }
            }
            None => {
                bm.ldata.add_layer(CdLayer {
                    name: pin_name,
                    data: AttrData::Bool(Arc::new(pins)),
                });
            }
        }
    }
    Ok(OpStatus::Finished)
}
