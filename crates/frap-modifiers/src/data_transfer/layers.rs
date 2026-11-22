// SPDX-License-Identifier: GPL-3.0-or-later
//! Matching source and destination data layers (creating or deleting destination layers when
//! allowed) and transferring the mapped values with the mix modes.
//!
//! Ported from blender/source/blender/blenkernel/intern/data_transfer.cc
//! (`data_transfer_layersmapping_generate` and its `_cdlayers`, `_cdlayers_multisrc_to_dst`,
//! `_add_item_attr` helpers, `transfer_attribute_name_strings`,
//! `data_transfer_interp_float_do`), blender/source/blender/blenkernel/intern/deform.cc
//! (`data_transfer_layersmapping_vgroups(_multisrc_to_dst)`, `vgroups_datatransfer_interp`)
//! and blender/source/blender/blenkernel/intern/customdata.cc (`CustomData_data_transfer`,
//! `customdata_data_transfer_interp_generic`, `customdata_data_transfer_interp_normal_normals`
//! and the `layerInterp_*` / `layerCopyValue_*` callbacks of the transferred types), Blender
//! 5.2.4, GPL-2.0-or-later.
//!
//! Float and boolean attributes have no `copyvalue` callback in Blender: their transferred
//! value replaces the destination whatever the mix mode and factor (only elements without a
//! source keep their value). Colors, UVs and custom normals mix.

use std::sync::Arc;

use frap_bmesh::shape::deform;
use frap_mesh::{AttrData, Attribute, DeformVert, Domain, Mesh};
use smol_str::SmolStr;

use super::math::{
    V3, blend_add_byte, blend_add_float, blend_interpolate_byte, blend_interpolate_float,
    blend_mix_byte, blend_mix_float, blend_mul_byte, blend_mul_float, blend_sub_byte,
    blend_sub_float, interp_slerp_safe, interpf, normalized, round_fl_to_uchar_clamp,
};
use super::modes::*;
use super::remap::Remap;
use super::space::SpaceTransform;

/// `ME_VERT` / `ME_EDGE` / `ME_LOOP` / `ME_POLY`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Elem {
    Vert,
    Edge,
    Loop,
    Poly,
}

/// The attribute types the transfer handles (`CD_PROP_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttrType {
    Float,
    Bool,
    Float2,
    FloatColor,
    ByteColor,
}

impl AttrType {
    fn matches(self, d: &AttrData) -> bool {
        matches!(
            (self, d),
            (AttrType::Float, AttrData::Float(_))
                | (AttrType::Bool, AttrData::Bool(_))
                | (AttrType::Float2, AttrData::Float2(_))
                | (AttrType::FloatColor, AttrData::FloatColor(_))
                | (AttrType::ByteColor, AttrData::ByteColor(_))
        )
    }

    fn default_column(self, len: usize) -> AttrData {
        match self {
            AttrType::Float => AttrData::Float(Arc::new(vec![0.0; len])),
            AttrType::Bool => AttrData::Bool(Arc::new(vec![false; len])),
            AttrType::Float2 => AttrData::Float2(Arc::new(vec![[0.0; 2]; len])),
            // Colour attributes default to opaque white (`ColorGeometry4f/4b` defaults).
            AttrType::FloatColor => AttrData::FloatColor(Arc::new(vec![[1.0; 4]; len])),
            AttrType::ByteColor => AttrData::ByteColor(Arc::new(vec![[255; 4]; len])),
        }
    }
}

/// The fake custom-data types of `data_transfer_intern.hh` (and the real ones used).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum CdType {
    /// `CD_FAKE_MDEFORMVERT`
    Vgroups,
    /// `CD_FAKE_BWEIGHT`
    BevelWeight,
    /// `CD_FAKE_SHARP`
    Sharp,
    /// `CD_FAKE_SEAM`
    Seam,
    /// `CD_FAKE_CREASE`
    Crease,
    /// `CD_FAKE_FREESTYLE_EDGE`
    FreestyleEdge,
    /// `CD_FAKE_FREESTYLE_FACE`
    FreestyleFace,
    /// `CD_FAKE_UV`
    Uv,
    /// `CD_FAKE_LNOR`
    Lnor,
    /// `CD_PROP_COLOR`
    PropColor,
    /// `CD_PROP_BYTE_COLOR`
    ByteColor,
    /// `CD_MVERT_SKIN` (skin data is not bound in frap: nothing to transfer).
    Skin,
}

/// `BKE_object_data_transfer_dttype_to_cdtype`
pub(crate) fn dttype_to_cdtype(t: u32) -> CdType {
    match t {
        DT_TYPE_MDEFORMVERT => CdType::Vgroups,
        DT_TYPE_SKIN => CdType::Skin,
        DT_TYPE_BWEIGHT_VERT | DT_TYPE_BWEIGHT_EDGE => CdType::BevelWeight,
        DT_TYPE_SHARP_EDGE | DT_TYPE_SHARP_FACE => CdType::Sharp,
        DT_TYPE_SEAM => CdType::Seam,
        DT_TYPE_CREASE => CdType::Crease,
        DT_TYPE_FREESTYLE_EDGE => CdType::FreestyleEdge,
        DT_TYPE_UV => CdType::Uv,
        DT_TYPE_FREESTYLE_FACE => CdType::FreestyleFace,
        DT_TYPE_LNOR => CdType::Lnor,
        DT_TYPE_MLOOPCOL_VERT | DT_TYPE_MLOOPCOL_LOOP => CdType::ByteColor,
        _ => CdType::PropColor,
    }
}

/// What one mapped layer is.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    /// Vertex group `src` into vertex group `dst`.
    Vgroup { src: u32, dst: u32 },
    /// A generic attribute of `domain` and `ty`.
    Attr {
        ty: AttrType,
        domain: Domain,
        src: SmolStr,
        dst: SmolStr,
    },
    /// Custom normals (into the temporary `CD_NORMAL` layer).
    Normals,
}

/// `CustomDataTransferLayerMap`
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerMap {
    pub item: Item,
    pub mix_mode: i32,
    pub mix_factor: f32,
}

/// The source of a transfer (the evaluated source mesh and its object's groups).
pub(crate) struct Src<'a> {
    pub mesh: &'a Mesh,
    /// `BKE_object_defgroup_list(ob_src)`.
    pub vgroup_names: &'a [SmolStr],
    /// `BKE_object_defgroup_active_index_get(ob_src) - 1`.
    pub active_vgroup: i32,
    /// The subsets of `BONE_SELECT` and `BONE_DEFORM` (`None`: the object has no such
    /// armature data, as `BKE_object_defgroup_subset_from_select_type` returning null).
    pub bone_select: Option<Vec<bool>>,
    pub bone_deform: Option<Vec<bool>>,
}

/// The destination of a transfer: its mesh and the object's active group index
/// (`vertex_groups.active_index`).
pub(crate) struct Dst<'a> {
    pub mesh: &'a mut Mesh,
    pub active_vgroup: &'a mut i32,
}

// ------------------------------------------------------------------------- vertex groups

/// `BKE_object_defgroup_add_name` (`None`: `BKE_object_defgroup_add`, named `Group`): the new
/// group's index, made active.
fn defgroup_add(dst: &mut Dst<'_>, name: Option<&str>) -> u32 {
    let n = deform::unique_group_name(&dst.mesh.vertex_group_names, None, name.unwrap_or("Group"));
    dst.mesh.vertex_group_names.push(n);
    let count = dst.mesh.vertex_group_names.len();
    *dst.active_vgroup = count as i32 - 1;
    (count - 1) as u32
}

/// `BKE_object_defgroup_remove` (Object Mode).
fn defgroup_remove(dst: &mut Dst<'_>, g: usize) {
    deform::remove_group_mesh(dst.mesh, g as u32);
    dst.mesh.remove_vertex_group_name(g);
    let empty = dst.mesh.vertex_group_names.is_empty();
    if *dst.active_vgroup + 1 > g as i32 {
        *dst.active_vgroup -= 1;
    }
    if !empty && *dst.active_vgroup + 1 < 1 {
        *dst.active_vgroup = 0;
    }
}

fn group_index(names: &[SmolStr], name: &str) -> Option<usize> {
    names.iter().position(|n| n == name)
}

/// `data_transfer_layersmapping_vgroups_multisrc_to_dst`
#[allow(clippy::too_many_arguments)]
fn vgroups_multisrc_to_dst(
    r_map: Option<&mut Vec<LayerMap>>,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    src: &Src<'_>,
    dst: &mut Dst<'_>,
    tolayers: i32,
    use_layers_src: &[bool],
) -> bool {
    let num_layers_src = use_layers_src.len();
    let tot_dst = dst.mesh.vertex_group_names.len();
    let mut items = Vec::new();
    match tolayers {
        DT_LAYERS_INDEX_DST => {
            let mut idx_dst = tot_dst;
            let mut idx_src = num_layers_src;
            while idx_src > 0 && !use_layers_src[idx_src - 1] {
                idx_src -= 1;
            }
            if idx_dst < idx_src {
                if use_create {
                    while idx_dst < idx_src {
                        defgroup_add(dst, None);
                        idx_dst += 1;
                    }
                } else {
                    idx_src = idx_dst;
                }
            } else if use_delete && idx_dst > idx_src {
                while idx_dst > idx_src {
                    idx_dst -= 1;
                    let last = dst.mesh.vertex_group_names.len() - 1;
                    defgroup_remove(dst, last);
                }
            }
            if r_map.is_some() {
                for i in (0..idx_src).rev() {
                    if !use_layers_src[i] {
                        continue;
                    }
                    items.push(Item::Vgroup {
                        src: i as u32,
                        dst: i as u32,
                    });
                }
            }
        }
        DT_LAYERS_NAME_DST => {
            if use_delete {
                let mut g = 0;
                while g < dst.mesh.vertex_group_names.len() {
                    let name = dst.mesh.vertex_group_names[g].clone();
                    if group_index(&src.mesh.vertex_group_names, &name).is_none() {
                        defgroup_remove(dst, g);
                    } else {
                        g += 1;
                    }
                }
            }
            for (idx_src, &used) in use_layers_src.iter().enumerate() {
                if !used {
                    continue;
                }
                let name = src.vgroup_names[idx_src].clone();
                let idx_dst = match group_index(&dst.mesh.vertex_group_names, &name) {
                    Some(i) => i as u32,
                    None if use_create => defgroup_add(dst, Some(&name)),
                    None => continue,
                };
                if r_map.is_some() {
                    items.push(Item::Vgroup {
                        src: idx_src as u32,
                        dst: idx_dst,
                    });
                }
            }
        }
        _ => return false,
    }
    if let Some(m) = r_map {
        for item in items {
            m.push(LayerMap {
                item,
                mix_mode,
                mix_factor,
            });
        }
    }
    true
}

/// `data_transfer_layersmapping_vgroups`
#[allow(clippy::too_many_arguments)]
fn vgroups(
    r_map: Option<&mut Vec<LayerMap>>,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    src: &Src<'_>,
    dst: &mut Dst<'_>,
    fromlayers: i32,
    tolayers: i32,
) -> bool {
    let src_count = src.vgroup_names.len();
    if src_count == 0 {
        if use_delete {
            // BKE_object_defgroup_remove_all
            while !dst.mesh.vertex_group_names.is_empty() {
                let last = dst.mesh.vertex_group_names.len() - 1;
                defgroup_remove(dst, last);
            }
            *dst.active_vgroup = 0;
        }
        return true;
    }
    if fromlayers == DT_LAYERS_ACTIVE_SRC || fromlayers >= 0 {
        let idx_src = if fromlayers >= 0 {
            if fromlayers as usize >= src_count {
                return false;
            }
            fromlayers as usize
        } else if src.active_vgroup < 0 || src.active_vgroup as usize >= src_count {
            return false;
        } else {
            src.active_vgroup as usize
        };
        let idx_dst: u32 = if tolayers >= 0 {
            tolayers as u32
        } else if tolayers == DT_LAYERS_ACTIVE_DST {
            let a = *dst.active_vgroup;
            if a < 0 || a as usize >= dst.mesh.vertex_group_names.len() {
                if !use_create {
                    return true;
                }
                let name = src.vgroup_names[idx_src].clone();
                defgroup_add(dst, Some(&name))
            } else {
                a as u32
            }
        } else if tolayers == DT_LAYERS_INDEX_DST {
            let mut num = src_count;
            if num <= idx_src {
                if !use_create {
                    return true;
                }
                while num <= idx_src {
                    defgroup_add(dst, None);
                    num += 1;
                }
            }
            idx_src as u32
        } else if tolayers == DT_LAYERS_NAME_DST {
            let name = src.vgroup_names[idx_src].clone();
            match group_index(&dst.mesh.vertex_group_names, &name) {
                Some(i) => i as u32,
                None if use_create => defgroup_add(dst, Some(&name)),
                None => return true,
            }
        } else {
            return false;
        };
        if let Some(m) = r_map {
            m.push(LayerMap {
                item: Item::Vgroup {
                    src: idx_src as u32,
                    dst: idx_dst,
                },
                mix_mode,
                mix_factor,
            });
        }
        return true;
    }
    let use_layers_src: Option<Vec<bool>> = match fromlayers {
        DT_LAYERS_ALL_SRC => Some(vec![true; src_count]),
        DT_LAYERS_VGROUP_SRC_BONE_SELECT => src.bone_select.clone(),
        DT_LAYERS_VGROUP_SRC_BONE_DEFORM => src.bone_deform.clone(),
        _ => None,
    };
    match use_layers_src {
        Some(mut u) => {
            u.resize(src_count, false);
            vgroups_multisrc_to_dst(
                r_map, mix_mode, mix_factor, use_create, use_delete, src, dst, tolayers, &u,
            )
        }
        None => false,
    }
}

// ------------------------------------------------------------------- generic attributes

fn attr_names(mesh: &Mesh, domain: Domain, ty: AttrType) -> Vec<SmolStr> {
    mesh.attributes()
        .iter()
        .filter(|a| a.domain == domain && ty.matches(&a.data))
        .map(|a| a.name.clone())
        .collect()
}

fn has_attr(mesh: &Mesh, name: &str, domain: Domain, ty: AttrType) -> bool {
    mesh.attributes()
        .get(name)
        .is_some_and(|a| a.domain == domain && ty.matches(&a.data))
}

fn domain_len(mesh: &Mesh, domain: Domain) -> usize {
    match domain {
        Domain::Point => mesh.verts_num(),
        Domain::Edge => mesh.edges_num(),
        Domain::Face => mesh.faces_num(),
        Domain::Corner => mesh.corners_num(),
    }
}

/// `dst_attributes.add(name, domain, type, AttributeInitDefaultValue())`: nothing when the
/// name is taken.
fn attr_add(mesh: &mut Mesh, name: &str, domain: Domain, ty: AttrType) {
    if mesh.attributes().get(name).is_some() {
        return;
    }
    let len = domain_len(mesh, domain);
    mesh.attributes_mut().insert(Attribute {
        name: name.into(),
        domain,
        data: ty.default_column(len),
    });
}

/// `bke::mesh::is_color_attribute`
fn is_color(a: &Attribute) -> bool {
    matches!(a.domain, Domain::Point | Domain::Corner)
        && matches!(a.data, AttrData::FloatColor(_) | AttrData::ByteColor(_))
}

/// `bke::mesh::is_uv_map`
fn is_uv(a: &Attribute) -> bool {
    a.domain == Domain::Corner && matches!(a.data, AttrData::Float2(_))
}

/// `transfer_{active,default}_{color,uv_map}_string`: an unset destination name takes the
/// source's when the destination has a matching layer, else the first matching layer.
fn transfer_name_string(
    dst: &mut Mesh,
    src: &Mesh,
    get: fn(&Mesh) -> Option<&SmolStr>,
    set: fn(&mut Mesh, Option<SmolStr>),
    pred: fn(&Attribute) -> bool,
) {
    if get(dst).is_some_and(|n| !n.is_empty()) {
        return;
    }
    let Some(name) = get(src).cloned() else {
        return;
    };
    if !src.attributes().get(&name).is_some_and(pred) {
        return;
    }
    if dst.attributes().get(&name).is_some_and(pred) {
        set(dst, Some(name));
    } else {
        let first = dst
            .attributes()
            .iter()
            .find(|a| pred(a))
            .map(|a| a.name.clone());
        if let Some(n) = first {
            set(dst, Some(n));
        }
    }
}

/// `transfer_attribute_name_strings`
pub(crate) fn transfer_attribute_name_strings(cd: Option<AttrType>, dst: &mut Mesh, src: &Mesh) {
    match cd {
        Some(AttrType::FloatColor | AttrType::ByteColor) => {
            transfer_name_string(
                dst,
                src,
                |m| m.color_active.as_ref(),
                |m, v| m.color_active = v,
                is_color,
            );
            transfer_name_string(
                dst,
                src,
                |m| m.color_default.as_ref(),
                |m, v| m.color_default = v,
                is_color,
            );
        }
        Some(AttrType::Float2) => {
            transfer_name_string(
                dst,
                src,
                |m| m.uv_active.as_ref(),
                |m, v| m.uv_active = v,
                is_uv,
            );
            transfer_name_string(
                dst,
                src,
                |m| m.uv_active_render.as_ref(),
                |m, v| m.uv_active_render = v,
                is_uv,
            );
        }
        _ => {}
    }
}

/// `data_transfer_layersmapping_cdlayers_multisrc_to_dst`
#[allow(clippy::too_many_arguments)]
fn cdlayers_multisrc_to_dst(
    r_map: Option<&mut Vec<LayerMap>>,
    ty: AttrType,
    domain: Domain,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    src: &Mesh,
    dst: &mut Mesh,
    src_names: &[SmolStr],
    dst_names: &[SmolStr],
    tolayers: i32,
    use_layers_src: Option<&[bool]>,
) -> bool {
    let Some(use_layers_src) = use_layers_src else {
        if use_delete {
            for n in dst_names.iter().rev() {
                dst.attributes_mut().remove(n);
            }
        }
        return true;
    };
    let mut items = Vec::new();
    match tolayers {
        DT_LAYERS_INDEX_DST => {
            let mut idx_dst = dst_names.len();
            let mut idx_src = use_layers_src.len();
            while idx_src > 0 && !use_layers_src[idx_src - 1] {
                idx_src -= 1;
            }
            if idx_dst < idx_src {
                if use_create {
                    while idx_dst < idx_src {
                        attr_add(dst, &src_names[idx_dst], domain, ty);
                        idx_dst += 1;
                    }
                } else {
                    idx_src = idx_dst;
                }
            } else if use_delete && idx_dst > idx_src {
                while idx_dst > idx_src {
                    idx_dst -= 1;
                    dst.attributes_mut().remove(&dst_names[idx_dst]);
                }
            }
            if r_map.is_some() {
                // The layers just created are named after the source ones (Blender reads past
                // its old destination name list here).
                let names_now = attr_names(dst, domain, ty);
                for i in (0..idx_src).rev() {
                    if !use_layers_src[i] {
                        continue;
                    }
                    let Some(d) = names_now.get(i) else { continue };
                    items.push(Item::Attr {
                        ty,
                        domain,
                        src: src_names[i].clone(),
                        dst: d.clone(),
                    });
                }
            }
        }
        DT_LAYERS_NAME_DST => {
            let mut to_delete: Vec<SmolStr> = Vec::new();
            for i in (0..use_layers_src.len()).rev() {
                if !use_layers_src[i] {
                    continue;
                }
                let name = src_names[i].clone();
                if dst.attributes().get(&name).is_none() {
                    if use_create {
                        attr_add(dst, &name, domain, ty);
                    } else {
                        continue;
                    }
                } else if use_delete {
                    to_delete.push(name.clone());
                }
                if r_map.is_some() {
                    items.push(Item::Attr {
                        ty,
                        domain,
                        src: name.clone(),
                        dst: name,
                    });
                }
            }
            for n in &to_delete {
                dst.attributes_mut().remove(n);
            }
        }
        _ => return false,
    }
    if let Some(m) = r_map {
        for item in items {
            m.push(LayerMap {
                item,
                mix_mode,
                mix_factor,
            });
        }
    }
    transfer_attribute_name_strings(Some(ty), dst, src);
    true
}

/// `data_transfer_layersmapping_cdlayers`: UV maps and color attributes.
#[allow(clippy::too_many_arguments)]
fn cdlayers(
    r_map: Option<&mut Vec<LayerMap>>,
    ty: AttrType,
    domain: Domain,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    src: &Mesh,
    dst: &mut Mesh,
    fromlayers: i32,
    tolayers: i32,
) -> bool {
    let src_names = attr_names(src, domain, ty);
    if src_names.is_empty() {
        return true;
    }
    let dst_names = attr_names(dst, domain, ty);
    if fromlayers == DT_LAYERS_ACTIVE_SRC || fromlayers >= 0 {
        let name_src: SmolStr = if fromlayers >= 0 {
            match src_names.get(fromlayers as usize) {
                Some(n) => n.clone(),
                None => return false,
            }
        } else {
            let active = match ty {
                AttrType::Float2 => src.uv_active.clone(),
                _ => src.color_active.clone(),
            };
            match active {
                Some(n) if !n.is_empty() => n,
                _ => return true,
            }
        };
        let idx_src = src_names.iter().position(|n| *n == name_src);
        if !has_attr(src, &name_src, domain, ty) {
            return true;
        }
        let name_dst: Option<SmolStr> = if tolayers >= 0 {
            dst_names.get(tolayers as usize).cloned()
        } else if tolayers == DT_LAYERS_ACTIVE_DST {
            let active = match ty {
                AttrType::Float2 => dst.uv_active.clone(),
                _ => dst.color_active.clone(),
            };
            match active {
                Some(n) if !n.is_empty() => Some(n),
                _ => {
                    if !use_create {
                        return true;
                    }
                    attr_add(dst, &name_src, domain, ty);
                    Some(name_src.clone())
                }
            }
        } else if tolayers == DT_LAYERS_INDEX_DST {
            let mut num = dst_names.len();
            let idx_dst = idx_src.unwrap_or(usize::MAX);
            if idx_dst != usize::MAX && num <= idx_dst {
                if !use_create {
                    return true;
                }
                while num <= idx_dst {
                    attr_add(dst, &src_names[num], domain, ty);
                    num += 1;
                }
            }
            // Blender looks the destination up with a name it never set: no layer.
            None
        } else if tolayers == DT_LAYERS_NAME_DST {
            if !has_attr(dst, &name_src, domain, ty) {
                if !use_create {
                    return true;
                }
                attr_add(dst, &name_src, domain, ty);
            }
            Some(name_src.clone())
        } else {
            return false;
        };
        let Some(name_dst) = name_dst.filter(|n| dst.attributes().get(n).is_some()) else {
            return false;
        };
        transfer_attribute_name_strings(Some(ty), dst, src);
        if let Some(m) = r_map {
            m.push(LayerMap {
                item: Item::Attr {
                    ty,
                    domain,
                    src: name_src,
                    dst: name_dst,
                },
                mix_mode,
                mix_factor,
            });
        }
        return true;
    }
    if fromlayers == DT_LAYERS_ALL_SRC {
        let use_layers = vec![true; src_names.len()];
        let ret = cdlayers_multisrc_to_dst(
            r_map,
            ty,
            domain,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            src,
            dst,
            &src_names,
            &dst_names,
            tolayers,
            (!use_layers.is_empty()).then_some(use_layers.as_slice()),
        );
        transfer_attribute_name_strings(Some(ty), dst, src);
        return ret;
    }
    false
}

/// `data_transfer_layersmapping_add_item_attr`: one named attribute.
#[allow(clippy::too_many_arguments)]
fn add_item_attr(
    r_map: Option<&mut Vec<LayerMap>>,
    ty: AttrType,
    domain: Domain,
    name: &str,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    src: &Mesh,
    dst: &mut Mesh,
) {
    if has_attr(src, name, domain, ty) {
        if dst.attributes().get(name).is_none() && use_create {
            attr_add(dst, name, domain, ty);
        }
        if let Some(m) = r_map
            && has_attr(dst, name, domain, ty)
        {
            m.push(LayerMap {
                item: Item::Attr {
                    ty,
                    domain,
                    src: name.into(),
                    dst: name.into(),
                },
                mix_mode,
                mix_factor,
            });
        }
    } else if use_delete {
        dst.attributes_mut().remove(name);
    }
}

/// `data_transfer_layersmapping_generate`: the layer maps of one data type (`r_map`), or only
/// the destination layout (`None`). `false` for invalid parameters.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate(
    mut r_map: Option<&mut Vec<LayerMap>>,
    src: &Src<'_>,
    dst: &mut Dst<'_>,
    elem: Elem,
    cd: CdType,
    mix_mode: i32,
    mix_factor: f32,
    use_create: bool,
    use_delete: bool,
    fromlayers: i32,
    tolayers: i32,
) -> bool {
    let sm = src.mesh;
    let attr = |r_map: Option<&mut Vec<LayerMap>>, dst: &mut Dst<'_>, ty, domain, name: &str| {
        add_item_attr(
            r_map, ty, domain, name, mix_mode, mix_factor, use_create, use_delete, sm, dst.mesh,
        );
        true
    };
    match (elem, cd) {
        (Elem::Vert, CdType::Skin) => true,
        (Elem::Vert, CdType::ByteColor) => cdlayers(
            r_map,
            AttrType::ByteColor,
            Domain::Point,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            sm,
            dst.mesh,
            fromlayers,
            tolayers,
        ),
        (Elem::Vert, CdType::PropColor) => cdlayers(
            r_map,
            AttrType::FloatColor,
            Domain::Point,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            sm,
            dst.mesh,
            fromlayers,
            tolayers,
        ),
        (Elem::Vert, CdType::Vgroups) => vgroups(
            r_map, mix_mode, mix_factor, use_create, use_delete, src, dst, fromlayers, tolayers,
        ),
        (Elem::Vert, CdType::BevelWeight) => attr(
            r_map,
            dst,
            AttrType::Float,
            Domain::Point,
            "bevel_weight_vert",
        ),
        (Elem::Vert, _) => false,
        (Elem::Edge, CdType::Seam) => attr(r_map, dst, AttrType::Bool, Domain::Edge, "uv_seam"),
        (Elem::Edge, CdType::Sharp) => attr(r_map, dst, AttrType::Bool, Domain::Edge, "sharp_edge"),
        (Elem::Edge, CdType::BevelWeight) => attr(
            r_map,
            dst,
            AttrType::Float,
            Domain::Edge,
            "bevel_weight_edge",
        ),
        (Elem::Edge, CdType::Crease) => {
            attr(r_map, dst, AttrType::Float, Domain::Edge, "crease_edge")
        }
        (Elem::Edge, CdType::FreestyleEdge) if r_map.is_some() => {
            attr(r_map, dst, AttrType::Bool, Domain::Edge, "freestyle_edge")
        }
        (Elem::Edge, _) => false,
        (Elem::Loop, CdType::Uv) => cdlayers(
            r_map,
            AttrType::Float2,
            Domain::Corner,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            sm,
            dst.mesh,
            fromlayers,
            tolayers,
        ),
        (Elem::Loop, CdType::Lnor) => {
            if let Some(m) = r_map.as_deref_mut() {
                m.push(LayerMap {
                    item: Item::Normals,
                    mix_mode,
                    mix_factor,
                });
            }
            true
        }
        (Elem::Loop, CdType::ByteColor) => cdlayers(
            r_map,
            AttrType::ByteColor,
            Domain::Corner,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            sm,
            dst.mesh,
            fromlayers,
            tolayers,
        ),
        (Elem::Loop, CdType::PropColor) => cdlayers(
            r_map,
            AttrType::FloatColor,
            Domain::Corner,
            mix_mode,
            mix_factor,
            use_create,
            use_delete,
            sm,
            dst.mesh,
            fromlayers,
            tolayers,
        ),
        (Elem::Loop, _) => false,
        (Elem::Poly, CdType::Sharp) => attr(r_map, dst, AttrType::Bool, Domain::Face, "sharp_face"),
        (Elem::Poly, CdType::FreestyleFace) => {
            attr(r_map, dst, AttrType::Bool, Domain::Face, "freestyle_face")
        }
        (Elem::Poly, _) => false,
    }
}

// ------------------------------------------------------------------------------ transfer

/// `data_transfer_interp_float_do`
pub(crate) fn interp_float_do(mix_mode: i32, val_dst: f32, val_src: f32, mix_factor: f32) -> f32 {
    if (mix_mode == CDT_MIX_REPLACE_ABOVE_THRESHOLD && val_dst < mix_factor)
        || (mix_mode == CDT_MIX_REPLACE_BELOW_THRESHOLD && val_dst > mix_factor)
    {
        return val_dst;
    }
    let val_ret = match mix_mode {
        CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD => return val_src,
        CDT_MIX_MIX => (val_dst + val_src) * 0.5,
        CDT_MIX_ADD => val_dst + val_src,
        CDT_MIX_SUB => val_dst - val_src,
        CDT_MIX_MUL => val_dst * val_src,
        _ => val_src,
    };
    interpf(val_ret, val_dst, mix_factor)
}

/// `vgroups_datatransfer_interp` for one destination vertex.
fn vgroup_interp(
    lm: &LayerMap,
    idx_src: u32,
    idx_dst: u32,
    dst: &mut DeformVert,
    sources: Option<&[&DeformVert]>,
    weights: &[f32],
    mix_factor: f32,
) {
    let dw_dst = deform::find(dst, idx_dst);
    let mut weight_src = 0.0f32;
    let mut has_dw_sources = false;
    if let Some(srcs) = sources {
        for i in (0..srcs.len()).rev() {
            for &(g, w) in srcs[i].weights.iter().rev() {
                if g == idx_src {
                    weight_src += w * weights[i];
                    has_dw_sources = true;
                    break;
                }
            }
        }
    }
    let weight_dst = match dw_dst {
        Some(k) => dst.weights[k].1,
        None if lm.mix_mode == CDT_MIX_REPLACE_ABOVE_THRESHOLD => return,
        None => 0.0,
    };
    let w = interp_float_do(lm.mix_mode, weight_dst, weight_src, mix_factor).clamp(0.0, 1.0);
    if !has_dw_sources {
        if let Some(k) = dw_dst {
            dst.weights[k].1 = w;
        }
    } else if let Some(k) = dw_dst {
        dst.weights[k].1 = w;
    } else {
        deform::add_notest(dst, idx_dst, w);
    }
}

/// `layerCopyValue_propcol`
fn copy_value_propcol(src: [f32; 4], dst: &mut [f32; 4], mix_mode: i32, f: f32) {
    if matches!(
        mix_mode,
        CDT_MIX_NOMIX | CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD
    ) {
        if matches!(
            mix_mode,
            CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD
        ) {
            let m = (dst[0] + dst[1] + dst[2]) / 3.0;
            if mix_mode == CDT_MIX_REPLACE_ABOVE_THRESHOLD && m < f {
                return;
            }
            if mix_mode == CDT_MIX_REPLACE_BELOW_THRESHOLD && m > f {
                return;
            }
        }
        *dst = src;
    } else {
        let tmp = match mix_mode {
            CDT_MIX_MIX => blend_mix_float(*dst, src),
            CDT_MIX_ADD => blend_add_float(*dst, src),
            CDT_MIX_SUB => blend_sub_float(*dst, src),
            CDT_MIX_MUL => blend_mul_float(*dst, src),
            _ => src,
        };
        *dst = blend_interpolate_float(*dst, tmp, f);
    }
}

/// `layerCopyValue_mloopcol`
fn copy_value_mloopcol(src: [u8; 4], dst: &mut [u8; 4], mix_mode: i32, f: f32) {
    if matches!(
        mix_mode,
        CDT_MIX_NOMIX | CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD
    ) {
        if matches!(
            mix_mode,
            CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD
        ) {
            let m = (f32::from(dst[0]) + f32::from(dst[1]) + f32::from(dst[2])) / 3.0;
            if mix_mode == CDT_MIX_REPLACE_ABOVE_THRESHOLD && m < f {
                return;
            }
            if mix_mode == CDT_MIX_REPLACE_BELOW_THRESHOLD && m > f {
                return;
            }
        }
        *dst = src;
    } else {
        let tmp = match mix_mode {
            CDT_MIX_MIX => blend_mix_byte(*dst, src),
            CDT_MIX_ADD => blend_add_byte(*dst, src),
            CDT_MIX_SUB => blend_sub_byte(*dst, src),
            CDT_MIX_MUL => blend_mul_byte(*dst, src),
            _ => src,
        };
        *dst = blend_interpolate_byte(*dst, tmp, f);
    }
}

/// `layerCopyValue_normal`
fn copy_value_normal(src: V3, dst: &mut V3, mix_mode: i32, f: f32) {
    if matches!(
        mix_mode,
        CDT_MIX_NOMIX | CDT_MIX_REPLACE_ABOVE_THRESHOLD | CDT_MIX_REPLACE_BELOW_THRESHOLD
    ) {
        *dst = src;
        return;
    }
    let d = *dst;
    let tmp = match mix_mode {
        CDT_MIX_MIX | CDT_MIX_ADD => normalized([d[0] + src[0], d[1] + src[1], d[2] + src[2]]),
        CDT_MIX_SUB => normalized([d[0] - src[0], d[1] - src[1], d[2] - src[2]]),
        CDT_MIX_MUL => normalized([d[0] * src[0], d[1] * src[1], d[2] * src[2]]),
        _ => src,
    };
    *dst = interp_slerp_safe(d, tmp, f);
}

/// The column of attribute `name`, mutably (copy-on-write).
fn column_mut<'m>(mesh: &'m mut Mesh, name: &str) -> Option<&'m mut AttrData> {
    mesh.attributes_mut().get_mut(name).map(|a| &mut a.data)
}

/// `CustomData_data_transfer` of one layer map: every destination element with sources gets
/// the interpolated source value mixed in with `mix_factor × mix_weights[i]`.
/// `normals` is the temporary `CD_NORMAL` layer of custom-normal transfer.
pub(crate) fn apply(
    remap: &Remap,
    lm: &LayerMap,
    mix_weights: Option<&[f32]>,
    src: &Mesh,
    dst: &mut Mesh,
    st: Option<&SpaceTransform>,
    normals: &mut Option<Vec<V3>>,
) {
    let factor = |i: usize| lm.mix_factor * mix_weights.map_or(1.0, |w| w[i]);
    match &lm.item {
        Item::Vgroup { src: gs, dst: gd } => {
            let src_dv = src.deform_verts();
            let dst_dv = dst.deform_verts_mut();
            for (i, it) in remap.items.iter().enumerate() {
                if it.sources.is_empty() || i >= dst_dv.len() {
                    continue;
                }
                let weights: Vec<f32> = it.sources.iter().map(|s| s.1).collect();
                let sources: Option<Vec<&DeformVert>> =
                    src_dv.map(|d| it.sources.iter().map(|s| &d[s.0]).collect());
                vgroup_interp(
                    lm,
                    *gs,
                    *gd,
                    &mut dst_dv[i],
                    sources.as_deref(),
                    &weights,
                    factor(i),
                );
            }
        }
        Item::Normals => {
            let src_nors = src.corner_normals();
            let Some(dst_nors) = normals.as_mut() else {
                return;
            };
            for (i, it) in remap.items.iter().enumerate() {
                if it.sources.is_empty() {
                    continue;
                }
                let mut no = [0.0f32; 3];
                for &(s, w) in &it.sources {
                    let n = src_nors[s];
                    no = [no[0] + n[0] * w, no[1] + n[1] * w, no[2] + n[2] * w];
                }
                let mut tmp = normalized(no);
                if let Some(st) = st {
                    tmp = st.invert_normal(tmp);
                }
                copy_value_normal(tmp, &mut dst_nors[i], lm.mix_mode, factor(i));
            }
        }
        Item::Attr {
            ty,
            src: sname,
            dst: dname,
            ..
        } => {
            let Some(src_col) = src.attributes().get(sname).map(|a| a.data.clone()) else {
                return;
            };
            let Some(dst_col) = column_mut(dst, dname) else {
                return;
            };
            match (ty, &src_col, dst_col) {
                (AttrType::Float, AttrData::Float(s), AttrData::Float(d)) => {
                    let d = Arc::make_mut(d);
                    for (i, it) in remap.items.iter().enumerate() {
                        if it.sources.is_empty() || i >= d.len() {
                            continue;
                        }
                        let mut r = 0.0f32;
                        for &(k, w) in &it.sources {
                            r += s[k] * w;
                        }
                        d[i] = r;
                    }
                }
                (AttrType::Bool, AttrData::Bool(s), AttrData::Bool(d)) => {
                    let d = Arc::make_mut(d);
                    for (i, it) in remap.items.iter().enumerate() {
                        if it.sources.is_empty() || i >= d.len() {
                            continue;
                        }
                        d[i] = it.sources.iter().any(|&(k, w)| s[k] && w > 0.0);
                    }
                }
                (AttrType::Float2, AttrData::Float2(s), AttrData::Float2(d)) => {
                    let d = Arc::make_mut(d);
                    for (i, it) in remap.items.iter().enumerate() {
                        if it.sources.is_empty() || i >= d.len() {
                            continue;
                        }
                        let mut r = [0.0f32; 2];
                        for &(k, w) in &it.sources {
                            r[0] += s[k][0] * w;
                            r[1] += s[k][1] * w;
                        }
                        let f = factor(i);
                        if lm.mix_mode == CDT_MIX_NOMIX {
                            d[i] = r;
                        } else {
                            let b = d[i];
                            d[i] = [b[0] * (1.0 - f) + r[0] * f, b[1] * (1.0 - f) + r[1] * f];
                        }
                    }
                }
                (AttrType::FloatColor, AttrData::FloatColor(s), AttrData::FloatColor(d)) => {
                    let d = Arc::make_mut(d);
                    for (i, it) in remap.items.iter().enumerate() {
                        if it.sources.is_empty() || i >= d.len() {
                            continue;
                        }
                        let mut r = [0.0f32; 4];
                        for &(k, w) in &it.sources {
                            for c in 0..4 {
                                r[c] += s[k][c] * w;
                            }
                        }
                        copy_value_propcol(r, &mut d[i], lm.mix_mode, factor(i));
                    }
                }
                (AttrType::ByteColor, AttrData::ByteColor(s), AttrData::ByteColor(d)) => {
                    let d = Arc::make_mut(d);
                    for (i, it) in remap.items.iter().enumerate() {
                        if it.sources.is_empty() || i >= d.len() {
                            continue;
                        }
                        let mut r = [0.0f32; 4];
                        for &(k, w) in &it.sources {
                            for c in 0..4 {
                                r[c] += f32::from(s[k][c]) * w;
                            }
                        }
                        let col = r.map(round_fl_to_uchar_clamp);
                        copy_value_mloopcol(col, &mut d[i], lm.mix_mode, factor(i));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_mix_modes() {
        assert_eq!(interp_float_do(CDT_MIX_TRANSFER, 0.2, 0.8, 1.0), 0.8);
        assert!((interp_float_do(CDT_MIX_TRANSFER, 0.2, 0.8, 0.5) - 0.5).abs() < 1e-6);
        assert!((interp_float_do(CDT_MIX_ADD, 0.2, 0.3, 1.0) - 0.5).abs() < 1e-6);
        assert!((interp_float_do(CDT_MIX_MUL, 0.5, 0.5, 1.0) - 0.25).abs() < 1e-6);
        assert!((interp_float_do(CDT_MIX_MIX, 0.2, 0.4, 1.0) - 0.3).abs() < 1e-6);
        // Thresholds keep the destination on the wrong side, replace (ignoring the factor)
        // on the right one.
        assert_eq!(
            interp_float_do(CDT_MIX_REPLACE_ABOVE_THRESHOLD, 0.2, 0.9, 0.5),
            0.2
        );
        assert_eq!(
            interp_float_do(CDT_MIX_REPLACE_ABOVE_THRESHOLD, 0.7, 0.9, 0.5),
            0.9
        );
        assert_eq!(
            interp_float_do(CDT_MIX_REPLACE_BELOW_THRESHOLD, 0.7, 0.9, 0.5),
            0.7
        );
    }

    #[test]
    fn vgroup_interp_adds_only_with_sources() {
        let lm = LayerMap {
            item: Item::Vgroup { src: 0, dst: 1 },
            mix_mode: CDT_MIX_TRANSFER,
            mix_factor: 1.0,
        };
        let a = DeformVert {
            weights: smallvec::smallvec![(0, 0.5)],
        };
        let b = DeformVert {
            weights: smallvec::smallvec![(2, 1.0)],
        };
        let mut d = DeformVert::default();
        vgroup_interp(&lm, 0, 1, &mut d, Some(&[&a, &b]), &[0.5, 0.5], 1.0);
        assert_eq!(d.weights.as_slice(), &[(1, 0.25)]);
        let mut d = DeformVert::default();
        vgroup_interp(&lm, 0, 1, &mut d, Some(&[&b]), &[1.0], 1.0);
        assert!(d.weights.is_empty(), "no source weight, no new entry");
        let lm_above = LayerMap {
            mix_mode: CDT_MIX_REPLACE_ABOVE_THRESHOLD,
            ..lm
        };
        let mut d = DeformVert::default();
        vgroup_interp(&lm_above, 0, 1, &mut d, Some(&[&a]), &[1.0], 0.1);
        assert!(d.weights.is_empty(), "ABOVE_THRESHOLD never adds");
    }

    #[test]
    fn color_copy_values() {
        let mut d = [0.5, 0.5, 0.5, 1.0];
        copy_value_propcol([1.0, 0.0, 0.0, 1.0], &mut d, CDT_MIX_TRANSFER, 0.5);
        assert_eq!(d, [0.75, 0.25, 0.25, 1.0]);
        let mut d = [0.1, 0.1, 0.1, 1.0];
        copy_value_propcol(
            [1.0, 1.0, 1.0, 1.0],
            &mut d,
            CDT_MIX_REPLACE_ABOVE_THRESHOLD,
            0.5,
        );
        assert_eq!(d, [0.1, 0.1, 0.1, 1.0]);
        let mut d = [10, 20, 30, 255];
        copy_value_mloopcol([200, 100, 0, 255], &mut d, CDT_MIX_NOMIX, 1.0);
        assert_eq!(d, [200, 100, 0, 255]);
    }
}
