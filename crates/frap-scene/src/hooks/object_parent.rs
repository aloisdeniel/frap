// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.parent` assignment (`rna_Object_parent_set` → `ED_object_parent`, verified on
//! 5.2.4): `matrix_parent_inverse` is **always reset to identity** (an RNA assignment makes the
//! child "jump"; `object.parent_set` is the operator that computes the inverse). `None`, or a
//! parent that would close a cycle (`BKE_object_parent_loop_check`), clears the parent and
//! resets `parent_type` to `OBJECT` and `parent_bone`, silently; otherwise `parent_type` is
//! kept. The object itself raises `TypeError: Object.parent ID type does not support assignment
//! to itself` (`PROP_ID_SELF_CHECK`, before the setter runs: nothing changes).

use frap_rna::{
    Field, IdRef, ObjectId, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook,
};

use crate::id::paths::{internal, object_of};
use crate::id::transform::would_cycle;
use crate::{Host, HostRead};

#[rna_hook(srna = "Object", prop = "parent", krate = crate)]
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
            .ok_or_else(|| internal("Object.parent: the object was removed"))?;
        Ok(Field::Id(o.parent.map(IdRef::any)))
    }

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let parent: Option<IdRef<ObjectId>> = match v {
            Value::Id(Some(a)) => IdRef::from_any(a),
            _ => None,
        };
        if parent == Some(ob) {
            // `PROP_ID_SELF_CHECK` (verified: `TypeError: … Object.parent ID type does not
            // support assignment to itself`).
            return Err(RnaError::new(
                RnaErrorKind::Type,
                PyExc::TypeError,
                "Object.parent ID type does not support assignment to itself",
            ));
        }
        let (main, log) = h.main_mut();
        let parent = parent.filter(|p| !would_cycle(&main.objects, ob, *p));
        if let Some(o) = main.objects.get_mut(ob, log) {
            o.matrix_parent_inverse = glam::Mat4::IDENTITY;
            o.parent = parent;
            if parent.is_none() {
                o.parent_type = "OBJECT".into();
                o.parent_bone = Default::default();
            }
        }
        Ok(())
    }
}
