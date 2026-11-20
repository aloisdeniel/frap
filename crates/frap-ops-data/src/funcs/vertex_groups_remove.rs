// SPDX-License-Identifier: GPL-3.0-or-later
//! `obj.vertex_groups.remove(group)` (`rna_Object_vgroup_remove` →
//! `BKE_object_defgroup_remove`, WP 6.3): the group's weights go (in the edit BMesh in Edit
//! Mode), higher group indices shift down, the active index follows. A group of another object
//! raises `RuntimeError: Error: DeformGroup 'X' not in object 'Y'`.

use frap_rna::{RnaError, RnaPtr, Seg, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_ptr, internal, report_error};
use super::vertex_groups_new::{group_of, object_of, remove_group, supported};

#[rna_func(srna = "VertexGroups", func = "remove")]
fn vertex_groups_remove(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "remove")?;
    let ob = object_of(this)?;
    let me = supported(h.main(), ob, "VertexGroups.remove()")?;
    let p = arg_ptr(args, "group").ok_or_else(|| internal("remove(): no group argument"))?;
    match group_of(h.main(), p) {
        Some((o, g)) if o == ob => {
            remove_group(h, ob, me, g)?;
            Ok(Value::None)
        }
        _ => {
            let group = match p.path.last() {
                Some(Seg::Key(k)) => k.to_string(),
                _ => String::new(),
            };
            let name = h.main().objects.name_of(ob).unwrap_or_default().to_owned();
            Err(report_error(format!(
                "DeformGroup '{group}' not in object '{name}'"
            )))
        }
    }
}
