// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF skins (WP 6.8): armatures as joint hierarchies (`export_def_bones` deform-bone filter,
//! `export_hierarchy_flatten_bones`, `export_armature_object_remove`, `export_leaf_bone`,
//! `export_rest_position_armature`), inverse bind matrices, the neutral bone of unweighted
//! vertices, and what the skinned mesh's writers need (`export/mesh/writers/skin.rs`: the
//! vertex-group ↔ joint mapping, `export_influence_nb`, `export_all_influences`, the
//! armature-space transforms). Joint nodes are recorded in `export::node_map::NodeMap::bones`.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors)
//! `blender/exp/skins.py` (`gather_skin`, `__gather_inverse_bind_matrices`,
//! `__gather_joints`), `blender/exp/joints.py` (`gather_joint_vnode`), `blender/exp/nodes.py`
//! (`gather_skin`, `get_objects_parented_to_bones`), `blender/exp/tree.py`
//! (`add_neutral_bones`, `get_unused_skins`), `blender/exp/primitive_extract.py`
//! (`prepare_data`, `__get_bone_data`, the skinning transforms of `__get_positions`,
//! `__get_normals`, `__get_tangents`) and `blender/exp/primitive_attributes.py`
//! (`__gather_skins`).
//!
//! The add-on gathers Python objects first and assigns array indices while traversing them;
//! frap appends as it gathers, in the traversal's order: a skinned node's children, then its
//! mesh, then its skin (the inverse bind matrices, then the joints, each joint after its child
//! joints and the objects parented to its bone). A skin is shared by every mesh of its
//! armature node (`@cached` by armature). The neutral bone a mesh with unweighted vertices
//! needs is decided when the skin is gathered, from the meshes the armature's skinned nodes
//! will export (the add-on decides it while extracting them, before any array is written).

use std::collections::{HashMap, HashSet};

use frap_gltf::json::{AccessorType, Index, Node, Skin};
use frap_gltf::{AccessorData, AccessorOpts};
use frap_math::glam::{Mat3, Mat4};
use frap_math::matrix::decompose_wxyz;
use frap_mesh::{DeformVert, Mesh};
use frap_rna::{IdRef, MeshId, ObjectId, SceneId};
use frap_scene::id::Object;
use smallvec::SmallVec;
use smol_str::SmolStr;

use crate::export::gather::Gather;
use crate::export::vtree::{
    BoneSettings, VTree, VType, armature_modifier_object, armature_of, axis_basis_change,
    inverted_safe, matmul, matrix_world,
};
use crate::export::{ExportError, apply};

/// The skin and armature options are honoured here (WP 6.8).
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_skins",
    "export_def_bones",
    "export_hierarchy_flatten_bones",
    "export_armature_object_remove",
    "export_leaf_bone",
    "export_rest_position_armature",
    "export_influence_nb",
    "export_all_influences",
];

/// `min_influence` of `__get_bone_data`: weights at or below it are not exported.
pub const MIN_INFLUENCE: f64 = 0.0001;

/// Name of the joint unweighted vertices are bound to.
pub const NEUTRAL_BONE: &str = "neutral_bone";

/// A written skin.
#[derive(Clone, Debug)]
struct Written {
    /// `doc.skins` index.
    index: Index,
    /// The neutral bone node, when one was added.
    neutral: Option<Index>,
}

/// Skin state of one export (`Gather::state`).
#[derive(Default)]
pub struct SkinState {
    /// Written skins per (scene, armature node).
    skins: HashMap<(IdRef<SceneId>, usize), Option<Written>>,
    /// Joint nodes per (scene, bone node).
    joints: HashMap<(IdRef<SceneId>, usize), Index>,
    /// Nodes with a skin (`vnode.skin`) per scene: their children's TRS skip them.
    skinned: HashSet<(IdRef<SceneId>, usize)>,
    /// Armature nodes whose neutral bone still has to be appended to their glTF node.
    pending_neutral: HashMap<(IdRef<SceneId>, usize), Index>,
    /// `check_if_we_can_remove_armature` refused once: no armature is removed for the rest of
    /// the export (the add-on sets `gltf_armature_object_remove = False`).
    pub remove_refused: bool,
}

impl SkinState {
    /// `gltf_armature_object_remove` for the next scene's tree.
    pub fn armature_object_remove(&self, settings: &crate::export::ExportSettings) -> bool {
        !self.remove_refused && BoneSettings::new(settings).armature_object_remove
    }
}

/// What the mesh writers need to skin one object's mesh (`PrimitiveCreator.prepare_data`).
#[derive(Clone, Debug, PartialEq)]
pub struct MeshSkin {
    /// Joint node names, in skin order (`skin.joints`, the neutral bone excluded).
    pub joints: Vec<SmolStr>,
    /// The object's vertex groups (`blender_object.vertex_groups`), by index.
    pub groups: Vec<SmolStr>,
    /// `blender_object.matrix_world`.
    pub object_world: Mat4,
    /// `matrix_world` of the ARMATURE modifier's object.
    pub armature_world: Mat4,
    /// `gltf_vertex_influences_nb`
    pub influence_nb: i32,
    /// `gltf_all_vertex_influences`
    pub all_influences: bool,
}

/// One vertex's `(joint, weight)` pairs, heaviest first.
pub type VertBones = SmallVec<[(u32, f32); 4]>;

impl MeshSkin {
    /// `group_to_joint`: per vertex group, the joint of the same name (the last joint of that
    /// name, as the add-on's dict keeps).
    fn group_to_joint(&self) -> Vec<Option<u32>> {
        let mut by_name: HashMap<&str, u32> = HashMap::new();
        for (i, j) in self.joints.iter().enumerate() {
            by_name.insert(j.as_str(), i as u32);
        }
        self.groups
            .iter()
            .map(|g| by_name.get(g.as_str()).copied())
            .collect()
    }

    /// `__get_bone_data`: per vertex its `(joint, weight)` pairs above [`MIN_INFLUENCE`] whose
    /// group names a joint, sorted by weight (heaviest first, ties in group order); a vertex
    /// with none gets the neutral bone (`(joints.len(), 1.0)`). Also returns whether the
    /// neutral bone is needed.
    pub fn vert_bones(
        &self,
        verts: usize,
        dverts: Option<&[DeformVert]>,
    ) -> (Vec<VertBones>, bool) {
        let g2j = self.group_to_joint();
        let neutral = self.joints.len() as u32;
        let mut need_neutral = false;
        let out = (0..verts)
            .map(|v| {
                let mut bones: VertBones = SmallVec::new();
                if let Some(dv) = dverts.and_then(|d| d.get(v)) {
                    for &(g, w) in &dv.weights {
                        if f64::from(w) <= MIN_INFLUENCE {
                            continue;
                        }
                        if let Some(Some(j)) = g2j.get(g as usize) {
                            bones.push((*j, w));
                        }
                    }
                }
                // Stable, heaviest first (Python's `sort(key=…, reverse=True)`).
                bones.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                if bones.is_empty() {
                    bones.push((neutral, 1.0));
                    need_neutral = true;
                }
                bones
            })
            .collect();
        (out, need_neutral)
    }

    /// `armature.matrix_world.inverted_safe() @ blender_object.matrix_world`.
    fn apply_matrix(&self) -> Mat4 {
        matmul(&inverted_safe(&self.armature_world), &self.object_world)
    }

    /// The linear part of `__get_normals`' `normal_transform`:
    /// `armature.matrix_world.to_3x3() @ apply_matrix.to_3x3().inverted_safe().transposed()`.
    pub fn normal_transform(&self) -> Mat3 {
        let a = Mat3::from_mat4(self.apply_matrix());
        let inv_t = inverted_safe3(&a).transpose();
        matmul3(&Mat3::from_mat4(self.armature_world), &inv_t)
    }

    /// `__get_tangents`' `apply_matrix.to_quaternion().to_matrix()`.
    pub fn tangent_transform(&self) -> Mat3 {
        let q = frap_math::quat::mat4_to_quat(&self.apply_matrix());
        frap_math::quat::quat_to_mat3(q)
    }
}

// Ported from blender/source/blender/python/mathutils/mathutils_Matrix.cc (Matrix_matmul).
/// `a @ b` of two 3×3 `mathutils.Matrix` (float products accumulated in double).
pub fn matmul3(a: &Mat3, b: &Mat3) -> Mat3 {
    let (a, b) = (a.to_cols_array_2d(), b.to_cols_array_2d());
    let mut r = [[0.0f32; 3]; 3];
    for (j, col) in r.iter_mut().enumerate() {
        for (i, v) in col.iter_mut().enumerate() {
            let mut dot = 0.0f64;
            for (k, ak) in a.iter().enumerate() {
                dot += f64::from(ak[i] * b[j][k]);
            }
            *v = dot as f32;
        }
    }
    Mat3::from_cols_array_2d(&r)
}

// Ported from blender/source/blender/blenlib/intern/math_matrix.cc (determinant_m3_array,
// adjoint_m3_m3) and mathutils_Matrix.cc (matrix_invert_safe_internal).
/// `Matrix.inverted_safe()` of a 3×3 matrix.
pub fn inverted_safe3(m: &Mat3) -> Mat3 {
    const EPS: f32 = 1e-8;
    let mut a = m.to_cols_array_2d();
    let det = |a: &[[f32; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
            - a[1][0] * (a[0][1] * a[2][2] - a[0][2] * a[2][1])
            + a[2][0] * (a[0][1] * a[1][2] - a[0][2] * a[1][1])
    };
    let mut d = det(&a);
    if d == 0.0 {
        for (i, col) in a.iter_mut().enumerate() {
            col[i] += EPS;
        }
        d = det(&a);
        if d == 0.0 {
            d = EPS;
        }
    }
    // adjoint_m3_m3 (on the column-major array, as Blender's m[col][row]).
    let mut r = [[0.0f32; 3]; 3];
    r[0][0] = a[1][1] * a[2][2] - a[1][2] * a[2][1];
    r[0][1] = -a[0][1] * a[2][2] + a[0][2] * a[2][1];
    r[0][2] = a[0][1] * a[1][2] - a[0][2] * a[1][1];
    r[1][0] = -a[1][0] * a[2][2] + a[1][2] * a[2][0];
    r[1][1] = a[0][0] * a[2][2] - a[0][2] * a[2][0];
    r[1][2] = -a[0][0] * a[1][2] + a[0][2] * a[1][0];
    r[2][0] = a[1][0] * a[2][1] - a[1][1] * a[2][0];
    r[2][1] = -a[0][0] * a[2][1] + a[0][1] * a[2][0];
    r[2][2] = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    for col in r.iter_mut() {
        for v in col.iter_mut() {
            *v /= d;
        }
    }
    Mat3::from_cols_array_2d(&r)
}

/// `PrimitiveCreator.apply_mat_to_all(matrix, vectors)` for one vector: numpy multiplies the
/// float32 vector by the float64 matrix (and adds the float64 translation), the result is
/// stored back as float32.
pub fn apply_mat(m: &Mat4, v: [f32; 3]) -> [f32; 3] {
    let c = m.to_cols_array_2d();
    std::array::from_fn(|r| {
        let x = f64::from(v[0]) * f64::from(c[0][r])
            + f64::from(v[1]) * f64::from(c[1][r])
            + f64::from(v[2]) * f64::from(c[2][r]);
        (x + f64::from(c[3][r])) as f32
    })
}

/// [`apply_mat`] of a 3×3 matrix, in float64 (callers round to float32 where the add-on
/// stores into a float32 array).
pub fn apply_mat3_f64(m: &Mat3, v: [f64; 3]) -> [f64; 3] {
    let c = m.to_cols_array_2d();
    std::array::from_fn(|r| {
        v[0] * f64::from(c[0][r]) + v[1] * f64::from(c[1][r]) + v[2] * f64::from(c[2][r])
    })
}

/// `normalize_vecs` of one float32 vector (`np.linalg.norm`, divided where non-zero).
pub fn normalize_f32(v: [f32; 3]) -> [f32; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n != 0.0 { v.map(|x| x / n) } else { v }
}

/// `normalize_vecs` of one float64 vector.
pub fn normalize_f64(v: [f64; 3]) -> [f64; 3] {
    let n = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if n != 0.0 { v.map(|x| x / n) } else { v }
}

/// The skin of node `i`'s mesh (`prepare_data`'s `self.skin`): with `export_skins`, the last
/// ARMATURE modifier's object, unless the object is parented to a bone of it; the skin of the
/// node's armature must exist. `None` also when the object has no ARMATURE modifier.
pub fn mesh_skin(g: &Gather, tree: &VTree, i: usize) -> Option<MeshSkin> {
    let bs = BoneSettings::new(g.settings);
    if !bs.skins {
        return None;
    }
    let main = g.main;
    let ob = main.objects.get(tree.nodes[i].object)?;
    let arm_ob_ref = armature_modifier_object(ob)??;
    let arm_ob = main.objects.get(arm_ob_ref)?;
    if ob.parent_type == "BONE"
        && ob
            .parent
            .and_then(|p| main.objects.get(p))
            .is_some_and(|p| p.header.name == arm_ob.header.name)
    {
        return None;
    }
    let arm = tree.nodes[i].armature?;
    let joints = skin_joint_names(g, tree, arm)?;
    let groups = ob
        .data
        .and_then(|d| d.typed::<MeshId>())
        .and_then(|m| main.meshes.get(m))
        .map(|m| m.vertex_group_names.clone())
        .unwrap_or_default();
    let p = &g.settings.props;
    Some(MeshSkin {
        joints,
        groups,
        object_world: matrix_world(main, ob),
        armature_world: matrix_world(main, arm_ob),
        influence_nb: p.export_influence_nb.value,
        all_influences: p.export_all_influences.value,
    })
}

/// Whether the object of node `i` has an ARMATURE modifier: with `export_skins` its mesh is
/// its own (the add-on's mesh cache key holds the object's modifiers).
pub fn has_armature_modifier(g: &Gather, ob: &Object) -> bool {
    BoneSettings::new(g.settings).skins && armature_modifier_object(ob).is_some()
}

/// The names of the joints of armature node `arm` (`skins.gather_skin` would give it a
/// skin), `None` when it gets none: not in the tree, not an armature, no bones, no joints.
fn skin_joint_names(g: &Gather, tree: &VTree, arm: usize) -> Option<Vec<SmolStr>> {
    if !BoneSettings::new(g.settings).skins || !tree.exists(arm) {
        return None;
    }
    let ob = g.main.objects.get(tree.nodes[arm].object)?;
    if tree.nodes[arm].kind != VType::Armature
        || armature_of(g.main, ob).is_none_or(|a| a.bones.is_empty())
    {
        return None;
    }
    let names: Vec<SmolStr> = tree
        .all_bones(arm)
        .iter()
        .map(|&b| joint_name(tree, b))
        .collect();
    (!names.is_empty()).then_some(names)
}

/// A joint node's name: the bone's, or `<bone>_leaf` for a leaf bone.
fn joint_name(tree: &VTree, b: usize) -> SmolStr {
    let n = &tree.nodes[b];
    match n.leaf_reference {
        Some(r) => SmolStr::new(format!(
            "{}_leaf",
            tree.nodes[r].bone.as_deref().unwrap_or("")
        )),
        None => n.bone.clone().unwrap_or_default(),
    }
}

/// The mesh node `j`'s extraction exports (the evaluated mesh with `export_apply`, else the
/// object's data).
fn exported_mesh(g: &Gather, ob_ref: IdRef<ObjectId>, ob: &Object) -> Option<std::sync::Arc<Mesh>> {
    if let Some(m) = apply::evaluated(g, ob_ref) {
        return Some(m);
    }
    let me = ob.data.and_then(|d| d.typed::<MeshId>())?;
    g.main.meshes.get_arc(me)
}

/// Whether the vertex groups of node `i`'s object assign any vertex (`nodes.gather_skin`'s
/// `any(vertex.groups …)` on the evaluated mesh).
fn any_vertex_in_groups(g: &Gather, ob_ref: IdRef<ObjectId>, ob: &Object) -> bool {
    exported_mesh(g, ob_ref, ob)
        .and_then(|m| {
            m.deform_verts()
                .map(|d| d.iter().any(|dv| !dv.weights.is_empty()))
        })
        .unwrap_or(false)
}

/// `nodes.gather_skin`: whether node `i` (an object) gets a skin, and its armature node.
pub fn node_skin_armature(g: &Gather, tree: &VTree, i: usize) -> Option<usize> {
    let n = &tree.nodes[i];
    if n.kind == VType::Bone {
        return None;
    }
    let main = g.main;
    let ob = main.objects.get(n.object)?;
    let arm_ob_ref = armature_modifier_object(ob)??;
    let mesh = ob
        .data
        .and_then(|d| d.typed::<MeshId>())
        .and_then(|m| main.meshes.get(m))?;
    if mesh.vertex_group_names.is_empty() || !any_vertex_in_groups(g, n.object, ob) {
        return None;
    }
    if ob.parent_type == "BONE"
        && ob
            .parent
            .and_then(|p| main.objects.get(p))
            .zip(main.objects.get(arm_ob_ref))
            .is_some_and(|(p, a)| p.header.name == a.header.name)
    {
        return None;
    }
    let arm = n.armature?;
    skin_joint_names(g, tree, arm)?;
    Some(arm)
}

/// Records that node `i` has a skin (`vnode.skin`), for its children's TRS.
pub fn set_skinned(g: &mut Gather, i: usize) {
    let scene = g.scene;
    g.state::<SkinState>().skinned.insert((scene, i));
}

/// Whether node `i` has a skin.
pub fn is_skinned(g: &mut Gather, i: usize) -> bool {
    let scene = g.scene;
    g.state::<SkinState>().skinned.contains(&(scene, i))
}

/// Whether a mesh of the armature node `arm` has a vertex no joint weighs
/// (`need_neutral_bone`): over the existing MESH object nodes skinned by `arm`.
fn needs_neutral_bone(g: &Gather, tree: &VTree, arm: usize) -> bool {
    (0..tree.nodes.len())
        .filter(|&j| {
            tree.exists(j)
                && tree.nodes[j].kind == VType::Object
                && tree.nodes[j].armature == Some(arm)
        })
        .any(|j| {
            let r = tree.nodes[j].object;
            let Some(ob) = g.main.objects.get(r) else {
                return false;
            };
            if ob.object_type() != "MESH" {
                return false;
            }
            let Some(skin) = mesh_skin(g, tree, j) else {
                return false;
            };
            let Some(mesh) = exported_mesh(g, r, ob) else {
                return false;
            };
            skin.vert_bones(mesh.verts_num(), mesh.deform_verts()).1
        })
}

/// TRS of a joint (`gather_joint_vnode`, `get_objects_parented_to_bones`): decomposed and
/// swizzled without `round_if_near`.
fn joint_trs(m: &Mat4, yup: bool) -> (Option<[f64; 3]>, Option<[f64; 4]>, Option<[f64; 3]>) {
    let (t, q, s) = decompose_wxyz(m);
    let (t, q, s) = if yup {
        ([t.x, t.z, -t.y], [q[0], q[1], q[3], -q[2]], [s.x, s.z, s.y])
    } else {
        (t.to_array(), q, s.to_array())
    };
    let f = f64::from;
    let translation = (t[0] != 0.0 || t[1] != 0.0 || t[2] != 0.0).then(|| t.map(f));
    let rotation = (q[0] != 1.0 || q[1] != 0.0 || q[2] != 0.0 || q[3] != 0.0)
        .then(|| [f(q[1]), f(q[2]), f(q[3]), f(q[0])]);
    let scale = (s[0] != 1.0 || s[1] != 1.0 || s[2] != 1.0).then(|| s.map(f));
    (translation, rotation, scale)
}

/// `gather_joint_vnode`: the joint node of bone node `b` (cached), after its child joints and
/// the objects parented to its bone (`get_objects_parented_to_bones`).
pub fn gather_joint(g: &mut Gather, tree: &VTree, b: usize) -> Result<Index, ExportError> {
    let scene = g.scene;
    if let Some(&i) = g.state::<SkinState>().joints.get(&(scene, b)) {
        return Ok(i);
    }
    let n = &tree.nodes[b];
    let local = match n.parent {
        Some(p) => matmul(&inverted_safe(&tree.nodes[p].matrix_world), &n.matrix_world),
        None => n.matrix_world,
    };
    let (translation, rotation, scale) = joint_trs(&local, g.settings.export_yup);
    let mut children = Vec::new();
    for &c in &n.children {
        if tree.nodes[c].kind == VType::Bone && tree.exists(c) {
            children.push(gather_joint(g, tree, c)?);
        }
    }
    // Objects parented to this bone, placed relative to it.
    for &c in &n.children {
        if tree.nodes[c].kind == VType::Bone || !tree.exists(c) {
            continue;
        }
        let ci = crate::export::nodes::gather_node(g, tree, c)?;
        let cn = &tree.nodes[c];
        let bone_node = cn.parent_bone.unwrap_or(b);
        let m = matmul(
            &inverted_safe(&tree.nodes[bone_node].matrix_world),
            &cn.matrix_world,
        );
        let (t, r, s) = joint_trs(&m, g.settings.export_yup);
        let node = &mut g.doc.nodes[ci as usize];
        node.translation = t;
        node.rotation = r;
        node.scale = s;
        children.push(ci);
    }
    let node = Node {
        children,
        name: Some(joint_name(tree, b).to_string()),
        rotation,
        scale,
        translation,
        ..Node::default()
    };
    g.doc.nodes.push(node);
    let index = (g.doc.nodes.len() - 1) as Index;
    g.state::<SkinState>().joints.insert((scene, b), index);
    if let (Some(bone), Some(arm)) = (&n.bone, n.armature) {
        let arm_ob = tree.nodes[arm].object;
        g.state::<crate::export::node_map::NodeMap>()
            .bones
            .insert((scene, arm_ob, bone.clone()), index);
    }
    Ok(index)
}

/// The inverse bind matrices of the joints of armature node `arm`, column-major.
fn inverse_bind_matrices(g: &Gather, tree: &VTree, arm: usize) -> Vec<[f32; 16]> {
    let main = g.main;
    let abc = axis_basis_change(g.settings.export_yup);
    let Some(arm_ob) = main.objects.get(tree.nodes[arm].object) else {
        return Vec::new();
    };
    let arm_mw = matrix_world(main, arm_ob);
    let rest = armature_of(main, arm_ob).map(frap_scene::edit_armature::rest::rest_pose);
    tree.all_bones(arm)
        .iter()
        .map(|&b| {
            let n = &tree.nodes[b];
            let m = match n.leaf_reference {
                None => {
                    let local = n
                        .bone
                        .as_deref()
                        .and_then(|name| {
                            let a = armature_of(main, arm_ob)?;
                            let i = a.bone_index(name)?;
                            rest.as_ref().and_then(|r| r.get(i)).map(|r| r.arm_mat)
                        })
                        .unwrap_or(Mat4::IDENTITY);
                    matmul(&arm_mw, &local)
                }
                Some(r) => matmul(
                    &arm_mw,
                    &tree.nodes[r].matrix_world_tail.unwrap_or(Mat4::IDENTITY),
                ),
            };
            inverted_safe(&matmul(&abc, &m)).to_cols_array()
        })
        .collect()
}

/// `skins.gather_skin` (cached per armature node): the skin of armature node `arm`, written
/// on first use: its inverse bind matrices, its joints, and the neutral bone when a mesh of
/// the armature needs it.
pub fn gather_skin(g: &mut Gather, tree: &VTree, arm: usize) -> Result<Option<Index>, ExportError> {
    gather_skin_ex(g, tree, arm, true)
}

fn gather_skin_ex(
    g: &mut Gather,
    tree: &VTree,
    arm: usize,
    with_neutral: bool,
) -> Result<Option<Index>, ExportError> {
    let scene = g.scene;
    if let Some(w) = g.state::<SkinState>().skins.get(&(scene, arm)) {
        return Ok(w.as_ref().map(|w| w.index));
    }
    if skin_joint_names(g, tree, arm).is_none() {
        g.state::<SkinState>().skins.insert((scene, arm), None);
        return Ok(None);
    }
    let main = g.main;
    let arm_ob = tree.nodes[arm].object;
    let name = main.objects.get(arm_ob).map(|o| o.header.name.to_string());
    let mut ibms = inverse_bind_matrices(g, tree, arm);
    let neutral = with_neutral && needs_neutral_bone(g, tree, arm);
    if neutral {
        // `add_neutral_bones`: the armature object's world matrix in glTF space.
        let abc = axis_basis_change(g.settings.export_yup);
        let arm_mw = main
            .objects
            .get(arm_ob)
            .map(|o| matrix_world(main, o))
            .unwrap_or(Mat4::IDENTITY);
        ibms.push(inverted_safe(&matmul(&abc, &arm_mw)).to_cols_array());
    }
    let data = AccessorData::new(
        AccessorType::Mat4,
        false,
        frap_gltf::accessor::Components::F32(ibms.into_iter().flatten().collect()),
    )
    .map_err(|e| ExportError::Invalid(e.to_string()))?;
    let ibm = g.buf.push_accessor(&data, &AccessorOpts::default());
    // Joints, written in `skin.joints` order (each after its child joints).
    let mut joints = Vec::new();
    for &b in tree.all_bones(arm) {
        joints.push(gather_joint(g, tree, b)?);
    }
    let mut neutral_node = None;
    if neutral {
        let abc = axis_basis_change(g.settings.export_yup);
        let (t, r, s) = joint_trs(&abc, false);
        g.doc.nodes.push(Node {
            name: Some(NEUTRAL_BONE.to_owned()),
            translation: t,
            rotation: r,
            scale: s,
            ..Node::default()
        });
        let ni = (g.doc.nodes.len() - 1) as Index;
        joints.push(ni);
        neutral_node = Some(ni);
        // Appended to the armature's node: now if it is written, else when it is.
        match g
            .state::<crate::export::node_map::NodeMap>()
            .object(scene, arm_ob)
        {
            Some(a) if tree.nodes[arm].keep => g.doc.nodes[a as usize].children.push(ni),
            _ => {
                g.state::<SkinState>()
                    .pending_neutral
                    .insert((scene, arm), ni);
            }
        }
    }
    g.doc.skins.push(Skin {
        inverse_bind_matrices: Some(ibm),
        joints,
        name,
        ..Skin::default()
    });
    let index = (g.doc.skins.len() - 1) as Index;
    g.state::<SkinState>().skins.insert(
        (scene, arm),
        Some(Written {
            index,
            neutral: neutral_node,
        }),
    );
    Ok(Some(index))
}

/// The neutral bone to append to armature node `arm`'s glTF node, if its skin added one
/// before the node was written.
pub fn take_pending_neutral(g: &mut Gather, arm: usize) -> Option<Index> {
    let scene = g.scene;
    g.state::<SkinState>().pending_neutral.remove(&(scene, arm))
}

/// `get_unused_skins` (+ `traverse_unused_skins`): after a scene, the skins of its armature
/// nodes that no kept object is skinned by (with `export_armature_object_remove`, only
/// armatures that passed the filters).
pub fn gather_unused_skins(g: &mut Gather, tree: &VTree) -> Result<(), ExportError> {
    for arm in 0..tree.nodes.len() {
        if !tree.exists(arm) || tree.nodes[arm].kind != VType::Armature {
            continue;
        }
        if tree.armature_object_remove && !tree.nodes[arm].arma_exported {
            continue;
        }
        let used = (0..tree.nodes.len()).any(|m| {
            tree.nodes[m].keep
                && tree.nodes[m].kind == VType::Object
                && tree.nodes[m].armature == Some(arm)
        });
        if !used {
            gather_skin_ex(g, tree, arm, false)?;
        }
    }
    Ok(())
}

/// Whether the written skin of armature node `arm` has a neutral bone.
pub fn has_neutral(g: &mut Gather, arm: usize) -> bool {
    let scene = g.scene;
    g.state::<SkinState>()
        .skins
        .get(&(scene, arm))
        .and_then(|w| w.as_ref())
        .is_some_and(|w| w.neutral.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_math::glam::Vec3;

    fn skin(joints: &[&str], groups: &[&str]) -> MeshSkin {
        MeshSkin {
            joints: joints.iter().map(|s| SmolStr::new(*s)).collect(),
            groups: groups.iter().map(|s| SmolStr::new(*s)).collect(),
            object_world: Mat4::IDENTITY,
            armature_world: Mat4::IDENTITY,
            influence_nb: 4,
            all_influences: false,
        }
    }

    #[test]
    fn bone_data_sorts_filters_and_falls_back_to_the_neutral_bone() {
        let s = skin(&["A", "B", "C"], &["C", "Other", "A", "B"]);
        let dv = |w: &[(u32, f32)]| DeformVert {
            weights: w.iter().copied().collect(),
        };
        let dverts = vec![
            dv(&[(0, 0.2), (2, 0.5), (3, 0.2)]),
            dv(&[(1, 0.9), (0, 0.00005)]),
            dv(&[]),
            dv(&[(7, 1.0), (3, 0.0001)]),
        ];
        let (b, neutral) = s.vert_bones(5, Some(&dverts));
        // Heaviest first; equal weights keep the group order (C before B).
        assert_eq!(b[0].as_slice(), &[(0, 0.5), (2, 0.2), (1, 0.2)]);
        // A group without a joint and a weight at or below 0.0001 are dropped.
        assert_eq!(b[1].as_slice(), &[(3, 1.0)]);
        assert_eq!(b[2].as_slice(), &[(3, 1.0)]);
        assert_eq!(b[3].as_slice(), &[(3, 1.0)]);
        // A vertex without a deform vertex too.
        assert_eq!(b[4].as_slice(), &[(3, 1.0)]);
        assert!(neutral);
        let (_, neutral) = s.vert_bones(1, Some(&dverts));
        assert!(!neutral);
    }

    #[test]
    fn duplicate_joint_names_map_to_the_last() {
        let s = skin(&["A", "A"], &["A"]);
        assert_eq!(s.group_to_joint(), vec![Some(1)]);
    }

    #[test]
    fn inverse_3x3_and_transforms() {
        let m = Mat3::from_cols_array(&[2.0, 0.0, 0.0, 0.0, 4.0, 0.0, 1.0, 0.0, 0.5]);
        let inv = inverted_safe3(&m);
        assert!((m * inv).abs_diff_eq(Mat3::IDENTITY, 1e-6));
        // Singular: finite.
        let z = Mat3::from_cols_array(&[0.0; 9]);
        assert!(
            inverted_safe3(&z)
                .to_cols_array()
                .iter()
                .all(|x| x.is_finite())
        );
        let mut s = skin(&[], &[]);
        s.object_world = Mat4::from_scale(Vec3::new(2.0, 1.0, 1.0));
        // Normals of a scaled object: the inverse transpose.
        let n = s.normal_transform();
        assert!((n.x_axis.x - 0.5).abs() < 1e-7);
        assert_eq!(apply_mat(&s.object_world, [1.0, 2.0, 3.0]), [2.0, 2.0, 3.0]);
        assert!(s.tangent_transform().abs_diff_eq(Mat3::IDENTITY, 1e-6));
    }

    #[test]
    fn joint_trs_has_no_rounding() {
        let m = Mat4::from_translation(Vec3::new(1e-7, 0.0, 0.0));
        let (t, r, s) = joint_trs(&m, true);
        assert_eq!(t, Some([f64::from(1e-7f32), 0.0, 0.0]));
        assert_eq!((r, s), (None, None));
        // The neutral bone of a Y-up export: the basis change as a rotation.
        let (t, r, s) = joint_trs(&axis_basis_change(true), false);
        assert!(t.is_none() && s.is_none());
        let r = r.unwrap();
        assert!(
            (r[0] + std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-6,
            "{r:?}"
        );
    }
}
