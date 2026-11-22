// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.update_shapes(use_mirror=False)`, tier T1 (`tests/golden/cases/object/
//! shape_keys_m8/update_shapes.py`). Port of `update_all_shape_keys_exec`,
//! `object_update_shapes_poll` (`editors/object/object_add.cc`) and
//! `ED_mesh_shapes_join_objects_exec(C, ensure_keys_exist = false, mirror)`
//! (`editors/mesh/meshtools.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Every other selected mesh object with the active mesh's vertex count updates the active
//! mesh's shape key named after it with its **evaluated** positions (its original mesh when the
//! evaluated one has another vertex count), in its own local space. `use_mirror` looks for the
//! name with `.L`/`.R` swapped and mirrors the updated block along local X (`N vertices
//! mirrored`). Reports `Updated N shape key(s)` (INFO).
//!
//! Refusals (`CANCELLED`, WARNING): `No source mesh objects selected`, `Selected meshes must
//! have equal numbers of vertices`; errors (`RuntimeError`): `Selected objects' names must use
//! .L or .R suffix` (with `use_mirror`), `No name matches between selected objects and shape
//! keys` (nothing to update).
//!
//! Poll: `active_shape_key_editable_poll` (an active mesh object with local data, not in Edit
//! Mode) and a shape key with at least one block.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::{Context, EvalMode};
use frap_spec_ops_object::object;
use smallvec::smallvec;

use super::join_shapes::editable_poll;
use super::shape_key_add::common;
use crate::support::weights::{self, Topology, mirror};

/// `object_update_shapes_poll`.
fn update_poll(c: &Context) -> Result<(), PollFail> {
    editable_poll(c, "object.update_shapes")?;
    let (ob, _) = poll::active(c).expect("invariant: checked by editable_poll");
    if !common::key_of(c, ob)
        .and_then(|k| c.main.shape_keys.get(k))
        .is_some_and(|k| !k.key_blocks.is_empty())
    {
        return Err(PollFail {
            message: None,
            unmet: smallvec![],
            hint: Some(
                "the active mesh has no shape keys: add them with bpy.ops.object.join_shapes()"
                    .into(),
            ),
        });
    }
    Ok(())
}

/// `object_update_shapes_poll`.
const UPDATE_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveType("MESH"),
        PollReq::ModeObject,
    ],
    check: update_poll,
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

#[operator(poll = UPDATE_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::UpdateShapes) -> OpResult {
    let use_mirror = p.use_mirror.value;
    let Some((active, me_active)) = weights::active_mesh(cx) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(k) = common::key_of(&cx.ctx(), active) else {
        return Ok(OpStatus::Cancelled);
    };
    let active_verts = cx.main().meshes.get(me_active).map_or(0, |m| m.verts_num());
    let candidates: Vec<(IdRef<ObjectId>, String, IdRef<MeshId>)> = {
        let c = cx.ctx();
        c.selected_editable_objects()
            .into_iter()
            .filter(|&r| r != active)
            .filter_map(|r| {
                let o = c.main.objects.get(r)?;
                (o.object_type() == "MESH").then_some(())?;
                Some((r, o.header.name.to_string(), o.data?.typed()?))
            })
            .collect()
    };
    let mut found_object = false;
    let mut found_non_equal = false;
    let mut compatible: Vec<(String, Vec<[f32; 3]>)> = Vec::new();
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
    if use_mirror
        && compatible
            .iter()
            .any(|(n, _)| !n.ends_with(".L") && !n.ends_with(".R"))
    {
        return Err(OpError::Runtime(
            "Selected objects' names must use .L or .R suffix".into(),
        ));
    }
    let mirr = if use_mirror {
        let topo =
            Topology::load(cx, me_active).ok_or_else(|| OpError::Runtime("no mesh".into()))?;
        mirror::object_mirror_map(&topo.positions, &topo.edges, false)
    } else {
        Vec::new()
    };
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let (mut totmirr, mut totfail, mut keys_changed) = (0, 0, 0);
    for (name, positions) in compatible {
        let name = mirrored_name(&name, use_mirror);
        let Some(i) = key.find_block(&name) else {
            continue;
        };
        keys_changed += 1;
        // `BKE_keyblock_update_from_mesh`.
        let data = &mut key.key_blocks[i].data;
        let n = data.len().min(positions.len());
        data[..n].copy_from_slice(&positions[..n]);
        if use_mirror {
            let (m, f) = weights::mirror_block(data, &mirr);
            totmirr += m;
            totfail += f;
        }
    }
    if keys_changed == 0 {
        return Err(OpError::Runtime(
            "No name matches between selected objects and shape keys".into(),
        ));
    }
    cx.report(
        ReportType::Info,
        format!("Updated {keys_changed} shape key(s)"),
    );
    if use_mirror {
        weights::report_mirror(cx, totmirr, totfail);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::mirrored_name;

    #[test]
    fn mirrored_names() {
        assert_eq!(mirrored_name("Arm.L", true), "Arm.R");
        assert_eq!(mirrored_name("Arm.R", true), "Arm.L");
        assert_eq!(mirrored_name("Arm", true), "Arm");
        assert_eq!(mirrored_name("Arm.L", false), "Arm.L");
    }
}
