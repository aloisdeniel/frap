// SPDX-License-Identifier: GPL-3.0-or-later
//! `Object.vertex_groups` (`rna_Object_vertex_groups_begin`, WP 6.3): the vertex groups of a
//! mesh object, whose names live on the mesh (`Mesh.vertex_group_names`, DNA-level, so linked
//! duplicates share them; §5.6). Objects of other types answer an empty collection, as
//! Blender does for types without vertex-group support.
//!
//! Items are `VertexGroup` proxies (`name`, `index`, `lock_weight`); `VertexGroup.name`
//! writes go through `hooks/vertex_group_name.rs`. frap's mesh has no per-group flags, so
//! `lock_weight` reads `False` and cannot be written yet (`DG_LOCK_WEIGHT`, M8).

use frap_rna::{
    CollRef, Field, IdCode, IdRef, Item, MeshId, ObjectId, PropDef, Rna, RnaColl, RnaError,
    RnaPtr, Seg, StructDef, StructRef, Value, rna_hook,
};
use smol_str::SmolStr;

use crate::HostRead;
use crate::data::Main;
use crate::id::paths::{internal, object_of};

/// `Object` binds `vertex_groups` here: the `rna_struct!` binding of `Object` belongs to
/// WP 1.10 and lists only the M1 members, so the hook declaration lives with the hook.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "Object",
    prop: "vertex_groups",
    coll: true,
};

/// The mesh of a mesh object (`None` for other types and dangling data).
pub(crate) fn object_mesh(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    let d = main.objects.get(ob)?.data?;
    if d.code != IdCode::ME {
        return None;
    }
    let me = d.typed::<MeshId>()?;
    main.meshes.get(me).map(|_| me)
}

/// The vertex-group names of an object (empty for unsupported types).
pub(crate) fn group_names(main: &Main, ob: IdRef<ObjectId>) -> &[SmolStr] {
    object_mesh(main, ob)
        .and_then(|me| main.meshes.get(me))
        .map_or(&[], |m| m.vertex_group_names.as_slice())
}

/// The index of the group a `vertex_groups[<key or index>]` segment names.
pub(crate) fn group_index(names: &[SmolStr], seg: &Seg) -> Option<usize> {
    match seg {
        Seg::Key(k) => names.iter().position(|n| n == k),
        Seg::Index(i) => {
            let n = names.len() as i64;
            let i = if *i < 0 { i + n } else { *i };
            (0..n).contains(&i).then_some(i as usize)
        }
        Seg::Prop(_) => None,
    }
}

/// The object and group index a `VertexGroup` pointer names.
pub(crate) fn group_of(main: &Main, this: &RnaPtr) -> Result<(IdRef<ObjectId>, usize), RnaError> {
    let ob = object_of(this)?;
    let names = group_names(main, ob);
    match this.path.as_slice() {
        [Seg::Prop("vertex_groups"), seg, ..] => group_index(names, seg)
            .map(|i| (ob, i))
            .ok_or_else(|| internal("VertexGroup: the vertex group was removed")),
        _ => Err(internal("not a vertex group pointer")),
    }
}

/// The canonical pointer `bpy.data.objects['X'].vertex_groups["Group"]`.
pub(crate) fn group_ptr(ob: IdRef<ObjectId>, name: &str) -> RnaPtr {
    RnaPtr::id(ob.any(), &frap_spec_rna::OBJECT)
        .child(Seg::Prop("vertex_groups"), &frap_spec_rna::VERTEX_GROUPS)
        .child(Seg::Key(name.into()), &frap_spec_rna::VERTEX_GROUP)
}

/// `Object.vertex_groups`.
pub(crate) struct VertexGroups<'h> {
    pub names: &'h [SmolStr],
}

impl RnaColl for VertexGroups<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::VERTEX_GROUP
    }
    fn len(&self) -> usize {
        self.names.len()
    }
    fn key_of(&self, i: usize) -> Option<SmolStr> {
        self.names.get(i).cloned()
    }
    fn find(&self, key: &str) -> Option<usize> {
        self.names.iter().position(|n| n == key)
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match self.names.get(i) {
            Some(n) => Ok(Item::Struct(StructRef::Owned(Box::new(VertexGroupRef {
                name: n.clone(),
                index: i,
            })))),
            None => Err(frap_rna::traits::index_out_of_range(i, self.names.len())),
        }
    }
}

/// One `VertexGroup` (read proxy).
pub(crate) struct VertexGroupRef {
    pub name: SmolStr,
    pub index: usize,
}

impl Rna for VertexGroupRef {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::VERTEX_GROUP
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(Field::Value(match prop.identifier {
            "name" => Value::Str(self.name.clone()),
            "index" => Value::Int(self.index as i64),
            "lock_weight" => Value::Bool(false),
            _ => return Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }))
    }
}

#[rna_hook(srna = "Object", prop = "vertex_groups", krate = crate)]
fn get<'h>(
    h: &'h dyn HostRead,
    this: &RnaPtr,
    _prop: &'static PropDef,
) -> Result<Field<'h>, RnaError> {
    let main = h.main();
    let ob = object_of(this)?;
    if main.objects.get(ob).is_none() {
        return Err(internal("Object.vertex_groups: the object was removed"));
    }
    Ok(Field::Coll(CollRef::Owned(Box::new(VertexGroups {
        names: group_names(main, ob),
    }))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_index_resolves_keys_and_negative_indices() {
        let names: Vec<SmolStr> = vec!["A".into(), "B".into()];
        assert_eq!(group_index(&names, &Seg::Key("B".into())), Some(1));
        assert_eq!(group_index(&names, &Seg::Index(-1)), Some(1));
        assert_eq!(group_index(&names, &Seg::Index(2)), None);
    }
}
