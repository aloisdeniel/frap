// SPDX-License-Identifier: GPL-3.0-or-later
//! The modifier stack of one object (§7.3 mesh pipeline, after the shape-key mix): the
//! [`frap_modifiers::ModEvalCx`] implementation and the call into
//! [`frap_modifiers::stack::evaluate`].
//!
//! Created by M4 W0 so that evaluators registered by WPs 4.2–4.6 run as soon as they land;
//! WP 4.1 completed it: the stack walk's Edit Mode rules live in `frap_modifiers::stack`, the
//! `depends_on` targets are part of the fingerprint (`cache/fingerprint.rs`), and
//! [`EvalCache::apply_modifier`] evaluates one modifier alone on the original mesh for
//! `object.modifier_apply`. Images are decoded per request (no cache: DISPLACE reads them once
//! per evaluation, and the evaluation result itself is cached by fingerprint).
//! Dependency cycles between modifier operands are broken here: an object whose stack is
//! being evaluated is not evaluated again through an operand (`ModError::Dependency`). Its
//! world matrix is known before its geometry, so an operand that only needs its transform (a
//! child of it: an armature parented to the mesh it deforms, a SIMPLE_DEFORM origin parented
//! to the mesh) reads the in-progress matrix ([`InProgress`], `cache.rs`) instead of
//! evaluating the object again (WP 6.5).
//! Every stack evaluation is counted per thread ([`EvalCache::stack_evaluations`], M8 WP 8.8):
//! the §21.7 budget "a property read in Edit Mode never re-evaluates modifiers" is a test on it.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use frap_boolean::BooleanSolver;
use frap_imbuf::{DecodedImage, PixelData};
use frap_mesh::Mesh;
use frap_modifiers::{EvaluatedPose, ModError, ModEvalCx};
use frap_rna::{IdKey, IdRef, ImageId, ObjectId, Report};
use frap_scene::id::{Image, Object};
use frap_scene::{EvalMode, Evaluator, HostRead, Main, ModifierStatus};
use frap_subdiv::Subdivider;
use glam::Mat4;
use smol_str::SmolStr;

use crate::EvalCache;

thread_local! {
    /// Modifier-stack evaluations run on this thread ([`EvalCache::stack_evaluations`]).
    static STACK_EVALUATIONS: Cell<u64> = const { Cell::new(0) };
}

fn count_stack_evaluation() {
    STACK_EVALUATIONS.with(|c| c.set(c.get() + 1));
}

/// What the stack produced for one object.
pub(crate) struct StackOut {
    pub mesh: Arc<Mesh>,
    pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    pub warnings: Vec<Report>,
}

/// Evaluates the modifier stack of `ob` on `base`, after the armature-parent virtual modifier
/// (`parent_type = 'ARMATURE'`, WP 6.5; not in Edit Mode, where the virtual modifier is not
/// enabled). `matrix_world` is the object's evaluated world matrix (deform modifiers read it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate(
    ev: &mut EvalCache,
    host: &dyn HostRead,
    ob_ref: IdRef<ObjectId>,
    ob: &Object,
    base: Arc<Mesh>,
    mode: EvalMode,
    in_edit_mode: bool,
    matrix_world: Mat4,
) -> StackOut {
    let virtual_arm = (!in_edit_mode)
        .then(|| frap_modifiers::types::armature::virtual_parent(host.main(), ob))
        .flatten();
    if ob.modifiers.is_empty() && virtual_arm.is_none() {
        return StackOut {
            mesh: base,
            modifier_status: Vec::new(),
            warnings: Vec::new(),
        };
    }
    count_stack_evaluation();
    ev.in_progress.push(ob_ref.key(), matrix_world);
    let mut cx = GeoCx {
        ev,
        host,
        ob_ref,
        ob,
        mode,
        own_world: matrix_world,
        reports: Vec::new(),
        prev_coords: None,
    };
    // A modifier may unwind (TIMEOUT, LIMIT_EXCEEDED); the cache outlives the statement, so the
    // cycle guard is cleared before the unwind goes on (else a later read reports a false cycle).
    let r = guarded(&mut cx, |cx| {
        let base = match virtual_arm {
            Some(arm) => frap_modifiers::types::armature::evaluate_virtual(base, arm, cx),
            None => base,
        };
        frap_modifiers::stack::evaluate(base, in_edit_mode, cx)
    });
    let mut warnings = std::mem::take(&mut cx.reports);
    cx.ev.in_progress.remove(ob_ref.key());
    warnings.extend(r.warnings);
    StackOut {
        mesh: r.mesh,
        modifier_status: r.modifier_status,
        warnings,
    }
}

/// What `object.modifier_apply` gets from [`EvalCache::apply_modifier`].
#[derive(Clone, Debug)]
pub struct AppliedModifier {
    /// The modifier's result on the original mesh (`None`: the modifier is disabled,
    /// `is_disabled`).
    pub mesh: Option<Mesh>,
    /// Reports of the evaluation (dependency problems, …).
    pub reports: Vec<Report>,
}

impl EvalCache {
    /// How many modifier stacks this thread has evaluated so far: every evaluation of an
    /// object with modifiers (or an armature parent) that missed the cache, and every
    /// [`EvalCache::apply_modifier`], whatever cache ran it. A diagnostic for the §21.7 budget
    /// tests (`frap-perf-tests`), counted per thread so parallel tests do not see each other's
    /// work.
    pub fn stack_evaluations() -> u64 {
        STACK_EVALUATIONS.with(Cell::get)
    }

    /// Evaluates modifier `index` of `ob` **alone on the original mesh**, as
    /// `create_applied_mesh_for_modifier` does for `object.modifier_apply` (the stack before it
    /// is ignored; viewport settings; `show_viewport` does not matter). Operand objects are
    /// evaluated through this cache. `Err` is the modifier's error.
    pub fn apply_modifier(
        &mut self,
        host: &dyn HostRead,
        ob_ref: IdRef<ObjectId>,
        index: usize,
    ) -> Result<AppliedModifier, ModError> {
        let main = host.main();
        let ob = main
            .objects
            .get(ob_ref)
            .ok_or_else(|| ModError::Error("frap: the object was removed".into()))?;
        let m = ob
            .modifiers
            .get(index)
            .ok_or_else(|| ModError::Error("frap: no such modifier".into()))?;
        let base = ob
            .data
            .and_then(|d| d.typed::<frap_rna::MeshId>())
            .and_then(|me| main.meshes.get(me))
            .cloned()
            .ok_or_else(|| ModError::Error("frap: the object has no mesh".into()))?;
        let own_world = self
            .object(host, ob_ref, EvalMode::Viewport)
            .map_or_else(|_| crate::world_matrix(ob, None), |r| r.matrix_world);
        count_stack_evaluation();
        self.in_progress.push(ob_ref.key(), own_world);
        let mut cx = GeoCx {
            ev: self,
            host,
            ob_ref,
            ob,
            mode: EvalMode::Viewport,
            own_world,
            reports: Vec::new(),
            prev_coords: None,
        };
        let r = guarded(&mut cx, |cx| {
            frap_modifiers::stack::evaluate_one(m, base, cx)
        });
        let reports = std::mem::take(&mut cx.reports);
        self.in_progress.remove(ob_ref.key());
        r.map(|mesh| AppliedModifier { mesh, reports })
    }
}

/// Runs `f`; if it unwinds, removes `cx.ob_ref` from the cycle guard first and resumes the unwind.
fn guarded<R>(cx: &mut GeoCx<'_, '_>, f: impl FnOnce(&mut GeoCx<'_, '_>) -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&mut *cx))) {
        Ok(r) => r,
        Err(payload) => {
            cx.ev.in_progress.remove(cx.ob_ref.key());
            std::panic::resume_unwind(payload)
        }
    }
}

/// [`ModEvalCx`] over the evaluation cache and the host state.
struct GeoCx<'e, 'h> {
    ev: &'e mut EvalCache,
    host: &'h dyn HostRead,
    ob_ref: IdRef<ObjectId>,
    ob: &'h Object,
    mode: EvalMode,
    /// The evaluated `matrix_world` of `ob` (computed before its geometry, WP 6.5).
    own_world: Mat4,
    reports: Vec<Report>,
    /// `ArmatureModifierData::vert_coords_prev` of this evaluation (WP 6.5).
    prev_coords: Option<Vec<[f32; 3]>>,
}

impl GeoCx<'_, '_> {
    fn cycle(&self, ob: IdRef<ObjectId>) -> bool {
        self.ev.in_progress.contains(ob.key())
    }
}

impl ModEvalCx for GeoCx<'_, '_> {
    fn main(&self) -> &Main {
        self.host.main()
    }

    fn object(&self) -> &Object {
        self.ob
    }

    fn object_ref(&self) -> IdRef<ObjectId> {
        self.ob_ref
    }

    fn evaluated_mesh(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<Mesh>, ModError> {
        if self.cycle(ob) {
            return Err(ModError::Dependency(
                "Dependency cycle detected between modifier operands".into(),
            ));
        }
        let r = self
            .ev
            .object(self.host, ob, self.mode)
            .map_err(|e| ModError::Dependency(e.message))?;
        r.mesh
            .clone()
            .ok_or_else(|| ModError::Dependency("the object has no mesh".into()))
    }

    fn object_matrix_world(&mut self, ob: IdRef<ObjectId>) -> Mat4 {
        if ob == self.ob_ref {
            return self.own_world;
        }
        // An object whose stack is being evaluated: its matrix is known before its geometry.
        if let Some(w) = self.ev.in_progress.world(ob.key()) {
            return w;
        }
        self.ev
            .object(self.host, ob, self.mode)
            .map(|r| r.matrix_world)
            .unwrap_or(Mat4::IDENTITY)
    }

    fn pose(&mut self, arm: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError> {
        // M5+M6 W0: the armature object's evaluated pose (`pose.rs`, WP 6.2) through the cache,
        // as `evaluated_mesh` reads operand geometry (the ARMATURE modifier, WP 6.5).
        if self.cycle(arm) {
            return Err(ModError::Dependency(
                "Dependency cycle detected between modifier operands".into(),
            ));
        }
        let r = self
            .ev
            .object(self.host, arm, self.mode)
            .map_err(|e| ModError::Dependency(e.message))?;
        r.pose
            .clone()
            .ok_or_else(|| ModError::Dependency("the object has no evaluated pose".into()))
    }

    fn image(&mut self, im: IdRef<ImageId>) -> Option<Arc<DecodedImage>> {
        let main = self.host.main();
        let image = main.images.get(im)?;
        decode(image, main.filepath.as_deref()).map(Arc::new)
    }

    fn mode(&self) -> EvalMode {
        self.mode
    }

    fn frame(&self) -> f32 {
        let host = self.host;
        host.main()
            .scenes
            .get(host.win().scene)
            .map_or(1.0, |s| s.frame_current as f32)
    }

    fn report(&mut self, r: Report) {
        if !self.reports.contains(&r) {
            self.reports.push(r);
        }
    }

    fn subdivider(&self) -> &dyn Subdivider {
        frap_subdiv::default_subdivider()
    }

    fn boolean_solver(&self) -> &dyn BooleanSolver {
        frap_boolean::default_solver()
    }

    fn armature_in_edit_mode(&self, arm: IdRef<ObjectId>) -> bool {
        let main = self.host.main();
        main.objects
            .get(arm)
            .and_then(|o| o.data)
            .and_then(|d| d.typed::<frap_rna::ArmatureId>())
            .is_some_and(|a| self.host.edit().armatures.contains_key(&a.key()))
    }

    fn prev_coords_slot(&mut self) -> Option<&mut Option<Vec<[f32; 3]>>> {
        Some(&mut self.prev_coords)
    }
}

/// Where an image file is read from (`//` relative to the saved file's folder).
fn resolve(filepath: &str, blend_path: Option<&str>) -> PathBuf {
    if let Some(rel) = filepath.strip_prefix("//") {
        let base = blend_path
            .and_then(|p| Path::new(p).parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        return base.join(rel);
    }
    PathBuf::from(filepath)
}

/// Decoded pixels of an image (packed bytes, a file, or a GENERATED fill), as the render
/// builder decodes them; `None` when it cannot be loaded.
fn decode(im: &Image, blend_path: Option<&str>) -> Option<DecodedImage> {
    if let Some(p) = &im.packed_file {
        return frap_imbuf::decode(&p.bytes).ok();
    }
    match im.source.as_str() {
        "FILE" | "SEQUENCE" | "MOVIE" | "TILED" if !im.filepath.is_empty() => {
            frap_imbuf::load(&resolve(&im.filepath, blend_path)).ok()
        }
        "GENERATED" => {
            let (w, h) = (
                im.generated_width.max(1) as u32,
                im.generated_height.max(1) as u32,
            );
            let n = (w as usize) * (h as usize);
            let mut px = Vec::with_capacity(n * 4);
            for _ in 0..n {
                px.extend_from_slice(&im.generated_color);
            }
            Some(DecodedImage {
                width: w,
                height: h,
                channels: 4,
                pixels: PixelData::F32(px),
            })
        }
        _ => None,
    }
}

/// The objects whose stacks are being evaluated (cycle guard), with their evaluated world
/// matrices (computed before their geometry).
#[derive(Clone, Debug, Default)]
pub(crate) struct InProgress(Vec<(IdKey, Mat4)>);

impl InProgress {
    pub(crate) fn push(&mut self, key: IdKey, world: Mat4) {
        self.0.push((key, world));
    }

    /// Forgets `key` (the innermost entry: entries are pushed and removed like a stack).
    pub(crate) fn remove(&mut self, key: IdKey) {
        if let Some(i) = self.0.iter().rposition(|(k, _)| *k == key) {
            self.0.remove(i);
        }
    }

    pub(crate) fn contains(&self, key: IdKey) -> bool {
        self.0.iter().any(|(k, _)| *k == key)
    }

    /// The world matrix of an object whose stack is being evaluated.
    pub(crate) fn world(&self, key: IdKey) -> Option<Mat4> {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| *k == key)
            .map(|(_, w)| *w)
    }
}
