// SPDX-License-Identifier: GPL-3.0-or-later
//! Shape keys in Edit Mode (WP 6.3): the `CD_SHAPEKEY` layers of a BMesh
//! ([`crate::types::ShapeLayer`]) built from the mesh's `Key` on entering Edit Mode
//! ([`load_layers`], the shape-key part of `BM_mesh_bm_from_me`), and the write-back on leaving
//! it ([`write_back`], `bm_to_mesh_shape`: the active block takes the edited positions, and
//! when it is the basis of other blocks every one of them is offset by the edit — "edit-mode
//! basis propagation"; the mesh positions take the reference key's coordinates when another
//! block was edited).
//!
//! Ported from blender/source/blender/bmesh/intern/bmesh_mesh_convert.cc (Blender 5.2.4,
//! GPL-2.0-or-later). frap's BMesh has no `CD_SHAPE_KEYINDEX` layer: in Blender a vertex
//! created while editing gets the layer's default (0, not `ORIGINDEX_NONE`) or its example's
//! index, so the "vertex did not exist on entering" branches never trigger in practice and
//! every vertex is treated as existing. Layers are matched to key blocks by position (key
//! blocks cannot be added, removed or reordered by the operators while editing).
//!
//! The key data itself lives in `frap-scene` (`Key`, a layer above): these functions take and
//! return plain coordinate arrays. [`deform`] holds the `MDeformVert` helpers (`BKE_defvert_*`)
//! shared by the vertex-group RNA functions and operators.

pub mod deform;

use smol_str::SmolStr;

use crate::types::{BMesh, ShapeLayer};

/// One key block as the conversion sees it.
#[derive(Clone, Copy, Debug)]
pub struct BlockRef<'a> {
    /// `KeyBlock.name`
    pub name: &'a str,
    /// `KeyBlock.data` (absolute coordinates, `totelem` entries).
    pub data: &'a [[f32; 3]],
    /// `KeyBlock.relative` (index of the block this one is relative to).
    pub relative: usize,
}

/// `BM_mesh_bm_from_me`'s shape-key part on a BMesh freshly converted from a mesh with
/// `verts_num` vertices (slot `i` = mesh vertex `i`): one layer per block holding its
/// coordinates, and with `use_shapekey` the vertex positions taken from the active block
/// (`active`, 0-based) when its element count matches. A block shorter than the mesh fills the
/// rest of its layer with the vertex positions.
pub fn load_layers(bm: &mut BMesh, blocks: &[BlockRef], active: Option<usize>, use_shapekey: bool) {
    let slots = bm.verts.capacity().max(bm.verts.next_slot() as usize);
    let verts: Vec<_> = bm.vert_ids();
    let n = verts.len();
    bm.shape_layers = blocks
        .iter()
        .map(|b| {
            let mut co = vec![[0.0f32; 3]; slots];
            for (i, &v) in verts.iter().enumerate() {
                co[v.slot()] = b.data.get(i).copied().unwrap_or(bm.vert(v).co);
            }
            ShapeLayer {
                name: SmolStr::from(b.name),
                co,
            }
        })
        .collect();
    if use_shapekey
        && let Some(a) = active
        && let Some(b) = blocks.get(a)
        && b.data.len() == n
    {
        for (i, &v) in verts.iter().enumerate() {
            bm.vert_mut(v).co = b.data[i];
        }
    }
}

/// What [`write_back`] computed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WriteBack {
    /// New data of every key block, in vertex iteration order (`BMesh::to_mesh` order).
    pub blocks: Vec<Vec<[f32; 3]>>,
    /// The mesh positions to write instead of the edited coordinates (the reference key's
    /// layer), when a block other than the reference key was being edited.
    pub positions: Option<Vec<[f32; 3]>>,
}

/// `BKE_keyblock_get_dependent_keys`: the blocks that use block `index` as their basis,
/// directly or through other blocks (`None` when there are none or the key is absolute).
pub fn dependent_keys(relative: &[usize], index: usize, use_relative: bool) -> Option<Vec<bool>> {
    if !use_relative {
        return None;
    }
    let count = relative.len();
    if index >= count {
        return None;
    }
    let mut marked = vec![false; count];
    marked[index] = true;
    let mut found = false;
    loop {
        let mut updated = false;
        for (i, &r) in relative.iter().enumerate() {
            if !marked[i] && r < count && marked[r] {
                marked[i] = true;
                updated = true;
                found = true;
            }
        }
        if !updated {
            break;
        }
    }
    if !found {
        return None;
    }
    marked[index] = false;
    Some(marked)
}

/// `bm_to_mesh_shape` with `active_shapekey_to_mvert = false` (leaving Edit Mode): the new
/// coordinates of every block of a key whose blocks are `blocks` (reference key first), with
/// `active` the edited block (`bm->shapenr - 1`, `None` when no block is active). The shape
/// layers are updated in place to the written values (Blender writes them back too, so a
/// second write-back does not apply the basis offset twice).
pub fn write_back(
    bm: &mut BMesh,
    blocks: &[BlockRef],
    active: Option<usize>,
    use_relative: bool,
) -> WriteBack {
    let verts = bm.vert_ids();
    let nl = bm.shape_layers.len();
    let actkey = active.filter(|&a| a < blocks.len());
    let actkey_has_layer = actkey.is_some_and(|a| a < nl);
    let relative: Vec<usize> = blocks.iter().map(|b| b.relative).collect();
    // Editing a basis updates the blocks relative to it.
    let mut ofs: Option<Vec<[f32; 3]>> = None;
    let mut dependent = None;
    if let Some(a) = actkey
        && actkey_has_layer
        && let Some(d) = dependent_keys(&relative, a, use_relative)
    {
        let layer = &bm.shape_layers[a];
        ofs = Some(
            verts
                .iter()
                .map(|&v| {
                    let co = bm.vert(v).co;
                    let orig = layer.co[v.slot()];
                    [co[0] - orig[0], co[1] - orig[1], co[2] - orig[2]]
                })
                .collect(),
        );
        dependent = Some(d);
    }
    // The mesh keeps the basis coordinates when another block was edited (written in the
    // active block's iteration, so only when the active block has a layer).
    let positions = (actkey_has_layer && actkey != Some(0)).then(|| {
        verts
            .iter()
            .map(|&v| bm.shape_layers[0].co[v.slot()])
            .collect::<Vec<_>>()
    });
    let mut out = Vec::with_capacity(blocks.len());
    for (k, b) in blocks.iter().enumerate() {
        if k < nl {
            let apply = ofs.is_some()
                && Some(k) != actkey
                && dependent.as_ref().is_some_and(|d: &Vec<bool>| d[k]);
            let mut data = Vec::with_capacity(verts.len());
            for (i, &v) in verts.iter().enumerate() {
                let s = v.slot();
                let mut co = if Some(k) == actkey {
                    bm.vert(v).co
                } else {
                    bm.shape_layers[k].co[s]
                };
                if apply && let Some(o) = &ofs {
                    co = [co[0] + o[i][0], co[1] + o[i][1], co[2] + o[i][2]];
                }
                bm.shape_layers[k].co[s] = co;
                data.push(co);
            }
            out.push(data);
        } else {
            // No layer (a block added while editing): the block's own data where it reaches,
            // the edited coordinates elsewhere.
            out.push(
                verts
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| b.data.get(i).copied().unwrap_or(bm.vert(v).co))
                    .collect(),
            );
        }
    }
    WriteBack {
        blocks: out,
        positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CreateFlag;

    fn three_verts() -> BMesh {
        let mut bm = BMesh::default();
        for x in 0..3 {
            bm.vert_create([x as f32, 0.0, 0.0], None, CreateFlag::empty());
        }
        bm
    }

    #[test]
    fn dependent_keys_follow_relative_chains() {
        // Basis <- A <- B, C relative to itself.
        let rel = [0, 0, 1, 3];
        assert_eq!(
            dependent_keys(&rel, 0, true),
            Some(vec![false, true, true, false])
        );
        assert_eq!(
            dependent_keys(&rel, 1, true),
            Some(vec![false, false, true, false])
        );
        assert_eq!(dependent_keys(&rel, 2, true), None);
        assert_eq!(dependent_keys(&rel, 0, false), None);
    }

    #[test]
    fn editing_the_basis_offsets_dependent_blocks() {
        let mut bm = three_verts();
        let basis = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let key1 = [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let blocks = [
            BlockRef {
                name: "Basis",
                data: &basis,
                relative: 0,
            },
            BlockRef {
                name: "Key 1",
                data: &key1,
                relative: 0,
            },
        ];
        load_layers(&mut bm, &blocks, Some(0), true);
        let v1 = bm.vert_ids()[1];
        bm.vert_mut(v1).co = [1.0, 0.0, 5.0];
        let wb = write_back(&mut bm, &blocks, Some(0), true);
        assert_eq!(wb.positions, None);
        assert_eq!(wb.blocks[0][1], [1.0, 0.0, 5.0]);
        assert_eq!(wb.blocks[1][1], [1.0, 1.0, 5.0]);
        assert_eq!(wb.blocks[1][0], [0.0, 1.0, 0.0]);
        // A second write-back applies nothing twice.
        let wb2 = write_back(&mut bm, &blocks, Some(0), true);
        assert_eq!(wb2.blocks[1][1], [1.0, 1.0, 5.0]);
    }

    #[test]
    fn editing_another_block_keeps_the_basis_positions() {
        let mut bm = three_verts();
        let basis = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let key1 = [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let blocks = [
            BlockRef {
                name: "Basis",
                data: &basis,
                relative: 0,
            },
            BlockRef {
                name: "Key 1",
                data: &key1,
                relative: 0,
            },
        ];
        load_layers(&mut bm, &blocks, Some(1), true);
        let v0 = bm.vert_ids()[0];
        assert_eq!(bm.vert(v0).co, [0.0, 1.0, 0.0]);
        bm.vert_mut(v0).co = [0.0, 2.0, 0.0];
        let wb = write_back(&mut bm, &blocks, Some(1), true);
        assert_eq!(wb.positions.as_deref(), Some(&basis[..]));
        assert_eq!(wb.blocks[0], basis.to_vec());
        assert_eq!(wb.blocks[1][0], [0.0, 2.0, 0.0]);
    }
}
