// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.vertex_groups.active_index` (`rna_Object_active_vertex_group_index_get/set`,
//! WP 6.3): the 0-based index of the active vertex group, `-1` without groups. frap keeps it
//! on the object (`Object::vertex_groups_active_index`, Blender's `actdef - 1`; Blender 5.x
//! stores `vertex_group_active_index` on the mesh, so linked duplicates share it there). The
//! setter stores what it is given within the dumped hard range, as Blender stores it
//! unchecked (verified on 5.2.4: `5` with three groups reads back `5`).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::group_names;
use crate::id::paths::object_of;
use crate::{Host, HostRead};

/// `VertexGroups` is an API struct without an `rna_struct!` binding of its own.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "VertexGroups",
    prop: "active_index",
    coll: false,
};

#[rna_hook(srna = "VertexGroups", prop = "active_index", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let main = h.main();
        let ob = object_of(this)?;
        let act = if group_names(main, ob).is_empty() {
            -1
        } else {
            main.objects
                .get(ob)
                .map_or(-1, |o| o.vertex_groups_active_index)
        };
        Ok(Field::Value(Value::Int(i64::from(act))))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let Value::Int(i) = v else {
            return Ok(());
        };
        let (main, log) = h.main_mut();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.vertex_groups_active_index =
                i.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        }
        Ok(())
    }
}
