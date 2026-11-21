// SPDX-License-Identifier: GPL-3.0-or-later
//! The virtual node tree: glTF nodes copied into a tree of `VNode`s, then transformed into a
//! shape Blender can hold (one data type per object, armatures above the joints of each skin,
//! skinned meshes moved under their armature, cameras and lights rotated to Blender's
//! conventions, bones given a bind pose and prettified edit bones).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/vnode.py` (Apache-2.0, The glTF-Blender-IO
//! authors): `compute_vnodes`, `init_vnodes`, `mark_bones_and_armas`,
//! `deepest_common_ancestor`, `move_skinned_meshes`, `reparent`, `fixup_multitype_nodes`,
//! `correct_cameras_and_lights`, `local_rotation`, `pick_bind_pose`, `prettify_bones`,
//! `pick_bone_length`, `pick_bone_rotation`, `temperance`, `rotate_edit_bone`,
//! `calc_bone_matrices`, `VNode.trs`; `nearby_signed_perm_matrix` from
//! `blender/com/gltf2_blender_math.py`. EXT_mesh_gpu_instancing (`manage_gpu_instancing`,
//! M7 WP 7.5, `import/instancing.rs`) adds `VNode.Inst` children. Python's dict of vnodes
//! keyed by mixed ids (`3`, `'root'`, `'3.skinned'`, `'3.camera'`) is an arena here, in the
//! same insertion order, except that the instance vnodes (`'3.0'`, `'3.1'`, …, inserted right
//! after node 3 in Python) follow the glTF nodes, so glTF node `i` stays vnode `i`; no pass
//! iterates over them (they have no skin, camera, light or bone).

use std::collections::{HashMap, HashSet};

use frap_gltf::Document;
use frap_math::glam::{Mat3, Mat4, Vec3};
use frap_math::quat::QuatWxyz;
use frap_rna::{IdRef, ObjectId};

use super::convert::{Units, camera_correction, qconj, qmul, qrot, quat, scale, scale_rot_swap};
use super::settings::BoneHeuristic;

const QUAT_ONE: QuatWxyz = [1.0, 0.0, 0.0, 0.0];

/// `MIN_BONE_LENGTH`: shorter bones are deleted by Blender.
pub(crate) const MIN_BONE_LENGTH: f64 = 0.004;

/// `VNode.type`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum VKind {
    /// `VNode.Object`
    Object,
    /// `VNode.Bone`
    Bone,
    /// `VNode.Inst`: one instance of an `EXT_mesh_gpu_instancing` node (M7 WP 7.5).
    Inst,
    /// `VNode.DummyRoot`
    DummyRoot,
}

/// A virtual node.
#[derive(Clone, Debug)]
pub(crate) struct VNode {
    /// glTF node name.
    pub name: Option<String>,
    /// Fallback name (`Node_<i>`).
    pub default_name: String,
    /// Children, in glTF order (moved data appended).
    pub children: Vec<usize>,
    /// Parent vnode.
    pub parent: Option<usize>,
    /// Type.
    pub kind: VKind,
    /// `is_arma`: this object holds the armature of a skin.
    pub is_arma: bool,
    /// `arma_name`: the armature datablock's name (the skin's name or `Armature`).
    pub arma_name: String,
    /// `bone_arma`: the armature vnode of a bone.
    pub bone_arma: Option<usize>,
    /// Base TRS in Blender space.
    pub base_trs: ([f32; 3], QuatWxyz, [f32; 3]),
    /// Rotation applied after the base TRS (children of corrected cameras/lights/bones).
    pub rotation_after: QuatWxyz,
    /// Rotation applied before the base TRS (camera/light correction, bone rotation).
    pub rotation_before: QuatWxyz,
    /// glTF node whose mesh this vnode instantiates.
    pub mesh_node_idx: Option<usize>,
    /// glTF node whose camera this vnode instantiates.
    pub camera_node_idx: Option<usize>,
    /// glTF node whose KHR_lights_punctual light this vnode instantiates.
    pub light_node_idx: Option<usize>,
    /// glTF scenes this node belongs to.
    pub scenes: Vec<usize>,
    /// The glTF node this vnode was created from (`isinstance(vnode_id, int)`).
    pub gltf_node: Option<usize>,
    /// The created object (for a bone: its armature's object).
    pub blender_object: Option<IdRef<ObjectId>>,
    /// The bone this node became (`vnode.blender_bone_name`; set by skin import, WP 6.8, and
    /// read by animation import, WP 6.7, to target pose bones). M5+M6 W0 contract.
    pub blender_bone_name: Option<smol_str::SmolStr>,
    /// `bind_trans` / `bind_rot`: the bone's bind pose relative to its parent.
    pub bind_trans: [f32; 3],
    /// See `bind_trans`.
    pub bind_rot: QuatWxyz,
    /// `editbone_trans` / `editbone_rot`: the edit bone relative to its parent.
    pub editbone_trans: [f32; 3],
    /// See `editbone_trans`.
    pub editbone_rot: QuatWxyz,
    /// `bone_length` (`pick_bone_length`).
    pub bone_length: f64,
    /// `bind_arma_mat`: bone → armature space in the bind pose.
    pub bind_arma_mat: Mat4,
    /// `editbone_arma_mat`: bone → armature space of the edit bone.
    pub editbone_arma_mat: Mat4,
    /// `mesh_idx` of an `Inst` vnode: the glTF mesh it instantiates (M7 WP 7.5).
    pub inst_mesh_idx: Option<usize>,
    /// The glTF node whose `EXT_mesh_gpu_instancing` made this `Inst` vnode.
    pub inst_of: Option<usize>,
    /// The instance's index in that node's instance attributes.
    pub inst_index: Option<usize>,
}

impl VNode {
    fn new(default_name: String) -> VNode {
        VNode {
            name: None,
            default_name,
            children: Vec::new(),
            parent: None,
            kind: VKind::Object,
            is_arma: false,
            arma_name: String::new(),
            bone_arma: None,
            base_trs: ([0.0; 3], QUAT_ONE, [1.0; 3]),
            rotation_after: QUAT_ONE,
            rotation_before: QUAT_ONE,
            mesh_node_idx: None,
            camera_node_idx: None,
            light_node_idx: None,
            scenes: Vec::new(),
            gltf_node: None,
            blender_object: None,
            blender_bone_name: None,
            bind_trans: [0.0; 3],
            bind_rot: QUAT_ONE,
            editbone_trans: [0.0; 3],
            editbone_rot: QUAT_ONE,
            bone_length: 1.0,
            bind_arma_mat: Mat4::IDENTITY,
            editbone_arma_mat: Mat4::IDENTITY,
            inst_mesh_idx: None,
            inst_of: None,
            inst_index: None,
        }
    }

    /// `VNode.trs()`: `(rotation after) (base TRS) (rotation before)`.
    pub fn trs(&self) -> ([f32; 3], QuatWxyz, [f32; 3]) {
        let (t, r, s) = self.base_trs;
        (
            qrot(self.rotation_after, t),
            qmul(qmul(self.rotation_after, r), self.rotation_before),
            scale_rot_swap(self.rotation_before, s),
        )
    }
}

/// The vnode arena; `root` is the dummy root inserted by `init_vnodes`.
#[derive(Clone, Debug)]
pub(crate) struct VTree {
    /// Every vnode; glTF node `i` is vnode `i`.
    pub nodes: Vec<VNode>,
    /// The dummy root.
    pub root: usize,
}

/// `get_node_trs`.
fn node_trs(units: Units, n: &frap_gltf::json::Node) -> ([f32; 3], QuatWxyz, [f32; 3]) {
    if let Some(m) = n.matrix {
        let (loc, q, size) = frap_math::matrix::decompose_wxyz(&units.matrix(m));
        return (loc.to_array(), q, size.to_array());
    }
    (
        units.loc(n.translation_or_default()),
        quat(n.rotation_or_default()),
        scale(n.scale_or_default()),
    )
}

/// The glTF nodes an animation channel targets (`node.animations` is non-empty): core
/// channels by `target.node`, `KHR_animation_pointer` channels by a `/nodes/<i>/…` pointer.
pub(crate) fn animated_nodes(doc: &Document) -> HashSet<usize> {
    let mut out = HashSet::new();
    for a in &doc.animations {
        for c in &a.channels {
            match c.target.node {
                Some(n) => {
                    out.insert(n as usize);
                }
                None => {
                    if let Some(i) = pointer_target(&c.target.extensions, "nodes") {
                        out.insert(i);
                    }
                }
            }
        }
    }
    out
}

/// The index of a `KHR_animation_pointer` channel's `/<kind>/<i>/…` pointer.
pub(crate) fn pointer_target(ext: &frap_gltf::json::Extensions, kind: &str) -> Option<usize> {
    let p = ext
        .0
        .get("KHR_animation_pointer")?
        .get("pointer")?
        .as_str()?;
    let tab: Vec<&str> = p.split('/').collect();
    if tab.len() >= 4 && tab[1] == kind {
        tab[2].parse().ok()
    } else {
        None
    }
}

/// `Vector((0, 0, 0))` translation then rotation: `Matrix.Translation(t) @
/// Quaternion(r).to_matrix().to_4x4()`.
fn tr_matrix(t: [f32; 3], r: QuatWxyz) -> Mat4 {
    let m3 = frap_math::quat::quat_to_mat3(r);
    let mut m = Mat4::from_mat3(m3);
    m.w_axis = Vec3::from_array(t).extend(1.0);
    m
}

/// `nearby_signed_perm_matrix(rot)`: `rot.to_matrix()` snapped to the closest signed
/// permutation (row by row, the largest entry of each row becomes ±1).
pub(crate) fn nearby_signed_perm_matrix(rot: QuatWxyz) -> Mat3 {
    let m = frap_math::quat::quat_to_mat3(rot);
    let mut rows: [[f32; 3]; 3] = std::array::from_fn(|r| [m.x_axis[r], m.y_axis[r], m.z_axis[r]]);
    let sign = |v: f32| if v > 0.0 { 1.0 } else { -1.0 };
    let x = rows[0];
    let (a, b, c) = (x[0].abs(), x[1].abs(), x[2].abs());
    let i = if a >= b && a >= c {
        0
    } else if b >= c {
        1
    } else {
        2
    };
    rows[0][i] = sign(x[i]);
    rows[0][(i + 1) % 3] = 0.0;
    rows[0][(i + 2) % 3] = 0.0;
    let y = rows[1];
    let (a, b) = (y[(i + 1) % 3].abs(), y[(i + 2) % 3].abs());
    let j = if a >= b { (i + 1) % 3 } else { (i + 2) % 3 };
    rows[1][j] = sign(y[j]);
    rows[1][(j + 1) % 3] = 0.0;
    rows[1][(j + 2) % 3] = 0.0;
    let k = 3 - i - j;
    let z = rows[2];
    rows[2][k] = sign(z[k]);
    rows[2][(k + 1) % 3] = 0.0;
    rows[2][(k + 2) % 3] = 0.0;
    Mat3::from_cols_array_2d(&std::array::from_fn(|col| {
        std::array::from_fn(|r| rows[r][col])
    }))
}

/// `Vector.length` of an f32 vector (mathutils sums in double).
fn length(v: [f32; 3]) -> f64 {
    super::convert::length_f64(v)
}

impl VTree {
    /// `compute_vnodes` up to `correct_cameras_and_lights`; the bone passes need the inverse
    /// bind matrices and the settings ([`VTree::compute_bones`]).
    pub fn compute(doc: &Document, units: Units) -> VTree {
        let mut t = VTree::init(doc, units);
        t.mark_bones_and_armas(doc);
        t.move_skinned_meshes(doc);
        t.fixup_multitype_nodes();
        t.correct_cameras_and_lights();
        t
    }

    /// The bone passes of `compute_vnodes`: `pick_bind_pose`, `prettify_bones`,
    /// `calc_bone_matrices`. `inv_binds[s]`: the inverse bind matrices of skin `s` (Blender
    /// space), `None` without an accessor.
    pub fn compute_bones(
        &mut self,
        doc: &Document,
        inv_binds: &[Option<Vec<Mat4>>],
        heuristic: BoneHeuristic,
        guess_original_bind_pose: bool,
    ) {
        self.pick_bind_pose(doc, inv_binds, guess_original_bind_pose);
        self.prettify_bones(heuristic);
        self.calc_bone_matrices();
    }

    /// `init_vnodes` (the Blender scene/collection bookkeeping it also does lives in
    /// `scenes.rs`).
    fn init(doc: &Document, units: Units) -> VTree {
        let mut nodes: Vec<VNode> = Vec::with_capacity(doc.nodes.len() + 1);
        let mut instanced: Vec<(usize, usize, usize)> = Vec::new();
        for (i, n) in doc.nodes.iter().enumerate() {
            let mut v = VNode::new(format!("Node_{i}"));
            // `vnode.name or …` everywhere it is read: "" falls back like a missing name.
            v.name = super::truthy_name(&n.name);
            // An invalid child index (Python's `KeyError`) is dropped, so it can never name
            // one of the instance vnodes appended after the glTF nodes.
            v.children = n
                .children
                .iter()
                .map(|&c| c as usize)
                .filter(|&c| c < doc.nodes.len())
                .collect();
            v.base_trs = node_trs(units, n);
            v.gltf_node = Some(i);
            if let Some(m) = n.mesh {
                // `manage_gpu_instancing` (M7 WP 7.5): the node's mesh goes to its instances.
                match super::instancing::extension(doc, i) {
                    Some(ext) => {
                        let count = super::instancing::instance_count(doc, &ext);
                        instanced.push((i, m as usize, count));
                    }
                    None => v.mesh_node_idx = Some(i),
                }
            }
            if n.camera.is_some() {
                v.camera_node_idx = Some(i);
            }
            if n.extensions.contains("KHR_lights_punctual") {
                v.light_node_idx = Some(i);
            }
            nodes.push(v);
        }
        // The instance vnodes `'<i>.<k>'` (TRS read later, `instancing::fill_trs`).
        for (i, mesh, count) in instanced {
            for k in 0..count {
                let mut v = VNode::new(format!("Node_{i}.{k}"));
                v.kind = VKind::Inst;
                v.inst_mesh_idx = Some(mesh);
                v.inst_of = Some(i);
                v.inst_index = Some(k);
                let id = nodes.len();
                nodes.push(v);
                nodes[i].children.push(id);
            }
        }
        for id in 0..nodes.len() {
            for c in nodes[id].children.clone() {
                if let Some(child) = nodes.get_mut(c) {
                    child.parent = Some(id);
                }
            }
        }
        fn add_to_scene(nodes: &mut [VNode], idx_scene: usize, node: usize, depth: usize) {
            // Cycles are invalid glTF (the validator rejects them); the depth bound keeps a
            // hostile file from recursing forever.
            if depth > nodes.len() {
                return;
            }
            let Some(v) = nodes.get_mut(node) else {
                return;
            };
            v.scenes.push(idx_scene);
            for c in v.children.clone() {
                add_to_scene(nodes, idx_scene, c, depth + 1);
            }
        }
        for (idx_scene, scene) in doc.scenes.iter().enumerate() {
            for &n in &scene.nodes {
                add_to_scene(&mut nodes, idx_scene, n as usize, 0);
            }
        }
        let roots: Vec<usize> = (0..nodes.len())
            .filter(|&i| nodes[i].parent.is_none())
            .collect();
        let root = nodes.len();
        let mut r = VNode::new("Root".into());
        r.kind = VKind::DummyRoot;
        r.children = roots.clone();
        nodes.push(r);
        for i in roots {
            nodes[i].parent = Some(root);
        }
        VTree { nodes, root }
    }

    fn push_child(&mut self, parent: usize, mut v: VNode) -> usize {
        v.parent = Some(parent);
        v.scenes = self.nodes[parent].scenes.clone();
        let id = self.nodes.len();
        self.nodes.push(v);
        self.nodes[parent].children.push(id);
        id
    }

    /// `path_from_root`.
    fn path_from_root(&self, mut id: usize) -> Vec<usize> {
        let mut path = Vec::new();
        while path.len() <= self.nodes.len() {
            path.push(id);
            match self.nodes[id].parent {
                Some(p) => id = p,
                None => break,
            }
        }
        path.reverse();
        path
    }

    /// `deepest_common_ancestor`.
    fn deepest_common_ancestor(&self, ids: &[usize]) -> usize {
        let mut prefix: Vec<usize> = Vec::new();
        for &id in ids {
            let path = self.path_from_root(id);
            if prefix.is_empty() {
                prefix = path;
            } else {
                let n = prefix.iter().zip(&path).take_while(|(a, b)| a == b).count();
                prefix.truncate(n);
            }
        }
        prefix.last().copied().unwrap_or(self.root)
    }

    /// `mark_bones_and_armas`: the deepest common ancestor of each skin's joints (and
    /// skeleton) becomes an armature object, everything between it and a joint a bone; then
    /// each bone learns its armature.
    fn mark_bones_and_armas(&mut self, doc: &Document) {
        let n = doc.nodes.len();
        for skin in &doc.skins {
            let mut descendants: Vec<usize> = skin
                .joints
                .iter()
                .map(|&j| j as usize)
                .filter(|&j| j < n)
                .collect();
            if descendants.is_empty() {
                continue;
            }
            if let Some(s) = skin.skeleton.map(|s| s as usize).filter(|&s| s < n) {
                descendants.push(s);
            }
            let mut arma = self.deepest_common_ancestor(&descendants);
            if skin.joints.iter().any(|&j| j as usize == arma) {
                arma = self.nodes[arma].parent.unwrap_or(self.root);
            }
            if self.nodes[arma].kind != VKind::Bone {
                let scenes: Vec<usize> = {
                    let mut s: Vec<usize> = skin
                        .joints
                        .iter()
                        .filter_map(|&j| self.nodes.get(j as usize))
                        .flat_map(|v| v.scenes.iter().copied())
                        .collect();
                    s.sort_unstable();
                    s.dedup();
                    s
                };
                let v = &mut self.nodes[arma];
                v.kind = VKind::Object;
                v.is_arma = true;
                v.arma_name = skin
                    .name
                    .clone()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Armature".into());
                v.scenes = scenes;
            }
            for &j in &skin.joints {
                let mut j = j as usize;
                let mut guard = 0;
                while j != arma && j < self.nodes.len() && guard <= self.nodes.len() {
                    self.nodes[j].kind = VKind::Bone;
                    self.nodes[j].is_arma = false;
                    match self.nodes[j].parent {
                        Some(p) => j = p,
                        None => break,
                    }
                    guard += 1;
                }
            }
        }
        // `visit`: each bone's armature is the nearest armature above it.
        let mut stack: Vec<(usize, Option<usize>)> = vec![(self.root, None)];
        let mut seen = vec![false; self.nodes.len()];
        while let Some((id, cur)) = stack.pop() {
            if std::mem::replace(&mut seen[id], true) {
                continue;
            }
            let v = &mut self.nodes[id];
            let cur = if v.is_arma {
                Some(id)
            } else if v.kind == VKind::Bone {
                v.bone_arma = cur;
                cur
            } else {
                None
            };
            for &c in self.nodes[id].children.iter().rev() {
                if c < seen.len() {
                    stack.push((c, cur));
                }
            }
        }
    }

    /// `reparent`.
    fn reparent(&mut self, id: usize, new_parent: usize) {
        if self.nodes[id].parent == Some(new_parent) {
            return;
        }
        if let Some(p) = self.nodes[id].parent
            && let Some(k) = self.nodes[p].children.iter().position(|&c| c == id)
        {
            self.nodes[p].children.remove(k);
        }
        self.nodes[id].parent = Some(new_parent);
        self.nodes[new_parent].children.push(id);
    }

    /// `move_skinned_meshes`: a skinned mesh goes under the armature of its skin (the node
    /// itself when nothing else depends on it, else a `<id>.skinned` child of the armature).
    fn move_skinned_meshes(&mut self, doc: &Document) {
        let animated = animated_nodes(doc);
        let ids: Vec<usize> = (0..self.nodes.len()).collect();
        for id in ids {
            let Some(mesh_node) = self.nodes[id].mesh_node_idx else {
                continue;
            };
            let Some(skin) = doc.nodes[mesh_node].skin.map(|s| s as usize) else {
                continue;
            };
            let Some(arma) = doc
                .skins
                .get(skin)
                .and_then(|s| s.joints.first())
                .and_then(|&j| self.nodes.get(j as usize))
                .and_then(|v| v.bone_arma)
            else {
                continue;
            };
            let v = &self.nodes[id];
            let is_animated =
                !doc.animations.is_empty() && v.gltf_node.is_some() && animated.contains(&id);
            let ok_to_move = !is_animated
                && v.kind == VKind::Object
                && !v.is_arma
                && v.children.is_empty()
                && v.camera_node_idx.is_none()
                && v.light_node_idx.is_none();
            if ok_to_move {
                self.reparent(id, arma);
                self.nodes[id].base_trs = ([0.0; 3], QUAT_ONE, [1.0; 3]);
                continue;
            }
            let mut nv = VNode::new("Node".into());
            nv.mesh_node_idx = Some(mesh_node);
            let scenes = self.nodes[id].scenes.clone();
            let nid = self.push_child(arma, nv);
            self.nodes[nid].scenes = scenes;
            self.nodes[id].mesh_node_idx = None;
        }
    }

    /// `fixup_multitype_nodes`: an object holds one of armature, mesh, camera, light (a bone
    /// none); extra data moves to new children (`'<id>.mesh'`, `'<id>.camera'`,
    /// `'<id>.light'`).
    fn fixup_multitype_nodes(&mut self) {
        let ids: Vec<usize> = (0..self.nodes.len()).collect();
        for id in ids {
            let mut needs_move = self.nodes[id].is_arma || self.nodes[id].kind == VKind::Bone;
            let scenes = self.nodes[id].scenes.clone();
            if let Some(m) = self.nodes[id].mesh_node_idx {
                if needs_move {
                    let mut v = VNode::new("Node".into());
                    v.mesh_node_idx = Some(m);
                    let n = self.push_child(id, v);
                    self.nodes[n].scenes = scenes.clone();
                    self.nodes[id].mesh_node_idx = None;
                }
                needs_move = true;
            }
            if let Some(cam) = self.nodes[id].camera_node_idx {
                if needs_move {
                    let mut v = VNode::new("Node".into());
                    v.camera_node_idx = Some(cam);
                    let n = self.push_child(id, v);
                    self.nodes[n].scenes = scenes.clone();
                    self.nodes[id].camera_node_idx = None;
                }
                needs_move = true;
            }
            if let Some(light) = self.nodes[id].light_node_idx
                && needs_move
            {
                let mut v = VNode::new("Node".into());
                v.light_node_idx = Some(light);
                let n = self.push_child(id, v);
                self.nodes[n].scenes = scenes;
                self.nodes[id].light_node_idx = None;
            }
        }
    }

    /// `local_rotation`: `(new world) = (old world) @ rot` without moving the children.
    fn local_rotation(&mut self, id: usize, rot: QuatWxyz) {
        self.nodes[id].rotation_before = qmul(self.nodes[id].rotation_before, rot);
        let rot_inv = qconj(rot);
        for c in self.nodes[id].children.clone() {
            self.nodes[c].rotation_after = qmul(rot_inv, self.nodes[c].rotation_after);
        }
    }

    /// `correct_cameras_and_lights`.
    fn correct_cameras_and_lights(&mut self) {
        let rot = camera_correction();
        for id in 0..self.nodes.len() {
            let v = &self.nodes[id];
            if v.camera_node_idx.is_none() && v.light_node_idx.is_none() {
                continue;
            }
            self.local_rotation(id, rot);
        }
    }

    /// `pick_bind_pose`: each bone's bind pose, its node TRS or (with
    /// `guess_original_bind_pose`) solved from the inverse bind matrices of it and its parent.
    fn pick_bind_pose(&mut self, doc: &Document, inv_binds: &[Option<Vec<Mat4>>], guess: bool) {
        let mut binds: HashMap<usize, Mat4> = HashMap::new();
        if guess {
            binds.insert(self.root, Mat4::IDENTITY);
            for (s, skin) in doc.skins.iter().enumerate() {
                let Some(Some(ibms)) = inv_binds.get(s) else {
                    continue;
                };
                if let Some(mut skel) = skin.skeleton.map(|k| k as usize) {
                    if skin.joints.iter().any(|&j| j as usize == skel) {
                        skel = self
                            .nodes
                            .get(skel)
                            .and_then(|v| v.parent)
                            .unwrap_or(self.root);
                    }
                    binds.entry(skel).or_insert(Mat4::IDENTITY);
                }
                for (i, &j) in skin.joints.iter().enumerate() {
                    if let Some(m) = ibms.get(i) {
                        binds.insert(j as usize, *m);
                    }
                }
            }
        }
        for id in 0..self.nodes.len() {
            if self.nodes[id].kind != VKind::Bone {
                continue;
            }
            let (t, r, _) = self.nodes[id].base_trs;
            let mut bt = t;
            let mut br = r;
            if guess
                && let (Some(own), Some(parent)) = (
                    binds.get(&id),
                    self.nodes[id].parent.and_then(|p| binds.get(&p)),
                )
            {
                let local =
                    crate::export::vtree::matmul(parent, &crate::export::vtree::inverted_safe(own));
                let (t, q, _) = frap_math::matrix::decompose_wxyz(&local);
                bt = t.to_array();
                br = q;
            }
            let v = &mut self.nodes[id];
            v.bind_trans = bt;
            v.bind_rot = br;
            v.editbone_trans = bt;
            v.editbone_rot = br;
        }
    }

    /// The depth-first order of `visit('root')`.
    fn dfs(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![self.root];
        let mut seen = vec![false; self.nodes.len()];
        while let Some(id) = stack.pop() {
            if std::mem::replace(&mut seen[id], true) {
                continue;
            }
            out.push(id);
            for &c in self.nodes[id].children.iter().rev() {
                if c < seen.len() {
                    stack.push(c);
                }
            }
        }
        out
    }

    /// `pick_bone_length`.
    fn pick_bone_length(&self, id: usize) -> f64 {
        let v = &self.nodes[id];
        let lens: Vec<f64> = v
            .children
            .iter()
            .map(|&c| &self.nodes[c])
            .filter(|c| c.kind == VKind::Bone)
            .map(|c| length(c.editbone_trans))
            .filter(|&l| l > MIN_BONE_LENGTH)
            .collect();
        if let Some(m) = lens.iter().copied().reduce(f64::min) {
            return m;
        }
        if let Some(p) = v.parent
            && self.nodes[p].kind == VKind::Bone
        {
            return self.nodes[p].bone_length;
        }
        let own = length(v.editbone_trans);
        if own > MIN_BONE_LENGTH {
            return own;
        }
        1.0
    }

    /// `temperance`: point the bone at the centroid of its children (snapped to the local axes
    /// for `TEMPERANCE`), else keep the parent's rotation.
    fn temperance(&self, id: usize, parent_rot: Option<QuatWxyz>, snap: bool) -> Option<QuatWxyz> {
        let locs: Vec<[f32; 3]> = self.nodes[id]
            .children
            .iter()
            .map(|&c| &self.nodes[c])
            .filter(|c| c.kind == VKind::Bone)
            .map(|c| c.editbone_trans)
            .filter(|&t| length(t) > MIN_BONE_LENGTH)
            .collect();
        if locs.is_empty() {
            return parent_rot;
        }
        let mut centroid = [0.0f32; 3];
        for l in &locs {
            centroid = [centroid[0] + l[0], centroid[1] + l[1], centroid[2] + l[2]];
        }
        let rot = crate::export::mesh::post::morph_targets::rotation_difference(
            [0.0, 1.0, 0.0],
            centroid,
        );
        Some(if snap {
            frap_math::quat::mat3_to_quat(&nearby_signed_perm_matrix(rot))
        } else {
            rot
        })
    }

    /// `rotate_edit_bone`: rotates one edit bone without moving anything else.
    fn rotate_edit_bone(&mut self, id: usize, rot: QuatWxyz) {
        self.nodes[id].editbone_rot = qmul(self.nodes[id].editbone_rot, rot);
        let rot_inv = qconj(rot);
        for c in self.nodes[id].children.clone() {
            let child = &mut self.nodes[c];
            if child.kind == VKind::Bone {
                child.editbone_trans = qrot(rot_inv, child.editbone_trans);
                child.editbone_rot = qmul(rot_inv, child.editbone_rot);
            }
        }
        self.local_rotation(id, rot);
    }

    /// `prettify_bones`: bone lengths and directions (`bone_heuristic`).
    fn prettify_bones(&mut self, heuristic: BoneHeuristic) {
        let order = self.dfs();
        // The parent's chosen rotation (`visit(child, parent_rot=rot)`).
        let mut rot_of: HashMap<usize, Option<QuatWxyz>> = HashMap::new();
        for id in order {
            if self.nodes[id].kind != VKind::Bone {
                rot_of.insert(id, None);
                continue;
            }
            let parent_rot = self.nodes[id]
                .parent
                .and_then(|p| rot_of.get(&p).copied())
                .flatten();
            self.nodes[id].bone_length = self.pick_bone_length(id);
            let rot = match heuristic {
                BoneHeuristic::Blender => {
                    let h = (2f64.sqrt() / 2.0) as f32;
                    Some([h, h, 0.0, 0.0])
                }
                BoneHeuristic::Temperance => self.temperance(id, parent_rot, true),
                BoneHeuristic::Fortune => self.temperance(id, parent_rot, false),
            };
            if let Some(r) = rot {
                self.rotate_edit_bone(id, r);
            }
            rot_of.insert(id, rot);
        }
    }

    /// `calc_bone_matrices`: bone → armature space, bind pose and edit bones.
    fn calc_bone_matrices(&mut self) {
        use crate::export::vtree::matmul;
        for id in self.dfs() {
            if self.nodes[id].kind != VKind::Bone {
                continue;
            }
            let (pb, pe) = match self.nodes[id].parent {
                Some(p) if self.nodes[p].kind == VKind::Bone => {
                    (self.nodes[p].bind_arma_mat, self.nodes[p].editbone_arma_mat)
                }
                _ => (Mat4::IDENTITY, Mat4::IDENTITY),
            };
            let v = &self.nodes[id];
            let b = matmul(&pb, &tr_matrix(v.bind_trans, v.bind_rot));
            let e = matmul(&pe, &tr_matrix(v.editbone_trans, v.editbone_rot));
            self.nodes[id].bind_arma_mat = b;
            self.nodes[id].editbone_arma_mat = e;
        }
    }

    /// The bones of armature vnode `arma`, depth first (`create_bones`' `bone_ids`).
    pub fn bones_of(&self, arma: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut stack: Vec<usize> = self.nodes[arma].children.iter().rev().copied().collect();
        let mut seen = vec![false; self.nodes.len()];
        while let Some(id) = stack.pop() {
            if id >= seen.len() || std::mem::replace(&mut seen[id], true) {
                continue;
            }
            if self.nodes[id].kind != VKind::Bone {
                continue;
            }
            out.push(id);
            stack.extend(self.nodes[id].children.iter().rev().copied());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_permutation_snaps_each_row() {
        // 30° about Z: rows (0.87, -0.5, 0), (0.5, 0.87, 0), (0, 0, 1) → identity.
        let a = 30f32.to_radians() / 2.0;
        let q = [a.cos(), 0.0, 0.0, a.sin()];
        assert_eq!(nearby_signed_perm_matrix(q), Mat3::IDENTITY);
        // 120° about Z: row 0 (-0.5, -0.87, 0) → (0, -1, 0).
        let a = 120f32.to_radians() / 2.0;
        let q = [a.cos(), 0.0, 0.0, a.sin()];
        let m = nearby_signed_perm_matrix(q);
        assert_eq!(m.y_axis.x, -1.0);
        assert_eq!(m.x_axis.y, 1.0);
        assert_eq!(m.z_axis.z, 1.0);
    }

    #[test]
    fn translation_rotation_matrix() {
        let m = tr_matrix([1.0, 2.0, 3.0], QUAT_ONE);
        assert_eq!(m, Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0)));
    }
}
