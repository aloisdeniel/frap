// SPDX-License-Identifier: GPL-3.0-or-later
//! Object creation for every vnode, depth first: meshes (one Blender mesh per glTF mesh and
//! cache key), cameras, lights, armatures (with their bones, WP 6.8) and empties, with their
//! transform, parent (an object, or the bone of an armature) and collections; skinned meshes
//! get an ARMATURE modifier, meshes with morph targets their weights as shape-key values.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/node.py` (`BlenderNode.create_vnode`,
//! `create_object`, `calc_empty_display_size`, `create_mesh_object`, `set_morph_weights`,
//! `setup_skinning`; the armature parts in `skins.rs`), Apache-2.0, The glTF-Blender-IO
//! authors.
//!
//! A bone vnode records its armature's object as `blender_object` (the active object of a
//! scene whose first root is a joint is that armature, `set_active_object`).

use frap_rna::{IdAny, IdRef, MeshId, ObjectId};
use frap_scene::id::Object;
use frap_scene::{ChangeLog, Main};

use super::convert::length_f64;
use super::scenes::{CollTarget, link_object};
use super::settings::BoneHeuristic;
use super::vnode::VKind;
use super::{Gltf, ImportError, MeshKey, cameras, extras, lights, mesh, morph, skins};

/// `BlenderNode.create_vnode`: creates `root` and its descendants, pre-order (an armature
/// object's bones right after it).
pub(crate) fn create_vnode(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    root: usize,
) -> Result<(), ImportError> {
    skins::compute_bones(cx)?;
    let mut stack = vec![root];
    let mut visited = vec![false; cx.vtree.nodes.len()];
    while let Some(id) = stack.pop() {
        // A node listed twice (invalid glTF) is created once.
        if std::mem::replace(&mut visited[id], true) {
            continue;
        }
        if cx.vtree.nodes[id].kind == VKind::Object {
            let (ob, bone_shape) = create_object(cx, main, log, id)?;
            cx.vtree.nodes[id].blender_object = Some(ob);
            cx.objects.push(ob);
            if cx.vtree.nodes[id].is_arma {
                skins::create_bones(cx, main, log, id, bone_shape);
            }
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

/// `BlenderNode.create_object`; also returns the bone shape of a new armature (`BLENDER`
/// heuristic).
fn create_object(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    id: usize,
) -> Result<(IdRef<ObjectId>, Option<IdRef<ObjectId>>), ImportError> {
    let v = cx.vtree.nodes[id].clone();
    let mut empty_size = None;
    let mut mesh_node = None;
    let (name, data): (String, Option<IdAny>) = if let Some(node_idx) = v.mesh_node_idx {
        let (name, data, me) = create_mesh_object(cx, main, log, &v.name, node_idx)?;
        mesh_node = me.map(|me| (node_idx, me));
        (name, data)
    } else if let Some(node_idx) = v.camera_node_idx {
        let cam_idx = cx.doc.nodes[node_idx].camera.unwrap_or(0) as usize;
        let cam = cameras::create(cx, main, log, cam_idx)?;
        let cam_name = main.cameras.name_of(cam).unwrap_or("Camera").to_owned();
        (v.name.clone().unwrap_or(cam_name), Some(cam.any()))
    } else if let Some(node_idx) = v.light_node_idx {
        let la = lights::create(cx, main, log, node_idx)?;
        let la_name = main.lights.name_of(la).unwrap_or("Light").to_owned();
        (v.name.clone().unwrap_or(la_name), Some(la.any()))
    } else if v.is_arma {
        let arm = skins::create_armature(cx, main, log, id);
        let arm_name = arm
            .typed::<frap_rna::ArmatureId>()
            .and_then(|a| main.armatures.name_of(a))
            .unwrap_or("Armature")
            .to_owned();
        (v.name.clone().unwrap_or(arm_name), Some(arm))
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
        match pv.kind {
            VKind::Object => ob.parent = pv.blender_object,
            VKind::Bone => {
                // Parented to the bone, moved back from its tip to its root.
                ob.parent = pv.bone_arma.and_then(|a| cx.vtree.nodes[a].blender_object);
                ob.parent_type = "BONE".into();
                ob.parent_bone = pv.blender_bone_name.clone().unwrap_or_default();
                ob.location[1] += -(pv.bone_length as f32);
            }
            VKind::DummyRoot => {}
        }
    }
    let ob = main.objects.insert(&name, ob, log);
    let mut bone_shape = None;
    if v.is_arma && cx.settings.bone_heuristic == BoneHeuristic::Blender {
        bone_shape = skins::armature_display(cx, main, log, ob);
    }

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

    // Morph weights and skinning of a mesh object.
    if let Some((node_idx, me)) = mesh_node {
        let mesh_idx = cx.doc.nodes[node_idx].mesh.unwrap_or(0) as usize;
        if mesh::has_shape_keys(&cx.doc.meshes[mesh_idx]) {
            morph::set_morph_weights(&cx.doc, main, log, node_idx, mesh_idx, me);
        }
        if let Some(skin) = cx.doc.nodes[node_idx].skin.map(|s| s as usize)
            && let Some(arma_ob) = cx
                .doc
                .skins
                .get(skin)
                .and_then(|s| s.joints.first())
                .and_then(|&j| cx.vtree.nodes.get(j as usize))
                .and_then(|b| b.bone_arma)
                .and_then(|a| cx.vtree.nodes[a].blender_object)
        {
            skins::setup_skinning(main, log, ob, arma_ob);
        }
    }
    Ok((ob, bone_shape))
}

/// What [`create_mesh_object`] gives: the object name, its data, and the data when it is a mesh.
type MeshObject = (String, Option<IdAny>, Option<IdRef<MeshId>>);

/// `BlenderNode.create_mesh_object`: the (cached) mesh of glTF node `node_idx`, the object
/// name, and the mesh when it is a Blender mesh.
fn create_mesh_object(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    vname: &Option<String>,
    node_idx: usize,
) -> Result<MeshObject, ImportError> {
    let node = &cx.doc.nodes[node_idx];
    let mesh_idx = node.mesh.unwrap_or(0) as usize;
    if mesh_idx >= cx.doc.meshes.len() {
        // Invalid reference: an empty object, as the add-on does.
        return Ok((
            vname.clone().unwrap_or_else(|| "Invalid Mesh Index".into()),
            None,
            None,
        ));
    }
    let has_shapekeys = mesh::has_shape_keys(&cx.doc.meshes[mesh_idx]);
    let skin = node
        .skin
        .map(|s| s as usize)
        .filter(|&s| s < cx.doc.skins.len());
    // Same key = instances of one Blender mesh; a node whose morph weights are animated never
    // shares (Blender's shape keys live on the mesh).
    let key = if has_shapekeys && morph::weight_animated_nodes(&cx.doc).contains(&node_idx) {
        None
    } else {
        Some(MeshKey {
            mesh: mesh_idx,
            skin: node.skin,
            weights: if has_shapekeys {
                node.weights.iter().map(|w| w.to_bits()).collect()
            } else {
                Vec::new()
            },
        })
    };
    // A mesh or, with `import_point_as_pointcloud` and only POINTS primitives, a point cloud
    // (WP 3.7, `mesh::create_data`).
    let data = match key.as_ref().and_then(|k| cx.mesh_cache.get(k)) {
        Some(&m) => m,
        None => {
            let m = mesh::create_data(cx, main, log, mesh_idx, skin)?;
            if let Some(k) = key {
                cx.mesh_cache.insert(k, m);
            }
            m
        }
    };
    let data_name = mesh::data_name(main, data);
    Ok((vname.clone().unwrap_or(data_name), Some(data), data.typed()))
}
