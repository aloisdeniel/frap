// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.shape_key_transfer(mode='OFFSET', use_clamp=False)`, tier T1
//! (`tests/golden/cases/object/shape_keys_m8/transfer.py`). Port of the Python operator
//! `ShapeTransfer` (`scripts/startup/bl_operators/object.py`, Blender 5.2.4,
//! GPL-2.0-or-later), with the `mathutils` arithmetic of [`weights::geom`].
//!
//! The active shape key of the one other selected mesh (the source) is copied to the active
//! mesh (the target) as a new block with the source block's name: the target gets a key with
//! a `Basis` first when it has none, the new block has value 0, becomes active, and the target
//! gets `show_only_shape_key`. The new coordinates start from the target's basis (its mesh
//! without a key) and follow the source's deformation (source block − source basis):
//!
//! * `OFFSET`: the same offset per vertex;
//! * `RELATIVE_FACE`: per face corner, the deformed point in the frame of the corner's
//!   triangle (previous, corner, next vertex) moved to the target's triangle
//!   (`barycentric_transform`), averaged over the vertex's corners;
//! * `RELATIVE_EDGE`: the same per edge end, with triangles made of the edge and the end's
//!   vertex normal scaled by the edge length (the source mesh's and the target mesh's vertex
//!   normals).
//!
//! `use_clamp` (not with `OFFSET`) keeps each vertex's offset as long as the source's.
//! Reports (`RuntimeError`, nothing changes): `Expected one other selected mesh object to copy
//! from`, `Both objects must be meshes`, `Other object has no shape key`; a vertex count that
//! differs: WARNING `Skipping 'Cube', vertex count differs` (`FINISHED`, nothing added).
//!
//! Poll: an active mesh object, not in Edit Mode.

use frap_op::operator;
use frap_op::prelude::*;
use frap_op::{Poll, PollFail, PollReq};
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::id::Key;
use frap_scene::{Context, ObjectMode};
use frap_spec_ops_object::object;
use smallvec::smallvec;

use crate::support::weights::{self, geom};
use geom::V3;

/// `ShapeTransfer.poll`: `obj and obj.type == 'MESH' and obj.mode != 'EDIT'`.
fn transfer_poll(c: &Context) -> Result<(), PollFail> {
    let Some((_, ob)) = poll::active(c) else {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "make the mesh to copy the shape to active",
        ));
    };
    if ob.object_type() != "MESH" {
        return Err(poll::fail(
            PollReq::ActiveType("MESH"),
            "make the mesh to copy the shape to active",
        ));
    }
    if ob.mode == ObjectMode::Edit {
        return Err(PollFail {
            message: None,
            unmet: smallvec![PollReq::ModeObject],
            hint: Some("bpy.ops.object.mode_set(mode='OBJECT')".into()),
        });
    }
    Ok(())
}

/// `ShapeTransfer.poll`.
const TRANSFER_POLL: Poll = Poll {
    requires: &[
        PollReq::ActiveObject,
        PollReq::ActiveType("MESH"),
        PollReq::ModeObject,
    ],
    check: transfer_poll,
};

/// What `_main` reads of a mesh.
struct Side {
    /// `me.vertices[i].co`
    positions: Vec<V3>,
    /// `me.vertices[i].normal`
    normals: Vec<V3>,
    /// The basis block's coordinates (the positions without a key).
    basis: Vec<V3>,
}

fn side(cx: &OpCtx, me: IdRef<MeshId>) -> Option<Side> {
    let m = cx.main().meshes.get(me)?;
    let positions = m.positions().to_vec();
    let basis = m
        .shape_keys
        .and_then(|k| cx.main().shape_keys.get(k))
        .and_then(|k| k.key_blocks.first())
        .map_or_else(|| positions.clone(), |kb| kb.data.clone());
    Some(Side {
        normals: m.vert_normals().to_vec(),
        positions,
        basis,
    })
}

/// The per-vertex candidates of `median_coords`.
fn transfer(
    mode: &str,
    src_shape: &[V3],
    src: &Side,
    tgt_coords: &[V3],
    tgt_normals: &[V3],
    faces: &[Vec<usize>],
    edges: &[[usize; 2]],
) -> Vec<Vec<V3>> {
    let n = src.positions.len();
    let orig = &src.basis;
    let mut median: Vec<Vec<V3>> = vec![Vec::new(); n];
    match mode {
        "RELATIVE_FACE" => {
            for idxs in faces {
                if idxs.len() < 2 {
                    continue;
                }
                let mut v_before = idxs[idxs.len() - 2];
                let mut v = idxs[idxs.len() - 1];
                for &v_after in idxs {
                    let pt = geom::barycentric_transform(
                        src_shape[v],
                        [orig[v_before], orig[v], orig[v_after]],
                        [tgt_coords[v_before], tgt_coords[v], tgt_coords[v_after]],
                    );
                    median[v].push(pt);
                    v_before = v;
                    v = v_after;
                }
            }
        }
        "RELATIVE_EDGE" => {
            for &[i1, i2] in edges {
                let (v1, v2) = (orig[i1], orig[i2]);
                let edge_length = geom::length(geom::sub(v1, v2)) as f32;
                let n1loc = geom::add(v1, geom::scale(src.normals[i1], edge_length));
                let n2loc = geom::add(v2, geom::scale(src.normals[i2], edge_length));
                let (v1_to, v2_to) = (tgt_coords[i1], tgt_coords[i2]);
                let edlen_to = geom::length(geom::sub(v1_to, v2_to)) as f32;
                let n1loc_to = geom::add(v1_to, geom::scale(tgt_normals[i1], edlen_to));
                let n2loc_to = geom::add(v2_to, geom::scale(tgt_normals[i2], edlen_to));
                median[i1].push(geom::barycentric_transform(
                    src_shape[i1],
                    [v2, v1, n1loc],
                    [v2_to, v1_to, n1loc_to],
                ));
                median[i2].push(geom::barycentric_transform(
                    src_shape[i2],
                    [v1, v2, n2loc],
                    [v1_to, v2_to, n2loc_to],
                ));
            }
        }
        _ => {
            for (i, m) in median.iter_mut().enumerate() {
                m.push(geom::add(tgt_coords[i], geom::sub(src_shape[i], orig[i])));
            }
        }
    }
    median
}

/// `ob_add_shape(ob, name)`: a block from the mesh (with a `Basis` first on a mesh without
/// shape keys), value 0, named `name`, active, and `show_only_shape_key` on. Returns its
/// index.
fn add_shape(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    name: &str,
) -> Result<(IdRef<frap_rna::KeyId>, usize), OpError> {
    let (main, log) = cx.main_mut();
    let missing = || OpError::Runtime("Object does not support shapes".into());
    let (k, mut i) = Key::object_insert(main, log, ob, Some("Key"), false).ok_or_else(missing)?;
    let key = main.shape_keys.get_mut(k, log).ok_or_else(missing)?;
    key.key_blocks[i].set_value(1.0);
    if key.key_blocks.len() == 1 {
        key.rename_block(i, "Basis");
        let (_, j) = Key::object_insert(main, log, ob, Some("Key"), false).ok_or_else(missing)?;
        i = j;
    }
    let key = main.shape_keys.get_mut(k, log).ok_or_else(missing)?;
    key.key_blocks[i].set_value(0.0);
    key.rename_block(i, name);
    let last = key.key_blocks.len() - 1;
    let o = main.objects.get_mut(ob, log).ok_or_else(missing)?;
    o.active_shape_key_index = last as i32;
    o.show_only_shape_key = true;
    Ok((k, i))
}

#[operator(poll = TRANSFER_POLL, tier = T1)]
fn exec(cx: &mut OpCtx, p: object::ShapeKeyTransfer) -> OpResult {
    let c = cx.ctx();
    let Some((target, _)) = poll::active(&c) else {
        return Ok(OpStatus::Cancelled);
    };
    let others: Vec<IdRef<ObjectId>> = c
        .selected_editable_objects()
        .into_iter()
        .filter(|&r| r != target)
        .collect();
    if others.len() != 1 {
        return Err(OpError::Runtime(
            "Expected one other selected mesh object to copy from".into(),
        ));
    }
    let source = others[0];
    let mesh_of = |r: IdRef<ObjectId>| {
        c.main
            .objects
            .get(r)
            .filter(|o| o.object_type() == "MESH")
            .and_then(|o| weights::mesh_of(&c, o))
    };
    let (Some(me_src), Some(me_tgt)) = (mesh_of(source), mesh_of(target)) else {
        return Err(OpError::Runtime("Both objects must be meshes".into()));
    };
    let Some((k_src, a_src)) = weights::active_block(&c, source) else {
        return Err(OpError::Runtime("Other object has no shape key".into()));
    };
    let mode = p.mode.value.identifier();
    let use_clamp = p.use_clamp.value && mode != "OFFSET";
    let Some(kb) = c.main.shape_keys.get(k_src).map(|k| &k.key_blocks[a_src]) else {
        return Ok(OpStatus::Cancelled);
    };
    let orig_key_name = kb.name.to_string();
    let src_shape = kb.data.clone();
    let Some(src) = side(cx, me_src) else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(tgt) = side(cx, me_tgt) else {
        return Ok(OpStatus::Cancelled);
    };
    if tgt.positions.len() != src.positions.len() {
        let name = cx
            .main()
            .objects
            .name_of(target)
            .unwrap_or_default()
            .to_owned();
        cx.report(
            ReportType::Warning,
            format!("Skipping '{name}', vertex count differs"),
        );
        return Ok(OpStatus::Finished);
    }
    let n = src.positions.len();
    if src_shape.len() < n || src.basis.len() < n || tgt.basis.len() < n {
        return Err(OpError::Runtime(
            "the shape keys do not match the vertex count".into(),
        ));
    }
    let (faces, edges) = {
        let Some(m) = cx.main().meshes.get(me_src) else {
            return Ok(OpStatus::Cancelled);
        };
        let cv = m.corner_verts();
        let faces: Vec<Vec<usize>> = (0..m.face_offsets().len().saturating_sub(1))
            .map(|f| m.face(f).map(|c| cv[c] as usize).collect())
            .collect();
        let edges: Vec<[usize; 2]> = m
            .edges()
            .iter()
            .map(|e| [e[0] as usize, e[1] as usize])
            .collect();
        (faces, edges)
    };
    let tgt_coords = tgt.basis.clone();
    let median = transfer(
        mode,
        &src_shape,
        &src,
        &tgt_coords,
        &tgt.normals,
        &faces,
        &edges,
    );
    let (k, i) = add_shape(cx, target, &orig_key_name)?;
    let (main, log) = cx.main_mut();
    let Some(key) = main.shape_keys.get_mut(k, log) else {
        return Ok(OpStatus::Cancelled);
    };
    let data = &mut key.key_blocks[i].data;
    for (v, cos) in median.iter().enumerate() {
        if cos.is_empty() || v >= data.len() {
            continue;
        }
        let sum = cos[1..].iter().fold(cos[0], |a, b| geom::add(a, *b));
        let mut co = geom::div(sum, cos.len() as f32);
        if use_clamp {
            let len_from = geom::length(geom::sub(src_shape[v], src.basis[v]));
            let ofs = geom::with_length(geom::sub(co, tgt_coords[v]), len_from);
            co = geom::add(tgt_coords[v], ofs);
        }
        data[v] = co;
    }
    Ok(OpStatus::Finished)
}
