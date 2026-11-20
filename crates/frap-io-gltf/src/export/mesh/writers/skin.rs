// SPDX-License-Identifier: GPL-3.0-or-later
//! Skinning writer (WP 6.8, `export_skins`): for a skinned mesh (`MeshExportCx::skin`, decided
//! by `export/skins.rs`), `JOINTS_n` / `WEIGHTS_n` per glTF vertex from the vertex groups, and
//! the mesh moved into world space (glTF skins ignore the node transform): positions by the
//! object's world matrix, normals by the armature's inverse-transpose `normal_transform`,
//! tangents by the rotation of the object relative to the armature. No split-key column:
//! joints and weights are per vertex.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors)
//! `blender/exp/primitive_extract.py` (`__get_bone_data`, `primitive_creation_not_shared`'s
//! joint sets, the skinning branches of `__get_positions`, `__get_normals`, `__get_tangents`,
//! `__get_bitangent_signs`) and `blender/exp/primitive_attributes.py` (`__gather_skins`):
//!
//! - every vertex lists its `(joint, weight)` pairs above 0.0001, heaviest first; one set of
//!   four per `JOINTS_n`/`WEIGHTS_n`, as many sets as the most influenced vertex of the mesh
//!   needs, padded with `(0, 0.0)`;
//! - `export_influence_nb` keeps `ceil(n / 4)` sets (all with `export_all_influences`) and
//!   zeroes the last influences of the last set beyond `n`;
//! - `JOINTS_n` is UNSIGNED_BYTE when every value of the set is below 256, else
//!   UNSIGNED_SHORT; `WEIGHTS_n` is FLOAT, each vertex normalised to a sum of 1 over the kept
//!   sets (float32);
//! - a vertex no joint weighs is bound to the neutral bone (index = number of joints).

use frap_mesh::Mesh;

use crate::export::ExportError;
use crate::export::mesh::convert::{axis, round4};
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_ATTRIBUTE_WRITERS, PrimitiveAttributeWriter,
    PrimitiveDraft, SplitKey,
};
use crate::export::skins::{MeshSkin, apply_mat, apply_mat3_f64, normalize_f32, normalize_f64};

/// This writer's name.
pub const NAME: &str = "skin";

/// The vertex positions the primitives are built from (the reference key's when shape keys are
/// exported), in world space (`loc_transform = blender_object.matrix_world`).
pub fn skinned_positions(cx: &MeshExportCx, skin: &MeshSkin) -> Vec<[f32; 3]> {
    crate::export::mesh::post::morph_targets::base_positions(cx)
        .iter()
        .map(|&p| apply_mat(&skin.object_world, p))
        .collect()
}

/// A corner normal of the skinned mesh before the Y-up swizzle: rounded to 4 decimals,
/// normalised, moved by `normal_transform` (stored as float32), normalised again, zero → +Z.
pub fn skinned_normal(n: [f32; 3], t: &frap_math::glam::Mat3) -> [f32; 3] {
    let r = normalize_f32(n.map(round4));
    let m = apply_mat3_f64(t, r.map(f64::from)).map(|x| x as f32);
    let mut m = normalize_f32(m);
    if m == [0.0; 3] {
        m = [0.0, 0.0, 1.0];
    }
    m
}

/// A corner tangent of the skinned mesh before the Y-up swizzle: rounded, rotated by
/// `tangent_transform`, normalised and rounded again (numpy keeps float64 here).
fn skinned_tangent(t: [f32; 3], m: &frap_math::glam::Mat3) -> [f32; 3] {
    let r = t.map(|x| f64::from(round4(x)));
    let v = normalize_f64(apply_mat3_f64(m, r));
    v.map(|x| ((x * 10000.0).round_ties_even() / 10000.0) as f32)
}

/// `__gather_skins`: the `JOINTS_n` and `WEIGHTS_n` columns of one primitive from its
/// vertices' bone lists (`num_sets` sets of four).
pub fn joint_weight_columns(
    bones: &[&[(u32, f32)]],
    num_sets: usize,
    influence_nb: i32,
    all_influences: bool,
) -> (Vec<AttrColumn>, Vec<AttrColumn>) {
    if num_sets == 0 {
        return (Vec::new(), Vec::new());
    }
    let max_set = num_sets as i64 - 1;
    let wanted = if all_influences {
        max_set
    } else {
        (i64::from(influence_nb) + 3).div_euclid(4) - 1
    }
    .min(max_set);
    let last = max_set.min(wanted);
    if last < 0 {
        return (Vec::new(), Vec::new());
    }
    let to_remove = if all_influences {
        0
    } else {
        ((wanted + 1) * 4 - i64::from(influence_nb)).max(0) as usize
    };
    let mut joint_cols = Vec::new();
    let mut weight_sets: Vec<Vec<[f32; 4]>> = Vec::new();
    for s in 0..=last as usize {
        let mut js: Vec<[u32; 4]> = Vec::with_capacity(bones.len());
        let mut ws: Vec<[f32; 4]> = Vec::with_capacity(bones.len());
        for b in bones {
            let mut j = [0u32; 4];
            let mut w = [0f32; 4];
            for k in 0..4 {
                if let Some(&(jj, ww)) = b.get(4 * s + k) {
                    j[k] = jj;
                    w[k] = ww;
                }
            }
            js.push(j);
            ws.push(w);
        }
        let small = js.iter().flatten().all(|&j| j < 256);
        if s == last as usize {
            for i in 0..to_remove.min(4) {
                let idx = 3 - i;
                for (j, w) in js.iter_mut().zip(ws.iter_mut()) {
                    j[idx] = 0;
                    w[idx] = 0.0;
                }
            }
        }
        let values = if small {
            AttrValues::U8x4(js.iter().map(|j| j.map(|x| x as u8)).collect())
        } else {
            AttrValues::U16x4(js.iter().map(|j| j.map(|x| x as u16)).collect())
        };
        joint_cols.push(AttrColumn {
            values,
            normalized: false,
        });
        weight_sets.push(ws);
    }
    // Normalise so each vertex's weights sum to 1 (float32, sets summed in order).
    let n = bones.len();
    let totals: Vec<f32> = (0..n)
        .map(|v| {
            let mut t = 0.0f32;
            for (s, ws) in weight_sets.iter().enumerate() {
                let w = ws[v];
                let sum = ((w[0] + w[1]) + w[2]) + w[3];
                t = if s == 0 { sum } else { t + sum };
            }
            t
        })
        .collect();
    let weight_cols = weight_sets
        .into_iter()
        .map(|ws| AttrColumn {
            values: AttrValues::F32x4(
                ws.iter()
                    .zip(&totals)
                    .map(|(w, &t)| w.map(|x| x / t))
                    .collect(),
            ),
            normalized: false,
        })
        .collect();
    (joint_cols, weight_cols)
}

fn run(cx: &MeshExportCx, _: &SplitKey, d: &mut PrimitiveDraft) -> Result<(), ExportError> {
    let Some(skin) = cx.skin else {
        return Ok(());
    };
    let yup = cx.settings.export_yup;
    let mesh: &Mesh = cx.mesh;
    // POSITION in world space.
    let positions = skinned_positions(cx, skin);
    d.positions = d
        .vertices
        .iter()
        .map(|&v| axis(positions[v as usize], yup))
        .collect();
    // NORMAL and TANGENT (corner data: not on loose edges and points).
    if !d.corners.contains(&u32::MAX) {
        if !d.normals.is_empty() {
            let normals = mesh.corner_normals();
            let t = skin.normal_transform();
            d.normals = d
                .corners
                .iter()
                .map(|&c| axis(skinned_normal(normals[c as usize], &t), yup))
                .collect();
        }
        if let Some(col) = d.attributes.get_mut("TANGENT")
            && let AttrValues::F32x4(values) = &mut col.values
            && let Some(tangents) = crate::export::mesh::writers::tangent::mesh_tangents(mesh)
        {
            let m = skin.tangent_transform();
            for (v, &c) in values.iter_mut().zip(&d.corners) {
                let t = tangents[c as usize];
                let [x, y, z] = axis(skinned_tangent([t[0], t[1], t[2]], &m), yup);
                // The sign flips only for a mirroring transform; a rotation never mirrors.
                *v = [x, y, z, t[3]];
            }
        }
    }
    // JOINTS_n / WEIGHTS_n.
    let (bones, _) = skin.vert_bones(mesh.verts_num(), mesh.deform_verts());
    let max_influences = bones.iter().map(|b| b.len()).max().unwrap_or(0);
    let num_sets = max_influences.div_ceil(4);
    let prim: Vec<&[(u32, f32)]> = d
        .vertices
        .iter()
        .map(|&v| bones.get(v as usize).map(|b| b.as_slice()).unwrap_or(&[]))
        .collect();
    let (joints, weights) =
        joint_weight_columns(&prim, num_sets, skin.influence_nb, skin.all_influences);
    for (s, c) in joints.into_iter().enumerate() {
        d.attributes.insert(format!("JOINTS_{s}").into(), c);
    }
    for (s, c) in weights.into_iter().enumerate() {
        d.attributes.insert(format!("WEIGHTS_{s}").into(), c);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_ATTRIBUTE_WRITERS)]
static SKIN: PrimitiveAttributeWriter = PrimitiveAttributeWriter {
    name: NAME,
    order: 500,
    key: None,
    run,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn weights(c: &AttrColumn) -> Vec<[f32; 4]> {
        match &c.values {
            AttrValues::F32x4(v) => v.clone(),
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn one_set_normalised_and_padded() {
        let a: &[(u32, f32)] = &[(2, 0.6), (0, 0.2)];
        let b: &[(u32, f32)] = &[(3, 1.0)];
        let (j, w) = joint_weight_columns(&[a, b], 1, 4, false);
        assert_eq!(j.len(), 1);
        assert_eq!(
            j[0].values,
            AttrValues::U8x4(vec![[2, 0, 0, 0], [3, 0, 0, 0]])
        );
        let w = weights(&w[0]);
        assert_eq!(w[0], [0.6 / 0.8f32, 0.2 / 0.8f32, 0.0, 0.0]);
        assert_eq!(w[1], [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn influence_limit_drops_the_lightest() {
        let a: &[(u32, f32)] = &[(1, 0.4), (2, 0.3), (3, 0.2), (4, 0.1), (5, 0.05)];
        // Two sets needed; 3 influences kept: one set, its last weight zeroed.
        let (j, w) = joint_weight_columns(&[a], 2, 3, false);
        assert_eq!(j.len(), 1);
        assert_eq!(j[0].values, AttrValues::U8x4(vec![[1, 2, 3, 0]]));
        let w = weights(&w[0]);
        assert!((w[0][0] - 0.4 / 0.9).abs() < 1e-7 && w[0][3] == 0.0);
        // All influences: both sets, normalised over both.
        let (j, w) = joint_weight_columns(&[a], 2, 3, true);
        assert_eq!(j.len(), 2);
        assert_eq!(j[1].values, AttrValues::U8x4(vec![[5, 0, 0, 0]]));
        let total: f32 = weights(&w[0])[0].iter().chain(weights(&w[1])[0].iter()).sum();
        assert!((total - 1.0).abs() < 1e-6);
        // Asking for more than needed keeps what exists.
        let (j, _) = joint_weight_columns(&[a], 2, 12, false);
        assert_eq!(j.len(), 2);
    }

    #[test]
    fn joints_widen_to_ushort() {
        let a: &[(u32, f32)] = &[(300, 1.0)];
        let (j, _) = joint_weight_columns(&[a], 1, 4, false);
        assert_eq!(j[0].values, AttrValues::U16x4(vec![[300, 0, 0, 0]]));
    }

    #[test]
    fn skinned_normal_is_renormalised() {
        let t = frap_math::glam::Mat3::from_diagonal(frap_math::glam::Vec3::new(2.0, 1.0, 1.0));
        let n = skinned_normal([0.6, 0.8, 0.0], &t);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-6);
        assert!(n[0] > 0.8);
        assert_eq!(skinned_normal([0.0; 3], &t), [0.0, 0.0, 1.0]);
    }
}
