// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.material_slot_select()`, tier T1 (`tests/golden/cases/edit/select/
//! material_slot.py`). Port of `material_slot_select_exec`, `material_slot_de_select`,
//! `object_array_for_shading_edit_mode_enabled` (`editors/render/render_shading.cc`),
//! `objects_in_mode_or_selected` (`editors/object/object_edit.cc`),
//! `BKE_object_material_get` / `BKE_object_material_index_get_with_hint`
//! (`blenkernel/intern/material.cc`) and `EDBM_deselect_by_material`
//! (`editors/mesh/editmesh_select.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! In Edit Mode, selects the visible faces (with their edges and vertices) that use the
//! material of the active object's active slot, on every mesh in Edit Mode that has that
//! material (the active object's slot index first, else the first slot holding it). The
//! selection is not flushed. Without such faces, or outside Edit Mode, the operator is
//! `CANCELLED` (it has no poll, as in Blender).

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

#[operator(poll = poll::ALWAYS, tier = T1)]
fn exec(cx: &mut OpCtx, _p: object::MaterialSlotSelect) -> OpResult {
    common::de_select(cx, true)
}

/// The shared body of `object.material_slot_select` / `material_slot_deselect`.
pub(crate) mod common {
    use frap_bmesh::{BMesh, ElemFlag};
    use frap_op::prelude::*;
    use frap_rna::{IdRef, MaterialId, MeshId, ObjectId};
    use frap_scene::id::{Object, slots};
    use frap_scene::{Context, Main, ObjectMode};

    use super::super::material_slot_add::common::{actcol, materials_supported};

    /// `BKE_object_material_get(ob, act)`: the material of slot `act` (1-based), clamped to
    /// the slots; `None` without slots or for an empty slot.
    pub(crate) fn material_get(main: &Main, ob: &Object, act: i32) -> Option<IdRef<MaterialId>> {
        let n = slots::slot_count(main, ob) as i32;
        if n == 0 {
            return None;
        }
        let i = (act - 1).clamp(0, n - 1) as usize;
        slots::slot(main, ob, i).1
    }

    /// `BKE_object_material_index_get_with_hint`: the 0-based slot holding `ma`, `hint` first.
    pub(crate) fn index_with_hint(
        main: &Main,
        ob: &Object,
        ma: Option<IdRef<MaterialId>>,
        hint: i32,
    ) -> Option<usize> {
        let n = slots::slot_count(main, ob) as i32;
        if hint >= 0 && hint < n && material_get(main, ob, hint + 1) == ma {
            return Some(hint as usize);
        }
        (0..n)
            .find(|&i| material_get(main, ob, i + 1) == ma)
            .map(|i| i as usize)
    }

    /// `object_array_for_shading_edit_mode_enabled`: the objects in the active object's mode
    /// (each data once) when it is in Edit Mode, else the selected objects (each data once);
    /// only objects in Edit Mode whose materials can be edited.
    fn shading_edit_objects(c: &Context) -> Vec<IdRef<ObjectId>> {
        let active_in_mode = poll::active(c).is_some_and(|(_, o)| o.mode == ObjectMode::Edit);
        let candidates = if active_in_mode {
            c.objects_in_mode_unique_data()
        } else {
            let mut seen = Vec::new();
            c.selected_objects()
                .into_iter()
                .filter(|r| {
                    let d = c.main.objects.get(*r).and_then(|o| o.data);
                    if seen.contains(&d) {
                        return false;
                    }
                    seen.push(d);
                    true
                })
                .collect()
        };
        candidates
            .into_iter()
            .filter(|r| {
                c.main
                    .objects
                    .get(*r)
                    .is_some_and(|o| o.mode == ObjectMode::Edit && materials_supported(c, o))
            })
            .collect()
    }

    // Ported from blender/source/blender/editors/mesh/editmesh_select.cc
    // (EDBM_deselect_by_material).
    /// `EDBM_deselect_by_material`: (de)selects the visible faces of material `index`.
    pub(crate) fn deselect_by_material(bm: &mut BMesh, index: i16, select: bool) -> bool {
        let mut changed = false;
        for f in bm.face_ids() {
            if bm.face(f).flag.contains(ElemFlag::HIDDEN) {
                continue;
            }
            if bm.face(f).mat_nr == index {
                changed = true;
                bm.face_select_set(f, select);
            }
        }
        changed
    }

    /// `material_slot_de_select(C, select)`.
    pub(crate) fn de_select(cx: &mut OpCtx, select: bool) -> OpResult {
        let work: Vec<(IdRef<MeshId>, i16)> = {
            let c = cx.ctx();
            let obact = poll::active(&c).map(|(_, o)| o);
            let (mat_active, hint) = match obact {
                Some(o) => {
                    let act = actcol(o, slots::slot_count(c.main, o)) as i32;
                    (material_get(c.main, o, act), act - 1)
                }
                None => (None, -1),
            };
            shading_edit_objects(&c)
                .into_iter()
                .filter_map(|r| {
                    let ob = c.main.objects.get(r)?;
                    if slots::slot_count(c.main, ob) == 0 {
                        return None;
                    }
                    let idx = index_with_hint(c.main, ob, mat_active, hint)?;
                    let me = ob.data?.typed::<MeshId>()?;
                    Some((me, idx as i16))
                })
                .collect()
        };
        let mut changed_multi = false;
        for (me, idx) in work {
            let has_edit = {
                let (_, edit) = cx.split_edit();
                edit.meshes.contains_key(&me.key())
            };
            if !has_edit {
                continue;
            }
            if deselect_by_material(cx.bmesh_mut(me)?, idx, select) {
                changed_multi = true;
            }
        }
        Ok(if changed_multi {
            OpStatus::Finished
        } else {
            OpStatus::Cancelled
        })
    }
}
