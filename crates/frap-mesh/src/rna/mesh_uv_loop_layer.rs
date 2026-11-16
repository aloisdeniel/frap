// SPDX-License-Identifier: GPL-3.0-or-later
//! UV maps and legacy vertex colours as layer collections:
//! - `Mesh.uv_layers` (`MeshUVLoopLayer`): every FLOAT2 attribute on CORNER (internal names
//!   included, verified), with `data[i]` (`MeshUVLoop.uv/pin_uv`), `uv[i]`
//!   (`Float2AttributeValue`) and `pin[i]` (`BoolAttributeValue` over `.pn.<name>`, empty while
//!   that layer does not exist).
//! - `Mesh.vertex_colors` (`MeshLoopColorLayer`): every BYTE_COLOR attribute on CORNER, with
//!   `data[i].color` = the stored sRGB bytes / 255 (no colour conversion, verified).
//!
//! `active`/`active_render` compare the layer name with the mesh's active names; writing
//! `True` makes the layer active, writing `False` changes nothing (verified). Writing
//! `pin_uv` creates `.pn.<name>`. `MeshUVLoopLayer.active_clone` is not bound: frap's `Mesh`
//! has no clone-layer field.
//!
//! Blender 5.2.4 returns `Float2AttributeValue` items from `MeshUVLoopLayer.pin` (they read the
//! UV coordinates, an upstream bug); frap returns the documented `BoolAttributeValue` items.

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, SmolStr, ToRnaValue};
use frap_rna::{
    Bulk, Field, MathTag, PropDef, RnaCollMutGuard, RnaError, SetOutcome, StructDef, Value,
};
use frap_spec_rna as spec;

use super::attribute::{nth, nth_mut, pin_name, positions_where, rename};
use super::attribute_value::AttrValues;
use super::bind::{soa, value};
use super::bulk;
use super::color;
use super::columns;
use super::soa::{self as soa_mod, SoaKind};
use crate::mesh::{AttrData, Attribute, Domain, Mesh};

/// `MeshUVLoopLayer` bindings.
pub(crate) static UV_LAYER_BINDINGS: [BindingDecl; 6] = [
    soa!("data", spec::MESH_UV_LOOP, &UV_LOOP_BINDINGS),
    value!("name", SmolStr),
    value!("active", bool),
    value!("active_render", bool),
    soa!(
        "uv",
        spec::FLOAT2_ATTRIBUTE_VALUE,
        &[value!("vector", [f32; 2])]
    ),
    soa!("pin", spec::BOOL_ATTRIBUTE_VALUE, &[value!("value", bool)]),
];

/// `MeshUVLoop` bindings.
static UV_LOOP_BINDINGS: [BindingDecl; 2] = [value!("uv", [f32; 2]), value!("pin_uv", bool)];

/// `MeshLoopColorLayer` bindings.
pub(crate) static COLOR_LAYER_BINDINGS: [BindingDecl; 4] = [
    value!("name", SmolStr),
    value!("active", bool),
    value!("active_render", bool),
    soa!("data", spec::MESH_LOOP_COLOR, &[value!("color", [f32; 4])]),
];

fn is_uv(a: &Attribute) -> bool {
    a.domain == Domain::Corner && matches!(a.data, AttrData::Float2(_))
}

fn is_loop_color(a: &Attribute) -> bool {
    a.domain == Domain::Corner && matches!(a.data, AttrData::ByteColor(_))
}

/// Shared layer behaviour of `uv_layers` and `vertex_colors`.
trait LayerKind {
    const SRNA: &'static StructDef;
    fn keeps(a: &Attribute) -> bool;
    fn active(m: &mut Mesh) -> &mut Option<SmolStr>;
    fn active_render(m: &mut Mesh) -> &mut Option<SmolStr>;
    fn active_ref(m: &Mesh) -> Option<&str>;
    fn active_render_ref(m: &Mesh) -> Option<&str>;
}

/// Attribute-set position of layer `i`.
fn layer_pos<L: LayerKind>(m: &Mesh, i: usize) -> usize {
    // `usize::MAX` (no attribute) when a rename through this proxy removed a layer before it.
    positions_where(m, L::keeps)
        .get(i)
        .copied()
        .unwrap_or(usize::MAX)
}

fn layer_len<L: LayerKind>(m: &Mesh) -> usize {
    m.attributes().iter().filter(|a| L::keeps(a)).count()
}

fn layer_key<L: LayerKind>(m: &Mesh, i: usize) -> Option<SmolStr> {
    m.attributes()
        .iter()
        .filter(|a| L::keeps(a))
        .nth(i)
        .map(|a| a.name.clone())
}

fn layer_find<L: LayerKind>(m: &Mesh, key: &str) -> Option<usize> {
    m.attributes()
        .iter()
        .filter(|a| L::keeps(a))
        .position(|a| a.name == key)
}

/// Reads the layer properties both layer structs share; `None` for the others.
fn layer_get<L: LayerKind>(m: &Mesh, pos: usize, prop: &PropDef) -> Option<Value> {
    let name = nth(m, pos)?.name.clone();
    Some(match prop.identifier {
        "name" => Value::Str(name),
        "active" => Value::Bool(L::active_ref(m) == Some(name.as_str())),
        "active_render" => Value::Bool(L::active_render_ref(m) == Some(name.as_str())),
        _ => return None,
    })
}

/// Writes the shared layer properties; `Ok(false)` for the others.
fn layer_set<L: LayerKind>(
    m: &mut Mesh,
    pos: usize,
    prop: &'static PropDef,
    v: Value,
) -> Result<bool, RnaError> {
    let Some(name) = nth(m, pos).map(|a| a.name.clone()) else {
        return Ok(false);
    };
    match prop.identifier {
        "name" => rename(m, pos, SmolStr::from_rna_value(prop, v)?),
        "active" => {
            if bool::from_rna_value(prop, v)? {
                *L::active(m) = Some(name);
            }
        }
        "active_render" => {
            if bool::from_rna_value(prop, v)? {
                *L::active_render(m) = Some(name);
            }
        }
        _ => return Ok(false),
    }
    Ok(true)
}

/// UV maps.
pub(crate) struct UvLayers;

impl LayerKind for UvLayers {
    const SRNA: &'static StructDef = &spec::MESH_UV_LOOP_LAYER;
    fn keeps(a: &Attribute) -> bool {
        is_uv(a)
    }
    fn active(m: &mut Mesh) -> &mut Option<SmolStr> {
        &mut m.uv_active
    }
    fn active_render(m: &mut Mesh) -> &mut Option<SmolStr> {
        &mut m.uv_active_render
    }
    fn active_ref(m: &Mesh) -> Option<&str> {
        m.uv_active.as_deref()
    }
    fn active_render_ref(m: &Mesh) -> Option<&str> {
        m.uv_active_render.as_deref()
    }
}

impl SoaKind for UvLayers {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        <Self as LayerKind>::SRNA
    }
    fn len(m: &Mesh, _: ()) -> usize {
        layer_len::<Self>(m)
    }
    fn key_of(m: &Mesh, _: (), i: usize) -> Option<SmolStr> {
        layer_key::<Self>(m, i)
    }
    fn find(m: &Mesh, _: (), key: &str) -> Option<usize> {
        layer_find::<Self>(m, key)
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" => Ok(soa_mod::coll_field::<UvLoops>(m, pos)),
            "uv" => Ok(soa_mod::coll_field::<AttrValues>(m, pos)),
            "pin" => Ok(soa_mod::coll_field::<UvPins>(m, pos)),
            _ => layer_get::<Self>(m, pos, prop)
                .map(Field::Value)
                .ok_or_else(|| soa_mod::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" | "uv" | "pin" => Err(soa_mod::read_only::<Self>(m, ctx, i, prop)),
            _ if layer_set::<Self>(m, pos, prop, v)? => Ok(SetOutcome::Done),
            _ => Err(soa_mod::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn coll_mut<'a>(
        m: &'a mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<RnaCollMutGuard<'a>, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" => Ok(soa_mod::coll_mut_guard::<UvLoops>(m, pos)),
            "uv" => Ok(soa_mod::coll_mut_guard::<AttrValues>(m, pos)),
            "pin" => Ok(soa_mod::coll_mut_guard::<UvPins>(m, pos)),
            _ => Err(frap_rna::traits::wrong_access(
                Self::item_srna(m, ctx, i),
                prop.identifier,
                "a collection",
            )),
        }
    }
}

/// The UV column at `pos`.
fn uv_col(m: &Mesh, pos: usize) -> &[[f32; 2]] {
    match nth(m, pos).map(|a| &a.data) {
        Some(AttrData::Float2(v)) => v,
        _ => &[],
    }
}

/// The pin layer name of the UV map at `pos`.
fn pin_of(m: &Mesh, pos: usize) -> SmolStr {
    pin_name(nth(m, pos).map_or("", |a| a.name.as_str()))
}

/// `uv_layers[n].data` (`MeshUVLoop`; context: the UV map's position).
pub(crate) struct UvLoops;

impl SoaKind for UvLoops {
    type Ctx = usize;

    fn item_type(_: &Mesh, _: usize) -> &'static StructDef {
        &spec::MESH_UV_LOOP
    }
    fn len(m: &Mesh, pos: usize) -> usize {
        uv_col(m, pos).len()
    }
    fn get<'a>(
        m: &'a Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        Ok(Field::Value(match prop.identifier {
            "uv" => uv_col(m, pos)[i].to_rna_value(prop, MathTag::for_subtype(prop.subtype)),
            "pin_uv" => Value::Bool(columns::flag(m, &pin_of(m, pos), Domain::Corner, i)),
            _ => return Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }))
    }
    fn set(
        m: &mut Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "uv" => {
                let uv = <[f32; 2]>::from_rna_value(prop, v)?;
                if let Some(AttrData::Float2(c)) = nth_mut(m, pos).map(|a| &mut a.data) {
                    std::sync::Arc::make_mut(c)[i] = uv;
                }
            }
            "pin_uv" => {
                let pin = pin_of(m, pos);
                columns::flags_mut(m, &pin, Domain::Corner)[i] = bool::from_rna_value(prop, v)?;
            }
            _ => return Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }
        Ok(SetOutcome::Done)
    }
    fn foreach_get(
        m: &Mesh,
        pos: usize,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, Self::len(m, pos))?;
        Ok(match prop.identifier {
            "uv" => bulk::f32_rows(uv_col(m, pos), range),
            "pin_uv" => {
                bulk::bool_slice(&columns::flags(m, &pin_of(m, pos), Domain::Corner), range)
            }
            _ => return bulk::generic_get::<Self>(m, pos, prop, range),
        })
    }
    fn foreach_set(
        m: &mut Mesh,
        pos: usize,
        prop: &'static PropDef,
        data: &Bulk,
    ) -> Result<(), RnaError> {
        let n = Self::len(m, pos);
        match prop.identifier {
            "uv" => {
                let data = &bulk::check_set(prop, data, n, 2)?;
                let rows = bulk::rows::<f32, 2>(&bulk::f32s(prop, data));
                if let Some(AttrData::Float2(c)) = nth_mut(m, pos).map(|a| &mut a.data) {
                    *std::sync::Arc::make_mut(c) = rows;
                }
                Ok(())
            }
            "pin_uv" => {
                let data = &bulk::check_set(prop, data, n, 1)?;
                let pin = pin_of(m, pos);
                *columns::flags_mut(m, &pin, Domain::Corner) = bulk::bools(data);
                Ok(())
            }
            _ => bulk::generic_set::<Self>(m, pos, prop, data),
        }
    }
}

/// `uv_layers[n].pin` (`BoolAttributeValue` over `.pn.<name>`; context: the UV map's position).
pub(crate) struct UvPins;

impl UvPins {
    fn exists(m: &Mesh, pos: usize) -> bool {
        m.attributes().get(&pin_of(m, pos)).is_some()
    }
}

impl SoaKind for UvPins {
    type Ctx = usize;

    fn item_type(_: &Mesh, _: usize) -> &'static StructDef {
        &spec::BOOL_ATTRIBUTE_VALUE
    }
    fn len(m: &Mesh, pos: usize) -> usize {
        if Self::exists(m, pos) {
            m.corners_num()
        } else {
            0
        }
    }
    fn get<'a>(
        m: &'a Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        match prop.identifier {
            "value" => Ok(Field::Value(Value::Bool(columns::flag(
                m,
                &pin_of(m, pos),
                Domain::Corner,
                i,
            )))),
            _ => Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "value" => {
                let pin = pin_of(m, pos);
                columns::flags_mut(m, &pin, Domain::Corner)[i] = bool::from_rna_value(prop, v)?;
                Ok(SetOutcome::Done)
            }
            _ => Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }
    }
}

/// Legacy vertex colours.
pub(crate) struct LoopColorLayers;

impl LayerKind for LoopColorLayers {
    const SRNA: &'static StructDef = &spec::MESH_LOOP_COLOR_LAYER;
    fn keeps(a: &Attribute) -> bool {
        is_loop_color(a)
    }
    fn active(m: &mut Mesh) -> &mut Option<SmolStr> {
        &mut m.color_active
    }
    fn active_render(m: &mut Mesh) -> &mut Option<SmolStr> {
        &mut m.color_default
    }
    fn active_ref(m: &Mesh) -> Option<&str> {
        m.color_active.as_deref()
    }
    fn active_render_ref(m: &Mesh) -> Option<&str> {
        m.color_default.as_deref()
    }
}

impl SoaKind for LoopColorLayers {
    type Ctx = ();

    fn item_type(_: &Mesh, _: ()) -> &'static StructDef {
        <Self as LayerKind>::SRNA
    }
    fn len(m: &Mesh, _: ()) -> usize {
        layer_len::<Self>(m)
    }
    fn key_of(m: &Mesh, _: (), i: usize) -> Option<SmolStr> {
        layer_key::<Self>(m, i)
    }
    fn find(m: &Mesh, _: (), key: &str) -> Option<usize> {
        layer_find::<Self>(m, key)
    }
    fn get<'a>(
        m: &'a Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" => Ok(soa_mod::coll_field::<LoopColors>(m, pos)),
            _ => layer_get::<Self>(m, pos, prop)
                .map(Field::Value)
                .ok_or_else(|| soa_mod::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" => Err(soa_mod::read_only::<Self>(m, ctx, i, prop)),
            _ if layer_set::<Self>(m, pos, prop, v)? => Ok(SetOutcome::Done),
            _ => Err(soa_mod::no_attr::<Self>(m, ctx, i, prop)),
        }
    }
    fn coll_mut<'a>(
        m: &'a mut Mesh,
        ctx: (),
        i: usize,
        prop: &'static PropDef,
    ) -> Result<RnaCollMutGuard<'a>, RnaError> {
        let pos = layer_pos::<Self>(m, i);
        match prop.identifier {
            "data" => Ok(soa_mod::coll_mut_guard::<LoopColors>(m, pos)),
            _ => Err(frap_rna::traits::wrong_access(
                Self::item_srna(m, ctx, i),
                prop.identifier,
                "a collection",
            )),
        }
    }
}

/// The byte colour column at `pos`.
fn byte_col(m: &Mesh, pos: usize) -> &[[u8; 4]] {
    match nth(m, pos).map(|a| &a.data) {
        Some(AttrData::ByteColor(v)) => v,
        _ => &[],
    }
}

/// `vertex_colors[n].data` (`MeshLoopColor`; context: the layer's position).
pub(crate) struct LoopColors;

impl SoaKind for LoopColors {
    type Ctx = usize;

    fn item_type(_: &Mesh, _: usize) -> &'static StructDef {
        &spec::MESH_LOOP_COLOR
    }
    fn len(m: &Mesh, pos: usize) -> usize {
        byte_col(m, pos).len()
    }
    fn get<'a>(
        m: &'a Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        match prop.identifier {
            "color" => Ok(Field::Value(
                color::byte_to_float(byte_col(m, pos)[i])
                    .to_rna_value(prop, MathTag::for_subtype(prop.subtype)),
            )),
            _ => Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }
    }
    fn set(
        m: &mut Mesh,
        pos: usize,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        match prop.identifier {
            "color" => {
                let c = color::float_to_byte(<[f32; 4]>::from_rna_value(prop, v)?);
                if let Some(AttrData::ByteColor(col)) = nth_mut(m, pos).map(|a| &mut a.data) {
                    std::sync::Arc::make_mut(col)[i] = c;
                }
                Ok(SetOutcome::Done)
            }
            _ => Err(soa_mod::no_attr::<Self>(m, pos, i, prop)),
        }
    }
    fn foreach_get(
        m: &Mesh,
        pos: usize,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::check_range(&range, Self::len(m, pos))?;
        match prop.identifier {
            "color" => Ok(Bulk::F32(
                byte_col(m, pos)[range]
                    .iter()
                    .flat_map(|c| color::byte_to_float(*c))
                    .collect(),
            )),
            _ => bulk::generic_get::<Self>(m, pos, prop, range),
        }
    }
}
