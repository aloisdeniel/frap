// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshVertex` (`mesh.vertices[i]`) and `VertexGroupElement` (`vertices[i].groups[j]`).
//!
//! Columns: `position` (`co`), `.select_vert`, `.hide_vert`, the cached vertex normals and the
//! `MDeformVert` weights. `undeformed_co` equals `co` on an original mesh (verified: Blender
//! reads `CD_ORCO`, which only evaluated meshes carry).

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{
    Bulk, Field, MathTag, PropDef, RnaCollMutGuard, RnaError, SetOutcome, StructDef, Value,
};
use frap_spec_rna::{MESH_VERTEX, VERTEX_GROUP_ELEMENT};

use super::bind::{readonly, soa, value};
use super::bulk;
use super::columns::{self, HIDE_VERT};
use super::soa::{self, SoaKind};
use crate::mesh::{Domain, Mesh, SELECT_VERT};

/// `MeshVertex` bindings.
pub(crate) static BINDINGS: [BindingDecl; 7] = [
    value!("co", [f32; 3]),
    readonly!("normal", [f32; 3]),
    value!("select", bool),
    value!("hide", bool),
    soa!("groups", VERTEX_GROUP_ELEMENT, &GROUP_BINDINGS),
    readonly!("index", i32),
    readonly!("undeformed_co", [f32; 3]),
];

/// `VertexGroupElement` bindings.
static GROUP_BINDINGS: [BindingDecl; 2] = [readonly!("group", i32), value!("weight", f32)];

fn tag(prop: &PropDef) -> MathTag {
    MathTag::for_subtype(prop.subtype)
}

/// `mesh.vertices`.
pub(crate) struct Vertices;

impl SoaKind for Vertices {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &MESH_VERTEX
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.verts_num()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let v = match prop.identifier {
            "co" | "undeformed_co" => m.positions()[i].to_rna_value(prop, tag(prop)),
            "normal" => m.vert_normals()[i].to_rna_value(prop, tag(prop)),
            "select" => Value::Bool(columns::flag(m, SELECT_VERT, Domain::Point, i)),
            "hide" => Value::Bool(columns::flag(m, HIDE_VERT, Domain::Point, i)),
            "index" => (i as i32).to_rna_value(prop, MathTag::None),
            "groups" => return Ok(soa::coll_field::<VertGroups>(m, i)),
            _ => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        };
        Ok(Field::Value(v))
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "co" => m.positions_mut()[i] = <[f32; 3]>::from_rna_value(prop, v)?,
            "select" => {
                columns::flags_mut(m, SELECT_VERT, Domain::Point)[i] =
                    bool::from_rna_value(prop, v)?
            }
            "hide" => {
                columns::flags_mut(m, HIDE_VERT, Domain::Point)[i] = bool::from_rna_value(prop, v)?
            }
            "normal" | "index" | "undeformed_co" | "groups" => {
                return Err(soa::read_only::<Self>(m, ctx, i, prop));
            }
            _ => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        }
        Ok(SetOutcome::Done)
    }
    fn coll_mut<'a>(
        m: &'a mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<RnaCollMutGuard<'a>, RnaError> {
        match prop.identifier {
            "groups" => Ok(soa::coll_mut_guard::<VertGroups>(m, i)),
            _ => Err(frap_rna::traits::wrong_access(
                Self::item_srna(m, ctx, i),
                prop.identifier,
                "a collection",
            )),
        }
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, m.verts_num())?;
        Ok(match prop.identifier {
            "co" | "undeformed_co" => bulk::f32_rows(m.positions(), range),
            "normal" => bulk::f32_rows(&m.vert_normals(), range),
            "select" => bulk::bool_slice(&columns::flags(m, SELECT_VERT, Domain::Point), range),
            "hide" => bulk::bool_slice(&columns::flags(m, HIDE_VERT, Domain::Point), range),
            "index" => bulk::indices(range),
            _ => return bulk::generic_get::<Self>(m, ctx, prop, range),
        })
    }
    fn foreach_set(
        m: &mut Mesh,
        ctx: (),
        prop: &'static PropDef,
        data: &Bulk,
    ) -> Result<(), RnaError> {
        let n = m.verts_num();
        match prop.identifier {
            "co" => {
                let data = &bulk::check_set(prop, data, n, 3)?;
                let rows = bulk::rows::<f32, 3>(&bulk::f32s(prop, data));
                m.positions_mut().copy_from_slice(&rows);
            }
            "select" | "hide" => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let name = if prop.identifier == "select" {
                    SELECT_VERT
                } else {
                    HIDE_VERT
                };
                *columns::flags_mut(m, name, Domain::Point) = bulk::bools(data);
            }
            _ => return bulk::generic_set::<Self>(m, ctx, prop, data),
        }
        Ok(())
    }
}

/// `vertices[v].groups`: the vertex's `MDeformVert` entries (context: the vertex).
pub(crate) struct VertGroups;

impl SoaKind for VertGroups {
    type Ctx = usize;

    fn item_type(_: &Mesh, _: usize) -> &'static StructDef {
        &VERTEX_GROUP_ELEMENT
    }
    fn len(m: &Mesh, v: usize) -> usize {
        m.deform_verts()
            .and_then(|d| d.get(v))
            .map_or(0, |d| d.weights.len())
    }
    fn get<'a>(
        m: &'a Mesh,
        v: usize,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let (group, weight) = m
            .deform_verts()
            .and_then(|d| d.get(v))
            .and_then(|d| d.weights.get(i))
            .copied()
            .unwrap_or_default();
        Ok(Field::Value(match prop.identifier {
            "group" => (group as i32).to_rna_value(prop, MathTag::None),
            "weight" => weight.to_rna_value(prop, MathTag::None),
            _ => return Err(soa::no_attr::<Self>(m, v, i, prop)),
        }))
    }
    fn set(
        m: &mut Mesh,
        v: usize,
        i: usize,
        prop: &'static PropDef,
        val: Value,
    ) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "weight" => {
                let w = f32::from_rna_value(prop, val)?;
                if let Some(e) = m
                    .deform_verts_mut()
                    .get_mut(v)
                    .and_then(|d| d.weights.get_mut(i))
                {
                    e.1 = w;
                }
                Ok(SetOutcome::Done)
            }
            "group" => Err(soa::read_only::<Self>(m, v, i, prop)),
            _ => Err(soa::no_attr::<Self>(m, v, i, prop)),
        }
    }
}
