// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh pipeline of one object (§7.3): base mesh (the edit BMesh view in Edit Mode, the
//! original otherwise) → shape-key mix (M6) → modifiers in stack order with `use_pin_to_last`
//! last (`crate::geometry` → `frap_modifiers::stack`, M4 W0): a type without an evaluator
//! passes the mesh through with Blender-style WARNING (§7.2); without modifiers the evaluated
//! mesh is the base mesh's own `Arc`. In Edit Mode only the modifiers shown in Edit Mode whose
//! type supports it take part (`editbmesh_calc_modifiers`, WP 4.1, `stack::enabled`).

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdCode, IdRef, MeshId, ObjectId, Report};
use frap_scene::id::Object;
use frap_scene::{EvalMode, HostRead, ModifierStatus};
use smol_str::SmolStr;

/// The evaluated geometry of one object.
pub(crate) struct MeshResult {
    pub mesh: Option<Arc<Mesh>>,
    pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    pub warnings: Vec<Report>,
}

/// The object's mesh datablock, when its data is an existing mesh.
pub(crate) fn mesh_of(host: &dyn HostRead, ob: &Object) -> Option<IdRef<MeshId>> {
    let d = ob.data?;
    if d.code != IdCode::ME {
        return None;
    }
    let me = d.typed::<MeshId>()?;
    host.main().meshes.get(me).is_some().then_some(me)
}

/// Evaluates the geometry of `ob` (a mesh object; `None` otherwise): the base mesh, then the
/// modifier stack (`crate::geometry`, M4 W0).
pub(crate) fn evaluate(
    ev: &mut super::EvalCache,
    host: &dyn HostRead,
    ob_ref: IdRef<ObjectId>,
    ob: &Object,
    mode: EvalMode,
) -> MeshResult {
    let Some(me) = mesh_of(host, ob) else {
        return MeshResult {
            mesh: None,
            modifier_status: Vec::new(),
            warnings: Vec::new(),
        };
    };
    let in_edit_mode = host.edit().meshes.contains_key(&me.key());
    // Edit Mode: the cached read-only view of the edit BMesh (§6.3); otherwise the original.
    let base = host.mesh_view(me);
    // Shape keys before the modifiers (`key_eval.rs`, WP 6.3).
    let base = crate::key_eval::apply(host, ob, base);
    let out = crate::geometry::evaluate(ev, host, ob_ref, ob, base, mode, in_edit_mode);
    MeshResult {
        mesh: Some(out.mesh),
        modifier_status: out.modifier_status,
        warnings: out.warnings,
    }
}
