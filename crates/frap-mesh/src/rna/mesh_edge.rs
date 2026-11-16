// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshEdge` (`mesh.edges[i]`).
//!
//! Columns: `.edge_verts`, `.select_edge`, `.hide_edge`, `uv_seam` (`use_seam`), `sharp_edge`
//! (`use_edge_sharp`). `is_loose` is true for an edge no face corner uses
//! (`Mesh::loose_edges`). `vertices` refuses indices outside the vertex range (`ValueError`,
//! frap deviation; Blender stores them for `Mesh.validate()` to repair).

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{Bulk, Field, MathTag, PropDef, RnaError, SetOutcome, StructDef, Value};
use frap_spec_rna::MESH_EDGE;

use super::bind::{readonly, value};
use super::bulk;
use super::columns::{self, HIDE_EDGE};
use super::soa::{self, SoaKind};
use crate::mesh::{Domain, Mesh, SELECT_EDGE, SHARP_EDGE, UV_SEAM};

/// `MeshEdge` bindings.
pub(crate) static BINDINGS: [BindingDecl; 7] = [
    value!("vertices", [i32; 2]),
    value!("select", bool),
    value!("hide", bool),
    value!("use_seam", bool),
    value!("use_edge_sharp", bool),
    readonly!("is_loose", bool),
    readonly!("index", i32),
];

/// The boolean column behind a flag property.
fn flag_column(prop: &str) -> Option<&'static str> {
    Some(match prop {
        "select" => SELECT_EDGE,
        "hide" => HIDE_EDGE,
        "use_seam" => UV_SEAM,
        "use_edge_sharp" => SHARP_EDGE,
        _ => return None,
    })
}

/// Per edge: true when no face corner uses it.
fn loose_edges(m: &Mesh) -> Vec<bool> {
    let mut loose = vec![true; m.edges_num()];
    for &e in m.corner_edges() {
        if let Some(l) = usize::try_from(e).ok().and_then(|e| loose.get_mut(e)) {
            *l = false;
        }
    }
    loose
}

/// `mesh.edges`.
pub(crate) struct Edges;

impl SoaKind for Edges {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &MESH_EDGE
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.edges_num()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let v = match prop.identifier {
            "vertices" => m.edges()[i].to_rna_value(prop, MathTag::None),
            "is_loose" => Value::Bool(!m.corner_edges().contains(&(i as i32))),
            "index" => (i as i32).to_rna_value(prop, MathTag::None),
            p => match flag_column(p) {
                Some(name) => Value::Bool(columns::flag(m, name, Domain::Edge, i)),
                None => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
            },
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
            "vertices" => {
                let e = <[i32; 2]>::from_rna_value(prop, v)?;
                let e = columns::check_indices(
                    "MeshEdge.vertices",
                    e.map(i64::from),
                    m.verts_num(),
                    "vertices",
                )?;
                let e = [e[0], e[1]];
                if let Some(col) = columns::edges_mut(m) {
                    col[i] = e;
                }
            }
            "is_loose" | "index" => return Err(soa::read_only::<Self>(m, ctx, i, prop)),
            p => match flag_column(p) {
                Some(name) => {
                    columns::flags_mut(m, name, Domain::Edge)[i] = bool::from_rna_value(prop, v)?
                }
                None => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
            },
        }
        Ok(SetOutcome::Done)
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, m.edges_num())?;
        Ok(match prop.identifier {
            "vertices" => bulk::i32_rows(m.edges(), range),
            "is_loose" => bulk::bool_slice(&loose_edges(m), range),
            "index" => bulk::indices(range),
            p => match flag_column(p) {
                Some(name) => bulk::bool_slice(&columns::flags(m, name, Domain::Edge), range),
                None => return bulk::generic_get::<Self>(m, ctx, prop, range),
            },
        })
    }
    fn foreach_set(
        m: &mut Mesh,
        ctx: (),
        prop: &'static PropDef,
        data: &Bulk,
    ) -> Result<(), RnaError> {
        let n = m.edges_num();
        match (prop.identifier, flag_column(prop.identifier)) {
            ("vertices", _) => {
                let data = &bulk::check_set(prop, data, n, 2)?;
                let rows = bulk::rows::<i32, 2>(&columns::check_indices(
                    "MeshEdge.vertices",
                    bulk::raw_i64s(data),
                    m.verts_num(),
                    "vertices",
                )?);
                if let Some(col) = columns::edges_mut(m) {
                    col.copy_from_slice(&rows);
                }
            }
            (_, Some(name)) => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                *columns::flags_mut(m, name, Domain::Edge) = bulk::bools(data);
            }
            _ => return bulk::generic_set::<Self>(m, ctx, prop, data),
        }
        Ok(())
    }
}
