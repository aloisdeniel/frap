// SPDX-License-Identifier: GPL-3.0-or-later
//! `ED_armature_bone_rename` (WP 6.2): the setter of `Bone.name`, `EditBone.name` and
//! `PoseBone.name`. The new name is made unique among the edit bones (in Edit Mode) or the
//! rest bones, then the pose channels of every object using the armature, the `parent_bone`
//! of objects bone-parented to it, and the vertex groups of meshes deformed by it (ARMATURE
//! modifier) follow; a vertex-group name collision keeps the vertex groups and reports
//! Blender's WARNING. Last, the animation paths that named the bone follow
//! (`BKE_animdata_fix_paths_rename_all(&arm->id, "bones", old, new)`, see [`fix_anim_paths`]):
//! `pose.bones["Old"].location` becomes `pose.bones["New"].location` in the action (and the
//! NLA strip actions) assigned to every armature and object, and an F-curve group named after
//! the old bone takes the new name.
//!
//! Ported from blender/source/blender/editors/armature/armature_naming.cc
//! (`ED_armature_bone_rename`) and blender/source/blender/blenkernel/intern/anim_data.cc
//! (`BKE_animdata_fix_paths_rename`, `rna_path_rename_fix`, `fcurves_path_rename_fix`),
//! Blender 5.2.4, GPL-2.0-or-later. Constraint subtargets, hook modifiers and drivers are not
//! modelled by frap.

use std::sync::Arc;

use frap_anim::ActionStrip;
use frap_anim::eval::props::{PathSeg, parse_path, resolve, root_struct};
use frap_rna::ptr::rna_key_repr;
use frap_rna::{ActionId, ArmatureId, IdRef, MeshId, ObjectId, Report, ReportType};
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
    fix_anim_paths(h, arm, old, &name);
    Some(name)
}

/// An action and the slot handle an ID animates it with.
type ActionSlotRef = (IdRef<ActionId>, i32);

/// The ID an animation path is resolved from (`owner_id`).
#[derive(Copy, Clone)]
enum Owner {
    Armature(IdRef<ArmatureId>),
    Object(IdRef<ObjectId>),
}

/// `BKE_animdata_fix_paths_rename_all(&arm->id, "bones", old, new)` with `verify_paths`: for
/// every ID with animation data (`BKE_animdata_main_cb` order: armatures, then objects; the
/// other ID types cannot resolve a `bones` path), the F-curves of its assigned action slot
/// and of its NLA strips' action slots whose path has `bones["old"]` (the first `bones`
/// followed by the first `["old"]`) are rewritten when the old path no longer resolves on the
/// ID and the new one does; the F-curve's group, when named `old`, is renamed too.
///
/// Ported from blender/source/blender/blenkernel/intern/anim_data.cc and
/// anim_data_bmain_utils.cc, Blender 5.2.4, GPL-2.0-or-later.
pub fn fix_anim_paths<H: RenameHost + ?Sized>(
    h: &mut H,
    arm: IdRef<ArmatureId>,
    old: &str,
    new: &str,
) {
    const PREFIX: &str = "bones";
    let (old_key, new_key) = (rna_key_repr(old), rna_key_repr(new));
    let edit_names: Option<Vec<SmolStr>> = h
        .rename_edit_mut()
        .armatures
        .get(&arm.key())
        .map(|ea| ea.edit_bones.iter().map(|b| b.name.clone()).collect());
    // `(owner, [(action, slot handle)])`: the active action, then the NLA strips.
    let owners: Vec<(Owner, Vec<ActionSlotRef>)> = {
        let main = h.rename_main();
        let slots = |ad: Option<&frap_anim::AnimData>| -> Vec<ActionSlotRef> {
            let Some(ad) = ad else {
                return Vec::new();
            };
            let mut v = Vec::new();
            if let Some(a) = ad.action
                && ad.action_slot_handle != 0
            {
                v.push((a, ad.action_slot_handle));
            }
            for t in &ad.nla_tracks {
                for s in &t.strips {
                    if let Some(a) = s.action {
                        v.push((a, s.action_slot_handle));
                    }
                }
            }
            v
        };
        let arms = main
            .armatures
            .iter()
            .map(|(r, a)| (Owner::Armature(r), slots(a.animation_data.as_ref())));
        let obs = main
            .objects
            .iter()
            .map(|(r, o)| (Owner::Object(r), slots(o.animation_data.as_ref())));
        arms.chain(obs).filter(|(_, v)| !v.is_empty()).collect()
    };
    for (owner, actions) in owners {
        for (act, handle) in actions {
            // The F-curves to rename, `(layer, strip, channelbag, fcurve, new path)`.
            let edits: Vec<(usize, usize, usize, usize, String)> = {
                let main = h.rename_main();
                let Some(a) = main.actions.get(act) else {
                    continue;
                };
                let mut edits = Vec::new();
                for (li, l) in a.layers.iter().enumerate() {
                    for (si, s) in l.strips.iter().enumerate() {
                        let ActionStrip::Keyframe(ks) = s else {
                            continue;
                        };
                        for (ci, cb) in ks.channelbags.iter().enumerate() {
                            if cb.slot_handle != handle {
                                continue;
                            }
                            for (fi, f) in cb.fcurves.iter().enumerate() {
                                let Some(p) =
                                    rename_path_fix(&f.data_path, PREFIX, &old_key, &new_key)
                                else {
                                    continue;
                                };
                                let resolves = |path: &str| {
                                    path_resolves(main, owner, edit_names.as_deref(), path)
                                };
                                if !resolves(&f.data_path) && resolves(&p) {
                                    edits.push((li, si, ci, fi, p));
                                }
                            }
                        }
                    }
                }
                edits
            };
            if edits.is_empty() {
                continue;
            }
            let (main, log) = h.rename_main_mut();
            let Some(a) = main.actions.get_mut(act, log) else {
                continue;
            };
            for (li, si, ci, fi, p) in edits {
                let ActionStrip::Keyframe(ks) = &mut a.layers[li].strips[si] else {
                    continue;
                };
                let cb = &mut ks.channelbags[ci];
                cb.fcurves[fi].data_path = p.into();
                // `fcu->grp` named after the old bone takes the new name (frap's F-curves
                // name their group, so its other members follow).
                if cb.fcurves[fi].group.as_deref() == Some(old) {
                    if let Some(g) = cb.groups.iter_mut().find(|g| g.name == old) {
                        g.name = new.into();
                    }
                    for f in &mut cb.fcurves {
                        if f.group.as_deref() == Some(old) {
                            f.group = Some(new.into());
                        }
                    }
                }
            }
        }
    }
}

/// `rna_path_rename_fix` without the verification: when the first `prefix` in `path` is
/// immediately followed by the first `old_key`, the path with `new_key` in its place.
fn rename_path_fix(path: &str, prefix: &str, old_key: &str, new_key: &str) -> Option<String> {
    let p = path.find(prefix)?;
    let o = path.find(old_key)?;
    (p + prefix.len() == o)
        .then(|| format!("{}{}{}", &path[..o], new_key, &path[o + old_key.len()..]))
}

/// `check_rna_path_is_valid(owner_id, path)` for the paths a bone rename can touch: the path
/// must follow the RNA definitions from the owner's struct (`data.` continuing from the
/// object's armature) and the bone it names (`pose.bones["…"]` on the object's pose,
/// `bones["…"]` on the armature's rest bones, `edit_bones["…"]` on its edit bones) must
/// exist. Other collection keys on the way are not checked.
fn path_resolves(main: &Main, owner: Owner, edit: Option<&[SmolStr]>, path: &str) -> bool {
    let arm_path = |a: IdRef<ArmatureId>, path: &str| -> bool {
        let Some(arm) = main.armatures.get(a) else {
            return false;
        };
        if root_struct("ARMATURE")
            .and_then(|r| resolve(r, path))
            .is_none()
        {
            return false;
        }
        match parse_path(path).as_deref() {
            Some([PathSeg::Prop(c), PathSeg::Key(k), ..]) if c == "bones" => {
                arm.bones.iter().any(|b| b.name == *k)
            }
            Some([PathSeg::Prop(c), PathSeg::Key(k), ..]) if c == "edit_bones" => {
                edit.is_some_and(|e| e.iter().any(|n| n == k))
            }
            _ => true,
        }
    };
    match owner {
        Owner::Armature(a) => arm_path(a, path),
        Owner::Object(o) => {
            let Some(ob) = main.objects.get(o) else {
                return false;
            };
            if let Some(rest) = path.strip_prefix("data.") {
                return ob
                    .data
                    .and_then(|d| d.typed::<ArmatureId>())
                    .is_some_and(|a| arm_path(a, rest));
            }
            if root_struct("OBJECT")
                .and_then(|r| resolve(r, path))
                .is_none()
            {
                return false;
            }
            match parse_path(path).as_deref() {
                Some([PathSeg::Prop(p), PathSeg::Prop(c), PathSeg::Key(k), ..])
                    if p == "pose" && c == "bones" =>
                {
                    ob.pose.as_ref().is_some_and(|p| p.bone(k).is_some())
                }
                _ => true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::rename_path_fix;

    #[test]
    fn rename_path_fix_matches_first_prefix_and_key() {
        let (o, n) = (r#"["A"]"#, r#"["B"]"#);
        assert_eq!(
            rename_path_fix(r#"pose.bones["A"].location"#, "bones", o, n).as_deref(),
            Some(r#"pose.bones["B"].location"#)
        );
        assert_eq!(
            rename_path_fix(r#"pose.bones["AB"].location"#, "bones", o, n),
            None
        );
        // `strstr` finds the first `["A"]`, which is not right after `bones`.
        assert_eq!(
            rename_path_fix(
                r#"pose.bones["X"].constraints["A"].influence"#,
                "bones",
                o,
                n
            ),
            None
        );
        assert_eq!(
            rename_path_fix(r#"data.bones["A"].bbone_x"#, "bones", o, n).as_deref(),
            Some(r#"data.bones["B"].bbone_x"#)
        );
    }
}
