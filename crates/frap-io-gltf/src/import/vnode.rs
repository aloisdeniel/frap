// SPDX-License-Identifier: GPL-3.0-or-later
//! The virtual node tree: glTF nodes copied into a tree of `VNode`s, then transformed into a
//! shape Blender can hold (one data type per object, cameras and lights rotated to Blender's
//! conventions).
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/vnode.py` (Apache-2.0, The glTF-Blender-IO
//! authors): `compute_vnodes`, `init_vnodes`, `fixup_multitype_nodes`,
//! `correct_cameras_and_lights`, `local_rotation`, `VNode.trs`. The armature passes
//! (`mark_bones_and_armas`, `move_skinned_meshes`, `pick_bind_pose`, `prettify_bones`,
//! `calc_bone_matrices`) arrive with skins in M6; EXT_mesh_gpu_instancing (`manage_gpu_instancing`)
//! in M7. Python's dict of vnodes keyed by mixed ids (`3`, `'root'`, `'3.camera'`) is an arena
//! here, in the same insertion order.

use frap_gltf::Document;
use frap_math::quat::QuatWxyz;
use frap_rna::{IdRef, ObjectId};

use super::convert::{Units, camera_correction, qconj, qmul, qrot, quat, scale, scale_rot_swap};

const QUAT_ONE: QuatWxyz = [1.0, 0.0, 0.0, 0.0];

/// `VNode.type`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum VKind {
    /// `VNode.Object`
    Object,
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
    /// Base TRS in Blender space.
    pub base_trs: ([f32; 3], QuatWxyz, [f32; 3]),
    /// Rotation applied after the base TRS (children of corrected cameras/lights).
    pub rotation_after: QuatWxyz,
    /// Rotation applied before the base TRS (camera/light correction).
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
    /// The created object.
    pub blender_object: Option<IdRef<ObjectId>>,
}

impl VNode {
    fn new(default_name: String) -> VNode {
        VNode {
            name: None,
            default_name,
            children: Vec::new(),
            parent: None,
            kind: VKind::Object,
            base_trs: ([0.0; 3], QUAT_ONE, [1.0; 3]),
            rotation_after: QUAT_ONE,
            rotation_before: QUAT_ONE,
            mesh_node_idx: None,
            camera_node_idx: None,
            light_node_idx: None,
            scenes: Vec::new(),
            gltf_node: None,
            blender_object: None,
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

impl VTree {
    /// `compute_vnodes` (static subset: see the module docs).
    pub fn compute(doc: &Document, units: Units) -> VTree {
        let mut t = VTree::init(doc, units);
        t.fixup_multitype_nodes();
        t.correct_cameras_and_lights();
        t
    }

    /// `init_vnodes` (the Blender scene/collection bookkeeping it also does lives in
    /// `scenes.rs`).
    fn init(doc: &Document, units: Units) -> VTree {
        let mut nodes: Vec<VNode> = Vec::with_capacity(doc.nodes.len() + 1);
        for (i, n) in doc.nodes.iter().enumerate() {
            let mut v = VNode::new(format!("Node_{i}"));
            v.name = n.name.clone();
            v.children = n.children.iter().map(|&c| c as usize).collect();
            v.base_trs = node_trs(units, n);
            v.gltf_node = Some(i);
            if n.mesh.is_some() {
                v.mesh_node_idx = Some(i);
            }
            if n.camera.is_some() {
                v.camera_node_idx = Some(i);
            }
            if n.extensions.contains("KHR_lights_punctual") {
                v.light_node_idx = Some(i);
            }
            nodes.push(v);
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

    /// `fixup_multitype_nodes`: an object holds one of mesh, camera, light; extra data moves to
    /// new children (`'<id>.camera'`, `'<id>.light'`). Bones and armatures arrive in M6.
    fn fixup_multitype_nodes(&mut self) {
        let ids: Vec<usize> = (0..self.nodes.len()).collect();
        for id in ids {
            let mut needs_move = false;
            if self.nodes[id].mesh_node_idx.is_some() {
                needs_move = true;
            }
            if let Some(cam) = self.nodes[id].camera_node_idx {
                if needs_move {
                    let mut v = VNode::new("Node".into());
                    v.camera_node_idx = Some(cam);
                    self.push_child(id, v);
                    self.nodes[id].camera_node_idx = None;
                }
                needs_move = true;
            }
            if let Some(light) = self.nodes[id].light_node_idx
                && needs_move
            {
                let mut v = VNode::new("Node".into());
                v.light_node_idx = Some(light);
                self.push_child(id, v);
                self.nodes[id].light_node_idx = None;
            }
        }
    }

    /// `correct_cameras_and_lights` (+ `local_rotation`).
    fn correct_cameras_and_lights(&mut self) {
        let rot = camera_correction();
        for id in 0..self.nodes.len() {
            let v = &self.nodes[id];
            if v.camera_node_idx.is_none() && v.light_node_idx.is_none() {
                continue;
            }
            self.nodes[id].rotation_before = qmul(self.nodes[id].rotation_before, rot);
            let rot_inv = qconj(rot);
            for c in self.nodes[id].children.clone() {
                self.nodes[c].rotation_after = qmul(rot_inv, self.nodes[c].rotation_after);
            }
        }
    }
}
