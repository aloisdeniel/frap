// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.transform.vertex_random(offset=…, uniform=…, normal=…, seed=…)`, tier T1
//! (`tests/golden/cases/edit/transform/vertex_random.py`). Port of `object_rand_verts_exec`
//! and `object_rand_transverts` (`editors/object/object_random.cc`) with the edit-mesh part of
//! `ED_transverts_create_from_obedit` (`editors/util/ed_transverts.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! The transformed vertices follow the select mode: selected vertices (vertex mode), the
//! vertices of selected edges (edge mode) or of selected faces (face mode), hidden ones
//! excluded. Each moves in object space by `offset · t` along a random unit vector, `t`
//! between `uniform` and 1, optionally slerped towards its normal by `normal`. The generator
//! is Blender's `RandomNumberGenerator` seeded with `seed`, plus the hash of the object's ID
//! name (`"OB" + name`) for every object but the first. `CANCELLED` when no mesh had a vertex
//! to move.

use frap_bmesh::{BMesh, ElemFlag, SelectMode, VertId};
use frap_math::BliRng;
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_transform::transform;
use glam::Vec3;

use crate::edit::bmq::{co, has, vno};
use crate::edit::mathx::{dot, madd, slerp_safe};

/// `ED_transverts_create_from_obedit` (meshes): the vertices to move, in mesh order.
fn transverts(bm: &BMesh) -> Vec<VertId> {
    let mut on = vec![false; bm.verts.slots.len()];
    if bm.select_mode.contains(SelectMode::VERT) {
        for v in bm.vert_ids() {
            on[v.slot()] = !has(bm, v, ElemFlag::HIDDEN) && has(bm, v, ElemFlag::SELECT);
        }
    } else if bm.select_mode.contains(SelectMode::EDGE) {
        for e in bm.edge_ids() {
            if !has(bm, e, ElemFlag::HIDDEN) && has(bm, e, ElemFlag::SELECT) {
                for v in bm.edge(e).v {
                    on[v.slot()] = true;
                }
            }
        }
    } else {
        for f in bm.face_ids() {
            if !has(bm, f, ElemFlag::HIDDEN) && has(bm, f, ElemFlag::SELECT) {
                for v in bm.face_verts(f) {
                    on[v.slot()] = true;
                }
            }
        }
    }
    bm.vert_ids().into_iter().filter(|v| on[v.slot()]).collect()
}

/// `BLI_ghashutil_strhash_p` (djb2 on the bytes, as `unsigned char`).
fn strhash(s: &str) -> u32 {
    let mut h: u32 = 5381;
    for b in s.bytes() {
        h = (h << 5).wrapping_add(h).wrapping_add(u32::from(b));
    }
    h
}

#[operator(
    poll = poll::EDITMESH,
    tier = T1,
    notes = [
        "The offset is an object-space distance; every object after the first adds the hash of \
         its ID name to the seed (as Blender), so results do not depend on the object order."
    ]
)]
fn exec(cx: &mut OpCtx, p: transform::VertexRandom) -> OpResult {
    let offset = p.offset.value;
    let uniform = p.uniform.value;
    let normal_factor = p.normal.value;
    let seed = p.seed.value as u32;
    let use_normal = normal_factor != 0.0;
    let meshes = helpers::iter_edit_meshes(cx);
    let mut changed = false;
    for (index, (ob, me)) in meshes.into_iter().enumerate() {
        let name = cx
            .main()
            .objects
            .get(ob)
            .map(|o| o.header.name.to_string())
            .unwrap_or_default();
        let moves: Vec<(VertId, Vec3)> = {
            let (_, edit) = cx.split_edit();
            let Some(em) = edit.meshes.get(&me.key()) else {
                continue;
            };
            let bm = &em.bm;
            let tv = transverts(bm);
            if tv.is_empty() {
                continue;
            }
            let mut seed_iter = seed;
            if index != 0 {
                seed_iter = seed_iter.wrapping_add(strhash(&format!("OB{name}")));
            }
            let mut rng = BliRng::new(seed_iter);
            tv.into_iter()
                .map(|v| {
                    let t = (uniform + (1.0 - uniform) * rng.get_float()).max(0.0);
                    let mut vec = Vec3::from_array(rng.get_unit_float3());
                    if use_normal {
                        let n = vno(bm, v);
                        let no = if dot(vec, n) < 0.0 { -n } else { n };
                        vec = slerp_safe(vec, no, normal_factor);
                    }
                    (v, madd(co(bm, v), vec, offset * t))
                })
                .collect()
        };
        let bm = cx.bmesh_mut(me)?;
        for (v, c) in moves {
            bm.vert_mut(v).co = c.to_array();
        }
        bm.normals_update();
        changed = true;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
