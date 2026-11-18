// SPDX-License-Identifier: GPL-3.0-or-later
//! The BEVEL modifier against Blender 5.2.4: element counts and coordinate sums of
//! `evaluated_get(depsgraph).to_mesh()` for each limit method and the main options (the full
//! comparisons are the golden cases of `tests/golden/cases/modifiers/bevel_decimate/`).

use std::sync::Arc;

use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::monkey::{Monkey, monkey};
use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use smallvec::smallvec;

use super::*;

/// `(verts, edges, faces)` and the sums of x, y, z and of the squared lengths.
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

fn assert_stats(name: &str, me: &Mesh, want: Stats) {
    let (c, s) = stats(me);
    assert_eq!(c, want.0, "{name}: counts");
    let (w, tol) = (want.1, |v: f64| 2e-4 + v.abs() * 1e-6);
    for (got, exp) in [(s.0, w.0), (s.1, w.1), (s.2, w.2), (s.3, w.3)] {
        assert!(
            (got - exp).abs() <= tol(exp),
            "{name}: sums {s:?}, Blender {w:?}"
        );
    }
}

fn settings(f: impl FnOnce(&mut BevelSettings)) -> BevelSettings {
    let mut s = BevelSettings::default();
    f(&mut s);
    s
}

fn run(name: &str, me: Mesh, s: &BevelSettings, totcol: i32, want: Stats) -> Beveled {
    let out = modify_mesh(s, me, totcol).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_stats(name, &out.mesh, want);
    out
}

fn cube() -> Mesh {
    cube_mesh(&Cube::default())
}

fn with_attr(mut me: Mesh, name: &str, domain: Domain, values: &[(usize, f32)]) -> Mesh {
    let n = me.domain_size(domain);
    let mut v = vec![0.0f32; n];
    for &(i, w) in values {
        v[i] = w;
    }
    me.attributes_mut().insert(Attribute {
        name: name.into(),
        domain,
        data: AttrData::Float(Arc::new(v)),
    });
    me
}

/// Group `G`: vertices 0–3 weigh 1, vertex 4 weighs 0.4.
fn with_vgroup(mut me: Mesh) -> Mesh {
    me.vertex_group_names.push("G".into());
    let d = me.deform_verts_mut();
    for (i, dv) in d.iter_mut().enumerate() {
        let w = match i {
            0..=3 => 1.0,
            4 => 0.4,
            _ => continue,
        };
        *dv = DeformVert {
            weights: smallvec![(0, w)],
        };
    }
    me
}

#[test]
fn angle_limit_defaults() {
    // The factory cube: every edge is at 90° > 30°.
    run(
        "default",
        cube(),
        &BevelSettings::default(),
        0,
        ((24, 48, 26), (0.0, 0.0, 0.0, 62.88)),
    );
    run(
        "seg3",
        cube(),
        &settings(|s| {
            s.segments = 3;
            s.profile = 0.7;
            s.width = 0.2;
        }),
        0,
        ((96, 192, 98), (0.0, 0.0, 0.0, 236.9582)),
    );
    run(
        "monkey seg2",
        monkey(&Monkey::default()),
        &settings(|s| {
            s.segments = 2;
            s.width = 0.05;
        }),
        0,
        (
            (2480, 4919, 2441),
            (-0.00029, -772.4567, 181.09364, 2246.67747),
        ),
    );
}

#[test]
fn affect_vertices_and_limit_none() {
    run(
        "vertices",
        cube(),
        &settings(|s| {
            s.affect = "VERTICES".into();
            s.width = 0.3;
            s.segments = 2;
        }),
        0,
        ((56, 84, 30), (0.0, 0.0, 0.0, 145.33296)),
    );
    run(
        "none",
        cube(),
        &settings(|s| {
            s.limit_method = "NONE".into();
            s.width = 0.2;
            s.segments = 2;
        }),
        0,
        ((56, 108, 54), (0.0, 0.0, 0.0, 132.73521)),
    );
}

#[test]
fn weight_limits() {
    run(
        "edge weights",
        with_attr(
            cube(),
            "bevel_weight_edge",
            Domain::Edge,
            &[(0, 1.0), (1, 0.5), (2, 1.0)],
        ),
        &settings(|s| {
            s.limit_method = "WEIGHT".into();
            s.width = 0.3;
        }),
        0,
        ((12, 19, 9), (-2.9987, 0.3, 0.0, 31.65306)),
    );
    run(
        "vertex weights",
        with_attr(
            cube(),
            "bevel_weight_vert",
            Domain::Point,
            &[(0, 1.0), (3, 0.5)],
        ),
        &settings(|s| {
            s.limit_method = "WEIGHT".into();
            s.affect = "VERTICES".into();
            s.width = 0.4;
        }),
        0,
        ((12, 18, 8), (-3.4, 0.2, 0.2, 33.0)),
    );
    // No weight attribute: nothing is beveled.
    run(
        "no weights",
        cube(),
        &settings(|s| s.limit_method = "WEIGHT".into()),
        0,
        ((8, 12, 6), (0.0, 0.0, 0.0, 24.0)),
    );
}

#[test]
fn integer_weights_are_converted_and_removed() {
    let mut me = cube();
    me.attributes_mut().insert(Attribute {
        name: "bevel_weight_edge".into(),
        domain: Domain::Edge,
        data: AttrData::Int(Arc::new(
            (0..12).map(|i| i32::from(i == 0 || i == 2)).collect(),
        )),
    });
    let out = modify_mesh(
        &settings(|s| {
            s.limit_method = "WEIGHT".into();
            s.width = 0.3;
        }),
        me,
        0,
    )
    .unwrap();
    assert_eq!(out.mesh.faces_num(), 8, "two edges beveled");
    assert!(out.mesh.attributes().get("bevel_weight_edge.001").is_none());
    assert!(matches!(
        out.mesh
            .attributes()
            .get("bevel_weight_edge")
            .map(|a| &a.data),
        Some(AttrData::Int(_))
    ));
}

#[test]
fn vertex_group_limit() {
    run(
        "vgroup",
        with_vgroup(cube()),
        &settings(|s| {
            s.limit_method = "VGROUP".into();
            s.vertex_group = "G".into();
            s.width = 0.2;
        }),
        0,
        ((12, 20, 10), (-3.2, 0.0, 0.0, 31.68)),
    );
    run(
        "vgroup inverted",
        with_vgroup(cube()),
        &settings(|s| {
            s.limit_method = "VGROUP".into();
            s.vertex_group = "G".into();
            s.invert_vertex_group = true;
            s.width = 0.2;
        }),
        0,
        ((12, 20, 10), (3.2, 0.0, 0.0, 31.68)),
    );
    run(
        "vgroup vertices",
        with_vgroup(cube()),
        &settings(|s| {
            s.limit_method = "VGROUP".into();
            s.vertex_group = "G".into();
            s.affect = "VERTICES".into();
            s.width = 0.4;
        }),
        0,
        ((16, 24, 10), (-6.4, 0.0, 0.0, 40.32)),
    );
}

#[test]
fn offset_types_clamp_and_material() {
    run(
        "percent",
        monkey(&Monkey::default()),
        &settings(|s| {
            s.offset_type = "PERCENT".into();
            s.width_pct = 20.0;
        }),
        0,
        (
            (1286, 2608, 1324),
            (0.02385, -409.35367, 87.68605, 1140.81196),
        ),
    );
    run(
        "width arc",
        cylinder(&Cylinder::default()),
        &settings(|s| {
            s.offset_type = "WIDTH".into();
            s.width = 0.1;
            s.miter_outer = "MITER_ARC".into();
            s.segments = 2;
        }),
        0,
        ((192, 352, 162), (0.0, 0.0, 0.0, 361.23778)),
    );
    run(
        "no clamp",
        cube(),
        &settings(|s| {
            s.use_clamp_overlap = false;
            s.width = 0.6;
        }),
        0,
        ((24, 48, 26), (0.0, 0.0, 0.0, 31.68)),
    );
    run(
        "clamped",
        cube(),
        &settings(|s| s.width = 1.5),
        0,
        ((24, 48, 26), (0.0, 0.0, 0.0, 24.0)),
    );
    let out = run(
        "material",
        cube(),
        &settings(|s| {
            s.material = 1;
            s.width = 0.2;
        }),
        2,
        ((24, 48, 26), (0.0, 0.0, 0.0, 54.72)),
    );
    let Some(AttrData::Int(mi)) = out.mesh.attributes().get("material_index").map(|a| &a.data)
    else {
        panic!("material_index");
    };
    // 6 rebuilt faces keep slot 0, the 12 edge faces and 8 corners use slot 1.
    assert_eq!(mi.iter().filter(|&&m| m == 1).count(), 20);
}

#[test]
fn disabled_unsupported_and_reports() {
    let zero = ModifierSettings::Bevel(settings(|s| s.width = 0.0));
    assert!(is_disabled(&zero).is_some());
    let pct = ModifierSettings::Bevel(settings(|s| {
        s.width = 0.0;
        s.offset_type = "PERCENT".into();
    }));
    assert!(is_disabled(&pct).is_none(), "PERCENT reads width_pct");
    assert!(is_disabled(&ModifierSettings::Bevel(BevelSettings::default())).is_none());
    let custom = settings(|s| s.profile_type = "CUSTOM".into());
    assert!(matches!(
        modify_mesh(&custom, cube(), 0),
        Err(ModError::Error(m)) if m.contains("CUSTOM")
    ));
    let harden = modify_mesh(&settings(|s| s.harden_normals = true), cube(), 0).unwrap();
    assert_eq!(harden.warnings.len(), 1);
    assert!(harden.warnings[0].contains("harden_normals"));
    // An empty mesh passes through.
    let empty = Mesh::new("Empty");
    assert_eq!(
        modify_mesh(&BevelSettings::default(), empty, 0)
            .unwrap()
            .mesh
            .verts_num(),
        0
    );
}
