// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.active_shape_key_index` writes (`rna_Object_active_shape_key_index_set`, then
//! `rna_Object_active_shape_update`, WP 6.3): the index is stored as given (verified on 5.2.4:
//! `99` with two blocks reads back `99`, `active_shape_key` is then `None`; the range function
//! only limits the UI), a negative one as 0 (Blender stores it and reads `max(shapenr - 1, 0)`
//! = 0), and a mesh in Edit Mode is written back and re-entered so the edit BMesh shows the
//! new block (`EDBM_mesh_load` + `EDBM_mesh_make`). Reads stay with `Object`'s binding.

use std::sync::Arc;

use frap_bmesh::{BMesh, FromMeshParams, ToMeshParams};
use frap_rna::{IdRef, MeshId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::object_mesh;
use crate::id::paths::object_of;
use crate::{EditMesh, Host, ViewCache};

/// `Object`'s `rna_struct!` binding (WP 1.10) binds the field; the write goes through here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "Object",
    prop: "active_shape_key_index",
    coll: false,
};

/// `EDBM_mesh_load` + `EDBM_mesh_make` of `me` with the active block going from `old` to
/// `new` (both 0-based).
fn reload_edit_mesh(h: &mut dyn Host, me: IdRef<MeshId>, old: usize, new: usize) {
    let Some(em) = h.edit_mut().meshes.remove(&me.key()) else {
        return;
    };
    let mut bm: BMesh = Arc::unwrap_or_clone(em.bm);
    let select_mode = bm.select_mode;
    let (main, log) = h.main_mut();
    let Some(template) = main.meshes.get(me).cloned() else {
        return;
    };
    let mut loaded = bm.to_mesh(
        &template,
        &ToMeshParams {
            calc_object_remap: true,
            update_shapekey_indices: false,
        },
    );
    if let Some(k) = template.shape_keys
        && let Some(key) = main.shape_keys.get_mut(k, log)
        && let Some(pos) = key.write_back_from_bmesh(&mut bm, Some(old))
    {
        loaded.set_positions(pos);
    }
    if let Some(m) = main.meshes.get_mut(me, log) {
        let header = m.header.clone();
        *m = loaded;
        m.header = header;
    }
    let Some(mesh) = main.meshes.get(me) else {
        return;
    };
    let mut bm = BMesh::from_mesh(mesh, &FromMeshParams::default());
    if let Some(key) = mesh.shape_keys.and_then(|k| main.shape_keys.get(k)) {
        key.load_into_bmesh(&mut bm, Some(new));
    }
    bm.select_mode = select_mode;
    bm.select_flush_mode();
    h.edit_mut().meshes.insert(
        me.key(),
        EditMesh {
            bm: Arc::new(bm),
            bm_gen: frap_rna::id::next_generation(),
            view: ViewCache::default(),
        },
    );
}

#[rna_hook(srna = "Object", prop = "active_shape_key_index", krate = crate)]
fn set(h: &mut dyn Host, this: &RnaPtr, _prop: &'static PropDef, v: Value) -> Result<(), RnaError> {
    let ob = object_of(this)?;
    let Value::Int(i) = v else {
        return Ok(());
    };
    let main = h.main();
    let me = object_mesh(main, ob);
    let totkey = me
        .and_then(|me| main.meshes.get(me))
        .and_then(|m| m.shape_keys)
        .and_then(|k| main.shape_keys.get(k))
        .map_or(0, |k| k.key_blocks.len());
    let new = i.clamp(0, i64::from(i16::MAX)) as i32;
    let old = main.objects.get(ob).map_or(0, |o| o.active_shape_key_index);
    let in_edit = me.is_some_and(|m| h.edit().meshes.contains_key(&m.key()));
    {
        let (main, log) = h.main_mut();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.active_shape_key_index = new;
        }
    }
    if in_edit
        && totkey > 0
        && let Some(me) = me
    {
        reload_edit_mesh(h, me, old.max(0) as usize, new as usize);
    }
    Ok(())
}
