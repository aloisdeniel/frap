// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.armature.flip_names(do_strip_numbers=False)`, tier T0 (WP 8.3.10). Port of
//! `armature_flip_names_exec` and `ED_armature_bones_flip_names`
//! (blender/source/blender/editors/armature/armature_naming.cc), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! On every armature in Edit Mode, the selected visible bones (and, with
//! `Armature.use_mirror_x`, their unselected X mirrors) are renamed to their flipped side name
//! (`BLI_string_flip_side_name`: `.L`↔`.R`, `L_`↔`R_`, `Left`↔`Right`, …; `do_strip_numbers`
//! drops a trailing `.NNN`). Renames go through `ED_armature_bone_rename` (pose channels,
//! bone-parented objects and vertex groups follow); a name taken by another bone is first
//! made unique and retried once all bones were renamed, so `arm.L`/`arm.R` pairs swap. Always
//! `FINISHED`.
//!
//! [`rename_cx`] adapts the operator context to `frap_scene::edit_armature::rename`.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::edit_armature::names::flip_side_name;
use frap_spec_ops_anim::armature;

use super::armature__select_all::common;

/// The operator context as a `RenameHost` (shared with `armature.autoside_names`).
pub(crate) mod rename_cx {
    use frap_op::prelude::*;
    use frap_rna::{ArmatureId, IdRef, Report};
    use frap_scene::edit_armature::rename::{RenameHost, rename_bone_in};
    use frap_scene::{ChangeLog, EditState, Main};
    use smol_str::SmolStr;

    struct Cx<'a, 'h>(&'a mut OpCtx<'h>);

    impl RenameHost for Cx<'_, '_> {
        fn rename_main(&self) -> &Main {
            self.0.main()
        }
        fn rename_main_mut(&mut self) -> (&mut Main, &mut ChangeLog) {
            self.0.main_mut()
        }
        fn rename_edit_mut(&mut self) -> &mut EditState {
            self.0.split_edit().1
        }
        fn rename_report(&mut self, r: Report) {
            self.0.report(r.level, r.message);
        }
    }

    /// `ED_armature_bone_rename(bmain, arm, old, new)`: the name the bone got.
    pub(crate) fn rename(
        cx: &mut OpCtx,
        arm: IdRef<ArmatureId>,
        old: &str,
        new: &str,
    ) -> Option<SmolStr> {
        let r = rename_bone_in(&mut Cx(cx), arm, old, new);
        if r.is_some() {
            cx.main_mut().1.modified.insert(arm.any());
        }
        r
    }
}

#[operator(
    poll = poll::EDITARMATURE,
    tier = T0,
    spec = armature::OPSPEC_ARMATURE_OT_FLIP_NAMES
)]
fn exec(cx: &mut OpCtx, p: armature::ArmatureFlipNames) -> OpResult {
    let strip = p.do_strip_numbers.value;
    for (_, arm) in common::edit_armatures(cx) {
        let Some((a, ea)) = common::snapshot(cx, arm) else {
            continue;
        };
        // The bones to flip, by index (Blender keeps pointers to their name buffers).
        let mut bones: Vec<usize> = Vec::new();
        for i in 0..ea.edit_bones.len() {
            if !ea.is_selected(&a, i) {
                continue;
            }
            bones.push(i);
            if a.use_mirror_x
                && let Some(f) = ea.mirrored(i)
                && !ea.edit_bones[f].select[frap_scene::edit_armature::SEL]
            {
                bones.push(f);
            }
        }
        if bones.is_empty() {
            continue;
        }
        let name_of = |cx: &mut OpCtx, i: usize| -> Option<String> {
            let (_, edit) = cx.split_edit();
            edit.armatures
                .get(&arm.key())
                .and_then(|ea| ea.edit_bones.get(i))
                .map(|b| b.name.to_string())
        };
        // First pass: rename blindly; keep those that did not get their flipped name.
        let mut conflicts: Vec<(usize, String)> = Vec::new();
        for &i in &bones {
            let Some(name) = name_of(cx, i) else {
                continue;
            };
            let flip = flip_side_name(&name, strip);
            rename_cx::rename(cx, arm, &name, &flip);
            if name_of(cx, i).as_deref() != Some(flip.as_str()) {
                conflicts.push((i, flip));
            }
        }
        // Second pass: the other bone of a swapped pair has moved away by now.
        for (i, flip) in conflicts {
            if let Some(name) = name_of(cx, i) {
                rename_cx::rename(cx, arm, &name, &flip);
            }
        }
    }
    Ok(OpStatus::Finished)
}
