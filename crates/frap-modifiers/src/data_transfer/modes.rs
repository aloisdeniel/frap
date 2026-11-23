// SPDX-License-Identifier: GPL-3.0-or-later
//! The enums of the data transfer with Blender's values and their RNA identifiers:
//! `eMeshRemapMode` (`MREMAP_*`), `eDTType` (`DT_TYPE_*`), the layer selections
//! (`DT_LAYERS_*`), `DT_MULTILAYER_INDEX_*` and the mix modes (`CDT_MIX_*`).
//!
//! Values from blender/source/blender/makesdna/DNA_modifier_enums.h, identifiers from
//! blender/source/blender/makesrna/intern/rna_modifier.cc (`rna_enum_dt_method_*_items`,
//! `rna_enum_dt_layers_select_*_items`, `rna_enum_dt_mix_mode_items`), Blender 5.2.4.

#![allow(missing_docs)]

pub const MREMAP_USE_VERT: u32 = 1 << 4;
pub const MREMAP_USE_EDGE: u32 = 1 << 5;
pub const MREMAP_USE_LOOP: u32 = 1 << 6;
pub const MREMAP_USE_POLY: u32 = 1 << 7;
pub const MREMAP_USE_NEAREST: u32 = 1 << 8;
pub const MREMAP_USE_NORPROJ: u32 = 1 << 9;
pub const MREMAP_USE_INTERP: u32 = 1 << 10;
pub const MREMAP_USE_NORMAL: u32 = 1 << 11;

pub const MREMAP_MODE_VERT: u32 = 1 << 24;
pub const MREMAP_MODE_VERT_NEAREST: u32 = MREMAP_MODE_VERT | MREMAP_USE_VERT | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_VERT_EDGE_NEAREST: u32 =
    MREMAP_MODE_VERT | MREMAP_USE_EDGE | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_VERT_EDGEINTERP_NEAREST: u32 =
    MREMAP_MODE_VERT | MREMAP_USE_EDGE | MREMAP_USE_NEAREST | MREMAP_USE_INTERP;
pub const MREMAP_MODE_VERT_FACE_NEAREST: u32 =
    MREMAP_MODE_VERT | MREMAP_USE_POLY | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_VERT_POLYINTERP_NEAREST: u32 =
    MREMAP_MODE_VERT | MREMAP_USE_POLY | MREMAP_USE_NEAREST | MREMAP_USE_INTERP;
pub const MREMAP_MODE_VERT_POLYINTERP_VNORPROJ: u32 =
    MREMAP_MODE_VERT | MREMAP_USE_POLY | MREMAP_USE_NORPROJ | MREMAP_USE_INTERP;

pub const MREMAP_MODE_EDGE: u32 = 1 << 25;
pub const MREMAP_MODE_EDGE_VERT_NEAREST: u32 =
    MREMAP_MODE_EDGE | MREMAP_USE_VERT | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_EDGE_NEAREST: u32 = MREMAP_MODE_EDGE | MREMAP_USE_EDGE | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_EDGE_POLY_NEAREST: u32 =
    MREMAP_MODE_EDGE | MREMAP_USE_POLY | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_EDGE_EDGEINTERP_VNORPROJ: u32 =
    MREMAP_MODE_EDGE | MREMAP_USE_VERT | MREMAP_USE_NORPROJ | MREMAP_USE_INTERP;

pub const MREMAP_MODE_LOOP: u32 = 1 << 26;
pub const MREMAP_MODE_LOOP_NEAREST_LOOPNOR: u32 =
    MREMAP_MODE_LOOP | MREMAP_USE_LOOP | MREMAP_USE_VERT | MREMAP_USE_NEAREST | MREMAP_USE_NORMAL;
pub const MREMAP_MODE_LOOP_NEAREST_POLYNOR: u32 =
    MREMAP_MODE_LOOP | MREMAP_USE_POLY | MREMAP_USE_VERT | MREMAP_USE_NEAREST | MREMAP_USE_NORMAL;
pub const MREMAP_MODE_LOOP_POLY_NEAREST: u32 =
    MREMAP_MODE_LOOP | MREMAP_USE_POLY | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_LOOP_POLYINTERP_NEAREST: u32 =
    MREMAP_MODE_LOOP | MREMAP_USE_POLY | MREMAP_USE_NEAREST | MREMAP_USE_INTERP;
pub const MREMAP_MODE_LOOP_POLYINTERP_LNORPROJ: u32 =
    MREMAP_MODE_LOOP | MREMAP_USE_POLY | MREMAP_USE_NORPROJ | MREMAP_USE_INTERP;

pub const MREMAP_MODE_POLY: u32 = 1 << 27;
pub const MREMAP_MODE_POLY_NEAREST: u32 = MREMAP_MODE_POLY | MREMAP_USE_POLY | MREMAP_USE_NEAREST;
pub const MREMAP_MODE_POLY_NOR: u32 = MREMAP_MODE_POLY | MREMAP_USE_POLY | MREMAP_USE_NORMAL;
pub const MREMAP_MODE_POLY_POLYINTERP_PNORPROJ: u32 =
    MREMAP_MODE_POLY | MREMAP_USE_POLY | MREMAP_USE_NORPROJ | MREMAP_USE_INTERP;

pub const MREMAP_MODE_TOPOLOGY: u32 =
    MREMAP_MODE_VERT | MREMAP_MODE_EDGE | MREMAP_MODE_LOOP | MREMAP_MODE_POLY;

pub const DT_TYPE_MDEFORMVERT: u32 = 1 << 0;
pub const DT_TYPE_SKIN: u32 = 1 << 2;
pub const DT_TYPE_BWEIGHT_VERT: u32 = 1 << 3;
pub const DT_TYPE_SHARP_EDGE: u32 = 1 << 8;
pub const DT_TYPE_SEAM: u32 = 1 << 9;
pub const DT_TYPE_CREASE: u32 = 1 << 10;
pub const DT_TYPE_BWEIGHT_EDGE: u32 = 1 << 11;
pub const DT_TYPE_FREESTYLE_EDGE: u32 = 1 << 12;
pub const DT_TYPE_MPROPCOL_VERT: u32 = 1 << 16;
pub const DT_TYPE_LNOR: u32 = 1 << 17;
pub const DT_TYPE_UV: u32 = 1 << 24;
pub const DT_TYPE_SHARP_FACE: u32 = 1 << 25;
pub const DT_TYPE_FREESTYLE_FACE: u32 = 1 << 26;
pub const DT_TYPE_MLOOPCOL_VERT: u32 = 1 << 27;
pub const DT_TYPE_MPROPCOL_LOOP: u32 = 1 << 28;
pub const DT_TYPE_MLOOPCOL_LOOP: u32 = 1 << 29;
pub const DT_TYPE_MAX: u32 = 30;
pub const DT_TYPE_VERT_ALL: u32 = DT_TYPE_MDEFORMVERT
    | DT_TYPE_SKIN
    | DT_TYPE_BWEIGHT_VERT
    | DT_TYPE_MPROPCOL_VERT
    | DT_TYPE_MLOOPCOL_VERT;
pub const DT_TYPE_EDGE_ALL: u32 = DT_TYPE_SHARP_EDGE
    | DT_TYPE_SEAM
    | DT_TYPE_CREASE
    | DT_TYPE_BWEIGHT_EDGE
    | DT_TYPE_FREESTYLE_EDGE;
pub const DT_TYPE_LOOP_ALL: u32 =
    DT_TYPE_LNOR | DT_TYPE_UV | DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP;
pub const DT_TYPE_POLY_ALL: u32 = DT_TYPE_UV | DT_TYPE_SHARP_FACE | DT_TYPE_FREESTYLE_FACE;

/// `DT_DATATYPE_IS_VERT`
pub fn is_vert(t: u32) -> bool {
    matches!(
        t,
        DT_TYPE_MDEFORMVERT | DT_TYPE_SKIN | DT_TYPE_BWEIGHT_VERT | DT_TYPE_MLOOPCOL_VERT
    ) || t == DT_TYPE_MPROPCOL_VERT
        || t == DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT
}

/// `DT_DATATYPE_IS_EDGE`
pub fn is_edge(t: u32) -> bool {
    matches!(
        t,
        DT_TYPE_CREASE | DT_TYPE_SHARP_EDGE | DT_TYPE_SEAM | DT_TYPE_BWEIGHT_EDGE
    ) || t == DT_TYPE_FREESTYLE_EDGE
}

/// `DT_DATATYPE_IS_LOOP`
pub fn is_loop(t: u32) -> bool {
    matches!(t, DT_TYPE_UV | DT_TYPE_LNOR | DT_TYPE_MLOOPCOL_LOOP)
        || t == DT_TYPE_MPROPCOL_LOOP
        || t == DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP
}

/// `DT_DATATYPE_IS_FACE`
pub fn is_face(t: u32) -> bool {
    matches!(t, DT_TYPE_UV | DT_TYPE_SHARP_FACE | DT_TYPE_FREESTYLE_FACE)
}

/// `DT_DATATYPE_IS_MULTILAYERS`
pub fn is_multilayers(t: u32) -> bool {
    t == DT_TYPE_MDEFORMVERT
        || t == DT_TYPE_UV
        || (t != 0 && t & !(DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT) == 0)
        || (t != 0 && t & !(DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP) == 0)
}

pub const DT_MULTILAYER_INDEX_INVALID: i32 = -1;
pub const DT_MULTILAYER_INDEX_MDEFORMVERT: i32 = 0;
pub const DT_MULTILAYER_INDEX_VCOL_LOOP: i32 = 2;
pub const DT_MULTILAYER_INDEX_UV: i32 = 3;
pub const DT_MULTILAYER_INDEX_VCOL_VERT: i32 = 4;
pub const DT_MULTILAYER_INDEX_MAX: usize = 5;

pub const DT_LAYERS_ACTIVE_SRC: i32 = -1;
pub const DT_LAYERS_ALL_SRC: i32 = -2;
pub const DT_LAYERS_VGROUP_SRC: i32 = 1 << 8;
pub const DT_LAYERS_VGROUP_SRC_BONE_SELECT: i32 = -(DT_LAYERS_VGROUP_SRC | 1);
pub const DT_LAYERS_VGROUP_SRC_BONE_DEFORM: i32 = -(DT_LAYERS_VGROUP_SRC | 2);
pub const DT_LAYERS_ACTIVE_DST: i32 = -1;
pub const DT_LAYERS_NAME_DST: i32 = -2;
pub const DT_LAYERS_INDEX_DST: i32 = -3;

pub const CDT_MIX_NOMIX: i32 = -1;
pub const CDT_MIX_TRANSFER: i32 = 0;
pub const CDT_MIX_REPLACE_ABOVE_THRESHOLD: i32 = 1;
pub const CDT_MIX_REPLACE_BELOW_THRESHOLD: i32 = 2;
pub const CDT_MIX_MIX: i32 = 16;
pub const CDT_MIX_ADD: i32 = 17;
pub const CDT_MIX_SUB: i32 = 18;
pub const CDT_MIX_MUL: i32 = 19;

/// `BKE_object_data_transfer_dttype_to_srcdst_index`
pub fn srcdst_index(t: u32) -> i32 {
    match t {
        DT_TYPE_MDEFORMVERT => DT_MULTILAYER_INDEX_MDEFORMVERT,
        DT_TYPE_UV => DT_MULTILAYER_INDEX_UV,
        DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT => DT_MULTILAYER_INDEX_VCOL_VERT,
        x if x == DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT => DT_MULTILAYER_INDEX_VCOL_VERT,
        DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP => DT_MULTILAYER_INDEX_VCOL_LOOP,
        x if x == DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP => DT_MULTILAYER_INDEX_VCOL_LOOP,
        _ => DT_MULTILAYER_INDEX_INVALID,
    }
}

/// The `DT_TYPE_*` bits of a data-type identifier (`data_types_*` items and the operators'
/// `data_type`); `COLOR_VERTEX` / `COLOR_CORNER` are two bits each.
pub fn data_type_bits(id: &str) -> u32 {
    match id {
        "VGROUP_WEIGHTS" => DT_TYPE_MDEFORMVERT,
        "BEVEL_WEIGHT_VERT" => DT_TYPE_BWEIGHT_VERT,
        "COLOR_VERTEX" => DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT,
        "SHARP_EDGE" => DT_TYPE_SHARP_EDGE,
        "SEAM" => DT_TYPE_SEAM,
        "CREASE" => DT_TYPE_CREASE,
        "BEVEL_WEIGHT_EDGE" => DT_TYPE_BWEIGHT_EDGE,
        "FREESTYLE_EDGE" => DT_TYPE_FREESTYLE_EDGE,
        "CUSTOM_NORMAL" => DT_TYPE_LNOR,
        "COLOR_CORNER" => DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP,
        "UV" => DT_TYPE_UV,
        "SMOOTH" => DT_TYPE_SHARP_FACE,
        "FREESTYLE_FACE" => DT_TYPE_FREESTYLE_FACE,
        _ => 0,
    }
}

/// `rna_enum_dt_method_vertex_items`
pub fn vert_mapping(id: &str) -> u32 {
    match id {
        "TOPOLOGY" => MREMAP_MODE_TOPOLOGY,
        "EDGE_NEAREST" => MREMAP_MODE_VERT_EDGE_NEAREST,
        "EDGEINTERP_NEAREST" => MREMAP_MODE_VERT_EDGEINTERP_NEAREST,
        "POLY_NEAREST" => MREMAP_MODE_VERT_FACE_NEAREST,
        "POLYINTERP_NEAREST" => MREMAP_MODE_VERT_POLYINTERP_NEAREST,
        "POLYINTERP_VNORPROJ" => MREMAP_MODE_VERT_POLYINTERP_VNORPROJ,
        _ => MREMAP_MODE_VERT_NEAREST,
    }
}

/// `rna_enum_dt_method_edge_items`
pub fn edge_mapping(id: &str) -> u32 {
    match id {
        "TOPOLOGY" => MREMAP_MODE_TOPOLOGY,
        "VERT_NEAREST" => MREMAP_MODE_EDGE_VERT_NEAREST,
        "POLY_NEAREST" => MREMAP_MODE_EDGE_POLY_NEAREST,
        "EDGEINTERP_VNORPROJ" => MREMAP_MODE_EDGE_EDGEINTERP_VNORPROJ,
        _ => MREMAP_MODE_EDGE_NEAREST,
    }
}

/// `rna_enum_dt_method_loop_items`
pub fn loop_mapping(id: &str) -> u32 {
    match id {
        "TOPOLOGY" => MREMAP_MODE_TOPOLOGY,
        "NEAREST_NORMAL" => MREMAP_MODE_LOOP_NEAREST_LOOPNOR,
        "NEAREST_POLY" => MREMAP_MODE_LOOP_POLY_NEAREST,
        "POLYINTERP_NEAREST" => MREMAP_MODE_LOOP_POLYINTERP_NEAREST,
        "POLYINTERP_LNORPROJ" => MREMAP_MODE_LOOP_POLYINTERP_LNORPROJ,
        _ => MREMAP_MODE_LOOP_NEAREST_POLYNOR,
    }
}

/// `rna_enum_dt_method_poly_items`
pub fn poly_mapping(id: &str) -> u32 {
    match id {
        "TOPOLOGY" => MREMAP_MODE_TOPOLOGY,
        "NORMAL" => MREMAP_MODE_POLY_NOR,
        "POLYINTERP_PNORPROJ" => MREMAP_MODE_POLY_POLYINTERP_PNORPROJ,
        _ => MREMAP_MODE_POLY_NEAREST,
    }
}

/// `rna_enum_dt_layers_select_src_items`
pub fn layers_select_src(id: &str) -> i32 {
    match id {
        "ACTIVE" => DT_LAYERS_ACTIVE_SRC,
        "BONE_SELECT" => DT_LAYERS_VGROUP_SRC_BONE_SELECT,
        "BONE_DEFORM" => DT_LAYERS_VGROUP_SRC_BONE_DEFORM,
        _ => DT_LAYERS_ALL_SRC,
    }
}

/// `rna_enum_dt_layers_select_dst_items`
pub fn layers_select_dst(id: &str) -> i32 {
    match id {
        "ACTIVE" => DT_LAYERS_ACTIVE_DST,
        "INDEX" => DT_LAYERS_INDEX_DST,
        _ => DT_LAYERS_NAME_DST,
    }
}

/// `rna_enum_dt_mix_mode_items`
pub fn mix_mode(id: &str) -> i32 {
    match id {
        "ABOVE_THRESHOLD" => CDT_MIX_REPLACE_ABOVE_THRESHOLD,
        "BELOW_THRESHOLD" => CDT_MIX_REPLACE_BELOW_THRESHOLD,
        "MIX" => CDT_MIX_MIX,
        "ADD" => CDT_MIX_ADD,
        "SUB" => CDT_MIX_SUB,
        "MUL" => CDT_MIX_MUL,
        _ => CDT_MIX_TRANSFER,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_type_classes_follow_blender() {
        assert!(is_vert(DT_TYPE_MPROPCOL_VERT | DT_TYPE_MLOOPCOL_VERT));
        assert!(is_loop(DT_TYPE_UV) && is_face(DT_TYPE_UV));
        assert!(!is_vert(DT_TYPE_UV));
        assert!(is_multilayers(DT_TYPE_MLOOPCOL_LOOP));
        assert!(!is_multilayers(DT_TYPE_SHARP_EDGE));
        assert_eq!(
            srcdst_index(DT_TYPE_MPROPCOL_LOOP | DT_TYPE_MLOOPCOL_LOOP),
            2
        );
        assert_eq!(srcdst_index(DT_TYPE_MLOOPCOL_VERT), 4);
        assert_eq!(DT_LAYERS_VGROUP_SRC_BONE_SELECT, -257);
        assert_eq!(MREMAP_MODE_TOPOLOGY & MREMAP_USE_EDGE, 0);
    }
}
