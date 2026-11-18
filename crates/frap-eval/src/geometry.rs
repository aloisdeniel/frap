// SPDX-License-Identifier: GPL-3.0-or-later
//! The modifier stack of one object (§7.3 mesh pipeline, after the shape-key mix): the
//! [`frap_modifiers::ModEvalCx`] implementation and the call into
//! [`frap_modifiers::stack::evaluate`].
//!
//! Created by M4 W0 so that evaluators registered by WPs 4.2–4.6 run as soon as they land;
//! WP 4.1 owns this file (and `cache/{mesh,fingerprint}.rs`, reassigned) and completes it:
//! `depends_on` targets in the fingerprint, edit-mode cage rules, image caching.
//! Dependency cycles between modifier operands are broken here: an object whose stack is
//! being evaluated is not evaluated again through an operand (`ModError::Dependency`).

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

/// What the stack produced for one object.
pub(crate) struct StackOut {
    pub mesh: Arc<Mesh>,
    pub modifier_status: Vec<(SmolStr, ModifierStatus)>,
    pub warnings: Vec<Report>,
}

/// Evaluates the modifier stack of `ob` on `base`.
pub(crate) fn evaluate(
    ev: &mut EvalCache,
    host: &dyn HostRead,
    ob_ref: IdRef<ObjectId>,
    ob: &Object,
    base: Arc<Mesh>,
    mode: EvalMode,
    in_edit_mode: bool,
) -> StackOut {
    if ob.modifiers.is_empty() {
        return StackOut {
            mesh: base,
            modifier_status: Vec::new(),
            warnings: Vec::new(),
        };
    }
    ev.in_progress.push(ob_ref.key());
    let mut cx = GeoCx {
        ev,
        host,
        ob_ref,
        ob,
        mode,
        reports: Vec::new(),
    };
    let r = frap_modifiers::stack::evaluate(base, in_edit_mode, &mut cx);
    let mut warnings = std::mem::take(&mut cx.reports);
    cx.ev.in_progress.retain(|k| *k != ob_ref.key());
    warnings.extend(r.warnings);
    StackOut {
        mesh: r.mesh,
        modifier_status: r.modifier_status,
        warnings,
    }
}

/// [`ModEvalCx`] over the evaluation cache and the host state.
struct GeoCx<'e, 'h> {
    ev: &'e mut EvalCache,
    host: &'h dyn HostRead,
    ob_ref: IdRef<ObjectId>,
    ob: &'h Object,
    mode: EvalMode,
    reports: Vec<Report>,
}

impl GeoCx<'_, '_> {
    fn cycle(&self, ob: IdRef<ObjectId>) -> bool {
        self.ev.in_progress.contains(&ob.key())
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
        if ob == self.ob_ref || self.cycle(ob) {
            // The object's own matrix does not depend on its geometry.
            if let Some(o) = self.host.main().objects.get(ob) {
                return crate::world_matrix(o, None);
            }
        }
        self.ev
            .object(self.host, ob, self.mode)
            .map(|r| r.matrix_world)
            .unwrap_or(Mat4::IDENTITY)
    }

    fn pose(&mut self, arm: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError> {
        let _ = arm;
        Err(ModError::Dependency(
            "frap: armature poses are evaluated from M6".into(),
        ))
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

/// Keys of the objects whose stacks are being evaluated (cycle guard).
pub(crate) type InProgress = Vec<IdKey>;
