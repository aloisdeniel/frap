// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.uv.unwrap(method, fill_holes, correct_aspect, use_subsurf_data,
//! use_original_bounds, margin_method, margin, no_flip, iterations, use_weights, weight_group,
//! weight_factor)`, tier T2 (`tests/golden/cases/uv/unwrap/*.py`). Port of `unwrap_exec`,
//! `unwrap_options_get`, `unwrap_options_sync_toolsettings`, `uvedit_unwrap` and
//! `uvedit_pack_islands_multi` (`editors/uvedit/uvedit_unwrap_ops.cc`), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Poll `ED_operator_uvmap` (a mesh in Edit Mode with faces). Every mesh in Edit Mode (unique
//! data) gets a UV map when it has none (`UVMap`, the new UVs selected); then its selected,
//! visible faces are cut into charts at seams and mesh boundaries, each chart is flattened
//! (pinned UVs are constraints), its scale averaged, and the islands are packed into the unit
//! square (rotation `ANY`, pinned islands ignored, `margin_method`/`margin`). The operator
//! always finishes; without any visible face (no UV map: no selected face) it cancels.
//!
//! **Remembered settings.** Like Blender, a property the caller does not pass is taken from
//! the scene's tool settings (`unwrapper`, `uvcalc_margin`, `uvcalc_flag`, …; the factory
//! scene unwraps `ANGLE_BASED` with `fill_holes`, a `bpy.data.scenes.new()` scene
//! `CONFORMAL` without), and a passed one is stored there for the next call (and for
//! `uv.pack_islands`' `margin`).
//!
//! **Reports** as Blender's: INFO `Object has non-uniform scale, …` / `Object has negative
//! scale, …`, `Subdivision Surface modifier needs to be first to work with unwrap`, WARNING
//! `Unwrap failed to solve N of M island(s), edge seams may need to be added` (a closed
//! cube without seams) or `Unwrap could not solve any island(s), …`. frap adds a WARNING when
//! the solve produced non-finite UVs (huge coordinates), which Blender writes silently.
//!
//! **Methods** (WP 8.1). `ANGLE_BASED` flattens each chart's angles with ABF++
//! (`frap_uv::abf`, falling back to the 3D angles when ABF++ does not converge, as Blender) and
//! builds the UVs with LSCM; `CONFORMAL` is LSCM alone; `MINIMUM_STRETCH` runs SLIM
//! (`frap_uv::slim`: `iterations`, `no_flip`, and with `use_weights` the vertex group
//! `weight_group` weighted by `weight_factor`; holes are always filled), as Blender's official
//! builds, which ship SLIM. A chart SLIM cannot solve (an edge shorter than 1e-5) gets zero
//! UVs and counts in the failure WARNING.
//!
//! **frap differences.** `use_subsurf_data` with a Subdivision Surface modifier first in the
//! stack and `use_original_bounds=True` (which stitches the selection first, `uv.stitch`)
//! answer `OP_NOT_IMPLEMENTED` (M8).

pub(crate) mod uvedit;

use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_uv::uv;
use frap_spec_ops_uv::uv::unwrap::{MarginMethod, Method};
use frap_uv::PackParams;
use frap_uv::slim::SlimOptions;
use smol_str::SmolStr;

use uvedit::{
    Handle, PackTarget, UVCALC_FILLHOLES, UVCALC_NO_ASPECT_CORRECT, UVCALC_UNWRAP_NO_FLIP,
    UVCALC_UNWRAP_USE_WEIGHTS, UVCALC_USESUBSURF, UnwrapOptions, UvOb, UvTs,
};

/// The operator's properties after `unwrap_options_sync_toolsettings`.
#[derive(Clone, Debug)]
struct Synced {
    method: Method,
    margin: f32,
    fill_holes: bool,
    correct_aspect: bool,
    use_subsurf: bool,
    weight_group: SmolStr,
    iterations: i32,
    weight_factor: f32,
    no_flip: bool,
    use_weights: bool,
}

/// `unwrap_options_sync_toolsettings`: unset properties come from the tool settings, set
/// ones are stored there.
fn sync_toolsettings(cx: &mut OpCtx, p: &uv::Unwrap) -> Synced {
    let scene = cx.ctx().win.scene;
    let mut ts = cx.ctx().tool_settings().clone();
    let before = ts.clone();
    let flag = |ts: &mut frap_scene::generated::plain::ToolSettings,
                prop: &frap_rna::Prop<bool>,
                bit: i32,
                flipped: bool|
     -> bool {
        if prop.is_set {
            if prop.value ^ flipped {
                ts.uvcalc_flag |= bit;
            } else {
                ts.uvcalc_flag &= !bit;
            }
            prop.value
        } else {
            (ts.uvcalc_flag & bit != 0) ^ flipped
        }
    };
    let method = if p.method.is_set {
        ts.unwrapper = p.method.value.identifier().into();
        p.method.value
    } else {
        Method::from_identifier(&ts.unwrapper).unwrap_or(Method::Conformal)
    };
    let margin = if p.margin.is_set {
        ts.uvcalc_margin = p.margin.value;
        p.margin.value
    } else {
        ts.uvcalc_margin
    };
    let iterations = if p.iterations.is_set {
        ts.uvcalc_iterations = p.iterations.value;
        p.iterations.value
    } else {
        ts.uvcalc_iterations
    };
    let weight_factor = if p.weight_factor.is_set {
        ts.uvcalc_weight_factor = p.weight_factor.value;
        p.weight_factor.value
    } else {
        ts.uvcalc_weight_factor
    };
    let weight_group = if p.weight_group.is_set {
        // `char uvcalc_weight_group[64]`.
        let mut g = p.weight_group.value.to_string();
        while g.len() > 63 {
            g.pop();
        }
        ts.uvcalc_weight_group = g.as_str().into();
        p.weight_group.value.clone()
    } else {
        ts.uvcalc_weight_group.clone()
    };
    let fill_holes = flag(&mut ts, &p.fill_holes, UVCALC_FILLHOLES, false);
    let correct_aspect = flag(&mut ts, &p.correct_aspect, UVCALC_NO_ASPECT_CORRECT, true);
    let use_subsurf = flag(&mut ts, &p.use_subsurf_data, UVCALC_USESUBSURF, false);
    let no_flip = flag(&mut ts, &p.no_flip, UVCALC_UNWRAP_NO_FLIP, false);
    let use_weights = flag(&mut ts, &p.use_weights, UVCALC_UNWRAP_USE_WEIGHTS, false);
    if ts != before {
        let (main, log) = cx.main_mut();
        if let Some(s) = main.scenes.get_mut(scene, log) {
            s.tool_settings = ts;
        }
    }
    Synced {
        method,
        margin,
        fill_holes,
        correct_aspect,
        use_subsurf,
        weight_group,
        iterations,
        weight_factor,
        no_flip,
        use_weights,
    }
}

/// `modifier_unwrap_state`: whether the object's first modifier is a Subdivision Surface
/// with levels > 0 (only then does `use_subsurf_data` apply).
fn subsurf_first(cx: &OpCtx, ob: frap_rna::IdRef<frap_rna::ObjectId>) -> bool {
    let Some(o) = cx.main().objects.get(ob) else {
        return false;
    };
    matches!(
        o.modifiers.first().map(|m| &m.settings),
        Some(frap_scene::id::ModifierSettings::Subsurf(s)) if s.levels > 0
    )
}

/// `mat4_to_size` and `is_negative_m4` of a world matrix.
fn scale_state(m: &[[f32; 4]; 4]) -> ([f32; 3], bool) {
    let len = |c: &[f32; 4]| (c[0] * c[0] + c[1] * c[1] + c[2] * c[2]).sqrt();
    let size = [len(&m[0]), len(&m[1]), len(&m[2])];
    let a = [m[0][0], m[0][1], m[0][2]];
    let b = [m[1][0], m[1][1], m[1][2]];
    let c = [m[2][0], m[2][1], m[2][2]];
    let det = a[0] * (b[1] * c[2] - b[2] * c[1]) - b[0] * (a[1] * c[2] - a[2] * c[1])
        + c[0] * (a[1] * b[2] - a[2] * b[1]);
    (size, det < 0.0)
}

/// `uvedit_have_selection` (`only_selected_uvs` false): a visible face (Blender: the mesh
/// without a UV map needs a selected face).
fn have_selection(bm: &frap_bmesh::BMesh, ts: UvTs) -> bool {
    uvedit::have_selection(bm, ts, false)
}

#[operator(
    poll = poll::EDITMESH,
    tier = T2,
    support = Partial(
        "use_subsurf_data with a Subdivision Surface modifier first and \
         use_original_bounds=True answer OP_NOT_IMPLEMENTED"
    )
)]
fn exec(cx: &mut OpCtx, p: uv::Unwrap) -> OpResult {
    uvedit::check_uvmap(cx)?;
    let objects = uvedit::edit_objects(cx);
    let synced = sync_toolsettings(cx, &p);
    let ts = UvTs::of(&cx.ctx());

    // `unwrap_options_get(op, nullptr, nullptr)` (no Image Editor: only selected faces).
    let method = synced.method;
    let use_slim = method == Method::MinimumStretch;
    let weight_influence = if synced.weight_group.is_empty() || !synced.use_weights {
        0.0
    } else {
        synced.weight_factor
    };
    let slim = SlimOptions::new(synced.iterations, synced.no_flip, weight_influence);
    let options = UnwrapOptions {
        topology_from_uvs: false,
        topology_from_uvs_use_seams: false,
        only_selected_faces: true,
        only_selected_uvs: false,
        // SLIM requires hole filling.
        fill_holes: synced.fill_holes || use_slim,
        correct_aspect: synced.correct_aspect,
        pin_unselected: false,
        use_abf: method == Method::AngleBased,
        use_subsurf: synced.use_subsurf,
        use_original_bounds: p.use_original_bounds.value,
        weight_group: synced.weight_group.clone(),
    };

    let have = objects.iter().any(|&(_, me)| {
        let (_, edit) = cx.split_edit();
        edit.meshes
            .get(&me.key())
            .is_some_and(|em| have_selection(&em.bm, ts))
    });
    if !have {
        return Ok(OpStatus::Cancelled);
    }

    // Add UVs where missing; scale reports.
    let mut subsurf_error = options.use_subsurf;
    let mut reported_nonuniform = false;
    let mut reported_negative = false;
    let mut unwrap_obs = Vec::new();
    for &(ob, me) in &objects {
        if !uvedit::ensure_uvs(cx, me)? {
            continue;
        }
        unwrap_obs.push((ob, me));
        if subsurf_error && subsurf_first(cx, ob) {
            subsurf_error = false;
        }
        if reported_nonuniform || reported_negative {
            continue;
        }
        let m = uvedit::object_to_world(cx.main(), ob);
        let (s, negative) = scale_state(&m);
        if !((s[0] - s[1]).abs() < 1e-4 && (s[1] - s[2]).abs() < 1e-4) {
            cx.report(
                ReportType::Info,
                "Object has non-uniform scale, unwrap will operate on a non-scaled version of \
                 the mesh",
            );
            reported_nonuniform = true;
        } else if negative {
            cx.report(
                ReportType::Info,
                "Object has negative scale, unwrap will operate on a non-flipped version of the \
                 mesh",
            );
            reported_negative = true;
        }
    }
    if subsurf_error {
        cx.report(
            ReportType::Info,
            "Subdivision Surface modifier needs to be first to work with unwrap",
        );
    }
    if options.use_original_bounds {
        return Err(OpError::NotImplemented(
            "frap: uv.unwrap(use_original_bounds=True) stitches the selection first (uv.stitch), \
             which lands with the UV editor operators in M8"
                .into(),
        ));
    }
    if options.use_subsurf && unwrap_obs.iter().any(|&(ob, _)| subsurf_first(cx, ob)) {
        return Err(OpError::NotImplemented(
            "frap: uv.unwrap(use_subsurf_data=True) on an object whose first modifier is a \
             Subdivision Surface is M8"
                .into(),
        ));
    }

    // `uvedit_unwrap_multi`.
    let base = uvedit::path_base(cx);
    let mut obs = UvOb::gather(cx, &unwrap_obs);
    let mut count_changed = 0u32;
    let mut count_failed = 0u32;
    let mut non_finite = 0usize;
    let mut aspects = Vec::with_capacity(obs.len());
    for o in &mut obs {
        let aspect = uvedit::aspect_y(cx.main(), &base, o.ob, &o.bm);
        aspects.push(aspect);
        // `cd_weight_index`: looked up whether or not `use_weights` (only SLIM reads it).
        let weight = uvedit::defgroup_index(cx.main(), o.me, &options.weight_group);
        let one = core::slice::from_mut(o);
        let mut h: Handle = uvedit::construct_param_handle(one, ts, aspect, &[weight], &options);
        count_failed += h.failed;
        let c = if use_slim {
            h.pm.slim_solve(&slim, options.use_original_bounds)
        } else {
            h.pm.lscm_begin(false, options.use_abf, options.use_original_bounds);
            let c = h.pm.lscm_solve();
            h.pm.lscm_end();
            c
        };
        count_changed += c.changed;
        count_failed += c.failed;
        h.pm.average(true, false, false);
        h.pm.flush(&mut h.uvs);
        non_finite += h.pm.non_finite_charts();
        h.scatter(one);
    }

    // Pack.
    let mut params = PackParams::default();
    params.only_selected_uvs = options.only_selected_uvs;
    params.only_selected_faces = options.only_selected_faces;
    params.use_seams = true;
    params.correct_aspect = options.correct_aspect;
    params.pin_unselected = options.pin_unselected;
    params.rotate_method = frap_uv::RotationMethod::Any;
    params.pin_method = frap_uv::PinMethod::Ignore;
    params.margin_method = margin_method(p.margin_method.value);
    params.margin = synced.margin;
    uvedit::pack_islands_multi(&mut obs, ts, &aspects, PackTarget::Udim, &mut params);
    for o in obs {
        o.commit(cx)?;
    }

    if count_failed == 0 && count_changed == 0 {
        cx.report(
            ReportType::Warning,
            "Unwrap could not solve any island(s), edge seams may need to be added",
        );
    } else if count_failed > 0 {
        cx.report(
            ReportType::Warning,
            format!(
                "Unwrap failed to solve {count_failed} of {} island(s), edge seams may need to \
                 be added",
                count_changed + count_failed
            ),
        );
    }
    if non_finite > 0 {
        cx.report(
            ReportType::Warning,
            format!(
                "frap: {non_finite} island(s) got non-finite UVs (coordinates too large for the \
                 single-precision solver)"
            ),
        );
    }
    Ok(OpStatus::Finished)
}

/// `eUVPackIsland_MarginMethod` of the operator enum.
pub(crate) fn margin_method(m: MarginMethod) -> frap_uv::MarginMethod {
    margin_method_of(m.identifier())
}

/// `eUVPackIsland_MarginMethod` of a `margin_method` identifier (`SCALED`, `ADD`,
/// `FRACTION`; every UV operator has its own enum type for the same items).
pub(crate) fn margin_method_of(identifier: &str) -> frap_uv::MarginMethod {
    match identifier {
        "ADD" => frap_uv::MarginMethod::Add,
        "FRACTION" => frap_uv::MarginMethod::Fraction,
        _ => frap_uv::MarginMethod::Scaled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_state_detects_non_uniform_and_negative() {
        let id = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        assert_eq!(scale_state(&id), ([1.0; 3], false));
        let mut neg = id;
        neg[0][0] = -1.0;
        assert!(scale_state(&neg).1);
        let mut s = id;
        s[2][2] = 2.0;
        assert_eq!(scale_state(&s).0, [1.0, 1.0, 2.0]);
    }

    #[test]
    fn margin_methods_map() {
        assert_eq!(margin_method(MarginMethod::Add), frap_uv::MarginMethod::Add);
        assert_eq!(
            margin_method(MarginMethod::Fraction),
            frap_uv::MarginMethod::Fraction
        );
    }
}
