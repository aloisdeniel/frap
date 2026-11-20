// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.active_shape_key_index` writes (`rna_Object_active_shape_key_index_set`, then
//! `rna_Object_active_shape_update`, WP 6.3): `ob->shapenr = value + 1` is stored as given
//! (verified on 5.2.4: `99` with two blocks reads back `99`, `active_shape_key` is then `None`;
//! the range function only limits the UI). frap keeps `shapenr - 1` in
//! `Object::active_shape_key_index`, so `-1` (`shapenr` 0) survives: the property reads
//! `max(shapenr - 1, 0)` = 0 (`Object`'s binding) while `active_shape_key` is `None`. A mesh
//! in Edit Mode is written back and re-entered so the edit BMesh shows the new block
//! (`EDBM_mesh_load` + `EDBM_mesh_make`).

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

/// `EDBM_mesh_load` + `EDBM_mesh_make` of `me` for an object whose `shapenr` is now
/// `new_shapenr`: the old BMesh writes back the block it was made with (`bm->shapenr`, the
/// basis when the object's `shapenr` is 0), the new one loads block `new_shapenr - 1`.
fn reload_edit_mesh(h: &mut dyn Host, me: IdRef<MeshId>, new_shapenr: i32) {
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
        && !key.key_blocks.is_empty()
    {
        if new_shapenr == 0 {
            bm.shapenr = 1;
        }
        if let Some(pos) = key.write_back_from_bmesh(&mut bm) {
            loaded.set_positions(pos);
        }
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
    if let Some(key) = mesh
        .shape_keys
        .and_then(|k| main.shape_keys.get(k))
        .filter(|k| !k.key_blocks.is_empty())
    {
        let active = match new_shapenr {
            0 => Some(0),
            n => usize::try_from(n - 1).ok(),
        };
        key.load_into_bmesh(&mut bm, active);
        bm.normals_update();
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
    let new = i.clamp(i64::from(i16::MIN), i64::from(i16::MAX) - 1) as i32;
    let in_edit = me.is_some_and(|m| h.edit().meshes.contains_key(&m.key()));
    {
        let (main, log) = h.main_mut();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.active_shape_key_index = new;
        }
    }
    if in_edit && let Some(me) = me {
        reload_edit_mesh(h, me, new + 1);
    }
    Ok(())
}
