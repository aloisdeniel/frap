// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh_exact` (T1, §20.4): index by index. Topology exact; positions, corner normals, UVs
//! and float attributes within `tol`; selection and active element exact.

use serde_json::Value as Json;

use super::rna_values::{close, json_close, short};
use crate::GMesh;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "mesh_exact";

/// At most this many element mismatches are listed per array (the rest is counted).
pub const MAX_LISTED: usize = 10;

pub(crate) fn floats<const N: usize>(
    comparator: &'static str,
    path: &str,
    frap: &[[f32; N]],
    blender: &[[f32; N]],
    tol: f32,
    out: &mut Vec<Mismatch>,
) {
    if frap.len() != blender.len() {
        out.push(Mismatch::new(
            comparator,
            path,
            format!("frap has {} elements, blender {}", frap.len(), blender.len()),
        ));
        return;
    }
    let mut bad = 0usize;
    for (i, (a, b)) in frap.iter().zip(blender).enumerate() {
        if a.iter()
            .zip(b)
            .all(|(x, y)| close(*x as f64, *y as f64, tol as f64))
        {
            continue;
        }
        bad += 1;
        if bad <= MAX_LISTED {
            out.push(Mismatch::new(
                comparator,
                format!("{path}[{i}]"),
                format!("frap {a:?} != blender {b:?}"),
            ));
        }
    }
    if bad > MAX_LISTED {
        out.push(Mismatch::new(
            comparator,
            path,
            format!("… and {} more differing elements", bad - MAX_LISTED),
        ));
    }
}

pub(crate) fn exact<T: PartialEq + std::fmt::Debug>(
    comparator: &'static str,
    path: &str,
    frap: &[T],
    blender: &[T],
    out: &mut Vec<Mismatch>,
) {
    if frap.len() != blender.len() {
        out.push(Mismatch::new(
            comparator,
            path,
            format!("frap has {} elements, blender {}", frap.len(), blender.len()),
        ));
        return;
    }
    let mut bad = 0usize;
    for (i, (a, b)) in frap.iter().zip(blender).enumerate() {
        if a != b {
            bad += 1;
            if bad <= MAX_LISTED {
                out.push(Mismatch::new(
                    comparator,
                    format!("{path}[{i}]"),
                    format!("frap {a:?} != blender {b:?}"),
                ));
            }
        }
    }
    if bad > MAX_LISTED {
        out.push(Mismatch::new(
            comparator,
            path,
            format!("… and {} more differing elements", bad - MAX_LISTED),
        ));
    }
}

/// Exact equality of edit-mode JSON (selection, active, history).
pub(crate) fn json_exact(
    comparator: &'static str,
    path: &str,
    frap: &Json,
    blender: &Json,
    out: &mut Vec<Mismatch>,
) {
    if frap != blender {
        out.push(Mismatch::new(
            comparator,
            path,
            format!("frap {} != blender {}", short(frap), short(blender)),
        ));
    }
}

/// Compares two meshes index by index. `what` names the mesh in mismatch paths
/// (`meshes["Cube"]`).
pub fn compare(what: &str, frap: &GMesh, blender: &GMesh, tol: f32) -> Vec<Mismatch> {
    let mut out = Vec::new();
    floats(
        NAME,
        &format!("{what}.positions"),
        &frap.positions,
        &blender.positions,
        tol,
        &mut out,
    );
    exact(NAME, &format!("{what}.edges"), &frap.edges, &blender.edges, &mut out);
    exact(NAME, &format!("{what}.faces"), &frap.faces, &blender.faces, &mut out);
    floats(
        NAME,
        &format!("{what}.corner_normals"),
        &frap.corner_normals,
        &blender.corner_normals,
        tol,
        &mut out,
    );
    let fk: Vec<&String> = frap.uv_layers.keys().collect();
    let bk: Vec<&String> = blender.uv_layers.keys().collect();
    if fk != bk {
        out.push(Mismatch::new(
            NAME,
            format!("{what}.uv_layers"),
            format!("frap {fk:?} != blender {bk:?}"),
        ));
    }
    for (name, f) in &frap.uv_layers {
        if let Some(b) = blender.uv_layers.get(name) {
            floats(
                NAME,
                &format!("{what}.uv_layers[{name:?}]"),
                f,
                b,
                tol,
                &mut out,
            );
        }
    }
    let fa = serde_json::to_value(&frap.attributes).expect("invariant: JSON map");
    let ba = serde_json::to_value(&blender.attributes).expect("invariant: JSON map");
    json_close(
        NAME,
        &format!("{what}.attributes"),
        &fa,
        &ba,
        tol as f64,
        &mut out,
    );
    json_exact(
        NAME,
        &format!("{what}.selection"),
        &frap.selection,
        &blender.selection,
        &mut out,
    );
    json_exact(
        NAME,
        &format!("{what}.active"),
        &frap.active,
        &blender.active,
        &mut out,
    );
    json_exact(
        NAME,
        &format!("{what}.select_history"),
        &frap.select_history,
        &blender.select_history,
        &mut out,
    );
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;

    /// Blender's default cube (positions, edges, faces as dumped from 5.2.4).
    pub(crate) fn cube() -> GMesh {
        let positions = vec![
            [1.0, 1.0, 1.0],
            [1.0, 1.0, -1.0],
            [1.0, -1.0, 1.0],
            [1.0, -1.0, -1.0],
            [-1.0, 1.0, 1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [-1.0, -1.0, -1.0],
        ];
        let faces: Vec<Vec<u32>> = vec![
            vec![0, 4, 6, 2],
            vec![3, 2, 6, 7],
            vec![7, 6, 4, 5],
            vec![5, 1, 3, 7],
            vec![1, 0, 2, 3],
            vec![5, 4, 0, 1],
        ];
        let mut edges: Vec<[u32; 2]> = Vec::new();
        for f in &faces {
            for i in 0..f.len() {
                let (a, b) = (f[i], f[(i + 1) % f.len()]);
                let e = [a.min(b), a.max(b)];
                if !edges.contains(&e) {
                    edges.push(e);
                }
            }
        }
        let mut corner_normals = Vec::new();
        let mut uvs = Vec::new();
        for (fi, f) in faces.iter().enumerate() {
            let n = super::super::mesh_canonical::tests::newell(&positions, f);
            for (ci, _) in f.iter().enumerate() {
                corner_normals.push(n);
                uvs.push([fi as f32 * 0.1 + ci as f32 * 0.01, ci as f32 * 0.25]);
            }
        }
        let mut uv_layers = BTreeMap::new();
        uv_layers.insert("UVMap".to_string(), uvs);
        let mut attributes = BTreeMap::new();
        attributes.insert(
            "sharp_face".to_string(),
            json!({"domain": "FACE", "data_type": "BOOLEAN", "data": [true, true, true, true, true, true]}),
        );
        GMesh {
            positions,
            edges,
            faces,
            corner_normals,
            uv_layers,
            attributes,
            selection: Json::Null,
            active: Json::Null,
            select_history: Json::Null,
        }
    }

    #[test]
    fn equal_to_itself_within_tolerance() {
        let a = cube();
        let mut b = a.clone();
        b.positions[3][1] += 1e-7;
        assert_eq!(compare("m", &a, &b, 1e-5), vec![]);
    }

    #[test]
    fn index_order_matters() {
        let a = cube();
        let mut b = a.clone();
        b.positions.swap(0, 1);
        assert_eq!(compare("m", &a, &b, 1e-5).len(), 2);
    }

    #[test]
    fn differs_under_one_change() {
        let a = cube();
        type Change = Box<dyn Fn(&mut GMesh)>;
        let changes: Vec<Change> = vec![
            Box::new(|m| m.positions[5][2] = 0.5),
            Box::new(|m| m.edges[2] = [0, 7]),
            Box::new(|m| m.faces[4][1] = 7),
            Box::new(|m| m.corner_normals[0][0] = 0.3),
            Box::new(|m| m.uv_layers.get_mut("UVMap").unwrap()[3][0] = 0.9),
            Box::new(|m| {
                m.attributes.get_mut("sharp_face").unwrap()["data"][2] = json!(false);
            }),
            Box::new(|m| m.selection = json!({"vert": [0], "edge": [], "face": []})),
            Box::new(|m| m.active = json!({"type": "VERT", "index": 0})),
        ];
        for (i, change) in changes.iter().enumerate() {
            let mut b = a.clone();
            change(&mut b);
            let m = compare("m", &a, &b, 1e-5);
            assert_eq!(m.len(), 1, "change {i}: {m:?}");
        }
    }

    #[test]
    fn long_lists_are_capped() {
        let a = cube();
        let mut b = a.clone();
        for p in b.corner_normals.iter_mut() {
            p[0] += 1.0;
        }
        let m = compare("m", &a, &b, 1e-5);
        assert_eq!(m.len(), MAX_LISTED + 1);
        assert!(m.last().unwrap().detail.contains("14 more"));
    }
}
