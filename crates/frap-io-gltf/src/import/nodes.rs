// SPDX-License-Identifier: GPL-3.0-or-later
//! Object creation for every vnode, depth first: meshes (one Blender mesh per glTF mesh and
//! cache key), cameras, lights and empties, with their transform, parent and collections.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/node.py` (`BlenderNode.create_vnode`,
//! `create_object`, `calc_empty_display_size`, `create_mesh_object`), Apache-2.0, The
//! glTF-Blender-IO authors. Armatures, bones and skinning arrive in M6.

use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::id::Object;
use frap_scene::{ChangeLog, Main};

use super::convert::length_f64;
use super::scenes::{CollTarget, link_object};
use super::vnode::VKind;
use super::{Gltf, ImportError, MeshKey, cameras, extras, lights, mesh};

/// `BlenderNode.create_vnode`: creates `root` and its descendants, pre-order.
pub(crate) fn create_vnode(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    root: usize,
) -> Result<(), ImportError> {
    let mut stack = vec![root];
    let mut visited = vec![false; cx.vtree.nodes.len()];
    while let Some(id) = stack.pop() {
        // A node listed twice (invalid glTF) is created once.
        if std::mem::replace(&mut visited[id], true) {
            continue;
        }
        if cx.vtree.nodes[id].kind == VKind::Object {
            let ob = create_object(cx, main, log, id)?;
            cx.vtree.nodes[id].blender_object = Some(ob);
            cx.objects.push(ob);
        }
        for &c in cx.vtree.nodes[id].children.iter().rev() {
            if c < visited.len() {
                stack.push(c);
            }
        }
    }
    Ok(())
}

/// `calc_empty_display_size`: 0.4 × the smallest distance to the parent or a child.
fn empty_display_size(cx: &Gltf, id: usize) -> f32 {
    let v = &cx.vtree.nodes[id];
    let sizes = std::iter::once(id)
        .chain(v.children.iter().copied())
        .filter_map(|vid| cx.vtree.nodes.get(vid))
        .map(|n| length_f64(n.trs().0) * 0.4);
    let min = sizes.fold(None, |m: Option<f64>, x| Some(m.map_or(x, |m| m.min(x))));
    let size = min.unwrap_or(1.0).max(0.001);
    // RNA hard range of `empty_display_size`.
    (size as f32).clamp(0.0001, 1000.0)
}

/// `BlenderNode.create_object`.
fn create_object(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    id: usize,
) -> Result<IdRef<ObjectId>, ImportError> {
    let v = cx.vtree.nodes[id].clone();
    let mut empty_size = None;
    let (name, data): (String, Option<IdAny>) = if let Some(node_idx) = v.mesh_node_idx {
        create_mesh_object(cx, main, log, &v.name, node_idx)?
    } else if let Some(node_idx) = v.camera_node_idx {
        let cam_idx = cx.doc.nodes[node_idx].camera.unwrap_or(0) as usize;
        let cam = cameras::create(cx, main, log, cam_idx)?;
        let cam_name = main.cameras.name_of(cam).unwrap_or("Camera").to_owned();
        (v.name.clone().unwrap_or(cam_name), Some(cam.any()))
    } else if let Some(node_idx) = v.light_node_idx {
        let la = lights::create(cx, main, log, node_idx)?;
        let la_name = main.lights.name_of(la).unwrap_or("Light").to_owned();
        (v.name.clone().unwrap_or(la_name), Some(la.any()))
    } else {
        empty_size = Some(empty_display_size(cx, id));
        (
            v.name.clone().unwrap_or_else(|| v.default_name.clone()),
            None,
        )
    };
    let mut ob = Object::new("", data);
    if let Some(s) = empty_size {
        ob.empty_display_size = s;
    }
    let (t, r, s) = v.trs();
    ob.location = t;
    ob.rotation_mode = "QUATERNION".into();
    ob.rotation_quaternion = r;
    ob.scale = s;
    if let Some(n) = v.gltf_node {
        extras::set_extras(&mut ob.header, cx.doc.nodes[n].extras.as_ref(), &[]);
    }
    if let Some(p) = v.parent {
        let pv = &cx.vtree.nodes[p];
        if pv.kind == VKind::Object {
            ob.parent = pv.blender_object;
        }
    }
    let ob = main.objects.insert(&name, ob, log);

    // Link into the collections of the vnode's glTF scenes.
    if v.scenes.is_empty() {
        if let Some(&t) = cx.collections.get(&None) {
            link_object(main, t, ob, log);
        }
    } else {
        for &c in &v.scenes {
            let target = if cx.settings.import_scene_as_collection {
                cx.collections.get(&Some(c)).copied()
            } else if cx.doc.scenes.len() == 1 {
                cx.blender_scenes.get(c).map(|s| CollTarget::Master(*s))
            } else if Some(c) == cx.doc.scene.map(|s| s as usize) {
                cx.collections.get(&Some(c)).copied()
            } else {
                cx.blender_scenes.get(c).map(|s| CollTarget::Master(*s))
            };
            if let Some(t) = target {
                link_object(main, t, ob, log);
            }
        }
    }
    Ok(ob)
}

/// `BlenderNode.create_mesh_object`: the (cached) mesh of glTF node `node_idx` and the object
/// name.
fn create_mesh_object(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    vname: &Option<String>,
    node_idx: usize,
) -> Result<(String, Option<IdAny>), ImportError> {
    let node = &cx.doc.nodes[node_idx];
    let mesh_idx = node.mesh.unwrap_or(0) as usize;
    if mesh_idx >= cx.doc.meshes.len() {
        // Invalid reference: an empty object, as the add-on does.
        return Ok((
            vname.clone().unwrap_or_else(|| "Invalid Mesh Index".into()),
            None,
        ));
    }
    let has_shapekeys = mesh::has_shape_keys(&cx.doc.meshes[mesh_idx]);
    let key = MeshKey {
        mesh: mesh_idx,
        skin: node.skin,
        weights: if has_shapekeys {
            node.weights.iter().map(|w| w.to_bits()).collect()
        } else {
            Vec::new()
        },
    };
    // A mesh or, with `import_point_as_pointcloud` and only POINTS primitives, a point cloud
    // (WP 3.7, `mesh::create_data`).
    let data = match cx.mesh_cache.get(&key) {
        Some(&m) => m,
        None => {
            let m = mesh::create_data(cx, main, log, mesh_idx)?;
            cx.mesh_cache.insert(key, m);
            m
        }
    };
    let data_name = mesh::data_name(main, data);
    Ok((vname.clone().unwrap_or(data_name), Some(data)))
}
