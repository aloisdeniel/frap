// SPDX-License-Identifier: GPL-3.0-or-later
//! Primitive topology: every glTF mode (POINTS … TRIANGLE_FAN) decoded to points, edges and
//! triangles.
//!
//! Ported from io_scene_gltf2 `points_edges_tris` (`blender/imp/mesh.py`, Apache-2.0, The
//! glTF-Blender-IO authors), so frap's importer converts strips, fans and loops exactly as
//! Blender's does: strip triangle `i` is `(i, i+1, i+2)` for even `i` and `(i, i+2, i+1)` for
//! odd `i`; fan triangle `i` is `(0, i+1, i+2)`; a line loop closes back to its first vertex.
//! Where the add-on would fail on too few indices (numpy shape errors) this returns nothing,
//! and trailing indices that do not form a whole line or triangle are dropped.

use crate::accessor::read_accessor;
use crate::error::GltfError;
use crate::json::{Document, Mode, Primitive};

/// Decoded topology of one primitive, as vertex indices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Topology {
    /// POINTS.
    pub points: Vec<u32>,
    /// LINES, LINE_LOOP, LINE_STRIP.
    pub edges: Vec<[u32; 2]>,
    /// TRIANGLES, TRIANGLE_STRIP, TRIANGLE_FAN (counter-clockwise front faces kept).
    pub triangles: Vec<[u32; 3]>,
}

/// Converts the vertex sequence `indices` drawn with `mode` into points, edges or triangles.
pub fn decode_topology(mode: Mode, indices: &[u32]) -> Topology {
    let mut t = Topology::default();
    let n = indices.len();
    match mode {
        Mode::Points => t.points = indices.to_vec(),
        Mode::Lines => {
            t.edges = indices.chunks_exact(2).map(|c| [c[0], c[1]]).collect();
        }
        Mode::LineLoop if n >= 2 => {
            t.edges = (0..n).map(|i| [indices[i], indices[(i + 1) % n]]).collect();
        }
        Mode::LineStrip if n >= 2 => {
            t.edges = indices.windows(2).map(|w| [w[0], w[1]]).collect();
        }
        Mode::Triangles => {
            t.triangles = indices
                .chunks_exact(3)
                .map(|c| [c[0], c[1], c[2]])
                .collect();
        }
        Mode::TriangleStrip if n >= 3 => {
            t.triangles = (0..n - 2)
                .map(|i| {
                    let (a, b, c) = (indices[i], indices[i + 1], indices[i + 2]);
                    if i % 2 == 0 { [a, b, c] } else { [a, c, b] }
                })
                .collect();
        }
        Mode::TriangleFan if n >= 3 => {
            t.triangles = (0..n - 2)
                .map(|i| [indices[0], indices[i + 1], indices[i + 2]])
                .collect();
        }
        Mode::LineLoop | Mode::LineStrip | Mode::TriangleStrip | Mode::TriangleFan => {}
    }
    t
}

/// The vertex sequence of a primitive: its `indices` accessor, or `0..count` of its
/// attributes (POSITION, else the first attribute) when it has none.
pub fn primitive_indices<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
    prim: &Primitive,
) -> Result<Vec<u32>, GltfError> {
    if let Some(i) = prim.indices {
        return read_accessor(doc, buffers, i)?.to_u32();
    }
    let attr = prim
        .attributes
        .get("POSITION")
        .or_else(|| prim.attributes.values().next())
        .ok_or_else(|| GltfError::Data("primitive has no attributes".into()))?;
    // Decoding the attribute bounds the vertex count by real data: a bare `count` from the
    // JSON could ask for billions of indices.
    let count = read_accessor(doc, buffers, *attr)?.count();
    let count = u32::try_from(count)
        .map_err(|_| GltfError::Data(format!("{count} vertices is too many")))?;
    Ok((0..count).collect())
}

/// Reads and decodes the topology of `prim`.
pub fn read_topology<B: AsRef<[u8]>>(
    doc: &Document,
    buffers: &[B],
    prim: &Primitive,
) -> Result<Topology, GltfError> {
    Ok(decode_topology(
        prim.mode_or_default(),
        &primitive_indices(doc, buffers, prim)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected outputs are the diagrams in the add-on's `points_edges_tris`.
    #[test]
    fn every_mode_matches_blender() {
        let idx = [0, 1, 2, 3, 4];
        assert_eq!(decode_topology(Mode::Points, &idx).points, idx);
        assert_eq!(
            decode_topology(Mode::Lines, &[0, 1, 2, 3]).edges,
            [[0, 1], [2, 3]]
        );
        // in: 0123, out: 01122330
        assert_eq!(
            decode_topology(Mode::LineLoop, &[0, 1, 2, 3]).edges,
            [[0, 1], [1, 2], [2, 3], [3, 0]]
        );
        // in: 0123, out: 011223
        assert_eq!(
            decode_topology(Mode::LineStrip, &[0, 1, 2, 3]).edges,
            [[0, 1], [1, 2], [2, 3]]
        );
        assert_eq!(
            decode_topology(Mode::Triangles, &[0, 1, 2, 3, 4, 5]).triangles,
            [[0, 1, 2], [3, 4, 5]]
        );
        // in: 01234, out: 012 132 234
        assert_eq!(
            decode_topology(Mode::TriangleStrip, &idx).triangles,
            [[0, 1, 2], [1, 3, 2], [2, 3, 4]]
        );
        // in: 01234, out: 012 023 034
        assert_eq!(
            decode_topology(Mode::TriangleFan, &idx).triangles,
            [[0, 1, 2], [0, 2, 3], [0, 3, 4]]
        );
    }

    #[test]
    fn degenerate_inputs() {
        for m in Mode::ALL {
            assert_eq!(decode_topology(m, &[]), Topology::default(), "{m:?}");
        }
        assert!(
            decode_topology(Mode::TriangleStrip, &[0, 1])
                .triangles
                .is_empty()
        );
        assert!(
            decode_topology(Mode::TriangleFan, &[0, 1])
                .triangles
                .is_empty()
        );
        assert!(decode_topology(Mode::LineLoop, &[0]).edges.is_empty());
        assert!(decode_topology(Mode::LineStrip, &[0]).edges.is_empty());
        assert_eq!(decode_topology(Mode::Lines, &[0, 1, 2]).edges, [[0, 1]]);
        assert_eq!(
            decode_topology(Mode::Triangles, &[0, 1, 2, 3]).triangles,
            [[0, 1, 2]]
        );
    }

    #[test]
    fn strip_and_fan_keep_winding() {
        // A quad as a strip and as a fan: both triangles counter-clockwise in XY.
        let p = [[0.0f32, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let area = |t: [u32; 3]| {
            let (a, b, c) = (p[t[0] as usize], p[t[1] as usize], p[t[2] as usize]);
            (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
        };
        for t in decode_topology(Mode::TriangleStrip, &[0, 1, 2, 3]).triangles {
            assert!(area(t) > 0.0, "{t:?}");
        }
        for t in decode_topology(Mode::TriangleFan, &[0, 1, 3, 2]).triangles {
            assert!(area(t) > 0.0, "{t:?}");
        }
    }
}
