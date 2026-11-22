// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.circularize(factor=1.0, fit_method='LEAST_SQUARE', angle=0.0,
//! use_custom_radius=False, custom_radius=1.0, regular=True, flatten=1.0,
//! lock=(False, False, False))` (new in 5.x), tier T1
//! (`tests/golden/cases/edit/tools_m8/circularize.py`). Port of `edbm_circularize_exec`
//! (`editors/mesh/editmesh_circularize.cc`), `BKE_object_get_mirror_axes`
//! (`blenkernel/intern/object.cc`) and `bmo_circularize_exec` ([`bmo`]), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! For every mesh in Edit Mode with at least three selected vertices, the boundary chains of
//! the selection are shaped into circles (see [`bmo`]); the radius is `custom_radius` with
//! `use_custom_radius`. The axes of the object's visible MIRROR modifiers that merge and
//! have no mirror object count as mirror planes (chains ending on one stay on it). The
//! normals are updated. `CANCELLED` when no mesh had three selected vertices.

use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::ModifierSettings;
use frap_spec_ops_mesh::mesh::{self, circularize::FitMethod as Fit};

use super::split::edbm;

pub(crate) mod bmo;

use bmo::{CircularizeIn, FitMethod};

/// `BKE_object_get_mirror_axes`.
fn mirror_axes(cx: &OpCtx, ob: frap_rna::IdRef<frap_rna::ObjectId>) -> [bool; 3] {
    let mut axis = [false; 3];
    let Some(o) = cx.main().objects.get(ob) else {
        return axis;
    };
    for md in &o.modifiers {
        if let ModifierSettings::Mirror(m) = &md.settings
            && md.show_viewport
        {
            // A mirror object may have any transform; no merge means no continuity.
            if m.mirror_object.is_some() || !m.use_mirror_merge {
                continue;
            }
            for (a, on) in axis.iter_mut().zip(m.use_axis) {
                *a |= on;
            }
        }
    }
    axis
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::Circularize) -> OpResult {
    let custom_radius = if p.use_custom_radius.value {
        p.custom_radius.value
    } else {
        0.0
    };
    let fit_method = match p.fit_method.value {
        Fit::LeastSquare => FitMethod::LeastSquare,
        Fit::Contract => FitMethod::Contract,
    };
    let mut changed = false;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        if edbm::read(cx, me, edbm::sel_counts).unwrap_or_default().0 < 3 {
            continue;
        }
        let mirror = mirror_axes(cx, ob);
        let bm = cx.bmesh_mut(me)?;
        let geom = edbm::selected_geom(bm);
        bmo::circularize(
            bm,
            &CircularizeIn {
                geom,
                factor: p.factor.value,
                custom_radius,
                angle: p.angle.value,
                fit_method,
                flatten: p.flatten.value,
                regular: p.regular.value,
                lock: p.lock.value,
                mirror,
            },
        );
        bm.normals_update();
        changed = true;
    }
    Ok(if changed {
        OpStatus::Finished
    } else {
        OpStatus::Cancelled
    })
}
