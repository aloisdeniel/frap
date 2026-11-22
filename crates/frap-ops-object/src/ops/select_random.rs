// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.select_random(ratio=0.5, seed=0, action='SELECT')`, tier T0
//! (`tests/golden/cases/object/select_m8/random*.py`). Ported from
//! blender/source/blender/editors/object/object_select.cc (`object_select_random_exec`) with
//! `BLI_array_randomize` (`blenlib/intern/rand.cc`, `frap_math::rng`), Blender 5.2.4,
//! GPL-2.0-or-later. Verified on 5.2.4:
//!
//! - poll `objects_selectable_poll`; the visible, selectable bases (base order) are shuffled
//!   with a generator seeded by `seed` (as given: only an interactive invoke bumps it), and the
//!   first `int(count × ratio)` of them are selected (`SELECT`) or deselected (`DESELECT`);
//!   the others keep their selection; always `FINISHED`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_object::object::{self, select_random::Action};

use super::select_grouped::SELECTABLE;
use super::select_grouped::bases::Bases;

/// The base indices `object_select_random_exec` changes: the first `len × ratio` (truncated)
/// of the shuffled list.
fn picked(selectable: &[usize], ratio: f32, seed: i32) -> Vec<usize> {
    let mut elems = selectable.to_vec();
    frap_math::rng::array_randomize(&mut elems, seed as u32);
    // `const int count_select = elem_map_len * randfac;`
    let count = (elems.len() as f32 * ratio) as usize;
    elems.truncate(count.min(elems.len()));
    elems
}

#[operator(poll = SELECTABLE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::SelectRandom) -> OpResult {
    let mut b = Bases::load(cx);
    let select = p.action.value == Action::Select;
    for i in picked(&b.selectable(), p.ratio.value, p.seed.value) {
        b.select(i, select);
    }
    b.commit(cx);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_is_truncated_and_ratio_one_takes_all() {
        let all: Vec<usize> = (0..9).collect();
        assert_eq!(picked(&all, 0.5, 0).len(), 4);
        assert_eq!(picked(&all, 0.3, 2).len(), 2);
        let mut every = picked(&all, 1.0, 5);
        every.sort_unstable();
        assert_eq!(every, all);
        assert!(picked(&all, 0.0, 1).is_empty());
    }
}
