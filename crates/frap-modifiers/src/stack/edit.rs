// SPDX-License-Identifier: GPL-3.0-or-later
//! Stack editing shared by `Object.modifiers.new/remove/move/clear`, the `object.modifier_*`
//! operators, `object.shade_auto_smooth`, `object.subdivision_set` and the `use_pin_to_last` /
//! `is_active` setters.
//!
//! Ported from blender/source/blender/editors/object/object_modifier.cc (`modifier_add`,
//! `object_modifier_check_move_before/after`, `modifier_move_up/down/to_index`,
//! `modifier_copy`), blender/source/blender/blenkernel/intern/modifier.cc
//! (`modifier_allocate_and_init`, `BKE_modifiers_add_at_end_if_possible`,
//! `BKE_modifier_remove_from_list`, `BKE_modifier_unique_name`, `BKE_modifier_copydata_ex`),
//! blender/source/blender/blenkernel/intern/object.cc (`BKE_object_modifier_set_active`,
//! `BKE_object_active_modifier`) and blender/source/blender/makesrna/intern/rna_modifier.cc
//! (`rna_Modifier_use_pin_to_last_set`, `rna_Modifier_is_active_set`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (`-b --factory-startup`): `HOOK` is inserted after the leading deform-only
//! run (before a `Mirror`); a second `CLOTH` is refused with `Only one modifier of this type is
//! allowed`; `modifiers.new('Subdivision', 'BEVEL')` next to a `Subdivision` is
//! `Subdivision.001`; `modifiers.move(2, 99)` raises `Cannot move modifier beyond the end of the
//! stack`; pinning moves the modifier to the end of the unpinned run, unpinning a modifier that
//! is not pinned changes nothing; removing the active modifier activates the next one (the
//! previous one at the end of the list).
//!
//! `persistent_uid` is not Blender's hash-seeded random number but one more than the largest in
//! use (deviation D-47, as `object.parent_set`).

use frap_rna::naming::unique_subname;
use frap_scene::id::{Modifier, ModifierSettings, Object};
use smol_str::SmolStr;

use crate::{ModKind, ModTypeFlags, ModifierType, registry};

/// Why `modifier_add` refused (both are WARNING reports in Blender).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddError {
    /// `Modifiers cannot be added to object '<name>'`.
    Unsupported(String),
    /// `Only one modifier of this type is allowed`.
    Single,
}

impl std::fmt::Display for AddError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddError::Unsupported(name) => {
                write!(f, "Modifiers cannot be added to object '{name}'")
            }
            AddError::Single => f.write_str("Only one modifier of this type is allowed"),
        }
    }
}

/// `BKE_modifier_new`: a fresh modifier of `ty` with the type's default name and settings,
/// shown in viewport and render, in Edit Mode when the type enables it there, expanded.
pub fn new_modifier(ty: ModifierType) -> Modifier {
    let meta = registry::meta(ty);
    Modifier {
        name: SmolStr::new_static(meta.name),
        show_viewport: true,
        show_render: true,
        show_in_editmode: meta.flags.contains(ModTypeFlags::ENABLE_IN_EDITMODE),
        show_on_cage: false,
        show_expanded: true,
        is_active: false,
        use_pin_to_last: false,
        persistent_uid: 0,
        settings: ModifierSettings::new(ty),
    }
}

fn type_meta(m: &Modifier) -> Option<registry::TypeMeta> {
    super::type_of(m).map(registry::meta)
}

fn is_deform_only(m: &Modifier) -> bool {
    type_meta(m).is_some_and(|t| t.kind == ModKind::OnlyDeform)
}

fn requires_original_data(m: &Modifier) -> bool {
    type_meta(m).is_some_and(|t| t.flags.contains(ModTypeFlags::REQUIRES_ORIGINAL_DATA))
}

/// `BKE_modifier_unique_name`: `requested` (the type's default name when empty) made unique
/// among the other modifiers of the list (63 bytes, `.001` suffixes).
pub fn unique_name(mods: &[Modifier], skip: Option<usize>, requested: &str, ty: &str) -> SmolStr {
    let default = ModifierType::from_identifier(ty).map_or("Modifier", |t| registry::meta(t).name);
    unique_subname(
        &|n: &str| {
            mods.iter()
                .enumerate()
                .any(|(j, m)| Some(j) != skip && m.name == n)
        },
        requested,
        63,
        default,
    )
}

/// A new `persistent_uid` (D-47: one more than the largest in use).
pub fn next_persistent_uid(mods: &[Modifier]) -> u32 {
    mods.iter()
        .map(|m| m.persistent_uid)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

/// `BKE_object_active_modifier`.
pub fn active(mods: &[Modifier]) -> Option<usize> {
    mods.iter().position(|m| m.is_active)
}

/// `BKE_object_modifier_set_active` (`None` clears it).
pub fn set_active(mods: &mut [Modifier], i: Option<usize>) {
    for (j, m) in mods.iter_mut().enumerate() {
        m.is_active = Some(j) == i;
    }
}

/// `BKE_modifiers_add_at_end_if_possible`: the index a new modifier of `ty` is inserted at.
fn insert_index(mods: &[Modifier], ty: ModifierType) -> usize {
    // Before the trailing run of pinned modifiers.
    let mut next = mods.len();
    for (i, m) in mods.iter().enumerate().rev() {
        if m.use_pin_to_last {
            next = i;
        } else {
            break;
        }
    }
    let meta = registry::meta(ty);
    if meta.flags.contains(ModTypeFlags::REQUIRES_ORIGINAL_DATA) || ty == ModifierType::Hook {
        // After the leading run of deform-only modifiers (stopping before a pinned one).
        let mut i = 0;
        while i < mods.len() && is_deform_only(&mods[i]) {
            if mods.get(i + 1).is_some_and(|n| n.use_pin_to_last) {
                break;
            }
            i += 1;
        }
        next = i;
    }
    next
}

/// `ed::object::modifier_add` (without the particle-system special case): adds a modifier of
/// `ty` named `name` (the type's default name when `None` or empty), unique, at Blender's
/// position, and makes it the active one. Returns its index.
pub fn add(ob: &mut Object, name: Option<&str>, ty: ModifierType) -> Result<usize, AddError> {
    if !registry::object_supports(ob.object_type(), ty) {
        return Err(AddError::Unsupported(ob.header.name.to_string()));
    }
    let meta = registry::meta(ty);
    if meta.flags.contains(ModTypeFlags::SINGLE)
        && ob
            .modifiers
            .iter()
            .any(|m| m.settings.type_identifier() == ty.identifier())
    {
        return Err(AddError::Single);
    }
    let mut md = new_modifier(ty);
    md.persistent_uid = next_persistent_uid(&ob.modifiers);
    let at = insert_index(&ob.modifiers, ty);
    md.name = unique_name(&ob.modifiers, None, name.unwrap_or(""), ty.identifier());
    ob.modifiers.insert(at, md);
    set_active(&mut ob.modifiers, Some(at));
    Ok(at)
}

/// `BKE_modifier_remove_from_list`: removes modifier `i`; when it was the active one, the
/// next one (the previous one at the end of the list) becomes active.
pub fn remove(mods: &mut Vec<Modifier>, i: usize) -> Modifier {
    if mods[i].is_active {
        let next = if i + 1 < mods.len() {
            Some(i + 1)
        } else if i > 0 {
            Some(i - 1)
        } else {
            None
        };
        set_active(mods, next);
    }
    mods.remove(i)
}

/// Why a move was refused: Blender's report text, or `None` for the silent refusals (a pinned
/// modifier blocks an unpinned one).
pub type MoveError = Option<&'static str>;

/// `object_modifier_check_move_before(md, md_prev)`.
fn check_move_before(md: &Modifier, prev: Option<&Modifier>) -> Result<(), MoveError> {
    let Some(prev) = prev else {
        return Err(Some("Cannot move modifier beyond the start of the list"));
    };
    if md.use_pin_to_last && !prev.use_pin_to_last {
        return Err(None);
    }
    if !is_deform_only(md) && requires_original_data(prev) {
        return Err(Some("Cannot move above a modifier requiring original data"));
    }
    Ok(())
}

/// `object_modifier_check_move_after(md, md_next)`.
fn check_move_after(md: &Modifier, next: Option<&Modifier>) -> Result<(), MoveError> {
    let Some(next) = next else {
        return Err(Some("Cannot move modifier beyond the end of the list"));
    };
    if next.use_pin_to_last && !md.use_pin_to_last {
        return Err(None);
    }
    if requires_original_data(md) && !is_deform_only(next) {
        return Err(Some("Cannot move beyond a non-deforming modifier"));
    }
    Ok(())
}

/// `ed::object::modifier_move_up`.
pub fn move_up(mods: &mut [Modifier], i: usize) -> Result<(), MoveError> {
    check_move_before(&mods[i], i.checked_sub(1).map(|p| &mods[p]))?;
    mods.swap(i - 1, i);
    Ok(())
}

/// `ed::object::modifier_move_down`.
pub fn move_down(mods: &mut [Modifier], i: usize) -> Result<(), MoveError> {
    check_move_after(&mods[i], mods.get(i + 1))?;
    mods.swap(i, i + 1);
    Ok(())
}

/// `ed::object::modifier_move_to_index`: moves modifier `from` to `index`, one step at a time
/// with the move checks; with `allow_partial` it stops before the first refused step (unless
/// the very first step is refused).
pub fn move_to_index(
    mods: &mut Vec<Modifier>,
    from: usize,
    index: i64,
    allow_partial: bool,
) -> Result<(), MoveError> {
    if index < 0 || index >= mods.len() as i64 {
        return Err(Some("Cannot move modifier beyond the end of the stack"));
    }
    let index = index as usize;
    if from < index {
        let mut target = from;
        while target < index {
            if let Err(e) = check_move_after(&mods[from], mods.get(target + 1)) {
                if !allow_partial || target == from {
                    return Err(e);
                }
                break;
            }
            target += 1;
        }
        let md = mods.remove(from);
        mods.insert(target, md);
    } else if from > index {
        let mut target = from;
        while target > index {
            if let Err(e) = check_move_before(&mods[from], target.checked_sub(1).map(|p| &mods[p]))
            {
                if !allow_partial || target == from {
                    return Err(e);
                }
                break;
            }
            target -= 1;
        }
        let md = mods.remove(from);
        mods.insert(target, md);
    }
    Ok(())
}

/// `rna_Modifier_use_pin_to_last_set`: sets the flag, then moves the modifier to the end of
/// the unpinned run (pinning) or to the start of the pinned run (unpinning), stopping where a
/// move check refuses. Returns the modifier's new index.
pub fn set_pin_to_last(mods: &mut Vec<Modifier>, i: usize, value: bool) -> usize {
    // The RNA setter's implementation lives with the data (`frap-scene`), so the hook and
    // this function share it; `pin_rules_match_the_type_table` checks its type lists.
    Modifier::set_pin_to_last(mods, i, value)
}

/// `ed::object::modifier_copy` (non-particle types): a copy of modifier `i` (settings, flags,
/// pin) inserted after it, with a unique name, a new `persistent_uid`, made active. Returns
/// its index.
pub fn copy(mods: &mut Vec<Modifier>, i: usize) -> usize {
    let mut nmd = mods[i].clone();
    nmd.persistent_uid = next_persistent_uid(mods);
    nmd.name = unique_name(
        mods,
        None,
        &mods[i].name,
        mods[i].settings.type_identifier(),
    );
    mods.insert(i + 1, nmd);
    set_active(mods, Some(i + 1));
    i + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_rna::{IdKey, IdRef, MeshId};

    fn cube() -> Object {
        let me = IdRef::<MeshId>::from_key(IdKey::next());
        Object::new("Cube", Some(me.into()))
    }

    fn names(o: &Object) -> Vec<&str> {
        o.modifiers.iter().map(|m| m.name.as_str()).collect()
    }

    #[test]
    fn add_follows_blender_positions_and_names() {
        // The 5.2.4 probe: SUBSURF, HOOK, MIRROR, CLOTH, CLOTH again, PARTICLE_SYSTEM.
        let mut o = cube();
        add(&mut o, None, ModifierType::Subsurf).unwrap();
        add(&mut o, None, ModifierType::Hook).unwrap();
        add(&mut o, None, ModifierType::Mirror).unwrap();
        add(&mut o, None, ModifierType::Cloth).unwrap();
        assert_eq!(
            add(&mut o, None, ModifierType::Cloth),
            Err(AddError::Single)
        );
        add(&mut o, None, ModifierType::ParticleSystem).unwrap();
        assert_eq!(
            names(&o),
            ["Hook", "Subdivision", "Mirror", "Cloth", "ParticleSystem"]
        );
        assert_eq!(active(&o.modifiers), Some(4));
        add(&mut o, Some(""), ModifierType::Bevel).unwrap();
        add(&mut o, Some("Subdivision"), ModifierType::Bevel).unwrap();
        assert_eq!(names(&o)[5..], ["Bevel", "Subdivision.001"]);
        let m = &o.modifiers[1];
        assert!(m.show_viewport && m.show_render && m.show_in_editmode && m.show_expanded);
        assert!(!m.show_on_cage && !m.use_pin_to_last);
        assert!(
            !o.modifiers[0].show_in_editmode,
            "HOOK does not enable Edit Mode"
        );
        let mut cam = Object::new("Camera", None);
        cam.data = Some(IdRef::<frap_rna::CameraId>::from_key(IdKey::next()).into());
        assert_eq!(
            add(&mut cam, None, ModifierType::Subsurf)
                .unwrap_err()
                .to_string(),
            "Modifiers cannot be added to object 'Camera'"
        );
    }

    #[test]
    fn moves_and_pins_match_blender() {
        let mut o = cube();
        for t in [
            ModifierType::Subsurf,
            ModifierType::Hook,
            ModifierType::Mirror,
            ModifierType::Cloth,
            ModifierType::ParticleSystem,
            ModifierType::Bevel,
        ] {
            add(&mut o, None, t).unwrap();
        }
        add(&mut o, Some("Subdivision"), ModifierType::Bevel).unwrap();
        move_to_index(&mut o.modifiers, 0, 5, false).unwrap();
        move_to_index(&mut o.modifiers, 0, 2, false).unwrap();
        assert_eq!(
            move_to_index(&mut o.modifiers, 2, 99, false),
            Err(Some("Cannot move modifier beyond the end of the stack"))
        );
        assert_eq!(
            names(&o),
            [
                "Mirror",
                "Cloth",
                "Subdivision",
                "ParticleSystem",
                "Bevel",
                "Hook",
                "Subdivision.001"
            ]
        );
        // Remove Hook, then Mirror; make Cloth active and pin it, then pin Subdivision.
        remove(&mut o.modifiers, 5);
        remove(&mut o.modifiers, 0);
        set_active(&mut o.modifiers, Some(0));
        set_pin_to_last(&mut o.modifiers, 0, true);
        set_pin_to_last(&mut o.modifiers, 0, true);
        assert_eq!(
            names(&o),
            [
                "ParticleSystem",
                "Bevel",
                "Subdivision.001",
                "Subdivision",
                "Cloth"
            ]
        );
        add(&mut o, None, ModifierType::Weld).unwrap();
        assert_eq!(names(&o)[3], "Weld");
        set_pin_to_last(&mut o.modifiers, 1, false);
        assert_eq!(names(&o)[1], "Bevel");
        // A pinned modifier cannot move below an unpinned one, silently.
        assert_eq!(move_up(&mut o.modifiers, 4), Err(None));
        assert_eq!(
            move_up(&mut o.modifiers, 0),
            Err(Some("Cannot move modifier beyond the start of the list"))
        );
        assert_eq!(
            move_down(&mut o.modifiers, 5),
            Err(Some("Cannot move modifier beyond the end of the list"))
        );
    }

    #[test]
    fn remove_moves_the_active_flag() {
        let mut o = cube();
        for t in [
            ModifierType::Subsurf,
            ModifierType::Bevel,
            ModifierType::Weld,
        ] {
            add(&mut o, None, t).unwrap();
        }
        set_active(&mut o.modifiers, Some(1));
        remove(&mut o.modifiers, 1);
        assert_eq!(active(&o.modifiers), Some(1));
        remove(&mut o.modifiers, 1);
        assert_eq!(active(&o.modifiers), Some(0));
        let i = copy(&mut o.modifiers, 0);
        assert_eq!(names(&o), ["Subdivision", "Subdivision.001"]);
        assert_eq!(active(&o.modifiers), Some(i));
        assert_ne!(o.modifiers[0].persistent_uid, o.modifiers[1].persistent_uid);
    }

    #[test]
    fn pin_rules_match_the_type_table() {
        for t in ModifierType::ALL {
            let meta = registry::meta(t);
            assert_eq!(
                Modifier::DEFORM_ONLY_TYPES.contains(&t.identifier()),
                meta.kind == ModKind::OnlyDeform,
                "{t:?}"
            );
            assert_eq!(
                Modifier::REQUIRES_ORIGINAL_DATA_TYPES.contains(&t.identifier()),
                meta.flags.contains(ModTypeFlags::REQUIRES_ORIGINAL_DATA),
                "{t:?}"
            );
        }
    }

    #[test]
    fn requires_original_data_rules() {
        let mut o = cube();
        add(&mut o, None, ModifierType::Subsurf).unwrap();
        add(&mut o, None, ModifierType::Multires).unwrap();
        // MULTIRES goes after the (empty) leading deform run: first.
        assert_eq!(names(&o), ["Multires", "Subdivision"]);
        assert_eq!(
            move_up(&mut o.modifiers, 1),
            Err(Some("Cannot move above a modifier requiring original data"))
        );
        assert_eq!(
            move_down(&mut o.modifiers, 0),
            Err(Some("Cannot move beyond a non-deforming modifier"))
        );
    }
}
