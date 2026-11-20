// SPDX-License-Identifier: GPL-3.0-or-later
//! Object matrices as Blender's object operators see them. Ports of `BKE_object_apply_mat4_ex`,
//! `BKE_object_apply_parent_inverse` (`blenkernel/intern/object.cc`) and `ignore_parent_tx`
//! (`editors/object/object_transform.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! **Stale world matrices.** An operator reads `ob->object_to_world()`, the matrix of the last
//! depsgraph evaluation, which does not change while the operator runs. frap computes world
//! matrices fresh (D-04), so operators that read world matrices after changing other objects
//! take a [`Worlds`] snapshot first: it is the state Blender's operator sees (verified, e.g.
//! `parent_set(keep_transform=True)` of a parent and its child keeps the child's world matrix
//! from before the operator).
//!
//! Bone children (`parent_type = 'BONE'`) see their parent through the posed bone, as
//! `BKE_object_get_parent_matrix` does: [`Worlds::of`] and [`Worlds::parent_matrix`] read
//! `world_matrix_in` / `parent_matrix_in` (WP 6.5).

use std::collections::BTreeMap;

use frap_math::matrix::ObjectTransform;
use frap_rna::{IdRef, ObjectId};
use frap_scene::id::Object;
use frap_scene::id::transform::{parent_matrix_in, world_matrix_in};
use frap_scene::{ChangeLog, Main};
use glam::{Mat3, Mat4};

/// World matrices of every object, as evaluated before the operator ran.
#[derive(Clone, Debug, Default)]
pub struct Worlds(BTreeMap<IdRef<ObjectId>, Mat4>);

impl Worlds {
    /// Snapshot of every object of `main`.
    pub fn of(main: &Main) -> Worlds {
        Worlds(
            main.objects
                .iter()
                .map(|(r, _)| (r, world_matrix_in(main, r)))
                .collect(),
        )
    }
    /// The evaluated world matrix of `ob` (identity for an unknown object).
    pub fn get(&self, ob: IdRef<ObjectId>) -> Mat4 {
        self.0.get(&ob).copied().unwrap_or(Mat4::IDENTITY)
    }
    /// `BKE_object_get_parent_matrix` of `o` from the snapshot: its parent's world matrix,
    /// times the posed bone for `parent_type = 'BONE'` (`None` without an existing parent).
    pub fn parent_matrix(&self, main: &Main, o: &Object) -> Option<Mat4> {
        let p = o.parent?;
        let po = main.objects.get(p)?;
        Some(parent_matrix_in(main, o, po, self.get(p)))
    }
    /// Records a newly evaluated matrix (`BKE_object_where_is_calc` on the evaluated copy).
    pub fn set(&mut self, ob: IdRef<ObjectId>, m: Mat4) {
        self.0.insert(ob, m);
    }
}

/// `BKE_object_apply_mat4_ex(ob, mat, parent, parentinv, use_compat)`: decomposes `mat` into
/// the transform channels (shear is lost), removing the parent's effect when `parent_world` is
/// given (`parent_world · matrix_parent_inverse` is undone first) and the delta channels.
pub fn apply_mat4(o: &mut Object, mat: &Mat4, use_compat: bool, parent_world: Option<&Mat4>) {
    let local = match parent_world {
        Some(pw) => (*pw * o.matrix_parent_inverse).inverse() * *mat,
        None => *mat,
    };
    let mut t: ObjectTransform = o.transform();
    t.apply_mat4(&local, use_compat);
    o.set_transform(&t);
}

/// `BKE_object_where_is_calc` of an object given its parent's world matrix: `parent_world ·
/// matrix_parent_inverse · matrix_basis`, or `matrix_basis` without a parent.
pub fn where_is(o: &Object, parent_world: Option<&Mat4>) -> Mat4 {
    match (o.parent, parent_world) {
        (Some(_), Some(pw)) => *pw * o.matrix_parent_inverse * o.matrix_basis(),
        _ => o.matrix_basis(),
    }
}

/// `ignore_parent_tx`: after `parent` changed (its new evaluated world matrix is
/// `parent_world`), every child is re-expressed so that its world matrix stays the one of
/// `worlds`: the child's channels take the decomposition of its world matrix (compatible Euler)
/// and its `matrix_parent_inverse` becomes the inverse of its parent matrix (`parent_world`,
/// times the posed bone for a bone child). The children's new
/// matrices are recorded in `worlds` (unchanged by construction). Children are visited in
/// `bpy.data.objects` order.
pub fn ignore_parent_tx(
    main: &mut Main,
    log: &mut ChangeLog,
    parent: IdRef<ObjectId>,
    parent_world: &Mat4,
    worlds: &Worlds,
) {
    let children: Vec<IdRef<ObjectId>> = main
        .objects
        .iter()
        .filter(|(_, o)| o.parent == Some(parent))
        .map(|(r, _)| r)
        .collect();
    for c in children {
        let w = worlds.get(c);
        // `BKE_object_calc_parent`: the parent matrix of this child (the posed bone of a
        // `parent_type = 'BONE'` child).
        let inv = match (main.objects.get(c), main.objects.get(parent)) {
            (Some(o), Some(po)) => parent_matrix_in(main, o, po, *parent_world).inverse(),
            _ => parent_world.inverse(),
        };
        if let Some(o) = main.objects.get_mut(c, log) {
            apply_mat4(o, &w, true, None);
            o.matrix_parent_inverse = inv;
        }
    }
}

/// `orthogonalize_m4_stable(R, 0, true)` on the 3×3 part of `m` (translation kept).
fn orthogonalize_stable_x(m: &Mat4) -> Mat4 {
    let (mut v1, mut v2, mut v3) = (
        m.x_axis.truncate(),
        m.y_axis.truncate(),
        m.z_axis.truncate(),
    );
    let len_sq_v1 = v1.length_squared();
    if len_sq_v1 > 0.0 {
        v2 += v1 * (-v2.dot(v1) / len_sq_v1);
        v3 += v1 * (-v3.dot(v1) / len_sq_v1);
        v1 *= 1.0 / len_sq_v1.sqrt();
    }
    let len_sq_v2 = v2.length_squared();
    if len_sq_v2 > 0.0 {
        v3 += v2 * (-v3.dot(v2) / len_sq_v2);
        v2 *= 1.0 / len_sq_v2.sqrt();
    }
    let n = frap_math::matrix::normalize_v3(v3.to_array()).0;
    let mut out = *m;
    out.x_axis = v1.extend(m.x_axis.w);
    out.y_axis = v2.extend(m.y_axis.w);
    out.z_axis = glam::Vec3::from_array(n).extend(m.z_axis.w);
    out
}

/// `BKE_object_apply_parent_inverse`: the parent's orthonormalized world transform becomes the
/// child's origin (`matrix_parent_inverse = parent⁻¹ · orthonormal(parent)`) and the channels
/// are recomputed so that the world matrix `world` stays (no compatible Euler).
pub fn apply_parent_inverse(o: &mut Object, parent_world: &Mat4, world: &Mat4) {
    let par_imat = parent_world.inverse();
    let par_locrot = orthogonalize_stable_x(parent_world);
    o.matrix_parent_inverse = par_imat * par_locrot;
    let ob_local = o.matrix_parent_inverse.inverse() * par_imat * *world;
    apply_mat4(o, &ob_local, false, None);
}

/// The 3×3 part of a 4×4 matrix.
pub fn mat3(m: &Mat4) -> Mat3 {
    Mat3::from_mat4(*m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn apply_parent_inverse_keeps_world_and_strips_scale() {
        // Parent at (1, 2, 3), turned 0.5 about Z, scaled (2, 1, 1): parent_no_inverse_set
        // (keep_transform=True) gives matrix_parent_inverse = diag(0.5, 1, 1) (verified).
        let pw = Mat4::from_scale_rotation_translation(
            Vec3::new(2.0, 1.0, 1.0),
            glam::Quat::from_rotation_z(0.5),
            Vec3::new(1.0, 2.0, 3.0),
        );
        let world = Mat4::from_translation(Vec3::new(4.0, 1.0, 6.0));
        let mut o = Object::new("Child", None);
        o.parent = Some(IdRef::from_key(frap_rna::IdKey::next()));
        apply_parent_inverse(&mut o, &pw, &world);
        let want = Mat4::from_scale(Vec3::new(0.5, 1.0, 1.0));
        assert!(
            o.matrix_parent_inverse.abs_diff_eq(want, 1e-5),
            "{:?}",
            o.matrix_parent_inverse
        );
        let w = where_is(&o, Some(&pw));
        assert!(w.abs_diff_eq(world, 1e-5));
    }

    #[test]
    fn apply_mat4_through_a_parent() {
        let pw = Mat4::from_translation(Vec3::new(1.0, 0.0, 0.0));
        let mut o = Object::new("C", None);
        o.parent = Some(IdRef::from_key(frap_rna::IdKey::next()));
        apply_mat4(
            &mut o,
            &Mat4::from_translation(Vec3::new(3.0, 0.0, 0.0)),
            false,
            Some(&pw),
        );
        assert_eq!(o.location, [2.0, 0.0, 0.0]);
    }
}
