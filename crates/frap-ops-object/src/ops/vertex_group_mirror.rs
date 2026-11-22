// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.vertex_group_mirror(mirror_weights=True, flip_group_names=True,
//! all_groups=False, use_topology=False)`, tier T1 (`tests/golden/cases/object/vgroups_m8/
//! mirror.py`, `mirror_topology.py`). Port of `vertex_group_mirror_exec`, `vgroup_mirror` and
//! `dvert_mirror_op` (`editors/object/object_vgroup.cc`, Blender 5.2.4, GPL-2.0-or-later).
//!
//! Every pair of X-mirrored vertices of the active mesh (spatially, within `0.00002` of the
//! mirrored position, or by topology with `use_topology`) is visited once:
//!
//! * In Object Mode both vertices count as selected: their weights of the active group (every
//!   group with `all_groups`) swap, and with `flip_group_names` both vertices move their
//!   weights to the flipped group names (`.L` ↔ `.R`; the active group and its partner only
//!   without `all_groups`).
//! * In Edit Mode a pair with both vertices selected does the same; a pair with one selected
//!   vertex copies the other one's weights to it (then flips them).
//!
//! The pairs come from the edit BMesh cache (`EDBM_verts_mirror_cache_begin`) in Edit Mode and
//! from `mesh_get_x_mirror_vert` in Object Mode. Reports `N vertices mirrored` (INFO) or `N
//! vertices mirrored, M failed` (WARNING, M vertices without a mirror); nothing happens without
//! an active group, without weights, or with neither `mirror_weights` nor `flip_group_names`
//! (`0 vertices mirrored`). Vertex-group locks are not honoured, as in Blender.
//!
//! Poll `vertex_group_poll`.

use frap_bmesh::shape::deform;
use frap_mesh::DeformVert;
use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollReq};
use frap_scene::Context;
use frap_spec_ops_object::object;

use super::vertex_group_add::common;
use crate::support::weights::{self, Topology, Weights, defvert, groups, mirror};

fn mirror_poll(c: &Context) -> Result<(), frap_op::PollFail> {
    common::group_poll(c, "object.vertex_group_mirror")
}

/// `vertex_group_poll`.
const MIRROR_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveEditable,
        PollReq::ActiveType("MESH"),
    ],
    check: mirror_poll,
};

/// What `dvert_mirror_op` does.
struct MirrorOp<'a> {
    flip_map: &'a [i32],
    mirror_weights: bool,
    flip_vgroups: bool,
    all_vgroups: bool,
    act: u32,
}

impl MirrorOp<'_> {
    /// `dvert_mirror_op(dvert, dvert_mirr, sel, sel_mirr, …)` on vertices `a` and `b`.
    fn apply(&self, dv: &mut [DeformVert], a: usize, b: usize, sel: bool, sel_mirr: bool) {
        if sel && sel_mirr {
            if self.mirror_weights {
                if self.all_vgroups {
                    dv.swap(a, b);
                } else {
                    let act = self.act;
                    match (deform::find(&dv[a], act), deform::find(&dv[b], act)) {
                        (Some(i), Some(j)) => {
                            let (wa, wb) = (dv[a].weights[i].1, dv[b].weights[j].1);
                            dv[a].weights[i].1 = wb;
                            dv[b].weights[j].1 = wa;
                        }
                        (Some(i), None) => {
                            let w = dv[a].weights[i].1;
                            *deform::ensure(&mut dv[b], act) = w;
                            deform::remove_at(&mut dv[a], i);
                        }
                        (None, Some(j)) => {
                            let w = dv[b].weights[j].1;
                            *deform::ensure(&mut dv[a], act) = w;
                            deform::remove_at(&mut dv[b], j);
                        }
                        (None, None) => {}
                    }
                }
            }
            if self.flip_vgroups {
                defvert::flip(&mut dv[a], self.flip_map);
                defvert::flip(&mut dv[b], self.flip_map);
            }
            return;
        }
        // The selected vertex is the target, the other one the source.
        let (t, s) = if sel_mirr { (b, a) } else { (a, b) };
        if self.mirror_weights {
            let src = dv[s].clone();
            if self.all_vgroups {
                defvert::copy(&mut dv[t], &src);
            } else {
                defvert::copy_index(&mut dv[t], self.act, &src, self.act);
            }
        }
        if self.flip_vgroups {
            defvert::flip(&mut dv[t], self.flip_map);
        }
    }
}

#[operator(poll = MIRROR_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::VertexGroupMirror) -> OpResult {
    let Some((ob, me)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let (mirror_weights, flip_vgroups, all_vgroups, use_topology) = (
        p.mirror_weights.value,
        p.flip_group_names.value,
        p.all_groups.value,
        p.use_topology.value,
    );
    let def_nr = {
        let c = cx.ctx();
        c.main
            .objects
            .get(ob)
            .and_then(|o| weights::active_group(&c, o))
    };
    let (mut totmirr, mut totfail) = (0usize, 0usize);
    if let Some(def_nr) = def_nr.filter(|_| mirror_weights || flip_vgroups)
        && let Some(mut w) = Weights::load(cx, me)
    {
        let names = weights::group_names(cx, me);
        let flip_map = match (flip_vgroups, all_vgroups) {
            (false, _) => Vec::new(),
            (true, true) => groups::flip_map(&names, &[], false, false),
            (true, false) => groups::flip_map_single(&names, false, def_nr),
        };
        let topo = Topology::load(cx, me).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        let op = MirrorOp {
            flip_map: &flip_map,
            mirror_weights,
            flip_vgroups,
            all_vgroups,
            act: def_nr as u32,
        };
        let n = w.len();
        let mirr = if w.edit {
            mirror::edit_mirror_map(&topo.positions, &topo.edges, use_topology)
        } else {
            mirror::object_mirror_map(&topo.positions, &topo.edges, use_topology)
        };
        let mut tag = vec![false; n];
        for v in 0..n {
            if tag[v] {
                continue;
            }
            let Some(m) = mirr[v] else {
                totfail += 1;
                continue;
            };
            if m == v || tag[m] {
                continue;
            }
            // Object Mode: no paint selection, both count as selected.
            let (sel, sel_mirr) = if w.edit {
                (w.select[v], w.select[m])
            } else {
                (true, true)
            };
            if sel || sel_mirr {
                op.apply(&mut w.dv, v, m, sel, sel_mirr);
                totmirr += 1;
            }
            tag[v] = true;
            tag[m] = true;
        }
        w.store(cx)?;
    }
    weights::report_mirror(cx, totmirr, totfail);
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dv(w: &[(u32, f32)]) -> DeformVert {
        DeformVert {
            weights: w.iter().copied().collect(),
        }
    }

    #[test]
    fn both_selected_swap_then_flip() {
        // Groups 0 = G.L, 1 = G.R.
        let flip_map = [1, 0];
        let op = MirrorOp {
            flip_map: &flip_map,
            mirror_weights: true,
            flip_vgroups: true,
            all_vgroups: false,
            act: 0,
        };
        let mut d = vec![dv(&[(0, 0.5)]), dv(&[(1, 0.2)])];
        op.apply(&mut d, 0, 1, true, true);
        // The G.L weight moved to vertex 1, then both vertices flipped their names.
        assert_eq!(d[0].weights.as_slice(), &[] as &[(u32, f32)]);
        assert_eq!(d[1].weights.as_slice(), &[(0, 0.2), (1, 0.5)]);
    }

    #[test]
    fn one_selected_copies_from_the_other() {
        let op = MirrorOp {
            flip_map: &[],
            mirror_weights: true,
            flip_vgroups: false,
            all_vgroups: true,
            act: 0,
        };
        let mut d = vec![dv(&[(0, 0.5)]), dv(&[(1, 0.2)])];
        op.apply(&mut d, 0, 1, false, true);
        assert_eq!(d[1].weights.as_slice(), &[(0, 0.5)]);
        assert_eq!(d[0].weights.as_slice(), &[(0, 0.5)]);
    }
}
