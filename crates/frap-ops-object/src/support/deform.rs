// SPDX-License-Identifier: GPL-3.0-or-later
//! The deform modifier `object.parent_set` adds for the `CURVE`, `LATTICE` and `ARMATURE*`
//! types. Ports of the modifier part of `ED_object_parent_set` (`editors/object/
//! object_relations.cc`), `BKE_modifiers_is_deformed_by_{curve,lattice,armature}`
//! (`blenkernel/intern/modifier.cc`) and the defaults of `ED_object_modifier_add` /
//! `BKE_modifier_new`, Blender 5.2.4, GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (mesh Cube parented to Light): `LATTICE` adds a `Lattice` LATTICE
//! modifier whose `object` is Light, `CURVE` a `Curve` modifier, every `ARMATURE*` type an
//! `Armature` modifier, appended at the end of the stack with a unique name (`Lattice.001`
//! next to a modifier named `Lattice`); the new modifier is the active one, `show_viewport`,
//! `show_render` and `show_expanded` are on, `show_in_editmode` and `show_on_cage` off; the
//! parent gains no user. A second `LATTICE` with the same parent adds nothing; with another
//! parent it adds `Lattice.002`. A Light or Camera child (no modifiers) gets none.
//!
//! Virtual modifiers (the armature-parent and shape-key ones) are not considered by
//! `deformed_by` (frap has no armature parents before M6). `persistent_uid` is not Blender's
//! hash-seeded random number: frap takes one more than the largest in use.

use frap_rna::naming::unique_subname;
use frap_rna::{IdAny, IdCode};
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::{Modifier, ModifierSettings, Object};

/// The modifier type a parent type adds (`None` for the other types).
pub fn parent_type_modifier(parent_type: &str) -> Option<ModifierType> {
    match parent_type {
        "CURVE" => Some(ModifierType::Curve),
        "LATTICE" => Some(ModifierType::Lattice),
        "ARMATURE" | "ARMATURE_NAME" | "ARMATURE_AUTO" | "ARMATURE_ENVELOPE" => {
            Some(ModifierType::Armature)
        }
        _ => None,
    }
}

/// Whether an object can take a deform modifier (frap's object types: meshes only).
pub fn supports_deform_modifiers(o: &Object) -> bool {
    o.data.map(|d| d.code) == Some(IdCode::ME)
}

fn target(settings: &ModifierSettings) -> Option<Option<IdAny>> {
    match settings {
        ModifierSettings::Curve(s) => Some(s.object),
        ModifierSettings::Lattice(s) => Some(s.object),
        ModifierSettings::Armature(s) => Some(s.object),
        _ => None,
    }
}

/// `BKE_modifiers_is_deformed_by_*`: the object of the first modifier of type `ty` whose object
/// is selected, else the object of the last modifier of that type.
pub fn deformed_by(
    o: &Object,
    ty: ModifierType,
    is_selected: &dyn Fn(IdAny) -> bool,
) -> Option<IdAny> {
    let mut last = None;
    for m in &o.modifiers {
        if m.settings.type_identifier() != ty.identifier() {
            continue;
        }
        let ob = target(&m.settings).flatten();
        if let Some(t) = ob
            && is_selected(t)
        {
            return Some(t);
        }
        last = Some(ob);
    }
    last.flatten()
}

/// Adds the deform modifier of `ty` pointing at `parent` unless `o` is already deformed by it
/// (the `switch (partype)` of `ED_object_parent_set`). Returns whether one was added.
pub fn add_parent_deform(
    o: &mut Object,
    ty: ModifierType,
    parent: IdAny,
    is_selected: &dyn Fn(IdAny) -> bool,
) -> bool {
    if !supports_deform_modifiers(o) || deformed_by(o, ty, is_selected) == Some(parent) {
        return false;
    }
    let mut settings = ModifierSettings::new(ty);
    match &mut settings {
        ModifierSettings::Curve(s) => s.object = Some(parent),
        ModifierSettings::Lattice(s) => s.object = Some(parent),
        ModifierSettings::Armature(s) => s.object = Some(parent),
        _ => return false,
    }
    let base = match ty {
        ModifierType::Curve => "Curve",
        ModifierType::Lattice => "Lattice",
        _ => "Armature",
    };
    let name = unique_subname(
        &|n: &str| o.modifiers.iter().any(|m| m.name == n),
        base,
        63,
        base,
    );
    let persistent_uid = o
        .modifiers
        .iter()
        .map(|m| m.persistent_uid)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for m in &mut o.modifiers {
        m.is_active = false;
    }
    o.modifiers.push(Modifier {
        name,
        show_viewport: true,
        show_render: true,
        show_in_editmode: false,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid,
        settings,
    });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::{IdKey, IdRef, MeshId, ObjectId};

    fn mesh_object() -> Object {
        let me = IdRef::<MeshId>::from_key(IdKey::next());
        Object::new("Cube", Some(me.into()))
    }

    fn ob() -> IdAny {
        IdRef::<ObjectId>::from_key(IdKey::next()).into()
    }

    #[test]
    fn parent_types_map_to_modifiers() {
        assert_eq!(parent_type_modifier("LATTICE"), Some(ModifierType::Lattice));
        assert_eq!(parent_type_modifier("CURVE"), Some(ModifierType::Curve));
        assert_eq!(
            parent_type_modifier("ARMATURE_AUTO"),
            Some(ModifierType::Armature)
        );
        assert_eq!(parent_type_modifier("OBJECT"), None);
        assert_eq!(parent_type_modifier("FOLLOW"), None);
    }

    #[test]
    fn adds_once_per_parent_with_unique_names() {
        let mut o = mesh_object();
        let (light, camera) = (ob(), ob());
        let none = |_: IdAny| false;
        assert!(add_parent_deform(
            &mut o,
            ModifierType::Lattice,
            light,
            &none
        ));
        assert!(!add_parent_deform(
            &mut o,
            ModifierType::Lattice,
            light,
            &none
        ));
        assert!(add_parent_deform(
            &mut o,
            ModifierType::Armature,
            light,
            &none
        ));
        assert!(add_parent_deform(
            &mut o,
            ModifierType::Lattice,
            camera,
            &none
        ));
        let names: Vec<_> = o.modifiers.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Lattice", "Armature", "Lattice.001"]);
        let active: Vec<_> = o.modifiers.iter().map(|m| m.is_active).collect();
        assert_eq!(active, [false, false, true]);
        let m = &o.modifiers[0];
        assert!(m.show_viewport && m.show_render && m.show_expanded);
        assert!(!m.show_in_editmode && !m.show_on_cage);
        assert_eq!(deformed_by(&o, ModifierType::Lattice, &none), Some(camera));
        assert_eq!(
            deformed_by(&o, ModifierType::Lattice, &|t| t == light),
            Some(light)
        );
    }

    #[test]
    fn objects_without_modifiers_get_none() {
        let mut o = Object::new("Empty", None);
        assert!(!add_parent_deform(
            &mut o,
            ModifierType::Curve,
            ob(),
            &|_| false
        ));
        assert!(o.modifiers.is_empty());
    }
}
