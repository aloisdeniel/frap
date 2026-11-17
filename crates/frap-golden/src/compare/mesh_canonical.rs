// SPDX-License-Identifier: GPL-3.0-or-later
//! `mesh_canonical` (T2, §20.4): order-free mesh comparison.
//!
//! - Vertices are matched by quantised position (1e-4 grid, or `tol` when larger); ties
//!   (coincident vertices) are broken by the neighbourhood (quantised positions of edge
//!   neighbours), then by index.
//! - Faces become rotation-normalised cycles of matched vertices (winding kept) and are
//!   compared as a multiset; edges as a set of matched vertex pairs.
//! - Corner normals, UVs and corner attributes are compared per (face key, corner position);
//!   point/edge/face attributes through the vertex/edge/face matching; selection and active
//!   element through the same matching.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value as Json;

use super::mesh_exact::{MAX_LISTED, floats, json_exact};
use super::rna_values::json_close;
use crate::GMesh;
use crate::case::Mismatch;

/// Comparator name.
pub const NAME: &str = "mesh_canonical";

/// Quantisation grid for vertex matching (§20.4).
pub const GRID: f32 = 1e-4;

type Cell = [i64; 3];

fn cell(p: [f32; 3], grid: f32) -> Cell {
    p.map(|x| (x / grid).round() as i64)
}

fn neighbours(m: &GMesh) -> Vec<Vec<u32>> {
    let mut n = vec![Vec::new(); m.positions.len()];
    for e in &m.edges {
        let (a, b) = (e[0] as usize, e[1] as usize);
        if a < n.len() && b < n.len() {
            n[a].push(e[1]);
            n[b].push(e[0]);
        }
    }
    n
}

fn signature(m: &GMesh, nb: &[Vec<u32>], v: usize, grid: f32) -> Vec<Cell> {
    let mut s: Vec<Cell> = nb[v]
        .iter()
        .map(|&w| cell(m.positions[w as usize], grid))
        .collect();
    s.sort();
    s
}

/// Matches every frap vertex to a blender vertex. `Err` lists unmatched frap vertices.
fn match_vertices(frap: &GMesh, blender: &GMesh, grid: f32) -> Result<Vec<usize>, Vec<usize>> {
    let mut grid_map: HashMap<Cell, Vec<usize>> = HashMap::new();
    for (i, p) in blender.positions.iter().enumerate() {
        grid_map.entry(cell(*p, grid)).or_default().push(i);
    }
    let (fnb, bnb) = (neighbours(frap), neighbours(blender));
    let mut used = vec![false; blender.positions.len()];
    let mut map = vec![usize::MAX; frap.positions.len()];
    let mut unmatched = Vec::new();
    for (i, p) in frap.positions.iter().enumerate() {
        let c = cell(*p, grid);
        let mut cands: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = grid_map.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                        cands.extend(v.iter().copied().filter(|&j| {
                            !used[j]
                                && blender.positions[j]
                                    .iter()
                                    .zip(p)
                                    .all(|(a, b)| (a - b).abs() <= grid)
                        }));
                    }
                }
            }
        }
        cands.sort_unstable();
        let pick = if cands.len() > 1 {
            let sig = signature(frap, &fnb, i, grid);
            cands
                .iter()
                .copied()
                .find(|&j| signature(blender, &bnb, j, grid) == sig)
                .or(cands.first().copied())
        } else {
            cands.first().copied()
        };
        match pick {
            Some(j) => {
                used[j] = true;
                map[i] = j;
            }
            None => unmatched.push(i),
        }
    }
    if unmatched.is_empty() {
        Ok(map)
    } else {
        Err(unmatched)
    }
}

/// Rotation that puts the smallest element first.
fn canonical_rotation(cycle: &[u32]) -> usize {
    cycle
        .iter()
        .enumerate()
        .min_by_key(|(_, v)| **v)
        .map(|(i, _)| i)
        .unwrap_or(0)
}

fn rotated(cycle: &[u32], r: usize) -> Vec<u32> {
    (0..cycle.len()).map(|k| cycle[(r + k) % cycle.len()]).collect()
}

fn corner_offsets(m: &GMesh) -> Vec<usize> {
    let mut o = Vec::with_capacity(m.faces.len() + 1);
    let mut acc = 0;
    o.push(0);
    for f in &m.faces {
        acc += f.len();
        o.push(acc);
    }
    o
}

/// Reorders a per-element JSON array (`frap` order) into blender order with `map[frap] =
/// blender`. `None` when the lengths do not fit.
fn reorder(data: &Json, map: &[usize], n: usize) -> Option<Json> {
    let a = data.as_array()?;
    if a.len() != map.len() || map.iter().any(|&j| j >= n) {
        return None;
    }
    let mut out = vec![Json::Null; n];
    for (i, &j) in map.iter().enumerate() {
        out[j] = a[i].clone();
    }
    Some(Json::Array(out))
}

fn remap_selection(sel: &Json, maps: &[(&str, &[usize])]) -> Json {
    match sel {
        Json::Object(o) => {
            let mut out = serde_json::Map::new();
            for (k, v) in o {
                let map = maps.iter().find(|(name, _)| name == k).map(|(_, m)| *m);
                let v = match v.as_array() {
                    Some(a) if a.iter().all(|x| x.is_u64()) => {
                        let mut idx: Vec<u64> = a
                            .iter()
                            .filter_map(|x| x.as_u64())
                            .map(|i| match map {
                                Some(m) => m.get(i as usize).map_or(u64::MAX, |j| *j as u64),
                                None => i,
                            })
                            .collect();
                        idx.sort_unstable();
                        Json::from(idx)
                    }
                    _ => v.clone(),
                };
                out.insert(k.clone(), v);
            }
            Json::Object(out)
        }
        _ => sel.clone(),
    }
}

fn remap_element(el: &Json, maps: &[(&str, &[usize])]) -> Json {
    let (Some(t), Some(i)) = (
        el.get("type").and_then(|t| t.as_str()),
        el.get("index").and_then(|i| i.as_u64()),
    ) else {
        return el.clone();
    };
    let key = match t {
        "VERT" => "vert",
        "EDGE" => "edge",
        "FACE" => "face",
        _ => return el.clone(),
    };
    let Some((_, m)) = maps.iter().find(|(name, _)| *name == key) else {
        return el.clone();
    };
    let j = m.get(i as usize).map_or(u64::MAX, |j| *j as u64);
    serde_json::json!({"type": t, "index": j})
}

/// Compares two meshes order-free.
pub fn compare(what: &str, frap: &GMesh, blender: &GMesh, tol: f32) -> Vec<Mismatch> {
    let mut out = Vec::new();
    let grid = GRID.max(tol);
    let counts = |m: &GMesh| (m.positions.len(), m.edges.len(), m.faces.len());
    if counts(frap) != counts(blender) {
        out.push(Mismatch::new(
            NAME,
            what,
            format!(
                "frap (verts, edges, faces) {:?} != blender {:?}",
                counts(frap),
                counts(blender)
            ),
        ));
        return out;
    }
    let vmap = match match_vertices(frap, blender, grid) {
        Ok(m) => m,
        Err(unmatched) => {
            for &i in unmatched.iter().take(MAX_LISTED) {
                out.push(Mismatch::new(
                    NAME,
                    format!("{what}.positions[{i}]"),
                    format!("frap vertex {:?} has no blender counterpart", frap.positions[i]),
                ));
            }
            if unmatched.len() > MAX_LISTED {
                out.push(Mismatch::new(
                    NAME,
                    format!("{what}.positions"),
                    format!("… and {} more unmatched vertices", unmatched.len() - MAX_LISTED),
                ));
            }
            return out;
        }
    };
    // Positions in blender order.
    let mut pos = vec![[0.0f32; 3]; frap.positions.len()];
    for (i, &j) in vmap.iter().enumerate() {
        pos[j] = frap.positions[i];
    }
    floats(NAME, &format!("{what}.positions"), &pos, &blender.positions, tol, &mut out);

    // Edges: an injective matching (a duplicated frap edge standing in for a missing one is
    // reported both as the duplicate and as the blender edge left unmatched).
    let mut bedge: HashMap<[u32; 2], Vec<usize>> = HashMap::new();
    for (i, e) in blender.edges.iter().enumerate().rev() {
        bedge
            .entry([e[0].min(e[1]), e[0].max(e[1])])
            .or_default()
            .push(i);
    }
    let mut emap = vec![usize::MAX; frap.edges.len()];
    let mut bad_edges = 0;
    for (i, e) in frap.edges.iter().enumerate() {
        let (a, b) = (
            vmap.get(e[0] as usize).map_or(u32::MAX, |j| *j as u32),
            vmap.get(e[1] as usize).map_or(u32::MAX, |j| *j as u32),
        );
        let key = [a.min(b), a.max(b)];
        let msg = match bedge.get_mut(&key) {
            Some(free) => match free.pop() {
                Some(j) => {
                    emap[i] = j;
                    continue;
                }
                None => format!("frap edge {e:?} duplicates an edge already matched"),
            },
            None => format!("frap edge {e:?} has no blender counterpart"),
        };
        bad_edges += 1;
        if bad_edges <= MAX_LISTED {
            out.push(Mismatch::new(NAME, format!("{what}.edges[{i}]"), msg));
        }
    }
    let mut left: Vec<usize> = bedge.into_values().flatten().collect();
    left.sort_unstable();
    for j in left {
        bad_edges += 1;
        if bad_edges <= MAX_LISTED {
            out.push(Mismatch::new(
                NAME,
                format!("{what}.edges"),
                format!(
                    "blender edge {j} {:?} has no frap counterpart",
                    blender.edges[j]
                ),
            ));
        }
    }

    // Faces: rotation-normalised cycles of blender vertex indices.
    let mut bfaces: BTreeMap<Vec<u32>, Vec<(usize, usize)>> = BTreeMap::new();
    for (g, f) in blender.faces.iter().enumerate() {
        let r = canonical_rotation(f);
        bfaces.entry(rotated(f, r)).or_default().push((g, r));
    }
    let mut fmap = vec![usize::MAX; frap.faces.len()];
    let (fo, bo) = (corner_offsets(frap), corner_offsets(blender));
    let mut cmap = vec![usize::MAX; fo.last().copied().unwrap_or(0)];
    let mut bad_faces = 0;
    for (fi, f) in frap.faces.iter().enumerate() {
        let mapped: Vec<u32> = f
            .iter()
            .map(|&v| vmap.get(v as usize).map_or(u32::MAX, |j| *j as u32))
            .collect();
        let r = canonical_rotation(&mapped);
        let key = rotated(&mapped, r);
        match bfaces.get_mut(&key).and_then(|v| v.pop()) {
            Some((g, rg)) => {
                fmap[fi] = g;
                let n = f.len();
                for k in 0..n {
                    cmap[fo[fi] + (r + k) % n] = bo[g] + (rg + k) % n;
                }
            }
            None => {
                bad_faces += 1;
                if bad_faces <= MAX_LISTED {
                    out.push(Mismatch::new(
                        NAME,
                        format!("{what}.faces[{fi}]"),
                        format!("frap face {f:?} (blender vertices {key:?}) has no counterpart"),
                    ));
                }
            }
        }
    }
    if bad_edges > MAX_LISTED || bad_faces > MAX_LISTED {
        out.push(Mismatch::new(
            NAME,
            what,
            format!("{bad_edges} unmatched edges, {bad_faces} unmatched faces in total"),
        ));
    }
    if bad_edges > 0 || bad_faces > 0 {
        return out;
    }

    // Corner data through the corner matching.
    let corners = cmap.len();
    let reorder_floats = |v: &[[f32; 3]]| -> Option<Vec<[f32; 3]>> {
        if v.len() != corners {
            return None;
        }
        let mut o = vec![[0.0; 3]; corners];
        for (i, &j) in cmap.iter().enumerate() {
            o[j] = v[i];
        }
        Some(o)
    };
    match reorder_floats(&frap.corner_normals) {
        Some(cn) => floats(
            NAME,
            &format!("{what}.corner_normals"),
            &cn,
            &blender.corner_normals,
            tol,
            &mut out,
        ),
        None => out.push(Mismatch::new(
            NAME,
            format!("{what}.corner_normals"),
            format!(
                "frap has {} corner normals for {corners} corners",
                frap.corner_normals.len()
            ),
        )),
    }
    let fk: Vec<&String> = frap.uv_layers.keys().collect();
    let bk: Vec<&String> = blender.uv_layers.keys().collect();
    if fk != bk {
        out.push(Mismatch::new(
            NAME,
            format!("{what}.uv_layers"),
            format!("frap {fk:?} != blender {bk:?}"),
        ));
    }
    for (name, uv) in &frap.uv_layers {
        let Some(b) = blender.uv_layers.get(name) else {
            continue;
        };
        if uv.len() != corners {
            out.push(Mismatch::new(
                NAME,
                format!("{what}.uv_layers[{name:?}]"),
                format!("frap has {} UVs for {corners} corners", uv.len()),
            ));
            continue;
        }
        let mut o = vec![[0.0f32; 2]; corners];
        for (i, &j) in cmap.iter().enumerate() {
            o[j] = uv[i];
        }
        floats(NAME, &format!("{what}.uv_layers[{name:?}]"), &o, b, tol, &mut out);
    }

    // Attributes through the element matchings.
    for (name, fa) in &frap.attributes {
        let path = format!("{what}.attributes[{name:?}]");
        let Some(ba) = blender.attributes.get(name) else {
            out.push(Mismatch::new(NAME, path, "only in frap"));
            continue;
        };
        let (map, n): (&[usize], usize) = match fa.get("domain").and_then(|d| d.as_str()) {
            Some("POINT") => (&vmap, blender.positions.len()),
            Some("EDGE") => (&emap, blender.edges.len()),
            Some("FACE") => (&fmap, blender.faces.len()),
            Some("CORNER") => (&cmap, corners),
            _ => {
                json_close(NAME, &path, fa, ba, tol as f64, &mut out);
                continue;
            }
        };
        let mut fa2 = fa.clone();
        match fa.get("data").and_then(|d| reorder(d, map, n)) {
            Some(d) => fa2["data"] = d,
            None => {
                out.push(Mismatch::new(
                    NAME,
                    &path,
                    "frap data length does not match the element count",
                ));
                continue;
            }
        }
        json_close(NAME, &path, &fa2, ba, tol as f64, &mut out);
    }
    for name in blender
        .attributes
        .keys()
        .filter(|n| !frap.attributes.contains_key(*n))
    {
        out.push(Mismatch::new(
            NAME,
            format!("{what}.attributes[{name:?}]"),
            "only in blender",
        ));
    }

    // Edit-mode state through the matchings.
    let maps: [(&str, &[usize]); 3] = [("vert", &vmap), ("edge", &emap), ("face", &fmap)];
    json_exact(
        NAME,
        &format!("{what}.selection"),
        &remap_selection(&frap.selection, &maps),
        &remap_selection(&blender.selection, &[]),
        &mut out,
    );
    json_exact(
        NAME,
        &format!("{what}.active"),
        &remap_element(&frap.active, &maps),
        &blender.active,
        &mut out,
    );
    let hist = match &frap.select_history {
        Json::Array(a) => Json::Array(a.iter().map(|e| remap_element(e, &maps)).collect()),
        other => other.clone(),
    };
    json_exact(
        NAME,
        &format!("{what}.select_history"),
        &hist,
        &blender.select_history,
        &mut out,
    );
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::compare::mesh_exact::tests::cube;
    use serde_json::json;

    /// Newell normal of a polygon.
    pub(crate) fn newell(p: &[[f32; 3]], f: &[u32]) -> [f32; 3] {
        let mut n = [0.0f32; 3];
        for i in 0..f.len() {
            let a = p[f[i] as usize];
            let b = p[f[(i + 1) % f.len()] as usize];
            n[0] += (a[1] - b[1]) * (a[2] + b[2]);
            n[1] += (a[2] - b[2]) * (a[0] + b[0]);
            n[2] += (a[0] - b[0]) * (a[1] + b[1]);
        }
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n.map(|x| x / l)
    }

    /// Applies a vertex permutation, a face permutation and per-face rotations, carrying every
    /// per-element array along.
    pub(crate) fn permuted(m: &GMesh) -> GMesh {
        let nv = m.positions.len();
        // new index of old vertex i
        let vperm: Vec<u32> = (0..nv as u32).map(|i| (i * 5 + 3) % nv as u32).collect();
        let mut positions = vec![[0.0; 3]; nv];
        for (i, p) in m.positions.iter().enumerate() {
            positions[vperm[i] as usize] = *p;
        }
        let mut edges: Vec<[u32; 2]> = m
            .edges
            .iter()
            .map(|e| [vperm[e[1] as usize], vperm[e[0] as usize]])
            .collect();
        edges.reverse();
        let offs = corner_offsets(m);
        let mut faces = Vec::new();
        let mut cn = Vec::new();
        let mut uv: BTreeMap<String, Vec<[f32; 2]>> = m
            .uv_layers
            .keys()
            .map(|k| (k.clone(), Vec::new()))
            .collect();
        let mut face_data = Vec::new();
        let sharp = m.attributes.get("sharp_face").map(|a| a["data"].clone());
        for fi in (0..m.faces.len()).rev() {
            let f = &m.faces[fi];
            let r = fi % f.len();
            faces.push(
                (0..f.len())
                    .map(|k| vperm[f[(r + k) % f.len()] as usize])
                    .collect::<Vec<_>>(),
            );
            for k in 0..f.len() {
                let c = offs[fi] + (r + k) % f.len();
                cn.push(m.corner_normals[c]);
                for (name, l) in uv.iter_mut() {
                    l.push(m.uv_layers[name][c]);
                }
            }
            if let Some(s) = &sharp {
                face_data.push(s[fi].clone());
            }
        }
        let mut attributes = m.attributes.clone();
        if sharp.is_some() {
            attributes.get_mut("sharp_face").unwrap()["data"] = Json::Array(face_data);
        }
        GMesh {
            positions,
            edges,
            faces,
            corner_normals: cn,
            uv_layers: uv,
            attributes,
            selection: m.selection.clone(),
            active: m.active.clone(),
            select_history: m.select_history.clone(),
        }
    }

    #[test]
    fn equal_under_permutation() {
        let a = cube();
        let b = permuted(&a);
        assert_ne!(a, b);
        assert_eq!(compare("m", &b, &a, 1e-5), vec![]);
        assert_eq!(compare("m", &a, &b, 1e-5), vec![]);
        // mesh_exact sees the difference.
        assert!(!crate::compare::mesh_exact::compare("m", &a, &b, 1e-5).is_empty());
    }

    #[test]
    fn selection_is_remapped() {
        let mut a = cube();
        a.selection = json!({"vert": [0, 1], "edge": [], "face": [2]});
        a.active = json!({"type": "VERT", "index": 1});
        a.select_history = json!([{"type": "VERT", "index": 0}, {"type": "VERT", "index": 1}]);
        let mut b = permuted(&a);
        let vperm = |i: u64| (i * 5 + 3) % 8;
        b.selection = json!({"vert": [vperm(0), vperm(1)], "edge": [], "face": [3]});
        b.active = json!({"type": "VERT", "index": vperm(1)});
        b.select_history = json!([
            {"type": "VERT", "index": vperm(0)},
            {"type": "VERT", "index": vperm(1)}
        ]);
        assert_eq!(compare("m", &b, &a, 1e-5), vec![]);
        b.active = json!({"type": "VERT", "index": vperm(0)});
        assert_eq!(compare("m", &b, &a, 1e-5).len(), 1);
    }

    #[test]
    fn differs_under_one_change() {
        let a = cube();
        type Change = Box<dyn Fn(&mut GMesh)>;
        let changes: Vec<Change> = vec![
            Box::new(|m| m.positions[5][2] = 0.5),
            Box::new(|m| m.positions[5][2] += 5e-5),
            Box::new(|m| m.faces[0].reverse()),
            // A duplicated edge in place of another (counts unchanged).
            Box::new(|m| m.edges[3] = m.edges[7]),
            Box::new(|m| m.edges[3] = [m.edges[7][1], m.edges[7][0]]),
            Box::new(|m| m.corner_normals[7][1] = 0.3),
            Box::new(|m| m.uv_layers.get_mut("UVMap").unwrap()[3][0] = 0.9),
            Box::new(|m| {
                m.attributes.get_mut("sharp_face").unwrap()["data"][2] = json!(false);
            }),
        ];
        for (i, change) in changes.iter().enumerate() {
            let mut b = permuted(&a);
            change(&mut b);
            let m = compare("m", &b, &a, 1e-5);
            assert!(!m.is_empty(), "change {i} not detected");
        }
    }

    #[test]
    fn coincident_vertices_use_the_neighbourhood() {
        // Two triangles sharing no vertex but with coincident corners at the origin.
        let m = GMesh {
            positions: vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0],
                [0.0, -1.0, 0.0],
            ],
            edges: vec![[0, 1], [1, 2], [0, 2], [3, 4], [4, 5], [3, 5]],
            faces: vec![vec![0, 1, 2], vec![3, 4, 5]],
            corner_normals: vec![[0.0, 0.0, 1.0]; 6],
            uv_layers: BTreeMap::new(),
            attributes: BTreeMap::new(),
            selection: Json::Null,
            active: Json::Null,
            select_history: Json::Null,
        };
        let mut swapped = m.clone();
        // Swap the two coincident vertices' indices.
        swapped.edges = vec![[3, 1], [1, 2], [3, 2], [0, 4], [4, 5], [0, 5]];
        swapped.faces = vec![vec![3, 1, 2], vec![0, 4, 5]];
        assert_eq!(compare("m", &swapped, &m, 1e-5), vec![]);
    }

    #[test]
    fn count_mismatch_is_reported_once() {
        let a = cube();
        let mut b = a.clone();
        b.positions.push([9.0; 3]);
        let m = compare("m", &a, &b, 1e-5);
        assert_eq!(m.len(), 1);
    }
}
