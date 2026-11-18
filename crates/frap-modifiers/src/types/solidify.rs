// SPDX-License-Identifier: GPL-3.0-or-later
//! SOLIDIFY (`SolidifyModifier`): gives a surface thickness. `EXTRUDE` ("Simple", [`extrude`])
//! offsets a copy of the mesh along the vertex normals and joins the boundaries with rim
//! faces; `NON_MANIFOLD` ("Complex", [`nonmanifold`]) splits every vertex by the faces around
//! it so non-manifold edges and vertices get proper shells and rims.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_solidify.cc (Blender 5.2.4).

pub(crate) mod extrude;
pub(crate) mod nonmanifold;

use frap_mesh::Mesh;

use crate::prelude::*;

#[modifier(
    SOLIDIFY,
    kind = Constructive,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, _cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Solidify(s) = &md.settings else {
        return Ok(mesh);
    };
    // `ctx->object->totcol`: the object's material slots, the mesh's material count.
    let totcol = mesh.materials.len();
    let result = match s.solidify_mode.as_str() {
        "NON_MANIFOLD" => nonmanifold::solidify_nonmanifold(s, &mesh, totcol)?,
        _ => extrude::solidify_extrude(s, &mesh, totcol),
    };
    // frap (ARCHITECTURE §12): the shell is a bounded multiple of the input; `--max-elements`
    // backstop (unwinds with `ElementsExceeded`, answered as `LIMIT_EXCEEDED`).
    frap_math::budget::check_corners(result.corners_num() as u64);
    Ok(result)
}

/// Shared helpers of the two modes (`BLI_math` ports).
pub(crate) mod math {
    pub(crate) type V3 = [f32; 3];

    pub(crate) fn sub(a: V3, b: V3) -> V3 {
        [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
    }
    pub(crate) fn add(a: V3, b: V3) -> V3 {
        [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
    }
    pub(crate) fn mul(a: V3, f: f32) -> V3 {
        [a[0] * f, a[1] * f, a[2] * f]
    }
    pub(crate) fn madd(a: V3, b: V3, f: f32) -> V3 {
        [a[0] + b[0] * f, a[1] + b[1] * f, a[2] + b[2] * f]
    }
    pub(crate) fn dot(a: V3, b: V3) -> f32 {
        a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
    }
    pub(crate) fn cross(a: V3, b: V3) -> V3 {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    }
    pub(crate) fn len_sq(a: V3) -> f32 {
        dot(a, a)
    }
    pub(crate) fn len(a: V3) -> f32 {
        dot(a, a).sqrt()
    }
    pub(crate) fn len_v3v3(a: V3, b: V3) -> f32 {
        len(sub(a, b))
    }
    pub(crate) fn len_squared_v3v3(a: V3, b: V3) -> f32 {
        len_sq(sub(a, b))
    }
    pub(crate) fn negate(a: V3) -> V3 {
        [-a[0], -a[1], -a[2]]
    }
    /// `normalize_v3_v3`: the unit vector and the original length (zero stays zero).
    pub(crate) fn normalize(a: V3) -> (V3, f32) {
        let d = dot(a, a);
        if d > 1.0e-35 {
            let l = d.sqrt();
            (mul(a, 1.0 / l), l)
        } else {
            ([0.0; 3], 0.0)
        }
    }
    pub(crate) fn normalized(a: V3) -> V3 {
        normalize(a).0
    }
    /// `safe_asinf`.
    pub(crate) fn safe_asinf(a: f32) -> f32 {
        libm::asinf(a.clamp(-1.0, 1.0))
    }
    /// `angle_normalized_v3v3`.
    pub(crate) fn angle_normalized_v3v3(v1: V3, v2: V3) -> f32 {
        if dot(v1, v2) >= 0.0 {
            return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
        }
        std::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, negate(v2)) / 2.0)
    }
    /// `angle_v3v3`.
    pub(crate) fn angle_v3v3(a: V3, b: V3) -> f32 {
        angle_normalized_v3v3(normalized(a), normalized(b))
    }
    /// `project_plane_normalized_v3_v3v3`.
    pub(crate) fn project_plane_normalized(p: V3, v_plane: V3) -> V3 {
        let m = dot(p, v_plane);
        madd(p, v_plane, -m)
    }
    /// `angle_signed_on_axis_v3v3_v3`.
    pub(crate) fn angle_signed_on_axis_v3v3_v3(v1: V3, v2: V3, axis: V3) -> f32 {
        let v1p = project_plane_normalized(v1, axis);
        let v2p = project_plane_normalized(v2, axis);
        let mut angle = angle_v3v3(v1p, v2p);
        let t = cross(v2p, v1p);
        if dot(t, axis) < 0.0 {
            angle = (std::f64::consts::PI * 2.0) as f32 - angle;
        }
        angle
    }
    /// `shell_v3v3_normalized_to_dist`.
    pub(crate) fn shell_v3v3_normalized_to_dist(a: V3, b: V3) -> f32 {
        let angle_cos = dot(a, b).abs();
        if angle_cos < 1.0e-8 {
            1.0
        } else {
            1.0 / angle_cos
        }
    }
    /// `mid_v3_v3v3_angle_weighted`.
    pub(crate) fn mid_v3_v3v3_angle_weighted(a: V3, b: V3) -> V3 {
        let (r, l) = normalize(add(a, b));
        let angle = (std::f64::consts::FRAC_2_PI as f32 * 2.0) * libm::acosf(l / 2.0);
        mul(r, angle)
    }
}
