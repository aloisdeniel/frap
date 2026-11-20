// SPDX-License-Identifier: GPL-3.0-or-later
//! Shape-key evaluation (WP 6.3): the `ShapeKey` virtual modifier Blender puts in front of the
//! stack of a mesh object with a key (`BKE_modifiers_get_virtual_modifierlist`,
//! `MOD_shapekey.cc` → `BKE_key_evaluate_object_ex`): relative keys mix every unmuted block's
//! `value × (block − its relative_key)`, weighted by the block's vertex group, onto the
//! reference key's data; `show_only_shape_key` shows the active block alone; absolute keys
//! interpolate by `eval_time` ([`frap_scene::id::Key::evaluate`]). Runs before the modifier
//! stack (`cache/mesh.rs`).
//!
//! In Edit Mode the virtual modifier runs only with `use_shape_key_edit_mode`
//! (`OB_SHAPE_EDIT_MODE`, off by default and not bound by frap yet): the edit BMesh positions
//! (the active block's, `BM_mesh_bm_from_me`) are shown as they are.
//!
//! Ported from blender/source/blender/blenkernel/intern/key.cc and
//! blender/source/blender/modifiers/intern/MOD_shapekey.cc (Blender 5.2.4, GPL-2.0-or-later).
//! Vertex-group weights follow `get_weights_array`: a group the mesh does not have, or a mesh
//! without weights, gives the block full influence.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_scene::HostRead;
use frap_scene::id::Object;

/// The base mesh of `ob` with its shape keys mixed in.
pub(crate) fn apply(host: &dyn HostRead, ob: &Object, base: Arc<Mesh>) -> Arc<Mesh> {
    let Some(key) = base.shape_keys.and_then(|k| host.main().shape_keys.get(k)) else {
        return base;
    };
    if key.key_blocks.is_empty() {
        return base;
    }
    if ob
        .data
        .is_some_and(|d| host.edit().meshes.contains_key(&d.key))
    {
        return base;
    }
    let weights = |name: &str| -> Option<Vec<f32>> {
        let g = base.vertex_group_names.iter().position(|n| n == name)? as u32;
        let dv = base.deform_verts()?;
        Some(
            dv.iter()
                .map(|d| frap_bmesh_weight(d, g))
                .collect::<Vec<f32>>(),
        )
    };
    let active = usize::try_from(ob.active_shape_key_index)
        .ok()
        .filter(|&a| a < key.key_blocks.len());
    match key.evaluate_mesh(base.verts_num(), active, ob.show_only_shape_key, &weights) {
        Some(pos) if pos.as_slice() != base.positions() => {
            let mut m = (*base).clone();
            m.set_positions(pos);
            Arc::new(m)
        }
        _ => base,
    }
}

/// `BKE_defvert_find_weight`.
fn frap_bmesh_weight(d: &frap_mesh::DeformVert, g: u32) -> f32 {
    d.weights
        .iter()
        .find(|(x, _)| *x == g)
        .map_or(0.0, |(_, w)| *w)
}
