// SPDX-License-Identifier: GPL-3.0-or-later
//! Mesh-level rules against Blender 5.2.4 dumps (`spec/blender-5.2/primitives/{mesh,pydata}__*`):
//! `normals_domain`, face/vertex/corner normals (smooth fans split at sharp edges, sharp faces,
//! non-manifold edges and winding flips), loop triangles (quad split rule, `BLI_polyfill_2d`
//! for n-gons) and the Rust `Mesh::from_pydata` (attribute order and values, `mesh_calc_edges`
//! edge order including the >= 1000-face multi-map order).

mod common;

use std::sync::Arc;

use common::*;
use frap_mesh::{AttrData, Attribute, Domain, Mesh};
use serde_json::Value as J;

fn domain(s: &str) -> Domain {
    match s {
        "POINT" => Domain::Point,
        "EDGE" => Domain::Edge,
        "FACE" => Domain::Face,
        _ => Domain::Corner,
    }
}

/// Rebuilds a dumped mesh: topology, then every non-topology attribute in dump order.
fn rebuild(jm: &J) -> Mesh {
    let fs = faces(&jm["faces"]);
    let mut offsets = vec![0i32];
    for f in &fs {
        offsets.push(offsets.last().unwrap() + f.len() as i32);
    }
    let mut m = Mesh::new(jm["name"].as_str().unwrap());
    m.set_positions(f3(&jm["positions"]));
    m.set_topology(offsets, fs.concat(), i2(&jm["edges"]));
    assert_eq!(
        m.corner_edges(),
        ilist(&jm["corner_edges"]).as_slice(),
        "rebuilt corner edges"
    );
    for (name, (dom, dtype, data)) in dump_attrs(jm) {
        if data.is_null() {
            continue;
        }
        let data = match dtype.as_str() {
            "BOOLEAN" => AttrData::Bool(Arc::new(bools(&data))),
            "FLOAT2" => AttrData::Float2(Arc::new(f2(&data))),
            "FLOAT_VECTOR" => AttrData::Float3(Arc::new(f3(&data))),
            "INT16_2D" => AttrData::Int16x2(Arc::new(
                i2(&data)
                    .iter()
                    .map(|v| [v[0] as i16, v[1] as i16])
                    .collect(),
            )),
            other => panic!("unsupported {other}"),
        };
        m.attributes_mut().insert(Attribute {
            name: name.into(),
            domain: domain(&dom),
            data,
        });
    }
    m
}

/// Normals are compared bit for bit (`f32::to_bits`) so CI on every OS fails on any drift.
fn check_derived(what: &str, m: &Mesh, jm: &J) {
    assert_eq!(
        m.normals_domain().identifier(),
        jm["normals_domain"].as_str().unwrap(),
        "{what}: normals_domain"
    );
    for (label, mine, key) in [
        ("face normals", m.face_normals(), "face_normals"),
        ("vertex normals", m.vert_normals(), "vertex_normals"),
        ("corner normals", m.corner_normals(), "corner_normals"),
    ] {
        let want = f3(&jm[key]);
        let label = format!("{what}: {label}");
        bits3(&label, &mine, &want);
    }
    let lt: Vec<Vec<i32>> = m
        .looptris()
        .iter()
        .map(|t| t.iter().map(|&c| c as i32).collect())
        .collect();
    assert_eq!(lt, faces(&jm["loop_triangles"]), "{what}: loop triangles");
    let lf: Vec<i32> = m.looptri_faces().iter().map(|&f| f as i32).collect();
    assert_eq!(
        lf,
        ilist(&jm["loop_triangle_polygons"]),
        "{what}: loop triangle faces"
    );
}

#[test]
fn normals_and_looptris_match_blender() {
    let names = cases("mesh__");
    assert!(names.len() >= 8, "{names:?}");
    for name in names {
        let j = load(&name);
        let jm = &j["mesh"];
        let m = if let Some(input) = j.get("input") {
            from_pydata(input)
        } else {
            rebuild(jm)
        };
        check_derived(&name, &m, jm);
    }
}

fn from_pydata(input: &J) -> Mesh {
    let mut m = Mesh::new("pydata");
    m.from_pydata(
        &f3(&input["vertices"]),
        &i2(&input["edges"]),
        &faces(&input["faces"]),
        input["shade_flat"].as_bool().unwrap(),
    );
    m
}

#[test]
fn from_pydata_matches_blender() {
    let mut names = cases("pydata__");
    names.extend(
        cases("mesh__")
            .into_iter()
            .filter(|n| load(n).get("input").is_some()),
    );
    assert!(names.len() >= 6, "{names:?}");
    for name in names {
        let j = load(&name);
        let jm = &j["mesh"];
        let m = from_pydata(&j["input"]);
        bits3(
            &format!("{name}: positions"),
            m.positions(),
            &f3(&jm["positions"]),
        );
        assert_eq!(m.edges(), i2(&jm["edges"]).as_slice(), "{name}: edges");
        assert_eq!(mesh_faces(&m), faces(&jm["faces"]), "{name}: faces");
        assert_eq!(
            m.corner_edges(),
            ilist(&jm["corner_edges"]).as_slice(),
            "{name}: corner edges"
        );
        check_attributes(&name, &m, jm, true);
        check_derived(&name, &m, jm);
    }
}

#[test]
fn normals_domain_rules() {
    // FACE when every face or every edge is sharp; POINT when nothing is; CORNER otherwise;
    // POINT without faces; custom normals decide by their domain (Blender 5.2 rules).
    let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
    assert_eq!(m.normals_domain().identifier(), "FACE");
    m.shade_smooth();
    assert_eq!(m.normals_domain().identifier(), "POINT");
    m.set_bool_attribute(frap_mesh::SHARP_EDGE, Domain::Edge, vec![false; 12]);
    assert!(
        m.attributes().get(frap_mesh::SHARP_EDGE).is_some(),
        "a plain write keeps an all-false sharp_edge"
    );
    assert_eq!(m.normals_domain().identifier(), "POINT");
    let mut one = vec![false; 12];
    one[3] = true;
    m.set_bool_attribute(frap_mesh::SHARP_EDGE, Domain::Edge, one);
    assert_eq!(m.normals_domain().identifier(), "CORNER");
    m.set_bool_attribute(frap_mesh::SHARP_EDGE, Domain::Edge, vec![true; 12]);
    assert_eq!(m.normals_domain().identifier(), "FACE");
    m.attributes_mut().remove(frap_mesh::SHARP_EDGE);
    m.attributes_mut().insert(Attribute {
        name: frap_mesh::CUSTOM_NORMAL.into(),
        domain: Domain::Corner,
        data: AttrData::Int16x2(Arc::new(vec![[0, 0]; 24])),
    });
    assert_eq!(m.normals_domain().identifier(), "CORNER");
    // Zero custom data leaves the automatic normals unchanged.
    let fresh = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
    let mut smooth = fresh.clone();
    smooth.shade_smooth();
    let auto = smooth.vert_normals();
    let vn = m.vert_normals();
    close3("custom zero", &vn, &auto, 1e-6);
    let empty = Mesh::new("empty");
    assert_eq!(empty.normals_domain().identifier(), "POINT");
    assert!(
        empty.attributes().is_empty(),
        "bpy.data.meshes.new has no attributes"
    );
}

#[test]
fn all_false_flags_are_removed_only_by_bm_to_me() {
    // Blender 5.2.4: RNA proxies and attribute writes keep all-false flags; only
    // BM_mesh_bm_to_me trims them, and shade_smooth removes sharp_face.
    let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
    m.set_bool_attribute(frap_mesh::SHARP_FACE, Domain::Face, vec![false; 6]);
    assert_eq!(m.sharp_faces().unwrap(), &[false; 6]);
    m.set_bool_attribute(frap_mesh::UV_SEAM, Domain::Edge, vec![false; 12]);
    assert!(m.attributes().get(frap_mesh::UV_SEAM).is_some());
    m.set_bool_attribute("my_flag", Domain::Point, vec![false; 8]);
    m.attributes_mut().insert(Attribute {
        name: frap_mesh::SHARP_EDGE.into(),
        domain: Domain::Edge,
        data: AttrData::Bool(Arc::new(vec![false; 12])),
    });
    let mut mixed = vec![false; 12];
    mixed[0] = true;
    m.set_bool_attribute(frap_mesh::UV_SEAM, Domain::Edge, mixed);
    // bm_to_me: all-false flags go, a mixed flag and non-flag booleans stay.
    assert_eq!(
        m.remove_all_false_flags(),
        vec![frap_mesh::SHARP_FACE, frap_mesh::SHARP_EDGE]
    );
    assert!(m.attributes().get(frap_mesh::UV_SEAM).is_some());
    assert!(m.attributes().get("my_flag").is_some());
    // shade_flat / shade_smooth (Python API).
    m.shade_flat();
    assert_eq!(m.sharp_faces().unwrap(), &[true; 6]);
    m.shade_smooth();
    assert!(m.sharp_faces().is_none());
}
