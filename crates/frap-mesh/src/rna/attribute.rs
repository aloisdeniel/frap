// SPDX-License-Identifier: GPL-3.0-or-later
//! Attribute layers: `Mesh.attributes` / `Mesh.color_attributes` (`Attribute`, refined to
//! `FloatAttribute`, `ByteColorAttribute`, …), and the renaming rules shared with
//! `MeshUVLoopLayer.name` and `MeshLoopColorLayer.name`.
//!
//! Verified on Blender 5.2.4:
//! - `attributes` lists every attribute in creation order, internal (`.`-prefixed) ones
//!   included; `color_attributes` keeps the FLOAT_COLOR/BYTE_COLOR ones on POINT/CORNER
//!   (internal ones included); `find`/`[i]` index the filtered list.
//! - `storage_type` is `'SINGLE'` for attributes Blender stores as one shared value (a
//!   primitive's `.select_*`, `sharp_face`) and `'ARRAY'` otherwise. frap has no single-value
//!   storage and always reports `'ARRAY'`.
//! - `is_required`: `position`, `.edge_verts`, `.corner_vert`, `.corner_edge`; their `name` is
//!   read-only (`AttributeError`).
//! - Renaming to an existing name **replaces** that attribute; renaming to `''`, or to a
//!   built-in name (`sharp_face`, `material_index`, …) with another domain or type, reports an
//!   ERROR and changes nothing (no exception); renaming a UV map renames its `.pn.<name>` pin
//!   layer; the active UV/colour names follow the rename.

use std::ops::Range;

use frap_rna::testing::BindingDecl;
use frap_rna::traits::{FromRnaValue, SmolStr};
use frap_rna::{Bulk, Field, PropDef, RnaCollMutGuard, RnaError, SetOutcome, StructDef, Value};
use frap_spec_rna as spec;

use super::attribute_value::{self, AttrValues};
use super::bind::{other, readonly, soa, value};
use super::bulk;
use super::columns::{CORNER_EDGE, CORNER_VERT, EDGE_VERTS, MATERIAL_INDEX, POSITION};
use super::soa::{self, SoaKind};
use crate::mesh::{AttrData, Attribute, Domain, Mesh, SHARP_EDGE, SHARP_FACE};

/// `Attribute` bindings (base struct of every attribute).
pub(crate) static BINDINGS: [BindingDecl; 6] = [
    value!("name", SmolStr),
    readonly!("data_type", &'static str),
    readonly!("storage_type", &'static str),
    readonly!("domain", &'static str),
    readonly!("is_internal", bool),
    readonly!("is_required", bool),
];

/// One refined attribute struct and its `data` binding (checked by `check_bindings` in the
/// tests; the runtime dispatch is [`refined_srna`]).
#[cfg(test)]
pub(crate) struct Refined {
    /// `FloatAttribute`, …
    pub(crate) srna: &'static StructDef,
    /// `data` binding of the refined struct.
    pub(crate) data: &'static [BindingDecl],
}

macro_rules! refined {
    ($($srna:ident => $item:ident [$($b:expr),*]),* $(,)?) => {
        /// Every refined attribute struct and its `data` binding (for `check_bindings`).
        #[cfg(test)]
        pub(crate) static REFINED: &[Refined] = &[$(Refined {
            srna: &spec::$srna,
            data: &[soa!("data", spec::$item, &[$($b),*])],
        }),*];
    };
}

refined! {
    FLOAT_ATTRIBUTE => FLOAT_ATTRIBUTE_VALUE [value!("value", f32)],
    INT_ATTRIBUTE => INT_ATTRIBUTE_VALUE [value!("value", i32)],
    BYTE_INT_ATTRIBUTE => BYTE_INT_ATTRIBUTE_VALUE [value!("value", i8)],
    BOOL_ATTRIBUTE => BOOL_ATTRIBUTE_VALUE [value!("value", bool)],
    FLOAT2_ATTRIBUTE => FLOAT2_ATTRIBUTE_VALUE [value!("vector", [f32; 2])],
    FLOAT_VECTOR_ATTRIBUTE => FLOAT_VECTOR_ATTRIBUTE_VALUE [value!("vector", [f32; 3])],
    FLOAT_COLOR_ATTRIBUTE => FLOAT_COLOR_ATTRIBUTE_VALUE
        [value!("color", [f32; 4]), value!("color_srgb", [f32; 4])],
    BYTE_COLOR_ATTRIBUTE => BYTE_COLOR_ATTRIBUTE_VALUE
        [value!("color", [f32; 4]), value!("color_srgb", [f32; 4])],
    SHORT2_ATTRIBUTE => SHORT2_ATTRIBUTE_VALUE [value!("value", [i32; 2])],
    INT2_ATTRIBUTE => INT2_ATTRIBUTE_VALUE [value!("value", [i32; 2])],
    QUATERNION_ATTRIBUTE => QUATERNION_ATTRIBUTE_VALUE [value!("value", [f32; 4])],
    FLOAT4X4_ATTRIBUTE => FLOAT4X4_ATTRIBUTE_VALUE [value!("value", [f32; 16])],
    STRING_ATTRIBUTE => STRING_ATTRIBUTE_VALUE [value!("value", SmolStr)],
}

/// The refined `Attribute` struct of a data type. FLOAT4 has no dumped struct
/// (`Float4Attribute` is missing from `spec/blender-5.2/rna`), so it reads as the base
/// `Attribute` and has no `data`.
pub(crate) fn refined_srna(data: &AttrData) -> &'static StructDef {
    match data {
        AttrData::Float(_) => &spec::FLOAT_ATTRIBUTE,
        AttrData::Int(_) => &spec::INT_ATTRIBUTE,
        AttrData::Int8(_) => &spec::BYTE_INT_ATTRIBUTE,
        AttrData::Bool(_) => &spec::BOOL_ATTRIBUTE,
        AttrData::Float2(_) => &spec::FLOAT2_ATTRIBUTE,
        AttrData::Float3(_) => &spec::FLOAT_VECTOR_ATTRIBUTE,
        AttrData::FloatColor(_) => &spec::FLOAT_COLOR_ATTRIBUTE,
        AttrData::ByteColor(_) => &spec::BYTE_COLOR_ATTRIBUTE,
        AttrData::Int16x2(_) => &spec::SHORT2_ATTRIBUTE,
        AttrData::Int32x2(_) => &spec::INT2_ATTRIBUTE,
        AttrData::Quaternion(_) => &spec::QUATERNION_ATTRIBUTE,
        AttrData::Float4x4(_) => &spec::FLOAT4X4_ATTRIBUTE,
        AttrData::String(_) => &spec::STRING_ATTRIBUTE,
        _ => &spec::ATTRIBUTE,
    }
}

/// Attributes whose name is read-only (`Attribute.is_required`).
pub(crate) const REQUIRED: [&str; 4] = [POSITION, EDGE_VERTS, CORNER_VERT, CORNER_EDGE];

/// Built-in mesh attributes with a fixed domain and type: a rename onto one of these names
/// must match both (`Domain unsupported for "sharp_face" attribute`).
const BUILTIN: [(&str, Domain, &str); 7] = [
    (POSITION, Domain::Point, "FLOAT_VECTOR"),
    (EDGE_VERTS, Domain::Edge, "INT32_2D"),
    (CORNER_VERT, Domain::Corner, "INT"),
    (CORNER_EDGE, Domain::Corner, "INT"),
    (SHARP_FACE, Domain::Face, "BOOLEAN"),
    (SHARP_EDGE, Domain::Edge, "BOOLEAN"),
    (MATERIAL_INDEX, Domain::Face, "INT"),
];

/// The pin layer of a UV map (`.pn.<uv name>`).
pub(crate) fn pin_name(uv: &str) -> SmolStr {
    SmolStr::from(format!(".pn.{uv}"))
}

/// Attribute at position `pos` of the attribute set.
pub(crate) fn nth(m: &Mesh, pos: usize) -> Option<&Attribute> {
    m.attributes().iter().nth(pos)
}

/// Attribute at position `pos`, mutably (clears the mesh caches).
pub(crate) fn nth_mut(m: &mut Mesh, pos: usize) -> Option<&mut Attribute> {
    m.attributes_mut().iter_mut().nth(pos)
}

/// Positions (in the attribute set) of the attributes a layer collection lists.
pub(crate) fn positions_where(m: &Mesh, keep: impl Fn(&Attribute) -> bool) -> Vec<usize> {
    m.attributes()
        .iter()
        .enumerate()
        .filter(|(_, a)| keep(a))
        .map(|(k, _)| k)
        .collect()
}

/// `Attribute.name` / `MeshUVLoopLayer.name` / `MeshLoopColorLayer.name` write (module docs).
pub(crate) fn rename(m: &mut Mesh, pos: usize, new: SmolStr) {
    let Some(a) = nth(m, pos) else { return };
    let (old, domain, data_type) = (a.name.clone(), a.domain, a.data.data_type());
    let is_uv = domain == Domain::Corner && data_type == "FLOAT2";
    if new == old || new.is_empty() {
        return; // Blender: "Attribute name cannot be empty" (ERROR report, no exception).
    }
    if let Some((_, d, t)) = BUILTIN.iter().find(|(n, ..)| *n == new.as_str())
        && (*d != domain || *t != data_type)
    {
        return; // Blender: "Domain/Type unsupported for "<name>" attribute" (ERROR report).
    }
    let attrs = m.attributes_mut();
    attrs.remove(&new);
    if let Some(a) = attrs.get_mut(&old) {
        a.name = new.clone();
    }
    if is_uv {
        let (old_pin, new_pin) = (pin_name(&old), pin_name(&new));
        if attrs.get(&old_pin).is_some() {
            attrs.remove(&new_pin);
            if let Some(p) = attrs.get_mut(&old_pin) {
                p.name = new_pin;
            }
        }
    }
    for active in [
        &mut m.uv_active,
        &mut m.uv_active_render,
        &mut m.color_active,
        &mut m.color_default,
        // `attributes_active_index` does not move: the renamed attribute stays active.
        &mut m.attributes_active,
    ] {
        if active.as_deref() == Some(old.as_str()) {
            *active = Some(new.clone());
        }
    }
}

/// Which attributes an `Attribute` collection lists.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttrFilter {
    /// `mesh.attributes`
    All,
    /// `mesh.color_attributes`
    Color,
}

impl AttrFilter {
    fn keeps(self, a: &Attribute) -> bool {
        match self {
            AttrFilter::All => true,
            AttrFilter::Color => {
                matches!(a.data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
                    && matches!(a.domain, Domain::Point | Domain::Corner)
            }
        }
    }
}

/// `mesh.attributes` / `mesh.color_attributes`.
pub(crate) struct Attributes;

impl Attributes {
    /// Attribute-set position of item `i`.
    fn pos(m: &Mesh, f: AttrFilter, i: usize) -> usize {
        match f {
            AttrFilter::All => i,
            // `usize::MAX` (no attribute) when a rename removed an earlier colour attribute.
            AttrFilter::Color => positions_where(m, |a| f.keeps(a))
                .get(i)
                .copied()
                .unwrap_or(usize::MAX),
        }
    }
}

impl SoaKind for Attributes {
    type Ctx = AttrFilter;

    fn item_type(_: &Mesh, _: AttrFilter) -> &'static StructDef {
        &spec::ATTRIBUTE
    }
    fn item_srna(m: &Mesh, f: AttrFilter, i: usize) -> &'static StructDef {
        nth(m, Self::pos(m, f, i)).map_or(&spec::ATTRIBUTE, |a| refined_srna(&a.data))
    }
    fn len(m: &Mesh, f: AttrFilter) -> usize {
        m.attributes().iter().filter(|a| f.keeps(a)).count()
    }
    fn key_of(m: &Mesh, f: AttrFilter, i: usize) -> Option<SmolStr> {
        m.attributes()
            .iter()
            .filter(|a| f.keeps(a))
            .nth(i)
            .map(|a| a.name.clone())
    }
    fn find(m: &Mesh, f: AttrFilter, key: &str) -> Option<usize> {
        m.attributes()
            .iter()
            .filter(|a| f.keeps(a))
            .position(|a| a.name == key)
    }
    fn get<'a>(
        m: &'a Mesh,
        f: AttrFilter,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<Field<'a>, RnaError> {
        let pos = Self::pos(m, f, i);
        let Some(a) = nth(m, pos) else {
            return Err(soa::no_attr::<Self>(m, f, i, prop));
        };
        let v = match prop.identifier {
            "name" => Value::Str(a.name.clone()),
            "data_type" => Value::Enum(a.data.data_type().into()),
            "storage_type" => Value::Enum("ARRAY".into()),
            "domain" => Value::Enum(a.domain.identifier().into()),
            "is_internal" => Value::Bool(a.name.starts_with('.')),
            "is_required" => Value::Bool(REQUIRED.contains(&a.name.as_str())),
            "data" if attribute_value::has_values(&a.data) => {
                return Ok(soa::coll_field::<AttrValues>(m, pos));
            }
            _ => return Err(soa::no_attr::<Self>(m, f, i, prop)),
        };
        Ok(Field::Value(v))
    }
    fn set(
        m: &mut Mesh,
        f: AttrFilter,
        i: usize,
        prop: &'static PropDef,
        v: Value,
    ) -> Result<SetOutcome, RnaError> {
        let pos = Self::pos(m, f, i);
        let required = nth(m, pos).is_some_and(|a| REQUIRED.contains(&a.name.as_str()));
        match prop.identifier {
            "name" if required => Err(soa::read_only::<Self>(m, f, i, prop)),
            "name" => {
                rename(m, pos, SmolStr::from_rna_value(prop, v)?);
                Ok(SetOutcome::Done)
            }
            "data_type" | "storage_type" | "domain" | "is_internal" | "is_required" | "data" => {
                Err(soa::read_only::<Self>(m, f, i, prop))
            }
            _ => Err(soa::no_attr::<Self>(m, f, i, prop)),
        }
    }
    fn coll_mut<'a>(
        m: &'a mut Mesh,
        f: AttrFilter,
        i: usize,
        prop: &'static PropDef,
    ) -> Result<RnaCollMutGuard<'a>, RnaError> {
        let pos = Self::pos(m, f, i);
        match (prop.identifier, nth(m, pos)) {
            ("data", Some(a)) if attribute_value::has_values(&a.data) => {
                Ok(soa::coll_mut_guard::<AttrValues>(m, pos))
            }
            _ => Err(frap_rna::traits::wrong_access(
                Self::item_srna(m, f, i),
                prop.identifier,
                "a collection",
            )),
        }
    }
    fn foreach_get(
        m: &Mesh,
        f: AttrFilter,
        prop: &'static PropDef,
        range: Range<usize>,
    ) -> Result<Bulk, RnaError> {
        bulk::generic_get::<Self>(m, f, prop, range)
    }
}

/// `Mesh.attributes` collection binding.
pub(crate) const ATTRIBUTES: BindingDecl = soa!("attributes", spec::ATTRIBUTE, &BINDINGS);
/// `Mesh.color_attributes` collection binding.
pub(crate) const COLOR_ATTRIBUTES: BindingDecl =
    soa!("color_attributes", spec::ATTRIBUTE, &BINDINGS);
/// `Mesh.materials` binding.
pub(crate) const MATERIALS: BindingDecl = other!("materials", IdColl, item = spec::MATERIAL);
