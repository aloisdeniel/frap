// SPDX-License-Identifier: GPL-3.0-or-later
//! The vertex-group options of the WP 4.2 modifiers against Blender 5.2.4
//! (`testdata/vertex_groups.json`, written by Blender from a 4×3 grid with the groups `Arm.L`,
//! `Arm.R` and `Other`). Written before frap had a vertex-group API (M4); since WP 6.10 the
//! options are also golden cases (`tests/golden/cases/modifiers/vgroups/`).

use std::collections::BTreeMap;

use frap_mesh::{DeformVert, Mesh};
use frap_scene::generated::modifier_settings::{
    DisplaceSettings, MirrorSettings, SolidifySettings, WeldSettings,
};
use smallvec::smallvec;

use super::util::unit_m4;

fn dump() -> serde_json::Value {
    serde_json::from_str(include_str!("testdata/vertex_groups.json")).expect("JSON")
}

/// `grid(x_subdivisions=3, y_subdivisions=2, size=2)` with the dump's vertex groups.
fn grid() -> Mesh {
    use frap_mesh::primitives::grid::{Grid, grid};
    let mut m = grid(&Grid {
        x_subdivisions: 3,
        y_subdivisions: 2,
        size: 2.0,
        ..Default::default()
    });
    m.vertex_group_names = vec!["Arm.L".into(), "Arm.R".into(), "Other".into()];
    let mut dv = vec![DeformVert::default(); m.verts_num()];
    for (v, g, w) in [
        (0, 0, 1.0),
        (1, 0, 1.0),
        (2, 0, 1.0),
        (5, 0, 1.0),
        (6, 0, 0.25),
        (7, 0, 0.25),
        (3, 1, 0.5),
        (9, 1, 0.5),
        (4, 2, 0.75),
        (8, 2, 0.75),
        (10, 2, 0.75),
        (11, 2, 0.75),
    ] {
        dv[v] = DeformVert {
            weights: smallvec![(g, w)],
        };
    }
    *m.deform_verts_mut() = dv;
    m
}

fn check(case: &str, m: &Mesh) {
    let d = dump();
    let e = &d[case];
    let pos = e["positions"].as_array().unwrap();
    assert_eq!(m.verts_num(), pos.len(), "{case}: vertex count");
    for (i, (p, b)) in m.positions().iter().zip(pos).enumerate() {
        for k in 0..3 {
            let b = b[k].as_f64().unwrap() as f32;
            assert!(
                (p[k] - b).abs() < 1e-5,
                "{case}: vertex {i} frap {p:?} blender {b}"
            );
        }
    }
    let faces = e["faces"].as_array().unwrap();
    assert_eq!(m.faces_num(), faces.len(), "{case}: face count");
    for (i, f) in faces.iter().enumerate() {
        let a: Vec<i64> = m.face(i).map(|c| i64::from(m.corner_verts()[c])).collect();
        let b: Vec<i64> = f
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_i64().unwrap())
            .collect();
        assert_eq!(a, b, "{case}: face {i}");
    }
    let dv = m.deform_verts().expect("weights");
    for (i, w) in e["weights"].as_array().unwrap().iter().enumerate() {
        let b: BTreeMap<String, f32> = w
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.as_f64().unwrap() as f32))
            .collect();
        let a: BTreeMap<String, f32> = dv[i]
            .weights
            .iter()
            .map(|&(g, w)| (m.vertex_group_names[g as usize].to_string(), w))
            .collect();
        assert_eq!(
            a.len(),
            b.len(),
            "{case}: groups of vertex {i}: {a:?} vs {b:?}"
        );
        for (k, bw) in &b {
            assert!(
                (a[k] - bw).abs() < 1e-6,
                "{case}: vertex {i} {k}: {a:?} vs {b:?}"
            );
        }
    }
}

#[test]
fn weld_vertex_group() {
    let g = grid();
    let mut s = WeldSettings {
        merge_threshold: 0.8,
        vertex_group: "Arm.L".into(),
        ..WeldSettings::default()
    };
    check("weld_vgroup", &super::weld(&g, &s).unwrap());
    s.invert_vertex_group = true;
    check("weld_vgroup_invert", &super::weld(&g, &s).unwrap());
    s.mode = "CONNECTED".into();
    check("weld_connected_invert", &super::weld(&g, &s).unwrap());
}

#[test]
fn mirror_vertex_groups() {
    let mut g = grid();
    for p in g.positions_mut() {
        p[0] += 1.0;
    }
    let mut s = MirrorSettings::default();
    check(
        "mirror_vgroups",
        &crate::types::mirror::mirror_apply_on_axis(&s, None, &g, 0),
    );
    s.use_mirror_merge = false;
    check(
        "mirror_vgroups_nomerge",
        &crate::types::mirror::mirror_apply_on_axis(&s, None, &g, 0),
    );
}

#[test]
fn displace_vertex_group() {
    let mut s = DisplaceSettings {
        vertex_group: "Arm.L".into(),
        direction: "Z".into(),
        ..DisplaceSettings::default()
    };
    let mut m = grid();
    crate::types::displace::displace(&s, None, None, &unit_m4(), None, &mut m);
    check("displace_vgroup", &m);
    s.invert_vertex_group = true;
    let mut m = grid();
    crate::types::displace::displace(&s, None, None, &unit_m4(), None, &mut m);
    check("displace_vgroup_invert", &m);
}

#[test]
fn solidify_vertex_groups() {
    let g = grid();
    let mut s = SolidifySettings {
        thickness: 0.3,
        vertex_group: "Other".into(),
        thickness_vertex_group: 0.2,
        shell_vertex_group: "Arm.R".into(),
        rim_vertex_group: "Other".into(),
        ..SolidifySettings::default()
    };
    use crate::types::solidify::{extrude, nonmanifold};
    check(
        "solidify_simple_vgroups",
        &extrude::solidify_extrude(&s, &g, 0),
    );
    s.use_even_offset = true;
    check(
        "solidify_even_vgroups",
        &extrude::solidify_extrude(&s, &g, 0),
    );
    s.use_even_offset = false;
    s.solidify_mode = "NON_MANIFOLD".into();
    check(
        "solidify_complex_vgroups",
        &nonmanifold::solidify_nonmanifold(&s, &g, 0).unwrap(),
    );
    s.use_flat_faces = true;
    check(
        "solidify_complex_flat",
        &nonmanifold::solidify_nonmanifold(&s, &g, 0).unwrap(),
    );
}
