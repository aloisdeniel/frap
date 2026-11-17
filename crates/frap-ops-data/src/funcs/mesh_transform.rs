// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.transform(matrix, shape_keys=False)` (`BKE_mesh_transform`): every vertex position
//! by the 4×4 matrix (a `mathutils.Matrix` or nested rows). Verified on 5.2.4:
//! `Matrix.Translation((1, 0, 0))` moves `(1, 1, 1)` to `(2, 1, 1)`; a scale matrix given as
//! rows scales. frap meshes have no shape keys (M6) or custom normals to transform in M1.

use frap_rna::traits::FromRnaValue;
use frap_rna::{RnaError, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{internal, mesh_of};

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
    let (main, log) = h.main_mut();
    main.meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("transform(): the mesh was removed"))?
        .transform(&m);
    Ok(Value::None)
}
