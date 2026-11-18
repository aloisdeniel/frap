// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_face_by_sides(number=4, type='EQUAL', extend=True)`, tier T1
//! (`tests/golden/cases/edit/select/loose_sides.py`). Port of `edbm_select_face_by_sides_exec`
//! and `is_count_a_match` (`editors/mesh/editmesh_select.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! Per mesh in Edit Mode (after deselecting all unless `extend`), every visible face whose
//! number of sides is `LESS`, `EQUAL`, `GREATER` or `NOTEQUAL` to `number` is selected (with
//! its edges and vertices), then the selection is flushed.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, select_face_by_sides::Type};

use super::select_more::common;

/// `is_count_a_match`.
pub(crate) fn is_count_a_match(ty: Type, value_test: i64, value_reference: i64) -> bool {
    match ty {
        Type::Less => value_test < value_reference,
        Type::Equal => value_test == value_reference,
        Type::Greater => value_test > value_reference,
        Type::Notequal => value_test != value_reference,
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectFaceBySides) -> OpResult {
    let number = i64::from(p.number.value);
    let ty = p.r#type.value;
    for (_, me) in common::edit_meshes(cx) {
        let bm = cx.bmesh_mut(me)?;
        let mut changed = false;
        if !p.extend.value {
            common::deselect_all(bm);
            changed = true;
        }
        for f in bm.face_ids() {
            if common::hidden(bm, f) {
                continue;
            }
            if is_count_a_match(ty, i64::from(bm.face(f).len), number) {
                changed = true;
                bm.face_select_set(f, true);
            }
        }
        if changed {
            common::selectmode_flush(bm);
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comparisons() {
        assert!(is_count_a_match(Type::Less, 3, 4));
        assert!(!is_count_a_match(Type::Less, 4, 4));
        assert!(is_count_a_match(Type::Equal, 4, 4));
        assert!(is_count_a_match(Type::Greater, 5, 4));
        assert!(is_count_a_match(Type::Notequal, 3, 4));
        assert!(!is_count_a_match(Type::Notequal, 4, 4));
    }
}
