// SPDX-License-Identifier: GPL-3.0-or-later
//! `ED_armature_bone_rename` (WP 6.2): the setter of `Bone.name`, `EditBone.name` and
//! `PoseBone.name`. The new name is made unique among the edit bones (in Edit Mode) or the
//! rest bones, then the pose channels of every object using the armature, the `parent_bone`
//! of objects bone-parented to it, and the vertex groups of meshes deformed by it (ARMATURE
//! modifier) follow; a vertex-group name collision keeps the vertex groups and reports
//! Blender's WARNING.
//!
//! Ported from blender/source/blender/editors/armature/armature_naming.cc
//! (`ED_armature_bone_rename`), Blender 5.2.4, GPL-2.0-or-later. Constraint subtargets,
//! animation paths and hook modifiers are not modelled by frap.

use std::sync::Arc;

use frap_rna::{ArmatureId, IdRef, MeshId, ObjectId, Report, ReportType};
use smol_str::SmolStr;

use crate::generated::modifier_settings::ModifierSettings;
use crate::{ChangeLog, EditState, Host, Main};

/// What a bone rename touches: `bpy.data`, the edit state and the report channel. Implemented
/// for every [`Host`] (the RNA setters) and by the operators' own adapter over their context
/// (`armature.flip_names`, `armature.autoside_names`, WP 8.3.10).
pub trait RenameHost {
    /// `bpy.data`.
    fn rename_main(&self) -> &Main;
    /// `bpy.data` and the change log.
    fn rename_main_mut(&mut self) -> (&mut Main, &mut ChangeLog);
    /// The edit state (edit bones of armatures in Edit Mode).
    fn rename_edit_mut(&mut self) -> &mut EditState;
    /// Adds a report (the vertex-group collision WARNING).
    fn rename_report(&mut self, r: Report);
}

impl RenameHost for dyn Host + '_ {
    fn rename_main(&self) -> &Main {
        self.main()
    }
    fn rename_main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
        self.main_mut()
    }
    fn rename_edit_mut(&mut self) -> &mut EditState {
        self.edit_mut()
    }
    fn rename_report(&mut self, r: Report) {
        self.report(r);
    }
}

/// An object a bone rename may touch: `(object, uses the armature, bone-parented to the
/// renamed bone, deformed by the armature, its mesh)`.
type Affected = (IdRef<ObjectId>, bool, bool, bool, Option<IdRef<MeshId>>);

/// Renames bone `old` of `arm` to (a unique version of) `new`; returns the name it got, or
/// `None` when nothing is named `old` (or the names are equal).
pub fn rename_bone(
    h: &mut dyn Host,
    arm: IdRef<ArmatureId>,
    old: &str,
    new: &str,
) -> Option<SmolStr> {
    rename_bone_in(h, arm, old, new)
}

/// [`rename_bone`] over any [`RenameHost`].
pub fn rename_bone_in<H: RenameHost + ?Sized>(
    h: &mut H,
    arm: IdRef<ArmatureId>,
    old: &str,
    new: &str,
) -> Option<SmolStr> {
    let new = frap_rna::naming::truncate_utf8(new, super::names::MAX_BONE_NAME);
    if old == new {
        return None;
    }
    let key = arm.key();
    let name: SmolStr = if let Some(ea) = h.rename_edit_mut().armatures.get_mut(&key) {
        let ea = Arc::make_mut(ea);
        let i = ea.find(old)?;
        let unique = ea.unique_name(new, None);
        ea.edit_bones[i].name = unique.clone();
        unique
    } else {
        let a = h.rename_main().armatures.get(arm)?;
        let i = a.bone_index(old)?;
        let unique = frap_rna::naming::unique_subname(
            &|n| a.bones.iter().any(|b| b.name == n),
            new,
            super::names::MAX_BONE_NAME,
            "Bone",
        );
        let (main, log) = h.rename_main_mut();
        main.armatures.get_mut(arm, log)?.bones[i].name = unique.clone();
        unique
    };
    let objects: Vec<Affected> = {
        let main = h.rename_main();
        main.objects
            .iter()
            .map(|(r, o)| {
                let uses = o.data == Some(arm.any());
                let parented = o
                    .parent
                    .and_then(|p| main.objects.get(p))
                    .is_some_and(|p| p.data == Some(arm.any()))
                    && o.parent_type == "BONE"
                    && o.parent_bone == old;
                let deformed = o.modifiers.iter().any(|m| match &m.settings {
                    ModifierSettings::Armature(s) => s
                        .object
                        .and_then(|a| a.typed::<ObjectId>())
                        .and_then(|a| main.objects.get(a))
                        .is_some_and(|a| a.data == Some(arm.any())),
                    _ => false,
                });
                let mesh = o.data.and_then(|d| d.typed::<MeshId>());
                (r, uses, parented, deformed, mesh)
            })
            .collect()
    };
    for (ob, uses, parented, deformed, mesh) in objects {
        if uses || parented {
            let (main, log) = h.rename_main_mut();
            if let Some(o) = main.objects.get_mut(ob, log) {
                if uses && let Some(pb) = o.pose.as_mut().and_then(|p| p.bone_mut(old)) {
                    pb.name = name.clone();
                }
                if parented {
                    o.parent_bone = name.clone();
                }
            }
        }
        if deformed && let Some(me) = mesh {
            let (collides, has_old, ob_name) = {
                let main = h.rename_main();
                let m = main.meshes.get(me);
                (
                    m.is_some_and(|m| m.vertex_group_names.contains(&name)),
                    m.is_some_and(|m| m.vertex_group_names.iter().any(|g| g == old)),
                    main.objects
                        .get(ob)
                        .map(|o| o.header.name.clone())
                        .unwrap_or_default(),
                )
            };
            if collides {
                h.rename_report(Report::new(
                    ReportType::Warning,
                    format!(
                        "New bone name collides with an existing vertex group name, vertex group names are unchanged. ({ob_name}::{name})"
                    ),
                ));
            } else if has_old {
                let (main, log) = h.rename_main_mut();
                if let Some(m) = main.meshes.get_mut(me, log) {
                    for g in &mut m.vertex_group_names {
                        if g == old {
                            *g = name.clone();
                        }
                    }
                }
            }
        }
    }
    Some(name)
}
