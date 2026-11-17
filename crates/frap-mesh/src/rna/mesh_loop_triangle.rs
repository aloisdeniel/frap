// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshLoopTriangle` (`mesh.loop_triangles[i]`), `MeshNormalValue` (`vertex_normals`,
//! `polygon_normals`, `corner_normals`) and `ReadOnlyInteger` (`loop_triangle_polygons`).
//! Everything here is read-only and derived from the cached loop triangles and normals.
//!
//! `MeshLoopTriangle.normal` is the triangle's own normal (`normal_tri_v3` of its three
//! positions), `split_normals` the corner normals of its three corners (row `k` = corner `k`).

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::ToRnaValue;
use frap_rna::{Bulk, Field, MathTag, PropDef, RnaError, SetOutcome, StructDef, Value};
use frap_spec_rna::{MESH_LOOP_TRIANGLE, MESH_NORMAL_VALUE, READ_ONLY_INTEGER};

use super::bind::readonly;
use super::bulk;
use super::columns::{self, MATERIAL_INDEX};
use super::soa::{self, SoaKind};
use crate::mesh::{Domain, Mesh, SHARP_FACE};

/// `MeshLoopTriangle` bindings.
pub(crate) static BINDINGS: [BindingDecl; 9] = [
    readonly!("vertices", [i32; 3]),
    readonly!("loops", [i32; 3]),
    readonly!("polygon_index", i32),
    readonly!("normal", [f32; 3]),
    readonly!("split_normals", [f32; 9]),
    readonly!("area", f32),
    readonly!("index", i32),
    readonly!("material_index", i32),
    readonly!("use_smooth", bool),
];

/// `MeshNormalValue` bindings.
pub(crate) static NORMAL_BINDINGS: [BindingDecl; 1] = [readonly!("vector", [f32; 3])];

/// `ReadOnlyInteger` bindings.
pub(crate) static INTEGER_BINDINGS: [BindingDecl; 1] = [readonly!("value", i32)];

/// Read-only properties of these element structs refuse writes like Blender.
fn refuse<K: SoaKind>(
    m: &Mesh,
    ctx: K::Ctx,
    i: usize,
    prop: &PropDef,
    bound: &[BindingDecl],
) -> RnaError {
    if bound.iter().any(|b| b.prop == prop.identifier) {
        soa::read_only::<K>(m, ctx, i, prop)
    } else {
        soa::no_attr::<K>(m, ctx, i, prop)
    }
}

/// `mesh.loop_triangles`.
pub(crate) struct LoopTriangles;

impl SoaKind for LoopTriangles {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &MESH_LOOP_TRIANGLE
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.looptris().len()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let lt = m.looptris()[i];
        let corners = lt.map(|c| c as usize);
        let verts = corners.map(|c| m.corner_verts()[c]);
        let face = || m.looptri_faces()[i] as usize;
        let none = MathTag::None;
        let v = match prop.identifier {
            "vertices" => verts.to_rna_value(prop, none),
            "loops" => lt.map(|c| c as i32).to_rna_value(prop, none),
            "polygon_index" => (face() as i32).to_rna_value(prop, none),
            "normal" => {
                let p = verts.map(|v| columns::position(m, v));
                columns::normal_tri(p[0], p[1], p[2])
                    .to_rna_value(prop, MathTag::for_subtype(prop.subtype))
            }
            "split_normals" => {
                let cn = m.corner_normals();
                let mut rows = [0.0f32; 9];
                for (k, c) in corners.iter().enumerate() {
                    rows[3 * k..3 * k + 3].copy_from_slice(&cn[*c]);
                }
                rows.to_rna_value(prop, MathTag::for_subtype(prop.subtype))
            }
            "area" => {
                let p = verts.map(|v| columns::position(m, v));
                columns::area_tri(p[0], p[1], p[2]).to_rna_value(prop, none)
            }
            "index" => (i as i32).to_rna_value(prop, none),
            "material_index" => {
                columns::ints(m, MATERIAL_INDEX, Domain::Face)[face()].to_rna_value(prop, none)
            }
            "use_smooth" => Value::Bool(!columns::flag(m, SHARP_FACE, Domain::Face, face())),
            _ => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        };
        Ok(Field::Value(v))
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        _v: Value,
    ) -> Result<SetOutcome, RnaError> {
        Err(refuse::<Self>(m, ctx, i, prop, &BINDINGS))
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        let tris = m.looptris();
        bulk::check_range(&range, tris.len())?;
        let cv = m.corner_verts();
        Ok(match prop.identifier {
            "vertices" => Bulk::I32(
                tris[range]
                    .iter()
                    .flat_map(|t| t.map(|c| cv[c as usize]))
                    .collect(),
            ),
            "loops" => Bulk::I32(
                tris[range]
                    .iter()
                    .flat_map(|t| t.map(|c| c as i32))
                    .collect(),
            ),
            "polygon_index" => {
                Bulk::I32(m.looptri_faces()[range].iter().map(|f| *f as i32).collect())
            }
            "index" => bulk::indices(range),
            "material_index" | "use_smooth" => {
                let faces = m.looptri_faces();
                if prop.identifier == "material_index" {
                    let mi = columns::ints(m, MATERIAL_INDEX, Domain::Face);
                    Bulk::I32(faces[range].iter().map(|f| mi[*f as usize]).collect())
                } else {
                    let sharp = columns::flags(m, SHARP_FACE, Domain::Face);
                    Bulk::Bool(faces[range].iter().map(|f| !sharp[*f as usize]).collect())
                }
            }
            _ => return bulk::generic_get::<Self>(m, ctx, prop, range),
        })
    }
}

/// Which cached normal array a `MeshNormalValue` collection reads.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum NormalDomain {
    /// `vertex_normals`
    Vert,
    /// `polygon_normals`
    Face,
    /// `corner_normals`
    Corner,
}

fn normals(m: &Mesh, d: NormalDomain) -> std::sync::Arc<Vec<[f32; 3]>> {
    match d {
        NormalDomain::Vert => m.vert_normals(),
        NormalDomain::Face => m.face_normals(),
        NormalDomain::Corner => m.corner_normals(),
    }
}

/// `vertex_normals` / `polygon_normals` / `corner_normals`.
pub(crate) struct NormalValues;

impl SoaKind for NormalValues {
    type Ctx = NormalDomain;

    fn item_type(_: &Mesh, _: NormalDomain) -> &'static StructDef {
        &MESH_NORMAL_VALUE
    }
    fn len(m: &Mesh, d: NormalDomain) -> usize {
        match d {
            NormalDomain::Vert => m.verts_num(),
            NormalDomain::Face => m.faces_num(),
            NormalDomain::Corner => m.corners_num(),
        }
    }
    fn get<'a>(
        m: &'a Mesh,
        d: NormalDomain,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        match prop.identifier {
            "vector" => Ok(Field::Value(
                normals(m, d)
                    .get(i)
                    .copied()
                    .unwrap_or_default()
                    .to_rna_value(prop, MathTag::for_subtype(prop.subtype)),
            )),
            _ => Err(soa::no_attr::<Self>(m, d, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        d: NormalDomain,
        i: usize,
        prop: &'static PropDef,
        _v: Value,
    ) -> Result<SetOutcome, RnaError> {
        Err(refuse::<Self>(m, d, i, prop, &NORMAL_BINDINGS))
    }
    fn foreach_get(
        m: &Mesh,
        d: NormalDomain,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, Self::len(m, d))?;
        match prop.identifier {
            "vector" => {
                let n = normals(m, d);
                if n.len() < range.end {
                    return bulk::generic_get::<Self>(m, d, prop, range);
                }
                Ok(bulk::f32_rows(&n, range))
            }
            _ => bulk::generic_get::<Self>(m, d, prop, range),
        }
    }
}

/// `loop_triangle_polygons`.
pub(crate) struct LoopTrianglePolygons;

impl SoaKind for LoopTrianglePolygons {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &READ_ONLY_INTEGER
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.looptris().len()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        match prop.identifier {
            "value" => Ok(Field::Value(
                (m.looptri_faces()[i] as i32).to_rna_value(prop, MathTag::None),
            )),
            _ => Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        _v: Value,
    ) -> Result<SetOutcome, RnaError> {
        Err(refuse::<Self>(m, ctx, i, prop, &INTEGER_BINDINGS))
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, Self::len(m, ctx))?;
        match prop.identifier {
            "value" => Ok(Bulk::I32(
                m.looptri_faces()[range].iter().map(|f| *f as i32).collect(),
            )),
            _ => bulk::generic_get::<Self>(m, ctx, prop, range),
        }
    }
}
