// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.join_shapes(use_mirror=False)`, tier T0 (`tests/golden/cases/
//! weights_shapes/join_shapes.py`). Port of `join_shapes_exec`,
//! `active_shape_key_editable_poll` (`editors/object/object_add.cc`),
//! `ED_mesh_shapes_join_objects_exec` (`editors/mesh/meshtools.cc`) and `shape_key_mirror`
//! (`editors/object/object_shapekey.cc`), Blender 5.2.4, GPL-2.0-or-later:
//!
//! every other selected mesh object with the active mesh's vertex count becomes a shape key of
//! the active mesh named after the object, holding that object's **evaluated** mesh positions
//! (its original mesh when the evaluated one has another vertex count), in its own local
//! space (no transform). The active mesh gets a key with a `Basis` block first when it has
//! none. `use_mirror` swaps `.L`/`.R` in the names and mirrors each new block along local X
//! (`N vertices mirrored`). Reports (all `CANCELLED`): `No source mesh objects selected`,
//! `Selected meshes must have equal numbers of vertices`, `Selected objects' names must use .L
//! or .R suffix` (error).
//!
//! Poll: an active mesh object, not in Edit Mode (`This operation is not supported in edit
//! mode`), with local data (`Cannot edit external library data`).

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::Key;
use frap_scene::{Context, EvalMode, ObjectMode};
use frap_spec_ops_object::object;
use smallvec::smallvec;

/// `active_shape_key_editable_poll`.
pub(crate) fn editable_poll(c: &Context, idname: &str) -> Result<(), PollFail> {
    let msg = |why: &str| Some(format!("Operator bpy.ops.{idname}.poll() {why}"));
    let Some((_, ob)) = poll::active(c) else {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "make the mesh to add the shapes to active",
        ));
    };
    if ob.object_type() != "MESH" {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make the mesh to add the shapes to active",
        ));
    }
    if ob.mode == ObjectMode::Edit {
        return Err(PollFail {
            message: msg("This operation is not supported in edit mode"),
            unmet: smallvec![PollReq::ModeObject],
            hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
        });
    }
    if ob
        .data
        .and_then(|d| c.main.header(d))
        .is_some_and(|h| h.library.is_some())
    {
        return Err(PollFail {
            message: msg("Cannot edit external library data"),
            unmet: smallvec![PollReq::ActiveEditable],
            hint: None,
        });
    }
    Ok(())
}

fn join_shapes_poll(c: &Context) -> Result<(), PollFail> {
    editable_poll(c, "object.join_shapes")
}

/// `active_shape_key_editable_poll`.
const JOIN_SHAPES_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveType("MESH"),
        PollReq::ModeObject,
    ],
    check: join_shapes_poll,
};

/// `create_mirrored_name`.
fn mirrored_name(name: &str, mirror: bool) -> String {
    if !mirror {
        return name.to_owned();
    }
    if let Some(b) = name.strip_suffix(".L") {
        return format!("{b}.R");
    }
    if let Some(b) = name.strip_suffix(".R") {
        return format!("{b}.L");
    }
    name.to_owned()
}

/// `mesh_get_x_mirror_vert_spatial`: the vertex nearest to `positions[i]` mirrored along X,
/// within `KD_THRESH` (`ED_mesh_mirror_spatial_table_lookup`).
fn mirror_vert(positions: &[[f32; 3]], i: usize) -> Option<usize> {
    const KD_THRESH: f32 = 0.00002;
    let p = positions[i];
    let v = [-p[0], p[1], p[2]];
    let mut best: Option<(usize, f32)> = None;
    for (j, q) in positions.iter().enumerate() {
        let d = ((q[0] - v[0]).powi(2) + (q[1] - v[1]).powi(2) + (q[2] - v[2]).powi(2)).sqrt();
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((j, d));
        }
    }
    best.filter(|(_, d)| *d < KD_THRESH).map(|(j, _)| j)
}

/// `shape_key_mirror` (spatial): swaps mirrored vertices and flips X. Returns
/// `(mirrored, failed)`.
fn mirror_block(positions: &[[f32; 3]], data: &mut [[f32; 3]]) -> (usize, usize) {
    let mut tag = vec![false; data.len()];
    let (mut totmirr, mut totfail) = (0, 0);
    for i1 in 0..positions.len().min(data.len()) {
        match mirror_vert(positions, i1) {
            Some(i2) if i2 == i1 => {
                data[i1][0] = -data[i1][0];
                tag[i1] = true;
                totmirr += 1;
            }
            Some(i2) if i2 < data.len() => {
                if !tag[i1] && !tag[i2] {
                    data.swap(i1, i2);
                    data[i1][0] = -data[i1][0];
                    data[i2][0] = -data[i2][0];
                    totmirr += 1;
                }
                tag[i1] = true;
                tag[i2] = true;
            }
            _ => totfail += 1,
        }
    }
    (totmirr, totfail)
}

/// `ED_mesh_shapes_join_objects_exec(C, ensure_keys_exist = true, mirror)`.
fn join_shapes(cx: &mut OpCtx, mirror: bool) -> OpResult {
    let c = cx.ctx();
    let Some((active, ob)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(me_active): Option<IdRef<MeshId>> = ob.data.and_then(|d| d.typed()) else {
        return Ok(OpStatus::Cancelled);
    };
    let selected = c.selected_editable_objects();
    let active_verts = c.main.meshes.get(me_active).map_or(0, |m| m.verts_num());
    let mut found_object = false;
    let mut found_non_equal = false;
    let mut compatible: Vec<(String, Vec<[f32; 3]>)> = Vec::new();
    let candidates: Vec<(IdRef<ObjectId>, String, IdRef<MeshId>)> = selected
        .iter()
        .filter(|&&r| r != active)
        .filter_map(|&r| {
            let o = c.main.objects.get(r)?;
            (o.object_type() == "MESH").then_some(())?;
            Some((r, o.header.name.to_string(), o.data?.typed()?))
        })
        .collect();
    for (r, name, me) in candidates {
        found_object = true;
        let evaluated = {
            let (host, ev) = cx.split_eval();
            ev.object(host, r, EvalMode::Viewport)
                .ok()
                .and_then(|e| e.mesh.clone())
        };
        if let Some(m) = evaluated.filter(|m| m.verts_num() == active_verts) {
            compatible.push((name, m.positions().to_vec()));
            continue;
        }
        if let Some(m) = cx
            .main()
            .meshes
            .get(me)
            .filter(|m| m.verts_num() == active_verts)
        {
            compatible.push((name, m.positions().to_vec()));
            continue;
        }
        found_non_equal = true;
    }
    if !found_object {
        cx.report(ReportType::Warning, "No source mesh objects selected");
        return Ok(OpStatus::Cancelled);
    }
    if found_non_equal {
        cx.report(
            ReportType::Warning,
            "Selected meshes must have equal numbers of vertices",
        );
        return Ok(OpStatus::Cancelled);
    }
    if compatible.is_empty() {
        cx.report(
            ReportType::Warning,
            "No additional selected meshes with equal vertex count to join",
        );
        return Ok(OpStatus::Cancelled);
    }
    if mirror
        && compatible
            .iter()
            .any(|(n, _)| !n.ends_with(".L") && !n.ends_with(".R"))
    {
        return Err(OpError::Runtime(
            "Selected objects' names must use .L or .R suffix".into(),
        ));
    }
    let base_positions = cx
        .main()
        .meshes
        .get(me_active)
        .map(|m| m.positions().to_vec())
        .unwrap_or_default();
    let (main, log) = cx.main_mut();
    let had_key = main
        .meshes
        .get(me_active)
        .and_then(|m| m.shape_keys)
        .is_some_and(|k| main.shape_keys.get(k).is_some());
    let k = Key::ensure_for_mesh(main, log, me_active)
        .ok_or_else(|| OpError::Runtime("the mesh was removed".into()))?;
    let key = main
        .shape_keys
        .get_mut(k, log)
        .ok_or_else(|| OpError::Runtime("the key was removed".into()))?;
    if !had_key {
        let i = key.add_block(None);
        key.key_blocks[i].data = base_positions.clone();
    }
    let (mut totmirr, mut totfail) = (0, 0);
    for (name, positions) in compatible {
        let name = mirrored_name(&name, mirror);
        let i = key.add_block(Some(&name));
        key.key_blocks[i].data = positions;
        if mirror {
            let (m, f) = mirror_block(&base_positions, &mut key.key_blocks[i].data);
            totmirr += m;
            totfail += f;
        }
    }
    if mirror {
        if totfail > 0 {
            cx.report(
                ReportType::Warning,
                format!("{totmirr} vertices mirrored, {totfail} failed"),
            );
        } else {
            cx.report(ReportType::Info, format!("{totmirr} vertices mirrored"));
        }
    }
    Ok(OpStatus::Finished)
}

#[operator(poll = JOIN_SHAPES_POLL, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::JoinShapes) -> OpResult {
    join_shapes(cx, p.use_mirror.value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mirrored_names_swap_sides() {
        assert_eq!(mirrored_name("Arm.L", true), "Arm.R");
        assert_eq!(mirrored_name("Arm.R", true), "Arm.L");
        assert_eq!(mirrored_name("Arm.L", false), "Arm.L");
    }

    #[test]
    fn mirror_block_swaps_pairs_and_flips_x() {
        let pos = [[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
        let mut data = vec![[1.0, 2.0, 0.0], [-1.0, 3.0, 0.0], [0.5, 1.0, 0.0]];
        assert_eq!(mirror_block(&pos, &mut data), (2, 0));
        assert_eq!(data[0], [1.0, 3.0, 0.0]);
        assert_eq!(data[1], [-1.0, 2.0, 0.0]);
        assert_eq!(data[2], [-0.5, 1.0, 0.0]);
    }
}
