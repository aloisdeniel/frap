// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshLoop` (`mesh.loops[i]`, face corners).
//!
//! Columns: `.corner_vert` (`vertex_index`), `.corner_edge` (`edge_index`), the cached corner
//! normals. `tangent`, `bitangent` and `bitangent_sign` read zero until `Mesh.calc_tangents()`
//! has run (verified on 5.2.4); frap has no tangent layer yet, so they always read zero.

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{Bulk, Field, MathTag, PropDef, RnaError, SetOutcome, StructDef};
use frap_spec_rna::MESH_LOOP;

use super::bind::{readonly, value};
use super::bulk;
use super::columns::{self, CORNER_EDGE, CORNER_VERT};
use super::soa::{self, SoaKind};
use crate::mesh::Mesh;

/// `MeshLoop` bindings.
pub(crate) static BINDINGS: [BindingDecl; 7] = [
    value!("vertex_index", i32),
    value!("edge_index", i32),
    readonly!("index", i32),
    readonly!("normal", [f32; 3]),
    readonly!("tangent", [f32; 3]),
    readonly!("bitangent_sign", f32),
    readonly!("bitangent", [f32; 3]),
];

/// The column behind an index property.
fn index_column(prop: &str) -> Option<&'static str> {
    match prop {
        "vertex_index" => Some(CORNER_VERT),
        "edge_index" => Some(CORNER_EDGE),
        _ => None,
    }
}

/// `mesh.loops`.
pub(crate) struct Loops;

impl SoaKind for Loops {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &MESH_LOOP
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.corners_num()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let none = MathTag::None;
        let v = match prop.identifier {
            "vertex_index" => m.corner_verts()[i].to_rna_value(prop, none),
            "edge_index" => m
                .corner_edges()
                .get(i)
                .copied()
                .unwrap_or(0)
                .to_rna_value(prop, none),
            "index" => (i as i32).to_rna_value(prop, none),
            "normal" => {
                m.corner_normals()[i].to_rna_value(prop, MathTag::for_subtype(prop.subtype))
            }
            "tangent" | "bitangent" => {
                [0.0f32; 3].to_rna_value(prop, MathTag::for_subtype(prop.subtype))
            }
            "bitangent_sign" => 0.0f32.to_rna_value(prop, none),
            _ => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        };
        Ok(Field::Value(v))
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        v: frap_rna::Value,
    ) -> Result<SetOutcome, RnaError> {
        match index_column(prop.identifier) {
            Some(name) => {
                let x = i32::from_rna_value(prop, v)?;
                if let Some(col) = columns::int_col_mut(m, name)
                    && let Some(c) = col.get_mut(i)
                {
                    *c = x;
                }
                Ok(SetOutcome::Done)
            }
            None if is_bound(prop) => Err(soa::read_only::<Self>(m, ctx, i, prop)),
            None => Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, m.corners_num())?;
        Ok(match prop.identifier {
            "vertex_index" => bulk::i32_slice(m.corner_verts(), range),
            "edge_index" if m.corner_edges().len() == m.corners_num() => {
                bulk::i32_slice(m.corner_edges(), range)
            }
            "normal" => bulk::f32_rows(&m.corner_normals(), range),
            "index" => bulk::indices(range),
            _ => return bulk::generic_get::<Self>(m, ctx, prop, range),
        })
    }
    fn foreach_set(
        m: &mut Mesh,
        ctx: (),
        prop: &'static PropDef,
        data: &frap_rna::Bulk,
    ) -> Result<(), RnaError> {
        let n = m.corners_num();
        match index_column(prop.identifier) {
            Some(name) => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let v = bulk::i32s(prop, data);
                if let Some(col) = columns::int_col_mut(m, name)
                    && col.len() == v.len()
                {
                    col.copy_from_slice(&v);
                }
                Ok(())
            }
            None => bulk::generic_set::<Self>(m, ctx, prop, data),
        }
    }
}

/// True for the properties `MeshLoop` binds (read-only ones included).
fn is_bound(prop: &PropDef) -> bool {
    BINDINGS.iter().any(|b| b.prop == prop.identifier)
}
