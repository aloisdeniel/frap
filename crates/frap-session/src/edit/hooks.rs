// SPDX-License-Identifier: GPL-3.0-or-later
//! RNA hooks that reach the edit BMesh (WP 2.2):
//! - `ToolSettings.mesh_select_mode` (`rna_Scene_editmesh_select_mode_set`,
//!   `makesrna/intern/rna_scene.cc`, Blender 5.2.4): an empty mode is ignored; otherwise
//!   `ts->selectmode` is stored and the edit mesh of the window's **active** object (when it is
//!   a mesh in Edit Mode with another mode) switches with `EDBM_selectmode_set`: the history of
//!   types the mode cannot select is stripped and the selection re-derived (switching from
//!   three selected vertices to face mode leaves nothing selected). Other objects in Edit Mode
//!   keep their mode, as in Blender.
//! - `Mesh.total_vert_sel` / `total_edge_sel` / `total_face_sel` (`rna_Mesh_tot_*_get`): the
//!   edit BMesh's selection counts while the mesh is in Edit Mode, 0 otherwise.

use std::sync::Arc;

use frap_bmesh::SelectMode;
use frap_rna::{Field, IdRef, MeshId, PropDef, RnaError, RnaPtr, Root, SceneId, Value, rna_hook};
use frap_scene::{Host, HostRead, ViewCache};

/// The `=> hook` declarations of bound properties (`HOOK_DECLS`, F-TR-3).
macro_rules! decl {
    ($name:ident, $srna:literal, $prop:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
            srna: $srna,
            prop: $prop,
            coll: false,
        };
    };
}

decl!(DECL_SELECT_MODE, "ToolSettings", "mesh_select_mode");
decl!(DECL_TOT_VERT, "Mesh", "total_vert_sel");
decl!(DECL_TOT_EDGE, "Mesh", "total_edge_sel");
decl!(DECL_TOT_FACE, "Mesh", "total_face_sel");

/// `SCE_SELECT_*` bits of a `mesh_select_mode` value.
fn select_mode(bits: [bool; 3]) -> SelectMode {
    let mut m = SelectMode::empty();
    for (on, b) in bits
        .iter()
        .zip([SelectMode::VERT, SelectMode::EDGE, SelectMode::FACE])
    {
        if *on {
            m |= b;
        }
    }
    m
}

#[rna_hook(srna = "ToolSettings", prop = "mesh_select_mode")]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let Value::BoolArray(a) = v else {
        return Ok(());
    };
    let bits = [0, 1, 2].map(|i| a.get(i).copied().unwrap_or(false));
    let mode = select_mode(bits);
    if mode.is_empty() {
        // `if (selectmode)`: Blender ignores an empty mode.
        return Ok(());
    }
    let scene = match this.root {
        Root::Id(id) => id.typed::<SceneId>(),
        _ => None,
    }
    .unwrap_or(h.win().scene);
    {
        let (main, log) = h.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.tool_settings.mesh_select_mode = bits;
        }
        log.context_changed = true;
    }
    // The active object of the window's view layer.
    let me = {
        let c = h.ctx();
        c.active_object()
            .and_then(|ob| c.main.objects.get(ob))
            .and_then(|o| o.data)
            .and_then(|d| d.typed::<MeshId>())
    };
    let Some(me) = me else {
        return Ok(());
    };
    let changed = match h.edit().meshes.get(&me.key()) {
        Some(em) => em.bm.select_mode != mode,
        None => false,
    };
    if !changed {
        return Ok(());
    }
    h.main_mut().1.modified.insert(me.any());
    if let Some(em) = h.edit_mut().meshes.get_mut(&me.key()) {
        em.bm_gen += 1;
        em.view = ViewCache::default();
        Arc::make_mut(&mut em.bm).selectmode_set(mode);
    }
    Ok(())
}

/// The edit BMesh's selection counts of the mesh at `this`, `None` outside Edit Mode.
fn counts(h: &dyn HostRead, this: &RnaPtr) -> Option<(usize, usize, usize)> {
    let Root::Id(id) = this.root else {
        return None;
    };
    let me: IdRef<MeshId> = id.typed::<MeshId>()?;
    h.edit()
        .meshes
        .get(&me.key())
        .map(|em| em.bm.selected_counts())
}

fn count<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    pick: fn((usize, usize, usize)) -> usize,
) -> Field<'h> {
    let n = counts(h, this).map_or(0, pick);
    Field::Value(Value::Int(i64::try_from(n).unwrap_or(i64::MAX)))
}

#[rna_hook(srna = "Mesh", prop = "total_vert_sel")]
fn total_vert_sel<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    Ok(count(h, this, |c| c.0))
}

#[rna_hook(srna = "Mesh", prop = "total_edge_sel")]
fn total_edge_sel<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    Ok(count(h, this, |c| c.1))
}

#[rna_hook(srna = "Mesh", prop = "total_face_sel")]
fn total_face_sel<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    Ok(count(h, this, |c| c.2))
}
