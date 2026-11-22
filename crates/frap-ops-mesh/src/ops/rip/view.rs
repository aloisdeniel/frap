// SPDX-License-Identifier: GPL-3.0-or-later
//! The region projection `mesh.rip` measures in: the WINDOW region of the factory Layout
//! 3D View, as `blender -b` loads it (1574 × 954 pixels, perspective lens 50 mm, clip
//! 0.01–1000: the `window_matrix` of `spec/blender-5.2/startup.json`), looking through frap's
//! virtual view matrix (D-10). The "mouse" is the 3D cursor projected into that region,
//! rounded to whole pixels like an event's `mval` (headless-extended, deviations D-150).
//!
//! Ported from blender/source/blender/editors/space_view3d/view3d_project.cc
//! (`ED_view3d_project_float_v2_m4`, `ED_view3d_ob_project_mat_get`), Blender 5.2.4,
//! GPL-2.0-or-later.

use super::super::rip_edge::math::{V2, V3, mul_m4_v4};
use super::super::sort_elements::mul_m4_m4m4;

/// `region->winx` of the factory Layout 3D View's WINDOW region in `blender -b`.
pub(crate) const WINX: i32 = 1574;
/// `region->winy`.
pub(crate) const WINY: i32 = 954;

/// `rv3d->winmat` of the factory Layout 3D View (`m[col][row]`).
pub(crate) const WINMAT: [[f32; 4]; 4] = [
    [1.388_888_8, 0.0, 0.0, 0.0],
    [0.0, 2.291_521, 0.0, 0.0],
    [0.0, 0.0, -1.000_020_1, -1.0],
    [0.0, 0.0, -0.020_000_2, 0.0],
];

/// The projection of one object: `winmat · (viewmat · object_to_world)`.
pub(crate) struct Projection {
    pub mat: [[f32; 4]; 4],
}

impl Projection {
    /// `ED_view3d_ob_project_mat_get(rv3d, ob)`.
    pub(crate) fn for_object(viewmat: &[[f32; 4]; 4], obmat: &[[f32; 4]; 4]) -> Projection {
        let vmat = mul_m4_m4m4(viewmat, obmat);
        Projection {
            mat: mul_m4_m4m4(&WINMAT, &vmat),
        }
    }

    /// `ED_view3d_project_float_v2_m4(region, co, mat)`: region pixel coordinates, `(0, 0)`
    /// behind the view.
    pub(crate) fn project(&self, co: V3) -> V2 {
        let v = mul_m4_v4(&self.mat, [co[0], co[1], co[2], 1.0]);
        if v[3] > f32::EPSILON {
            let hx = WINX as f32 / 2.0;
            let hy = WINY as f32 / 2.0;
            [hx + hx * v[0] / v[3], hy + hy * v[1] / v[3]]
        } else {
            [0.0, 0.0]
        }
    }
}

/// The mouse position (`event->mval` as floats) for the world-space point `cursor`: its
/// projection through `winmat · viewmat`, rounded to whole pixels.
pub(crate) fn mouse_from_cursor(viewmat: &[[f32; 4]; 4], cursor: V3) -> V2 {
    let ident = frap_bmesh::IDENTITY;
    let p = Projection::for_object(viewmat, &ident).project(cursor);
    [p[0].round(), p[1].round()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_view_projects_the_origin_near_the_centre() {
        // The factory viewport looks at (about) the origin.
        let viewmat: [[f32; 4]; 4] = [
            [0.410_028_4, -0.401_742_58, 0.818_828_3, 0.0],
            [0.911_976_5, 0.193_643_42, -0.361_664_95, 0.0],
            [-0.013_264_662, 0.895_044_9, 0.445_778_9, 0.0],
            [0.050_467_845, 4.074_536_3e-10, -18.388_048, 1.0],
        ];
        let m = mouse_from_cursor(&viewmat, [0.0; 3]);
        assert!((m[0] - 789.0).abs() < 4.0 && (m[1] - 477.0).abs() < 4.0, "{m:?}");
    }
}
