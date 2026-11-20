// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF skin import (WP 6.8): armatures and bones from the joint hierarchy (`vnode.rs`:
//! `mark_bones_and_armas`, `move_skinned_meshes`, `pick_bind_pose`, `bone_heuristic`
//! `BLENDER`/`TEMPERANCE`/`FORTUNE`, `guess_original_bind_pose`), vertex groups and the
//! ARMATURE modifier on skinned meshes, ported from io_scene_gltf2 5.2.41
//! `blender/imp/skin.py` and `vnode.py`. WP 6.8 also owns `import/{vnode,nodes}.rs`
//! (reassigned).
//!
//! M5+M6 W0: [`IMPLEMENTED`] is `false`, so the importer keeps its WARNING ("skins are imported
//! from M6").

/// Whether skins are imported (WP 6.8 sets it).
pub(crate) const IMPLEMENTED: bool = false;
