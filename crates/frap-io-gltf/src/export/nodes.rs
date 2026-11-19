// SPDX-License-Identifier: GPL-3.0-or-later
//! Nodes (ARCHITECTURE §17.2): one glTF node per kept vtree node, appended **post-order**
//! (children before their parent, as the add-on's traversal does), with the TRS of the node
//! relative to its kept parent.
//!
//! Ported from blender/scripts/addons_core/io_scene_gltf2/blender/exp/nodes.py
//! (`gather_node`, `__gather_trans_rot_scale`, `__convert_swizzle_*`) and
//! blender/com/gltf2_blender_math.py (`round_if_near`, threshold `2e-6`):
//! - local = `parent.matrix_world.inverted_safe() @ matrix_world` (the world matrix for a root),
//!   decomposed like `Matrix.decompose()` (shear is lost), the rotation normalised;
//! - Y-up: translation `(x, z, -y)`, rotation `(w, x, z, -y)` written `[x, y, z, w]`, scale
//!   `(x, z, y)`;
//! - each component within `2e-6` of the identity value snaps to it, and a component equal to
//!   the identity is omitted. `matrix` is never written.

use frap_gltf::json::{Index, Node};
use frap_math::glam::Mat4;
use frap_math::matrix::decompose_wxyz;
use frap_math::quat::normalize_qt;

use crate::export::gather::Gather;
use crate::export::mesh::gather::gather_mesh;
use crate::export::vtree::VTree;
use crate::export::{ExportError, cameras, extras, lights};

/// `round_if_near(value, target)`.
pub fn round_if_near(value: f32, target: f32) -> f32 {
    if (value - target).abs() > 2.0e-6 {
        value
    } else {
        target
    }
}

/// glTF TRS of a local matrix, each `None` when it equals the identity (omitted).
#[allow(clippy::type_complexity)]
pub fn trs(m: &Mat4, export_yup: bool) -> (Option<[f64; 3]>, Option<[f64; 4]>, Option<[f64; 3]>) {
    let (loc, quat, size) = decompose_wxyz(m);
    let mut q = normalize_qt(quat).0;
    if q == [0.0; 4] {
        q = [1.0, 0.0, 0.0, 0.0];
    }
    let (t, r, s) = if export_yup {
        (
            [loc.x, loc.z, -loc.y],
            [q[0], q[1], q[3], -q[2]],
            [size.x, size.z, size.y],
        )
    } else {
        ([loc.x, loc.y, loc.z], q, [size.x, size.y, size.z])
    };
    let t = t.map(|c| round_if_near(c, 0.0));
    let r = [
        round_if_near(r[0], 1.0),
        round_if_near(r[1], 0.0),
        round_if_near(r[2], 0.0),
        round_if_near(r[3], 0.0),
    ];
    let s = s.map(|c| round_if_near(c, 1.0));
    let f = |x: f32| f64::from(x);
    let translation = t.iter().any(|c| *c != 0.0).then(|| t.map(f));
    let rotation = (r[0] != 1.0 || r[1] != 0.0 || r[2] != 0.0 || r[3] != 0.0)
        .then(|| [f(r[1]), f(r[2]), f(r[3]), f(r[0])]);
    let scale = s.iter().any(|c| *c != 1.0).then(|| s.map(f));
    (translation, rotation, scale)
}

/// Gathers vtree node `i` and its kept subtree; returns the node's index.
pub fn gather_node(g: &mut Gather, tree: &VTree, i: usize) -> Result<Index, ExportError> {
    let r = tree.nodes[i].object;
    let main = g.main;
    let ob = main
        .objects
        .get(r)
        .ok_or_else(|| ExportError::Invalid("vtree object vanished".into()))?;
    // Alphabetical members: `camera`, `children` (post-order), `extensions` (lights), `extras`,
    // then `mesh`, then the node itself.
    let camera = cameras::gather(g, ob)?;
    let mut children = Vec::with_capacity(tree.nodes[i].children.len());
    for &c in &tree.nodes[i].children {
        children.push(gather_node(g, tree, c)?);
    }
    let mut node = Node {
        camera,
        children,
        name: Some(ob.header.name.to_string()),
        ..Node::default()
    };
    lights::apply(g, ob, &mut node)?;
    node.extras = extras::object(g, ob);
    node.mesh = match ob.object_type() {
        // Point clouds become POINTS meshes (WP 3.7, `mesh/extract/pointcloud.rs`).
        "POINTCLOUD" => crate::export::mesh::extract::pointcloud::gather(g, ob)?,
        _ => gather_mesh(g, ob)?,
    };
    let (translation, rotation, scale) = trs(&tree.local_matrix(i), g.settings.export_yup);
    node.rotation = rotation;
    node.scale = scale;
    node.translation = translation;
    g.doc.nodes.push(node);
    Ok((g.doc.nodes.len() - 1) as Index)
}

#[cfg(test)]
mod tests {
    use frap_math::glam::{Quat, Vec3};

    use super::*;

    #[test]
    fn identity_is_omitted() {
        assert_eq!(trs(&Mat4::IDENTITY, true), (None, None, None));
        // Within 2e-6 of the identity snaps.
        let m = Mat4::from_translation(Vec3::new(1e-6, -1.5e-6, 0.0));
        assert_eq!(trs(&m, true).0, None);
    }

    #[test]
    fn yup_swizzle() {
        let m = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::IDENTITY,
            Vec3::new(1.0, 2.0, 3.0),
        );
        let (t, r, s) = trs(&m, true);
        assert_eq!(t, Some([1.0, 3.0, -2.0]));
        assert_eq!(r, None);
        assert_eq!(s, Some([1.0, 3.0, 2.0]));
        let (t, _, s) = trs(&m, false);
        assert_eq!(t, Some([1.0, 2.0, 3.0]));
        assert_eq!(s, Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn rotation_about_blender_z_is_gltf_y() {
        let a = std::f32::consts::FRAC_PI_2;
        let m = Mat4::from_rotation_z(a);
        let (_, r, _) = trs(&m, true);
        let r = r.unwrap();
        // Blender quaternion (w, 0, 0, z) → glTF xyzw (0, z, 0, w).
        assert!(r[0].abs() < 1e-7 && r[2].abs() < 1e-7);
        assert!((r[1] - f64::from((a / 2.0).sin())).abs() < 1e-6);
        assert!((r[3] - f64::from((a / 2.0).cos())).abs() < 1e-6);
    }

    #[test]
    fn negative_scale_keeps_rotation_proper() {
        let m = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        let (_, r, s) = trs(&m, true);
        // Blender's mat3_to_rot_size negates every axis for a mirrored matrix.
        assert_eq!(s, Some([-1.0, -1.0, -1.0]));
        assert!(r.is_some());
    }
}
