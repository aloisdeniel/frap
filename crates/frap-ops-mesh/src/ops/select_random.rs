// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_random(ratio=0.5, seed=0, action='SELECT')`, tier T1
//! (`tests/golden/cases/edit/select/random*.py`). Port of `edbm_select_random_exec`
//! (`editors/mesh/editmesh_select.cc`), `BLI_array_randomize` (`blenlib/intern/rand.cc`) and
//! `BLI_ghashutil_strhash_p` (`blenlib/intern/BLI_ghash_utils.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Per mesh in Edit Mode, the visible elements of the highest select mode (vertices, else
//! edges, else faces) are shuffled with `BLI_rng` seeded by `seed`, and the first
//! `int(count * ratio)` are (de)selected; the selection is then flushed. The result is
//! identical to Blender's for the same seed. Objects after the first (active) one add the hash
//! of their `id.name` (`"OB" + name`) to the seed, so each object gets its own pattern
//! independent of object order. Called from Python the seed is used as given (Blender
//! increments it only when the operator is invoked from the UI).

use frap_bmesh::{BMElem, SelectMode};
use frap_math::rng::array_randomize;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, select_random::Action};

use super::select_more::common;

/// `BLI_ghashutil_strhash_p` (djb2 over `signed char`s).
pub(crate) fn strhash_p(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for &b in s.as_bytes() {
        h = (h << 5).wrapping_add(h).wrapping_add(b as i8 as i32 as u32);
    }
    h
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::SelectRandom) -> OpResult {
    let select = p.action.value == Action::Select;
    let randfac = p.ratio.value;
    let seed = p.seed.value;
    for (ob_index, (ob, me)) in common::edit_meshes(cx).into_iter().enumerate() {
        let mut seed_iter = seed as u32;
        // This gives a consistent result regardless of object order.
        if ob_index != 0 {
            let name = cx
                .main()
                .objects
                .get(ob)
                .map(|o| format!("OB{}", o.header.name))
                .unwrap_or_default();
            seed_iter = seed_iter.wrapping_add(strhash_p(&name));
        }
        let bm = cx.bmesh_mut(me)?;
        let mode = common::primary_mode(bm.select_mode);
        let mut elem_map: Vec<BMElem> = if mode == SelectMode::VERT {
            bm.vert_ids().into_iter().map(BMElem::Vert).collect()
        } else if mode == SelectMode::EDGE {
            bm.edge_ids().into_iter().map(BMElem::Edge).collect()
        } else {
            bm.face_ids().into_iter().map(BMElem::Face).collect()
        };
        elem_map.retain(|&e| !common::hidden(bm, e));
        array_randomize(&mut elem_map, seed_iter);
        let count_select = (elem_map.len() as f32 * randfac) as usize;
        for &e in elem_map.iter().take(count_select) {
            bm.select_set(e, select);
        }
        if select {
            // Was `EDBM_select_flush_from_verts`, but it over-selects in edge/face mode.
            common::selectmode_flush(bm);
        } else {
            bm.deselect_flush();
        }
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strhash_matches_djb2_on_signed_chars() {
        assert_eq!(strhash_p(""), 5381);
        // 5381 * 33 + 'O' (79), then * 33 + 'B' (66).
        assert_eq!(strhash_p("OB"), (5381u32 * 33 + 79) * 33 + 66);
        // Bytes >= 0x80 are negative `signed char`s, sign-extended before the add.
        let e = strhash_p("\u{e9}");
        let h1 = (5381u32 * 33).wrapping_add(0xFFFF_FFC3);
        assert_eq!(e, h1.wrapping_mul(33).wrapping_add(0xFFFF_FFA9));
    }
}
