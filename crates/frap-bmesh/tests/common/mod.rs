// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared helpers of the BMesh core tests: rebuilding a `frap_mesh::Mesh` from a Blender mesh
//! dump (`tests/blender/dump_core.py`) and comparing meshes at T1.
#![allow(dead_code)]

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, AttributeSet, DeformVert, Domain, Mesh};
use serde_json::Value as J;
use smol_str::SmolStr;

/// T1 tolerance for positions, UVs and other float values (ARCHITECTURE §2.1).
pub const TOL: f64 = 1e-5;

pub fn fixture() -> J {
    let p =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core.blender.json");
    serde_json::from_str(
        &std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
    )
    .unwrap()
}

fn f(v: &J) -> f32 {
    v.as_f64().unwrap() as f32
}
fn fa<const N: usize>(v: &J) -> [f32; N] {
    let a = v.as_array().unwrap();
    std::array::from_fn(|i| f(&a[i]))
}
fn ia<const N: usize>(v: &J) -> [i64; N] {
    let a = v.as_array().unwrap();
    std::array::from_fn(|i| a[i].as_i64().unwrap())
}
fn arr(v: &J) -> &Vec<J> {
    v.as_array().unwrap()
}

pub fn domain(s: &str) -> Domain {
    match s {
        "POINT" => Domain::Point,
        "EDGE" => Domain::Edge,
        "FACE" => Domain::Face,
        "CORNER" => Domain::Corner,
        _ => panic!("domain {s}"),
    }
}

/// The column of a dumped attribute.
pub fn attr_data(t: &str, data: &J) -> AttrData {
    let d = arr(data);
    match t {
        "BOOLEAN" => AttrData::Bool(Arc::new(d.iter().map(|x| x.as_bool().unwrap()).collect())),
        "FLOAT" => AttrData::Float(Arc::new(d.iter().map(f).collect())),
        "INT" => AttrData::Int(Arc::new(
            d.iter().map(|x| x.as_i64().unwrap() as i32).collect(),
        )),
        "INT8" => AttrData::Int8(Arc::new(
            d.iter().map(|x| x.as_i64().unwrap() as i8).collect(),
        )),
        "FLOAT2" => AttrData::Float2(Arc::new(d.iter().map(fa::<2>).collect())),
        "FLOAT_VECTOR" => AttrData::Float3(Arc::new(d.iter().map(fa::<3>).collect())),
        "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(d.iter().map(fa::<4>).collect())),
        "BYTE_COLOR" => AttrData::ByteColor(Arc::new(
            d.iter().map(|x| ia::<4>(x).map(|v| v as u8)).collect(),
        )),
        "INT16_2D" => AttrData::Int16x2(Arc::new(
            d.iter().map(|x| ia::<2>(x).map(|v| v as i16)).collect(),
        )),
        "INT32_2D" => AttrData::Int32x2(Arc::new(
            d.iter().map(|x| ia::<2>(x).map(|v| v as i32)).collect(),
        )),
        "QUATERNION" => AttrData::Quaternion(Arc::new(d.iter().map(fa::<4>).collect())),
        "STRING" => AttrData::String(Arc::new(
            d.iter()
                .map(|x| SmolStr::from(x.as_str().unwrap()))
                .collect(),
        )),
        _ => panic!("attribute type {t}"),
    }
}

/// Rebuilds a mesh from a dump: topology, every attribute in dump order, vertex groups and the
/// active/render UV map.
pub fn mesh_from_dump(m: &J) -> Mesh {
    let positions: Vec<[f32; 3]> = arr(&m["positions"]).iter().map(fa::<3>).collect();
    let edges: Vec<[i32; 2]> = arr(&m["edges"])
        .iter()
        .map(|e| ia::<2>(e).map(|v| v as i32))
        .collect();
    let mut offsets = vec![0i32];
    let mut corner_verts = Vec::new();
    for fc in arr(&m["faces"]) {
        corner_verts.extend(arr(fc).iter().map(|v| v.as_i64().unwrap() as i32));
        offsets.push(corner_verts.len() as i32);
    }
    let corner_edges: Vec<i32> = arr(&m["corner_edges"])
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect();
    let mut me = Mesh::new("Mesh");
    me.set_topology(offsets, corner_verts.clone(), edges.clone());
    let mut set = AttributeSet::default();
    for a in arr(&m["attributes"]) {
        let name = a["name"].as_str().unwrap();
        let d = domain(a["domain"].as_str().unwrap());
        let data = match name {
            "position" => AttrData::Float3(Arc::new(positions.clone())),
            ".edge_verts" => AttrData::Int32x2(Arc::new(edges.clone())),
            ".corner_vert" => AttrData::Int(Arc::new(corner_verts.clone())),
            ".corner_edge" => AttrData::Int(Arc::new(corner_edges.clone())),
            _ => attr_data(a["data_type"].as_str().unwrap(), &a["data"]),
        };
        set.insert(Attribute {
            name: name.into(),
            domain: d,
            data,
        });
    }
    *me.attributes_mut() = set;
    if let Some(groups) = m.get("deform") {
        let dv: Vec<DeformVert> = arr(groups)
            .iter()
            .map(|g| {
                let mut d = DeformVert::default();
                for p in arr(g) {
                    d.weights
                        .push((p[0].as_i64().unwrap() as u32, p[1].as_f64().unwrap() as f32));
                }
                d
            })
            .collect();
        *me.deform_verts_mut() = dv;
    }
    me.uv_active = m["uv_active"].as_str().map(SmolStr::from);
    me.uv_active_render = m["uv_render"].as_str().map(SmolStr::from);
    me
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= TOL
}

fn cmp_floats(what: &str, a: &[f32], b: &[f64], errs: &mut Vec<String>) {
    if a.len() != b.len() {
        errs.push(format!("{what}: {} values, Blender {}", a.len(), b.len()));
        return;
    }
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        if !close(*x as f64, *y) {
            errs.push(format!("{what}[{i}]: {x} vs Blender {y}"));
            return;
        }
    }
}

fn flat(v: &J) -> Vec<f64> {
    match v {
        J::Array(a) => a.iter().flat_map(flat).collect(),
        J::Number(n) => vec![n.as_f64().unwrap()],
        J::Bool(b) => vec![*b as i32 as f64],
        _ => vec![],
    }
}

/// Flattened values of a frap column (byte colours as linear floats like the dump).
fn col_values(d: &AttrData) -> Vec<f64> {
    match d {
        AttrData::Bool(v) => v.iter().map(|&b| b as i32 as f64).collect(),
        AttrData::Float(v) => v.iter().map(|&x| x as f64).collect(),
        AttrData::Int(v) => v.iter().map(|&x| x as f64).collect(),
        AttrData::Int8(v) => v.iter().map(|&x| x as f64).collect(),
        AttrData::Float2(v) => v.iter().flatten().map(|&x| x as f64).collect(),
        AttrData::Float3(v) => v.iter().flatten().map(|&x| x as f64).collect(),
        AttrData::Float4(v) | AttrData::FloatColor(v) | AttrData::Quaternion(v) => {
            v.iter().flatten().map(|&x| x as f64).collect()
        }
        AttrData::ByteColor(v) => v.iter().flatten().map(|&x| x as f64).collect(),
        AttrData::Int16x2(v) => v.iter().flatten().map(|&x| x as f64).collect(),
        AttrData::Int32x2(v) => v.iter().flatten().map(|&x| x as f64).collect(),
        _ => vec![],
    }
}

/// Every T1 difference between a frap mesh and a Blender dump (empty when equal).
pub fn compare_mesh(me: &Mesh, b: &J, skip_attrs: &[&str]) -> Vec<String> {
    let mut errs = Vec::new();
    let pos: Vec<f32> = me.positions().iter().flatten().copied().collect();
    cmp_floats("positions", &pos, &flat(&b["positions"]), &mut errs);
    let edges: Vec<[i64; 2]> = me
        .edges()
        .iter()
        .map(|e| [e[0] as i64, e[1] as i64])
        .collect();
    let bedges: Vec<[i64; 2]> = arr(&b["edges"]).iter().map(ia::<2>).collect();
    if edges != bedges {
        errs.push(format!(
            "edges differ:\n  frap    {edges:?}\n  Blender {bedges:?}"
        ));
    }
    let faces: Vec<Vec<i64>> = (0..me.faces_num())
        .map(|i| {
            me.corner_verts()[me.face(i)]
                .iter()
                .map(|&v| v as i64)
                .collect()
        })
        .collect();
    let bfaces: Vec<Vec<i64>> = arr(&b["faces"])
        .iter()
        .map(|f| arr(f).iter().map(|v| v.as_i64().unwrap()).collect())
        .collect();
    if faces != bfaces {
        errs.push(format!(
            "faces differ:\n  frap    {faces:?}\n  Blender {bfaces:?}"
        ));
    }
    let ce: Vec<i64> = me.corner_edges().iter().map(|&e| e as i64).collect();
    let bce: Vec<i64> = arr(&b["corner_edges"])
        .iter()
        .map(|v| v.as_i64().unwrap())
        .collect();
    if ce != bce {
        errs.push(format!(
            "corner edges differ:\n  frap    {ce:?}\n  Blender {bce:?}"
        ));
    }
    let names: Vec<String> = me
        .attributes()
        .iter()
        .map(|a| {
            format!(
                "{}:{}:{}",
                a.name,
                a.domain.identifier(),
                a.data.data_type()
            )
        })
        .collect();
    let bnames: Vec<String> = arr(&b["attributes"])
        .iter()
        .map(|a| {
            format!(
                "{}:{}:{}",
                a["name"].as_str().unwrap(),
                a["domain"].as_str().unwrap(),
                a["data_type"].as_str().unwrap()
            )
        })
        .collect();
    if names != bnames {
        errs.push(format!(
            "attribute order differs:\n  frap    {names:?}\n  Blender {bnames:?}"
        ));
    }
    for ba in arr(&b["attributes"]) {
        let name = ba["name"].as_str().unwrap();
        if ba["data"].is_null() || skip_attrs.contains(&name) {
            continue;
        }
        let Some(a) = me.attributes().get(name) else {
            continue;
        };
        if let (AttrData::String(v), J::Array(bv)) = (&a.data, &ba["data"]) {
            let bs: Vec<&str> = bv.iter().map(|x| x.as_str().unwrap()).collect();
            let fs: Vec<&str> = v.iter().map(|s| s.as_str()).collect();
            if fs != bs {
                errs.push(format!("attribute {name}: {fs:?} vs Blender {bs:?}"));
            }
            continue;
        }
        let fv = col_values(&a.data);
        let bv = flat(&ba["data"]);
        let tol = TOL;
        if fv.len() != bv.len() {
            errs.push(format!(
                "attribute {name}: {} values vs Blender {}",
                fv.len(),
                bv.len()
            ));
            continue;
        }
        if let Some(i) = (0..fv.len()).find(|&i| (fv[i] - bv[i]).abs() > tol) {
            errs.push(format!(
                "attribute {name}[{i}]: {} vs Blender {} (all: {fv:?} vs {bv:?})",
                fv[i], bv[i]
            ));
        }
    }
    if !me.uv_maps().is_empty() {
        let uva = me.uv_active.as_deref();
        if b["uv_active"].as_str() != uva {
            errs.push(format!("uv_active {uva:?} vs Blender {:?}", b["uv_active"]));
        }
        let uvr = me.uv_active_render.as_deref();
        if b["uv_render"].as_str() != uvr {
            errs.push(format!("uv_render {uvr:?} vs Blender {:?}", b["uv_render"]));
        }
    }
    match (me.deform_verts(), b.get("deform")) {
        (Some(dv), Some(bd)) => {
            let fd: Vec<Vec<(u32, f32)>> = dv.iter().map(|d| d.weights.to_vec()).collect();
            let bdv: Vec<Vec<(u32, f32)>> = arr(bd)
                .iter()
                .map(|g| {
                    arr(g)
                        .iter()
                        .map(|p| (p[0].as_i64().unwrap() as u32, p[1].as_f64().unwrap() as f32))
                        .collect()
                })
                .collect();
            if fd.len() != bdv.len()
                || fd.iter().zip(&bdv).any(|(a, b)| {
                    a.len() != b.len()
                        || a.iter()
                            .zip(b)
                            .any(|(x, y)| x.0 != y.0 || (x.1 - y.1).abs() > 1e-5)
                })
            {
                errs.push(format!(
                    "deform verts differ:\n  frap    {fd:?}\n  Blender {bdv:?}"
                ));
            }
        }
        (None, None) => {}
        (Some(dv), None) if dv.iter().all(|d| d.weights.is_empty()) => {}
        (a, b) => errs.push(format!(
            "deform presence differs: frap {} Blender {}",
            a.is_some(),
            b.is_some()
        )),
    }
    errs
}
