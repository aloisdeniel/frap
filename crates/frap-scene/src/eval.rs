// SPDX-License-Identifier: GPL-3.0-or-later
//! Evaluation results (ARCHITECTURE §7.3, CONTRACT frozen at M1 W0). Computed by `frap-eval`
//! behind the `Evaluator` trait (`host.rs`); defined here so lower layers can name them.

use std::sync::Arc;

use frap_mesh::Mesh;
use frap_rna::Report;
use glam::Mat4;
use smol_str::SmolStr;

/// Evaluated pose of an armature object (M6: per-bone pose matrices).
#[derive(Clone, Debug, Default)]
pub struct EvaluatedPose {
    /// `(bone name, pose matrix in armature space)` in bone order.
    pub bones: Vec<(SmolStr, Mat4)>,
}

/// What happened to one modifier during evaluation (`--verbosity full`, `--describe`).
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ModifierStatus {
    /// Evaluated by frap.
    Evaluated,
    /// Skipped (`show_viewport`/`show_render`/`show_in_editmode` off).
    Skipped,
    /// No evaluator in frap: passed through with a WARNING (§7.2).
    PassedThrough,
    /// `is_disabled` (e.g. "Boolean: no operand").
    Disabled(String),
    /// Evaluation failed.
    Error(String),
}

/// An evaluated object (depsgraph-lite result, §7.3).
#[derive(Clone, Debug)]
pub struct EvaluatedObject {
    /// `matrix_world` (parents and `matrix_parent_inverse` applied).
    pub matrix_world: Mat4,
    /// Evaluated mesh (shape keys + modifiers); shares the original `Arc` without modifiers.
    pub mesh: Option<Arc<Mesh>>,
    /// Local-space bounds (`bound_box`, `dimensions`).
    pub bounds: Option<[[f32; 3]; 2]>,
    /// Evaluated pose (armatures).
    pub pose: Option<Arc<EvaluatedPose>>,
    /// Per-modifier status by modifier name.
    pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    /// Evaluation warnings (reports).
    pub warnings: Vec<Report>,
}
