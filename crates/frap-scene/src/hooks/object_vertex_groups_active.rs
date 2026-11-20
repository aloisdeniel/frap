// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.vertex_groups.active` (`rna_Object_active_vertex_group_get/set`, WP 6.3): the
//! active vertex group, `None` without one; assigning a group of this object makes it active
//! (`VertexGroup 'X' not found in object 'Y'` for another object's group).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, Value, rna_hook};

use super::object_vertex_groups::{group_names, group_of, group_ptr};
use crate::id::paths::object_of;
use crate::{Host, HostRead};

/// `VertexGroups` is an API struct without an `rna_struct!` binding of its own.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "VertexGroups",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "VertexGroups", prop = "active", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let main = h.main();
        let ob = object_of(this)?;
        let names = group_names(main, ob);
        let act = main
            .objects
            .get(ob)
            .map_or(-1, |o| o.vertex_groups_active_index);
        Ok(Field::Value(
            match usize::try_from(act).ok().and_then(|i| names.get(i)) {
                Some(n) => Value::Ptr(Box::new(group_ptr(ob, n))),
                None => Value::None,
            },
        ))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let Value::Ptr(p) = v else {
            return Ok(());
        };
        let target = match group_of(h.main(), &p) {
            Ok((o, i)) if o == ob => Some(i),
            _ => None,
        };
        let Some(i) = target else {
            let group = match p.path.last() {
                Some(frap_rna::Seg::Key(k)) => k.to_string(),
                _ => String::new(),
            };
            let name = h.main().objects.name_of(ob).unwrap_or_default().to_owned();
            return Err(RnaError::new(
                frap_rna::RnaErrorKind::Other,
                frap_rna::PyExc::RuntimeError,
                format!("Error: VertexGroup '{group}' not found in object '{name}'\n"),
            ));
        };
        let (main, log) = h.main_mut();
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.vertex_groups_active_index = i as i32;
        }
        Ok(())
    }
}
