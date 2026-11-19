// SPDX-License-Identifier: GPL-3.0-or-later
//! glTF meshes → one Blender mesh per glTF mesh: every primitive's vertices appended (unique
//! indices per primitive, in index order), TRIANGLES/TRIANGLE_STRIP/TRIANGLE_FAN as triangles,
//! LINES/LINE_STRIP/LINE_LOOP as loose edges, POINTS as loose vertices; UV maps, colour
//! attributes, `_`/`KHR_` custom attributes, material slots, `merge_vertices`,
//! `import_shading`, edges, validation and custom split normals.
//!
//! With `import_point_as_pointcloud`, a glTF mesh whose primitives are all POINTS becomes a
//! `PointCloud` instead (`create_pointcloud`, WP 3.7): one point per distinct index of each
//! primitive (sorted), positions converted to Z-up, every `_`/`KHR_` attribute as a POINT
//! attribute (zeros where a primitive lacks it), the mesh's extras as custom properties, named
//! like the mesh (`PointCloud_<i>` without a name); no material slots.
//!
//! Ported from io_scene_gltf2 5.2.41 `blender/imp/mesh.py` (`create_mesh`, `do_primitives`,
//! `set_poly_smoothing`, `merge_duplicate_verts`, `colors_rgb_to_rgba`, `uvs_gltf_to_blender`,
//! `create_pointcloud`, `do_primitives_pointcloud`), `blender/imp/node.py`
//! (`create_mesh_object`: `is_point_cloud`) and `blender/com/gltf2_blender_utils.py`
//! (`fast_structured_np_unique`: byte-wise unique order), Apache-2.0, The glTF-Blender-IO
//! authors. Skinning and shape keys arrive in M6, Draco/meshopt in M7.

mod custom_normals;
mod validate;

use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use frap_gltf::json::{AccessorType, ComponentType, Mode, Primitive};
use frap_gltf::primitive::{decode_topology, primitive_indices};
use frap_mesh::{
    AttrData, Attribute, Domain, Mesh, SELECT_POLY, SELECT_VERT, SHARP_EDGE, SHARP_FACE,
};
use frap_rna::{IdAny, IdCode, IdRef, MaterialId, MeshId, PointCloudId};
use frap_scene::id::PointCloud;
use frap_scene::{ChangeLog, Main};
use smol_str::SmolStr;

use super::convert::{normals_batch, uvs_batch};
use super::materials::COLOR_0_ATTRIBUTE;
use super::settings::ImportShading;
use super::{Gltf, ImportError, extras, materials};

/// `UV_MAX`, `COLOR_MAX`: TEXCOORD_n / COLOR_n sets imported.
const UV_MAX: usize = 8;
const COLOR_MAX: usize = 8;

/// Whether a glTF mesh gets shape keys (`pymesh.shapekey_names` is non-empty): a primitive
/// has morph targets.
pub(crate) fn has_shape_keys(m: &frap_gltf::json::Mesh) -> bool {
    m.primitives.iter().any(|p| !p.targets.is_empty())
}

/// A custom (`_`/`KHR_`-prefixed) attribute being gathered.
struct Custom {
    name: String,
    components: usize,
    blender_type: Option<&'static str>,
    data: Vec<f32>,
}

/// `get_attribute_type`.
fn custom_attribute_type(ct: ComponentType, ty: AccessorType) -> Option<&'static str> {
    use ComponentType as C;
    match (ty.components(), ct) {
        (1, C::Float) => Some("FLOAT"),
        (1, C::UnsignedByte) => Some("INT"),
        (2, C::Float) => Some("FLOAT2"),
        (3, C::Float) => Some("FLOAT_VECTOR"),
        (4, C::Float) => Some("FLOAT_COLOR"),
        (4, C::UnsignedShort) | (4, C::UnsignedByte) => Some("BYTE_COLOR"),
        (16, C::Float) => Some("FLOAT4X4"),
        _ => None,
    }
}

// Ported from blender/source/blender/blenlib/intern/math_color.cc (linearrgb_to_srgb,
// linearrgb_to_srgb_uchar4) and math_base_inline.c (unit_float_to_uchar_clamp).
fn linear_to_srgb_u8(c: f32) -> u8 {
    let s = if c < 0.003_130_8 {
        if c < 0.0 { 0.0 } else { c * 12.92 }
    } else {
        1.055 * libm::powf(c, 1.0 / 2.4) - 0.055
    };
    unit_float_to_uchar_clamp(s)
}

fn unit_float_to_uchar_clamp(v: f32) -> u8 {
    if v <= 0.0 || v.is_nan() {
        0
    } else if v > 1.0 - 0.5 / 255.0 {
        255
    } else {
        (255.0 * v + 0.5) as u8
    }
}

/// `ByteColorAttributeValue.color = linear`: sRGB bytes, alpha linear.
fn byte_color(c: [f32; 4]) -> [u8; 4] {
    [
        linear_to_srgb_u8(c[0]),
        linear_to_srgb_u8(c[1]),
        linear_to_srgb_u8(c[2]),
        unit_float_to_uchar_clamp(c[3]),
    ]
}

/// The per-primitive results the material and shading passes need.
struct PrimFaces {
    /// Index into `pymesh.primitives`.
    prim: usize,
    /// Triangles contributed (`prim.num_faces`).
    num_faces: usize,
}

/// `np.unique(indices, return_inverse=True)`.
fn unique_inverse(indices: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut unique: Vec<u32> = indices.to_vec();
    unique.sort_unstable();
    unique.dedup();
    let pos: HashMap<u32, u32> = unique
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect();
    let inverse = indices.iter().map(|v| pos[v]).collect();
    (unique, inverse)
}

/// Gathers `src[i]` for every `i` in `idx`, failing on an out-of-range index.
fn gather<T: Copy>(src: &[T], idx: &[u32], what: &str) -> Result<Vec<T>, ImportError> {
    idx.iter()
        .map(|&i| {
            src.get(i as usize).copied().ok_or_else(|| {
                ImportError::Data(format!(
                    "{what} index {i} out of range ({} elements)",
                    src.len()
                ))
            })
        })
        .collect()
}

/// `is_point_cloud` of `create_mesh_object`: `import_point_as_pointcloud` and every primitive
/// of the mesh is POINTS.
pub(crate) fn is_point_cloud(cx: &Gltf, mesh_idx: usize) -> bool {
    cx.settings.import_point_as_pointcloud
        && cx.doc.meshes[mesh_idx]
            .primitives
            .iter()
            .all(|p| p.mode_or_default() == Mode::Points)
}

/// The Blender data of glTF mesh `mesh_idx`: a mesh, or a point cloud ([`is_point_cloud`]).
pub(crate) fn create_data(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    mesh_idx: usize,
) -> Result<IdAny, ImportError> {
    if is_point_cloud(cx, mesh_idx) {
        Ok(create_pointcloud(cx, main, log, mesh_idx)?.any())
    } else {
        Ok(create_mesh(cx, main, log, mesh_idx)?.any())
    }
}

/// The name of a mesh or point cloud made by [`create_data`].
pub(crate) fn data_name(main: &Main, data: IdAny) -> String {
    let name = match data.code {
        IdCode::PT => {
            IdRef::<PointCloudId>::from_any(data).and_then(|r| main.pointclouds.name_of(r))
        }
        _ => IdRef::<MeshId>::from_any(data).and_then(|r| main.meshes.name_of(r)),
    };
    name.unwrap_or("Mesh").to_owned()
}

/// A `_`/`KHR_` attribute of a point cloud being gathered.
struct PcAttr {
    name: String,
    components: usize,
    blender_type: Option<&'static str>,
    data: Vec<f32>,
}

/// `create_pointcloud`: a new point cloud for glTF mesh `mesh_idx`.
pub(crate) fn create_pointcloud(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    mesh_idx: usize,
) -> Result<IdRef<PointCloudId>, ImportError> {
    let pypc = cx.doc.meshes[mesh_idx].clone();
    let name = super::truthy_name(&pypc.name).unwrap_or_else(|| format!("PointCloud_{mesh_idx}"));
    let mut locs: Vec<[f32; 3]> = Vec::new();
    let mut attrs: Vec<PcAttr> = Vec::new();
    for prim in &pypc.primitives {
        let Some(&pos_acc) = prim.attributes.get("POSITION") else {
            continue;
        };
        if prim.extensions.contains("KHR_draco_mesh_compression") {
            return Err(ImportError::NotImplemented(
                "KHR_draco_mesh_compression: Draco-compressed primitives are imported from M7"
                    .into(),
            ));
        }
        if prim.mode_or_default() != Mode::Points {
            let what = super::truthy_name(&pypc.name).unwrap_or_else(|| "[unnamed]".into());
            cx.report(
                frap_rna::ReportType::Warning,
                format!(
                    "Point Cloud import: Primitive mode is not POINTS, skipping primitive in mesh {what}"
                ),
            );
            continue;
        }
        let indices = match prim.indices {
            Some(_) => primitive_indices(&cx.doc, &cx.buffers, prim)?,
            None => {
                let n = cx
                    .doc
                    .accessors
                    .get(pos_acc as usize)
                    .map_or(0, |a| a.count as u32);
                (0..n).collect()
            }
        };
        let (unique, _) = unique_inverse(&indices);
        let before = locs.len();
        let vs = cx.accessor_f32::<3>(pos_acc)?;
        locs.extend(gather(&vs, &unique, "POSITION")?);
        for (k, &acc) in &prim.attributes {
            if !(k.starts_with('_') || k.starts_with("KHR_")) || attrs.iter().any(|a| &a.name == k)
            {
                continue;
            }
            let a = cx
                .doc
                .accessors
                .get(acc as usize)
                .ok_or_else(|| ImportError::Data(format!("accessor {acc} does not exist")))?;
            attrs.push(PcAttr {
                name: k.clone(),
                components: a.type_.components(),
                blender_type: custom_attribute_type(a.component_type, a.type_),
                data: vec![0.0; before * a.type_.components()],
            });
        }
        for a in attrs.iter_mut() {
            match prim.attributes.get(&a.name) {
                Some(&acc) => {
                    let flat = cx.accessor(acc)?.to_f32();
                    for &u in &unique {
                        let s = u as usize * a.components;
                        let e = s + a.components;
                        let v = flat.get(s..e).ok_or_else(|| {
                            ImportError::Data(format!("{} index {u} out of range", a.name))
                        })?;
                        a.data.extend_from_slice(v);
                    }
                }
                None => a
                    .data
                    .extend(std::iter::repeat_n(0.0, unique.len() * a.components)),
            }
        }
    }
    cx.units.locs_batch(&mut locs);
    let mut pc = PointCloud::new("");
    pc.resize(locs.len());
    let n = locs.len();
    let bad = |e: String| ImportError::Data(e);
    pc.set_attribute("position", AttrData::Float3(Arc::new(locs)))
        .map_err(bad)?;
    for a in attrs {
        let Some(ty) = a.blender_type else {
            continue;
        };
        let data = match ty {
            "FLOAT" => AttrData::Float(Arc::new(a.data)),
            "INT" => AttrData::Int(Arc::new(a.data.iter().map(|&v| v as i32).collect())),
            "FLOAT2" => AttrData::Float2(Arc::new(chunks::<2>(&a.data))),
            "FLOAT_VECTOR" => AttrData::Float3(Arc::new(chunks::<3>(&a.data))),
            "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(chunks::<4>(&a.data))),
            "BYTE_COLOR" => AttrData::ByteColor(Arc::new(
                chunks::<4>(&a.data).into_iter().map(byte_color).collect(),
            )),
            "FLOAT4X4" => AttrData::Float4x4(Arc::new(
                chunks::<16>(&a.data)
                    .into_iter()
                    .map(|m| std::array::from_fn(|col| std::array::from_fn(|row| m[col * 4 + row])))
                    .collect(),
            )),
            _ => continue,
        };
        if data.len() == n {
            pc.set_attribute(&a.name, data).map_err(bad)?;
        }
    }
    extras::set_extras(&mut pc.header, pypc.extras.as_ref(), &[]);
    Ok(main.pointclouds.insert(&name, pc, log))
}

/// `create_mesh`: a new Blender mesh for glTF mesh `mesh_idx`.
pub(crate) fn create_mesh(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    mesh_idx: usize,
) -> Result<IdRef<MeshId>, ImportError> {
    let pymesh = cx.doc.meshes[mesh_idx].clone();
    // `pymesh.name or 'Mesh_%d'`
    let name = super::truthy_name(&pymesh.name).unwrap_or_else(|| format!("Mesh_{mesh_idx}"));
    let mut mesh = do_primitives(cx, main, log, &pymesh)?;
    extras::set_extras(&mut mesh.header, pymesh.extras.as_ref(), &["targetNames"]);
    Ok(main.meshes.insert(&name, mesh, log))
}

/// `do_primitives`: builds the mesh data of `pymesh`.
fn do_primitives(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    pymesh: &frap_gltf::json::Mesh,
) -> Result<Mesh, ImportError> {
    let shading = cx.settings.import_shading;
    let with_pos = |p: &&Primitive| p.attributes.contains_key("POSITION");

    // ---- scan
    let mut has_normals = false;
    let mut num_uvs = 0usize;
    let mut num_cols = 0usize;
    let mut customs: Vec<Custom> = Vec::new();
    for prim in pymesh.primitives.iter().filter(with_pos) {
        if shading == ImportShading::Normals && prim.attributes.contains_key("NORMAL") {
            has_normals = true;
        }
        let count = |prefix: &str, max: usize| {
            (0..max)
                .take_while(|i| prim.attributes.contains_key(&format!("{prefix}{i}")))
                .count()
        };
        num_uvs = num_uvs.max(count("TEXCOORD_", UV_MAX));
        num_cols = num_cols.max(count("COLOR_", COLOR_MAX));
        for (k, &acc) in &prim.attributes {
            if (k.starts_with('_') || k.starts_with("KHR_"))
                && !customs.iter().any(|c| &c.name == k)
            {
                let a =
                    cx.doc.accessors.get(acc as usize).ok_or_else(|| {
                        ImportError::Data(format!("accessor {acc} does not exist"))
                    })?;
                customs.push(Custom {
                    name: k.clone(),
                    components: a.type_.components(),
                    blender_type: custom_attribute_type(a.component_type, a.type_),
                    data: Vec::new(),
                });
            }
        }
    }
    // Colour sets used by a POINTS/LINES primitive live on vertices, the others on corners.
    let explicit_non_tri = |p: &Primitive| {
        matches!(
            p.mode,
            Some(Mode::Points | Mode::Lines | Mode::LineLoop | Mode::LineStrip)
        )
    };
    let col_point_domain: Vec<bool> = (0..num_cols)
        .map(|i| {
            let tag = format!("COLOR_{i}");
            pymesh
                .primitives
                .iter()
                .any(|p| explicit_non_tri(p) && p.attributes.keys().any(|a| a.contains(&tag)))
        })
        .collect();

    // ---- gather
    let mut num_faces = 0usize;
    let mut vert_locs: Vec<[f32; 3]> = Vec::new();
    let mut vert_normals: Vec<[f32; 3]> = Vec::new();
    let mut edge_vidxs: Vec<u32> = Vec::new();
    let mut loop_vidxs: Vec<u32> = Vec::new();
    let mut loop_uvs: Vec<Vec<[f32; 2]>> = vec![Vec::new(); num_uvs];
    let mut loop_cols: Vec<Vec<[f32; 4]>> = vec![Vec::new(); num_cols];
    let mut prim_faces: Vec<PrimFaces> = Vec::new();

    for (pi, prim) in pymesh.primitives.iter().enumerate() {
        let Some(&pos_acc) = prim.attributes.get("POSITION") else {
            prim_faces.push(PrimFaces {
                prim: pi,
                num_faces: 0,
            });
            continue;
        };
        if prim.extensions.contains("KHR_draco_mesh_compression") {
            return Err(ImportError::NotImplemented(
                "KHR_draco_mesh_compression: Draco-compressed primitives are imported from M7"
                    .into(),
            ));
        }
        let vert_index_base = vert_locs.len() as u32;
        let raw = primitive_indices(&cx.doc, &cx.buffers, prim)?;
        let mode = prim.mode_or_default();
        let topo = decode_topology(mode, &raw);
        let (indices, is_edges, is_tris): (Vec<u32>, bool, bool) = match mode {
            Mode::Points => (topo.points, false, false),
            Mode::Lines | Mode::LineLoop | Mode::LineStrip => {
                (topo.edges.iter().flatten().copied().collect(), true, false)
            }
            _ => (
                topo.triangles.iter().flatten().copied().collect(),
                false,
                true,
            ),
        };
        let (unique, inverse) = unique_inverse(&indices);
        let vs = cx.accessor_f32::<3>(pos_acc)?;
        vert_locs.extend(gather(&vs, &unique, "POSITION")?);
        if has_normals {
            match prim.attributes.get("NORMAL") {
                Some(&n) => {
                    let ns = cx.accessor_f32::<3>(n)?;
                    vert_normals.extend(gather(&ns, &unique, "NORMAL")?);
                }
                None => vert_normals.extend(std::iter::repeat_n([0.0; 3], unique.len())),
            }
        }
        let prim_vidxs: Vec<u32> = inverse.iter().map(|i| i + vert_index_base).collect();
        if is_edges {
            edge_vidxs.extend_from_slice(&prim_vidxs);
        }
        let mut faces = 0;
        if is_tris {
            faces = indices.len() / 3;
            num_faces += faces;
            loop_vidxs.extend_from_slice(&prim_vidxs);
            for (uv_i, layer) in loop_uvs.iter_mut().enumerate() {
                match prim.attributes.get(&format!("TEXCOORD_{uv_i}")) {
                    Some(&a) => {
                        let uvs = cx.accessor_f32::<2>(a)?;
                        layer.extend(gather(&uvs, &indices, "TEXCOORD")?);
                    }
                    None => layer.extend(std::iter::repeat_n([0.0; 2], indices.len())),
                }
            }
        }
        for (col_i, layer) in loop_cols.iter_mut().enumerate() {
            let point = col_point_domain[col_i];
            // Corner-domain colours only exist on triangles (the add-on would fail on the
            // length mismatch for a POINTS/LINES primitive here).
            if !point && !is_tris {
                continue;
            }
            let sel: &[u32] = if point { &unique } else { &indices };
            match prim.attributes.get(&format!("COLOR_{col_i}")) {
                Some(&a) => {
                    let acc = cx.accessor(a)?;
                    let n = acc.type_.components();
                    let flat = acc.to_f32();
                    let cols: Vec<[f32; 4]> = flat
                        .chunks_exact(n.max(1))
                        .map(|c| {
                            [
                                c[0],
                                c.get(1).copied().unwrap_or(0.0),
                                c.get(2).copied().unwrap_or(0.0),
                                if n == 4 { c[3] } else { 1.0 },
                            ]
                        })
                        .collect();
                    layer.extend(gather(&cols, sel, "COLOR")?);
                }
                None => layer.extend(std::iter::repeat_n([1.0; 4], sel.len())),
            }
        }
        for c in customs.iter_mut() {
            match prim.attributes.get(&c.name) {
                Some(&a) => {
                    let acc = cx.accessor(a)?;
                    let flat = acc.to_f32();
                    for &u in &unique {
                        let s = u as usize * c.components;
                        let e = s + c.components;
                        if e > flat.len() {
                            return Err(ImportError::Data(format!(
                                "{} index {u} out of range",
                                c.name
                            )));
                        }
                        c.data.extend_from_slice(&flat[s..e]);
                    }
                }
                None => c
                    .data
                    .extend(std::iter::repeat_n(0.0, unique.len() * c.components)),
            }
        }
        prim_faces.push(PrimFaces {
            prim: pi,
            num_faces: faces,
        });
    }

    if cx.settings.merge_vertices {
        merge_duplicate_verts(
            &mut vert_locs,
            &mut vert_normals,
            &mut loop_vidxs,
            &mut edge_vidxs,
            &mut customs,
        );
    }

    // ---- glTF → Blender
    cx.units.locs_batch(&mut vert_locs);
    normals_batch(&mut vert_normals);
    for uvs in loop_uvs.iter_mut() {
        uvs_batch(uvs);
    }

    // ---- create
    let verts_num = vert_locs.len();
    let mut mesh = Mesh::new("");
    mesh.attributes_mut()
        .insert(bool_attr(SELECT_VERT, Domain::Point, vec![true; verts_num]));
    mesh.set_positions(vert_locs);
    let offsets: Vec<i32> = (0..=num_faces).map(|f| (3 * f) as i32).collect();
    let corner_verts: Vec<i32> = loop_vidxs.iter().map(|&v| v as i32).collect();
    let loose: Vec<[i32; 2]> = edge_vidxs
        .chunks_exact(2)
        .map(|e| [e[0] as i32, e[1] as i32])
        .collect();
    mesh.set_topology(offsets, corner_verts, loose);
    if num_faces > 0 {
        mesh.attributes_mut()
            .insert(bool_attr(SELECT_POLY, Domain::Face, vec![true; num_faces]));
    }
    for (uv_i, uvs) in loop_uvs.into_iter().enumerate() {
        let name: SmolStr = if uv_i == 0 {
            "UVMap".into()
        } else {
            format!("UVMap.{uv_i:03}").into()
        };
        mesh.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uvs)),
        });
        if uv_i == 0 {
            mesh.uv_active = Some(name.clone());
            mesh.uv_active_render = Some(name);
        }
    }
    for (col_i, cols) in loop_cols.into_iter().enumerate() {
        let name: SmolStr = if col_i == 0 {
            COLOR_0_ATTRIBUTE.into()
        } else {
            format!("{COLOR_0_ATTRIBUTE}.{col_i:03}").into()
        };
        let domain = if col_point_domain[col_i] {
            Domain::Point
        } else {
            Domain::Corner
        };
        mesh.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain,
            data: AttrData::ByteColor(Arc::new(cols.into_iter().map(byte_color).collect())),
        });
        if col_i == 0 {
            mesh.color_active = Some(name.clone());
            mesh.color_default = Some(name);
        }
    }

    assign_materials(cx, main, log, pymesh, &prim_faces, num_faces, &mut mesh)?;

    for c in customs {
        let Some(ty) = c.blender_type else {
            continue;
        };
        let data = match ty {
            "FLOAT" => AttrData::Float(Arc::new(c.data)),
            "INT" => AttrData::Int(Arc::new(c.data.iter().map(|&v| v as i32).collect())),
            "FLOAT2" => AttrData::Float2(Arc::new(chunks::<2>(&c.data))),
            "FLOAT_VECTOR" => AttrData::Float3(Arc::new(chunks::<3>(&c.data))),
            "FLOAT_COLOR" => AttrData::FloatColor(Arc::new(chunks::<4>(&c.data))),
            "BYTE_COLOR" => AttrData::ByteColor(Arc::new(
                chunks::<4>(&c.data).into_iter().map(byte_color).collect(),
            )),
            "FLOAT4X4" => AttrData::Float4x4(Arc::new(
                chunks::<16>(&c.data)
                    .into_iter()
                    .map(|m| std::array::from_fn(|col| std::array::from_fn(|row| m[col * 4 + row])))
                    .collect(),
            )),
            _ => continue,
        };
        mesh.attributes_mut().insert(Attribute {
            name: c.name.into(),
            domain: Domain::Point,
            data,
        });
    }

    set_poly_smoothing(
        cx,
        pymesh,
        &prim_faces,
        &mut mesh,
        &vert_normals,
        &loop_vidxs,
    );
    mesh.calc_edges(true, true);
    validate::validate(&mut mesh);

    if has_normals {
        set_custom_normals(&mut mesh, &vert_normals);
    }
    Ok(mesh)
}

fn chunks<const N: usize>(v: &[f32]) -> Vec<[f32; N]> {
    v.chunks_exact(N)
        .map(|c| std::array::from_fn(|i| c[i]))
        .collect()
}

fn bool_attr(name: &str, domain: Domain, values: Vec<bool>) -> Attribute {
    Attribute {
        name: name.into(),
        domain,
        data: AttrData::Bool(Arc::new(values)),
    }
}

/// The material-slot pass of `do_primitives`.
fn assign_materials(
    cx: &mut Gltf,
    main: &mut Main,
    log: &mut ChangeLog,
    pymesh: &frap_gltf::json::Mesh,
    prim_faces: &[PrimFaces],
    num_faces: usize,
    mesh: &mut Mesh,
) -> Result<(), ImportError> {
    let prims = &pymesh.primitives;
    let has_materials = prims.iter().any(|p| p.material.is_some())
        || prims.iter().any(|p| p.attributes.contains_key("COLOR_0"));
    if !has_materials {
        return Ok(());
    }
    let merge = cx.settings.import_merge_material_slots;
    let mut slots: Vec<Option<IdRef<MaterialId>>> = Vec::new();
    let mut default_materials: Option<usize> = None;
    let mut empty_slot: Option<usize> = None;
    let mut material_indices = vec![0i32; num_faces];
    let mut f = 0usize;
    for pf in prim_faces {
        let prim = &prims[pf.prim];
        let vertex_color = prim.attributes.contains_key("COLOR_0");
        let index = if let Some(m) = prim.material {
            let mat = materials::create(cx, main, log, Some(m as usize), vertex_color)?;
            if merge {
                match slots.iter().position(|s| *s == Some(mat)) {
                    Some(i) => i,
                    None => {
                        slots.push(Some(mat));
                        slots.len() - 1
                    }
                }
            } else {
                slots.push(Some(mat));
                slots.len() - 1
            }
        } else if vertex_color {
            match (merge, default_materials) {
                (true, Some(i)) => i,
                (false, Some(i)) => {
                    let m = slots[i];
                    slots.push(m);
                    slots.len() - 1
                }
                (_, None) => {
                    let mat = materials::create(cx, main, log, None, true)?;
                    slots.push(Some(mat));
                    default_materials = Some(slots.len() - 1);
                    slots.len() - 1
                }
            }
        } else if merge {
            *empty_slot.get_or_insert_with(|| {
                slots.push(None);
                slots.len() - 1
            })
        } else {
            slots.push(None);
            slots.len() - 1
        };
        for v in &mut material_indices[f..f + pf.num_faces] {
            *v = index as i32;
        }
        f += pf.num_faces;
    }
    mesh.materials = slots;
    mesh.attributes_mut().insert(Attribute {
        name: "material_index".into(),
        domain: Domain::Face,
        data: AttrData::Int(Arc::new(material_indices)),
    });
    Ok(())
}

/// `set_poly_smoothing`: the `sharp_face` attribute.
fn set_poly_smoothing(
    cx: &Gltf,
    pymesh: &frap_gltf::json::Mesh,
    prim_faces: &[PrimFaces],
    mesh: &mut Mesh,
    vert_normals: &[[f32; 3]],
    loop_vidxs: &[u32],
) {
    let num_polys = mesh.faces_num();
    let has_normal = |pf: &PrimFaces| pymesh.primitives[pf.prim].attributes.contains_key("NORMAL");
    let sharps: Vec<bool> = match cx.settings.import_shading {
        ImportShading::Flat => vec![true; num_polys],
        ImportShading::Smooth => {
            let mut s = vec![false; num_polys];
            let mut f = 0;
            for pf in prim_faces {
                if !has_normal(pf) {
                    s[f..f + pf.num_faces].fill(true);
                }
                f += pf.num_faces;
            }
            s
        }
        ImportShading::Normals => {
            let poly_normals = frap_geom::normals::normals_calc_faces(
                mesh.positions(),
                mesh.face_offsets(),
                mesh.corner_verts(),
            );
            // numpy compares float32 data with the weak scalar 0.9999999 as float32.
            let threshold = 0.999_999_9_f64 as f32;
            let mut s = vec![false; num_polys];
            let mut f = 0;
            for pf in prim_faces {
                if !has_normal(pf) {
                    s[f..f + pf.num_faces].fill(true);
                    f += pf.num_faces;
                    continue;
                }
                for face in f..f + pf.num_faces {
                    let pn = poly_normals[face];
                    let smooth = (0..3).any(|k| {
                        let vn = vert_normals[loop_vidxs[3 * face + k] as usize];
                        let d = vn[0] * pn[0] + vn[1] * pn[1] + vn[2] * pn[2];
                        d <= threshold
                    });
                    s[face] = !smooth;
                }
                f += pf.num_faces;
            }
            s
        }
    };
    mesh.attributes_mut()
        .insert(bool_attr(SHARP_FACE, Domain::Face, sharps));
}

/// `mesh.normals_split_custom_set_from_vertices(vert_normals)`.
fn set_custom_normals(mesh: &mut Mesh, vert_normals: &[[f32; 3]]) {
    let face_normals = frap_geom::normals::normals_calc_faces(
        mesh.positions(),
        mesh.face_offsets(),
        mesh.corner_verts(),
    );
    let vn = mesh.vert_normals();
    let sharp_faces = mesh.sharp_face_flags().map(|c| c.into_owned());
    let sharp_edges = mesh.sharp_edge_flags().map(|c| c.into_owned());
    let data = custom_normals::encode_from_vertices(
        &custom_normals::Input {
            positions: mesh.positions(),
            face_offsets: mesh.face_offsets(),
            corner_verts: mesh.corner_verts(),
            corner_edges: mesh.corner_edges(),
            face_normals: &face_normals,
            vert_normals: &vn,
            sharp_edges: sharp_edges.as_deref(),
            sharp_faces: sharp_faces.as_deref(),
        },
        vert_normals,
    );
    let edges = mesh.edges_num();
    let attrs = mesh.attributes_mut();
    attrs.insert(Attribute {
        name: frap_mesh::CUSTOM_NORMAL.into(),
        domain: Domain::Corner,
        data: AttrData::Int16x2(Arc::new(data)),
    });
    if attrs.get(SHARP_EDGE).is_none() {
        attrs.insert(bool_attr(SHARP_EDGE, Domain::Edge, vec![false; edges]));
    }
}

/// Byte-wise key of one welded vertex (`fast_structured_np_unique` views the struct as UCS4
/// code units): the f32 bit patterns, `-0.0` folded to `0.0`.
fn dot_key(p: [f32; 3], n: Option<[f32; 3]>) -> Vec<u32> {
    let bits = |x: f32| if x == 0.0 { 0u32 } else { x.to_bits() };
    let mut k = vec![bits(p[0]), bits(p[1]), bits(p[2])];
    if let Some(n) = n {
        k.extend([bits(n[0]), bits(n[1]), bits(n[2])]);
    }
    k
}

/// `merge_duplicate_verts`: welds vertices with equal position (and rounded normal); the
/// result is in byte-sorted key order, each kept vertex taking its first occurrence's data.
fn merge_duplicate_verts(
    vert_locs: &mut Vec<[f32; 3]>,
    vert_normals: &mut Vec<[f32; 3]>,
    loop_vidxs: &mut [u32],
    edge_vidxs: &mut [u32],
    customs: &mut [Custom],
) {
    let has_normals = !vert_normals.is_empty();
    if has_normals {
        // numpy float32: `n *= 50000; n = trunc(n); n *= (1 / 50000)`.
        let inv = (1.0f64 / 50000.0) as f32;
        for n in vert_normals.iter_mut() {
            for c in n.iter_mut() {
                *c = (*c * 50000.0).trunc() * inv;
            }
        }
    }
    let fold = |x: f32| if x == 0.0 { 0.0 } else { x };
    let keys: Vec<Vec<u32>> = (0..vert_locs.len())
        .map(|i| dot_key(vert_locs[i], has_normals.then(|| vert_normals[i])))
        .collect();
    let mut order: Vec<usize> = (0..keys.len()).collect();
    // Stable: the first occurrence of each key comes first (`return_index`).
    order.sort_by(|&a, &b| keys[a].cmp(&keys[b]).then(Ordering::Equal));
    let mut inverse = vec![0u32; keys.len()];
    let mut first: Vec<usize> = Vec::new();
    for (pos, &i) in order.iter().enumerate() {
        if pos == 0 || keys[order[pos - 1]] != keys[i] {
            first.push(i);
        }
        inverse[i] = (first.len() - 1) as u32;
    }
    for v in loop_vidxs.iter_mut() {
        *v = inverse[*v as usize];
    }
    for v in edge_vidxs.iter_mut() {
        *v = inverse[*v as usize];
    }
    for c in customs.iter_mut() {
        let n = c.components;
        let mut data = Vec::with_capacity(first.len() * n);
        for &i in &first {
            data.extend_from_slice(&c.data[i * n..(i + 1) * n]);
        }
        c.data = data;
    }
    *vert_locs = first.iter().map(|&i| vert_locs[i].map(fold)).collect();
    if has_normals {
        *vert_normals = first.iter().map(|&i| vert_normals[i].map(fold)).collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_inverse_sorts() {
        let (u, inv) = unique_inverse(&[5, 2, 5, 9, 2]);
        assert_eq!(u, [2, 5, 9]);
        assert_eq!(inv, [1, 0, 1, 2, 0]);
    }

    #[test]
    fn merge_welds_equal_positions_and_normals() {
        let mut locs = vec![
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [-0.0, 0.0, 0.0],
        ];
        let mut normals = vec![[0.0, 0.0, 1.0]; 4];
        normals[2] = [0.0, 1.0, 0.0];
        let mut loops = vec![0, 1, 2, 3];
        let mut edges = vec![];
        merge_duplicate_verts(&mut locs, &mut normals, &mut loops, &mut edges, &mut []);
        // Keys sort byte-wise: 0.0 < 1.0; vertex 2 differs by its normal.
        assert_eq!(locs.len(), 3);
        assert_eq!(locs[0], [0.0, 0.0, 0.0]);
        assert_eq!(loops[1], loops[3]);
        assert_ne!(loops[0], loops[2]);
    }

    #[test]
    fn byte_colors_are_srgb() {
        assert_eq!(byte_color([0.0, 1.0, 0.5, 0.5]), [0, 255, 188, 128]);
    }
}
