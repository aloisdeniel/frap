// SPDX-License-Identifier: GPL-3.0-or-later
//! `ShapeKey.normals_vertex_get()` (`rna_KeyBlock_normals_vert_calc` →
//! `BKE_keyblock_mesh_calc_normals`, WP 6.3): the vertex normals of the key's mesh with the
//! block's coordinates, flattened (`3 × verts` floats; Blender returns a 1D tuple). Also the
//! helper of `normals_polygon_get` and `normals_split_get`.

use frap_rna::{ArrayShape, MathTag, RnaError, RnaPtr, Value, rna_func};
use frap_scene::id::Key;
use frap_scene::{FuncArgs, Host};

use super::_common::internal;

/// Which normals `BKE_keyblock_mesh_calc_normals` computes.
pub(crate) enum Normals {
    Vertex,
    Polygon,
    Split,
}

/// The normals of the block `this` names, on its key's mesh (`Key.user`).
pub(crate) fn block_normals(h: &dyn Host, this: &RnaPtr, which: Normals) -> Result<Value, RnaError> {
    let main = h.main();
    let (k, i) = Key::block_of(main, this).ok_or_else(|| internal("not a shape key pointer"))?;
    let key = main
        .shape_keys
        .get(k)
        .ok_or_else(|| internal("the key was removed"))?;
    let data = &key.key_blocks[i].data;
    // The mesh using the key (`Key.user`; a copied key may still name the original mesh).
    let mesh = main
        .meshes
        .iter()
        .find(|(_, m)| m.shape_keys == Some(k))
        .map(|(_, m)| m)
        .or_else(|| key.user.and_then(|me| main.meshes.get(me)));
    let flat: Vec<f32> = match mesh {
        None => Vec::new(),
        Some(m) => {
            let mut m = m.clone();
            {
                let pos = m.positions_mut();
                let n = data.len().min(pos.len());
                pos[..n].copy_from_slice(&data[..n]);
            }
            let out = match which {
                Normals::Vertex => m.vert_normals(),
                Normals::Polygon => m.face_normals(),
                Normals::Split => m.corner_normals(),
            };
            out.iter().flat_map(|c| c.iter().copied()).collect()
        }
    };
    Ok(Value::FloatArray {
        data: flat.into_iter().collect(),
        shape: ArrayShape::Dynamic,
        tag: MathTag::None,
    })
}

#[rna_func(srna = "ShapeKey", func = "normals_vertex_get")]
fn shape_key_normals_vertex_get(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    block_normals(h, this, Normals::Vertex)
}
