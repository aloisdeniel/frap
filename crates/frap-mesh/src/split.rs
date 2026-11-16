// SPDX-License-Identifier: GPL-3.0-or-later
//! `corner_split()`: splits vertices by (vertex, corner normal rounded to 4 dp then normalised,
//! every UV map, every corner colour attribute) as the glTF exporter and the renderer need
//! [GLTF §7.3]; the default cube gives 24 vertices (ARCHITECTURE §6.1). Tangents join the key
//! when MikkTSpace lands (M3).

use std::collections::HashMap;

use crate::mesh::{AttrData, Domain, Mesh};

/// Result of [`corner_split`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CornerSplit {
    /// Split vertex of every corner.
    pub corner_vert: Vec<u32>,
    /// For every split vertex, the first corner that produced it (corner order).
    pub vert_corner: Vec<u32>,
    /// For every split vertex, its source mesh vertex.
    pub vert_source: Vec<u32>,
}

impl CornerSplit {
    /// Number of split vertices.
    pub fn len(&self) -> usize {
        self.vert_corner.len()
    }
    /// True for a mesh without corners.
    pub fn is_empty(&self) -> bool {
        self.vert_corner.is_empty()
    }
}

/// `np.round(x, 4)` on float32 (ties to even), as the glTF exporter rounds normals.
pub fn round4(x: f32) -> f32 {
    (x * 10000.0f32).round_ties_even() / 10000.0f32
}

/// The exporter's normal key: rounded to 4 decimals, normalised, zero → `(0, 0, 1)`, `-0` → `0`.
pub fn normal_key(n: [f32; 3]) -> [f32; 3] {
    let r = [round4(n[0]), round4(n[1]), round4(n[2])];
    let len = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let mut k = if len != 0.0 {
        [r[0] / len, r[1] / len, r[2] / len]
    } else {
        r
    };
    if k == [0.0; 3] {
        k = [0.0, 0.0, 1.0];
    }
    k.map(|c| if c == 0.0 { 0.0 } else { c })
}

/// Splits the mesh's vertices per corner. Split vertices are numbered in order of their first
/// corner (deterministic; the glTF exporter applies its own byte-order sort on top).
pub fn corner_split(mesh: &Mesh) -> CornerSplit {
    let cv = mesh.corner_verts();
    let normals = mesh.corner_normals();
    let corner_cols: Vec<&AttrData> = mesh
        .attributes()
        .iter()
        .filter(|a| a.domain == Domain::Corner && !a.name.starts_with('.'))
        .filter(|a| {
            matches!(
                a.data,
                AttrData::Float2(_) | AttrData::FloatColor(_) | AttrData::ByteColor(_)
            )
        })
        .map(|a| &a.data)
        .collect();
    let mut map: HashMap<Vec<u32>, u32> = HashMap::with_capacity(cv.len());
    let mut out = CornerSplit {
        corner_vert: Vec::with_capacity(cv.len()),
        ..Default::default()
    };
    let canon = |x: f32| {
        if x == 0.0 {
            0.0f32.to_bits()
        } else {
            x.to_bits()
        }
    };
    for (c, &v) in cv.iter().enumerate() {
        let mut key = Vec::with_capacity(4 + 4 * corner_cols.len());
        key.push(v as u32);
        let n = normals.get(c).copied().unwrap_or([0.0, 0.0, 1.0]);
        key.extend(normal_key(n).map(|x| x.to_bits()));
        for col in &corner_cols {
            match col {
                AttrData::Float2(d) => key.extend(d[c].map(canon)),
                AttrData::FloatColor(d) => key.extend(d[c].map(canon)),
                AttrData::ByteColor(d) => key.push(u32::from_le_bytes(d[c])),
                _ => {}
            }
        }
        let next = out.vert_corner.len() as u32;
        let id = *map.entry(key).or_insert(next);
        if id == next {
            out.vert_corner.push(c as u32);
            out.vert_source.push(v as u32);
        }
        out.corner_vert.push(id);
    }
    out
}
