// SPDX-License-Identifier: GPL-3.0-or-later
//! The region projection `mesh.rip` measures in, and the "mouse" (headless-extended,
//! deviations D-150):
//! - by default the WINDOW region of the factory Layout 3D View, as `blender -b` loads it
//!   (1574 × 954 pixels, perspective lens 50 mm, clip 0.01–1000: the `window_matrix` of
//!   `spec/blender-5.2/startup.json`), looking through frap's virtual view matrix (D-10);
//! - with a `frap_direction`, the same region seen through an **orthographic** view looking
//!   along that direction, the view distance and lens of the factory view
//!   (`BKE_camera_params_from_view3d`, `BKE_camera_params_compute_viewplane`), the mouse ray
//!   `frap_location` + t·`frap_direction` through the region centre (as `mesh.rip_edge`'s
//!   `location`/`direction` are its mouse ray).
//!
//! The mouse is a world-space point (the 3D cursor, or `frap_location`) projected into the
//! region, rounded to whole pixels like an event's `mval`.
//!
//! Ported from blender/source/blender/editors/space_view3d/view3d_project.cc
//! (`ED_view3d_project_float_v2_m4`, `ED_view3d_ob_project_mat_get`) and
//! blender/source/blender/blenlib/intern/math_geom.cc (`orthographic_m4`), Blender 5.2.4,
//! GPL-2.0-or-later.

use super::super::rip_edge::math::{V2, V3, cross3, dot3, mul_m4_v4, normalize_v3};
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

/// `rv3d->dist` of the factory Layout 3D View (`view_distance` in `startup.json`).
pub(crate) const FACTORY_DIST: f32 = 18.388_048;
/// `v3d->lens` of the factory Layout 3D View.
const FACTORY_LENS: f32 = 50.0;
/// `BKE_camera_sensor_size(CAMERA_SENSOR_FIT_AUTO, 36, 24)`.
const SENSOR_SIZE: f32 = 36.0;
/// `v3d->clip_end` of the factory Layout 3D View.
const FACTORY_CLIP_END: f32 = 1000.0;

/// A region and the view it shows: what the rip measures in.
#[derive(Clone, Debug)]
pub(crate) struct RegionView {
    /// `region->winx`, `region->winy`.
    pub win: [i32; 2],
    /// `rv3d->winmat`.
    pub winmat: [[f32; 4]; 4],
    /// `rv3d->viewmat`.
    pub viewmat: [[f32; 4]; 4],
}

impl RegionView {
    /// The factory Layout 3D View's region looking through `viewmat`.
    pub(crate) fn factory(viewmat: [[f32; 4]; 4]) -> RegionView {
        RegionView {
            win: [WINX, WINY],
            winmat: WINMAT,
            viewmat,
        }
    }

    /// The factory region seen through an orthographic view looking along `direction` (not
    /// zero) with `location` at the region centre: `view_perspective = 'ORTHO'`, the factory
    /// `view_distance`, `view_rotation` turning −Z to `direction` (with +Y towards world +Z,
    /// or world +Y when looking straight up or down) and `view_location = location`.
    pub(crate) fn ortho_along(direction: V3, location: V3) -> RegionView {
        let (d, _) = normalize_v3(direction);
        let back = [-d[0], -d[1], -d[2]];
        let world_up = if d[2].abs() > 0.999_999 {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let k = dot3(world_up, back);
        let (up, _) = normalize_v3([
            world_up[0] - back[0] * k,
            world_up[1] - back[1] * k,
            world_up[2] - back[2] * k,
        ]);
        let right = cross3(up, back);
        // World to view: the rows are the view axes, `location` goes to the view origin.
        let rows = [right, up, back];
        let mut viewmat = [[0.0f32; 4]; 4];
        for (r, axis) in rows.iter().enumerate() {
            for (c, col) in viewmat.iter_mut().take(3).enumerate() {
                col[r] = axis[c];
            }
            viewmat[3][r] = -dot3(*axis, location);
        }
        viewmat[3][3] = 1.0;
        // `BKE_camera_params_from_view3d` (orthographic view) and
        // `BKE_camera_params_compute_viewplane` (sensor fit horizontal: winx > winy): the
        // view plane spans ±`ortho_scale · zoom / 2` = ±`ortho_scale` across the width.
        let ortho_scale = FACTORY_DIST * SENSOR_SIZE / FACTORY_LENS;
        let pixsize = ortho_scale / WINX as f32 * 2.0;
        let xmax = 0.5 * WINX as f32 * pixsize;
        let ymax = 0.5 * WINY as f32 * pixsize;
        let clip_end = FACTORY_CLIP_END * 0.5;
        RegionView {
            win: [WINX, WINY],
            winmat: orthographic_m4(-xmax, xmax, -ymax, ymax, -clip_end, clip_end),
            viewmat,
        }
    }

    /// The mouse (`event->mval` as floats) over the world-space point `co`: its projection,
    /// rounded to whole pixels.
    pub(crate) fn mouse_over(&self, co: V3) -> V2 {
        let p = Projection::for_object(self, &frap_bmesh::IDENTITY).project(co);
        [p[0].round(), p[1].round()]
    }
}

/// `orthographic_m4`.
fn orthographic_m4(
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
    near: f32,
    far: f32,
) -> [[f32; 4]; 4] {
    let x_delta = right - left;
    let y_delta = top - bottom;
    let z_delta = far - near;
    let mut m = [[0.0f32; 4]; 4];
    m[0][0] = 2.0 / x_delta;
    m[3][0] = -(right + left) / x_delta;
    m[1][1] = 2.0 / y_delta;
    m[3][1] = -(top + bottom) / y_delta;
    m[2][2] = -2.0 / z_delta;
    m[3][2] = -(far + near) / z_delta;
    m[3][3] = 1.0;
    m
}

/// The projection of one object: `winmat · (viewmat · object_to_world)` into the region.
pub(crate) struct Projection {
    pub mat: [[f32; 4]; 4],
    win: [i32; 2],
}

impl Projection {
    /// `ED_view3d_ob_project_mat_get(rv3d, ob)`.
    pub(crate) fn for_object(rv: &RegionView, obmat: &[[f32; 4]; 4]) -> Projection {
        let vmat = mul_m4_m4m4(&rv.viewmat, obmat);
        Projection {
            mat: mul_m4_m4m4(&rv.winmat, &vmat),
            win: rv.win,
        }
    }

    /// `ED_view3d_project_float_v2_m4(region, co, mat)`: region pixel coordinates, `(0, 0)`
    /// behind the view.
    pub(crate) fn project(&self, co: V3) -> V2 {
        let v = mul_m4_v4(&self.mat, [co[0], co[1], co[2], 1.0]);
        if v[3] > f32::EPSILON {
            let hx = self.win[0] as f32 / 2.0;
            let hy = self.win[1] as f32 / 2.0;
            [hx + hx * v[0] / v[3], hy + hy * v[1] / v[3]]
        } else {
            [0.0, 0.0]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACTORY_VIEWMAT: [[f32; 4]; 4] = [
        [0.410_028_4, -0.401_742_58, 0.818_828_3, 0.0],
        [0.911_976_5, 0.193_643_42, -0.361_664_95, 0.0],
        [-0.013_264_662, 0.895_044_9, 0.445_778_9, 0.0],
        [0.050_467_845, 4.074_536_3e-10, -18.388_048, 1.0],
    ];

    #[test]
    fn factory_view_projects_the_origin_near_the_centre() {
        // The factory viewport looks at (about) the origin.
        let m = RegionView::factory(FACTORY_VIEWMAT).mouse_over([0.0; 3]);
        assert!(
            (m[0] - 789.0).abs() < 4.0 && (m[1] - 477.0).abs() < 4.0,
            "{m:?}"
        );
    }

    #[test]
    fn ortho_view_puts_the_location_at_the_centre_and_looks_along_the_direction() {
        for (dir, loc) in [
            ([0.0, 0.0, -1.0], [0.3, -0.2, 1.0]),
            ([0.0, 0.0, 2.0], [0.0, 0.0, 0.0]),
            ([1.0, -2.0, 0.5], [4.0, 1.0, -3.0]),
        ] {
            let rv = RegionView::ortho_along(dir, loc);
            assert_eq!(rv.mouse_over(loc), [787.0, 477.0]);
            // Every point of the mouse ray projects onto the mouse.
            let far = [loc[0] + 7.0 * dir[0], loc[1] + 7.0 * dir[1], loc[2] + 7.0 * dir[2]];
            assert_eq!(rv.mouse_over(far), [787.0, 477.0]);
            // The scale is the factory ortho one: `ortho_scale` world units per half width.
            let p = Projection::for_object(&rv, &frap_bmesh::IDENTITY);
            let a = p.project(loc);
            let mut off = loc;
            // A world step perpendicular to `dir` (in the view plane).
            let (d, _) = normalize_v3(dir);
            let (side, _) = normalize_v3(cross3(d, [0.3, 0.5, 0.7]));
            for k in 0..3 {
                off[k] += side[k];
            }
            let b = p.project(off);
            let px = ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt();
            let expected = 787.0 / (FACTORY_DIST * 36.0 / 50.0);
            assert!((px - expected).abs() < 1e-2, "{px} vs {expected}");
        }
    }
}
