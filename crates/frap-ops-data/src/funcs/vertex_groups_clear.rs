// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.vertex_groups.clear()` (`rna_Object_vgroup_clear` → `BKE_object_defgroup_remove_all`,
//! WP 6.3): every group is removed in order (Edit Mode: from the edit BMesh); afterwards
//! `active_index` reads `-1`. frap keeps an all-empty weight column on the mesh where Blender
//! frees `CD_MDEFORMVERT` (no observable difference: no vertex has a weight).

use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::vertex_groups_new::{object_of, remove_group, supported};

#[rna_func(srna = "VertexGroups", func = "clear")]
fn vertex_groups_clear(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "clear")?;
    let ob = object_of(this)?;
    let me = supported(h.main(), ob, "VertexGroups.clear()")?;
    while h
        .main()
        .meshes
        .get(me)
        .is_some_and(|m| !m.vertex_group_names.is_empty())
    {
        remove_group(h, ob, me, 0)?;
    }
    let (main, log) = h.main_mut();
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.vertex_groups_active_index = -1;
    }
    Ok(Value::None)
}
