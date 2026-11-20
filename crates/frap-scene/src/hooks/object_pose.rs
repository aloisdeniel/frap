// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.pose` (WP 6.2): the pose channels of an armature object, `None` for other objects
//! and for an armature object whose pose was never built (Blender builds it when the object
//! is evaluated or leaves Edit Mode; `object.armature_add` and `mode_set` do in frap).

use frap_rna::{Field, PropDef, RnaError, RnaPtr, StructRef, Value, rna_hook};

use crate::HostRead;
use crate::id::paths::{internal, object_of};

/// `Object.pose` is not in the `Object` binding (owned by WP 1.10), so its hook binding is
/// declared here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
#[linkme(crate = frap_rna::linkme)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "Object",
    prop: "pose",
    coll: false,
};

#[rna_hook(srna = "Object", prop = "pose", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let ob = object_of(this)?;
    let o = h
        .main()
        .objects
        .get(ob)
        .ok_or_else(|| internal("Object.pose: the object was removed"))?;
    Ok(match (&o.pose, o.object_type()) {
        (Some(p), "ARMATURE") => Field::Struct(StructRef::Borrowed(p)),
        _ => Field::Value(Value::None),
    })
}
