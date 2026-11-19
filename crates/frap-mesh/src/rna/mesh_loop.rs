// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshLoop` (`mesh.loops[i]`, face corners).
//!
//! Columns: `.corner_vert` (`vertex_index`), `.corner_edge` (`edge_index`), the cached corner
//! normals. `tangent`, `bitangent` and `bitangent_sign` read the temporary tangent layer of
//! `Mesh.calc_tangents()` (`rna_MeshLoop_tangent_get`, `rna_MeshLoop_bitangent_get`:
//! `cross(corner normal, tangent) * sign`), zero without one (verified on 5.2.4).
//! `vertex_index`/`edge_index` refuse indices outside the vertex/edge range (`ValueError`,
//! frap deviation; Blender stores them for `Mesh.validate()` to repair).

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

/// The column behind an index property, the property's name for errors, the indexed domain's
/// size and its name.
fn index_column(m: &Mesh, prop: &str) -> Option<(&'static str, &'static str, usize, &'static str)> {
    match prop {
        "vertex_index" => Some((
            CORNER_VERT,
            "MeshLoop.vertex_index",
            m.verts_num(),
            "vertices",
        )),
        "edge_index" => Some((CORNER_EDGE, "MeshLoop.edge_index", m.edges_num(), "edges")),
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
                let v = match m.corner_tangents().map(|t| t[i]) {
                    None => [0.0f32; 3],
                    Some([x, y, z, _]) if prop.identifier == "tangent" => [x, y, z],
                    Some([x, y, z, w]) => {
                        let n = m.corner_normals()[i];
                        [
                            (n[1] * z - n[2] * y) * w,
                            (n[2] * x - n[0] * z) * w,
                            (n[0] * y - n[1] * x) * w,
                        ]
                    }
                };
                v.to_rna_value(prop, MathTag::for_subtype(prop.subtype))
            }
            "bitangent_sign" => m
                .corner_tangents()
                .map_or(0.0f32, |t| t[i][3])
                .to_rna_value(prop, none),
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
        match index_column(m, prop.identifier) {
            Some((name, what, len, of)) => {
                let x = i32::from_rna_value(prop, v)?;
                let x = columns::check_indices(what, [i64::from(x)], len, of)?[0];
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
        match index_column(m, prop.identifier) {
            Some((name, what, len, of)) => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let v = columns::check_indices(what, bulk::raw_i64s(data), len, of)?;
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
