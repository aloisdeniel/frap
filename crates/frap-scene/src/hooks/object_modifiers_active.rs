// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.modifiers.active` (`rna_Object_active_modifier_get/set`, Blender 5.2.4, verified):
//! reads the active modifier (`None` when there is none); assigning one of the object's
//! modifiers makes it the only active one, `None` clears the flag, and a modifier of another
//! object raises `RuntimeError: Error: Modifier "X" is not in the object's modifier list`.

use frap_rna::{Field, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook};

use crate::id::Modifier;
use crate::id::paths::{internal, object_of};
use crate::{Host, HostRead};

/// `ObjectModifiers` is the API struct of `Object.modifiers` (no binding of its own), so its
/// hook binding is declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ObjectModifiers",
    prop: "active",
    coll: false,
};

#[rna_hook(srna = "ObjectModifiers", prop = "active", krate = crate)]
mod hook {
    use super::*;

    pub fn get<'h>(
        h: &'h dyn HostRead,
        this: &RnaPtr,
        _prop: &'static PropDef,
    ) -> Result<Field<'h>, RnaError> {
        let ob = object_of(this)?;
        let o = h
            .main()
            .objects
            .get(ob)
            .ok_or_else(|| internal("modifiers.active: the object was removed"))?;
        Ok(match o.modifiers.iter().find(|m| m.is_active) {
            Some(m) => Field::Ptr(m.ptr(ob.any())),
            None => Field::Value(Value::None),
        })
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let (main, log) = h.main_mut();
        let o = main
            .objects
            .get(ob)
            .ok_or_else(|| internal("modifiers.active: the object was removed"))?;
        let target = match &v {
            Value::Ptr(p) => {
                let same_object = object_of(p).ok() == Some(ob);
                let i = same_object
                    .then(|| Modifier::index_in(&o.modifiers, &p.path))
                    .flatten();
                if i.is_none() {
                    let name = p
                        .path
                        .iter()
                        .rev()
                        .find_map(|s| match s {
                            frap_rna::Seg::Key(k) => Some(k.to_string()),
                            _ => None,
                        })
                        .unwrap_or_default();
                    return Err(RnaError::new(
                        RnaErrorKind::Other,
                        PyExc::RuntimeError,
                        format!(
                            "Error: Modifier \"{name}\" is not in the object's modifier list\n"
                        ),
                    ));
                }
                i
            }
            _ => None,
        };
        if let Some(o) = main.objects.get_mut(ob, log) {
            for (j, m) in o.modifiers.iter_mut().enumerate() {
                m.is_active = Some(j) == target;
            }
        }
        Ok(())
    }
}
