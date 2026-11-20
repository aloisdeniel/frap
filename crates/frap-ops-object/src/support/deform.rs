// SPDX-License-Identifier: GPL-3.0-or-later
//! The deform modifier `object.parent_set` adds for the `CURVE`, `LATTICE` and `ARMATURE*`
//! types, and the vertex groups of `ARMATURE_NAME` / `ARMATURE_ENVELOPE` (WP 6.5). Ports of
//! the modifier part of `ED_object_parent_set` (`editors/object/object_relations.cc`),
//! `BKE_modifiers_get_virtual_modifierlist` and `BKE_modifiers_is_deformed_by_{curve,lattice,
//! armature}` (`blenkernel/intern/modifier.cc`), the defaults of `ED_object_modifier_add` /
//! `BKE_modifier_new`, and `ED_object_vgroup_calc_from_armature` with `bone_looper`,
//! `vgroup_add_unique_bone_cb`, `dgroup_skinnable_cb`, `add_verts_to_dgroups` and
//! `envelope_bone_weighting` (`editors/armature/armature_skinning.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Verified on 5.2.4 (mesh Cube parented to Light): `LATTICE` adds a `Lattice` LATTICE
//! modifier whose `object` is Light, `CURVE` a `Curve` modifier, every `ARMATURE*` type an
//! `Armature` modifier, appended at the end of the stack with a unique name (`Lattice.001`
//! next to a modifier named `Lattice`); the new modifier is the active one, `show_viewport`,
//! `show_render` and `show_expanded` are on, `show_in_editmode` and `show_on_cage` off; the
//! parent gains no user. A second `LATTICE` with the same parent adds nothing; with another
//! parent it adds `Lattice.002`. A Light or Camera child (no modifiers) gets none.
//!
//! [`deformed_by`] walks the virtual modifier list: the shape-key modifier of a mesh with a
//! key, then the armature-parent one (`parent_type = 'ARMATURE'` with an armature parent),
//! then the object's modifiers. So a mesh already deformed by its armature parent gets no
//! ARMATURE modifier from `parent_set(type='ARMATURE')`. `persistent_uid` is not Blender's
//! hash-seeded random number: frap takes one more than the largest in use.
//!
//! Vertex groups from an armature, in `bone_looper`'s depth-first bone order, deforming bones
//! only:
//! - `ARMATURE_NAME`: an empty group per bone without one (the last one added becomes active);
//!   when groups were added, weights of group indices past the former group count are removed
//!   (`vgroup_data_clamp_range`);
//! - `ARMATURE_ENVELOPE`: a group per bone (existing ones reused, locked ones left alone), and
//!   every vertex gets the bone's envelope factor (`distfactor_to_bone` in world space, the
//!   radii and distance scaled by the armature's `mat4_to_scale`) as its weight, or leaves the
//!   group when the factor is 0; `xmirror` writes the same weight to the mirrored vertex
//!   (spatial X mirror) in the group of the flipped bone name.

use frap_bmesh::shape::deform as dv;
use frap_mesh::Mesh;
use frap_modifiers::types::armature::distfactor_to_bone;
use frap_rna::naming::unique_subname;
use frap_rna::{ArmatureId, IdAny, IdCode, IdRef, ObjectId};
use frap_scene::Main;
use frap_scene::edit_armature::names::flip_side_name;
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::{Armature, Modifier, ModifierSettings, Object};
use glam::{Mat4, Vec3};
use smol_str::SmolStr;

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

/// One entry of `BKE_modifiers_get_virtual_modifierlist`.
#[derive(Clone, Debug, PartialEq)]
pub enum StackEntry<'a> {
    /// The shape-key virtual modifier (a mesh with a `Key`).
    ShapeKey,
    /// A virtual deform modifier of a `PARSKEL` parent (frap has armature parents only;
    /// curve and lattice objects do not exist): its type and object.
    Parent(ModifierType, IdAny),
    /// One of the object's modifiers.
    Modifier(&'a Modifier),
}

/// The type of an object's data (`None` without data).
fn data_code(main: &Main, ob: IdRef<ObjectId>) -> Option<IdCode> {
    main.objects.get(ob)?.data.map(|d| d.code)
}

/// `BKE_modifiers_get_virtual_modifierlist`: the shape-key modifier, the parent's virtual
/// deform modifier (`parent_type = 'ARMATURE'`, Blender's `PARSKEL`, with an armature
/// parent), then the object's modifiers.
pub fn virtual_modifier_list<'a>(main: &Main, o: &'a Object) -> Vec<StackEntry<'a>> {
    let mut out = Vec::with_capacity(o.modifiers.len() + 2);
    let has_key = o
        .data
        .filter(|d| d.code == IdCode::ME)
        .and_then(|d| d.typed())
        .and_then(|me| main.meshes.get(me))
        .and_then(|m| m.shape_keys)
        .is_some_and(|k| main.shape_keys.get(k).is_some());
    if has_key {
        out.push(StackEntry::ShapeKey);
    }
    if let Some(p) = o.parent
        && o.parent_type == "ARMATURE"
        && data_code(main, p) == Some(IdCode::AR)
    {
        out.push(StackEntry::Parent(ModifierType::Armature, p.into()));
    }
    out.extend(o.modifiers.iter().map(StackEntry::Modifier));
    out
}

/// `BKE_modifiers_is_deformed_by_*`: over the virtual modifier list, the object of the first
/// modifier of type `ty` whose object is selected, else the object of the last modifier of
/// that type.
pub fn deformed_by(
    main: &Main,
    o: &Object,
    ty: ModifierType,
    is_selected: &dyn Fn(IdAny) -> bool,
) -> Option<IdAny> {
    let mut last = None;
    for e in virtual_modifier_list(main, o) {
        let ob = match e {
            StackEntry::ShapeKey => continue,
            StackEntry::Parent(t, ob) if t == ty => Some(ob),
            StackEntry::Parent(..) => continue,
            StackEntry::Modifier(m) if m.settings.type_identifier() == ty.identifier() => {
                target(&m.settings).flatten()
            }
            StackEntry::Modifier(_) => continue,
        };
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
    main: &Main,
    o: &mut Object,
    ty: ModifierType,
    parent: IdAny,
    is_selected: &dyn Fn(IdAny) -> bool,
) -> bool {
    if !supports_deform_modifiers(o) || deformed_by(main, o, ty, is_selected) == Some(parent) {
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

/// The armature of an armature object.
pub fn armature_of(main: &Main, ob: IdRef<ObjectId>) -> Option<&Armature> {
    let d = main.objects.get(ob)?.data?;
    main.armatures.get(d.typed::<ArmatureId>()?)
}

/// `bone_looper`'s order: depth first from the root bones, each bone before its children.
pub fn bone_order(arm: &Armature) -> Vec<usize> {
    let mut out = Vec::with_capacity(arm.bones.len());
    let mut stack: Vec<usize> = arm.children_of(None).into_iter().rev().collect();
    while let Some(i) = stack.pop() {
        if out.contains(&i) {
            continue;
        }
        out.push(i);
        stack.extend(arm.children_of(Some(i)).into_iter().rev());
    }
    out
}

/// `BKE_object_defgroup_add_name`: appends a group named `name` (made unique) and returns its
/// index; the caller makes it active.
fn add_group(me: &mut Mesh, name: &str) -> usize {
    let name = dv::unique_group_name(&me.vertex_group_names, None, name);
    me.vertex_group_names.push(name);
    me.vertex_group_names.len() - 1
}

/// `ED_object_vgroup_calc_from_armature(ARM_GROUPS_NAME)`: an empty group per deforming bone
/// that has none. Returns the index of the last group added (the new active group).
pub fn groups_from_bone_names(me: &mut Mesh, arm: &Armature) -> Option<usize> {
    let defbase_tot = me.vertex_group_names.len();
    let mut last = None;
    for i in bone_order(arm) {
        let b = &arm.bones[i];
        if b.use_deform && !me.vertex_group_names.contains(&b.name) {
            last = Some(add_group(me, &b.name));
        }
    }
    if last.is_some() && me.deform_verts().is_some() {
        // `vgroup_data_clamp_range`: weights of groups that did not exist become invalid.
        for d in me.deform_verts_mut().iter_mut() {
            let mut j = 0;
            while j < d.weights.len() {
                if d.weights[j].0 as usize >= defbase_tot {
                    dv::remove_at(d, j);
                } else {
                    j += 1;
                }
            }
        }
    }
    last
}

/// `mul_m4_v3`.
fn mul_m4_v3(m: &Mat4, v: Vec3) -> Vec3 {
    m.x_axis.truncate() * v.x
        + m.y_axis.truncate() * v.y
        + m.z_axis.truncate() * v.z
        + m.w_axis.truncate()
}

/// `mat4_to_scale`: the length of the unit diagonal vector the 3×3 part maps it to.
fn mat4_to_scale(m: &Mat4) -> f32 {
    // `float(M_SQRT1_3)`
    let s = 0.577_350_26_f32;
    m.transform_vector3(Vec3::splat(s)).length()
}

/// `mesh_get_x_mirror_vert_spatial`: the vertex nearest to `positions[i]` mirrored along X,
/// within `KD_THRESH` (`ED_mesh_mirror_spatial_table_lookup`).
fn mirror_vert(positions: &[[f32; 3]], i: usize) -> Option<usize> {
    const KD_THRESH: f32 = 0.00002;
    let p = positions[i];
    let v = Vec3::new(-p[0], p[1], p[2]);
    let mut best: Option<(usize, f32)> = None;
    for (j, q) in positions.iter().enumerate() {
        let d = Vec3::from_array(*q).distance(v);
        if best.is_none_or(|(_, bd)| d < bd) {
            best = Some((j, d));
        }
    }
    best.filter(|(_, d)| *d < KD_THRESH).map(|(j, _)| j)
}

/// What [`envelope_weights`] reads besides the mesh.
pub struct EnvelopeInput<'a> {
    /// The armature.
    pub arm: &'a Armature,
    /// The armature object's world matrix.
    pub arm_world: Mat4,
    /// The mesh object's world matrix.
    pub ob_world: Mat4,
    /// `xmirror`
    pub mirror: bool,
}

/// `add_verts_to_dgroups(heat = false)` with `envelope_bone_weighting`, in Object Mode (all
/// bones "selected"). Returns the index of the last group added (the new active group), if
/// any.
pub fn envelope_weights(me: &mut Mesh, inp: &EnvelopeInput<'_>) -> Option<usize> {
    let arm = inp.arm;
    let rest = frap_scene::edit_armature::rest::rest_pose(arm);
    let bones: Vec<usize> = bone_order(arm)
        .into_iter()
        .filter(|&i| arm.bones[i].use_deform)
        .collect();
    if bones.is_empty() {
        return None;
    }
    me.deform_verts_mut();
    let mut last_added = None;
    // `dgroup_skinnable_cb`: the group of each bone (created when missing; a locked one is
    // left alone).
    let groups: Vec<Option<usize>> = bones
        .iter()
        .map(|&i| {
            let name = &arm.bones[i].name;
            match me.vertex_group_names.iter().position(|n| n == name) {
                None => {
                    let g = add_group(me, name);
                    last_added = Some(g);
                    Some(g)
                }
                Some(g) if me.vertex_group_locked(g) => None,
                Some(g) => Some(g),
            }
        })
        .collect();
    let flips: Vec<Option<usize>> = groups
        .iter()
        .map(|g| {
            let g = (*g).filter(|_| inp.mirror)?;
            let flipped: SmolStr = flip_side_name(&me.vertex_group_names[g], false).into();
            me.vertex_group_names.iter().position(|n| *n == flipped)
        })
        .collect();
    let roots: Vec<Vec3> = bones
        .iter()
        .map(|&i| mul_m4_v3(&inp.arm_world, rest[i].arm_head))
        .collect();
    let tips: Vec<Vec3> = bones
        .iter()
        .map(|&i| mul_m4_v3(&inp.arm_world, rest[i].arm_tail))
        .collect();
    let scale = mat4_to_scale(&inp.arm_world);
    let local = me.positions().to_vec();
    let verts: Vec<Vec3> = local
        .iter()
        .map(|p| mul_m4_v3(&inp.ob_world, Vec3::from_array(*p)))
        .collect();
    let any_flip = flips.iter().any(Option::is_some);
    let dverts = me.deform_verts_mut();
    for (i, v) in verts.iter().enumerate() {
        let iflip = if any_flip {
            mirror_vert(&local, i)
        } else {
            None
        };
        for (j, &bi) in bones.iter().enumerate() {
            let b = &arm.bones[bi];
            let distance = distfactor_to_bone(
                *v,
                roots[j],
                tips[j],
                b.head_radius * scale,
                b.tail_radius * scale,
                b.envelope_distance * scale,
            );
            let mut write = |vi: usize, g: Option<usize>| {
                let (Some(g), Some(d)) = (g, dverts.get_mut(vi)) else {
                    return;
                };
                if distance != 0.0 {
                    dv::vert_add(d, g as u32, distance, dv::AssignMode::Replace);
                } else {
                    dv::remove_group(d, g as u32);
                }
            };
            write(i, groups[j]);
            if let Some(f) = iflip {
                write(f, flips[j]);
            }
        }
    }
    last_added
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_mesh::DeformVert;
    use frap_rna::{IdKey, MeshId};
    use frap_scene::id::Bone;

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
        let main = Main::default();
        let mut o = mesh_object();
        let (light, camera) = (ob(), ob());
        let none = |_: IdAny| false;
        let add = |o: &mut Object, ty, p| add_parent_deform(&main, o, ty, p, &none);
        assert!(add(&mut o, ModifierType::Lattice, light));
        assert!(!add(&mut o, ModifierType::Lattice, light));
        assert!(add(&mut o, ModifierType::Armature, light));
        assert!(add(&mut o, ModifierType::Lattice, camera));
        let names: Vec<_> = o.modifiers.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Lattice", "Armature", "Lattice.001"]);
        let active: Vec<_> = o.modifiers.iter().map(|m| m.is_active).collect();
        assert_eq!(active, [false, false, true]);
        let m = &o.modifiers[0];
        assert!(m.show_viewport && m.show_render && m.show_expanded);
        assert!(!m.show_in_editmode && !m.show_on_cage);
        assert_eq!(
            deformed_by(&main, &o, ModifierType::Lattice, &none),
            Some(camera)
        );
        assert_eq!(
            deformed_by(&main, &o, ModifierType::Lattice, &|t| t == light),
            Some(light)
        );
    }

    #[test]
    fn objects_without_modifiers_get_none() {
        let main = Main::default();
        let mut o = Object::new("Empty", None);
        assert!(!add_parent_deform(
            &main,
            &mut o,
            ModifierType::Curve,
            ob(),
            &|_| false
        ));
        assert!(o.modifiers.is_empty());
    }

    fn points(p: &[[f32; 3]]) -> Mesh {
        let mut me = Mesh::new("M");
        me.from_pydata(p, &[], &[], false);
        me
    }

    fn rig() -> Armature {
        let mut arm = Armature::new("Armature");
        let mut root = Bone::new("Root");
        root.head_local = [0.0, 0.0, 0.0];
        root.tail_local = [0.0, 0.0, 1.0];
        let mut nd = Bone::new("NoDeform");
        nd.use_deform = false;
        nd.head_local = [1.0, 0.0, 0.0];
        nd.tail_local = [1.0, 0.0, 1.0];
        let mut child = Bone::new("Arm.L");
        child.parent = Some(0);
        child.head_local = [0.0, 0.0, 1.0];
        child.tail_local = [1.0, 0.0, 1.0];
        let mut other = Bone::new("Arm.R");
        other.head_local = [0.0, 0.0, 1.0];
        other.tail_local = [-1.0, 0.0, 1.0];
        arm.bones = vec![root, nd, other, child];
        arm.bones[2].parent = Some(0);
        arm
    }

    #[test]
    fn bone_looper_order_is_depth_first() {
        let arm = rig();
        let names: Vec<_> = bone_order(&arm)
            .into_iter()
            .map(|i| arm.bones[i].name.as_str())
            .collect();
        assert_eq!(names, ["Root", "Arm.R", "Arm.L", "NoDeform"]);
    }

    #[test]
    fn name_groups_skip_existing_and_non_deforming() {
        let arm = rig();
        let mut me = points(&[[0.0; 3]]);
        me.vertex_group_names = vec!["Arm.R".into()];
        me.deform_verts_mut()[0] = DeformVert {
            weights: vec![(0, 0.5), (3, 1.0)].into(),
        };
        assert_eq!(groups_from_bone_names(&mut me, &arm), Some(2));
        assert_eq!(me.vertex_group_names, ["Arm.R", "Root", "Arm.L"]);
        // The weight of the out-of-range group 3 was dropped.
        assert_eq!(me.deform_verts().unwrap()[0].weights.as_slice(), [(0, 0.5)]);
        assert_eq!(groups_from_bone_names(&mut me, &arm), None);
    }

    #[test]
    fn envelope_weights_and_mirror() {
        let arm = rig();
        let mut me = points(&[
            [0.0, 0.0, 0.5],
            [0.9, 0.0, 1.0],
            [-0.9, 0.0, 1.0],
            [5.0, 5.0, 5.0],
        ]);
        let inp = EnvelopeInput {
            arm: &arm,
            arm_world: Mat4::IDENTITY,
            ob_world: Mat4::IDENTITY,
            mirror: true,
        };
        assert_eq!(envelope_weights(&mut me, &inp), Some(2));
        assert_eq!(me.vertex_group_names, ["Root", "Arm.R", "Arm.L"]);
        let d = me.deform_verts().unwrap();
        assert_eq!(d[0].weights.as_slice(), [(0, 1.0)]);
        // Vertex 1 is in Arm.L's envelope; its mirror (vertex 2) gets the same weight in
        // Arm.R's group, and vice versa.
        let w = |i: usize, g: u32| dv::weight(&d[i], g);
        assert!(w(1, 2) > 0.0 && w(2, 1) > 0.0);
        assert_eq!(w(1, 2), w(2, 1));
        assert!(d[3].weights.is_empty());
    }
}
