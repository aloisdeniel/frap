// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.visual_geometry_to_objects()`, tier T1 (the evaluated mesh,
//! `tests/golden/cases/object/data_m8/visual_geometry.py`). Port of
//! `visual_geometry_to_objects_exec` and `GeometryToObjectsBuilder`
//! (`editors/object/object_visual_geometry_to_objects.cc`), `object_get_evaluated_geometry_set`
//! (`blenkernel/intern/geometry_set_instances.cc`), `BKE_id_material_used_eval` and
//! `BKE_object_material_get_eval` (`blenkernel/intern/material.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Verified on 5.2.4, for every selected object (base order):
//! - a mesh object with vertices gets a new mesh object holding its **evaluated** mesh
//!   (modifiers applied), both named after the source object (`Cube.001`); two sources that
//!   share one evaluated mesh (linked data, no modifiers) share one new object;
//! - the new data gets as many materials as the highest face material index + 1, each the
//!   source slot's effective material (an object-linked slot's own one when set), all on the
//!   data;
//! - a point cloud with points gets a copy of its point cloud the same way;
//! - an empty instancing a collection gets a new empty, named after the collection,
//!   instancing it;
//! - cameras, lights, armatures and plain empties give nothing;
//! - each new object is linked to every collection holding the source (the scene's master
//!   collection, then `bpy.data.collections`), takes the source's local transform
//!   (`BKE_object_to_mat4`, deltas included) decomposed as XYZ Euler (`to_loc_rot_scale_safe`)
//!   and the source's parent and `matrix_parent_inverse`;
//! - then every base of the view layer is deselected, the new objects are selected and the
//!   first one is active; always `FINISHED` (with nothing new, everything is deselected).
//!
//! Geometry-nodes instances (and the instance collections Blender builds for them) do not
//! exist in frap: only an instancing empty yields an instance. A mesh whose stack holds a
//! modifier frap cannot evaluate answers `OP_NOT_IMPLEMENTED` (as `object.convert`) rather
//! than writing an unmodified mesh.

use std::sync::Arc;

use frap_math::glam::Mat4;
use frap_math::{EulerOrder, euler, matrix};
use frap_mesh::{AttrData, Mesh};
use frap_modifiers::stack;
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::{CollectionId, IdAny, IdCode, IdHeader, IdRef, MaterialId, ObjectId, PointCloudId};
use frap_scene::id::layers;
use frap_scene::id::{Object, slots};
use frap_scene::{BaseFlag, Main, ModifierStatus};
use frap_spec_ops_object::object;

use super::collection__objects_add_active::common::find_order;
use super::move_to_collection::common::link_object;

/// What one source object yields (`ComponentObjects`).
#[derive(Default)]
struct ComponentObjects {
    mesh_ob: Option<IdRef<ObjectId>>,
    pointcloud_ob: Option<IdRef<ObjectId>>,
    instance_objects: Vec<IdRef<ObjectId>>,
}

impl ComponentObjects {
    fn all_objects(&self) -> Vec<IdRef<ObjectId>> {
        self.mesh_ob
            .into_iter()
            .chain(self.pointcloud_ob)
            .chain(self.instance_objects.iter().copied())
            .collect()
    }
}

/// `set_local_object_transform`: `m` decomposed (`to_loc_rot_scale_safe<true>`) into the
/// location, XYZ Euler rotation and scale; the rotation mode becomes `XYZ`.
fn set_local_object_transform(ob: &mut Object, m: &Mat4) {
    let (loc, rot, scale) = matrix::mat4_to_loc_rot_size(m);
    ob.location = loc.to_array();
    ob.rotation_euler = euler::mat3_normalized_to_eul_o(&rot, EulerOrder::XYZ);
    ob.scale = scale.to_array();
    ob.rotation_mode = "XYZ".into();
}

/// The highest value of an INT material-index attribute + 1 (`BKE_id_material_used_eval`):
/// 0 without elements, 1 without the attribute.
fn materials_used(attr: Option<&AttrData>, elements: usize) -> usize {
    if elements == 0 {
        return 0;
    }
    match attr {
        Some(AttrData::Int(v)) => v.iter().copied().max().unwrap_or(0).clamp(0, 32766) as usize + 1,
        _ => 1,
    }
}

/// `copy_materials_to_new_geometry_object`: `BKE_object_material_get_eval(src, data_eval,
/// i + 1)` for each used slot (clamped to the slot count; an object-linked slot's material when
/// it has one, else the evaluated data's).
fn eval_materials(
    main: &Main,
    src: &Object,
    data_materials: &[Option<IdRef<MaterialId>>],
    used: usize,
) -> Vec<Option<IdRef<MaterialId>>> {
    let ob_slots = slots::slot_count(main, src);
    let slots_num = ob_slots.max(data_materials.len());
    (0..used)
        .map(|i| {
            if slots_num == 0 {
                return None;
            }
            let s = i.min(slots_num - 1);
            if s < ob_slots
                && let (frap_scene::id::SlotLink::Object, Some(m)) = slots::slot(main, src, s)
            {
                return Some(m);
            }
            data_materials.get(s).copied().flatten()
        })
        .collect()
}

/// `GeometryToObjectsBuilder`: the new objects by generated geometry (the evaluated mesh's
/// `Arc`, the point cloud), so shared geometry yields one object.
#[derive(Default)]
struct Builder {
    by_mesh: Vec<(*const Mesh, IdRef<ObjectId>)>,
    by_pointcloud: Vec<(IdRef<PointCloudId>, IdRef<ObjectId>)>,
}

impl Builder {
    fn mesh_object(
        &mut self,
        cx: &mut OpCtx,
        src: IdRef<ObjectId>,
        eval: &Arc<Mesh>,
    ) -> Option<IdRef<ObjectId>> {
        let key = Arc::as_ptr(eval);
        if let Some(&(_, o)) = self.by_mesh.iter().find(|(k, _)| *k == key) {
            return Some(o);
        }
        let (main, log) = cx.main_mut();
        let s = main.objects.get(src)?;
        let name = s.header.name.clone();
        let used = materials_used(
            eval.attributes().get("material_index").map(|a| &a.data),
            eval.faces_num(),
        );
        let materials = eval_materials(main, s, &eval.materials, used);
        let mut me: Mesh = (**eval).clone();
        me.header = IdHeader::new(name.as_str());
        me.shape_keys = None;
        me.gltf2_variants = Default::default();
        me.materials = materials;
        let me = main.meshes.insert(&name, me, log);
        let ob = main
            .objects
            .insert(&name, Object::new(&name, Some(me.any())), log);
        self.by_mesh.push((key, ob));
        Some(ob)
    }

    fn pointcloud_object(
        &mut self,
        cx: &mut OpCtx,
        src: IdRef<ObjectId>,
        pc: IdRef<PointCloudId>,
    ) -> Option<IdRef<ObjectId>> {
        if let Some(&(_, o)) = self.by_pointcloud.iter().find(|(k, _)| *k == pc) {
            return Some(o);
        }
        let (main, log) = cx.main_mut();
        let s = main.objects.get(src)?;
        let name = s.header.name.clone();
        let src_pc = main.pointclouds.get(pc)?;
        let used = materials_used(
            src_pc.attributes().get("material_index").map(|a| &a.data),
            src_pc.len(),
        );
        let materials = eval_materials(main, s, &src_pc.materials, used);
        let mut copy = src_pc.clone();
        copy.header = IdHeader::new(name.as_str());
        copy.animation_data = None;
        copy.materials = materials;
        let pc_new = main.pointclouds.insert(&name, copy, log);
        let ob = main
            .objects
            .insert(&name, Object::new(&name, Some(pc_new.any())), log);
        self.by_pointcloud.push((pc, ob));
        Some(ob)
    }

    /// `create_objects_for_instances` for the one instance of an instancing empty (a
    /// collection reference with the identity transform: the fallback path, one empty that
    /// instances the collection, named after it).
    fn collection_instance(cx: &mut OpCtx, coll: IdRef<CollectionId>) -> Option<IdRef<ObjectId>> {
        let (main, log) = cx.main_mut();
        let name = main.collections.get(coll)?.header.name.clone();
        let mut ob = Object::new(&name, None);
        ob.instance_type = "COLLECTION".into();
        ob.instance_collection = Some(coll);
        Some(main.objects.insert(&name, ob, log))
    }
}

/// The evaluated mesh of a mesh object, refusing a stack frap cannot evaluate.
fn evaluated_mesh(
    cx: &mut OpCtx,
    ob: IdRef<ObjectId>,
    name: &str,
) -> Result<Option<Arc<Mesh>>, OpError> {
    let eo = cx.evaluated(ob)?;
    if let Some((m, st)) = eo
        .modifier_status
        .iter()
        .find(|(_, st)| stack::status_is_frap_limitation(st))
    {
        if let ModifierStatus::Error(e) = st
            && stack::is_limit_error(e)
        {
            return Err(OpError::Limit(e.clone()));
        }
        let why = match st {
            ModifierStatus::Error(e) => e.clone(),
            _ => "its type has no frap evaluator".to_owned(),
        };
        return Err(OpError::NotImplemented(format!(
            "frap: object.visual_geometry_to_objects cannot evaluate modifier '{m}' of object \
             '{name}' ({why})"
        )));
    }
    for w in &eo.warnings {
        cx.report(w.level, w.message.clone());
    }
    Ok(eo.mesh.clone())
}

#[operator(
    poll = poll::ACTIVE_OBJECT,
    tier = T1,
    support = Partial(
        "meshes (evaluated), point clouds and collection-instancing empties; frap has no \
         geometry-nodes instances"
    )
)]
fn exec(cx: &mut OpCtx, _p: object::VisualGeometryToObjects) -> OpResult {
    let (selected, scene, layer) = {
        let c = cx.ctx();
        let vl = c.view_layer();
        let layer = c
            .scene()
            .view_layers
            .iter()
            .position(|v| v.name == vl.name)
            .unwrap_or(0);
        (c.selected_objects(), c.win.scene, layer)
    };
    let mut builder = Builder::default();
    let mut all_new: Vec<IdRef<ObjectId>> = Vec::new();
    for src in selected {
        let Some(o) = cx.main().objects.get(src) else {
            continue;
        };
        let (name, data, instance) = (
            o.header.name.clone(),
            o.data,
            o.instance_collection.filter(|_| o.data.is_none()),
        );
        let mut parts = ComponentObjects::default();
        match data.map(|d: IdAny| d.code) {
            Some(IdCode::ME) => {
                if let Some(eval) = evaluated_mesh(cx, src, &name)?
                    && eval.verts_num() > 0
                {
                    parts.mesh_ob = builder.mesh_object(cx, src, &eval);
                }
            }
            Some(IdCode::PT) => {
                if let Some(pc) = data.and_then(|d| d.typed::<PointCloudId>())
                    && cx.main().pointclouds.get(pc).is_some_and(|p| !p.is_empty())
                {
                    parts.pointcloud_ob = builder.pointcloud_object(cx, src, pc);
                }
            }
            None => {
                if let Some(coll) = instance {
                    parts
                        .instance_objects
                        .extend(Builder::collection_instance(cx, coll));
                }
            }
            _ => {}
        }
        let top = parts.all_objects();
        all_new.extend(top.iter().copied());
        let (main, log) = cx.main_mut();
        let collections = find_order(main, scene, src);
        let Some(s) = main.objects.get(src) else {
            continue;
        };
        let (src_local, parent, parentinv) = (s.matrix_basis(), s.parent, s.matrix_parent_inverse);
        for ob in top {
            for &c in &collections {
                link_object(main, log, scene, c, ob);
            }
            if let Some(o) = main.objects.get_mut(ob, log) {
                let old = o.matrix_basis();
                set_local_object_transform(o, &(src_local * old));
                o.parent = parent;
                o.matrix_parent_inverse = parentinv;
            }
        }
    }
    let (main, log) = cx.main_mut();
    layers::sync_all(main, log);
    // `BKE_view_layer_base_deselect_all`, then the new objects selected, the first active.
    if let Some(s) = main.scenes.get_mut(scene, log)
        && let Some(vl) = s.view_layers.get_mut(layer)
    {
        for b in &mut vl.bases {
            b.flag.remove(BaseFlag::SELECTED);
        }
        for &ob in &all_new {
            if let Some(b) = vl.base_mut(ob) {
                b.flag.insert(BaseFlag::SELECTED);
            }
        }
        if let Some(&first) = all_new.first()
            && vl.base(first).is_some()
        {
            vl.active = Some(first);
        }
    }
    log.context_changed = true;
    Ok(OpStatus::Finished)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn used_materials_follow_the_highest_index() {
        assert_eq!(materials_used(None, 0), 0);
        assert_eq!(materials_used(None, 6), 1);
        let idx = AttrData::Int(Arc::new(vec![0, 2, 1]));
        assert_eq!(materials_used(Some(&idx), 3), 3);
        let neg = AttrData::Int(Arc::new(vec![-4, -1]));
        assert_eq!(materials_used(Some(&neg), 2), 1);
    }

    #[test]
    fn transforms_decompose_as_xyz_euler() {
        let mut ob = Object::new("E", None);
        ob.rotation_mode = "QUATERNION".into();
        let m = Mat4::from_scale_rotation_translation(
            frap_math::glam::Vec3::new(2.0, 1.0, 0.5),
            frap_math::glam::Quat::from_rotation_z(0.5),
            frap_math::glam::Vec3::new(1.0, 2.0, 3.0),
        );
        set_local_object_transform(&mut ob, &m);
        assert_eq!(ob.rotation_mode, "XYZ");
        assert!((ob.rotation_euler[2] - 0.5).abs() < 1e-6);
        assert!((ob.scale[0] - 2.0).abs() < 1e-6);
        assert_eq!(ob.location, [1.0, 2.0, 3.0]);
    }
}
