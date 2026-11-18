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
            s.width = 20.0;
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
    // `width_pct` is the same value as `width`.
    let pct = ModifierSettings::Bevel(settings(|s| {
        s.width = 0.0;
        s.offset_type = "PERCENT".into();
    }));
    assert!(is_disabled(&pct).is_some(), "PERCENT reads the same amount");
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

/// `BevelModifier.width` and `width_pct` are one DNA field (`BevelModifierData::value`):
/// writing either is read back by both, and the amount the modifier uses is that value.
#[test]
fn width_and_width_pct_are_one_value() {
    use frap_rna::Rna;
    use frap_rna::{Field, Value};
    let srna = BevelSettings::default().srna();
    let (width, pct) = (
        srna.find_prop("width").unwrap(),
        srna.find_prop("width_pct").unwrap(),
    );
    let read = |s: &BevelSettings, p| match s.rna_get(p) {
        Some(Field::Value(Value::Float(v))) => v,
        _ => panic!("width is a stored float"),
    };
    let mut s = BevelSettings::default();
    assert!(s.rna_set(pct, Value::Float(0.25)).is_some());
    assert_eq!((read(&s, width), read(&s, pct)), (0.25, 0.25));
    assert_eq!(value(&s), 0.25);
    assert!(s.rna_set(width, Value::Float(30.0)).is_some());
    assert_eq!((read(&s, width), read(&s, pct)), (30.0, 30.0));
    // Blender 5.2.4: `width_pct = 0.25` with the default OFFSET bevels the cube by 0.25.
    let mut s = BevelSettings::default();
    assert!(s.rna_set(pct, Value::Float(0.25)).is_some());
    run(
        "width_pct offset",
        cube(),
        &s,
        0,
        ((24, 48, 26), (0.0, 0.0, 0.0, 51.0)),
    );
}

fn with_data(mut me: Mesh, name: &str, domain: Domain, data: AttrData) -> Mesh {
    me.attributes_mut().insert(Attribute {
        name: name.into(),
        domain,
        data,
    });
    me
}

/// Weight attributes on another domain or of another type are read as Blender's
/// `lookup<float>(name, domain)`: domain adaptation in the source type, then conversion.
/// Expected values from Blender 5.2.4 (`attributes.new` on the factory cube, defaults: white
/// for colours, zero otherwise).
#[test]
fn weight_domain_and_type_conversions_match_blender() {
    let weight = |s: &mut BevelSettings, width: f32, vertices: bool| {
        s.limit_method = "WEIGHT".into();
        s.width = width;
        if vertices {
            s.affect = "VERTICES".into();
        }
    };
    let mut b = vec![false; 8];
    b[..3].fill(true);
    // A BOOLEAN edge is the AND of its two vertices.
    run(
        "bool point->edge",
        with_data(
            cube(),
            "bevel_weight_edge",
            Domain::Point,
            AttrData::Bool(Arc::new(b)),
        ),
        &settings(|s| weight(s, 0.3, false)),
        0,
        ((11, 17, 8), (-2.1, -0.4, -0.4, 29.43)),
    );
    // A BOOLEAN vertex is the OR of its edges.
    let mut b = vec![false; 12];
    b[0] = true;
    run(
        "bool edge->point",
        with_data(
            cube(),
            "bevel_weight_vert",
            Domain::Edge,
            AttrData::Bool(Arc::new(b)),
        ),
        &settings(|s| weight(s, 0.3, true)),
        0,
        ((12, 18, 8), (-3.4, 0.0, -3.4, 32.94)),
    );
    // Integers mix with rounding (half away from zero), then convert.
    let mut i = vec![0i32; 8];
    i[0] = 1;
    i[1] = 2;
    run(
        "int point->edge",
        with_data(
            cube(),
            "bevel_weight_edge",
            Domain::Point,
            AttrData::Int(Arc::new(i)),
        ),
        &settings(|s| weight(s, 0.2, false)),
        0,
        ((16, 27, 13), (-2.4, -2.4, 0.0, 39.68)),
    );
    let mut i = vec![0i8; 12];
    i[0] = 3;
    i[1] = 1;
    run(
        "int8 edge->point",
        with_data(
            cube(),
            "bevel_weight_vert",
            Domain::Edge,
            AttrData::Int8(Arc::new(i)),
        ),
        &settings(|s| weight(s, 0.2, true)),
        0,
        ((12, 18, 8), (-3.6, 0.0, -3.6, 33.84)),
    );
    // FLOAT_VECTOR: the component average.
    let mut v = vec![[0.0f32; 3]; 8];
    v[0] = [0.3, 0.6, 0.9];
    run(
        "vector point",
        with_data(
            cube(),
            "bevel_weight_vert",
            Domain::Point,
            AttrData::Float3(Arc::new(v)),
        ),
        &settings(|s| weight(s, 0.4, true)),
        0,
        ((10, 15, 7), (-1.76, -1.76, -1.76, 28.7328)),
    );
    // FLOAT_COLOR: the luminance.
    let mut c = vec![[1.0f32; 4]; 8];
    c[0] = [1.0, 0.0, 0.0, 1.0];
    c[5] = [0.2, 0.9, 0.4, 1.0];
    run(
        "color point",
        with_data(
            cube(),
            "bevel_weight_vert",
            Domain::Point,
            AttrData::FloatColor(Arc::new(c)),
        ),
        &settings(|s| weight(s, 0.4, true)),
        0,
        ((24, 36, 14), (-0.20099, -0.42893, -0.20099, 58.52071)),
    );
    // BYTE_COLOR on corners: mixed in linear float per edge, stored as bytes, then luminance.
    let mut c = vec![[255u8; 4]; 24];
    c[0] = [255, 188, 137, 255];
    run(
        "bytecolor corner->edge",
        with_data(
            cube(),
            "bevel_weight_edge",
            Domain::Corner,
            AttrData::ByteColor(Arc::new(c)),
        ),
        &settings(|s| weight(s, 0.3, false)),
        0,
        ((24, 48, 26), (-0.14586, -0.07293, -0.07293, 47.93903)),
    );
    let mut f = vec![0.0f32; 6];
    f[0] = 0.8;
    run(
        "float face->edge",
        with_data(
            cube(),
            "bevel_weight_edge",
            Domain::Face,
            AttrData::Float(Arc::new(f)),
        ),
        &settings(|s| weight(s, 0.3, false)),
        0,
        ((12, 20, 10), (-3.52, 0.0, 0.0, 33.2928)),
    );
}

/// An absurd request fails at once with the `--max-elements` payload (LIMIT_EXCEEDED), as
/// `mesh.bevel` does, instead of building geometry for minutes.
#[test]
fn absurd_segments_exceed_the_element_budget_at_once() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let start = std::time::Instant::now();
    let r = catch_unwind(AssertUnwindSafe(|| {
        let _b = frap_math::budget::scope(frap_math::budget::Budget {
            timeout: None,
            max_elements: 10_000_000,
        });
        modify_mesh(
            &settings(|s| {
                s.segments = 1000;
                s.limit_method = "NONE".into();
            }),
            monkey(&Monkey::default()),
            0,
        )
    }));
    let payload = r.expect_err("unwinds");
    assert!(frap_math::budget::elements_exceeded(&*payload).is_some());
    assert!(start.elapsed() < std::time::Duration::from_secs(5));
    // A reasonable request under the same budget runs.
    let _b = frap_math::budget::scope(frap_math::budget::Budget {
        timeout: None,
        max_elements: 10_000_000,
    });
    assert!(modify_mesh(&settings(|s| s.segments = 4), cube(), 0).is_ok());
}
