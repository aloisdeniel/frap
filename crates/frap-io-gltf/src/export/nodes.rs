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
//!
//! Armatures (WP 6.8, `__gather_children`, `gather_skin`): an armature's children are its object
//! children, then its root bones as joints (`export/skins.rs`); a skinned node has a `skin` and
//! no TRS, and a child of a skinned node is placed relative to the skinned node's armature (or
//! in world space when the skinned node is not the armature's child), since glTF ignores a
//! skinned node's transform. With `export_armature_object_remove` bone children of any node
//! are joints. An object with an ARMATURE modifier exports a mesh of its own.

use frap_gltf::json::{Index, Node};
use frap_math::glam::Mat4;
use frap_math::matrix::decompose_wxyz;
use frap_math::quat::normalize_qt;
use frap_scene::id::Object;

use crate::export::gather::Gather;
use crate::export::mesh::gather::{gather_mesh, gather_mesh_with};
use crate::export::mesh::post::morph_targets;
use crate::export::vtree::{VTree, VType, inverted_safe, matmul};
use crate::export::{ExportError, cameras, extras, lights, skins};

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

/// Whether the mesh of `ob` will have primitives (a skinned node without a mesh gets no skin,
/// and its children are placed as for an unskinned parent).
fn mesh_has_primitives(
    g: &Gather,
    ob_ref: frap_rna::IdRef<frap_rna::ObjectId>,
    ob: &Object,
) -> bool {
    let mesh = match crate::export::apply::evaluated(g, ob_ref) {
        Some(m) => m,
        None => match ob
            .data
            .and_then(|d| d.typed::<frap_rna::MeshId>())
            .and_then(|m| g.main.meshes.get_arc(m))
        {
            Some(m) => m,
            None => return false,
        },
    };
    let p = &g.settings.props;
    mesh.faces_num() > 0
        || (p.use_mesh_edges.value && mesh.edges_num() > 0)
        || (p.use_mesh_vertices.value && mesh.verts_num() > 0)
}

/// `export_settings['gltf_collection_center']`, computed once per exported scene from its
/// filtered tree (`gather_scene` calls `calculate_collection_center` after `vtree.filter()`).
#[derive(Default)]
struct CollectionCenter(
    std::collections::HashMap<frap_rna::IdRef<frap_rna::SceneId>, frap_math::glam::Vec3>,
);

impl CollectionCenter {
    fn get(
        &mut self,
        scene: frap_rna::IdRef<frap_rna::SceneId>,
        tree: &VTree,
    ) -> frap_math::glam::Vec3 {
        *self
            .0
            .entry(scene)
            .or_insert_with(|| tree.collection_center())
    }
}

/// `__gather_trans_rot_scale`'s local matrix: relative to the kept parent, except that the
/// parent's own transform is skipped when the parent is skinned (glTF ignores it): then
/// relative to the skinned parent's armature when that parent is the armature's child, else
/// the world matrix.
fn node_local_matrix(g: &mut Gather, tree: &VTree, i: usize) -> Mat4 {
    let n = &tree.nodes[i];
    let Some(p) = n.parent else {
        // `at_collection_center` with `collection` (WP 8.12): a root moves by minus the
        // collection centre (`trans -= gltf_collection_center` after the decomposition, which
        // is the matrix's translation column).
        let mut m = n.matrix_world;
        if !g.settings.collection.is_empty() && g.settings.props.at_collection_center.value {
            let scene = g.scene;
            let c = g.state::<CollectionCenter>().get(scene, tree);
            m.w_axis -= c.extend(0.0);
        }
        return m;
    };
    if !skins::is_skinned(g, p) {
        return tree.local_matrix(i);
    }
    let pn = &tree.nodes[p];
    match (pn.parent, pn.armature) {
        (Some(pp), Some(arm)) if tree.nodes[pp].kind == VType::Armature => matmul(
            &inverted_safe(&tree.nodes[arm].matrix_world),
            &n.matrix_world,
        ),
        _ => n.matrix_world,
    }
}

/// Gathers vtree node `i` and its kept subtree; returns the node's index. A bone node (a root
/// of a scene whose armature object was removed, `export_armature_object_remove`) becomes a
/// joint.
pub fn gather_node(g: &mut Gather, tree: &VTree, i: usize) -> Result<Index, ExportError> {
    if tree.nodes[i].kind == VType::Bone {
        return skins::gather_joint(g, tree, i);
    }
    let r = tree.nodes[i].object;
    let main = g.main;
    let ob = main
        .objects
        .get(r)
        .ok_or_else(|| ExportError::Invalid("vtree object vanished".into()))?;
    // `gather_skin` first (the children's TRS depend on it); no skin without a mesh.
    let skin_arm = skins::node_skin_armature(g, tree, i)
        .filter(|_| ob.object_type() == "MESH" && mesh_has_primitives(g, r, ob));
    if skin_arm.is_some() {
        skins::set_skinned(g, i);
    }
    // Alphabetical members: `camera`, `children` (post-order), `extensions` (lights), `extras`,
    // then `mesh`, then `skin`, then the node itself.
    let camera = cameras::gather(g, ob)?;
    let mut children = Vec::with_capacity(tree.nodes[i].children.len());
    for &c in &tree.nodes[i].children {
        match tree.nodes[c].kind {
            VType::Bone if tree.armature_object_remove => {
                children.push(skins::gather_joint(g, tree, c)?)
            }
            VType::Bone => {}
            _ => children.push(gather_node(g, tree, c)?),
        }
    }
    if tree.nodes[i].kind == VType::Armature {
        for &b in tree.root_bones(i) {
            children.push(skins::gather_joint(g, tree, b)?);
        }
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
        // With `export_skins`, an object with an ARMATURE modifier exports a mesh of its own,
        // in world space when it is skinned (WP 6.8).
        _ if skins::has_armature_modifier(g, ob) => {
            let skin = skins::mesh_skin(g, tree, i);
            gather_mesh_with(g, ob, Some((r, skin)))?
        }
        _ => gather_mesh(g, ob)?,
    };
    if let Some(m) = node.mesh {
        morph_targets::set_mesh_weights(g, r, ob, m);
    }
    if let (Some(arm), Some(_)) = (skin_arm, node.mesh) {
        node.skin = skins::gather_skin(g, tree, arm)?;
    }
    if node.skin.is_none() {
        let (translation, rotation, scale) =
            trs(&node_local_matrix(g, tree, i), g.settings.export_yup);
        node.rotation = rotation;
        node.scale = scale;
        node.translation = translation;
    }
    // M7 W0 hook: the dequantization transform of a quantized mesh (WP 7.1).
    if let Some(m) = node.mesh {
        crate::export::compress::mesh_node(g, m, &mut node)?;
    }
    if tree.nodes[i].kind == VType::Armature
        && let Some(n) = skins::take_pending_neutral(g, i)
    {
        node.children.push(n);
    }
    g.doc.nodes.push(node);
    let index = (g.doc.nodes.len() - 1) as Index;
    // The object → node map the M6 exporters resolve channel targets with (M5+M6 W0).
    let scene = g.scene;
    g.state::<crate::export::node_map::NodeMap>()
        .objects
        .insert((scene, r), index);
    Ok(index)
}

/// End of a scene's gather (`__gather_scene` after its roots, then `get_unused_skins` of the
/// last scene): the skins of armatures no kept object is skinned by.
pub fn finish_scene(g: &mut Gather, tree: &VTree) -> Result<(), ExportError> {
    skins::gather_unused_skins(g, tree)
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
