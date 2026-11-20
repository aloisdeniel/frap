// SPDX-License-Identifier: GPL-3.0-or-later
//! Which glTF node each exported object (and, from WP 6.8, each bone) became: the contract the
//! M6 exporters share (M5+M6 W0). `nodes.rs` records every object node as it gathers it;
//! skins (WP 6.8) add the joint nodes of armature bones; animations (WP 6.7) and
//! `KHR_animation_pointer` (WP 6.9) resolve their channel targets through it. A `Gather` state
//! (`g.state::<NodeMap>()`), one per export, for the scene being gathered and every other.

use std::collections::HashMap;

use frap_gltf::json::Index;
use frap_rna::{IdRef, ObjectId, SceneId};
use smol_str::SmolStr;

/// Object / bone → glTF node index.
#[derive(Clone, Debug, Default)]
pub struct NodeMap {
    /// `(scene being gathered, object)` → node (an object linked into several scenes is
    /// gathered once per scene, M34S5-1).
    pub objects: HashMap<(IdRef<SceneId>, IdRef<ObjectId>), Index>,
    /// `(scene, armature object, bone name)` → joint node (WP 6.8).
    pub bones: HashMap<(IdRef<SceneId>, IdRef<ObjectId>, SmolStr), Index>,
}

impl NodeMap {
    /// The node of `ob` in `scene`.
    pub fn object(&self, scene: IdRef<SceneId>, ob: IdRef<ObjectId>) -> Option<Index> {
        self.objects.get(&(scene, ob)).copied()
    }
    /// The joint node of bone `bone` of armature object `ob` in `scene`.
    pub fn bone(&self, scene: IdRef<SceneId>, ob: IdRef<ObjectId>, bone: &str) -> Option<Index> {
        self.bones.get(&(scene, ob, SmolStr::new(bone))).copied()
    }
}
