// SPDX-License-Identifier: GPL-3.0-or-later
//! `Mesh.from_pydata(vertices, edges, faces, shade_flat=True)` (Python-only, `_bpy_types.py`),
//! over `frap_mesh::Mesh::from_pydata` (WP 1.4, which ports the RNA calls it makes and
//! Blender's attribute order). Moved here from WP 1.27 (decision log W0-22).
//!
//! Verified on Blender 5.2.4:
//! - every argument is a sequence (`object of type 'int' has no len()` otherwise); vertices
//!   are 3 numbers, edges 2 indices, faces any number of indices (lists or tuples);
//! - a non-number raises `TypeError: couldn't access the py sequence` (a float index too);
//!   vertices of the wrong length raise `RuntimeError: internal error setting the array`;
//! - on a mesh that already has geometry, adding elements raises the same `RuntimeError`
//!   (Blender has already appended them; frap's transaction rolls back), while empty lists
//!   only run `shade_flat()`;
//! - `shade_flat` (default True) creates `sharp_face`.
//!
//! Deviation (frap-only `ValueError`): Blender silently builds an invalid mesh from an index
//! outside the vertices or a face with fewer than 3 corners; frap refuses both, because the
//! later stages (normals, triangulation, export) rely on a valid mesh.

use frap_rna::{PyExc, RnaError, RnaErrorKind, RnaPtr, Value, rna_func};
use frap_scene::{FuncArgs, Host};

use super::_common::{arg_bool, internal, invalid, mesh_of};

fn not_a_sequence() -> RnaError {
    RnaError::new(
        RnaErrorKind::Type,
        PyExc::TypeError,
        "couldn't access the py sequence",
    )
}

fn bad_length() -> RnaError {
    RnaError::new(
        RnaErrorKind::Other,
        PyExc::RuntimeError,
        "internal error setting the array",
    )
}

/// The items of a sequence argument (`Value::Tuple` from the untyped binding).
fn items<'v>(args: &'v FuncArgs, name: &str) -> Result<&'v [Value], RnaError> {
    match args.get(name) {
        Some(Value::Tuple(v)) => Ok(v),
        _ => Err(internal(format!("from_pydata(): {name} is not a sequence"))),
    }
}

/// One element's numbers (a tuple/list of numbers, or a mathutils vector).
fn numbers(v: &Value) -> Result<Vec<f64>, RnaError> {
    match v {
        Value::Tuple(xs) => xs
            .iter()
            .map(|x| match x {
                Value::Int(i) => Ok(*i as f64),
                Value::Double(f) => Ok(*f),
                Value::Float(f) => Ok(*f as f64),
                Value::Bool(b) => Ok(f64::from(u8::from(*b))),
                _ => Err(not_a_sequence()),
            })
            .collect(),
        Value::FloatArray { data, .. } => Ok(data.iter().map(|x| *x as f64).collect()),
        _ => Err(not_a_sequence()),
    }
}

/// One element's indices (integers only, as `foreach_set` on an INT property).
fn indices(v: &Value) -> Result<Vec<i64>, RnaError> {
    match v {
        Value::Tuple(xs) => xs
            .iter()
            .map(|x| match x {
                Value::Int(i) => Ok(*i),
                Value::Bool(b) => Ok(i64::from(*b)),
                _ => Err(not_a_sequence()),
            })
            .collect(),
        _ => Err(not_a_sequence()),
    }
}

/// Checks an index against the vertex count (frap-only `ValueError`).
fn vertex_index(i: i64, nverts: usize, what: &str, n: usize) -> Result<i32, RnaError> {
    if i < 0 || i >= nverts as i64 {
        return Err(invalid(format!(
            "Mesh.from_pydata(): {what} {n} uses vertex index {i}, but there are {nverts} \
             vertices (Blender would build an invalid mesh)"
        )));
    }
    Ok(i as i32)
}

#[rna_func(srna = "Mesh", func = "from_pydata")]
fn mesh_from_pydata(
    h: &mut dyn Host,
    this: &RnaPtr,
    args: &mut FuncArgs,
) -> Result<Value, RnaError> {
    super::_common::refuse_detached(this, "from_pydata")?;
    let me = mesh_of(this)?;
    let shade_flat = arg_bool(args, "shade_flat", true);
    let mut verts = Vec::new();
    for v in items(args, "vertices")? {
        let xs = numbers(v)?;
        let [x, y, z] = xs[..] else {
            return Err(bad_length());
        };
        verts.push([x as f32, y as f32, z as f32]);
    }
    let mut edges = Vec::new();
    for (n, e) in items(args, "edges")?.iter().enumerate() {
        let ix = indices(e)?;
        let [a, b] = ix[..] else {
            return Err(bad_length());
        };
        edges.push([
            vertex_index(a, verts.len(), "edge", n)?,
            vertex_index(b, verts.len(), "edge", n)?,
        ]);
    }
    let mut faces = Vec::new();
    for (n, f) in items(args, "faces")?.iter().enumerate() {
        let ix = indices(f)?;
        if ix.len() < 3 {
            return Err(invalid(format!(
                "Mesh.from_pydata(): face {n} has {} corners; a face needs at least 3 \
                 (Blender would build an invalid mesh)",
                ix.len()
            )));
        }
        faces.push(
            ix.into_iter()
                .map(|i| vertex_index(i, verts.len(), "face", n))
                .collect::<Result<Vec<i32>, _>>()?,
        );
    }
    let (main, log) = h.main_mut();
    let mesh = main
        .meshes
        .get(me)
        .ok_or_else(|| internal("from_pydata(): the mesh was removed"))?;
    let has_geometry = mesh.verts_num() > 0 || mesh.edges_num() > 0 || mesh.faces_num() > 0;
    let adds = !verts.is_empty() || !edges.is_empty() || !faces.is_empty();
    if has_geometry && adds {
        return Err(bad_length());
    }
    let mesh = main
        .meshes
        .get_mut(me, log)
        .ok_or_else(|| internal("from_pydata(): the mesh was removed"))?;
    if adds {
        mesh.from_pydata(&verts, &edges, &faces, shade_flat);
    } else if shade_flat {
        mesh.shade_flat();
    }
    Ok(Value::None)
}
