// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.transform(matrix, shape_keys=False)` (`rna_Mesh_transform` → `bke::mesh_transform`):
//! every vertex position by the 4×4 matrix (a `mathutils.Matrix` or nested rows), and with
//! `shape_keys=True` every block of the mesh's key too (WP 6.3). Verified on 5.2.4:
//! `Matrix.Translation((1, 0, 0))` moves `(1, 1, 1)` to `(2, 1, 1)`; a scale matrix given as
//! rows scales; without `shape_keys` the key blocks keep their coordinates (so the evaluated
//! mesh, which starts from the basis block, does not move).

use frap_rna::traits::FromRnaValue;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, internal, mesh_of};

#[rna_func(srna = "Mesh", func = "transform")]
fn mesh_transform(h: &mut dyn Host, this: &RnaPtr, args: &mut FuncArgs) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "transform")?;
    let me = mesh_of(this)?;
    let prop = args
        .func
        .params
        .iter()
        .find(|p| p.identifier == "matrix")
        .ok_or_else(|| internal("transform(): no matrix parameter"))?;
    let v = args
        .get("matrix")
        .cloned()
        .ok_or_else(|| internal("transform(): matrix missing"))?;
    let m = glam::Mat4::from_rna_value(prop, v)?;
    let shape_keys = arg_bool(args, "shape_keys", false);
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("transform(): the mesh was removed"))?;
    mesh.transform(&m);
    let key = mesh.shape_keys;
    if shape_keys
        && let Some(k) = key
        && let Some(key) = main.shape_keys.get_mut(k, log)
    {
        for kb in &mut key.key_blocks {
            for p in &mut kb.data {
                *p = m.transform_point3(glam::Vec3::from_array(*p)).to_array();
            }
        }
    }
    Ok(Value::None)
}
