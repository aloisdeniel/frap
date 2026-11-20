// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF skin import (WP 6.8): armatures and bones from the joint hierarchy (`vnode.rs`:
//! `mark_bones_and_armas`, `move_skinned_meshes`, `pick_bind_pose`, `bone_heuristic`
//! `BLENDER`/`TEMPERANCE`/`FORTUNE`, `guess_original_bind_pose`), the edit bones and pose of
//! each armature, the bone shapes of the `BLENDER` heuristic, vertex groups and the ARMATURE
//! modifier on skinned meshes, the mesh moved into the bind pose.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors)
//! `blender/imp/node.py` (`create_object`'s armature branch, `armature_display`,
//! `create_bones`, `setup_skinning`) and `blender/imp/mesh.py` (`skin_into_bind_pose`);
//! `EditBone.align_roll` from blender/source/blender/editors/armature/armature_utils.cc
//! (`ED_armature_ebone_roll_to_vector`) and `EditBone.length` from
//! blender/source/blender/makesrna/intern/rna_armature.cc (`rna_EditBone_length_set`),
//! Blender 5.2.4, GPL-2.0-or-later.
//!
//! Not modelled by frap: `PoseBone.custom_shape_scale_xyz` and `use_custom_shape_bone_size`
//! (the bone shapes get the icosphere at its default scale) and custom properties on bones
//! (node `extras`).

use frap_gltf::json::AccessorType;
use frap_math::glam::{Mat4, Vec3};
use frap_rna::{IdAny, IdRef, ObjectId};
use frap_scene::edit_armature::EditArmature;
use frap_scene::edit_armature::math::vec_roll_to_mat3_normalized;
use frap_scene::generated::modifier_settings::ModifierType;
use frap_scene::id::{Armature, Collection, Modifier, ModifierSettings, Object, Pose};
use frap_scene::{ChangeLog, Main};

use super::convert::{qconj, qmul, qrot};
use super::settings::BoneHeuristic;
use super::vnode::VKind;
use super::{Gltf, ImportError};

/// Whether skins are imported (WP 6.8 sets it).
pub(crate) const IMPLEMENTED: bool = true;

/// The inverse bind matrices of every skin in Blender space (`None` without an accessor).
pub(crate) fn inverse_bind_matrices(cx: &Gltf) -> Result<Vec<Option<Vec<Mat4>>>, ImportError> {
    let mut out = Vec::with_capacity(cx.doc.skins.len());
    for skin in &cx.doc.skins {
        let Some(a) = skin.inverse_bind_matrices else {
            out.push(None);
            continue;
        };
        let data = cx.accessor(a)?;
        if data.type_ != AccessorType::Mat4 {
            return Err(ImportError::Data(format!(
                "inverseBindMatrices accessor {a} is not MAT4"
            )));
        }
        let flat = data.to_f32();
        out.push(Some(
            flat.chunks_exact(16)
                .map(|m| cx.units.matrix(std::array::from_fn(|i| f64::from(m[i]))))
                .collect(),
        ));
    }
    Ok(out)
}

/// `compute_vnodes`' bone passes, once, before any object is created.
pub(crate) fn compute_bones(cx: &mut Gltf) -> Result<(), ImportError> {
    if cx.doc.skins.is_empty() {
        return Ok(());
    }
    let ibms = inverse_bind_matrices(cx)?;
    let s = cx.settings;
    cx.vtree
        .compute_bones(&cx.doc, &ibms, s.bone_heuristic, s.guess_original_bind_pose);
    Ok(())
}

/// The armature datablock of armature vnode `id` (`bpy.data.armatures.new(vnode.arma_name)`).
pub(crate) fn create_armature(cx: &Gltf, main: &mut Main, log: &mut ChangeLog, id: usize) -> IdAny {
    let name = cx.vtree.nodes[id].arma_name.clone();
    main.armatures.insert(&name, Armature::new(""), log).any()
}

/// `armature_display` (`bone_heuristic = 'BLENDER'`): in front, relation lines from the
/// heads, and (unless `disable_bone_shape`) a new icosphere in the hidden `glTF_not_exported`
/// collection as the bone shape. Returns the icosphere.
pub(crate) fn armature_display(
    cx: &Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    ob: IdRef<ObjectId>,
) -> Option<IdRef<ObjectId>> {
    let arm = main.objects.get(ob).and_then(|o| o.data);
    if let Some(o) = main.objects.get_mut(ob, log) {
        o.show_in_front = true;
    }
    if let Some(a) = arm.and_then(|a| a.typed())
        && let Some(a) = main.armatures.get_mut(a, log)
    {
        a.relation_line_position = "HEAD".into();
    }
    if cx.settings.disable_bone_shape {
        return None;
    }
    let special = crate::export::vtree::SPECIAL_COLLECTION;
    let coll = match main.collections.find(special) {
        Some(c) => c,
        None => {
            let mut c = Collection::new("");
            c.hide_viewport = true;
            c.hide_render = true;
            let c = main.collections.insert(special, c, log);
            if let Some(s) = main.scenes.get_mut(cx.scene, log)
                && !s.collection.children.contains(&c)
            {
                s.collection.children.push(c);
            }
            c
        }
    };
    // `bpy.ops.mesh.primitive_ico_sphere_add(radius=1)`, moved to the special collection.
    let mesh = frap_mesh::primitives::ico_sphere::ico_sphere(
        &frap_mesh::primitives::ico_sphere::IcoSphere::default(),
    );
    let me = main.meshes.insert("Icosphere", mesh, log);
    let ico = main
        .objects
        .insert("Icosphere", Object::new("", Some(me.any())), log);
    if let Some(c) = main.collections.get_mut(coll, log) {
        c.objects.push(ico);
    }
    Some(ico)
}

// Ported from blender/source/blender/blenlib/intern/math_vector.cc (angle_v3v3,
// angle_normalized_v3v3, project_v3_v3v3_normalized).
fn angle_normalized(v1: Vec3, v2: Vec3) -> f32 {
    let safe_asin = |a: f32| {
        if a.abs() <= 1.0 {
            a.asin()
        } else {
            a.signum() * std::f32::consts::FRAC_PI_2
        }
    };
    if v1.dot(v2) >= 0.0 {
        2.0 * safe_asin((v1 - v2).length() / 2.0)
    } else {
        std::f32::consts::PI - 2.0 * safe_asin((v1 + v2).length() / 2.0)
    }
}

fn normalize(v: Vec3) -> (Vec3, f32) {
    frap_scene::edit_armature::math::normalize(v)
}

/// `ED_armature_ebone_roll_to_vector(bone, align_axis, false)`: the roll that points the
/// bone's Z axis along `align_axis` (unchanged for a zero-length bone or one along the axis).
pub(crate) fn roll_to_vector(head: Vec3, tail: Vec3, roll: f32, align_axis: Vec3) -> f32 {
    let (nor, len) = normalize(tail - head);
    if len <= f32::EPSILON || align_axis.dot(nor).abs() >= 1.0 - f32::EPSILON {
        return roll;
    }
    let mat = vec_roll_to_mat3_normalized(nor, 0.0);
    let vec = nor * align_axis.dot(nor);
    let proj = align_axis - vec;
    let (a, _) = normalize(proj);
    let (b, _) = normalize(mat.z_axis);
    let mut r = angle_normalized(a, b);
    if mat.z_axis.cross(proj).dot(nor) < 0.0 {
        r = -r;
    }
    r
}

/// `rna_EditBone_length_set`: the tail moved along the bone to `length` (+Z for a zero-length
/// bone).
fn set_length(head: Vec3, tail: Vec3, length: f32) -> Vec3 {
    let (mut delta, l) = normalize(tail - head);
    if l == 0.0 {
        delta = Vec3::Z;
    }
    head + delta * length
}

/// `Matrix @ Vector` of a 4×4 `mathutils.Matrix` and a 3D vector (`w = 1`, double sums).
fn mat_vec(m: &Mat4, v: Vec3) -> Vec3 {
    let c = m.to_cols_array_2d();
    let v4 = [v.x, v.y, v.z, 1.0];
    Vec3::from_array(std::array::from_fn(|r| {
        let mut dot = 0.0f64;
        for (k, col) in c.iter().enumerate() {
            dot += f64::from(col[r] * v4[k]);
        }
        dot as f32
    }))
}

/// `create_bones`: the edit bones of armature vnode `arma` (head/tail from
/// `editbone_arma_mat`, the heuristic's length, the roll aligned to the bone's Z axis,
/// parents), then the pose that gives each bone its glTF TRS, and the bone shapes of the
/// `BLENDER` heuristic.
pub(crate) fn create_bones(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    arma: usize,
    bone_shape: Option<IdRef<ObjectId>>,
) {
    let Some(ob) = cx.vtree.nodes[arma].blender_object else {
        return;
    };
    let Some(arm_ref) = main
        .objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<frap_rna::ArmatureId>())
    else {
        return;
    };
    let heuristic = cx.settings.bone_heuristic;
    let max_scale = main
        .objects
        .get(ob)
        .map(|o| o.scale[0].max(o.scale[1]).max(o.scale[2]))
        .unwrap_or(1.0);
    let bone_ids = cx.vtree.bones_of(arma);
    let mut ea = EditArmature::default();
    let mut index_of: Vec<(usize, usize)> = Vec::with_capacity(bone_ids.len());
    for &id in &bone_ids {
        let v = &cx.vtree.nodes[id];
        let name = v.name.clone().unwrap_or_else(|| v.default_name.clone());
        let i = ea.add_bone(&name);
        let m = v.editbone_arma_mat;
        let head = mat_vec(&m, Vec3::ZERO);
        let tail = mat_vec(&m, Vec3::Y);
        let length = if heuristic == BoneHeuristic::Blender {
            (v.bone_length / f64::from(max_scale)) as f32
        } else {
            v.bone_length as f32
        };
        let tail = set_length(head, tail, length);
        let (axis, _) = normalize(mat_vec(&m, Vec3::Z) - head);
        let b = &mut ea.edit_bones[i];
        b.head = head.to_array();
        b.tail = tail.to_array();
        b.use_connect = false;
        b.roll = roll_to_vector(head, tail, b.roll, axis);
        let bone_name = b.name.clone();
        cx.vtree.nodes[id].blender_bone_name = Some(bone_name);
        index_of.push((id, i));
    }
    for &(id, i) in &index_of {
        if let Some(p) = cx.vtree.nodes[id].parent
            && cx.vtree.nodes[p].kind == VKind::Bone
            && let Some(&(_, pi)) = index_of.iter().find(|(v, _)| *v == p)
        {
            ea.edit_bones[i].parent = Some(pi);
        }
    }
    // `mode_set(mode='OBJECT')`: the edit bones become the armature's bones.
    let arm_clone = match main.armatures.get_mut(arm_ref, log) {
        Some(arm) => {
            ea.apply_to(arm);
            arm.clone()
        }
        None => return,
    };
    let Some(o) = main.objects.get_mut(ob, log) else {
        return;
    };
    Pose::rebuild(&mut o.pose, &arm_clone);
    let pose = o.pose.get_or_insert_with(Pose::default);
    for &(id, _) in &index_of {
        let v = &cx.vtree.nodes[id];
        let Some(name) = v.blender_bone_name.as_deref() else {
            continue;
        };
        let Some(pb) = pose.bone_mut(name) else {
            continue;
        };
        // BoneTRS = EditBone · PoseBone: the pose that gives the bone its TRS.
        let (t, r, s) = v.trs();
        let (et, er) = (v.editbone_trans, v.editbone_rot);
        let er_inv = qconj(er);
        pb.location = qrot(er_inv, [t[0] - et[0], t[1] - et[1], t[2] - et[2]]);
        pb.rotation_mode = "QUATERNION".into();
        pb.rotation_quaternion = qmul(er_inv, r);
        pb.scale = s;
        if heuristic == BoneHeuristic::Blender && !cx.settings.disable_bone_shape {
            pb.custom_shape = bone_shape;
        }
    }
    // `view_layer.objects.active = blender_arma` (for Edit Mode).
    if let Some(s) = main.scenes.get_mut(cx.scene, log)
        && let Some(vl) = s.view_layers.iter_mut().find(|v| v.name == cx.view_layer)
    {
        vl.active = Some(ob);
    }
    for &id in &bone_ids {
        cx.vtree.nodes[id].blender_object = Some(ob);
    }
}

/// `setup_skinning`: an ARMATURE modifier deforming `ob` by `arma_ob`.
pub(crate) fn setup_skinning(
    main: &mut Main,
    log: &mut ChangeLog,
    ob: IdRef<ObjectId>,
    arma_ob: IdRef<ObjectId>,
) {
    let mut settings = ModifierSettings::new(ModifierType::Armature);
    if let ModifierSettings::Armature(a) = &mut settings {
        a.object = Some(arma_ob.any());
    }
    let Some(o) = main.objects.get_mut(ob, log) else {
        return;
    };
    let uid = o
        .modifiers
        .iter()
        .map(|m| m.persistent_uid)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    for m in o.modifiers.iter_mut() {
        m.is_active = false;
    }
    o.modifiers.push(Modifier {
        name: "Armature".into(),
        show_viewport: true,
        show_render: true,
        show_in_editmode: false,
        show_on_cage: false,
        show_expanded: true,
        is_active: true,
        use_pin_to_last: false,
        persistent_uid: uid,
        settings,
    });
}

/// `skin_into_bind_pose`: each vertex (and shape key) moved by its weighted joint matrices
/// (`bind_arma_mat @ inverse_bind`) into the armature's bind pose; normals rotated and
/// normalised. A vertex whose weights sum to 0 is given to its first joint (weight 1).
pub(crate) fn skin_into_bind_pose(
    joint_mats: &[[[f32; 4]; 4]],
    vert_joints: &[Vec<[u32; 4]>],
    vert_weights: &mut [Vec<[f32; 4]>],
    locs: &mut [&mut Vec<[f32; 3]>],
    normals: &mut [[f32; 3]],
) {
    let n = locs.first().map_or(0, |l| l.len());
    let jm = |j: u32| joint_mats.get(j as usize).copied().unwrap_or([[0.0; 4]; 4]);
    let accumulate = |v: usize, ws: &[Vec<[f32; 4]>]| {
        let mut m = [[0.0f32; 4]; 4];
        let mut sum = 0.0f32;
        for (js, ws) in vert_joints.iter().zip(ws) {
            for i in 0..4 {
                let w = ws[v][i];
                let j = jm(js[v][i]);
                for (r, row) in m.iter_mut().enumerate() {
                    for (c, x) in row.iter_mut().enumerate() {
                        *x += w * j[r][c];
                    }
                }
                sum += w;
            }
        }
        (m, sum)
    };
    let mut mats: Vec<[[f32; 4]; 4]> = Vec::with_capacity(n);
    for v in 0..n {
        let (mut m, mut sum) = accumulate(v, vert_weights);
        if sum == 0.0 && !vert_weights.is_empty() {
            vert_weights[0][v][0] = 1.0;
            (m, sum) = accumulate(v, vert_weights);
        }
        for row in m.iter_mut() {
            for x in row.iter_mut() {
                *x /= sum;
            }
        }
        mats.push(m);
    }
    let mul3 = |m: &[[f32; 4]; 4], p: [f32; 3]| -> [f32; 3] {
        std::array::from_fn(|r| m[r][0] * p[0] + m[r][1] * p[1] + m[r][2] * p[2])
    };
    for l in locs.iter_mut() {
        for (p, m) in l.iter_mut().zip(&mats) {
            let q = mul3(m, *p);
            *p = [q[0] + m[0][3], q[1] + m[1][3], q[2] + m[2][3]];
        }
    }
    if !normals.is_empty() {
        for (nrm, m) in normals.iter_mut().zip(&mats) {
            let q = mul3(m, *nrm);
            let len = (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt();
            *nrm = if len != 0.0 { q.map(|x| x / len) } else { q };
        }
    }
}

/// The joint matrices of skin `skin` (`bind_mat @ inv_bind`, rows): numpy's float32 copy of
/// the mathutils products.
pub(crate) fn joint_matrices(cx: &Gltf, skin: usize) -> Result<Vec<[[f32; 4]; 4]>, ImportError> {
    let pyskin = &cx.doc.skins[skin];
    let ibms = inverse_bind_matrices(cx)?;
    let inv: Vec<Mat4> = match ibms.get(skin).cloned().flatten() {
        Some(m) => m,
        None => vec![Mat4::IDENTITY; pyskin.joints.len()],
    };
    Ok(pyskin
        .joints
        .iter()
        .zip(inv.iter().chain(std::iter::repeat(&Mat4::IDENTITY)))
        .map(|(&j, ib)| {
            let bind = cx
                .vtree
                .nodes
                .get(j as usize)
                .map(|v| v.bind_arma_mat)
                .unwrap_or(Mat4::IDENTITY);
            let m = crate::export::vtree::matmul(&bind, ib).to_cols_array_2d();
            std::array::from_fn(|r| std::array::from_fn(|c| m[c][r]))
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roll_aligns_the_z_axis() {
        // A bone along +Y with its Z axis wanted along +X: roll -90°.
        let r = roll_to_vector(Vec3::ZERO, Vec3::Y, 0.0, Vec3::X);
        assert!((r.abs() - std::f32::consts::FRAC_PI_2).abs() < 1e-6, "{r}");
        let m = vec_roll_to_mat3_normalized(Vec3::Y, r);
        assert!((m.z_axis - Vec3::X).length() < 1e-6, "{m:?}");
        // Already aligned, zero-length, along the axis: unchanged.
        assert_eq!(roll_to_vector(Vec3::ZERO, Vec3::Y, 0.3, Vec3::Y), 0.3);
        assert_eq!(roll_to_vector(Vec3::ZERO, Vec3::ZERO, 0.3, Vec3::X), 0.3);
    }

    #[test]
    fn edit_bone_length_moves_the_tail() {
        let t = set_length(Vec3::ONE, Vec3::new(1.0, 3.0, 1.0), 0.5);
        assert_eq!(t, Vec3::new(1.0, 1.5, 1.0));
        assert_eq!(
            set_length(Vec3::ZERO, Vec3::ZERO, 2.0),
            Vec3::new(0.0, 0.0, 2.0)
        );
    }

    #[test]
    fn bind_pose_skinning() {
        // Two joints: identity and a translation by +1 in X; a vertex half on each.
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let mut tx = id;
        tx[0][3] = 1.0;
        let joints = vec![vec![[0, 1, 0, 0], [0, 0, 0, 0]]];
        let mut weights = vec![vec![[0.5, 0.5, 0.0, 0.0], [0.0; 4]]];
        let mut locs = vec![[0.0f32; 3], [2.0, 0.0, 0.0]];
        let mut normals = vec![[0.0, 0.0, 2.0], [0.0, 1.0, 0.0]];
        skin_into_bind_pose(
            &[id, tx],
            &joints,
            &mut weights,
            &mut [&mut locs],
            &mut normals,
        );
        assert_eq!(locs[0], [0.5, 0.0, 0.0]);
        // No weight: the first joint (identity) takes it.
        assert_eq!(locs[1], [2.0, 0.0, 0.0]);
        assert_eq!(weights[0][1][0], 1.0);
        assert_eq!(normals[0], [0.0, 0.0, 1.0]);
    }
}
