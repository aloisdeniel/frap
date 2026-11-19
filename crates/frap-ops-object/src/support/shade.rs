// SPDX-License-Identifier: GPL-3.0-or-later
//! Shading attributes of `object.shade_smooth/shade_flat/shade_smooth_by_angle`. Ports of
//! `bke::mesh_smooth_set`, `bke::mesh_sharp_edges_set_from_angle` (`blenkernel/intern/mesh.cc`),
//! `bke::mesh::edges_sharp_from_angle_set` / `mesh_edges_sharp_tag`
//! (`blenkernel/intern/mesh_normals.cc`) and `shade_poll` (`editors/object/object_edit.cc`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4: `shade_smooth` removes `sharp_face` (and `sharp_edge` unless
//! `keep_sharp_edges`); `shade_flat` re-creates `sharp_face` at the end of the attribute list,
//! all true; `shade_smooth_by_angle` removes `sharp_face` and tags the edges whose face angle
//! exceeds `angle` (factory cube, 30°: all 12 edges), keeping (or, with
//! `keep_sharp_edges=False`, clearing first) the existing tags; `angle` ≥ π is
//! `shade_smooth`, `angle == 0` is `shade_flat`.

use frap_mesh::{Domain, Mesh, SHARP_EDGE, SHARP_FACE};
use frap_op::{Poll, PollFail, PollReq, poll};
use frap_scene::{Context, ObjectMode};
use glam::Vec3;

/// `mesh_smooth_set`.
pub fn smooth_set(mesh: &mut Mesh, use_smooth: bool, keep_sharp_edges: bool) {
    let n = mesh.faces_num();
    let attrs = mesh.attributes_mut();
    if !keep_sharp_edges {
        attrs.remove(SHARP_EDGE);
    }
    attrs.remove(SHARP_FACE);
    if !use_smooth {
        mesh.set_bool_attribute(SHARP_FACE, Domain::Face, vec![true; n]);
    }
}

/// `INDEX_UNSET` / `INDEX_INVALID` of `mesh_edges_sharp_tag`.
const INDEX_UNSET: i32 = i32::MIN;
const INDEX_INVALID: i32 = -1;

/// `mesh_edges_sharp_tag` with an angle check and no sharp faces (`edges_sharp_from_angle_set`):
/// tags the manifold edges whose two faces meet at more than `split_angle`; an edge used by
/// more than two corners is untagged.
pub fn edges_sharp_from_angle(
    face_offsets: &[i32],
    corner_verts: &[i32],
    corner_edges: &[i32],
    face_normals: &[[f32; 3]],
    split_angle: f32,
    sharp_edges: &mut [bool],
) {
    if split_angle >= std::f32::consts::PI {
        return;
    }
    let split_angle_cos = libm::cosf(split_angle);
    let mut corner_face = vec![0usize; corner_verts.len()];
    for (f, w) in face_offsets.windows(2).enumerate() {
        corner_face[w[0] as usize..w[1] as usize].fill(f);
    }
    let mut e2l = vec![[0i32; 2]; sharp_edges.len()];
    for (face_i, w) in face_offsets.windows(2).enumerate() {
        for loop_index in w[0] as usize..w[1] as usize {
            let vert_i = corner_verts[loop_index];
            let edge_i = corner_edges[loop_index];
            let Some(e) = usize::try_from(edge_i).ok().filter(|e| *e < e2l.len()) else {
                continue;
            };
            let l = &mut e2l[e];
            if (l[0] | l[1]) == 0 {
                l[0] = loop_index as i32;
                // Every face counts as smooth here (no sharp faces).
                l[1] = INDEX_UNSET;
            } else if l[1] == INDEX_UNSET {
                let n0 = Vec3::from_array(face_normals[corner_face[l[0] as usize]]);
                let n1 = Vec3::from_array(face_normals[face_i]);
                let is_angle_sharp = n0.dot(n1) < split_angle_cos;
                if sharp_edges[e] || vert_i == corner_verts[l[0] as usize] || is_angle_sharp {
                    l[1] = INDEX_INVALID;
                    if is_angle_sharp {
                        sharp_edges[e] = true;
                    }
                } else {
                    l[1] = loop_index as i32;
                }
            } else if l[1] != INDEX_INVALID {
                // More than two corners use this edge.
                l[1] = INDEX_INVALID;
                sharp_edges[e] = false;
            }
        }
    }
}

/// `mesh_sharp_edges_set_from_angle`.
pub fn sharp_edges_set_from_angle(mesh: &mut Mesh, angle: f32, keep_sharp_edges: bool) {
    if angle >= std::f32::consts::PI {
        smooth_set(mesh, true, keep_sharp_edges);
        return;
    }
    if angle == 0.0 {
        smooth_set(mesh, false, keep_sharp_edges);
        return;
    }
    {
        let attrs = mesh.attributes_mut();
        if !keep_sharp_edges {
            attrs.remove(SHARP_EDGE);
        }
        attrs.remove(SHARP_FACE);
    }
    let n = mesh.edges_num();
    let mut sharp = mesh
        .bool_attribute(SHARP_EDGE, Domain::Edge)
        .map(|v| v.to_vec())
        .unwrap_or_else(|| vec![false; n]);
    sharp.resize(n, false);
    let normals = mesh.face_normals();
    edges_sharp_from_angle(
        mesh.face_offsets(),
        mesh.corner_verts(),
        mesh.corner_edges(),
        &normals,
        angle,
        &mut sharp,
    );
    mesh.set_bool_attribute(SHARP_EDGE, Domain::Edge, sharp);
}

/// `shade_poll`: an editable scene, and an active object (if any) that is not in Edit or
/// Sculpt Mode, has data and is local (verified: an active Empty fails the poll with the
/// default message).
pub fn shade_poll(c: &Context) -> Result<(), PollFail> {
    poll::scene_editable(c)?;
    let Some((_, ob)) = poll::active(c) else {
        return Ok(());
    };
    if ob.mode == ObjectMode::Edit {
        return Err(poll::fail(
            PollReq::ModeObject,
            "bpy.ops.object.mode_set(mode='OBJECT')",
        ));
    }
    if ob.data.is_none() {
        return Err(poll::fail(
            PollReq::ActiveObject,
            "make an object with data active, e.g. bpy.context.view_layer.objects.active = \
             bpy.context.selected_objects[0]",
        ));
    }
    let data_linked = ob
        .data
        .and_then(|d| c.main.header(d))
        .is_some_and(|h| h.library.is_some());
    if ob.header.library.is_some() || data_linked {
        return Err(poll::fail(
            PollReq::ActiveEditable,
            format!(
                "bpy.data.objects[{}].make_local()",
                poll::py_name(&ob.header.name)
            ),
        ));
    }
    Ok(())
}

/// `shade_poll` as a declarative poll.
pub const SHADE_POLL: Poll = Poll {
    requires: &[
        PollReq::SceneEditable,
        PollReq::ModeObject,
        PollReq::ActiveEditable,
    ],
    check: shade_poll,
};

/// What a shade operator does to each mesh.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shade {
    /// `shade_smooth`
    Smooth,
    /// `shade_flat`
    Flat,
    /// `shade_smooth_by_angle(angle)`
    ByAngle(f32),
}

/// `shade_smooth_exec`: the data of the selected editable objects (the active object alone
/// when it is in a paint mode; frap has none), each mesh once; linked data is skipped with a
/// WARNING; nothing changed → `CANCELLED`.
pub fn run(cx: &mut frap_op::OpCtx, how: Shade, keep_sharp_edges: bool) -> frap_op::OpResult {
    use frap_op::prelude::*;
    use frap_rna::{IdRef, MeshId};
    let (meshes, linked) = {
        let c = cx.ctx();
        let mut meshes: Vec<IdRef<MeshId>> = Vec::new();
        let mut linked = false;
        for ob in c.selected_editable_objects() {
            let Some(d) = c.main.objects.get(ob).and_then(|o| o.data) else {
                continue;
            };
            if c.main.header(d).is_some_and(|h| h.library.is_some()) {
                linked = true;
                continue;
            }
            if let Some(m) = d.typed::<MeshId>()
                && !meshes.contains(&m)
            {
                meshes.push(m);
            }
        }
        (meshes, linked)
    };
    // `shade_flat` / `shade_smooth` remove the first "Smooth by Angle" modifier of each
    // selected mesh object (`is_smooth_by_angle_modifier`, WP 4.1).
    if matches!(how, Shade::Smooth | Shade::Flat) {
        let objects = cx.ctx().selected_editable_objects();
        for ob in objects {
            let i = {
                let main = cx.main();
                main.objects
                    .get(ob)
                    .filter(|o| o.object_type() == "MESH")
                    .and_then(|o| {
                        o.modifiers.iter().position(|m| {
                            frap_modifiers::types::nodes::is_smooth_by_angle_modifier(main, m)
                        })
                    })
            };
            if let Some(i) = i {
                frap_modifiers::stack::edit::remove(&mut cx.object_mut(ob)?.modifiers, i);
            }
        }
    }
    for m in &meshes {
        let mesh = cx.mesh_mut(*m)?;
        match how {
            Shade::Smooth => smooth_set(mesh, true, keep_sharp_edges),
            Shade::Flat => smooth_set(mesh, false, keep_sharp_edges),
            Shade::ByAngle(a) => sharp_edges_set_from_angle(mesh, a, keep_sharp_edges),
        }
    }
    if linked {
        cx.report(ReportType::Warning, "Can't edit linked mesh or curve data");
    }
    if meshes.is_empty() {
        cx.report(
            ReportType::Warning,
            "frap: nothing to shade: no selected, editable object has mesh data",
        );
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Mesh {
        frap_mesh::primitives::cube(2.0, true, [1.0; 3])
    }

    #[test]
    fn smooth_and_flat() {
        let mut m = cube();
        m.set_bool_attribute(SHARP_EDGE, Domain::Edge, vec![false; 12]);
        smooth_set(&mut m, true, true);
        assert!(m.attributes().get(SHARP_FACE).is_none());
        assert!(m.attributes().get(SHARP_EDGE).is_some());
        smooth_set(&mut m, false, true);
        let names: Vec<_> = m.attributes().iter().map(|a| a.name.to_string()).collect();
        assert_eq!(names.last().map(String::as_str), Some(SHARP_FACE));
        smooth_set(&mut m, false, false);
        assert!(m.attributes().get(SHARP_EDGE).is_none());
    }

    #[test]
    fn cube_by_angle() {
        let mut m = cube();
        // The operator's default angle (30 degrees).
        sharp_edges_set_from_angle(&mut m, 30f32.to_radians(), true);
        assert_eq!(
            m.bool_attribute(SHARP_EDGE, Domain::Edge),
            Some(&[true; 12][..])
        );
        assert!(m.attributes().get(SHARP_FACE).is_none());
        let mut m = cube();
        sharp_edges_set_from_angle(&mut m, 1.6, true);
        assert_eq!(
            m.bool_attribute(SHARP_EDGE, Domain::Edge),
            Some(&[false; 12][..])
        );
        let mut m = cube();
        sharp_edges_set_from_angle(&mut m, 0.0, true);
        assert!(m.attributes().get(SHARP_FACE).is_some());
        sharp_edges_set_from_angle(&mut m, std::f32::consts::PI, true);
        assert!(m.attributes().get(SHARP_FACE).is_none());
        assert!(m.attributes().get(SHARP_EDGE).is_none());
    }
}
