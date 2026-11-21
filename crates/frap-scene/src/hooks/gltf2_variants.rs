// SPDX-License-Identifier: GPL-3.0-or-later
//! The glTF add-on's `KHR_materials_variants` RNA (WP 7.5, ARCHITECTURE §4.1, §17.2): the
//! PropertyGroups `io_scene_gltf2` registers while its preference `KHR_materials_variants_ui`
//! is on (`blender/com/gltf2_blender_ui.py`, `variant_register`):
//!
//! - `Scene.gltf2_KHR_materials_variants_variants` (`gltf2_KHR_materials_variants_variant`:
//!   `name`, `variant_idx`) and `Scene.gltf2_active_variant`;
//! - `Mesh.gltf2_variant_mesh_data` (`gltf2_KHR_materials_variants_primitive`:
//!   `material_slot_index`, `material`, `variants` of `gltf2_KHR_materials_variant_pointer`
//!   whose `variant` is a nested variant group, `active_variant_idx`),
//!   `Mesh.gltf2_variant_default_materials` (`gltf2_KHR_materials_variants_default_material`:
//!   `material_slot_index`, `default_material`) and `Mesh.gltf2_variant_pointer`.
//!
//! While the preference is off, reading or writing one of them is Blender's `AttributeError`
//! (`'Scene' object has no attribute 'gltf2_KHR_materials_variants_variants'`, verified on
//! 5.2.4 with `--factory-startup`). Every group also has `PropertyGroup.name`.
//!
//! The collections are ID-property collections (`bpy_prop_collection_idprop`): items are
//! addressed by index (`…variants[0]`), looked up by `name`, and changed with `add()`,
//! `remove(i)`, `clear()`, `move(a, b)`. frap-session dispatches those methods to the set hook
//! of the collection property with the value [`IdPropOp::decode`] reads (WP 7.5's additive
//! contract with frap-session, `session/funcs.rs::call_idprop`);
//! `remove`/`move` with an index out of range do nothing, as `RNA_property_collection_remove`
//! / `_move` (`rna_access.cc`).
//!
//! Writing a variant group's `name` runs the add-on's update callback
//! (`on_variant_name_update`): the name becomes unique among the names of
//! `bpy.data.scenes[0]`'s variants, skipping the one whose **position** equals the group's
//! `variant_idx` (`find_unused_name`: `Name.001`, …, at most 63 UTF-8 bytes).

use frap_mesh::gltf_variants::{
    GltfVariantDefaultMaterial, GltfVariantPointer, GltfVariantPrimitive, MeshGltfVariants,
};
use frap_rna::{
    CollRef, Field, IdCode, IdRef, Item, MaterialId, MeshId, PropDef, PyExc, Rna, RnaColl,
    RnaError, RnaErrorKind, RnaPtr, SceneId, Seg, StructDef, StructRef, Value,
};
use smol_str::SmolStr;

use crate::data::Main;
use crate::id::paths::{internal, root_id};
use crate::id::{GltfVariant, SceneGltfVariants};
use crate::{Host, HostRead};

/// The add-on module.
pub(crate) const ADDON: &str = "io_scene_gltf2";
/// The preference that registers the variant properties.
pub(crate) const UI_PREF: &str = "KHR_materials_variants_ui";
/// `on_variant_name_update` sets the name again from inside its own update; frap stops a
/// rename that never settles after this many rounds (Blender overflows its stack).
const MAX_RENAMES: usize = 1000;

/// Whether `KHR_materials_variants_ui` is on.
pub(crate) fn ui_on(h: &dyn HostRead) -> bool {
    h.ctx()
        .prefs
        .addons
        .get(ADDON)
        .and_then(|p| p.get(UI_PREF))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// Blender's answer for a property the add-on has not registered.
pub(crate) fn require_ui(h: &dyn HostRead, this: &RnaPtr, prop: &str) -> Result<(), RnaError> {
    if ui_on(h) {
        Ok(())
    } else {
        Err(frap_rna::traits::no_attribute(this.srna, prop))
    }
}

// ------------------------------------------------------------------ bpy_prop_collection_idprop

/// A `bpy_prop_collection_idprop` method, as frap-session hands it to the set hook of the
/// collection property.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum IdPropOp {
    /// `add()`: appends an item with default values.
    Add,
    /// `remove(index)` (the C `int` the index was truncated to).
    Remove(i32),
    /// `clear()`.
    Clear,
    /// `move(src_index, dst_index)`.
    Move(i32, i32),
}

impl IdPropOp {
    /// Decodes the set-hook value of a collection property: `("add",)`, `("remove", i)`,
    /// `("clear",)`, `("move", a, b)`.
    pub(crate) fn decode(v: &Value) -> Result<IdPropOp, RnaError> {
        let Value::Tuple(items) = v else {
            return Err(internal(
                "an ID-property collection is changed only by its methods",
            ));
        };
        let int = |i: usize| match items.get(i) {
            Some(Value::Int(n)) => Ok(*n as i32),
            _ => Err(internal(
                "an ID-property collection method without its index",
            )),
        };
        match items.first() {
            Some(Value::Str(s)) if s == "add" => Ok(IdPropOp::Add),
            Some(Value::Str(s)) if s == "clear" => Ok(IdPropOp::Clear),
            Some(Value::Str(s)) if s == "remove" => Ok(IdPropOp::Remove(int(1)?)),
            Some(Value::Str(s)) if s == "move" => Ok(IdPropOp::Move(int(1)?, int(2)?)),
            _ => Err(internal("unknown ID-property collection method")),
        }
    }

    /// Applies the method to a list (`RNA_property_collection_add/remove/clear/move`).
    pub(crate) fn apply<T: Default>(self, list: &mut Vec<T>) {
        let len = list.len() as i64;
        match self {
            IdPropOp::Add => list.push(T::default()),
            IdPropOp::Clear => list.clear(),
            IdPropOp::Remove(k) => {
                if (0..len).contains(&i64::from(k)) {
                    list.remove(k as usize);
                }
            }
            IdPropOp::Move(k, p) => {
                let (k64, p64) = (i64::from(k), i64::from(p));
                if (0..len).contains(&k64) && (0..len).contains(&p64) && k != p {
                    let item = list.remove(k as usize);
                    list.insert(p as usize, item);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ names

// Ported from io_scene_gltf2 5.2.41 `blender/com/gltf2_blender_utils.py` (`find_unused_name`),
// Apache-2.0, The glTF-Blender-IO authors.
/// A name not in `haystack`, at most 63 UTF-8 bytes: `desired`, then `desired.001`, … (the
/// stem shortened by whole characters while the result is too long).
pub(crate) fn find_unused_name(haystack: &[SmolStr], desired: &str) -> SmolStr {
    let mut stem: Vec<char> = desired.chars().take(63).collect();
    let mut suffix = String::new();
    let mut cntr = 1u64;
    loop {
        let name: String = stem.iter().collect::<String>() + &suffix;
        if name.len() > 63 {
            stem.pop();
            continue;
        }
        if !haystack.iter().any(|h| h == name.as_str()) {
            return SmolStr::new(name);
        }
        suffix = format!(".{cntr:03}");
        cntr += 1;
    }
}

/// `bpy.data.scenes[0]`'s variants.
fn first_scene_variants(main: &Main) -> &[GltfVariant] {
    main.scenes
        .iter()
        .next()
        .map_or(&[], |(_, s)| s.gltf2_variants.variants.as_slice())
}

/// `on_variant_name_update` for a group whose `variant_idx` is `variant_idx`: the name the
/// callback proposes for `name` (equal to `name` when it is unique).
fn proposed_name(main: &Main, variant_idx: i32, name: &str) -> SmolStr {
    let used: Vec<SmolStr> = first_scene_variants(main)
        .iter()
        .enumerate()
        .filter(|(i, _)| *i as i64 != i64::from(variant_idx))
        .map(|(_, v)| v.name.clone())
        .collect();
    find_unused_name(&used, name)
}

// ------------------------------------------------------------------ addressing

/// What a pointer under the variant properties addresses.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Place {
    /// `scene.gltf2_KHR_materials_variants_variants[i]`
    SceneVariant(IdRef<SceneId>, usize),
    /// `mesh.gltf2_variant_mesh_data[i]`
    Primitive(IdRef<MeshId>, usize),
    /// `mesh.gltf2_variant_mesh_data[i].variants[j]`
    Pointer(IdRef<MeshId>, usize, usize),
    /// `mesh.gltf2_variant_mesh_data[i].variants[j].variant`
    NestedVariant(IdRef<MeshId>, usize, usize),
    /// `mesh.gltf2_variant_default_materials[i]`
    Default(IdRef<MeshId>, usize),
}

fn index(seg: &Seg, len: usize) -> Option<usize> {
    match seg {
        Seg::Index(i) => {
            let n = len as i64;
            let i = if *i < 0 { i + n } else { *i };
            (0..n).contains(&i).then_some(i as usize)
        }
        _ => None,
    }
}

fn gone() -> RnaError {
    internal("the glTF variant item was removed")
}

/// Resolves `this` (an item of a variant collection, or the nested variant of a pointer).
pub(crate) fn place_of(main: &Main, this: &RnaPtr) -> Result<Place, RnaError> {
    let id = root_id(this)?;
    match id.code {
        IdCode::SC => {
            let sc: IdRef<SceneId> = IdRef::from_any(id).ok_or_else(gone)?;
            let s = main.scenes.get(sc).ok_or_else(gone)?;
            match this.path.as_slice() {
                [Seg::Prop("gltf2_KHR_materials_variants_variants"), seg] => {
                    let i = index(seg, s.gltf2_variants.variants.len()).ok_or_else(gone)?;
                    Ok(Place::SceneVariant(sc, i))
                }
                _ => Err(internal("not a glTF variant pointer")),
            }
        }
        IdCode::ME => {
            let me: IdRef<MeshId> = IdRef::from_any(id).ok_or_else(gone)?;
            let v = &main.meshes.get(me).ok_or_else(gone)?.gltf2_variants;
            match this.path.as_slice() {
                [Seg::Prop("gltf2_variant_default_materials"), seg] => Ok(Place::Default(
                    me,
                    index(seg, v.default_materials.len()).ok_or_else(gone)?,
                )),
                [Seg::Prop("gltf2_variant_mesh_data"), seg, rest @ ..] => {
                    let i = index(seg, v.mesh_data.len()).ok_or_else(gone)?;
                    let n = v.mesh_data[i].variants.len();
                    match rest {
                        [] => Ok(Place::Primitive(me, i)),
                        [Seg::Prop("variants"), seg] => {
                            Ok(Place::Pointer(me, i, index(seg, n).ok_or_else(gone)?))
                        }
                        [Seg::Prop("variants"), seg, Seg::Prop("variant")] => {
                            Ok(Place::NestedVariant(me, i, index(seg, n).ok_or_else(gone)?))
                        }
                        _ => Err(internal("not a glTF variant pointer")),
                    }
                }
                _ => Err(internal("not a glTF variant pointer")),
            }
        }
        _ => Err(internal("not a glTF variant pointer")),
    }
}

/// The scene a pointer is rooted at.
pub(crate) fn scene_root(this: &RnaPtr) -> Result<IdRef<SceneId>, RnaError> {
    crate::id::paths::scene_of(this)
}

/// The mesh a pointer is rooted at.
pub(crate) fn mesh_root(this: &RnaPtr) -> Result<IdRef<MeshId>, RnaError> {
    let id = root_id(this)?;
    if id.code != IdCode::ME {
        return Err(internal("the pointer is not rooted at a Mesh"));
    }
    IdRef::from_any(id).ok_or_else(|| internal("the pointer is not rooted at a Mesh"))
}

/// The variants of a scene, mutably.
pub(crate) fn scene_variants_mut(
    h: &mut dyn Host,
    sc: IdRef<SceneId>,
) -> Result<&mut SceneGltfVariants, RnaError> {
    let (main, log) = h.main_mut();
    main.scenes
        .get_mut(sc, log)
        .map(|s| &mut s.gltf2_variants)
        .ok_or_else(|| internal("the scene was removed"))
}

/// The variants of a mesh, mutably.
pub(crate) fn mesh_variants_mut(
    h: &mut dyn Host,
    me: IdRef<MeshId>,
) -> Result<&mut MeshGltfVariants, RnaError> {
    let (main, log) = h.main_mut();
    main.meshes
        .get_mut(me, log)
        .map(|m| &mut m.gltf2_variants)
        .ok_or_else(|| internal("the mesh was removed"))
}

// ------------------------------------------------------------------ writes

/// Writes `name` on the variant group at `place` and runs `on_variant_name_update`.
pub(crate) fn set_variant_name(
    h: &mut dyn Host,
    place: Place,
    name: SmolStr,
) -> Result<(), RnaError> {
    let mut name = name;
    for _ in 0..MAX_RENAMES {
        let variant_idx = {
            let (main, log) = h.main_mut();
            match place {
                Place::SceneVariant(sc, i) => {
                    let s = main.scenes.get_mut(sc, log).ok_or_else(gone)?;
                    let v = s.gltf2_variants.variants.get_mut(i).ok_or_else(gone)?;
                    v.name = name.clone();
                    v.variant_idx
                }
                Place::NestedVariant(me, i, j) => {
                    let m = main.meshes.get_mut(me, log).ok_or_else(gone)?;
                    let p = m
                        .gltf2_variants
                        .mesh_data
                        .get_mut(i)
                        .and_then(|p| p.variants.get_mut(j))
                        .ok_or_else(gone)?;
                    p.variant_name = name.clone();
                    p.variant_idx
                }
                _ => return Err(internal("not a glTF variant group")),
            }
        };
        let proposed = proposed_name(h.main(), variant_idx, &name);
        if proposed == name {
            return Ok(());
        }
        name = proposed;
    }
    Err(RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "frap: gltf2_KHR_materials_variants_variant.name: the add-on's name update never \
         settles (the variant's variant_idx is not its index in bpy.data.scenes[0]'s list, so \
         its own name always counts as taken; Blender overflows its stack here)",
    )
    .with_hint("set variant_idx to the variant's index in the list before setting its name"))
}

/// Writes an `INT` value.
pub(crate) fn int(v: &Value) -> Result<i32, RnaError> {
    match v {
        Value::Int(i) => Ok(*i as i32),
        Value::Bool(b) => Ok(i32::from(*b)),
        _ => Err(internal("an int property got another value")),
    }
}

/// Writes a `STRING` value.
pub(crate) fn string(v: &Value) -> Result<SmolStr, RnaError> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        _ => Err(internal("a string property got another value")),
    }
}

/// Writes a `Material` pointer value.
pub(crate) fn material(v: &Value) -> Result<Option<IdRef<MaterialId>>, RnaError> {
    match v {
        Value::Id(Some(a)) => IdRef::from_any(*a)
            .map(Some)
            .ok_or_else(|| internal("a Material pointer got another ID")),
        Value::Id(None) | Value::None => Ok(None),
        _ => Err(internal("a Material pointer got another value")),
    }
}

// ------------------------------------------------------------------ read proxies

fn str_field(s: &SmolStr) -> Field<'static> {
    Field::Value(Value::Str(s.clone()))
}

fn int_field(i: i32) -> Field<'static> {
    Field::Value(Value::Int(i64::from(i)))
}

fn mat_field(m: Option<IdRef<MaterialId>>) -> Field<'static> {
    Field::Id(m.map(|m| m.any()))
}

macro_rules! idprop_coll {
    ($name:ident, $item:ty, $srna:expr, $proxy:expr) => {
        /// An ID-property collection (items by index, keyed by `name`).
        pub(crate) struct $name<'h>(pub &'h [$item]);

        impl RnaColl for $name<'_> {
            fn item_type(&self) -> &'static StructDef {
                &$srna
            }
            fn len(&self) -> usize {
                self.0.len()
            }
            fn key_of(&self, i: usize) -> Option<SmolStr> {
                self.0.get(i).map(|x| x.name.clone())
            }
            fn find(&self, key: &str) -> Option<usize> {
                self.0.iter().position(|x| x.name == key)
            }
            fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
                match self.0.get(i) {
                    Some(x) => Ok(Item::Struct(StructRef::Owned(Box::new(($proxy)(x))))),
                    None => Err(frap_rna::traits::index_out_of_range(i, self.0.len())),
                }
            }
            /// `bpy_prop_collection_idprop` items are addressed by index
            /// (`…gltf2_variant_mesh_data[0]`, verified on 5.2.4).
            fn item_seg(&self, i: usize) -> Seg {
                Seg::Index(i as i64)
            }
        }
    };
}

idprop_coll!(
    SceneVariants,
    GltfVariant,
    frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_VARIANT,
    variant_ref
);

fn variant_ref(v: &GltfVariant) -> VariantRef<'_> {
    VariantRef {
        name: &v.name,
        variant_idx: v.variant_idx,
    }
}
idprop_coll!(
    Primitives,
    GltfVariantPrimitive,
    frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_PRIMITIVE,
    PrimitiveRef
);
idprop_coll!(
    Pointers,
    GltfVariantPointer,
    frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANT_POINTER,
    PointerRef
);
idprop_coll!(
    Defaults,
    GltfVariantDefaultMaterial,
    frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_DEFAULT_MATERIAL,
    DefaultRef
);

/// `gltf2_KHR_materials_variants_variant` (a scene's, or the `variant` of a pointer).
pub(crate) struct VariantRef<'h> {
    name: &'h SmolStr,
    variant_idx: i32,
}

impl Rna for VariantRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_VARIANT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        Ok(match prop.identifier {
            "name" => str_field(self.name),
            "variant_idx" => int_field(self.variant_idx),
            p => return Err(frap_rna::traits::no_attribute(self.srna(), p)),
        })
    }
}

/// `gltf2_KHR_materials_variants_primitive`.
pub(crate) struct PrimitiveRef<'h>(&'h GltfVariantPrimitive);

impl Rna for PrimitiveRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_PRIMITIVE
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let p = self.0;
        Ok(match prop.identifier {
            "name" => str_field(&p.name),
            "material_slot_index" => int_field(p.material_slot_index),
            "material" => mat_field(p.material),
            "variants" => Field::Coll(CollRef::Owned(Box::new(Pointers(&p.variants)))),
            "active_variant_idx" => int_field(p.active_variant_idx),
            other => return Err(frap_rna::traits::no_attribute(self.srna(), other)),
        })
    }
}

/// `gltf2_KHR_materials_variant_pointer`.
pub(crate) struct PointerRef<'h>(&'h GltfVariantPointer);

impl Rna for PointerRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANT_POINTER
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let p = self.0;
        Ok(match prop.identifier {
            "name" => str_field(&p.name),
            "variant" => Field::Struct(StructRef::Owned(Box::new(VariantRef {
                name: &p.variant_name,
                variant_idx: p.variant_idx,
            }))),
            other => return Err(frap_rna::traits::no_attribute(self.srna(), other)),
        })
    }
}

/// `gltf2_KHR_materials_variants_default_material`.
pub(crate) struct DefaultRef<'h>(&'h GltfVariantDefaultMaterial);

impl Rna for DefaultRef<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::GLTF2_KHR_MATERIALS_VARIANTS_DEFAULT_MATERIAL
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let d = self.0;
        Ok(match prop.identifier {
            "name" => str_field(&d.name),
            "material_slot_index" => int_field(d.material_slot_index),
            "default_material" => mat_field(d.default_material),
            other => return Err(frap_rna::traits::no_attribute(self.srna(), other)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unused_names_follow_the_addon() {
        let used: Vec<SmolStr> = vec!["Red".into(), "Red.001".into()];
        assert_eq!(find_unused_name(&used, "Red"), "Red.002");
        assert_eq!(find_unused_name(&used, "Blue"), "Blue");
        assert_eq!(find_unused_name(&[], &"x".repeat(70)), "x".repeat(63));
        // A taken 63-byte name loses characters to make room for the suffix.
        let long: SmolStr = "y".repeat(63).into();
        assert_eq!(
            find_unused_name(std::slice::from_ref(&long), &long),
            format!("{}.001", "y".repeat(59))
        );
        // Multi-byte characters are dropped whole.
        let e = "é".repeat(40);
        let n = find_unused_name(&[], &e);
        assert!(n.len() <= 63 && n.chars().all(|c| c == 'é'));
        assert_eq!(n.chars().count(), 31);
    }

    #[test]
    fn idprop_ops_ignore_out_of_range_indices() {
        let mut v = vec![1, 2, 3];
        IdPropOp::Remove(5).apply(&mut v);
        IdPropOp::Remove(-1).apply(&mut v);
        IdPropOp::Move(0, 3).apply(&mut v);
        assert_eq!(v, [1, 2, 3]);
        IdPropOp::Move(0, 2).apply(&mut v);
        assert_eq!(v, [2, 3, 1]);
        IdPropOp::Move(2, 0).apply(&mut v);
        assert_eq!(v, [1, 2, 3]);
        IdPropOp::Remove(1).apply(&mut v);
        IdPropOp::Add.apply(&mut v);
        assert_eq!(v, [1, 3, 0]);
        IdPropOp::Clear.apply(&mut v);
        assert!(v.is_empty());
    }

    #[test]
    fn ops_decode_from_the_session_encoding() {
        let t = |v: Vec<Value>| IdPropOp::decode(&Value::Tuple(v));
        assert_eq!(t(vec![Value::Str("add".into())]).unwrap(), IdPropOp::Add);
        assert_eq!(
            t(vec![
                Value::Str("move".into()),
                Value::Int(1),
                Value::Int(0)
            ])
            .unwrap(),
            IdPropOp::Move(1, 0)
        );
        // `PyLong_AsLong` then a C `int`: 2**32 removes item 0.
        assert_eq!(
            t(vec![Value::Str("remove".into()), Value::Int(1 << 32)]).unwrap(),
            IdPropOp::Remove(0)
        );
        assert!(IdPropOp::decode(&Value::Int(3)).is_err());
    }
}
