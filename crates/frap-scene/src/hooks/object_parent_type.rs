// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.parent_type` and `Object.parent_bone` assignment (WP 6.5; `rna_Object_parent_type_set`,
//! `rna_Object_parent_type_itemf`, `rna_Object_parent_bone_set` → `ed::object::parent_set`,
//! `editors/object/object_relations.cc`, Blender 5.2.4, GPL-2.0-or-later; verified on 5.2.4):
//!
//! - the items of `parent_type` depend on the parent: `OBJECT` always, `ARMATURE` and `BONE`
//!   for an armature parent, `LATTICE` for a lattice, `VERTEX` and `VERTEX_3` for a mesh;
//!   another value is `TypeError: bpy_struct: item.attr = val: enum "BONE" not found in
//!   ('OBJECT')`;
//! - assigning the current type changes nothing; another type resets `matrix_parent_inverse`
//!   to identity ("Always clear parentinv matrix for sake of consistency");
//! - `parent_bone` resets `matrix_parent_inverse` too; without a parent (or with a parent that
//!   would close a loop) both clear the parent data: no parent, `OBJECT`, `parent_bone` empty.

use frap_rna::{IdCode, IdRef, ObjectId, PropDef, PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_hook};
use smol_str::SmolStr;

use crate::Host;
use crate::id::paths::object_of;
use crate::id::transform::would_cycle;

/// `rna_Object_parent_type_itemf`: the parent types an object whose parent has data of type
/// `parent_code` can take (`None`: no parent). frap has no lattice or curve objects (Blender
/// adds `LATTICE` for a lattice, and the vertex types for curves, surfaces and lattices).
fn items(parent_code: Option<Option<IdCode>>) -> Vec<&'static str> {
    let mut v = vec!["OBJECT"];
    match parent_code {
        Some(Some(IdCode::AR)) => v.extend(["ARMATURE", "BONE"]),
        // `OB_TYPE_SUPPORT_PARVERT`
        Some(Some(IdCode::ME)) => v.extend(["VERTEX", "VERTEX_3"]),
        _ => {}
    }
    v
}

/// `ed::object::parent_set(ob, ob->parent, partype, substr)`.
fn parent_set(h: &mut dyn Host, ob: IdRef<ObjectId>, partype: Option<&str>, substr: Option<&str>) {
    let (main, log) = h.main_mut();
    let Some(o) = main.objects.get(ob) else {
        return;
    };
    let parent = o.parent;
    let valid = parent.is_some_and(|p| {
        main.objects.get(p).is_some() && !would_cycle(&main.objects, ob, p)
    });
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.matrix_parent_inverse = glam::Mat4::IDENTITY;
        if !valid {
            // `parent_clear_data`
            o.parent = None;
            o.parent_type = "OBJECT".into();
            o.parent_bone = SmolStr::default();
            return;
        }
        if let Some(t) = partype {
            o.parent_type = t.into();
        }
        if let Some(s) = substr {
            o.parent_bone = s.into();
        }
    }
}

#[rna_hook(srna = "Object", prop = "parent_type", krate = crate)]
mod parent_type {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let Some(value) = v.as_str() else {
            return Ok(());
        };
        let main = h.main();
        let Some(o) = main.objects.get(ob) else {
            return Ok(());
        };
        let parent_code = o
            .parent
            .map(|p| main.objects.get(p).and_then(|po| po.data).map(|d| d.code));
        let items = items(parent_code);
        if !items.contains(&value) {
            let listed = items
                .iter()
                .map(|n| format!("'{n}'"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(RnaError::new(
                RnaErrorKind::Enum,
                PyExc::TypeError,
                format!("bpy_struct: item.attr = val: enum \"{value}\" not found in ({listed})"),
            )
            .with_detail(
                "valid",
                items.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>(),
            ));
        }
        if o.parent_type == value {
            // "Skip if type did not change (otherwise we loose parent inverse)".
            return Ok(());
        }
        let value = value.to_owned();
        parent_set(h, ob, Some(&value), None);
        Ok(())
    }
}

#[rna_hook(srna = "Object", prop = "parent_bone", krate = crate)]
mod parent_bone {
    use super::*;

    pub fn set(
        h: &mut dyn Host,
        this: &RnaPtr,
        _prop: &'static PropDef,
        v: Value,
    ) -> Result<(), RnaError> {
        let ob = object_of(this)?;
        let Some(value) = v.as_str().map(str::to_owned) else {
            return Ok(());
        };
        parent_set(h, ob, None, Some(&value));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_follow_the_parent_type() {
        assert_eq!(items(None), ["OBJECT"]);
        assert_eq!(items(Some(None)), ["OBJECT"]);
        assert_eq!(items(Some(Some(IdCode::AR))), ["OBJECT", "ARMATURE", "BONE"]);
        assert_eq!(
            items(Some(Some(IdCode::ME))),
            ["OBJECT", "VERTEX", "VERTEX_3"]
        );
        assert_eq!(items(Some(Some(IdCode::LA))), ["OBJECT"]);
    }
}
