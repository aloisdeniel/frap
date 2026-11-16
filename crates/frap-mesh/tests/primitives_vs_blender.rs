// SPDX-License-Identifier: GPL-3.0-or-later
//! T1: every `frap_mesh::primitives` generator against Blender 5.2.4's `mesh.primitive_*_add`
//! (defaults and >= 2 variants each, `spec/blender-5.2/primitives/<primitive>__*.json`, written by
//! `tools/blender/dump_primitives.py`): vertex/edge/face/corner order, positions within 1e-5,
//! every attribute (order, domain, type, values: `UVMap`, `.select_*`, `sharp_face`,
//! `.uv_select_*`), UV-map flags, `normals_domain`, face/corner/vertex normals and loop
//! triangles. The UV sphere's edge and face order is not stable in Blender itself (see
//! `primitives/uv_sphere.rs`), so its topology is compared canonically.

mod common;

use std::collections::BTreeMap;

use common::*;
use frap_mesh::Mesh;
use frap_mesh::primitives::circle::{Circle, circle};
use frap_mesh::primitives::cone::{Cone, FillType, cone};
use frap_mesh::primitives::cube::{Cube, cube_mesh};
use frap_mesh::primitives::cylinder::{Cylinder, cylinder};
use frap_mesh::primitives::grid::{Grid, grid};
use frap_mesh::primitives::ico_sphere::{IcoSphere, ico_sphere};
use frap_mesh::primitives::monkey::{Monkey, monkey};
use frap_mesh::primitives::plane::{Plane, plane};
use frap_mesh::primitives::torus::{Torus, TorusMode, torus};
use frap_mesh::primitives::uv_sphere::{UvSphere, uv_sphere};
use frap_mesh::primitives::{IDENTITY, scale_matrix};
use serde_json::Value as J;

fn kw_f(kw: &J, k: &str, d: f32) -> f32 {
    kw.get(k)
        .and_then(|v| v.as_f64())
        .map(|v| v as f32)
        .unwrap_or(d)
}
fn kw_i(kw: &J, k: &str, d: i32) -> i32 {
    kw.get(k)
        .and_then(|v| v.as_i64())
        .map(|v| v as i32)
        .unwrap_or(d)
}
fn kw_b(kw: &J, k: &str, d: bool) -> bool {
    kw.get(k).and_then(|v| v.as_bool()).unwrap_or(d)
}
fn kw_fill(kw: &J, k: &str, d: FillType) -> FillType {
    match kw.get(k).and_then(|v| v.as_str()) {
        Some("NOTHING") => FillType::Nothing,
        Some("NGON") => FillType::Ngon,
        Some("TRIFAN") => FillType::Trifan,
        _ => d,
    }
}
fn kw_scale(kw: &J) -> [f32; 3] {
    kw.get("scale")
        .map(|v| f3(&J::Array(vec![v.clone()]))[0])
        .unwrap_or([1.0; 3])
}

/// Runs the generator for a dumped operator call.
fn generate(op: &str, kw: &J) -> Mesh {
    let calc_uvs = kw_b(kw, "calc_uvs", true);
    let scaled = scale_matrix(kw_scale(kw));
    match op {
        "mesh.primitive_plane_add" => plane(&Plane {
            size: kw_f(kw, "size", 2.0),
            calc_uvs,
            matrix: IDENTITY,
        }),
        "mesh.primitive_cube_add" => cube_mesh(&Cube {
            size: kw_f(kw, "size", 2.0),
            calc_uvs,
            matrix: scaled,
        }),
        "mesh.primitive_circle_add" => circle(&Circle {
            vertices: kw_i(kw, "vertices", 32),
            radius: kw_f(kw, "radius", 1.0),
            fill_type: kw_fill(kw, "fill_type", FillType::Nothing),
            calc_uvs,
            matrix: IDENTITY,
        }),
        "mesh.primitive_cylinder_add" => cylinder(&Cylinder {
            vertices: kw_i(kw, "vertices", 32),
            radius: kw_f(kw, "radius", 1.0),
            depth: kw_f(kw, "depth", 2.0),
            end_fill_type: kw_fill(kw, "end_fill_type", FillType::Ngon),
            calc_uvs,
            matrix: scaled,
        }),
        "mesh.primitive_cone_add" => cone(&Cone {
            vertices: kw_i(kw, "vertices", 32),
            radius1: kw_f(kw, "radius1", 1.0),
            radius2: kw_f(kw, "radius2", 0.0),
            depth: kw_f(kw, "depth", 2.0),
            end_fill_type: kw_fill(kw, "end_fill_type", FillType::Ngon),
            calc_uvs,
            matrix: scaled,
        }),
        "mesh.primitive_grid_add" => grid(&Grid {
            x_subdivisions: kw_i(kw, "x_subdivisions", 10),
            y_subdivisions: kw_i(kw, "y_subdivisions", 10),
            size: kw_f(kw, "size", 2.0),
            calc_uvs,
            matrix: IDENTITY,
        }),
        "mesh.primitive_monkey_add" => monkey(&Monkey {
            size: kw_f(kw, "size", 2.0),
            calc_uvs,
            matrix: IDENTITY,
        }),
        "mesh.primitive_uv_sphere_add" => uv_sphere(&UvSphere {
            segments: kw_i(kw, "segments", 32),
            ring_count: kw_i(kw, "ring_count", 16),
            radius: kw_f(kw, "radius", 1.0),
            calc_uvs,
            matrix: scaled,
        }),
        "mesh.primitive_ico_sphere_add" => ico_sphere(&IcoSphere {
            subdivisions: kw_i(kw, "subdivisions", 2),
            radius: kw_f(kw, "radius", 1.0),
            calc_uvs,
            matrix: scaled,
        }),
        "mesh.primitive_torus_add" => torus(&Torus {
            major_segments: kw_i(kw, "major_segments", 48),
            minor_segments: kw_i(kw, "minor_segments", 12),
            mode: if kw.get("mode").and_then(|m| m.as_str()) == Some("EXT_INT") {
                TorusMode::ExtInt
            } else {
                TorusMode::MajorMinor
            },
            major_radius: kw_f(kw, "major_radius", 1.0),
            minor_radius: kw_f(kw, "minor_radius", 0.25),
            abso_major_rad: kw_f(kw, "abso_major_rad", 1.25),
            abso_minor_rad: kw_f(kw, "abso_minor_rad", 0.75),
            generate_uvs: kw_b(kw, "generate_uvs", true),
        }),
        other => panic!("no generator for {other}"),
    }
}

/// Compares one primitive case; `canonical_topology` for the UV sphere.
fn check_case(name: &str) {
    check_json(name, &load(name));
}

fn check_json(name: &str, j: &J) {
    let op = j["op"].as_str().unwrap();
    let m = generate(op, &j["kwargs"]);
    let jm = &j["mesh"];
    let what = name;
    assert_eq!(
        m.header.name.as_str(),
        jm["name"].as_str().unwrap(),
        "{what}: mesh name"
    );
    let want_pos = f3(&jm["positions"]);
    close3(&format!("{what}: positions"), m.positions(), &want_pos, TOL);
    let exact = m
        .positions()
        .iter()
        .zip(&want_pos)
        .filter(|(a, b)| a == b)
        .count();
    eprintln!(
        "{what}: {exact}/{} positions bit-identical to Blender",
        want_pos.len()
    );
    let canonical = op == "mesh.primitive_uv_sphere_add";
    if canonical {
        check_canonical(what, &m, jm);
    } else {
        assert_eq!(m.edges(), i2(&jm["edges"]).as_slice(), "{what}: edges");
        assert_eq!(mesh_faces(&m), faces(&jm["faces"]), "{what}: faces");
        assert_eq!(
            m.corner_edges(),
            ilist(&jm["corner_edges"]).as_slice(),
            "{what}: corner edges"
        );
        check_attributes(what, &m, jm);
        close3(
            &format!("{what}: face normals"),
            &m.face_normals(),
            &f3(&jm["face_normals"]),
            TOL,
        );
        close3(
            &format!("{what}: corner normals"),
            &m.corner_normals(),
            &f3(&jm["corner_normals"]),
            TOL,
        );
        let lt: Vec<[i32; 3]> = m.looptris().iter().map(|t| t.map(|c| c as i32)).collect();
        let want_lt: Vec<[i32; 3]> = faces(&jm["loop_triangles"])
            .iter()
            .map(|t| [t[0], t[1], t[2]])
            .collect();
        assert_eq!(lt, want_lt, "{what}: loop triangles");
    }
    close3(
        &format!("{what}: vertex normals"),
        &m.vert_normals(),
        &f3(&jm["vertex_normals"]),
        TOL,
    );
    assert_eq!(
        m.normals_domain().identifier(),
        jm["normals_domain"].as_str().unwrap(),
        "{what}: normals_domain"
    );
    let layers: Vec<(String, bool, bool)> = jm["uv_layers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| {
            (
                l["name"].as_str().unwrap().into(),
                l["active"].as_bool().unwrap(),
                l["active_render"].as_bool().unwrap(),
            )
        })
        .collect();
    let mine: Vec<(String, bool, bool)> = m
        .uv_maps()
        .iter()
        .map(|(n, _)| {
            (
                n.to_string(),
                m.uv_active.as_deref() == Some(*n),
                m.uv_active_render.as_deref() == Some(*n),
            )
        })
        .collect();
    assert_eq!(mine, layers, "{what}: UV layers");
}

/// UV sphere: vertices index-exact (checked by the caller), edges as a set, faces as a multiset
/// of rotation-normalised cycles with per-corner UVs, attributes by name/order and per-element
/// counts.
fn check_canonical(what: &str, m: &Mesh, jm: &J) {
    let norm_edges = |e: &[[i32; 2]]| {
        let mut v: Vec<[i32; 2]> = e.iter().map(|e| [e[0].min(e[1]), e[0].max(e[1])]).collect();
        v.sort();
        v
    };
    assert_eq!(
        norm_edges(m.edges()),
        norm_edges(&i2(&jm["edges"])),
        "{what}: edge set"
    );
    let uv_mine = m.uv_maps().first().map(|(_, d)| d.to_vec());
    let uv_want = dump_attrs(jm).get("UVMap").map(|(_, _, d)| f2(d));
    let key = |faces: &[Vec<i32>], offsets: &[usize], uv: Option<&Vec<[f32; 2]>>| {
        let mut out: BTreeMap<Vec<i32>, Vec<[f32; 2]>> = BTreeMap::new();
        for (fi, f) in faces.iter().enumerate() {
            let c = canon_face(f);
            let rot = f.iter().position(|v| *v == c[0]).unwrap();
            let uvs = uv
                .map(|uv| {
                    (0..f.len())
                        .map(|k| uv[offsets[fi] + (rot + k) % f.len()])
                        .collect()
                })
                .unwrap_or_default();
            assert!(out.insert(c, uvs).is_none(), "{what}: duplicate face");
        }
        out
    };
    let mf = mesh_faces(m);
    let jf = faces(&jm["faces"]);
    let offs = |f: &[Vec<i32>]| {
        let mut o = vec![0usize];
        for x in f {
            o.push(o.last().unwrap() + x.len());
        }
        o
    };
    let a = key(&mf, &offs(&mf), uv_mine.as_ref());
    let b = key(&jf, &offs(&jf), uv_want.as_ref());
    assert_eq!(a.len(), b.len(), "{what}: face count");
    for ((fa, ua), (fb, ub)) in a.iter().zip(b.iter()) {
        assert_eq!(fa, fb, "{what}: face set");
        close2(&format!("{what}: UVs of face {fa:?}"), ua, ub, TOL);
    }
    assert_eq!(
        attr_names(m),
        dump_attr_names(jm),
        "{what}: attribute order"
    );
    for (name, (domain, dtype, data)) in dump_attrs(jm) {
        let a = m.attributes().get(&name).unwrap();
        assert_eq!(a.domain.identifier(), domain, "{what}: {name} domain");
        assert_eq!(a.data.data_type(), dtype, "{what}: {name} type");
        if dtype == "BOOLEAN" {
            let want = bools(&data);
            match &a.data {
                frap_mesh::AttrData::Bool(v) => {
                    assert_eq!(v.len(), want.len(), "{what}: {name} len");
                    assert_eq!(
                        v.iter().filter(|x| **x).count(),
                        want.iter().filter(|x| **x).count(),
                        "{what}: {name} true count"
                    );
                }
                _ => unreachable!(),
            }
        }
    }
}

macro_rules! primitive_tests {
    ($($test:ident => $prefix:literal, $min_variants:literal;)*) => {$(
        #[test]
        fn $test() {
            let names = cases($prefix);
            assert!(names.iter().any(|n| n.ends_with("__default")), "{} has no default case", $prefix);
            assert!(names.len() > $min_variants, "{}: need defaults + {} variants", $prefix, $min_variants);
            for n in names {
                check_case(&n);
            }
        }
    )*};
}

primitive_tests! {
    plane_matches_blender => "plane__", 2;
    cube_matches_blender => "cube__", 2;
    circle_matches_blender => "circle__", 2;
    uv_sphere_matches_blender => "uv_sphere__", 2;
    ico_sphere_matches_blender => "ico_sphere__", 2;
    cylinder_matches_blender => "cylinder__", 2;
    cone_matches_blender => "cone__", 2;
    grid_matches_blender => "grid__", 2;
    monkey_matches_blender => "monkey__", 2;
    torus_matches_blender => "torus__", 2;
}

/// Live parameter sweep (`FRAP_GOLDEN_LIVE=1` + Blender): `dump_primitives.py --sweep` (every
/// circle/cylinder/cone fill type and radius combination for 3..13 segments, ico spheres up to 5
/// subdivisions, UV spheres up to 64×32, grids, tori) compared like the committed cases.
#[test]
fn sweep_matches_live_blender() {
    if std::env::var("FRAP_GOLDEN_LIVE").as_deref() != Ok("1") {
        eprintln!("NOTICE: sweep_matches_live_blender skipped (set FRAP_GOLDEN_LIVE=1 to run)");
        return;
    }
    let bin = std::env::var_os("BLENDER_BIN")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            let p = std::path::PathBuf::from(std::env::var_os("HOME")?)
                .join("Applications/Blender-5.2.app/Contents/MacOS/Blender");
            p.is_file().then_some(p)
        });
    let Some(bin) = bin else {
        eprintln!(
            "NOTICE: sweep_matches_live_blender skipped (Blender 5.2 not found; set BLENDER_BIN)"
        );
        return;
    };
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = std::env::temp_dir().join(format!("frap-primitives-sweep-{}", std::process::id()));
    let status = std::process::Command::new(bin)
        .args(["-b", "--factory-startup", "--python"])
        .arg(root.join("tools/blender/dump_primitives.py"))
        .args(["--", "--sweep", "--out"])
        .arg(&out)
        .status()
        .expect("run Blender");
    assert!(status.success());
    let mut n = 0;
    for entry in std::fs::read_dir(&out).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_stem().unwrap().to_string_lossy().to_string();
        if !name.starts_with("sweep__") {
            continue;
        }
        let j: J = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        check_json(&name, &j);
        n += 1;
    }
    let _ = std::fs::remove_dir_all(&out);
    assert!(n >= 253, "{n} sweep cases");
}

#[test]
fn generators_are_deterministic() {
    // Bit-identical output for identical input (CI compares across OSes through the fixtures).
    let a = uv_sphere(&UvSphere::default());
    let b = uv_sphere(&UvSphere::default());
    assert_eq!(a.positions(), b.positions());
    let a = ico_sphere(&IcoSphere {
        subdivisions: 3,
        ..Default::default()
    });
    let b = ico_sphere(&IcoSphere {
        subdivisions: 3,
        ..Default::default()
    });
    assert_eq!(a.positions(), b.positions());
    assert_eq!(a.edges(), b.edges());
}
