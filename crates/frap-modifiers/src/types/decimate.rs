// SPDX-License-Identifier: GPL-3.0-or-later
//! DECIMATE (`DecimateModifier`): reduces the face count through a BMesh, with one of the three
//! `decimate_type`s:
//! - `COLLAPSE` (the default): quadric edge collapse down to `ratio` of the (triangulated) faces
//!   (`BM_mesh_decimate_collapse`), optionally weighted by `vertex_group`
//!   (`vertex_group_factor`, `invert_vertex_group`), kept triangulated
//!   (`use_collapse_triangulate`) and symmetric about `symmetry_axis` (`use_symmetry`);
//! - `UNSUBDIV`: un-subdivides `iterations` times (`BM_mesh_decimate_unsubdivide`);
//! - `DISSOLVE` ("Planar"): limited dissolve below `angle_limit`, `delimit`,
//!   `use_dissolve_boundaries` (`BM_mesh_decimate_dissolve`).
//!
//! `ratio` 1, `iterations` 0 and `angle_limit` 0 pass the mesh through unchanged; a mesh of 3
//! faces or fewer is a modifier error (`Modifier requires more than 3 input faces`, the mesh
//! passes unchanged). The read-only `face_count` is not stored by frap.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_decimate.cc (`modify_mesh`, Blender
//! 5.2.4, GPL-2.0-or-later).

use frap_bmesh::ops::decimate_collapse::{DecimateCollapseIn, decimate_collapse};
use frap_bmesh::ops::decimate_dissolve::decimate_dissolve;
use frap_bmesh::ops::decimate_unsubdivide::decimate_unsubdivide;
use frap_bmesh::ops::dissolve_limit::Delimit;
use frap_bmesh::{BMesh, FromMeshParams, ToMeshParams};
use frap_scene::generated::modifier_settings::DecimateSettings;

use crate::prelude::*;

#[modifier(
    DECIMATE,
    kind = Nonconstructive,
    flags = ACCEPTS_MESH | ACCEPTS_CVS,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Decimate(s) = &md.settings else {
        return Ok(mesh);
    };
    modify_mesh(s, mesh)
}

/// `symmetry_eps` of the modifier.
const SYMMETRY_EPS: f32 = 0.00002;

/// The vertex-group weights of the COLLAPSE mode (`vweights`), by vertex index: `None` without
/// a group name, with a zero factor, or when the mesh has no such group or no weights.
fn collapse_weights(s: &DecimateSettings, mesh: &Mesh) -> Option<Vec<f32>> {
    if s.vertex_group.is_empty() || s.vertex_group_factor <= 0.0 {
        return None;
    }
    // `MOD_get_vgroup`.
    let group = mesh
        .vertex_group_names
        .iter()
        .position(|g| g == &s.vertex_group)?;
    let dverts = mesh.deform_verts()?;
    Some(
        (0..mesh.verts_num())
            .map(|v| {
                let w = dverts
                    .get(v)
                    .and_then(|dv| dv.weights.iter().find(|w| w.0 as usize == group))
                    .map_or(0.0, |w| w.1);
                if s.invert_vertex_group { 1.0 - w } else { w }
            })
            .collect(),
    )
}

/// `BMO_Delimit` of the modifier's `delimit` set.
fn delimit(s: &DecimateSettings) -> Delimit {
    let has = |k: &str| s.delimit.iter().any(|d| d == k);
    Delimit {
        normal: has("NORMAL"),
        material: has("MATERIAL"),
        seam: has("SEAM"),
        sharp: has("SHARP"),
        uv: has("UV"),
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_decimate.cc (modify_mesh).
/// `modify_mesh` of the DECIMATE modifier.
pub(crate) fn modify_mesh(s: &DecimateSettings, mesh: Mesh) -> Result<Mesh, ModError> {
    let (calc_face_normal, calc_vert_normal) = match s.decimate_type.as_str() {
        "COLLAPSE" => {
            if s.ratio == 1.0 {
                return Ok(mesh);
            }
            (true, true)
        }
        "UNSUBDIV" => {
            if s.iterations == 0 {
                return Ok(mesh);
            }
            (false, false)
        }
        "DISSOLVE" => {
            if s.angle_limit == 0.0 {
                return Ok(mesh);
            }
            (true, false)
        }
        _ => return Ok(mesh),
    };
    if mesh.faces_num() <= 3 {
        return Err(ModError::Error(
            "Modifier requires more than 3 input faces".into(),
        ));
    }
    let vweights = if s.decimate_type == "COLLAPSE" {
        collapse_weights(s, &mesh)
    } else {
        None
    };
    let mut bm = BMesh::from_mesh(
        &mesh,
        &FromMeshParams {
            calc_face_normal,
            calc_vert_normal,
            use_shapekey: false,
            active_shapekey: 0,
        },
    );
    match s.decimate_type.as_str() {
        "COLLAPSE" => {
            let symmetry_axis = if s.use_symmetry {
                match s.symmetry_axis.as_str() {
                    "Y" => 1,
                    "Z" => 2,
                    _ => 0,
                }
            } else {
                -1
            };
            let mut i = DecimateCollapseIn::with_factor(s.ratio);
            i.vweights = vweights;
            i.vweight_factor = s.vertex_group_factor;
            i.do_triangulate = s.use_collapse_triangulate;
            i.symmetry_axis = symmetry_axis;
            i.symmetry_eps = SYMMETRY_EPS;
            decimate_collapse(&mut bm, &i);
        }
        "UNSUBDIV" => decimate_unsubdivide(&mut bm, s.iterations),
        _ => decimate_dissolve(
            &mut bm,
            s.angle_limit,
            s.use_dissolve_boundaries,
            delimit(s),
        ),
    }
    Ok(bm.to_mesh(&mesh, &ToMeshParams::default()))
}

#[cfg(test)]
mod tests;
