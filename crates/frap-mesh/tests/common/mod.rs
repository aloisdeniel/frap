// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers for the Blender-dump comparisons (`spec/blender-5.2/primitives/*.json`,
//! written by `tools/blender/dump_primitives.py`).
#![allow(dead_code)]

use std::collections::BTreeMap;

use frap_mesh::{AttrData, Mesh};
use serde_json::Value as J;

/// T1 position/UV/normal tolerance (ARCHITECTURE §2.1).
pub const TOL: f32 = 1e-5;

pub fn spec_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/blender-5.2/primitives")
}

pub fn load(name: &str) -> J {
    let p = spec_dir().join(format!("{name}.json"));
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap()
}

/// All case names with the given prefix.
pub fn cases(prefix: &str) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(spec_dir())
        .unwrap()
        .filter_map(|e| {
            let n = e.ok()?.file_name().into_string().ok()?;
            let n = n.strip_suffix(".json")?.to_string();
            n.starts_with(prefix).then_some(n)
        })
        .collect();
    out.sort();
    out
}

pub fn f3(v: &J) -> Vec<[f32; 3]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let a = x.as_array().unwrap();
            [0, 1, 2].map(|i| a[i].as_f64().unwrap() as f32)
        })
        .collect()
}

pub fn f2(v: &J) -> Vec<[f32; 2]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| {
            let a = x.as_array().unwrap();
            [a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32]
        })
        .collect()
}

pub fn i2(v: &J) -> Vec<[i32; 2]> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| [x[0].as_i64().unwrap() as i32, x[1].as_i64().unwrap() as i32])
        .collect()
}

pub fn ilist(v: &J) -> Vec<i32> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_i64().unwrap() as i32)
        .collect()
}

pub fn faces(v: &J) -> Vec<Vec<i32>> {
    v.as_array().unwrap().iter().map(ilist).collect()
}

pub fn bools(v: &J) -> Vec<bool> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_bool().unwrap())
        .collect()
}

/// Max component difference; panics with context above `tol`.
pub fn close3(what: &str, mine: &[[f32; 3]], want: &[[f32; 3]], tol: f32) -> f32 {
    assert_eq!(mine.len(), want.len(), "{what}: count");
    let mut worst = 0.0f32;
    for (i, (a, b)) in mine.iter().zip(want).enumerate() {
        for k in 0..3 {
            let d = (a[k] - b[k]).abs();
            assert!(d <= tol, "{what}[{i}]: frap {a:?} vs blender {b:?}");
            worst = worst.max(d);
        }
    }
    worst
}

pub fn close2(what: &str, mine: &[[f32; 2]], want: &[[f32; 2]], tol: f32) {
    assert_eq!(mine.len(), want.len(), "{what}: count");
    for (i, (a, b)) in mine.iter().zip(want).enumerate() {
        for k in 0..2 {
            assert!(
                (a[k] - b[k]).abs() <= tol,
                "{what}[{i}]: frap {a:?} vs blender {b:?}"
            );
        }
    }
}

/// Bitwise equality (`f32::to_bits`), for values frap must reproduce exactly on every OS.
pub fn bits3(what: &str, mine: &[[f32; 3]], want: &[[f32; 3]]) {
    assert_eq!(mine.len(), want.len(), "{what}: count");
    for (i, (a, b)) in mine.iter().zip(want).enumerate() {
        assert!(
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "{what}[{i}]: frap {a:?} vs blender {b:?} (not bit-identical)"
        );
    }
}

/// See [`bits3`].
pub fn bits2(what: &str, mine: &[[f32; 2]], want: &[[f32; 2]]) {
    assert_eq!(mine.len(), want.len(), "{what}: count");
    for (i, (a, b)) in mine.iter().zip(want).enumerate() {
        assert!(
            a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits()),
            "{what}[{i}]: frap {a:?} vs blender {b:?} (not bit-identical)"
        );
    }
}

pub fn mesh_faces(m: &Mesh) -> Vec<Vec<i32>> {
    (0..m.faces_num())
        .map(|f| m.corner_verts()[m.face(f)].to_vec())
        .collect()
}

/// Attribute names in order.
pub fn attr_names(m: &Mesh) -> Vec<String> {
    m.attributes().iter().map(|a| a.name.to_string()).collect()
}

pub fn dump_attr_names(jm: &J) -> Vec<String> {
    jm["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["name"].as_str().unwrap().to_string())
        .collect()
}

/// `(domain, data_type, data)` of a dumped attribute.
pub fn dump_attrs(jm: &J) -> BTreeMap<String, (String, String, J)> {
    jm["attributes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| {
            (
                a["name"].as_str().unwrap().to_string(),
                (
                    a["domain"].as_str().unwrap().to_string(),
                    a["data_type"].as_str().unwrap().to_string(),
                    a["data"].clone(),
                ),
            )
        })
        .collect()
}

/// Compares every non-topology attribute (domain, type, values; floats bit-identical with
/// `exact`, within `TOL` otherwise).
pub fn check_attributes(what: &str, m: &Mesh, jm: &J, exact: bool) {
    assert_eq!(
        attr_names(m),
        dump_attr_names(jm),
        "{what}: attribute order"
    );
    let want = dump_attrs(jm);
    for a in m.attributes().iter() {
        let (domain, dtype, data) = &want[a.name.as_str()];
        assert_eq!(a.domain.identifier(), domain, "{what}: {} domain", a.name);
        assert_eq!(a.data.data_type(), dtype, "{what}: {} type", a.name);
        if data.is_null() {
            continue;
        }
        match &a.data {
            AttrData::Bool(v) => {
                assert_eq!(v.as_slice(), bools(data).as_slice(), "{what}: {}", a.name)
            }
            AttrData::Float2(v) if exact => bits2(&format!("{what}: {}", a.name), v, &f2(data)),
            AttrData::Float2(v) => close2(&format!("{what}: {}", a.name), v, &f2(data), TOL),
            AttrData::Float3(v) if exact => bits3(&format!("{what}: {}", a.name), v, &f3(data)),
            AttrData::Float3(v) => {
                close3(&format!("{what}: {}", a.name), v, &f3(data), TOL);
            }
            AttrData::Int(v) => {
                assert_eq!(v.as_slice(), ilist(data).as_slice(), "{what}: {}", a.name)
            }
            other => panic!("{what}: unchecked attribute type {}", other.data_type()),
        }
    }
}

/// Rotation-normalised face cycle (smallest vertex first), for canonical comparisons.
pub fn canon_face(f: &[i32]) -> Vec<i32> {
    let k = f
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .map(|(i, _)| i)
        .unwrap_or(0);
    let mut out = f.to_vec();
    out.rotate_left(k);
    out
}
