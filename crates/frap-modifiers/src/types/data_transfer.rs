// SPDX-License-Identifier: GPL-3.0-or-later
//! DATA_TRANSFER (`DataTransferModifier`): transfers the layers of `data_types_verts`,
//! `data_types_edges`, `data_types_loops` and `data_types_polys` from the evaluated mesh of
//! `object` (in world space with `use_object_transform`) with the element mappings
//! `vert_mapping` / `edge_mapping` / `loop_mapping` / `poly_mapping`, `use_max_distance` /
//! `max_distance`, `ray_radius`, the layer selections `layers_*_select_src/_dst`, `mix_mode`,
//! `mix_factor` and the destination `vertex_group` (`invert_vertex_group`). The modifier never
//! creates layers: the destination must already have them (`object.datalayout_transfer`).
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_datatransfer.cc (Blender 5.2.4,
//! GPL-2.0-or-later); the transfer itself is `crate::data_transfer` (`BKE_object_data_transfer_ex`
//! with `me_dst`, islands precision 0). Blender's errors (topology mismatch, no edges/faces) are
//! modifier errors that keep the partial result; frap reports them as WARNINGs
//! (`Modifier 'DataTransfer': …`) and returns the same mesh. The evaluator reads the
//! `data_types_*` sets only, as Blender's does (`use_*_data` are UI toggles).

use frap_rna::{IdAny, ObjectId, Report, ReportType};
use frap_scene::generated::modifier_settings::DataTransferSettings;

use super::weld::util::m4_from_glam;
use crate::data_transfer::modes::{self, DT_MULTILAYER_INDEX_MAX};
use crate::data_transfer::{Destination, Source, SpaceTransform, TransferParams, transfer};
use crate::prelude::*;

/// `foreach_ID_link`: the source object.
fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::DataTransfer(d) = s {
        v.extend(d.object);
    }
    v
}

/// `is_disabled`: no source object (a non-mesh one is skipped in `eval`).
fn disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::DataTransfer(d) if d.object.is_none() => {
            Some("DataTransfer: no source object")
        }
        _ => None,
    }
}

/// The `DT_TYPE_*` bits of the modifier's four data-type sets (`dtmd->data_types`).
pub fn data_types(s: &DataTransferSettings) -> u32 {
    [
        &s.data_types_verts,
        &s.data_types_edges,
        &s.data_types_loops,
        &s.data_types_polys,
    ]
    .iter()
    .flat_map(|set| set.iter())
    .fold(0, |acc, id| acc | modes::data_type_bits(id))
}

/// `layers_select_src` / `_dst` by `DT_MULTILAYER_INDEX_*`.
pub fn layer_selects(
    s: &DataTransferSettings,
) -> ([i32; DT_MULTILAYER_INDEX_MAX], [i32; DT_MULTILAYER_INDEX_MAX]) {
    let mut src = [modes::DT_LAYERS_ALL_SRC; DT_MULTILAYER_INDEX_MAX];
    let mut dst = [modes::DT_LAYERS_NAME_DST; DT_MULTILAYER_INDEX_MAX];
    src[modes::DT_MULTILAYER_INDEX_MDEFORMVERT as usize] =
        modes::layers_select_src(&s.layers_vgroup_select_src);
    src[modes::DT_MULTILAYER_INDEX_VCOL_LOOP as usize] =
        modes::layers_select_src(&s.layers_vcol_loop_select_src);
    src[modes::DT_MULTILAYER_INDEX_UV as usize] = modes::layers_select_src(&s.layers_uv_select_src);
    src[modes::DT_MULTILAYER_INDEX_VCOL_VERT as usize] =
        modes::layers_select_src(&s.layers_vcol_vert_select_src);
    dst[modes::DT_MULTILAYER_INDEX_MDEFORMVERT as usize] =
        modes::layers_select_dst(&s.layers_vgroup_select_dst);
    dst[modes::DT_MULTILAYER_INDEX_VCOL_LOOP as usize] =
        modes::layers_select_dst(&s.layers_vcol_loop_select_dst);
    dst[modes::DT_MULTILAYER_INDEX_UV as usize] = modes::layers_select_dst(&s.layers_uv_select_dst);
    dst[modes::DT_MULTILAYER_INDEX_VCOL_VERT as usize] =
        modes::layers_select_dst(&s.layers_vcol_vert_select_dst);
    (src, dst)
}

#[modifier(
    DATA_TRANSFER,
    kind = NonGeometrical,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE,
    depends_on = deps,
    is_disabled = disabled,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::DataTransfer(s) = &md.settings else {
        return Ok(mesh);
    };
    let Some(ob_src) = s.object.and_then(|o| o.typed::<ObjectId>()) else {
        return Ok(mesh);
    };
    let Some(src_ob) = cx.main().objects.get(ob_src) else {
        return Ok(mesh);
    };
    if src_ob.object_type() != "MESH" || ob_src == cx.object_ref() {
        return Ok(mesh);
    }
    let src_active = src_ob.vertex_groups_active_index;
    let Ok(me_src) = cx.evaluated_mesh(ob_src) else {
        return Ok(mesh);
    };
    let ob_world = m4_from_glam(cx.object_matrix_world(cx.object_ref()));
    let src_world = m4_from_glam(cx.object_matrix_world(ob_src));
    let st = s
        .use_object_transform
        .then(|| SpaceTransform::from_matrices(&ob_world, &src_world));
    let (layers_src, layers_dst) = layer_selects(s);
    let params = TransferParams {
        data_types: data_types(s),
        use_create: false,
        vert_mode: modes::vert_mapping(&s.vert_mapping),
        edge_mode: modes::edge_mapping(&s.edge_mapping),
        loop_mode: modes::loop_mapping(&s.loop_mapping),
        poly_mode: modes::poly_mapping(&s.poly_mapping),
        space_transform: st,
        auto_transform: false,
        max_distance: if s.use_max_distance {
            s.max_distance
        } else {
            f32::MAX
        },
        ray_radius: s.ray_radius,
        islands_precision: 0.0,
        layers_src,
        layers_dst,
        mix_mode: modes::mix_mode(&s.mix_mode),
        mix_factor: s.mix_factor,
        vgroup_name: Some(s.vertex_group.clone()),
        invert_vgroup: s.invert_vertex_group,
    };
    let source = Source {
        mesh: &me_src,
        vgroup_names: &me_src.vertex_group_names,
        active_vgroup: src_active,
        bone_select: None,
        bone_deform: None,
        matrix_world: src_world,
    };
    let mut result = mesh;
    let mut active = cx.object().vertex_groups_active_index;
    let mut dst = Destination {
        mesh: &mut result,
        active_vgroup: &mut active,
        matrix_world: ob_world,
        is_modifier: true,
    };
    let out = transfer(&params, &source, &mut dst);
    for e in out.errors {
        cx.report(Report::new(
            ReportType::Warning,
            format!("Modifier '{}': {e}", md.name),
        ));
    }
    Ok(result)
}
