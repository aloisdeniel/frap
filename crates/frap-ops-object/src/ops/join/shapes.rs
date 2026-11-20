// SPDX-License-Identifier: GPL-3.0-or-later
//! The shape keys of `object.join` (WP 6.3). Port of `join_shape_keys`
//! (`editors/mesh/mesh_join.cc`, Blender 5.2.4, GPL-2.0-or-later), verified on 5.2.4:
//!
//! - the active mesh's blocks keep their data for its own vertices;
//! - a block name only a joined mesh has is appended (settings copied, `relative_key` remapped
//!   by name), its data for the active mesh's vertices being the active mesh's positions; the
//!   active mesh gets a (relative) key when it has none;
//! - for every joined part and every block, the part's block of the same name (or the part's
//!   positions when it has none) is transformed into the active object's space.
//!
//! Absolute keys are re-sorted afterwards (`BKE_key_sort`).

use frap_math::glam::{Mat4, Vec3};
use frap_mesh::Mesh;
use frap_scene::id::{Key, ShapeKey};
use smol_str::SmolStr;

/// One joined part as the shape-key merge sees it.
pub(crate) struct KeyPart<'a> {
    /// The part's mesh.
    pub mesh: &'a Mesh,
    /// Its key, if any.
    pub key: Option<&'a Key>,
    /// Into the active object's space; `None` for the active object.
    pub xform: Option<Mat4>,
}

/// The joined key blocks (`None` when no part has a key): the blocks of the active key (or of
/// a new key) for the joined vertex count, and whether the active mesh had a key.
pub(crate) fn join_shape_keys(parts: &[KeyPart]) -> Option<(Vec<ShapeKey>, bool)> {
    let first = parts.first()?;
    if parts.iter().all(|p| p.key.is_none()) {
        return None;
    }
    let total: usize = parts.iter().map(|p| p.mesh.verts_num()).sum();
    let n0 = first.mesh.verts_num();
    let mut blocks: Vec<ShapeKey> = first.key.map(|k| k.key_blocks.clone()).unwrap_or_default();
    for kb in &mut blocks {
        // `MEM_realloc`: the active mesh's part is kept, the rest is filled below.
        kb.data.resize(total, [0.0; 3]);
    }
    let mut names: Vec<SmolStr> = blocks.iter().map(|k| k.name.clone()).collect();
    for p in parts.iter().skip(1) {
        let Some(src) = p.key else {
            continue;
        };
        for src_kb in &src.key_blocks {
            if names.contains(&src_kb.name) {
                continue;
            }
            names.push(src_kb.name.clone());
            // `BKE_keyblock_add` (names are unique here) + `BKE_keyblock_copy_settings`.
            let curpos = blocks.last().map_or(-0.1, |k| k.pos);
            let mut kb = ShapeKey::new(src_kb.name.clone(), vec![[0.0; 3]; total]);
            kb.pos = curpos + 0.1;
            kb.copy_settings_from(src_kb);
            kb.data[..n0].copy_from_slice(&first.mesh.positions()[..n0]);
            if let Some(rel) = src.key_blocks.get(src_kb.relative_key)
                && let Some(i) = names.iter().position(|n| *n == rel.name)
            {
                kb.relative_key = i;
            }
            blocks.push(kb);
        }
    }
    let mut start = n0;
    for p in parts.iter().skip(1) {
        let n = p.mesh.verts_num();
        let m = p.xform.unwrap_or(Mat4::IDENTITY);
        for kb in &mut blocks {
            let src: &[[f32; 3]] = p
                .key
                .and_then(|k| k.find_block(&kb.name).map(|i| &k.key_blocks[i].data[..]))
                .unwrap_or(p.mesh.positions());
            for (j, dst) in kb.data[start..start + n].iter_mut().enumerate() {
                let s = src.get(j).copied().unwrap_or([0.0; 3]);
                *dst = m.transform_point3(Vec3::from(s)).to_array();
            }
        }
        start += n;
    }
    Some((blocks, first.key.is_some()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mesh(points: &[[f32; 3]]) -> Mesh {
        let mut m = Mesh::new("M");
        m.set_positions(points.to_vec());
        m
    }

    #[test]
    fn joined_blocks_take_part_shapes_or_positions() {
        let a = mesh(&[[0.0, 0.0, 0.0]]);
        let b = mesh(&[[1.0, 0.0, 0.0]]);
        let mut kb = Key::new("Key");
        for (n, z) in [("Basis", 0.0), ("Up", 1.0)] {
            let i = kb.add_block(Some(n));
            kb.key_blocks[i].data = vec![[1.0, 0.0, z]];
        }
        let parts = [
            KeyPart {
                mesh: &a,
                key: None,
                xform: None,
            },
            KeyPart {
                mesh: &b,
                key: Some(&kb),
                xform: Some(Mat4::from_translation(Vec3::new(0.0, 2.0, 0.0))),
            },
        ];
        let (blocks, had) = join_shape_keys(&parts).unwrap();
        assert!(!had);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].name, "Up");
        assert_eq!(blocks[1].data, vec![[0.0, 0.0, 0.0], [1.0, 2.0, 1.0]]);
        assert_eq!(blocks[0].data[1], [1.0, 2.0, 0.0]);
    }
}
