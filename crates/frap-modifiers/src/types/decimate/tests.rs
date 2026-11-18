// SPDX-License-Identifier: GPL-3.0-or-later
//! The DECIMATE modifier against Blender 5.2.4: element counts and coordinate sums of
//! `evaluated_get(depsgraph).to_mesh()` per mode (the COLLAPSE count tables per primitive are
//! in `frap-bmesh`'s `decimate_collapse` tests; the full comparisons are the golden cases of
//! `tests/golden/cases/modifiers/bevel_decimate/`).

use std::collections::BTreeSet;

use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::grid::{Grid, grid};
use frap_mesh::primitives::monkey::{Monkey, monkey};
use frap_mesh::primitives::plane::{Plane, plane};
use frap_mesh::{DeformVert, Mesh};
use smallvec::smallvec;

use super::*;

type Stats = ((usize, usize, usize), (f64, f64, f64, f64));

fn stats(me: &Mesh) -> Stats {
    let mut s = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for p in me.positions() {
        let [x, y, z] = p.map(f64::from);
        s.0 += x;
        s.1 += y;
        s.2 += z;
        s.3 += x * x + y * y + z * z;
    }
    ((me.verts_num(), me.edges_num(), me.faces_num()), s)
}

fn run(name: &str, me: Mesh, f: impl FnOnce(&mut DecimateSettings), want: Stats) {
    let mut s = DecimateSettings::default();
    f(&mut s);
    let out = modify_mesh(&s, me).unwrap_or_else(|e| panic!("{name}: {e}"));
    let (c, got) = stats(&out);
    assert_eq!(c, want.0, "{name}: counts");
    let w = want.1;
    let tol = |v: f64| 2e-4 + v.abs() * 1e-6;
    for (g, e) in [(got.0, w.0), (got.1, w.1), (got.2, w.2), (got.3, w.3)] {
        assert!(
            (g - e).abs() <= tol(e),
            "{name}: sums {got:?}, Blender {w:?}"
        );
    }
}

/// Suzanne with group `G`: weight 1 for x > 0, 0.1 elsewhere.
fn monkey_vgroup() -> Mesh {
    let mut me = monkey(&Monkey::default());
    me.vertex_group_names.push("G".into());
    let xs: Vec<f32> = me.positions().iter().map(|p| p[0]).collect();
    for (dv, x) in me.deform_verts_mut().iter_mut().zip(xs) {
        *dv = DeformVert {
            weights: smallvec![(0, if x > 0.0 { 1.0 } else { 0.1 })],
        };
    }
    me
}

#[test]
fn collapse_with_vertex_group_and_symmetry() {
    run(
        "vgroup",
        monkey_vgroup(),
        |s| {
            s.ratio = 0.3;
            s.vertex_group = "G".into();
        },
        ((160, 361, 203), (-38.93737, -41.68734, 16.64512, 139.93476)),
    );
    run(
        "vgroup inverted",
        monkey_vgroup(),
        |s| {
            s.ratio = 0.3;
            s.vertex_group = "G".into();
            s.invert_vertex_group = true;
            s.vertex_group_factor = 2.0;
        },
        ((246, 502, 258), (120.18864, -81.60595, 21.05316, 218.79782)),
    );
    run(
        "symmetry Y",
        monkey(&Monkey::default()),
        |s| {
            s.ratio = 0.4;
            s.use_symmetry = true;
            s.symmetry_axis = "Y".into();
        },
        ((213, 511, 300), (2.78117, -60.45724, 27.15457, 189.42193)),
    );
}

#[test]
fn unsubdivide() {
    run(
        "grid",
        grid(&Grid::default()),
        |s| {
            s.decimate_type = "UNSUBDIV".into();
            s.iterations = 2;
        },
        ((44, 85, 42), (0.0, 0.0, 0.0, 44.8)),
    );
    run(
        "monkey",
        monkey(&Monkey::default()),
        |s| {
            s.decimate_type = "UNSUBDIV".into();
            s.iterations = 1;
        },
        ((286, 637, 353), (0.0, -90.41406, 21.01562, 253.15497)),
    );
}

#[test]
fn planar_dissolve() {
    run(
        "monkey",
        monkey(&Monkey::default()),
        |s| {
            s.decimate_type = "DISSOLVE".into();
            s.angle_limit = 0.3;
        },
        ((423, 719, 298), (0.0, -137.79688, 35.23438, 376.65747)),
    );
    run(
        "cylinder",
        cylinder(&Cylinder::default()),
        |s| s.decimate_type = "DISSOLVE".into(),
        ((64, 96, 34), (0.0, 0.0, 0.0, 128.0)),
    );
    run(
        "cylinder boundaries",
        cylinder(&Cylinder::default()),
        |s| {
            s.decimate_type = "DISSOLVE".into();
            s.use_dissolve_boundaries = true;
            s.angle_limit = 0.5;
        },
        ((18, 27, 11), (-1.50199, 0.95019, 0.0, 36.0)),
    );
    run(
        "monkey delimit UV",
        monkey(&Monkey::default()),
        |s| {
            s.decimate_type = "DISSOLVE".into();
            s.angle_limit = 0.6;
            s.delimit = BTreeSet::from(["UV".into()]);
        },
        ((313, 484, 173), (0.03906, -107.82031, 26.6875, 289.00574)),
    );
}

#[test]
fn no_ops_and_errors() {
    // ratio 1, iterations 0 and angle 0 pass the mesh through (no triangulate round trip).
    let me = monkey(&Monkey::default());
    for f in [
        (|_: &mut DecimateSettings| {}) as fn(&mut DecimateSettings),
        |s| s.decimate_type = "UNSUBDIV".into(),
        |s| {
            s.decimate_type = "DISSOLVE".into();
            s.angle_limit = 0.0;
        },
    ] {
        let mut s = DecimateSettings::default();
        f(&mut s);
        let out = modify_mesh(&s, me.clone()).unwrap();
        assert_eq!(stats(&out).0, (507, 1005, 500));
    }
    // Blender: "Modifier requires more than 3 input faces", the mesh passes unchanged.
    let s = DecimateSettings {
        ratio: 0.5,
        ..DecimateSettings::default()
    };
    assert!(matches!(
        modify_mesh(&s, plane(&Plane::default())),
        Err(ModError::Error(m)) if m == "Modifier requires more than 3 input faces"
    ));
}
