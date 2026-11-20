// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.object.armature_add(radius=1, enter_editmode=False, align='WORLD', location=…,
//! rotation=…, scale=…)`, tier T0 (`tests/golden/cases/armature/add/**`; WP 6.2). Port of
//! `object_armature_add_exec` (blender/source/blender/editors/object/object_add.cc) and
//! `ED_armature_ebone_add_primitive` / `ED_armature_ebone_add`
//! (blender/source/blender/editors/armature/armature_add.cc), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4: object and data are both requested as `Armature` and uniquified
//! independently; placement like the primitives (cursor, `align`, a set `rotation` wins;
//! `scale` is ignored); the armature gets its default bone collection `Bones` (active) and one
//! bone `Bone` from the origin to `(0, 0, radius)` in it, active, with only its tail selected;
//! leaving Edit Mode builds the rest matrices and the pose (one `QUATERNION` channel, not
//! selected). With `enter_editmode=True` the object stays in Edit Mode (no pose yet, as in
//! Blender). Object Mode only (`ED_operator_objectmode`).

use std::sync::Arc;

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::ObjectMode;
use frap_scene::edit_armature::EditArmature;
use frap_scene::id::{Armature, BoneCollection, Pose};
use frap_spec_ops_object::object;

#[operator(poll = poll::OBJECTMODE, tier = T0)]
fn exec(cx: &mut OpCtx, p: object::ArmatureAdd) -> OpResult {
    let align = helpers::Align::from_identifier(p.align.value.identifier())
        .unwrap_or(helpers::Align::World);
    let at = super::camera_add::common::placement(cx, &p.location, &p.rotation, align, &p.scale)?;
    let mut arm = Armature::new("Armature");
    // `ANIM_armature_bonecoll_new(armature, "")` + `ANIM_armature_bonecoll_active_set`.
    arm.collections.push(BoneCollection {
        name: "Bones".into(),
        parent: None,
        is_visible: true,
        is_solo: false,
    });
    arm.active_collection = Some(0);
    // `ED_armature_ebone_add_primitive(obedit, radius, false)`.
    let mut ea = EditArmature::default();
    let b = ea.add_bone("Bone");
    ea.edit_bones[b].tail = [0.0, 0.0, p.radius.value];
    ea.edit_bones[b].collections.push("Bones".into());
    ea.active = Some(b);
    let data = {
        let (main, log) = cx.main_mut();
        main.armatures.insert("Armature", arm, log)
    };
    let ob = helpers::add_object_with_data(cx, "Armature", Some(data.any()), at, false)?;
    if p.enter_editmode.value {
        let (_, edit) = cx.split_edit();
        edit.armatures.insert(data.key(), Arc::new(ea));
        cx.object_mut(ob)?.mode = ObjectMode::Edit;
        cx.main_mut().1.context_changed = true;
    } else {
        // `editmode_exit_ex`: `ED_armature_from_edit`, `BKE_pose_rebuild`,
        // `flush_bone_selection_to_pose`.
        let a = {
            let (main, log) = cx.main_mut();
            let a = main
                .armatures
                .get_mut(data, log)
                .ok_or_else(|| OpError::Runtime("Cannot create editmode armature".into()))?;
            ea.apply_to(a);
            a.clone()
        };
        let o = cx.object_mut(ob)?;
        Pose::rebuild(&mut o.pose, &a);
        if let Some(pose) = o.pose.as_mut() {
            pose.flush_selection_from(&a);
        }
    }
    cx.result("object", ob.any());
    Ok(OpStatus::Finished)
}
