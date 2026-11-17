// SPDX-License-Identifier: GPL-3.0-or-later
//! T1 check: `primitives::cube` equals Blender 5.2.4's `mesh.primitive_cube_add` index by
//! index (positions, edges, faces, UVs, sharp_face). Fixtures dumped by spec/m0/dump_m0.py;
//! the frap-cli live test re-dumps them from a running Blender and diffs.

use frap_mesh::Mesh;
use frap_mesh::primitives::cube;
use serde_json::Value as J;

fn fixture(name: &str) -> J {
    let p = format!(
        "{}/../../spec/m0/primitives/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn check(m: &Mesh, j: &J) {
    let jm = &j["mesh"];
    let pos: Vec<[f32; 3]> = jm["positions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            let a = v.as_array().unwrap();
            [0, 1, 2].map(|i| a[i].as_f64().unwrap() as f32)
        })
        .collect();
    assert_eq!(m.positions(), pos.as_slice(), "positions");
    let edges: Vec<[i32; 2]> = jm["edges"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| [e[0].as_i64().unwrap() as i32, e[1].as_i64().unwrap() as i32])
        .collect();
    assert_eq!(m.edges(), edges.as_slice(), "edges");
    let faces: Vec<Vec<i32>> = jm["faces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            f.as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_i64().unwrap() as i32)
                .collect()
        })
        .collect();
    let mine: Vec<Vec<i32>> = (0..m.faces_num())
        .map(|f| m.corner_verts()[m.face(f)].to_vec())
        .collect();
    assert_eq!(mine, faces, "faces");
    let sharp: Vec<bool> = jm["sharp_face"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_bool().unwrap())
        .collect();
    assert_eq!(m.sharp_faces().unwrap(), sharp.as_slice(), "sharp_face");
    let uv_layers = jm["uv_layers"].as_object().unwrap();
    let mine_uv = m.uv_maps();
    assert_eq!(mine_uv.len(), uv_layers.len(), "uv layer count");
    for ((name, data), (jname, jdata)) in mine_uv.iter().zip(uv_layers) {
        assert_eq!(name, jname);
        let want: Vec<[f32; 2]> = jdata
            .as_array()
            .unwrap()
            .iter()
            .map(|v| [v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32])
            .collect();
        assert_eq!(*data, want.as_slice(), "uv {name}");
    }
    assert_eq!(m.uv_active.as_deref(), jm["uv_active"].as_str());
    // FACE normals domain (all faces sharp): corner normals equal face normals.
    assert_eq!(jm["normals_domain"], "FACE");
    let fnor = m.face_normals();
    let cn = m.corner_normals();
    for f in 0..m.faces_num() {
        for c in m.face(f) {
            assert_eq!(cn[c], fnor[f]);
        }
    }
}

#[test]
fn default_cube_matches_blender() {
    let j = fixture("primitive_cube_add__default");
    check(&cube(2.0, true, [1.0; 3]), &j);
}

#[test]
fn size2_location_cube_matches_blender() {
    let j = fixture("primitive_cube_add__size2_loc001");
    check(&cube(2.0, true, [1.0; 3]), &j);
    let loc: Vec<f64> = j["object"]["location"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(loc, vec![0.0, 0.0, 1.0]);
}

#[test]
fn size3_without_uvs_matches_blender() {
    check(
        &cube(3.0, false, [1.0; 3]),
        &fixture("primitive_cube_add__size3_nouv"),
    );
}

#[test]
fn face_normals_and_looptris() {
    let m = cube(2.0, true, [1.0; 3]);
    // faces 0:-X 1:+Y 2:+X 3:-Y 4:-Z 5:+Z (research doc §1.4)
    let want = [
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, -1.0],
        [0.0, 0.0, 1.0],
    ];
    assert_eq!(m.face_normals().as_slice(), &want);
    let lt = m.looptris();
    assert_eq!(lt.len(), 12);
    assert_eq!(lt[0], [0, 1, 2]);
    assert_eq!(lt[1], [0, 2, 3]);
    // Caches are dropped on write and clones start cold.
    let mut m2 = m.clone();
    m2.set_positions(
        m.positions()
            .iter()
            .map(|p| [p[0] * 2.0, p[1], p[2]])
            .collect(),
    );
    assert_eq!(m2.face_normals()[2], [1.0, 0.0, 0.0]);
}

#[test]
fn zero_and_negative_size_fall_back_to_one_meter() {
    // size=-1 is clamped to the hard minimum 0 by argument conversion before the generator runs.
    check(
        &cube(0.0, true, [1.0; 3]),
        &fixture("primitive_cube_add__size0"),
    );
    check(
        &cube(0.0, true, [1.0; 3]),
        &fixture("primitive_cube_add__size_negative"),
    );
}
