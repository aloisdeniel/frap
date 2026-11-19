// SPDX-License-Identifier: GPL-3.0-or-later
//! Blender 5.2.4 goldens of the UV core (WP 3.3 acceptance): `tests/fixtures/uv.blender.json`,
//! written by `tests/blender/dump_uv.py` (headless, everything selected, App. B.5 context).
//! Each test rebuilds the edit-mode mesh and replays the operator pipeline on `frap_uv`
//! (`tests/common`).
//!
//! * LSCM unwrap (solver-dependent, so a tolerance and not exactness): within 1e-4.
//! * Cube / cylinder / sphere projection and Smart UV Project (with its pack): equal to
//!   Blender's output (within 2e-6, the last bits of `atan2f` / `cosf` across libms).
//! * The island packer (every shape, rotation and margin method, overlap merging) on Blender's
//!   own inputs: equal to Blender, plus the T3 invariants (inside the unit square, no overlap).
//!   The repacks include axis-aligned rotations of already axis-aligned layouts, where the
//!   fitted angle is `atan2f(±0, -1)`: frap's `atan2f` follows the reference libm (macOS),
//!   which rounds it below pi (`param::bli::atan2f`). Elsewhere frap's `atan2f` can still
//!   differ from Apple's by one ulp (about 3% of random inputs), which only matters for an
//!   angle on a ±90° boundary.
//!   The bitmap (`xatlas`) packer cases take seconds optimized and minutes unoptimized (as in
//!   Blender, where they take seconds too), so they only run in release builds
//!   (`cargo test --release -p frap-uv`); `cylinder_pack_islands_merge_overlap` keeps the
//!   concave packer covered in debug runs.

mod common;

use common::*;
use frap_uv::project::{CurvedProjection, ProjectDirection};

const LSCM_TOL: f32 = 1e-4;
const EXACT_TOL: f32 = 2e-6;

fn case(name: &str) -> (Mesh, Vec<[f32; 2]>, Vec<[f32; 2]>) {
    let fx = fixture();
    let c = &fx["cases"][name];
    assert!(c.is_object(), "missing case {name}");
    let mesh = Mesh::from_json(before(&fx, c));
    let uvs = mesh.uvs.clone();
    (mesh, uvs, after(c))
}

/// A case whose first step is replayed from another case's Blender output (`from`): the mesh
/// of `name` with the UVs Blender wrote in `from`, the second step's operator properties, and
/// the expected UVs.
fn chained(name: &str, from: &str) -> (Mesh, Vec<[f32; 2]>, serde_json::Value, Vec<[f32; 2]>) {
    let fx = fixture();
    let c = &fx["cases"][name];
    assert!(c.is_object(), "missing case {name}");
    let mesh = Mesh::from_json(before(&fx, c));
    let uvs = after(&fx["cases"][from]);
    let kw = c["steps"][1][1].clone();
    (mesh, uvs, kw, after(c))
}

/// T3 packer invariants: every UV in the unit square (within `tol`), and no two islands'
/// faces overlap (triangle-triangle tests of the UV islands).
fn check_packed_invariants(name: &str, mesh: &Mesh, uvs: &[[f32; 2]], tol: f32) {
    for (i, uv) in uvs.iter().enumerate() {
        assert!(
            uv[0] >= -tol && uv[0] <= 1.0 + tol && uv[1] >= -tol && uv[1] <= 1.0 + tol,
            "{name}: corner {i} at {uv:?} outside the unit square"
        );
    }
    let islands = calc_uv_islands(mesh, uvs, false);
    let tris = |faces: &[usize]| -> Vec<[[f32; 2]; 3]> {
        let mut out = Vec::new();
        for &f in faces {
            let c: Vec<usize> = mesh.corners(f).collect();
            for k in 1..c.len() - 1 {
                out.push([uvs[c[0]], uvs[c[k]], uvs[c[k + 1]]]);
            }
        }
        out
    };
    // Shrink towards the centroid so touching edges do not count as overlaps.
    let shrink = |t: [[f32; 2]; 3]| -> [[f32; 2]; 3] {
        let c = [
            (t[0][0] + t[1][0] + t[2][0]) / 3.0,
            (t[0][1] + t[1][1] + t[2][1]) / 3.0,
        ];
        t.map(|p| [c[0] + (p[0] - c[0]) * 0.99, c[1] + (p[1] - c[1]) * 0.99])
    };
    let all: Vec<Vec<[[f32; 2]; 3]>> = islands.iter().map(|i| tris(i)).collect();
    for a in 0..all.len() {
        for b in a + 1..all.len() {
            for ta in &all[a] {
                for tb in &all[b] {
                    let (ta, tb) = (shrink(*ta), shrink(*tb));
                    assert!(!tri_overlap(ta, tb), "{name}: islands {a} and {b} overlap");
                }
            }
        }
    }
}

/// Strict 2D triangle overlap (separating axes on the six edge normals).
fn tri_overlap(a: [[f32; 2]; 3], b: [[f32; 2]; 3]) -> bool {
    for (t, o) in [(a, b), (b, a)] {
        for i in 0..3 {
            let (p, q) = (t[i], t[(i + 1) % 3]);
            let n = [q[1] - p[1], p[0] - q[0]];
            let proj = |x: [f32; 2]| n[0] * x[0] + n[1] * x[1];
            let (mut amin, mut amax) = (f32::MAX, f32::MIN);
            for x in t {
                amin = amin.min(proj(x));
                amax = amax.max(proj(x));
            }
            let (mut bmin, mut bmax) = (f32::MAX, f32::MIN);
            for x in o {
                bmin = bmin.min(proj(x));
                bmax = bmax.max(proj(x));
            }
            if amax <= bmin || bmax <= amin {
                return false;
            }
        }
    }
    true
}

fn check(name: &str, got: &[[f32; 2]], want: &[[f32; 2]], tol: f32) {
    let d = max_diff(got, want);
    assert!(
        d <= tol,
        "{name}: max |Δuv| = {d} > {tol}\n got: {got:?}\nwant: {want:?}"
    );
}

// ------------------------------------------------------------------ LSCM unwrap

#[test]
fn cube_unwrap_seams_lscm() {
    let (mesh, mut uvs, want) = case("cube_unwrap_seams");
    let (count, failed) = unwrap(&mesh, &mut uvs, false, false, 0.001);
    assert_eq!((count.changed, count.failed, failed), (1, 0, 0));
    check("cube_unwrap_seams", &uvs, &want, LSCM_TOL);
}

#[test]
fn cube_unwrap_seams_margin0() {
    let (mesh, mut uvs, want) = case("cube_unwrap_seams_margin0");
    unwrap(&mesh, &mut uvs, false, false, 0.0);
    check("cube_unwrap_seams_margin0", &uvs, &want, LSCM_TOL);
}

#[test]
fn cube_unwrap_seams_angle_based_falls_back_to_lscm() {
    // ABF++ is M8 (WP 8.1); on the seamed cube Blender's ABF and LSCM agree (App. B.5).
    let (mesh, mut uvs, want) = case("cube_unwrap_seams_angle");
    unwrap(&mesh, &mut uvs, true, false, 0.001);
    check("cube_unwrap_seams_angle", &uvs, &want, LSCM_TOL);
}

#[test]
fn cube_unwrap_without_seams_fails_and_repacks() {
    let (mesh, mut uvs, want) = case("cube_unwrap_noseams");
    let (count, failed) = unwrap(&mesh, &mut uvs, false, false, 0.001);
    // "Unwrap failed to solve 1 of 1 island(s)".
    assert_eq!((count.changed, count.failed, failed), (0, 0, 1));
    check("cube_unwrap_noseams", &uvs, &want, LSCM_TOL);
}

#[test]
fn cylinder_unwrap_three_charts() {
    let (mesh, mut uvs, want) = case("cyl8_unwrap_seams");
    let (count, _) = unwrap(&mesh, &mut uvs, false, false, 0.001);
    assert_eq!(count.changed, 3);
    check("cyl8_unwrap_seams", &uvs, &want, LSCM_TOL);
}

#[test]
fn sphere_unwrap_meridian_seam() {
    let (mesh, mut uvs, want) = case("sphere_unwrap_meridian");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("sphere_unwrap_meridian", &uvs, &want, LSCM_TOL);
}

#[test]
fn tube_unwrap_annulus() {
    let (mesh, mut uvs, want) = case("tube_unwrap");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("tube_unwrap", &uvs, &want, LSCM_TOL);
}

#[test]
fn tube_unwrap_fill_holes() {
    let (mesh, mut uvs, want) = case("tube_unwrap_fill");
    unwrap(&mesh, &mut uvs, false, true, 0.001);
    check("tube_unwrap_fill", &uvs, &want, LSCM_TOL);
}

#[test]
fn tube_unwrap_cut_open() {
    let (mesh, mut uvs, want) = case("tube_unwrap_seam");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("tube_unwrap_seam", &uvs, &want, LSCM_TOL);
}

#[test]
fn ngon_unwrap_polyfill_beautify() {
    let (mesh, mut uvs, want) = case("circle_ngon_unwrap");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("circle_ngon_unwrap", &uvs, &want, LSCM_TOL);
}

#[test]
fn grid_unwrap() {
    let (mesh, mut uvs, want) = case("grid_unwrap");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("grid_unwrap", &uvs, &want, LSCM_TOL);
}

// ------------------------------------------------------------------ projections

#[test]
fn cube_cube_project() {
    let (mesh, mut uvs, want) = case("cube_cube_project");
    cube_project(&mesh, &mut uvs);
    check("cube_cube_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cube_cylinder_project() {
    let (mesh, mut uvs, want) = case("cube_cylinder_project");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Cylinder,
        ProjectDirection::ViewOnEquator,
        false,
    );
    check("cube_cylinder_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cube_sphere_project() {
    let (mesh, mut uvs, want) = case("cube_sphere_project");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Sphere,
        ProjectDirection::ViewOnEquator,
        false,
    );
    check("cube_sphere_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cylinder_cylinder_project() {
    let (mesh, mut uvs, want) = case("cyl8_cylinder_project");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Cylinder,
        ProjectDirection::ViewOnEquator,
        false,
    );
    check("cyl8_cylinder_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn sphere_cylinder_project() {
    let (mesh, mut uvs, want) = case("sphere_cylinder_project");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Cylinder,
        ProjectDirection::ViewOnEquator,
        false,
    );
    check("sphere_cylinder_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn sphere_sphere_project() {
    let (mesh, mut uvs, want) = case("sphere_sphere_project");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Sphere,
        ProjectDirection::ViewOnEquator,
        false,
    );
    check("sphere_sphere_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn sphere_sphere_project_fan() {
    let (mesh, mut uvs, want) = case("sphere_sphere_project_fan");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Sphere,
        ProjectDirection::ViewOnEquator,
        true,
    );
    check("sphere_sphere_project_fan", &uvs, &want, EXACT_TOL);
}

#[test]
fn sphere_sphere_project_align_to_object() {
    let (mesh, mut uvs, want) = case("sphere_sphere_project_object");
    curved_project(
        &mesh,
        &mut uvs,
        CurvedProjection::Sphere,
        ProjectDirection::AlignToObject,
        false,
    );
    check("sphere_sphere_project_object", &uvs, &want, EXACT_TOL);
}

#[test]
fn cube_project_then_average_islands_scale() {
    let (mesh, mut uvs, want) = case("cube_cube_project_average");
    cube_project(&mesh, &mut uvs);
    average_islands_scale(&mesh, &mut uvs, false, false);
    check("cube_cube_project_average", &uvs, &want, EXACT_TOL);
}

// ------------------------------------------------------------------ smart project

#[test]
fn cube_smart_project_b5_layout() {
    let (mesh, mut uvs, want) = case("cube_smart_project");
    smart_project(&mesh, &mut uvs);
    check("cube_smart_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cylinder_smart_project() {
    let (mesh, mut uvs, want) = case("cyl8_smart_project");
    smart_project(&mesh, &mut uvs);
    check("cyl8_smart_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn grid_smart_project() {
    let (mesh, mut uvs, want) = case("grid_smart_project");
    smart_project(&mesh, &mut uvs);
    check("grid_smart_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cube_unwrap_seams_conformal() {
    let (mesh, mut uvs, want) = case("cube_unwrap_seams_conformal");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("cube_unwrap_seams_conformal", &uvs, &want, LSCM_TOL);
}

#[test]
fn grid_unwrap_two_pins() {
    let (mesh, mut uvs, want) = case("grid_unwrap_pin2");
    let (count, _) = unwrap(&mesh, &mut uvs, false, false, 0.001);
    assert_eq!(count.changed, 1);
    check("grid_unwrap_pin2", &uvs, &want, LSCM_TOL);
}

#[test]
fn sphere_unwrap_two_pins() {
    let (mesh, mut uvs, want) = case("sphere_unwrap_pin2");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("sphere_unwrap_pin2", &uvs, &want, LSCM_TOL);
}

#[test]
fn sphere_unwrap_single_pin() {
    // One pin: rotate to the AABB fit, keep the UV area, translate back onto the pin.
    let (mesh, mut uvs, want) = case("sphere_unwrap_pin1");
    unwrap(&mesh, &mut uvs, false, false, 0.001);
    check("sphere_unwrap_pin1", &uvs, &want, LSCM_TOL);
}

#[test]
fn tube_unwrap_with_tool_settings_defaults_is_valid() {
    // `bpy.ops.uv.unwrap()` with the factory tool settings is ANGLE_BASED with `fill_holes`:
    // ABF++ is M8 (WP 8.1), so frap's LSCM fallback differs on this non-developable chart; the
    // layout must still be a valid pack.
    let (mesh, mut uvs, _) = case("tube_unwrap_toolsettings");
    unwrap(&mesh, &mut uvs, true, true, 0.001);
    check_packed_invariants("tube_unwrap_toolsettings", &mesh, &uvs, 1e-6);
}

// ------------------------------------------------------------------ chained operators

#[test]
fn cylinder_cube_project() {
    let (mesh, mut uvs, want) = case("cyl8_cube_project");
    cube_project(&mesh, &mut uvs);
    check("cyl8_cube_project", &uvs, &want, EXACT_TOL);
}

#[test]
fn cylinder_average_islands_scale_with_shear() {
    let (mesh, mut uvs, _, want) = chained("cyl8_smart_average_shear", "cyl8_smart_project");
    average_islands_scale(&mesh, &mut uvs, true, true);
    check("cyl8_smart_average_shear", &uvs, &want, EXACT_TOL);
}

#[test]
fn monkey_smart_project() {
    let (mesh, mut uvs, want) = case("monkey_smart_project");
    smart_project(&mesh, &mut uvs);
    check("monkey_smart_project", &uvs, &want, EXACT_TOL);
}

/// `pack_islands` after `smart_project` in the same scene: an unset `margin` is the
/// `island_margin` (0) smart project stored in the tool settings.
fn pack_case(name: &str, from: &str) {
    let (mesh, mut uvs, kw, want) = chained(name, from);
    pack_islands_op(&mesh, &mut uvs, &kw, 0.0);
    check_packed_invariants(name, &mesh, &uvs, 1e-6);
    check(name, &uvs, &want, EXACT_TOL);
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn cube_pack_islands_concave() {
    pack_case("cube_smart_pack", "cube_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_default_concave() {
    pack_case("monkey_pack_default", "monkey_smart_project");
}

#[test]
fn monkey_pack_islands_aabb() {
    pack_case("monkey_pack_aabb", "monkey_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_convex() {
    pack_case("monkey_pack_convex", "monkey_smart_project");
}

#[test]
fn monkey_pack_islands_no_rotation() {
    pack_case("monkey_pack_norotate", "monkey_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_cardinal() {
    pack_case("monkey_pack_cardinal", "monkey_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_margin_fraction() {
    pack_case("monkey_pack_fraction", "monkey_smart_project");
}

#[test]
fn monkey_pack_islands_margin_add() {
    pack_case("monkey_pack_add", "monkey_smart_project");
}

#[test]
fn cylinder_pack_islands_merge_overlap() {
    let (mesh, mut uvs, kw, want) = chained("cyl8_pack_merge_overlap", "cyl8_cube_project");
    // The factory tool settings' `uvcalc_margin`.
    pack_islands_op(&mesh, &mut uvs, &kw, 0.001);
    check("cyl8_pack_merge_overlap", &uvs, &want, EXACT_TOL);
}

// Repacks of axis-aligned layouts with an axis-aligned rotation: the fitted angle of an island
// that is already aligned is `atan2f(±0, -1)`, which must round below pi as in Blender's libm
// (above pi, `angle_wrap` crosses ±90° and the island ends up turned by 180°).

#[test]
fn monkey_pack_islands_axis_aligned_x_aabb() {
    pack_case("monkey_pack_x_aabb", "monkey_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_axis_aligned_x_convex() {
    pack_case("monkey_pack_x_convex", "monkey_smart_project");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_pack_islands_axis_aligned_x_concave() {
    pack_case("monkey_pack_x_concave", "monkey_smart_project");
}

#[test]
fn monkey_smart_project_axis_aligned_x() {
    let (mesh, mut uvs, want) = case("monkey_smart_project_x");
    smart_project_rotate(&mesh, &mut uvs, frap_uv::RotationMethod::AxisAlignedX);
    check("monkey_smart_project_x", &uvs, &want, EXACT_TOL);
}

#[test]
fn monkey_smart_x_pack_islands_axis_aligned_y_aabb() {
    pack_case("monkey_smart_x_pack_y_aabb", "monkey_smart_project_x");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_smart_x_pack_islands_axis_aligned_y() {
    pack_case("monkey_smart_x_pack_y", "monkey_smart_project_x");
}

#[test]
fn monkey_smart_x_pack_islands_axis_aligned_aabb() {
    pack_case("monkey_smart_x_pack_aligned_aabb", "monkey_smart_project_x");
}

#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "xatlas packer: minutes unoptimized, run with --release"
)]
fn monkey_smart_x_pack_islands_axis_aligned() {
    pack_case("monkey_smart_x_pack_aligned", "monkey_smart_project_x");
}
