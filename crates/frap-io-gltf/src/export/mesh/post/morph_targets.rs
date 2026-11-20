// SPDX-License-Identifier: GPL-3.0-or-later
//! Morph targets (WP 6.8, `export_morph`): one target per exported shape key with `POSITION`
//! deltas, `NORMAL` deltas (`export_morph_normal`) and `TANGENT` deltas
//! (`export_morph_tangent`), written sparse when that is smaller (`export_try_sparse_sk`,
//! `export_try_omit_sparse_sk`); the mesh's default `weights` and `extras.targetNames`.
//!
//! Ported from io_scene_gltf2 5.2.41 (Apache-2.0, The glTF-Blender-IO authors)
//! `blender/com/data_path.py` (`get_sk_exported`, `skip_sk`), `blender/exp/primitive_extract.py`
//! (`prepare_data`'s `key_blocks`, `__get_positions`, `__get_normals`,
//! `__calc_morph_tangents`), `blender/exp/primitives.py` (`__gather_targets`),
//! `blender/exp/accessors.py` (`array_to_accessor`, `__try_sparse_accessor`) and
//! `blender/exp/obj_data.py` (`__gather_weights`, `__gather_extras`):
//!
//! - exported keys: every key block but the first, a muted one and one relative to itself;
//! - the base positions are the first exported key's reference key (`relative_key.points`),
//!   each target `key - base` (NaN → 0), Y-up; skinned meshes in world space;
//! - normals: rounded to 4 decimals (the base also normalised), moved and normalised for
//!   skinned meshes, zero → +Z, then `morph - base`; they split vertices like the base normals
//!   (the split-key column of `writers/morph_normal.rs`);
//! - morph tangents rotate the base tangent by the rotation from the morphed to the base
//!   normal (`Vector.rotation_difference`);
//! - an accessor is sparse when `indices + values + 170 + 64` bytes is smaller than the dense
//!   data; an all-zero target keeps one zero index, or no data at all with
//!   `export_try_omit_sparse_sk` (or when even that is not smaller).
//!
//! Shape keys of an `export_apply` mesh survive only when the modifiers did not rebuild the
//! mesh (Blender's temporary mesh keeps the key pointer): frap exports them when every key
//! block still has one point per vertex.

use std::borrow::Cow;

use frap_gltf::json::{AccessorType, ComponentType, Index};
use frap_gltf::{AccessorData, AccessorOpts, BufferBuilder};
use frap_mesh::Mesh;
use frap_rna::{IdRef, MeshId, ObjectId};
use frap_scene::Main;
use frap_scene::id::{Key, Object};
use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::export::gather::Gather;
use crate::export::mesh::convert::{axis, round4};
use crate::export::mesh::ext::{
    AttrColumn, AttrValues, MeshExportCx, PRIMITIVE_POST_PROCESS, PrimitiveDraft,
    PrimitivePostProcess,
};
use crate::export::skins::{apply_mat, apply_mat3_f64, normalize_f32};
use crate::export::{ExportError, ExportSettings};

/// The morph-target options are honoured here.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(crate::export::SUPPORTED_EXPORT_PROPS)]
static PROPS: &[&str] = &[
    "export_morph",
    "export_morph_normal",
    "export_morph_tangent",
    "export_try_sparse_sk",
    "export_try_omit_sparse_sk",
];

/// `get_sk_exported`: the indices of the exported key blocks.
pub fn sk_exported(key: &Key) -> Vec<usize> {
    let first = key.key_blocks.first().map(|k| k.name.clone());
    key.key_blocks
        .iter()
        .enumerate()
        .filter(|(i, k)| !(k.relative_key == *i || k.mute || Some(&k.name) == first.as_ref()))
        .map(|(i, _)| i)
        .collect()
}

/// The shape keys a mesh exports.
pub struct ExportedKeys<'m> {
    /// The key.
    pub key: &'m Key,
    /// Exported key blocks (indices), in order.
    pub blocks: Vec<usize>,
    /// The reference key of the first exported block (`key_blocks[0].relative_key`).
    pub reference: usize,
}

/// The shape keys `mesh` exports with `export_morph` (none when no block is exported, or when
/// the key does not match the mesh's vertices).
pub fn exported_keys<'m>(
    main: &'m Main,
    settings: &ExportSettings,
    mesh: &Mesh,
) -> Option<ExportedKeys<'m>> {
    if !settings.props.export_morph.value {
        return None;
    }
    let key = mesh.shape_keys.and_then(|k| main.shape_keys.get(k))?;
    let n = mesh.verts_num();
    if key.key_blocks.iter().any(|b| b.data.len() != n) {
        return None;
    }
    let blocks = sk_exported(key);
    let first = *blocks.first()?;
    let reference = key.key_blocks[first].relative_key;
    let reference = if reference < key.key_blocks.len() {
        reference
    } else {
        0
    };
    Some(ExportedKeys {
        key,
        blocks,
        reference,
    })
}

/// `__get_positions`' source: the reference key's points when shape keys are exported, else
/// the mesh's positions (Blender space, untransformed).
pub fn base_positions<'a>(cx: &MeshExportCx<'a>) -> Cow<'a, [[f32; 3]]> {
    match exported_keys(cx.main, cx.settings, cx.mesh) {
        Some(k) => Cow::Owned(k.key.key_blocks[k.reference].data.clone()),
        None => Cow::Borrowed(cx.mesh.positions()),
    }
}

/// Whether morph normals are exported (`use_morph_normals`).
fn use_morph_normals(s: &ExportSettings) -> bool {
    s.export_normals && s.props.export_morph.value && s.props.export_morph_normal.value
}

/// `ShapeKey.normals_split_get()`: the corner normals of the mesh with the key's positions.
pub fn key_corner_normals(mesh: &Mesh, positions: &[[f32; 3]]) -> std::sync::Arc<Vec<[f32; 3]>> {
    let mut m = mesh.clone();
    m.set_positions(positions.to_vec());
    m.corner_normals()
}

/// Base normals and morph normals per corner, in glTF space.
pub struct MorphNormals {
    /// `NORMAL`: the reference key's normals (`relative_key.normals_split_get()`).
    pub base: Vec<[f32; 3]>,
    /// Per exported key, the `NORMAL` target deltas.
    pub deltas: Vec<Vec<[f32; 3]>>,
}

/// The morph-normal deltas of every exported key, per corner, in glTF space (`None` without
/// morph normals): the split-key column of `writers/morph_normal.rs` and the `NORMAL` targets.
pub fn morph_normal_deltas(cx: &MeshExportCx) -> Option<Vec<Vec<[f32; 3]>>> {
    morph_normals(cx).map(|m| m.deltas)
}

/// [`morph_normal_deltas`] with the base normals: with morph normals the add-on reads the base
/// `NORMAL` from the reference key too (not from the evaluated mesh, whose shape keys are
/// mixed in).
pub fn morph_normals(cx: &MeshExportCx) -> Option<MorphNormals> {
    if !use_morph_normals(cx.settings) || cx.mesh.corners_num() == 0 {
        return None;
    }
    let keys = exported_keys(cx.main, cx.settings, cx.mesh)?;
    let yup = cx.settings.export_yup;
    let t = cx.skin.map(|s| s.normal_transform());
    let finish = |n: [f32; 3]| {
        let mut n = match &t {
            Some(t) => normalize_f32(apply_mat3_f64(t, n.map(f64::from)).map(|x| x as f32)),
            None => n,
        };
        if n == [0.0; 3] {
            n = [0.0, 0.0, 1.0];
        }
        n
    };
    let base: Vec<[f32; 3]> =
        key_corner_normals(cx.mesh, &keys.key.key_blocks[keys.reference].data)
            .iter()
            .map(|n| finish(normalize_f32(n.map(round4))))
            .collect();
    let deltas = keys
        .blocks
        .iter()
        .map(|&b| {
            let ns = key_corner_normals(cx.mesh, &keys.key.key_blocks[b].data);
            ns.iter()
                .zip(&base)
                .map(|(n, b)| {
                    let m = finish(n.map(round4));
                    axis([m[0] - b[0], m[1] - b[1], m[2] - b[2]], yup)
                })
                .collect()
        })
        .collect();
    Some(MorphNormals {
        base: base.into_iter().map(|n| axis(n, yup)).collect(),
        deltas,
    })
}

// Ported from blender/source/blender/blenlib/intern/math_rotation.cc
// (rotation_between_vecs_to_quat, mul_qt_v3), math_vector.cc (angle_normalized_v3v3,
// ortho_v3_v3) and math_base_inline.c (safe_asinf), Blender 5.2.4, GPL-2.0-or-later.
fn safe_asinf(a: f32) -> f32 {
    if a.abs() <= 1.0 {
        a.asin()
    } else if a < 0.0 {
        -std::f32::consts::FRAC_PI_2
    } else {
        std::f32::consts::FRAC_PI_2
    }
}

fn angle_normalized(v1: [f32; 3], v2: [f32; 3]) -> f32 {
    let dot = v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2];
    let len = |a: [f32; 3], b: [f32; 3]| {
        let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    };
    if dot >= 0.0 {
        2.0 * safe_asinf(len(v1, v2) / 2.0)
    } else {
        std::f32::consts::PI - 2.0 * safe_asinf(len(v1, v2.map(|x| -x)) / 2.0)
    }
}

fn ortho(v: [f32; 3]) -> [f32; 3] {
    let (x, y, z) = (v[0].abs(), v[1].abs(), v[2].abs());
    let axis = if x > y {
        if x > z { 0 } else { 2 }
    } else if y > z {
        1
    } else {
        2
    };
    match axis {
        0 => [-v[1] - v[2], v[0], v[0]],
        1 => [v[1], -v[0] - v[2], v[1]],
        _ => [v[2], v[2], -v[0] - v[1]],
    }
}

/// `Vector.rotation_difference(other)` (both normalised first).
fn rotation_difference(a: [f32; 3], b: [f32; 3]) -> [f32; 4] {
    let (v1, _) = frap_math::matrix::normalize_v3(a);
    let (v2, _) = frap_math::matrix::normalize_v3(b);
    let cross = [
        v1[1] * v2[2] - v1[2] * v2[1],
        v1[2] * v2[0] - v1[0] * v2[2],
        v1[0] * v2[1] - v1[1] * v2[0],
    ];
    let (axis, len) = frap_math::matrix::normalize_v3(cross);
    if len > f32::EPSILON {
        frap_math::quat::axis_angle_normalized_to_quat(axis, angle_normalized(v1, v2))
    } else if v1[0] * v2[0] + v1[1] * v2[1] + v1[2] * v2[2] > 0.0 {
        [1.0, 0.0, 0.0, 0.0]
    } else {
        frap_math::quat::axis_angle_to_quat(ortho(v1), std::f32::consts::PI)
    }
}

fn mul_qt_v3(q: [f32; 4], r: [f32; 3]) -> [f32; 3] {
    let mut r = r;
    let t0 = -q[1] * r[0] - q[2] * r[1] - q[3] * r[2];
    let t1 = q[0] * r[0] + q[2] * r[2] - q[3] * r[1];
    let t2 = q[0] * r[1] + q[3] * r[0] - q[1] * r[2];
    r[2] = q[0] * r[2] + q[1] * r[1] - q[2] * r[0];
    r[0] = t1;
    r[1] = t2;
    let t1 = t0 * -q[1] + r[0] * q[0] - r[1] * q[3] + r[2] * q[2];
    let t2 = t0 * -q[2] + r[1] * q[0] - r[2] * q[1] + r[0] * q[3];
    r[2] = t0 * -q[3] + r[2] * q[0] - r[0] * q[2] + r[1] * q[1];
    r[0] = t1;
    r[1] = t2;
    r
}

/// `__calc_morph_tangents` for one vertex (glTF space): the base tangent rotated from the
/// morphed normal to the base normal, minus the base tangent.
pub fn morph_tangent(n: [f32; 3], morph_delta: [f32; 3], t: [f32; 3]) -> [f32; 3] {
    let morph_n = [n[0] + morph_delta[0], n[1] + morph_delta[1], n[2] + morph_delta[2]];
    let q = rotation_difference(morph_n, n);
    let tm = mul_qt_v3(q, t);
    [tm[0] - t[0], tm[1] - t[1], tm[2] - t[2]]
}

fn run(cx: &MeshExportCx, d: &mut PrimitiveDraft, _: &mut BufferBuilder) -> Result<(), ExportError> {
    let Some(keys) = exported_keys(cx.main, cx.settings, cx.mesh) else {
        return Ok(());
    };
    let yup = cx.settings.export_yup;
    let lt = cx.skin.map(|s| s.object_world);
    let place = |p: [f32; 3]| match &lt {
        Some(m) => apply_mat(m, p),
        None => p,
    };
    let base: Vec<[f32; 3]> = keys.key.key_blocks[keys.reference]
        .data
        .iter()
        .map(|&p| place(p))
        .collect();
    // POSITION: from the reference key (the extractors used the mesh's positions).
    d.positions = d
        .vertices
        .iter()
        .map(|&v| axis(base[v as usize], yup))
        .collect();
    let corner_data = !d.corners.contains(&u32::MAX) && !d.normals.is_empty();
    let morph_normals = if corner_data { morph_normals(cx) } else { None };
    if let Some(m) = &morph_normals {
        d.normals = d.corners.iter().map(|&c| m.base[c as usize]).collect();
    }
    let normal_deltas = morph_normals.map(|m| m.deltas);
    let tangents = match (&normal_deltas, d.attributes.get("TANGENT")) {
        (Some(_), Some(AttrColumn {
            values: AttrValues::F32x4(t),
            ..
        })) if cx.settings.props.export_morph_tangent.value => Some(t.clone()),
        _ => None,
    };
    d.targets.clear();
    for (k, &b) in keys.blocks.iter().enumerate() {
        let data = &keys.key.key_blocks[b].data;
        let pos: Vec<[f32; 3]> = d
            .vertices
            .iter()
            .map(|&v| {
                let p = place(data[v as usize]);
                let o = base[v as usize];
                let dlt = [p[0] - o[0], p[1] - o[1], p[2] - o[2]].map(|x| if x.is_nan() { 0.0 } else { x });
                axis(dlt, yup)
            })
            .collect();
        let mut target: IndexMap<SmolStr, AttrColumn> = IndexMap::new();
        target.insert(
            "POSITION".into(),
            AttrColumn {
                values: AttrValues::F32x3(pos),
                normalized: false,
            },
        );
        if let Some(nd) = &normal_deltas {
            let ns: Vec<[f32; 3]> = d.corners.iter().map(|&c| nd[k][c as usize]).collect();
            if let Some(ts) = &tangents {
                let mt: Vec<[f32; 3]> = ts
                    .iter()
                    .zip(&d.normals)
                    .zip(&ns)
                    .map(|((t, n), m)| morph_tangent(*n, *m, [t[0], t[1], t[2]]))
                    .collect();
                target.insert(
                    "NORMAL".into(),
                    AttrColumn {
                        values: AttrValues::F32x3(ns),
                        normalized: false,
                    },
                );
                target.insert(
                    "TANGENT".into(),
                    AttrColumn {
                        values: AttrValues::F32x3(mt),
                        normalized: false,
                    },
                );
            } else {
                target.insert(
                    "NORMAL".into(),
                    AttrColumn {
                        values: AttrValues::F32x3(ns),
                        normalized: false,
                    },
                );
            }
        }
        d.targets.push(target);
    }
    Ok(())
}

#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(PRIMITIVE_POST_PROCESS)]
static MORPH_TARGETS: PrimitivePostProcess = PrimitivePostProcess {
    name: "morph_targets",
    order: 100,
    run,
};

/// How a target accessor is written (`array_to_accessor(..., sparse_type='SK')`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetLayout {
    /// A plain buffer view.
    Dense,
    /// Sparse values at these indices over zeros.
    Sparse(Vec<u32>),
    /// No buffer view and no sparse data (all zeros).
    Empty,
}

/// `__try_sparse_accessor` and the choice of `array_to_accessor`: `rows` elements of
/// `elem_size` bytes, `nonzero` the rows with a non-zero component.
pub fn target_layout(
    rows: usize,
    elem_size: usize,
    nonzero: Vec<u32>,
    try_sparse: bool,
    try_omit: bool,
) -> TargetLayout {
    if !try_sparse {
        return TargetLayout::Dense;
    }
    let omit = nonzero.is_empty();
    let indices = if omit { vec![0] } else { nonzero };
    let last = *indices.last().unwrap_or(&0);
    let index_size = if last <= 255 {
        1
    } else if last <= 65535 {
        2
    } else {
        4
    };
    let dense_size = rows * elem_size;
    let sparse_size = index_size * indices.len() + indices.len() * elem_size + 170 + 64;
    let sparse = sparse_size < dense_size;
    match (sparse, omit) {
        (false, false) => TargetLayout::Dense,
        (true, false) => TargetLayout::Sparse(indices),
        (_, true) if try_omit => TargetLayout::Empty,
        (true, true) => TargetLayout::Sparse(indices),
        (false, true) => TargetLayout::Empty,
    }
}

/// Writes one morph-target accessor (`POSITION` with min/max), sparse or dense as the add-on
/// decides.
pub fn push_target_accessor(
    buf: &mut BufferBuilder,
    settings: &ExportSettings,
    name: &str,
    data: &AccessorData,
) -> Result<Index, ExportError> {
    let p = &settings.props;
    let try_sparse = p.export_try_sparse_sk.value;
    let try_omit = try_sparse && p.export_try_omit_sparse_sk.value;
    let opts = AccessorOpts::attribute(name == "POSITION");
    let n = data.type_.components();
    let flat = data.to_f32();
    let rows = data.count();
    let nonzero: Vec<u32> = (0..rows)
        .filter(|&r| flat[r * n..(r + 1) * n].iter().any(|&x| x != 0.0))
        .map(|r| r as u32)
        .collect();
    match target_layout(rows, data.element_size(), nonzero, try_sparse, try_omit) {
        TargetLayout::Dense => Ok(buf.push_accessor(data, &opts)),
        TargetLayout::Sparse(indices) => buf
            .push_sparse(data, &indices, None, &opts)
            .map_err(|e| ExportError::Invalid(e.to_string())),
        TargetLayout::Empty => {
            let mut acc = frap_gltf::json::Accessor::new(
                None,
                ComponentType::Float,
                AccessorType::Vec3,
                rows as u64,
            );
            if opts.min_max
                && let Some((min, max)) = data.min_max()
            {
                acc.min = Some(min);
                acc.max = Some(max);
            }
            Ok(buf.push_accessor_obj(acc))
        }
    }
}

/// `__gather_weights` / `__gather_extras` of the glTF mesh `mesh_index` of object `ob`: the
/// exported keys' values as `weights` and their names as `extras.targetNames` (when the key has
/// more than the basis).
pub fn set_mesh_weights(g: &mut Gather, ob_ref: IdRef<ObjectId>, ob: &Object, mesh_index: Index) {
    let main = g.main;
    let evaluated = crate::export::apply::evaluated(g, ob_ref);
    let data: Option<&Mesh> = match &evaluated {
        Some(m) => Some(m),
        None => ob
            .data
            .and_then(|d| d.typed::<MeshId>())
            .and_then(|m| main.meshes.get(m)),
    };
    let Some(data) = data else {
        return;
    };
    if !g.settings.props.export_morph.value {
        return;
    }
    let Some(key) = data.shape_keys.and_then(|k| main.shape_keys.get(k)) else {
        return;
    };
    if key.key_blocks.iter().any(|b| b.data.len() != data.verts_num()) || key.key_blocks.len() < 2
    {
        return;
    }
    let blocks = sk_exported(key);
    let weights: Vec<f64> = blocks
        .iter()
        .map(|&b| f64::from(key.key_blocks[b].value))
        .collect();
    let names: Vec<serde_json::Value> = blocks
        .iter()
        .map(|&b| serde_json::Value::String(key.key_blocks[b].name.to_string()))
        .collect();
    let Some(m) = g.doc.meshes.get_mut(mesh_index as usize) else {
        return;
    };
    m.weights = weights;
    let mut extras = m
        .extras
        .as_ref()
        .and_then(|e| serde_json::from_str::<serde_json::Value>(e.0.get()).ok())
        .and_then(|v| match v {
            serde_json::Value::Object(o) => Some(o),
            _ => None,
        })
        .unwrap_or_default();
    extras.insert("targetNames".into(), serde_json::Value::Array(names));
    m.extras = Some(frap_gltf::json::Extras::from_value(&serde_json::Value::Object(
        extras,
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_scene::id::ShapeKey;

    #[test]
    fn exported_keys_skip_basis_muted_and_self_relative() {
        let mut k = Key::new("Key");
        for name in ["Basis", "A", "B", "C", "D"] {
            k.key_blocks.push(ShapeKey::new(name, vec![[0.0; 3]]));
        }
        k.key_blocks[2].mute = true;
        k.key_blocks[3].relative_key = 3;
        k.key_blocks[4].relative_key = 1;
        assert_eq!(sk_exported(&k), vec![1, 4]);
        // A later block named like the first one is skipped too (compared by name).
        k.key_blocks[4].name = "Basis".into();
        assert_eq!(sk_exported(&k), vec![1]);
    }

    #[test]
    fn sparse_heuristic() {
        // 100 VEC3 rows, 2 non-zero: 2 + 24 + 234 = 260 < 1200: sparse.
        assert_eq!(
            target_layout(100, 12, vec![3, 50], true, false),
            TargetLayout::Sparse(vec![3, 50])
        );
        // 10 rows: dense (120 bytes) is smaller.
        assert_eq!(target_layout(10, 12, vec![3], true, false), TargetLayout::Dense);
        // Not trying: dense.
        assert_eq!(target_layout(100, 12, vec![3], false, false), TargetLayout::Dense);
        // All zero: one zero index when that is smaller, else no data; omitted on request.
        assert_eq!(
            target_layout(100, 12, vec![], true, false),
            TargetLayout::Sparse(vec![0])
        );
        assert_eq!(target_layout(10, 12, vec![], true, false), TargetLayout::Empty);
        assert_eq!(target_layout(100, 12, vec![], true, true), TargetLayout::Empty);
        // Index width: past 255 the indices take two bytes.
        let idx: Vec<u32> = (0..30).map(|i| i * 10).collect();
        // 30 * 2 + 30 * 12 + 234 = 654 < 3600.
        assert!(matches!(
            target_layout(300, 12, idx, true, false),
            TargetLayout::Sparse(_)
        ));
    }

    #[test]
    fn morph_tangent_follows_the_normal() {
        // The morphed normal tilted 90° about Z: the tangent rotates back by -90°.
        let t = morph_tangent([0.0, 0.0, 1.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
        assert_eq!(t, [0.0; 3]);
        let t = morph_tangent([0.0, 1.0, 0.0], [1.0, -1.0, 0.0], [1.0, 0.0, 0.0]);
        // morph n = +X, base n = +Y: rotation X→Y applied to +X gives +Y.
        assert!((t[0] + 1.0).abs() < 1e-6 && (t[1] - 1.0).abs() < 1e-6, "{t:?}");
        // Opposite normals: a half turn about an orthogonal axis.
        let q = rotation_difference([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]);
        assert!(q[0].abs() < 1e-6);
    }
}
