// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh pipeline of one object (§7.3): base mesh (the edit BMesh view in Edit Mode, the
//! original otherwise) → shape-key mix (M6) → modifiers in stack order with
//! `use_pin_to_last` last. M1 has no modifier evaluators (they arrive with `frap-modifiers` in
//! M4), so every enabled modifier passes the mesh through with Blender-style WARNING (§7.2) and
//! the evaluated mesh is the base mesh's own `Arc`.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{IdCode, IdRef, MeshId, Report, ReportType};
use frap_scene::id::{Modifier, Object};
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

/// The modifiers in evaluation order: stack order, pinned modifiers last (stable).
fn stack_order(mods: &[Modifier]) -> impl Iterator<Item = &Modifier> {
    mods.iter()
        .filter(|m| !m.use_pin_to_last)
        .chain(mods.iter().filter(|m| m.use_pin_to_last))
}

/// Whether a modifier takes part in this evaluation (`BKE_modifier_is_enabled`'s mode flags).
fn enabled(m: &Modifier, mode: EvalMode, in_edit_mode: bool) -> bool {
    match mode {
        EvalMode::Viewport => m.show_viewport && (!in_edit_mode || m.show_in_editmode),
        EvalMode::Render => m.show_render,
    }
}

/// Evaluates the geometry of `ob` (a mesh object; `None` otherwise).
pub(crate) fn evaluate(host: &dyn HostRead, ob: &Object, mode: EvalMode) -> MeshResult {
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
    // Shape-key mixing (relative keys) arrives with M6 (`key_eval.rs`); `Key` has no blocks yet.
    let mut modifier_status = Vec::with_capacity(ob.modifiers.len());
    let mut warnings = Vec::new();
    for m in stack_order(&ob.modifiers) {
        let status = if !enabled(m, mode, in_edit_mode) {
            ModifierStatus::Skipped
        } else {
            let ty = m.settings.type_identifier();
            warnings.push(Report::new(
                ReportType::Warning,
                format!(
                    "Modifier '{}' ({ty}) is not evaluated by frap; passed through",
                    m.name
                ),
            ));
            ModifierStatus::PassedThrough
        };
        modifier_status.push((m.name.clone(), status));
    }
    MeshResult {
        mesh: Some(base),
        modifier_status,
        warnings,
    }
}
