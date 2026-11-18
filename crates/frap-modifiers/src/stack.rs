// SPDX-License-Identifier: GPL-3.0-or-later
//! The modifier stack walk (§7.3 "mesh pipeline per object"): modifiers in stack order with
//! `use_pin_to_last` ones last, skipping disabled ones (`show_viewport` / `show_render`,
//! `show_in_editmode` in Edit Mode), each through its registered evaluator; types without one
//! pass the mesh through with a WARNING.
//!
//! M4 W0 wrote this walk so the evaluators of WPs 4.2–4.6 run as soon as they register (the
//! fingerprint of `depends_on`, `is_disabled` reports, edit-mode cage rules and the rest of
//! `BKE_modifier_*` are WP 4.1's, which owns this file).

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{Report, ReportType};
use frap_scene::{EvalMode, ModifierStatus};
use smol_str::SmolStr;

use crate::{ModEvalCx, Modifier, ModifierType, registry};

/// The evaluated geometry and what happened to each modifier.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct StackResult {
    /// The evaluated mesh (the base mesh's own `Arc` when no modifier changed it).
    pub mesh: Arc<Mesh>,
    /// Per-modifier status by name, in evaluation order.
    pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    /// WARNINGs (pass-through types, modifier errors).
    pub warnings: Vec<Report>,
}

/// The modifiers in evaluation order: stack order, pinned modifiers last (stable).
pub fn stack_order(mods: &[Modifier]) -> impl Iterator<Item = &Modifier> {
    mods.iter()
        .filter(|m| !m.use_pin_to_last)
        .chain(mods.iter().filter(|m| m.use_pin_to_last))
}

/// Whether a modifier takes part in this evaluation (`BKE_modifier_is_enabled`'s mode flags).
pub fn enabled(m: &Modifier, mode: EvalMode, in_edit_mode: bool) -> bool {
    match mode {
        EvalMode::Viewport => m.show_viewport && (!in_edit_mode || m.show_in_editmode),
        EvalMode::Render => m.show_render,
    }
}

/// Evaluates the stack of `cx.object()` on `base`.
pub fn evaluate(base: Arc<Mesh>, in_edit_mode: bool, cx: &mut dyn ModEvalCx) -> StackResult {
    let mode = cx.mode();
    let mods: Vec<Modifier> = cx.object().modifiers.clone();
    let mut mesh = base;
    let mut modifier_status = Vec::with_capacity(mods.len());
    let mut warnings = Vec::new();
    for m in stack_order(&mods) {
        let ty = ModifierType::from_identifier(m.settings.type_identifier());
        let info = ty.and_then(registry::find);
        let status = if !enabled(m, mode, in_edit_mode) {
            ModifierStatus::Skipped
        } else if let Some(info) = info {
            if let Some(why) = (info.is_disabled)(&m.settings) {
                ModifierStatus::Disabled(why.to_owned())
            } else {
                let input = Arc::unwrap_or_clone(mesh.clone());
                match (info.eval)(m, input, cx) {
                    Ok(out) => {
                        mesh = Arc::new(out);
                        ModifierStatus::Evaluated
                    }
                    Err(e) => {
                        warnings.push(Report::new(
                            ReportType::Warning,
                            format!("Modifier '{}': {e}", m.name),
                        ));
                        ModifierStatus::Error(e.to_string())
                    }
                }
            }
        } else {
            warnings.push(Report::new(
                ReportType::Warning,
                format!(
                    "Modifier '{}' ({}) is not evaluated by frap; passed through",
                    m.name,
                    m.settings.type_identifier()
                ),
            ));
            ModifierStatus::PassedThrough
        };
        modifier_status.push((m.name.clone(), status));
    }
    StackResult {
        mesh,
        modifier_status,
        warnings,
    }
}
