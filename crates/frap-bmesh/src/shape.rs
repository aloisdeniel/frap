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
//! GPL-2.0-or-later). As in Blender:
//!
//! - every layer carries the `uid` of its key block (`CustomDataLayer.uid` = `KeyBlock.uid`)
//!   and the write-back matches layers to blocks by it, so blocks added or removed through
//!   RNA while editing (`Object.shape_key_add/remove` work in Edit Mode) never take another
//!   block's layer; a layer whose block is gone is re-added as a block by the caller
//!   ([`orphan_layers`]);
//! - `CD_SHAPE_KEYINDEX` ([`BMesh::shape_keyindex`]) records the mesh index of every vertex
//!   that existed on entering. `BM_vert_create` with an example vertex (edge split, so
//!   subdivide, bevel, …) sets it to `ORIGINDEX_NONE`; duplication and extrusion copy it. One
//!   such new vertex disables the basis propagation altogether, and keeps its edited
//!   coordinates in the mesh when a block other than the reference key was edited;
//! - the active block is the one the BMesh was made with (`bm->shapenr`, [`BMesh::shapenr`]),
//!   not the object's current one.
//!
//! The key data itself lives in `frap-scene` (`Key`, a layer above): these functions take and
//! return plain coordinate arrays. [`deform`] holds the `MDeformVert` helpers (`BKE_defvert_*`)
//! shared by the vertex-group RNA functions and operators.

pub mod deform;

use smol_str::SmolStr;

use crate::types::{BMesh, ORIGINDEX_NONE, ShapeLayer};

/// One key block as the conversion sees it.
#[derive(Clone, Copy, Debug)]
pub struct BlockRef<'a> {
    /// `KeyBlock.name`
    pub name: &'a str,
    /// `KeyBlock.data` (absolute coordinates, `totelem` entries).
    pub data: &'a [[f32; 3]],
    /// `KeyBlock.relative` (index of the block this one is relative to).
    pub relative: usize,
    /// `KeyBlock.uid` (unique in its key, from 1).
    pub uid: i32,
}

/// `BM_mesh_bm_from_me`'s shape-key part on a BMesh freshly converted from a mesh with
/// `verts_num` vertices (slot `i` = mesh vertex `i`): one layer per block holding its
/// coordinates and uid, the original key index of every vertex, and `bm->shapenr` = the
/// active block (`active`, 0-based) when its element count matches. With `use_shapekey` the
/// vertex positions are taken from that block. A block shorter than the mesh fills the rest of
/// its layer with the vertex positions.
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
                uid: b.uid,
            }
        })
        .collect();
    bm.shape_keyindex = (!blocks.is_empty()).then(|| {
        let mut ki = vec![0; slots];
        for (i, &v) in verts.iter().enumerate() {
            ki[v.slot()] = i as i32;
        }
        ki
    });
    if let Some(a) = active
        && let Some(b) = blocks.get(a)
        && b.data.len() == n
    {
        bm.shapenr = a as i32 + 1;
        if use_shapekey {
            for (i, &v) in verts.iter().enumerate() {
                bm.vert_mut(v).co = b.data[i];
            }
        }
    }
}

/// The first loop of `bm_to_mesh_shape`: the shape layers no block of `uids` (the key's
/// blocks) matches, as `(name, uid)` in layer order. The caller appends one block per entry
/// (`BKE_keyblock_add(key, layer.name)` with `uid` = the layer's) before [`write_back`].
pub fn orphan_layers(bm: &BMesh, uids: &[i32]) -> Vec<(SmolStr, i32)> {
    bm.shape_layers
        .iter()
        .filter(|l| !uids.contains(&l.uid))
        .map(|l| (l.name.clone(), l.uid))
        .collect()
}

/// What [`write_back`] computed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WriteBack {
    /// New data of every key block, in vertex iteration order (`BMesh::to_mesh` order).
    pub blocks: Vec<Vec<[f32; 3]>>,
    /// The mesh positions to write instead of the edited coordinates (the reference key's
    /// layer, a new vertex keeping its edited position), when a block other than the
    /// reference key was being edited.
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
/// coordinates of every block of a key whose blocks are `blocks` (reference key first, every
/// orphan layer already re-added, [`orphan_layers`]), with `active` the edited block
/// (`bm->shapenr - 1` looked up before the re-adding, `None` when no block is active). The
/// shape layers are updated in place to the written values (Blender writes them back too, so
/// a second write-back does not apply the basis offset twice).
pub fn write_back(
    bm: &mut BMesh,
    blocks: &[BlockRef],
    active: Option<usize>,
    use_relative: bool,
) -> WriteBack {
    let verts = bm.vert_ids();
    let layer_of = |bm: &BMesh, b: &BlockRef| bm.shape_layers.iter().position(|l| l.uid == b.uid);
    let actkey = active.filter(|&a| a < blocks.len());
    let act_layer = actkey.and_then(|a| layer_of(bm, &blocks[a]));
    let keyindex: Option<Vec<i32>> = bm
        .shape_keyindex
        .as_ref()
        .map(|ki| verts.iter().map(|v| ki[v.slot()]).collect());
    let relative: Vec<usize> = blocks.iter().map(|b| b.relative).collect();
    // Editing a basis updates the blocks relative to it, unless a vertex was created while
    // editing (the offset only exists for the vertices that existed on entering).
    let mut ofs: Option<Vec<[f32; 3]>> = None;
    let mut dependent = None;
    if let (Some(a), Some(al), Some(ki)) = (actkey, act_layer, &keyindex)
        && let Some(d) = dependent_keys(&relative, a, use_relative)
        && !ki.contains(&ORIGINDEX_NONE)
    {
        let layer = &bm.shape_layers[al];
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
    // The mesh keeps the basis coordinates when another block was edited.
    let refkey_layer = match (actkey, &keyindex) {
        (Some(a), Some(_)) if a != 0 => blocks.first().and_then(|b| layer_of(bm, b)),
        _ => None,
    };
    let mut positions = None;
    let mut out = Vec::with_capacity(blocks.len());
    for (k, b) in blocks.iter().enumerate() {
        if let Some(li) = layer_of(bm, b) {
            let is_act = Some(k) == actkey;
            let apply = ofs.is_some() && !is_act && dependent.as_ref().is_some_and(|d| d[k]);
            if is_act && let Some(rl) = refkey_layer {
                positions = Some(
                    verts
                        .iter()
                        .enumerate()
                        .map(|(i, &v)| match &keyindex {
                            Some(ki) if ki[i] != ORIGINDEX_NONE => bm.shape_layers[rl].co[v.slot()],
                            _ => bm.vert(v).co,
                        })
                        .collect::<Vec<_>>(),
                );
            }
            let mut data = Vec::with_capacity(verts.len());
            for (i, &v) in verts.iter().enumerate() {
                let s = v.slot();
                let mut co = if is_act {
                    bm.vert(v).co
                } else {
                    bm.shape_layers[li].co[s]
                };
                if apply && let Some(o) = &ofs {
                    co = [co[0] + o[i][0], co[1] + o[i][1], co[2] + o[i][2]];
                }
                bm.shape_layers[li].co[s] = co;
                data.push(co);
            }
            out.push(data);
        } else {
            // No layer (a block added while editing): reconstructed through the original key
            // indices where possible, the edited coordinates elsewhere.
            out.push(
                verts
                    .iter()
                    .enumerate()
                    .map(|(i, &v)| {
                        keyindex
                            .as_ref()
                            .map(|ki| ki[i])
                            .filter(|&k| k != ORIGINDEX_NONE)
                            .and_then(|k| b.data.get(k as usize).copied())
                            .unwrap_or(bm.vert(v).co)
                    })
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
                uid: 1,
            },
            BlockRef {
                name: "Key 1",
                data: &key1,
                relative: 0,
                uid: 2,
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
                uid: 1,
            },
            BlockRef {
                name: "Key 1",
                data: &key1,
                relative: 0,
                uid: 2,
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

    fn two_blocks<'a>(basis: &'a [[f32; 3]], key1: &'a [[f32; 3]]) -> [BlockRef<'a>; 2] {
        [
            BlockRef {
                name: "Basis",
                data: basis,
                relative: 0,
                uid: 1,
            },
            BlockRef {
                name: "Key 1",
                data: key1,
                relative: 0,
                uid: 2,
            },
        ]
    }

    #[test]
    fn a_vertex_made_from_an_example_stops_the_propagation() {
        let mut bm = three_verts();
        let basis = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let key1 = [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let blocks = two_blocks(&basis, &key1);
        load_layers(&mut bm, &blocks, Some(0), true);
        assert_eq!(bm.shapenr, 1);
        assert_eq!(bm.shape_keyindex.as_deref(), Some(&[0, 1, 2][..]));
        let v0 = bm.vert_ids()[0];
        let nv = bm.vert_create([5.0, 0.0, 0.0], Some(v0), CreateFlag::empty());
        assert_eq!(
            bm.shape_keyindex.as_ref().unwrap()[nv.slot()],
            ORIGINDEX_NONE
        );
        // Without an example the index is the layer default (0, "existed").
        let dv = bm.vert_create([6.0, 0.0, 0.0], None, CreateFlag::empty());
        assert_eq!(bm.shape_keyindex.as_ref().unwrap()[dv.slot()], 0);
        bm.vert_mut(v0).co = [0.0, 0.0, 1.0];
        let wb = write_back(&mut bm, &blocks, Some(0), true);
        assert_eq!(wb.blocks[0][0], [0.0, 0.0, 1.0]);
        // No offset reaches Key 1.
        assert_eq!(wb.blocks[1][0], [0.0, 1.0, 0.0]);
    }

    #[test]
    fn editing_another_block_keeps_new_vertices_where_edited() {
        let mut bm = three_verts();
        let basis = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let key1 = [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let blocks = two_blocks(&basis, &key1);
        load_layers(&mut bm, &blocks, Some(1), true);
        let v0 = bm.vert_ids()[0];
        bm.vert_create([7.0, 7.0, 7.0], Some(v0), CreateFlag::empty());
        let wb = write_back(&mut bm, &blocks, Some(1), true);
        let pos = wb.positions.expect("refkey positions");
        assert_eq!(pos[0], basis[0]);
        assert_eq!(pos[3], [7.0, 7.0, 7.0]);
        assert_eq!(wb.blocks[1][3], [7.0, 7.0, 7.0]);
    }

    #[test]
    fn layers_are_matched_by_uid() {
        let mut bm = three_verts();
        let basis = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]];
        let key1 = [[0.0, 1.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.0, 0.0]];
        let blocks = two_blocks(&basis, &key1);
        load_layers(&mut bm, &blocks, Some(0), true);
        // A block without a layer (added while editing) and Key 1 gone.
        let added = [[9.0, 9.0, 9.0], [8.0, 8.0, 8.0], [7.0, 7.0, 7.0]];
        let now = [
            blocks[0],
            BlockRef {
                name: "E1",
                data: &added,
                relative: 0,
                uid: 3,
            },
        ];
        assert_eq!(
            orphan_layers(&bm, &[1, 3]),
            vec![(SmolStr::from("Key 1"), 2)]
        );
        let wb = write_back(&mut bm, &now, Some(0), true);
        assert_eq!(wb.blocks[0], basis.to_vec());
        assert_eq!(wb.blocks[1], added.to_vec());
    }
}
