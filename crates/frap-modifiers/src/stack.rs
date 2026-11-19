// SPDX-License-Identifier: GPL-3.0-or-later
//! The modifier stack: the evaluation walk (§7.3 "mesh pipeline per object") and, in
//! [`edit`], the stack editing rules every front end shares (`modifiers.new/remove/move/clear`,
//! the `object.modifier_*` operators, `use_pin_to_last`, `is_active`).
//!
//! Ported from blender/source/blender/blenkernel/intern/mesh_data_update.cc
//! (`mesh_calc_modifiers`, `editbmesh_modifier_is_enabled`) and
//! blender/source/blender/blenkernel/intern/modifier.cc (`BKE_modifier_is_enabled`), Blender
//! 5.2.4, GPL-2.0-or-later:
//! - modifiers run in list order (the `use_pin_to_last` setter keeps pinned modifiers at the
//!   end of the list, [`edit::set_pin_to_last`]; [`stack_order`] also puts them last for a
//!   stack built in Rust or loaded from an older file);
//! - a modifier takes part when its mode flags are on (`show_viewport`, `show_render` in Render
//!   mode) and, for an object in Edit Mode (`required_mode |= eModifierMode_Editmode`, viewport
//!   and render alike), `show_in_editmode` is on **and** its type supports Edit Mode;
//! - `is_disabled` skips a modifier silently (status [`ModifierStatus::Disabled`]);
//! - a `RequiresOriginalData` type after a non-deforming modifier is skipped with Blender's
//!   modifier error `Modifier requires original data, bad stack position`;
//! - an evaluator error is the modifier's error (WARNING report, the stack continues with the
//!   input mesh, as `BKE_modifier_set_error` does);
//! - a type without a frap evaluator passes the mesh through with a WARNING (§7.2).

pub mod edit;

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::{Report, ReportType};
use frap_scene::{EvalMode, ModifierStatus};
use smol_str::SmolStr;

use crate::{ModEvalCx, ModKind, ModTypeFlags, Modifier, ModifierType, registry};

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

/// The modifiers in evaluation order: stack order, pinned modifiers last (stable). With the
/// list kept as Blender keeps it (pinned modifiers at the end) this is the list order.
pub fn stack_order(mods: &[Modifier]) -> impl Iterator<Item = &Modifier> {
    mods.iter()
        .filter(|m| !m.use_pin_to_last)
        .chain(mods.iter().filter(|m| m.use_pin_to_last))
}

/// The type of a modifier (`None` for an identifier frap does not know).
pub fn type_of(m: &Modifier) -> Option<ModifierType> {
    ModifierType::from_identifier(m.settings.type_identifier())
}

/// Whether a modifier takes part in this evaluation by its mode flags
/// (`BKE_modifier_is_enabled` without `is_disabled`): `show_viewport` (`show_render` in Render
/// mode); in Edit Mode `show_in_editmode` and a type that supports Edit Mode
/// (`eModifierTypeFlag_SupportsEditmode`) as well.
pub fn enabled(m: &Modifier, mode: EvalMode, in_edit_mode: bool) -> bool {
    if in_edit_mode {
        // `editbmesh_calc_modifiers`: `eModifierMode_Realtime | eModifierMode_Editmode`.
        let supports = type_of(m).is_some_and(|t| {
            registry::meta(t)
                .flags
                .contains(ModTypeFlags::SUPPORTS_EDITMODE)
        });
        return m.show_viewport && m.show_in_editmode && supports;
    }
    match mode {
        EvalMode::Viewport => m.show_viewport,
        EvalMode::Render => m.show_render,
    }
}

/// Blender's modifier error for a `RequiresOriginalData` type after a non-deforming modifier.
pub const BAD_STACK_POSITION: &str = "Modifier requires original data, bad stack position";

/// Evaluates the stack of `cx.object()` on `base`.
pub fn evaluate(base: Arc<Mesh>, in_edit_mode: bool, cx: &mut dyn ModEvalCx) -> StackResult {
    let mode = cx.mode();
    let mods: Vec<Modifier> = cx.object().modifiers.clone();
    let mut mesh = base;
    let mut modifier_status = Vec::with_capacity(mods.len());
    let mut warnings = Vec::new();
    let mut non_deform_applied = false;
    for m in stack_order(&mods) {
        let ty = type_of(m);
        let meta = ty.map(registry::meta);
        let info = ty.and_then(registry::find);
        let status = if !enabled(m, mode, in_edit_mode) {
            ModifierStatus::Skipped
        } else if let Some(why) = info.and_then(|i| (i.is_disabled)(&m.settings)) {
            ModifierStatus::Disabled(why.to_owned())
        } else if non_deform_applied
            && meta.is_some_and(|t| t.flags.contains(ModTypeFlags::REQUIRES_ORIGINAL_DATA))
        {
            warnings.push(Report::new(
                ReportType::Warning,
                format!("Modifier '{}': {BAD_STACK_POSITION}", m.name),
            ));
            ModifierStatus::Error(BAD_STACK_POSITION.to_owned())
        } else if let Some(info) = info {
            if meta.is_some_and(|t| t.kind != ModKind::OnlyDeform) {
                non_deform_applied = true;
            }
            let input = Arc::unwrap_or_clone(mesh.clone());
            match eval_within_limits(info, m, input, cx) {
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
        } else {
            if meta.is_some_and(|t| t.kind != ModKind::OnlyDeform) {
                non_deform_applied = true;
            }
            warnings.push(Report::new(ReportType::Warning, pass_through_message(m)));
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

/// Runs one evaluator. A result over `--max-elements` (or Blender's `int` index range) is
/// the modifier's error, whichever way the evaluator noticed it: an [`ElementsExceeded`]
/// unwind (a pre-check such as ARRAY's or BEVEL's, or the BMesh allocation backstop) or a
/// returned error (SCREW). The stack then passes the mesh through with a WARNING and
/// [`ModifierStatus::Error`], as Blender does for its own size caps (`MOD_array.cc`: "Array
/// too big"): one policy for every evaluator, so a read of the evaluated mesh succeeds and
/// names the modifier (M3+M4 stabilization round 2). A `--stmt-timeout` unwind still ends
/// the statement.
///
/// [`ElementsExceeded`]: frap_math::budget::ElementsExceeded
fn eval_within_limits(
    info: &crate::ModifierTypeInfo,
    m: &Modifier,
    input: Mesh,
    cx: &mut dyn ModEvalCx,
) -> Result<Mesh, crate::ModError> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (info.eval)(m, input, cx))) {
        Ok(r) => r,
        Err(payload) => match frap_math::budget::elements_exceeded(&*payload) {
            Some(e) => Err(crate::ModError::Error(e.to_string())),
            None => std::panic::resume_unwind(payload),
        },
    }
}

/// Whether a modifier error is `--max-elements` (or Blender's index range) refusing the
/// result ([`frap_math::budget::ElementsExceeded`]'s message).
pub fn is_limit_error(message: &str) -> bool {
    message.starts_with("frap: limit exceeded")
}

/// Whether a modifier error is a frap limitation rather than one Blender would report too: a
/// type without an evaluator, a node group frap cannot evaluate, an unsupported input
/// (messages starting with `frap:`). Front ends answer these with `NotImplementedError`
/// (`OP_NOT_IMPLEMENTED`) instead of Blender's error.
pub fn is_frap_limitation(message: &str) -> bool {
    message.starts_with("frap:") || message.starts_with("Unsupported node group")
}

/// Whether a stack status means the evaluated mesh is not what Blender computes: a type
/// without an evaluator, or an error that is a frap limitation ([`is_frap_limitation`]).
pub fn status_is_frap_limitation(status: &ModifierStatus) -> bool {
    match status {
        ModifierStatus::PassedThrough => true,
        ModifierStatus::Error(m) => is_frap_limitation(m),
        _ => false,
    }
}

/// The WARNING of a type without an evaluator (§7.2).
pub fn pass_through_message(m: &Modifier) -> String {
    format!(
        "Modifier '{}' ({}) is not evaluated by frap; passed through",
        m.name,
        m.settings.type_identifier()
    )
}

/// Evaluates one modifier alone on `base`, as `object.modifier_apply` does
/// (`create_applied_mesh_for_modifier`: the original mesh, never the stack before it; the
/// viewport mode flag is forced on, so `show_viewport` does not matter). `Ok(None)` when the
/// modifier is disabled (`is_disabled`); a type without an evaluator is an error.
pub fn evaluate_one(
    m: &Modifier,
    base: Mesh,
    cx: &mut dyn ModEvalCx,
) -> Result<Option<Mesh>, crate::ModError> {
    let Some(info) = type_of(m).and_then(registry::find) else {
        return Err(crate::ModError::Error(format!(
            "frap: modifier type {} has no evaluator yet; it cannot be applied",
            m.settings.type_identifier()
        )));
    };
    if (info.is_disabled)(&m.settings).is_some() {
        return Ok(None);
    }
    (info.eval)(m, base, cx).map(Some)
}
