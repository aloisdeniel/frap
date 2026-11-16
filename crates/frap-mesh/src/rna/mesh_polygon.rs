// SPDX-License-Identifier: GPL-3.0-or-later
//! `MeshPolygon` (`mesh.polygons[i]`).
//!
//! Columns: face offsets (`loop_start`, `loop_total`), `.corner_vert` (`vertices`),
//! `material_index`, `.select_poly`, `.hide_poly`, `sharp_face` (`use_smooth` is its
//! negation), the cached face normals.
//!
//! `vertices` is a variable-length array (the dump records it as `[3]`): a write must keep the
//! face size (`MeshPolygon.vertices: array length cannot be changed to 3 (expected 4)`, verified),
//! and `foreach_get/set` read and write the concatenated corner vertices (verified on a mesh
//! with a quad, a triangle and a pentagon). A `loop_start` write moves
//! one face offset like Blender, but frap refuses offsets that would decrease, leave the
//! corner range or not start at 0, and vertex indices outside `0..len(vertices)`
//! (`ValueError`; Blender accepts them and lets `Mesh.validate()` repair the mesh). Those
//! writes would leave corners no face uses or dangling indices, which frap's normals and
//! loop triangles do not support.

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, ToRnaValue};
use frap_rna::{Bulk, Field, MathTag, PropDef, RnaError, SetOutcome, StructDef, Value};
use frap_spec_rna::MESH_POLYGON;

use super::bind::{readonly, value};
use super::bulk;
use super::columns::{self, CORNER_EDGE, CORNER_VERT, EDGE_VERTS, HIDE_POLY, MATERIAL_INDEX};
use super::soa::{self, SoaKind};
use crate::mesh::{Domain, Mesh, SELECT_POLY, SHARP_FACE};

/// `MeshPolygon` bindings.
pub(crate) static BINDINGS: [BindingDecl; 11] = [
    value!("vertices", Vec<i32>),
    value!("loop_start", i32),
    readonly!("loop_total", i32),
    value!("material_index", i32),
    value!("select", bool),
    value!("hide", bool),
    value!("use_smooth", bool),
    readonly!("normal", [f32; 3]),
    readonly!("center", [f32; 3]),
    readonly!("area", f32),
    readonly!("index", i32),
];

fn tag(prop: &PropDef) -> MathTag {
    MathTag::for_subtype(prop.subtype)
}

/// Replaces the face offsets, keeping every other column as it is (`Mesh::set_topology` would
/// re-derive `.corner_edge` and create missing topology attributes, so both are restored).
fn write_offsets(m: &mut Mesh, offsets: Vec<i32>) -> Result<(), RnaError> {
    let corners = m.corners_num() as i32;
    // Blender 5.2.4 accepts a first offset above 0 (`polygons[0].loop_start = 2` on two quads
    // gives `loop_total` (2, 4)); frap refuses it like the other offsets it cannot represent.
    if offsets.first().is_some_and(|&o| o != 0)
        || offsets.windows(2).any(|w| w[0] > w[1])
        || offsets.last() != Some(&corners)
    {
        return Err(columns::value_error(format!(
            "frap: MeshPolygon.loop_start: face offsets {offsets:?} would decrease, leave the \
             corner range 0..{corners} or not start at 0, which frap meshes cannot represent"
        )));
    }
    let had: Vec<bool> = [EDGE_VERTS, CORNER_VERT, CORNER_EDGE]
        .iter()
        .map(|n| m.attributes().get(n).is_some())
        .collect();
    let corner_edges = m.corner_edges().to_vec();
    let (cv, edges) = (m.corner_verts().to_vec(), m.edges().to_vec());
    m.set_topology(offsets, cv, edges);
    if let Some(col) = columns::int_col_mut(m, CORNER_EDGE)
        && col.len() == corner_edges.len()
    {
        col.copy_from_slice(&corner_edges);
    }
    for (name, had) in [EDGE_VERTS, CORNER_VERT, CORNER_EDGE].iter().zip(had) {
        if !had {
            m.attributes_mut().remove(name);
        }
    }
    Ok(())
}

/// `mesh.polygons`.
pub(crate) struct Polygons;

impl SoaKind for Polygons {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        &MESH_POLYGON
    }
    fn len(m: &Mesh, _: ()) -> usize {
        m.faces_num()
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let v =
            match prop.identifier {
                "vertices" => columns::face_verts(m, i)
                    .to_vec()
                    .to_rna_value(prop, MathTag::None),
                "loop_start" => m.face_offsets()[i].to_rna_value(prop, MathTag::None),
                "loop_total" => (m.face_offsets()[i + 1] - m.face_offsets()[i])
                    .to_rna_value(prop, MathTag::None),
                "material_index" => columns::ints(m, MATERIAL_INDEX, Domain::Face)[i]
                    .to_rna_value(prop, MathTag::None),
                "select" => Value::Bool(columns::flag(m, SELECT_POLY, Domain::Face, i)),
                "hide" => Value::Bool(columns::flag(m, HIDE_POLY, Domain::Face, i)),
                "use_smooth" => Value::Bool(!columns::flag(m, SHARP_FACE, Domain::Face, i)),
                "normal" => m.face_normals()[i].to_rna_value(prop, tag(prop)),
                "center" => columns::face_center(m, i).to_rna_value(prop, tag(prop)),
                "area" => columns::face_area(m, i).to_rna_value(prop, MathTag::None),
                "index" => (i as i32).to_rna_value(prop, MathTag::None),
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
            "vertices" => {
                let verts = Vec::<i32>::from_rna_value(prop, v)?;
                let r = columns::face_range(m, i);
                if verts.len() != r.len() {
                    return Err(columns::value_error(format!(
                        "MeshPolygon.vertices: array length cannot be changed to {} (expected {})",
                        verts.len(),
                        r.len()
                    )));
                }
                let verts = columns::check_indices(
                    "MeshPolygon.vertices",
                    verts.into_iter().map(i64::from),
                    m.verts_num(),
                    "vertices",
                )?;
                if let Some(col) = columns::int_col_mut(m, CORNER_VERT) {
                    col[r].copy_from_slice(&verts);
                }
            }
            "loop_start" => {
                let mut offs = m.face_offsets().to_vec();
                offs[i] = i32::from_rna_value(prop, v)?;
                write_offsets(m, offs)?;
            }
            "material_index" => {
                columns::ints_mut(m, MATERIAL_INDEX, Domain::Face)[i] =
                    i32::from_rna_value(prop, v)?
            }
            "select" => {
                columns::flags_mut(m, SELECT_POLY, Domain::Face)[i] = bool::from_rna_value(prop, v)?
            }
            "hide" => {
                columns::flags_mut(m, HIDE_POLY, Domain::Face)[i] = bool::from_rna_value(prop, v)?
            }
            "use_smooth" => {
                columns::flags_mut(m, SHARP_FACE, Domain::Face)[i] = !bool::from_rna_value(prop, v)?
            }
            "loop_total" | "normal" | "center" | "area" | "index" => {
                return Err(soa::read_only::<Self>(m, ctx, i, prop));
            }
            _ => return Err(soa::no_attr::<Self>(m, ctx, i, prop)),
        }
        Ok(SetOutcome::Done)
    }
    fn foreach_get(
        m: &Mesh,
        ctx: (),
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, m.faces_num())?;
        let offs = m.face_offsets();
        Ok(match prop.identifier {
            "vertices" => Bulk::I32(
                range
                    .flat_map(|f| columns::face_verts(m, f).iter().copied())
                    .collect(),
            ),
            "loop_start" => bulk::i32_slice(offs, range),
            "loop_total" => Bulk::I32(range.map(|f| offs[f + 1] - offs[f]).collect()),
            "material_index" => {
                bulk::i32_slice(&columns::ints(m, MATERIAL_INDEX, Domain::Face), range)
            }
            "select" => bulk::bool_slice(&columns::flags(m, SELECT_POLY, Domain::Face), range),
            "hide" => bulk::bool_slice(&columns::flags(m, HIDE_POLY, Domain::Face), range),
            "use_smooth" => Bulk::Bool(
                columns::flags(m, SHARP_FACE, Domain::Face)[range]
                    .iter()
                    .map(|s| !s)
                    .collect(),
            ),
            "normal" => bulk::f32_rows(&m.face_normals(), range),
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
        let n = m.faces_num();
        match prop.identifier {
            "vertices" => {
                let corners = (0..n).map(|f| columns::face_range(m, f).len()).sum();
                let data = &bulk::check_set(prop, data, corners, 1)?;
                let verts = columns::check_indices(
                    "MeshPolygon.vertices",
                    bulk::raw_i64s(data),
                    m.verts_num(),
                    "vertices",
                )?;
                if let Some(col) = columns::int_col_mut(m, CORNER_VERT)
                    && col.len() == verts.len()
                {
                    col.copy_from_slice(&verts);
                }
            }
            "loop_start" => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let mut offs: Vec<i32> = bulk::raw_i64s(data)
                    .into_iter()
                    .map(|o| o.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
                    .collect();
                offs.push(m.corners_num() as i32);
                write_offsets(m, offs)?;
            }
            "material_index" => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                *columns::ints_mut(m, MATERIAL_INDEX, Domain::Face) = bulk::i32s(prop, data);
            }
            "select" | "hide" | "use_smooth" => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let mut v = bulk::bools(data);
                let name = match prop.identifier {
                    "select" => SELECT_POLY,
                    "hide" => HIDE_POLY,
                    _ => {
                        v.iter_mut().for_each(|b| *b = !*b);
                        SHARP_FACE
                    }
                };
                *columns::flags_mut(m, name, Domain::Face) = v;
            }
            _ => return bulk::generic_set::<Self>(m, ctx, prop, data),
        }
        Ok(())
    }
}
