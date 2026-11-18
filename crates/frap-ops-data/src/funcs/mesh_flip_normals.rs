// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.flip_normals()` (`rna_Mesh_flip_normals` → `bke::mesh_flip_faces(mesh, all faces)`,
//! `blenkernel/intern/mesh_flip_faces.cc`), Blender 5.2.4, GPL-2.0-or-later: the winding of
//! every face is reversed keeping its first corner (corners `0 1 2 3` become `0 3 2 1`); the
//! corner→edge map follows (`corner_edges[a - 1] ↔ corner_edges[b]`), and every other corner
//! attribute (UV maps, colours, `custom_normal`, …; strings excepted) has its values reversed
//! after the first corner, so each value stays with its vertex. Custom normals are not
//! re-encoded (Blender: "does not handle custom normals"). Refused in Edit Mode
//! (`RNA_EDITMODE_WRITE`, D-06).

use std::sync::Arc;

use frap_mesh::{AttrData, Domain, Mesh};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

/// Reverses `v[s+1..e]` for every face range.
fn flip_corner_data<T>(v: &mut [T], ranges: &[std::ops::Range<usize>]) {
    for r in ranges {
        if r.len() > 1 {
            v[r.start + 1..r.end].reverse();
        }
    }
}

// Ported from blender/source/blender/blenkernel/intern/mesh_flip_faces.cc (mesh_flip_faces).
/// `bke::mesh_flip_faces` on every face.
pub(crate) fn flip_faces(mesh: &mut Mesh) {
    let faces: Vec<std::ops::Range<usize>> = (0..mesh.faces_num()).map(|f| mesh.face(f)).collect();
    if faces.is_empty() {
        return;
    }
    let mut cv = mesh.corner_verts().to_vec();
    let mut ce = mesh.corner_edges().to_vec();
    for r in &faces {
        let size = r.len();
        for j in 0..size / 2 {
            let a = r.start + j + 1;
            let b = r.end - 1 - j;
            cv.swap(a, b);
            ce.swap(a - 1, b);
        }
    }
    let attrs = mesh.attributes_mut();
    for a in attrs.iter_mut() {
        if a.domain != Domain::Corner {
            continue;
        }
        match a.name.as_str() {
            ".corner_vert" => a.data = AttrData::Int(Arc::new(cv.clone())),
            ".corner_edge" => a.data = AttrData::Int(Arc::new(ce.clone())),
            _ => match &mut a.data {
                AttrData::Float(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Int(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Int8(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Bool(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Float2(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Float3(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Float4(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::FloatColor(v) => {
                    flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces)
                }
                AttrData::ByteColor(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Int16x2(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Int32x2(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                AttrData::Quaternion(v) => {
                    flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces)
                }
                AttrData::Float4x4(v) => flip_corner_data(Arc::make_mut(v).as_mut_slice(), &faces),
                _ => {}
            },
        }
    }
}

#[rna_func(srna = "Mesh", func = "flip_normals")]
fn mesh_flip_normals(
    h: &mut dyn Host,
    this: &RnaPtr,
    _args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "flip_normals")?;
    let me = mesh_of(this)?;
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("flip_normals(): the mesh was removed"))?;
    flip_faces(mesh);
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_top_face_keeps_first_corner() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let uv_before: Vec<[f32; 2]> = m.uv_maps()[0].1.to_vec();
        flip_faces(&mut m);
        let r = m.face(5);
        assert_eq!(&m.corner_verts()[r.clone()], &[7, 5, 1, 3]);
        // Each corner's edge runs to the next corner's vertex.
        for f in 0..m.faces_num() {
            let r = m.face(f);
            for c in r.clone() {
                let next = if c + 1 == r.end { r.start } else { c + 1 };
                let [a, b] = m.edges()[m.corner_edges()[c] as usize];
                let (v0, v1) = (m.corner_verts()[c], m.corner_verts()[next]);
                assert!((a, b) == (v0, v1) || (a, b) == (v1, v0));
            }
        }
        let uv_after = m.uv_maps()[0].1.to_vec();
        assert_eq!(uv_after[20], uv_before[20]);
        assert_eq!(uv_after[21], uv_before[23]);
        flip_faces(&mut m);
        assert_eq!(m.uv_maps()[0].1.to_vec(), uv_before);
    }
}
