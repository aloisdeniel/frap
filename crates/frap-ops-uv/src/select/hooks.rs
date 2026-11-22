// SPDX-License-Identifier: GPL-3.0-or-later
//! The RNA updates of the UV selection tool settings: `ToolSettings.use_uv_select_sync`
//! (`rna_Scene_uv_select_sync_update` → `ED_uvedit_select_sync_multi`) and
//! `ToolSettings.uv_select_mode` (`rna_Scene_uv_select_mode_update` →
//! `ED_uvedit_selectmode_clean_multi`). `uv_sticky_select_mode`'s update only redraws.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_scene.cc and
//! blender/source/blender/editors/uvedit/uvedit_select.cc, Blender 5.2.4, GPL-2.0-or-later.
//! As in Blender the update runs on every assignment (also of the current value) and uses the
//! context scene's tool settings; it reaches every mesh in Edit Mode with a UV map.

use std::sync::Arc;

use frap_rna::{PropDef, RnaError, RnaPtr, Root, SceneId, Value, rna_hook};
use frap_scene::Host;

use super::{Ts, UvSel};

/// Writes a field of the tool settings `this` points at (the window's scene without an ID
/// root).
fn set_field(
    h: &mut dyn Host,
    this: &RnaPtr,
    f: impl FnOnce(&mut frap_scene::generated::plain::ToolSettings),
) -> Result<(), RnaError> {
    let scene = match this.root {
        Root::Id(a) => a.typed::<SceneId>(),
        _ => None,
    }
    .unwrap_or(h.win().scene);
    let (main, log) = h.main_mut();
    let s = main.scenes.get_mut(scene, log).ok_or_else(|| {
        RnaError::new(
            frap_rna::RnaErrorKind::Other,
            frap_rna::PyExc::RuntimeError,
            "frap internal: the tool settings have no scene",
        )
    })?;
    f(&mut s.tool_settings);
    Ok(())
}

/// Runs `f` on the UV selection of every mesh in Edit Mode with a UV map, with the context
/// scene's tool settings.
fn each_edit_mesh(h: &mut dyn Host, f: impl Fn(&mut UvSel)) {
    let ts = Ts::of(&h.ctx());
    let keys: Vec<_> = h.edit().meshes.keys().copied().collect();
    for k in keys {
        let has_uv = h
            .edit()
            .meshes
            .get(&k)
            .is_some_and(|em| super::active_uv(&em.bm).is_some());
        if !has_uv {
            continue;
        }
        let em = h.edit_mut().meshes.get_mut(&k).expect("listed above");
        em.bm_gen += 1;
        em.view = frap_scene::ViewCache::default();
        let bm = Arc::make_mut(&mut em.bm);
        if let Some(mut s) = UvSel::new(ts, bm) {
            f(&mut s);
        }
    }
}

/// The value as a bool / enum identifier.
fn as_bool(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        _ => None,
    }
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL_SYNC: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ToolSettings",
    prop: "use_uv_select_sync",
    coll: false,
};

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL_MODE: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ToolSettings",
    prop: "uv_select_mode",
    coll: false,
};

/// `use_uv_select_sync`: switching sync selection off rebuilds the UV selection from the
/// mesh selection when the mesh and UV select modes match (`uv_select_sync_update`).
#[rna_hook(srna = "ToolSettings", prop = "use_uv_select_sync")]
mod sync {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Some(b) = as_bool(&v) else {
            return Ok(());
        };
        set_field(h, this, |ts| ts.use_uv_select_sync = b)?;
        if !Ts::of(&h.ctx()).sync {
            each_edit_mesh(h, |s| s.sync_update());
        }
        Ok(())
    }
}

/// `uv_select_mode`: the UV selection is cleaned for the new mode
/// (`ED_uvedit_selectmode_clean`).
#[rna_hook(srna = "ToolSettings", prop = "uv_select_mode")]
mod mode {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let Value::Enum(id) = v else {
            return Ok(());
        };
        set_field(h, this, |ts| ts.uv_select_mode = id.clone())?;
        each_edit_mesh(h, |s| s.selectmode_clean());
        Ok(())
    }
}
