// SPDX-License-Identifier: GPL-3.0-or-later
//! The glTF add-on's `KHR_materials_variants` operators on `bpy.data` (M8 W0, decision log
//! M8W0-6): `scene.gltf2_variant_add`, `gltf2_variant_remove`, `gltf2_display_variant`,
//! `gltf2_assign_to_variant`, `gltf2_reset_to_original`, `gltf2_assign_as_original`,
//! `gltf2_variants_slot_add`, `gltf2_material_to_variant` and `gltf2_remove_material_variant`
//! (`frap-ops-object/src/ops/scene__gltf2_*.rs` call these). They exist while the preference
//! `KHR_materials_variants_ui` is on ([`ui_on`]; the add-on's `variant_register`).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/com/gltf2_blender_ui.py` (`SCENE_OT_gltf2_*`),
//! Apache-2.0, The glTF-Blender-IO authors, including its quirks: every operator reads
//! `bpy.data.scenes[0]` (not the context scene); `gltf2_variant_remove` revisits a mesh once
//! per object using it, and removes the collected indices in ascending order from a list that
//! shrinks meanwhile; a lookup keeps the **last** matching mapping entry.

use frap_rna::{IdRef, MaterialId, MeshId, ObjectId};
use smol_str::SmolStr;

use crate::Preferences;
use crate::data::{ChangeLog, Main};
use crate::id::GltfVariant;
use crate::id::slots::{set_slot_material, slot, slot_count};
use frap_mesh::gltf_variants::{
    GltfVariantDefaultMaterial, GltfVariantPointer, GltfVariantPrimitive, MeshGltfVariants,
};

/// The add-on module.
const ADDON: &str = "io_scene_gltf2";
/// The preference that registers the variant operators and properties.
pub const UI_PREF: &str = "KHR_materials_variants_ui";
/// The name `gltf2_variant_add` gives (before the name update makes it unique).
pub const NEW_VARIANT_NAME: &str = "VariantName";

/// Whether `KHR_materials_variants_ui` is on (the variant operators are registered).
pub fn ui_on(prefs: &Preferences) -> bool {
    prefs
        .addons
        .get(ADDON)
        .and_then(|p| p.get(UI_PREF))
        .and_then(frap_rna::Value::as_bool)
        .unwrap_or(false)
}

// Ported from io_scene_gltf2 5.2.41 `blender/com/gltf2_blender_utils.py` (`find_unused_name`),
// Apache-2.0, The glTF-Blender-IO authors.
/// A name not in `haystack`, at most 63 UTF-8 bytes: `desired`, then `desired.001`, … (the
/// stem shortened by whole characters while the result is too long).
pub fn find_unused_name(haystack: &[SmolStr], desired: &str) -> SmolStr {
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

/// What an operator body could not do, as the add-on's `execute` would fail.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum VariantOpError {
    /// The active object's data is not a mesh (the add-on raises `AttributeError` reading
    /// `gltf2_variant_mesh_data` from it).
    NotAMesh,
    /// `variants[active_variant_idx]` / `material_slots[i]` out of range (the add-on raises
    /// `IndexError: bpy_prop_collection[index]: index N out of range, size M`).
    Index {
        /// The index read.
        index: i64,
        /// The collection's size.
        size: usize,
    },
}

impl std::fmt::Display for VariantOpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariantOpError::NotAMesh => f.write_str(
                "the active object's data has no attribute 'gltf2_variant_mesh_data' (not a mesh)",
            ),
            VariantOpError::Index { index, size } => write!(
                f,
                "IndexError: bpy_prop_collection[index]: index {index} out of range, size {size}"
            ),
        }
    }
}

impl std::error::Error for VariantOpError {}

/// `bpy.data.scenes[0]`'s variants.
pub fn first_scene_variants(main: &Main) -> &[GltfVariant] {
    main.scenes
        .iter()
        .next()
        .map_or(&[], |(_, s)| s.gltf2_variants.variants.as_slice())
}

/// `bpy.data.scenes[0].gltf2_active_variant`.
pub fn first_scene_active_variant(main: &Main) -> i32 {
    main.scenes
        .iter()
        .next()
        .map_or(0, |(_, s)| s.gltf2_variants.active_variant)
}

fn mesh_of(main: &Main, ob: IdRef<ObjectId>) -> Option<IdRef<MeshId>> {
    main.objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<MeshId>())
}

fn mesh_variants(main: &Main, me: IdRef<MeshId>) -> Option<&MeshGltfVariants> {
    main.meshes.get(me).map(|m| &m.gltf2_variants)
}

fn with_mesh_variants<R>(
    main: &mut Main,
    me: IdRef<MeshId>,
    log: &mut ChangeLog,
    f: impl FnOnce(&mut MeshGltfVariants) -> R,
) -> Option<R> {
    main.meshes
        .get_mut(me, log)
        .map(|m| f(&mut m.gltf2_variants))
}

/// The material of every material slot of `ob`.
fn slot_materials(main: &Main, ob: IdRef<ObjectId>) -> Vec<Option<IdRef<MaterialId>>> {
    let Some(o) = main.objects.get(ob) else {
        return Vec::new();
    };
    (0..slot_count(main, o))
        .map(|i| slot(main, o, i).1)
        .collect()
}

/// The number of material slots of `ob` (`len(obj.material_slots)`).
pub fn material_slot_count(main: &Main, ob: IdRef<ObjectId>) -> usize {
    main.objects.get(ob).map_or(0, |o| slot_count(main, o))
}

/// `obj.active_material_index`.
fn active_slot(main: &Main, ob: IdRef<ObjectId>) -> i64 {
    main.objects
        .get(ob)
        .map_or(0, |o| i64::from(o.active_material_index))
}

/// Python indexing of `material_slots[i]` (negative indices count from the end).
fn py_index(i: i64, size: usize) -> Result<usize, VariantOpError> {
    let n = size as i64;
    let j = if i < 0 { i + n } else { i };
    if (0..n).contains(&j) {
        Ok(j as usize)
    } else {
        Err(VariantOpError::Index { index: i, size })
    }
}

/// `SCENE_OT_gltf2_variant_add.execute`: appends a variant to `bpy.data.scenes[0]`, numbers
/// it, names it `VariantName` (made unique by `on_variant_name_update`) and makes it active.
pub fn variant_add(main: &mut Main, log: &mut ChangeLog) {
    let Some(sc) = main.scenes.iter().next().map(|(r, _)| r) else {
        return;
    };
    let used: Vec<SmolStr> = first_scene_variants(main)
        .iter()
        .map(|v| v.name.clone())
        .collect();
    let Some(s) = main.scenes.get_mut(sc, log) else {
        return;
    };
    let vs = &mut s.gltf2_variants;
    vs.variants.push(GltfVariant::default());
    let n = vs.variants.len();
    // `var.variant_idx = len(...) - 1`, then `var.name = "VariantName"`: the update proposes a
    // name unused by the other variants (every other entry, since `variant_idx` is the new
    // item's own position).
    let name = find_unused_name(&used, NEW_VARIANT_NAME);
    let v = &mut vs.variants[n - 1];
    v.variant_idx = (n - 1) as i32;
    v.name = name;
    vs.active_variant = (n - 1) as i32;
}

/// `SCENE_OT_gltf2_variant_remove.execute`: removes `bpy.data.scenes[0]`'s active variant and
/// renumbers or drops the mesh mappings that refer to it.
pub fn variant_remove(main: &mut Main, log: &mut ChangeLog) {
    let Some(sc) = main.scenes.iter().next().map(|(r, _)| r) else {
        return;
    };
    let active = first_scene_active_variant(main);
    if let Some(s) = main.scenes.get_mut(sc, log) {
        let list = &mut s.gltf2_variants.variants;
        if (0..list.len() as i64).contains(&i64::from(active)) {
            list.remove(active as usize);
        }
    }
    // `for obj in [o for o in bpy.data.objects if o.type == "MESH"]`: a mesh is processed
    // once per object using it.
    let meshes: Vec<IdRef<MeshId>> = main
        .objects
        .iter()
        .filter_map(|(_, o)| o.data.and_then(|d| d.typed::<MeshId>()))
        .collect();
    for me in meshes {
        with_mesh_variants(main, me, log, |mv| {
            let mut remove_idx_data = Vec::new();
            for (idx, prim) in mv.mesh_data.iter_mut().enumerate() {
                let mut remove_idx_variants = Vec::new();
                for (idx_var, v) in prim.variants.iter_mut().enumerate() {
                    if v.variant_idx == active {
                        remove_idx_variants.push(idx_var);
                    } else if v.variant_idx > active {
                        v.variant_idx -= 1;
                    }
                }
                for idx_var in remove_idx_variants {
                    remove_shifting(&mut prim.variants, idx_var);
                }
                if prim.variants.is_empty() {
                    remove_idx_data.push(idx);
                }
            }
            for idx_data in remove_idx_data {
                remove_shifting(&mut mv.mesh_data, idx_data);
            }
        });
    }
}

/// `collection.remove(i)` of an ID-property collection: out-of-range indices do nothing.
fn remove_shifting<T>(list: &mut Vec<T>, i: usize) {
    if i < list.len() {
        list.remove(i);
    }
}

/// `SCENE_OT_gltf2_display_variant.execute`: every mesh object's slots take the materials
/// its mesh maps to `bpy.data.scenes[0].gltf2_active_variant`.
pub fn display_variant(main: &mut Main, log: &mut ChangeLog) -> Result<(), VariantOpError> {
    let active = first_scene_active_variant(main);
    let objects: Vec<(IdRef<ObjectId>, IdRef<MeshId>)> = main
        .objects
        .iter()
        .filter_map(|(r, o)| o.data.and_then(|d| d.typed::<MeshId>()).map(|m| (r, m)))
        .collect();
    for (ob, me) in objects {
        let writes: Vec<(i32, Option<IdRef<MaterialId>>)> = mesh_variants(main, me)
            .map(|mv| {
                mv.mesh_data
                    .iter()
                    .filter(|p| p.variants.iter().any(|v| v.variant_idx == active))
                    .map(|p| (p.material_slot_index, p.material))
                    .collect()
            })
            .unwrap_or_default();
        for (slot_index, mat) in writes {
            let n = material_slot_count(main, ob);
            // `if slot < len(obj.material_slots)`: a negative index passes and counts from
            // the end.
            if i64::from(slot_index) < n as i64 {
                let i = py_index(i64::from(slot_index), n)?;
                set_slot_material(main, ob, i, mat, log);
            }
        }
    }
    Ok(())
}

/// `SCENE_OT_gltf2_assign_to_variant.execute`: records the active object's slot materials as
/// the active variant's.
pub fn assign_to_variant(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<(), VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let active = first_scene_active_variant(main);
    let slots = slot_materials(main, ob);
    with_mesh_variants(main, me, log, |mv| {
        for (slot_idx, mat) in slots.iter().enumerate() {
            let slot_idx = slot_idx as i32;
            let mut found = None;
            let mut variant_found = false;
            for (k, p) in mv.mesh_data.iter_mut().enumerate() {
                if p.material_slot_index == slot_idx && p.material == *mat {
                    found = Some(k);
                } else if p.material_slot_index == slot_idx
                    && p.variants.iter().any(|v| v.variant_idx == active)
                {
                    // The user changed the material: store the new one.
                    found = Some(k);
                    variant_found = true;
                    p.material = *mat;
                }
            }
            let k = match found {
                Some(k) => k,
                None => {
                    mv.mesh_data.push(GltfVariantPrimitive {
                        material_slot_index: slot_idx,
                        material: *mat,
                        ..Default::default()
                    });
                    mv.mesh_data.len() - 1
                }
            };
            if !variant_found {
                mv.mesh_data[k].variants.push(GltfVariantPointer {
                    variant_idx: active,
                    ..Default::default()
                });
            }
        }
    });
    Ok(())
}

/// `SCENE_OT_gltf2_reset_to_original.execute`: the active object's slots take the mesh's
/// default materials.
pub fn reset_to_original(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<(), VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let n = material_slot_count(main, ob);
    let writes: Vec<(usize, Option<IdRef<MaterialId>>)> = mesh_variants(main, me)
        .map(|mv| {
            (0..n)
                .filter_map(|s| mv.default_for_slot(s).map(|m| (s, m)))
                .collect()
        })
        .unwrap_or_default();
    for (s, m) in writes {
        set_slot_material(main, ob, s, m, log);
    }
    Ok(())
}

/// `SCENE_OT_gltf2_assign_as_original.execute`: records the active object's slot materials as
/// the mesh's default materials.
pub fn assign_as_original(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<(), VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let slots = slot_materials(main, ob);
    with_mesh_variants(main, me, log, |mv| {
        for (slot_idx, mat) in slots.iter().enumerate() {
            let slot_idx = slot_idx as i32;
            match mv
                .default_materials
                .iter_mut()
                .find(|d| d.material_slot_index == slot_idx)
            {
                Some(d) => d.default_material = *mat,
                None => mv.default_materials.push(GltfVariantDefaultMaterial {
                    material_slot_index: slot_idx,
                    default_material: *mat,
                    ..Default::default()
                }),
            }
        }
    });
    Ok(())
}

/// The active slot of an object: its index, its material and the index of its mapping entry.
struct ActiveSlot {
    index: i32,
    material: Option<IdRef<MaterialId>>,
    entry: Option<usize>,
}

/// The last mapping entry of the active slot whose material is the slot's (the add-on's
/// lookup loops keep the last match), with the active slot index.
fn active_slot_entry(
    main: &Main,
    ob: IdRef<ObjectId>,
    me: IdRef<MeshId>,
) -> Result<ActiveSlot, VariantOpError> {
    let n = material_slot_count(main, ob);
    let active = active_slot(main, ob);
    let mat = slot_materials(main, ob)[py_index(active, n)?];
    let found = mesh_variants(main, me).and_then(|mv| {
        mv.mesh_data
            .iter()
            .rposition(|p| i64::from(p.material_slot_index) == active && p.material == mat)
    });
    Ok(ActiveSlot {
        index: active as i32,
        material: mat,
        entry: found,
    })
}

/// `SCENE_OT_gltf2_variant_slot_add.execute`: adds `bpy.data.scenes[0]`'s active variant to
/// the mapping of the active object's active slot (creating the mapping entry).
pub fn variants_slot_add(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<(), VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let ActiveSlot {
        index: slot_idx,
        material: mat,
        entry: found,
    } = active_slot_entry(main, ob, me)?;
    let active = first_scene_active_variant(main);
    with_mesh_variants(main, me, log, |mv| {
        let k = match found {
            Some(k) => k,
            None => {
                mv.mesh_data.push(GltfVariantPrimitive {
                    material_slot_index: slot_idx,
                    material: mat,
                    ..Default::default()
                });
                mv.mesh_data.len() - 1
            }
        };
        mv.mesh_data[k].variants.push(GltfVariantPointer {
            variant_idx: active,
            ..Default::default()
        });
    });
    Ok(())
}

/// `SCENE_OT_gltf2_material_to_variant.execute`: re-points the active variant entry of the
/// active slot's mapping to the variant named by `Mesh.gltf2_variant_pointer`. `Ok(false)`
/// is the add-on's `CANCELLED` (no mapping for the slot, or no variant of that name).
pub fn material_to_variant(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<bool, VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let Some(k) = active_slot_entry(main, ob, me)?.entry else {
        return Ok(false);
    };
    let (j, pointer) = {
        let mv = mesh_variants(main, me).ok_or(VariantOpError::NotAMesh)?;
        let p = &mv.mesh_data[k];
        (
            py_index(i64::from(p.active_variant_idx), p.variants.len())?,
            mv.pointer.clone(),
        )
    };
    let Some(idx) = first_scene_variants(main)
        .iter()
        .find(|v| v.name == pointer)
        .map(|v| v.variant_idx)
    else {
        return Ok(false);
    };
    with_mesh_variants(main, me, log, |mv| {
        mv.mesh_data[k].variants[j].variant_idx = idx;
    });
    Ok(true)
}

/// `SCENE_OT_gltf2_remove_material_variant.execute`: removes the active variant entry of the
/// active slot's mapping, and the mapping when it has no variant left. `Ok(false)` is the
/// add-on's `CANCELLED` (no mapping for the slot).
pub fn remove_material_variant(
    main: &mut Main,
    ob: IdRef<ObjectId>,
    log: &mut ChangeLog,
) -> Result<bool, VariantOpError> {
    let me = mesh_of(main, ob).ok_or(VariantOpError::NotAMesh)?;
    let Some(k) = active_slot_entry(main, ob, me)?.entry else {
        return Ok(false);
    };
    with_mesh_variants(main, me, log, |mv| {
        let p = &mut mv.mesh_data[k];
        let j = p.active_variant_idx;
        if (0..p.variants.len() as i64).contains(&i64::from(j)) {
            p.variants.remove(j as usize);
        }
        if p.variants.is_empty() {
            mv.mesh_data.remove(k);
        }
    });
    Ok(true)
}

/// Whether the active object's data holds mapping entries (`len(obj.data.gltf2_variant_mesh_data)
/// > 0`, the poll of `gltf2_remove_material_variant`); `None` when the data is not a mesh.
pub fn mesh_data_len(main: &Main, ob: IdRef<ObjectId>) -> Option<usize> {
    mesh_of(main, ob)
        .and_then(|me| mesh_variants(main, me))
        .map(|mv| mv.mesh_data.len())
}

/// `obj.data.gltf2_variant_default_materials` length (the poll of
/// `gltf2_reset_to_original`); `None` when the data is not a mesh.
pub fn default_materials_len(main: &Main, ob: IdRef<ObjectId>) -> Option<usize> {
    mesh_of(main, ob)
        .and_then(|me| mesh_variants(main, me))
        .map(|mv| mv.default_materials.len())
}

/// `obj.data.gltf2_variant_pointer` (the poll of `gltf2_material_to_variant`); `None` when
/// the data is not a mesh.
pub fn pointer(main: &Main, ob: IdRef<ObjectId>) -> Option<&SmolStr> {
    mesh_of(main, ob)
        .and_then(|me| mesh_variants(main, me))
        .map(|mv| &mv.pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn python_indexing_of_slots() {
        assert_eq!(py_index(-1, 3), Ok(2));
        assert_eq!(py_index(2, 3), Ok(2));
        assert_eq!(
            py_index(-4, 3),
            Err(VariantOpError::Index { index: -4, size: 3 })
        );
        assert_eq!(
            VariantOpError::Index { index: 0, size: 0 }.to_string(),
            "IndexError: bpy_prop_collection[index]: index 0 out of range, size 0"
        );
    }

    #[test]
    fn shifting_removal_skips_out_of_range() {
        let mut v = vec![1, 2];
        remove_shifting(&mut v, 0);
        remove_shifting(&mut v, 1);
        assert_eq!(v, vec![2]);
    }

    #[test]
    fn preference_defaults_off() {
        assert!(!ui_on(&Preferences::default()));
    }
}
