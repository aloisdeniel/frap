// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_more()`, tier T0
//! (`tests/golden/cases/object/select_m8/more_less*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc (`object_select_more_less`,
//! `object_select_more_exec`), Blender 5.2.4, GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `ED_operator_objectmode`; both ends of every parent–child pair (the child a visible,
//!   selectable base) of which exactly one is selected and visible are tagged, and the tagged
//!   visible, selectable bases are selected (`select_less` deselects them instead): the
//!   selection grows by one level up and down the hierarchy;
//! - nothing changed → `CANCELLED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object;

use self::more_less::more_less;

/// The shared body of `select_more` and `select_less`.
pub(crate) mod more_less {
    use std::collections::BTreeSet;

    use frap_op::prelude::*;
    use frap_rna::{IdRef, ObjectId};

    use crate::ops::select_grouped::bases::Bases;

    /// `object_select_more_less(C, select)`: whether a base changed.
    pub(crate) fn more_less(cx: &mut OpCtx, select: bool) -> bool {
        let mut b = Bases::load(cx);
        let list = b.selectable();
        // `OB_DONE`: the selected objects.
        let done: BTreeSet<IdRef<ObjectId>> =
            b.selected().into_iter().map(|i| b.list[i].ob).collect();
        // `ID_TAG_DOIT`: both ends of a pair whose ends differ.
        let mut doit: BTreeSet<IdRef<ObjectId>> = BTreeSet::new();
        {
            let main = cx.main();
            for &i in &list {
                let ob = b.list[i].ob;
                if let Some(par) = main.objects.get(ob).and_then(|o| o.parent)
                    && done.contains(&ob) != done.contains(&par)
                {
                    doit.insert(ob);
                    doit.insert(par);
                }
            }
        }
        let mut changed = false;
        for i in list {
            if doit.contains(&b.list[i].ob) && b.list[i].selected != select {
                b.select(i, select);
                changed = true;
            }
        }
        b.commit(cx);
        changed
    }
}

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, _p: object::SelectMore) -> OpResult {
    if more_less(cx, true) {
        return Ok(OpStatus::Finished);
    }
    cx.report(
        ReportType::Warning,
        "frap: nothing to select: no parent or child of the selection is unselected",
    );
    Ok(OpStatus::Cancelled)
}
