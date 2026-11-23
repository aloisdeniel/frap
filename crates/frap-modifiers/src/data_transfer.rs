// SPDX-License-Identifier: GPL-3.0-or-later
//! The data-transfer core shared by the DATA_TRANSFER modifier (`types/data_transfer.rs`) and
//! the `object.data_transfer` / `object.datalayout_transfer` operators (WP 8.3.13): element
//! mapping between a source and a destination mesh ([`remap`]: every `vert_mapping`,
//! `edge_mapping`, `loop_mapping` and `poly_mapping` mode, UV islands and their A* inner-cut
//! correction), layer matching and the mixing of the transferred values ([`layers`]: vertex
//! groups, bevel weights, colour attributes, sharp/seam/crease/freestyle marks, UV maps, custom
//! normals, smooth faces).
//!
//! Ported from blender/source/blender/blenkernel/intern/data_transfer.cc
//! (`BKE_object_data_transfer_ex`, `BKE_object_data_transfer_layout`,
//! `data_transfer_dtdata_type_postprocess`, `BKE_object_data_transfer_get_dttypes_capacity`,
//! `BKE_object_data_transfer_get_dttypes_item_types`) and
//! blender/source/blender/blenkernel/intern/deform.cc
//! (`BKE_defvert_extract_vgroup_to_{vert,edge,loop,face}weights`), Blender 5.2.4,
//! GPL-2.0-or-later. The pieces are in `data_transfer/` (WP 8.3.13).
//!
//! Skin data (`DT_TYPE_SKIN`, not offered by the RNA enums) is not bound in frap.

mod astar;
pub(crate) mod bvhutils;
mod islands;
mod layers;
pub(crate) mod math;
pub mod modes;
mod remap;
mod space;

use frap_mesh::{DeformVert, Mesh};
use smol_str::SmolStr;

use self::layers::{CdType, Dst, Elem, LayerMap, Src};
use self::math::V3;
use self::modes::*;
use self::remap::{DstFaces, SrcMesh};
pub use self::space::SpaceTransform;

/// A 4×4 matrix in Blender's `float[4][4]` layout (`m[col][row]`).
pub type Mat4 = [[f32; 4]; 4];

/// What a transfer does (`BKE_object_data_transfer_ex` parameters).
#[derive(Clone, Debug)]
pub struct TransferParams {
    /// `DT_TYPE_*` bits.
    pub data_types: u32,
    /// Add missing destination layers (always false for the modifier).
    pub use_create: bool,
    /// `MREMAP_MODE_*` of each element type ([`modes::vert_mapping`] etc.).
    pub vert_mode: u32,
    pub edge_mode: u32,
    pub loop_mode: u32,
    pub poly_mode: u32,
    /// Destination → source object space (`use_object_transform`), `None` for local space.
    pub space_transform: Option<SpaceTransform>,
    /// `use_auto_transform`: the best-match transform replaces `space_transform`.
    pub auto_transform: bool,
    /// `max_distance` (`f32::MAX` without `use_max_distance`).
    pub max_distance: f32,
    /// `ray_radius`
    pub ray_radius: f32,
    /// `islands_precision` (0 for the modifier).
    pub islands_precision: f32,
    /// `layers_select_src` per `DT_MULTILAYER_INDEX_*` (`DT_LAYERS_*` codes).
    pub layers_src: [i32; DT_MULTILAYER_INDEX_MAX],
    /// `layers_select_dst` per `DT_MULTILAYER_INDEX_*`.
    pub layers_dst: [i32; DT_MULTILAYER_INDEX_MAX],
    /// `CDT_MIX_*`
    pub mix_mode: i32,
    /// `mix_factor`
    pub mix_factor: f32,
    /// The destination vertex group scaling the factor (the modifier's `vertex_group`).
    pub vgroup_name: Option<String>,
    /// `invert_vertex_group`
    pub invert_vgroup: bool,
}

/// The source object of a transfer.
pub struct Source<'a> {
    /// Its evaluated mesh.
    pub mesh: &'a Mesh,
    /// Its vertex-group names (`BKE_object_defgroup_list`).
    pub vgroup_names: &'a [SmolStr],
    /// `vertex_groups.active_index` (`-1` without groups).
    pub active_vgroup: i32,
    /// The groups named after selected pose bones (`BONE_SELECT`), when the object has a
    /// pose armature.
    pub bone_select: Option<Vec<bool>>,
    /// The groups named after deform bones (`BONE_DEFORM`), when the object has an armature.
    pub bone_deform: Option<Vec<bool>>,
    /// Its world matrix (the auto transform of meshes without vertices).
    pub matrix_world: Mat4,
}

/// The destination object of a transfer.
pub struct Destination<'a> {
    /// The mesh written: the evaluated mesh of the modifier, the object's data for the
    /// operators.
    pub mesh: &'a mut Mesh,
    /// The object's `vertex_groups.active_index` (new groups become active).
    pub active_vgroup: &'a mut i32,
    /// Its world matrix.
    pub matrix_world: Mat4,
    /// The modifier case (`me_dst` given): never creates layers.
    pub is_modifier: bool,
}

/// What a transfer did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Outcome {
    /// Whether any layer was transferred.
    pub changed: bool,
    /// The ERROR reports (Blender's `ReportList`), in order.
    pub errors: Vec<String>,
}

/// `BKE_defvert_extract_vgroup_to_vertweights`
fn vert_weights(dverts: &[DeformVert], g: u32, verts_num: usize, invert: bool) -> Vec<f32> {
    (0..verts_num)
        .map(|i| {
            let w = dverts
                .get(i)
                .and_then(|dv| dv.weights.iter().find(|x| x.0 == g))
                .map_or(0.0, |x| x.1);
            if invert { 1.0 - w } else { w }
        })
        .collect()
}

/// The `src`/`dst` layer selection of one data type.
fn layer_select(p: &TransferParams, dtdata_type: u32) -> (i32, i32) {
    let idx = srcdst_index(dtdata_type);
    if idx != DT_MULTILAYER_INDEX_INVALID {
        (p.layers_src[idx as usize], p.layers_dst[idx as usize])
    } else {
        (0, 0)
    }
}

/// `BKE_object_data_transfer_ex`: transfers `p.data_types` from `src` to `dst`.
pub fn transfer(p: &TransferParams, src: &Source<'_>, dst: &mut Destination<'_>) -> Outcome {
    let mut out = Outcome::default();
    let use_create = p.use_create && !dst.is_modifier;
    let use_delete = false;
    let me_src = src.mesh;

    // The destination group scaling the mix factor (its weights are read when each element
    // type first needs them, as Blender reads `mdef` then).
    let vg_index: Option<u32> = p.vgroup_name.as_deref().and_then(|name| {
        dst.mesh.deform_verts()?;
        let g = dst.mesh.vertex_group_names.iter().position(|n| n == name)?;
        Some(g as u32)
    });
    let vg_weights = |m: &Mesh| -> Option<Vec<f32>> {
        let g = vg_index?;
        Some(vert_weights(
            m.deform_verts().unwrap_or(&[]),
            g,
            m.verts_num(),
            p.invert_vgroup,
        ))
    };

    let mut st = p.space_transform;
    if p.auto_transform {
        st = Some(if me_src.verts_num() != 0 && dst.mesh.verts_num() != 0 {
            space::find_best_match(dst.mesh.positions(), me_src.positions())
        } else {
            SpaceTransform::from_matrices(&dst.matrix_world, &src.matrix_world)
        });
    }
    let st = st.as_ref();

    let srcm = SrcMesh::new(me_src);
    let lsrc = Src {
        mesh: me_src,
        vgroup_names: src.vgroup_names,
        active_vgroup: src.active_vgroup,
        bone_select: src.bone_select.clone(),
        bone_deform: src.bone_deform.clone(),
    };
    let mut geom_map: [Option<remap::Remap>; 4] = [None, None, None, None];
    let mut weights: [Option<Vec<f32>>; 4] = [None, None, None, None];
    let mut normals: Option<Vec<V3>> = None;
    let mut lay_map: Vec<LayerMap> = Vec::new();

    for i in 0..DT_TYPE_MAX {
        let dtdata_type = 1u32 << i;
        if p.data_types & dtdata_type == 0 {
            continue;
        }
        let cd = layers::dttype_to_cdtype(dtdata_type);
        let (fromlayers, tolayers) = layer_select(p, dtdata_type);

        if is_vert(dtdata_type) {
            if geom_map[0].is_none() {
                let (nd, ns) = (dst.mesh.verts_num(), me_src.verts_num());
                if p.vert_mode == MREMAP_MODE_TOPOLOGY && nd != ns {
                    out.errors.push(
                        "Source and destination meshes do not have the same number of vertices, \
                         'Topology' mapping cannot be used in this case"
                            .into(),
                    );
                    continue;
                }
                if p.vert_mode & MREMAP_USE_EDGE != 0 && me_src.edges_num() == 0 {
                    out.errors.push(
                        "Source mesh does not have any edges, none of the 'Edge' mappings can be \
                         used in this case"
                            .into(),
                    );
                    continue;
                }
                if p.vert_mode & MREMAP_USE_POLY != 0 && me_src.faces_num() == 0 {
                    out.errors.push(
                        "Source mesh does not have any faces, none of the 'Face' mappings can be \
                         used in this case"
                            .into(),
                    );
                    continue;
                }
                if nd == 0 || ns == 0 {
                    out.errors.push(
                        "Source or destination meshes do not have any vertices, cannot transfer \
                         vertex data"
                            .into(),
                    );
                    continue;
                }
                let positions_dst = dst.mesh.positions().to_vec();
                geom_map[0] = Some(remap::calc_verts(
                    p.vert_mode,
                    st,
                    p.max_distance,
                    p.ray_radius,
                    &positions_dst,
                    &srcm,
                    dst.mesh,
                ));
            }
            if weights[0].is_none() {
                weights[0] = vg_weights(dst.mesh);
            }
            run_layers(
                &mut out,
                &mut lay_map,
                &lsrc,
                dst,
                Elem::Vert,
                cd,
                p,
                use_create,
                use_delete,
                fromlayers,
                tolayers,
                &geom_map[0],
                weights[0].as_deref(),
                st,
                &mut normals,
            );
        }
        if is_edge(dtdata_type) {
            if geom_map[1].is_none() {
                let (nd, ns) = (dst.mesh.edges_num(), me_src.edges_num());
                if p.edge_mode == MREMAP_MODE_TOPOLOGY && nd != ns {
                    out.errors.push(
                        "Source and destination meshes do not have the same number of edges, \
                         'Topology' mapping cannot be used in this case"
                            .into(),
                    );
                    continue;
                }
                if p.edge_mode & MREMAP_USE_POLY != 0 && me_src.faces_num() == 0 {
                    out.errors.push(
                        "Source mesh does not have any faces, none of the 'Face' mappings can be \
                         used in this case"
                            .into(),
                    );
                    continue;
                }
                if nd == 0 || ns == 0 {
                    out.errors.push(
                        "Source or destination meshes do not have any edges, cannot transfer \
                         edge data"
                            .into(),
                    );
                    continue;
                }
                let positions_dst = dst.mesh.positions().to_vec();
                let edges_dst = dst.mesh.edges().to_vec();
                geom_map[1] = Some(remap::calc_edges(
                    p.edge_mode,
                    st,
                    p.max_distance,
                    p.ray_radius,
                    &positions_dst,
                    &edges_dst,
                    &srcm,
                    dst.mesh,
                ));
            }
            if weights[1].is_none()
                && let Some(vw) = vg_weights(dst.mesh)
            {
                weights[1] = Some(
                    dst.mesh
                        .edges()
                        .iter()
                        .map(|e| (vw[e[0] as usize] + vw[e[1] as usize]) * 0.5)
                        .collect(),
                );
            }
            run_layers(
                &mut out,
                &mut lay_map,
                &lsrc,
                dst,
                Elem::Edge,
                cd,
                p,
                use_create,
                use_delete,
                fromlayers,
                tolayers,
                &geom_map[1],
                weights[1].as_deref(),
                st,
                &mut normals,
            );
        }
        if is_loop(dtdata_type) {
            let gen_islands = cd == CdType::Uv;
            if geom_map[2].is_none() {
                let (nd, ns) = (dst.mesh.corners_num(), me_src.corners_num());
                if p.loop_mode == MREMAP_MODE_TOPOLOGY && nd != ns {
                    out.errors.push(
                        "Source and destination meshes do not have the same number of face \
                         corners, 'Topology' mapping cannot be used in this case"
                            .into(),
                    );
                    continue;
                }
                if p.loop_mode & MREMAP_USE_EDGE != 0 && me_src.edges_num() == 0 {
                    out.errors.push(
                        "Source mesh does not have any edges, none of the 'Edge' mappings can be \
                         used in this case"
                            .into(),
                    );
                    continue;
                }
                if nd == 0 || ns == 0 {
                    out.errors.push(
                        "Source or destination meshes do not have any faces, cannot transfer \
                         corner data"
                            .into(),
                    );
                    continue;
                }
                let positions = dst.mesh.positions().to_vec();
                let corner_verts = dst.mesh.corner_verts().to_vec();
                let face_offsets = dst.mesh.face_offsets().to_vec();
                let faces = DstFaces {
                    positions: &positions,
                    corner_verts: &corner_verts,
                    face_offsets: &face_offsets,
                };
                geom_map[2] = Some(remap::calc_loops(
                    p.loop_mode,
                    st,
                    p.max_distance,
                    p.ray_radius,
                    dst.mesh,
                    &faces,
                    &srcm,
                    gen_islands,
                    p.islands_precision,
                ));
            }
            if weights[2].is_none()
                && let Some(vw) = vg_weights(dst.mesh)
            {
                weights[2] = Some(
                    dst.mesh
                        .corner_verts()
                        .iter()
                        .map(|&v| vw[v as usize])
                        .collect(),
                );
            }
            if cd == CdType::Lnor && normals.is_none() {
                normals = Some(if p.mix_factor != 1.0 || weights[2].is_some() {
                    dst.mesh.corner_normals().to_vec()
                } else {
                    vec![[0.0; 3]; dst.mesh.corners_num()]
                });
            }
            run_layers(
                &mut out,
                &mut lay_map,
                &lsrc,
                dst,
                Elem::Loop,
                cd,
                p,
                use_create,
                use_delete,
                fromlayers,
                tolayers,
                &geom_map[2],
                weights[2].as_deref(),
                st,
                &mut normals,
            );
        }
        if is_face(dtdata_type) {
            if geom_map[3].is_none() {
                let (nd, ns) = (dst.mesh.faces_num(), me_src.faces_num());
                if p.poly_mode == MREMAP_MODE_TOPOLOGY && nd != ns {
                    out.errors.push(
                        "Source and destination meshes do not have the same number of faces, \
                         'Topology' mapping cannot be used in this case"
                            .into(),
                    );
                    continue;
                }
                if p.poly_mode & MREMAP_USE_EDGE != 0 && me_src.edges_num() == 0 {
                    out.errors.push(
                        "Source mesh does not have any edges, none of the 'Edge' mappings can be \
                         used in this case"
                            .into(),
                    );
                    continue;
                }
                if nd == 0 || ns == 0 {
                    out.errors.push(
                        "Source or destination meshes do not have any faces, cannot transfer \
                         face data"
                            .into(),
                    );
                    continue;
                }
                let positions = dst.mesh.positions().to_vec();
                let corner_verts = dst.mesh.corner_verts().to_vec();
                let face_offsets = dst.mesh.face_offsets().to_vec();
                let faces = DstFaces {
                    positions: &positions,
                    corner_verts: &corner_verts,
                    face_offsets: &face_offsets,
                };
                geom_map[3] = Some(remap::calc_faces(
                    p.poly_mode,
                    st,
                    p.max_distance,
                    p.ray_radius,
                    dst.mesh,
                    &faces,
                    &srcm,
                ));
            }
            if weights[3].is_none()
                && let Some(vw) = vg_weights(dst.mesh)
            {
                let m = &*dst.mesh;
                weights[3] = Some(
                    (0..m.faces_num())
                        .map(|f| {
                            let r = m.face(f);
                            let n = r.len();
                            let mut w = 0.0f32;
                            for &v in &m.corner_verts()[r] {
                                w += vw[v as usize];
                            }
                            w / n as f32
                        })
                        .collect(),
                );
            }
            run_layers(
                &mut out,
                &mut lay_map,
                &lsrc,
                dst,
                Elem::Poly,
                cd,
                p,
                use_create,
                use_delete,
                fromlayers,
                tolayers,
                &geom_map[3],
                weights[3].as_deref(),
                st,
                &mut normals,
            );
        }
        // data_transfer_dtdata_type_postprocess
        if dtdata_type == DT_TYPE_LNOR
            && out.changed
            && let Some(mut nors) = normals.take()
        {
            crate::types::normal_edit::custom_normals::mesh_set_custom_normals(
                dst.mesh, &mut nors, false,
            );
        }
    }
    out
}

/// One element type of one data type: the layer maps, then the values.
#[allow(clippy::too_many_arguments)]
fn run_layers(
    out: &mut Outcome,
    lay_map: &mut Vec<LayerMap>,
    lsrc: &Src<'_>,
    dst: &mut Destination<'_>,
    elem: Elem,
    cd: CdType,
    p: &TransferParams,
    use_create: bool,
    use_delete: bool,
    fromlayers: i32,
    tolayers: i32,
    map: &Option<remap::Remap>,
    weights: Option<&[f32]>,
    st: Option<&SpaceTransform>,
    normals: &mut Option<Vec<V3>>,
) {
    let mut d = Dst {
        mesh: dst.mesh,
        active_vgroup: dst.active_vgroup,
    };
    if layers::generate(
        Some(lay_map),
        lsrc,
        &mut d,
        elem,
        cd,
        p.mix_mode,
        p.mix_factor,
        use_create,
        use_delete,
        fromlayers,
        tolayers,
    ) {
        out.changed |= !lay_map.is_empty();
        if let Some(map) = map {
            for lm in lay_map.iter() {
                layers::apply(map, lm, weights, lsrc.mesh, d.mesh, st, normals);
            }
        }
        lay_map.clear();
    }
}

/// `BKE_object_data_transfer_layout`: creates (and with `use_delete` deletes) the destination
/// layers a transfer of `data_types` needs.
pub fn transfer_layout(
    data_types: u32,
    use_delete: bool,
    layers_src: &[i32; DT_MULTILAYER_INDEX_MAX],
    layers_dst: &[i32; DT_MULTILAYER_INDEX_MAX],
    src: &Source<'_>,
    dst: &mut Destination<'_>,
) {
    let lsrc = Src {
        mesh: src.mesh,
        vgroup_names: src.vgroup_names,
        active_vgroup: src.active_vgroup,
        bone_select: src.bone_select.clone(),
        bone_deform: src.bone_deform.clone(),
    };
    for i in 0..DT_TYPE_MAX {
        let dtdata_type = 1u32 << i;
        if data_types & dtdata_type == 0 {
            continue;
        }
        let cd = layers::dttype_to_cdtype(dtdata_type);
        let idx = srcdst_index(dtdata_type);
        let (fromlayers, tolayers) = if idx != DT_MULTILAYER_INDEX_INVALID {
            (layers_src[idx as usize], layers_dst[idx as usize])
        } else {
            (0, 0)
        };
        let mut d = Dst {
            mesh: dst.mesh,
            active_vgroup: dst.active_vgroup,
        };
        for (is, elem) in [
            (is_vert(dtdata_type), Elem::Vert),
            (is_edge(dtdata_type), Elem::Edge),
            (is_loop(dtdata_type), Elem::Loop),
            (is_face(dtdata_type), Elem::Poly),
        ] {
            if is {
                layers::generate(
                    None, &lsrc, &mut d, elem, cd, 0, 0.0, true, use_delete, fromlayers, tolayers,
                );
            }
        }
        let ty = match cd {
            CdType::PropColor => Some(layers::AttrType::FloatColor),
            CdType::ByteColor => Some(layers::AttrType::ByteColor),
            _ => None,
        };
        layers::transfer_attribute_name_strings(ty, d.mesh, src.mesh);
    }
}

/// `BKE_object_data_transfer_get_dttypes_item_types`: which element kinds (`[vert, edge, loop,
/// face]`) `data_types` touch (the RNA update of `data_types_*` sets `use_*_data` from it).
pub fn dttypes_item_types(data_types: u32) -> [bool; 4] {
    let mut r = [false; 4];
    for i in 0..DT_TYPE_MAX {
        let t = 1u32 << i;
        if data_types & t == 0 {
            continue;
        }
        r[0] |= is_vert(t);
        r[1] |= is_edge(t);
        r[2] |= is_loop(t);
        r[3] |= is_face(t);
    }
    r
}

/// `BKE_object_data_transfer_get_dttypes_capacity`: `(advanced_mixing, threshold)`, the mix
/// modes a data type offers beyond `REPLACE`.
pub fn dttypes_capacity(data_types: u32) -> (bool, bool) {
    let (mut adv, mut thr) = (false, false);
    for i in 0..DT_TYPE_MAX {
        let t = 1u32 << i;
        if data_types & t == 0 {
            continue;
        }
        match t {
            DT_TYPE_MDEFORMVERT => {
                adv = true;
                thr = true;
            }
            DT_TYPE_SKIN
            | DT_TYPE_SHARP_EDGE
            | DT_TYPE_SEAM
            | DT_TYPE_FREESTYLE_EDGE
            | DT_TYPE_SHARP_FACE
            | DT_TYPE_FREESTYLE_FACE => thr = true,
            DT_TYPE_MPROPCOL_VERT
            | DT_TYPE_MLOOPCOL_VERT
            | DT_TYPE_MPROPCOL_LOOP
            | DT_TYPE_MLOOPCOL_LOOP => {
                adv = true;
                thr = true;
            }
            DT_TYPE_LNOR => adv = true,
            _ => {}
        }
    }
    (adv, thr)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::weld::util::unit_m4;

    fn grid(n: usize, z: f32, name: &str) -> Mesh {
        let mut verts = Vec::new();
        for y in 0..n {
            for x in 0..n {
                verts.push([x as f32 * 0.37, y as f32 * 0.41, z]);
            }
        }
        let mut faces = Vec::new();
        for y in 0..n - 1 {
            for x in 0..n - 1 {
                let i = (y * n + x) as i32;
                faces.push(vec![i, i + 1, i + 1 + n as i32, i + n as i32]);
            }
        }
        let mut m = Mesh::new(name);
        m.from_pydata(&verts, &[], &faces, false);
        m
    }

    fn params(data_types: u32) -> TransferParams {
        TransferParams {
            data_types,
            use_create: true,
            vert_mode: MREMAP_MODE_VERT_NEAREST,
            edge_mode: MREMAP_MODE_EDGE_NEAREST,
            loop_mode: MREMAP_MODE_LOOP_NEAREST_POLYNOR,
            poly_mode: MREMAP_MODE_POLY_NEAREST,
            space_transform: None,
            auto_transform: false,
            max_distance: f32::MAX,
            ray_radius: 0.0,
            islands_precision: 0.0,
            layers_src: [DT_LAYERS_ALL_SRC; DT_MULTILAYER_INDEX_MAX],
            layers_dst: [DT_LAYERS_NAME_DST; DT_MULTILAYER_INDEX_MAX],
            mix_mode: CDT_MIX_TRANSFER,
            mix_factor: 1.0,
            vgroup_name: None,
            invert_vgroup: false,
        }
    }

    fn source_with_groups() -> Mesh {
        let mut src = grid(3, 0.0, "S");
        src.vertex_group_names = vec!["G".into(), "H".into()];
        let dv = src.deform_verts_mut();
        dv[0].weights.push((0, 0.25));
        dv[4].weights.push((1, 0.5));
        src
    }

    fn run(
        p: &TransferParams,
        src: &Mesh,
        dst: &mut Mesh,
        active: &mut i32,
        modifier: bool,
    ) -> Outcome {
        let source = Source {
            mesh: src,
            vgroup_names: &src.vertex_group_names,
            active_vgroup: 1,
            bone_select: None,
            bone_deform: None,
            matrix_world: unit_m4(),
        };
        let mut d = Destination {
            mesh: dst,
            active_vgroup: active,
            matrix_world: unit_m4(),
            is_modifier: modifier,
        };
        transfer(p, &source, &mut d)
    }

    #[test]
    fn operator_creates_groups_modifier_does_not() {
        let src = source_with_groups();
        let mut dst = grid(3, 0.1, "D");
        let mut active = -1;
        let out = run(
            &params(DT_TYPE_MDEFORMVERT),
            &src,
            &mut dst,
            &mut active,
            true,
        );
        assert!(!out.changed, "the modifier never creates groups");
        assert!(dst.vertex_group_names.is_empty());
        let out = run(
            &params(DT_TYPE_MDEFORMVERT),
            &src,
            &mut dst,
            &mut active,
            false,
        );
        assert!(out.changed && out.errors.is_empty());
        assert_eq!(dst.vertex_group_names, ["G", "H"]);
        assert_eq!(active, 1, "the last new group is active");
        let dv = dst.deform_verts().unwrap();
        assert_eq!(dv[0].weights.as_slice(), &[(0, 0.25)]);
        assert_eq!(dv[4].weights.as_slice(), &[(1, 0.5)]);
        // The modifier path on a destination that has the groups gives the same weights.
        let mut dst2 = grid(3, 0.1, "D2");
        dst2.vertex_group_names = vec!["G".into(), "H".into()];
        let mut a2 = 0;
        run(&params(DT_TYPE_MDEFORMVERT), &src, &mut dst2, &mut a2, true);
        assert_eq!(dst2.deform_verts(), dst.deform_verts());
    }

    #[test]
    fn topology_error_is_reported() {
        let src = source_with_groups();
        let mut dst = grid(4, 0.0, "D");
        dst.vertex_group_names = vec!["G".into()];
        let mut p = params(DT_TYPE_MDEFORMVERT | DT_TYPE_SHARP_EDGE);
        p.vert_mode = MREMAP_MODE_TOPOLOGY;
        let mut active = 0;
        let out = run(&p, &src, &mut dst, &mut active, false);
        assert_eq!(
            out.errors,
            [
                "Source and destination meshes do not have the same number of vertices, \
              'Topology' mapping cannot be used in this case"
            ]
        );
        // The source has no sharp_edge: nothing to transfer there either.
        assert!(!out.changed);
    }

    #[test]
    fn layout_by_name_with_delete_and_by_index() {
        let src = source_with_groups();
        let source = Source {
            mesh: &src,
            vgroup_names: &src.vertex_group_names,
            active_vgroup: 0,
            bone_select: None,
            bone_deform: None,
            matrix_world: unit_m4(),
        };
        let src_sel = [DT_LAYERS_ALL_SRC; DT_MULTILAYER_INDEX_MAX];
        let mut dst = grid(3, 0.1, "D");
        dst.vertex_group_names = vec!["Extra".into(), "G".into()];
        let mut active = 1;
        let mut d = Destination {
            mesh: &mut dst,
            active_vgroup: &mut active,
            matrix_world: unit_m4(),
            is_modifier: false,
        };
        let by_name = [DT_LAYERS_NAME_DST; DT_MULTILAYER_INDEX_MAX];
        transfer_layout(
            DT_TYPE_MDEFORMVERT,
            true,
            &src_sel,
            &by_name,
            &source,
            &mut d,
        );
        assert_eq!(
            d.mesh.vertex_group_names,
            ["G", "H"],
            "Extra removed, H added"
        );
        let mut dst = grid(3, 0.1, "D");
        let mut d = Destination {
            mesh: &mut dst,
            active_vgroup: &mut active,
            matrix_world: unit_m4(),
            is_modifier: false,
        };
        let by_index = [DT_LAYERS_INDEX_DST; DT_MULTILAYER_INDEX_MAX];
        transfer_layout(
            DT_TYPE_MDEFORMVERT,
            false,
            &src_sel,
            &by_index,
            &source,
            &mut d,
        );
        assert_eq!(d.mesh.vertex_group_names, ["Group", "Group.001"]);
    }

    #[test]
    fn item_types_and_capacity() {
        assert_eq!(dttypes_item_types(DT_TYPE_UV), [false, false, true, true]);
        assert_eq!(
            dttypes_item_types(DT_TYPE_MDEFORMVERT | DT_TYPE_SHARP_EDGE),
            [true, true, false, false]
        );
        assert_eq!(dttypes_capacity(DT_TYPE_LNOR), (true, false));
        assert_eq!(dttypes_capacity(DT_TYPE_CREASE), (false, false));
        assert_eq!(dttypes_capacity(DT_TYPE_SEAM), (false, true));
        assert_eq!(dttypes_capacity(DT_TYPE_MDEFORMVERT), (true, true));
    }
}
