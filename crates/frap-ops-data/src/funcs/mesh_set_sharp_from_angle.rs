// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.set_sharp_from_angle(angle=3.14159)` (`rna_Mesh_sharp_from_angle_set`,
//! `makesrna/intern/rna_mesh_api.cc`): removes `sharp_edge` and `sharp_face`, then
//! `bke::mesh_sharp_edges_set_from_angle(mesh, angle)` (`blenkernel/intern/mesh.cc`) with
//! `mesh::edges_sharp_from_angle_set` (`blenkernel/intern/mesh_normals.cc`), Blender 5.2.4,
//! GPL-2.0-or-later:
//! - `angle` ≥ π: every face smooth, no sharp edge (both attributes stay removed);
//! - `angle` = 0: every face flat (`sharp_face` re-created, all true);
//! - otherwise `sharp_edge` is re-created at the end of the attribute list and set on every
//!   manifold edge whose two faces meet at more than `angle` (consistently wound; an edge with
//!   more than two faces is not sharp).
//!
//! `angle` is clamped to `0 ..= π`. The active attribute keeps its index (Blender stores an
//! index), so it moves when the removed attributes came before it.

use frap_mesh::{Domain, Mesh, SHARP_EDGE, SHARP_FACE};
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};
use smol_str::SmolStr;

use super::_common::{internal, mesh_of};

/// Runs `f` on `mesh` keeping the active attribute's *index* (`Mesh.attributes_active_index`):
/// the attribute at that index afterwards becomes the active one (none past the end).
pub(crate) fn keep_active_index(mesh: &mut Mesh, f: impl FnOnce(&mut Mesh)) {
    let index = mesh
        .attributes_active
        .as_deref()
        .and_then(|n| mesh.attributes().iter().position(|a| a.name == n));
    f(mesh);
    if let Some(i) = index {
        let name = mesh
            .attributes()
            .iter()
            .nth(i)
            .map(|a| SmolStr::new(&a.name));
        mesh.attributes_active = name;
    }
}

/// `INDEX_UNSET` / `INDEX_INVALID` of `mesh_edges_sharp_tag`.
const INDEX_UNSET: i32 = i32::MIN;
const INDEX_INVALID: i32 = -1;

// Ported from blender/source/blender/blenkernel/intern/mesh_normals.cc (mesh_edges_sharp_tag
// with a split angle and no sharp faces, as edges_sharp_from_angle_set calls it).
/// Tags the manifold edges whose faces meet at more than `split_angle`.
fn edges_sharp_from_angle(mesh: &Mesh, split_angle: f32, sharp_edges: &mut [bool]) {
    if split_angle >= std::f32::consts::PI {
        return;
    }
    let split_angle_cos = libm::cosf(split_angle);
    let face_normals = mesh.face_normals();
    let corner_verts = mesh.corner_verts();
    let corner_edges = mesh.corner_edges();
    let mut corner_face = vec![0usize; corner_verts.len()];
    for f in 0..mesh.faces_num() {
        corner_face[mesh.face(f)].fill(f);
    }
    let mut e2l = vec![[0i32; 2]; sharp_edges.len()];
    for face_i in 0..mesh.faces_num() {
        for loop_index in mesh.face(face_i) {
            let vert_i = corner_verts[loop_index];
            let Some(e) = usize::try_from(corner_edges[loop_index])
                .ok()
                .filter(|e| *e < e2l.len())
            else {
                continue;
            };
            let l = &mut e2l[e];
            if (l[0] | l[1]) == 0 {
                l[0] = loop_index as i32;
                l[1] = INDEX_UNSET;
            } else if l[1] == INDEX_UNSET {
                let n0 = face_normals[corner_face[l[0] as usize]];
                let n1 = face_normals[face_i];
                let dot = n0[0] * n1[0] + n0[1] * n1[1] + n0[2] * n1[2];
                let is_angle_sharp = dot < split_angle_cos;
                if sharp_edges[e] || vert_i == corner_verts[l[0] as usize] || is_angle_sharp {
                    l[1] = INDEX_INVALID;
                    if is_angle_sharp {
                        sharp_edges[e] = true;
                    }
                } else {
                    l[1] = loop_index as i32;
                }
            } else if l[1] != INDEX_INVALID {
                l[1] = INDEX_INVALID;
                sharp_edges[e] = false;
            }
        }
    }
}

/// `rna_Mesh_sharp_from_angle_set`.
pub(crate) fn set_sharp_from_angle(mesh: &mut Mesh, angle: f32) {
    keep_active_index(mesh, |mesh| {
        let attrs = mesh.attributes_mut();
        attrs.remove(SHARP_EDGE);
        attrs.remove(SHARP_FACE);
        if angle >= std::f32::consts::PI {
            return;
        }
        if angle == 0.0 {
            let n = mesh.faces_num();
            mesh.set_bool_attribute(SHARP_FACE, Domain::Face, vec![true; n]);
            return;
        }
        let mut sharp = vec![false; mesh.edges_num()];
        edges_sharp_from_angle(mesh, angle, &mut sharp);
        mesh.set_bool_attribute(SHARP_EDGE, Domain::Edge, sharp);
    });
}

#[rna_func(srna = "Mesh", func = "set_sharp_from_angle")]
fn mesh_set_sharp_from_angle(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "set_sharp_from_angle")?;
    let me = mesh_of(this)?;
    let angle = match args.get("angle") {
        Some(Value::Float(f)) => *f,
        Some(Value::Int(i)) => *i as f32,
        _ => std::f32::consts::PI,
    }
    .clamp(0.0, std::f32::consts::PI);
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("set_sharp_from_angle(): the mesh was removed"))?;
    set_sharp_from_angle(mesh, angle);
    Ok(Value::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_angles() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        set_sharp_from_angle(&mut m, 0.5);
        assert_eq!(m.bool_attribute(SHARP_EDGE, Domain::Edge), Some(&[true; 12][..]));
        assert!(m.attributes().get(SHARP_FACE).is_none());
        set_sharp_from_angle(&mut m, 2.0);
        assert_eq!(m.bool_attribute(SHARP_EDGE, Domain::Edge), Some(&[false; 12][..]));
        set_sharp_from_angle(&mut m, 0.0);
        assert!(m.attributes().get(SHARP_EDGE).is_none());
        assert_eq!(m.bool_attribute(SHARP_FACE, Domain::Face), Some(&[true; 6][..]));
        set_sharp_from_angle(&mut m, std::f32::consts::PI);
        assert!(m.attributes().get(SHARP_FACE).is_none());
    }

    #[test]
    fn active_attribute_keeps_its_index() {
        let mut m = frap_mesh::primitives::cube(2.0, true, [1.0; 3]);
        let last = m.attributes().iter().last().map(|a| a.name.clone());
        m.attributes_active = last;
        // `sharp_face` (before the last attribute) is removed: the index now points past the
        // end.
        set_sharp_from_angle(&mut m, std::f32::consts::PI);
        assert_eq!(m.attributes_active, None);
    }
}
