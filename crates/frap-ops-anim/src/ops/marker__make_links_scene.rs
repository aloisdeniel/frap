// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.marker.make_links_scene(scene)`, tier T0 (WP 7.6). Port of
//! `ed_marker_make_links_scene_exec` (blender/source/blender/editors/animation/
//! anim_markers.cc), Blender 5.2.4, GPL-2.0-or-later: appends a copy of every selected marker
//! (name, frame, selection, camera) to the markers of `scene` (one of `bpy.data.scenes`,
//! `RNA_scene_itemf`). The context scene itself: `Cannot re-link markers into the same scene`;
//! a target with locked markers: `Target scene has locked markers` (both `RuntimeError`,
//! verified on 5.2.4). The target's camera does not switch. Poll
//! `ed_markers_poll_selected_markers` (`No markers are selected`; locked markers of the context
//! scene are allowed).

use frap_op::EnumItemOwned;
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::Context;
use frap_spec_ops_anim::marker;
use smol_str::SmolStr;

use super::marker__add::common;

/// `RNA_scene_itemf`: every scene of `bpy.data.scenes`, by name.
fn scene_items(c: &Context) -> Vec<EnumItemOwned> {
    c.main
        .scenes
        .iter()
        .map(|(_, s)| EnumItemOwned {
            identifier: s.header.name.clone(),
            name: s.header.name.clone(),
            description: SmolStr::default(),
        })
        .collect()
}

#[operator(
    poll = common::SELECTED_MAKE_LINKS,
    tier = T0,
    support = HeadlessExtended(common::CONTEXT_NOTE),
    dyn_enums = [("scene", scene_items)]
)]
fn exec(cx: &mut OpCtx, p: marker::MakeLinksScene) -> OpResult {
    let (from, to, copies) = {
        let c = cx.ctx();
        let to = c
            .main
            .scenes
            .iter()
            .find(|(_, s)| s.header.name == p.scene.value)
            .map(|(r, s)| (r, s.tool_settings.lock_markers));
        let copies: Vec<_> = common::markers(&c)
            .iter()
            .filter(|m| m.select)
            .cloned()
            .collect();
        (c.win.scene, to, copies)
    };
    let Some((to, locked)) = to else {
        return Err(OpError::Runtime("Scene not found".into()));
    };
    if to == from {
        return Err(OpError::Runtime(
            "Cannot re-link markers into the same scene".into(),
        ));
    }
    if locked {
        return Err(OpError::Runtime("Target scene has locked markers".into()));
    }
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(to, log) {
        s.timeline_markers.extend(copies);
    }
    Ok(OpStatus::Finished)
}
