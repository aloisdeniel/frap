// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.active_shape_key` (`rna_Object_active_shape_key_get`, WP 6.3): the block at
//! `active_shape_key_index` of the object's key, `None` without a key. Named after the
//! `object_shape_key*` hook prefix WP 6.3 owns.

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::object_mesh;
use crate::HostRead;
use crate::id::Key;
use crate::id::paths::object_of;

/// The `rna_struct!` binding of `Object` (WP 1.10) lists only the M1 members.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "Object",
    prop: "active_shape_key",
    coll: false,
};

#[rna_hook(srna = "Object", prop = "active_shape_key", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let ob = object_of(this)?;
    let key = object_mesh(main, ob)
        .and_then(|me| main.meshes.get(me))
        .and_then(|m| m.shape_keys);
    let act = main.objects.get(ob).map_or(0, |o| o.active_shape_key_index);
    let v = key
        .and_then(|k| Some((k, main.shape_keys.get(k)?)))
        .and_then(|(k, key)| {
            let kb = key.key_blocks.get(usize::try_from(act).ok()?)?;
            Some(Value::Ptr(Box::new(Key::block_ptr(k, &kb.name))))
        })
        .unwrap_or(Value::None);
    Ok(Field::Value(v))
}
