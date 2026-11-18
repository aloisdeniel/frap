// SPDX-License-Identifier: GPL-3.0-or-later
//! SUBSURF (`SubsurfModifier`, "Subdivision Surface"): Catmull-Clark (or `SIMPLE`)
//! subdivision through the evaluation context's [`frap_subdiv::Subdivider`], `levels` in the
//! viewport and `render_levels` in a render, with `quality`, `use_limit_surface`,
//! `use_creases` (`crease_edge` / `crease_vert`), `boundary_smooth` and `uv_smooth`.
//!
//! The result has Blender's element order (`subdiv_foreach.cc`) and Blender's attribute
//! propagation (`subdiv_mesh.cc`): vertex and corner attributes interpolated in their own
//! types ([`mix`]), edge attributes copied along coarse edges and defaulted on inner edges,
//! face attributes copied to the child faces, UV maps evaluated as face-varying data,
//! vertex-group weights mixed like `mix_deform_verts`; string attributes and custom normals
//! are not carried.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_subsurf.cc,
//! blenkernel/intern/subdiv_modifier.cc and subdiv_mesh.cc (Blender 5.2.4).
//!
//! Not yet evaluated (WARNING, the mesh is subdivided without them): `use_custom_normals`
//! with custom normals (the result has none; encoding custom normals needs a shared encoder),
//! and the scene's `render.use_simplify` level cap (`get_render_subsurf_level`; the
//! evaluation context does not expose the scene yet).

mod mix;

use std::sync::Arc;

use frap_mesh::{AttrData, Attribute, Domain};
use frap_rna::{Report, ReportType};
use frap_scene::generated::modifier_settings::SubsurfSettings;
use frap_subdiv::{
    FVarLinear, SubdivError, SubdivResult, SubdivSettings, SubdivTopology, VtxBoundary,
};

use crate::EvalMode;
use crate::prelude::*;

#[modifier(
    SUBSURF,
    kind = Constructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Subsurf(s) = &md.settings else {
        return Ok(mesh);
    };
    let settings = subdiv_settings(s, cx.mode());
    if settings.level == 0 {
        return Ok(mesh);
    }
    if s.use_custom_normals && mesh.attributes().get(frap_mesh::CUSTOM_NORMAL).is_some() {
        cx.report(Report::new(
            ReportType::Warning,
            format!(
                "Modifier '{}': frap does not interpolate custom normals yet (use_custom_normals); \
                 the subdivided mesh has none",
                md.name
            ),
        ));
    }
    frap_math::budget::check_corners(estimate_corners(&mesh, settings.level));
    let subdivider = cx.subdivider();
    match subdivide(&mesh, &settings, subdivider) {
        Ok(Some(out)) => Ok(out),
        Ok(None) => Ok(mesh),
        Err(e) => Err(e),
    }
}

/// `BKE_subsurf_modifier_settings_init` for the evaluation mode.
pub(crate) fn subdiv_settings(s: &SubsurfSettings, mode: EvalMode) -> SubdivSettings {
    let requested = match mode {
        EvalMode::Render => s.render_levels,
        _ => s.levels,
    };
    let mut out = SubdivSettings::default();
    out.is_simple = s.subdivision_type == "SIMPLE";
    out.is_adaptive = s.use_adaptive_subdivision;
    out.level = requested.max(0) as u32;
    out.use_creases = s.use_creases;
    out.vtx_boundary = VtxBoundary::from_boundary_smooth(&s.boundary_smooth);
    out.fvar_linear = FVarLinear::from_uv_smooth(&s.uv_smooth);
    out.use_limit_surface = s.use_limit_surface;
    out.quality = s.quality.max(1) as u32;
    out
}

/// Face corners of the result (`4 · faces`): a quad gives `(2^level)²` faces, any other face
/// one quarter grid per corner.
fn estimate_corners(mesh: &Mesh, level: u32) -> u64 {
    let level = level.min(30) as u64;
    let quad = 1u64 << (2 * level);
    let corner = if level == 0 {
        1
    } else {
        1u64 << (2 * (level - 1))
    };
    let offs = mesh.face_offsets();
    let mut faces = 0u64;
    for f in 0..mesh.faces_num() {
        let n = (offs[f + 1] - offs[f]) as u64;
        faces = faces.saturating_add(if n == 4 {
            quad
        } else {
            n.saturating_mul(corner)
        });
    }
    faces.saturating_mul(4)
}

fn float_attr<'a>(mesh: &'a Mesh, name: &str, domain: Domain) -> Option<&'a [f32]> {
    let a = mesh.attributes().get(name)?;
    if a.domain != domain {
        return None;
    }
    match &a.data {
        AttrData::Float(v) => Some(v.as_slice()),
        _ => None,
    }
}

/// Topology attributes, never carried generically.
fn is_topology(name: &str) -> bool {
    matches!(
        name,
        "position" | ".edge_verts" | ".corner_vert" | ".corner_edge"
    )
}

/// Subdivides `mesh`; `None` when the subdivider declines the topology (Blender returns the
/// input mesh: "happens on bad topology").
fn subdivide(
    mesh: &Mesh,
    settings: &SubdivSettings,
    subdivider: &dyn frap_subdiv::Subdivider,
) -> Result<Option<Mesh>, ModError> {
    let edges: Vec<[u32; 2]> = mesh
        .edges()
        .iter()
        .map(|e| [e[0] as u32, e[1] as u32])
        .collect();
    let offsets: Vec<u32> = mesh.face_offsets().iter().map(|&o| o as u32).collect();
    let corner_verts: Vec<u32> = mesh.corner_verts().iter().map(|&v| v as u32).collect();
    let corner_edges: Vec<u32> = mesh.corner_edges().iter().map(|&e| e as u32).collect();
    let uv_maps = mesh.uv_maps();
    let mut topo = SubdivTopology::new(
        mesh.positions(),
        &edges,
        &offsets,
        &corner_verts,
        &corner_edges,
    );
    if settings.use_creases {
        topo.edge_creases = float_attr(mesh, "crease_edge", Domain::Edge);
        topo.vert_creases = float_attr(mesh, "crease_vert", Domain::Point);
    }
    topo.uv_layers = uv_maps.iter().map(|(_, d)| *d).collect();
    let r = match subdivider.subdivide(&topo, settings) {
        Ok(r) => r,
        Err(SubdivError::TooLarge(m)) => return Err(ModError::Error(m)),
        Err(_) => return Ok(None),
    };
    let names: Vec<smol_str::SmolStr> = uv_maps.iter().map(|(n, _)| (*n).into()).collect();
    Ok(Some(build_mesh(mesh, r, &names)))
}

/// The subdivided mesh with the attributes of `src` carried as `subdiv_mesh.cc` does.
fn build_mesh(src: &Mesh, r: SubdivResult, uv_names: &[smol_str::SmolStr]) -> Mesh {
    let faces = mix::Faces {
        offsets: src.face_offsets(),
        corner_verts: src.corner_verts(),
        edges: src.edges(),
    };
    let mut out = src.clone();
    let face_offsets: Vec<i32> = r.face_offsets.iter().map(|&o| o as i32).collect();
    let corner_verts: Vec<i32> = r.corner_verts.iter().map(|&v| v as i32).collect();
    let corner_edges: Vec<i32> = r.corner_edges.iter().map(|&e| e as i32).collect();
    let edges: Vec<[i32; 2]> = r.edges.iter().map(|e| [e[0] as i32, e[1] as i32]).collect();
    // Start from the topology attributes only, then add the carried ones in the coarse
    // mesh's order (each domain in turn, as `subdiv_mesh_topology_info` creates them), the
    // UV maps last.
    let carried: Vec<Attribute> = src
        .attributes()
        .iter()
        .filter(|a| !is_topology(&a.name))
        .cloned()
        .collect();
    for a in &carried {
        out.attributes_mut().remove(&a.name);
    }
    out.set_topology(face_offsets, corner_verts, edges);
    out.attributes_mut().insert(Attribute {
        name: ".corner_edge".into(),
        domain: Domain::Corner,
        data: AttrData::Int(Arc::new(corner_edges)),
    });
    out.set_positions(r.positions);
    let nfaces = r.face_origin.len();
    let face_map: Vec<usize> = r.face_origin.iter().map(|&f| f as usize).collect();
    for domain in [Domain::Point, Domain::Edge, Domain::Face, Domain::Corner] {
        for a in carried.iter().filter(|a| a.domain == domain) {
            if matches!(a.data, AttrData::String(_)) {
                continue;
            }
            let data = match domain {
                Domain::Point => mix::interp_column(&a.data, &r.vert_sources, &faces, false),
                Domain::Edge => Some(gather_edges(&a.data, &r.edge_origin)),
                Domain::Face => Some(a.data.gather_with_default(&face_map, nfaces)),
                Domain::Corner => {
                    if a.name == frap_mesh::CUSTOM_NORMAL || uv_names.contains(&a.name) {
                        continue;
                    }
                    mix::interp_column(&a.data, &r.corner_sources, &faces, true)
                }
            };
            if let Some(data) = data {
                out.attributes_mut().insert(Attribute {
                    name: a.name.clone(),
                    domain,
                    data,
                });
            }
        }
    }
    for (name, uv) in uv_names.iter().zip(r.uv_layers) {
        out.attributes_mut().insert(Attribute {
            name: name.clone(),
            domain: Domain::Corner,
            data: AttrData::Float2(Arc::new(uv)),
        });
    }
    if let Some(dv) = src.deform_verts() {
        let new = mix::interp_deform(dv, &r.vert_sources, &faces);
        *out.deform_verts_mut() = new;
    }
    out
}

/// Edge attributes: copied along coarse edges, the type's default on inner edges.
fn gather_edges(data: &AttrData, origin: &[Option<u32>]) -> AttrData {
    // `gather_with_default` fills defaults past the mapped prefix, so map inner edges to a
    // default element appended after the coarse ones.
    let n = data.len();
    let with_default = append_default(data);
    let map: Vec<usize> = origin.iter().map(|o| o.map_or(n, |e| e as usize)).collect();
    with_default.gather_with_default(&map, origin.len())
}

fn append_default(data: &AttrData) -> AttrData {
    let n = data.len();
    let idx: Vec<usize> = (0..n).collect();
    // `gather_with_default(idx, n + 1)` copies the column and pads one default element.
    data.gather_with_default(&idx, n + 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frap_subdiv::catmull_clark::CatmullClark;

    fn cube() -> Mesh {
        frap_mesh::primitives::cube(2.0, true, [1.0; 3])
    }

    #[test]
    fn settings_follow_blender() {
        let s = SubsurfSettings::default();
        let v = subdiv_settings(&s, EvalMode::Viewport);
        assert_eq!(v.level, 1);
        assert!(v.use_limit_surface && v.use_creases && !v.is_simple);
        assert_eq!(v.quality, 3);
        assert_eq!(v.fvar_linear, FVarLinear::Boundaries);
        assert_eq!(v.vtx_boundary, VtxBoundary::EdgeOnly);
        assert_eq!(subdiv_settings(&s, EvalMode::Render).level, 2);
        let simple = SubsurfSettings {
            subdivision_type: "SIMPLE".into(),
            uv_smooth: "SMOOTH_ALL".into(),
            boundary_smooth: "PRESERVE_CORNERS".into(),
            ..SubsurfSettings::default()
        };
        let v = subdiv_settings(&simple, EvalMode::Viewport);
        assert!(v.is_simple);
        assert_eq!(v.fvar_linear, FVarLinear::None);
        assert_eq!(v.vtx_boundary, VtxBoundary::EdgeAndCorner);
    }

    #[test]
    fn cube_level_one_counts_and_attributes() {
        let m = cube();
        let s = subdiv_settings(&SubsurfSettings::default(), EvalMode::Viewport);
        let out = subdivide(&m, &s, &CatmullClark).unwrap().unwrap();
        assert_eq!(out.verts_num(), 26);
        assert_eq!(out.edges_num(), 48);
        assert_eq!(out.faces_num(), 24);
        assert_eq!(out.corners_num(), 96);
        // UV maps are carried (evaluated), one value per corner.
        let uvs = out.uv_maps();
        assert_eq!(uvs.len(), m.uv_maps().len());
        assert_eq!(uvs[0].1.len(), 96);
        // Coarse vertex 0 keeps its index and lies on the limit surface (|x| = 5/9·… < 1).
        let p = out.positions()[0];
        assert!(p.iter().all(|c| c.abs() < 1.0 && c.abs() > 0.4), "{p:?}");
    }

    #[test]
    fn face_and_edge_attributes_are_copied() {
        let mut m = cube();
        let nf = m.faces_num();
        m.attributes_mut().insert(Attribute {
            name: "material_index".into(),
            domain: Domain::Face,
            data: AttrData::Int(Arc::new((0..nf as i32).collect())),
        });
        let ne = m.edges_num();
        m.attributes_mut().insert(Attribute {
            name: "crease_edge".into(),
            domain: Domain::Edge,
            data: AttrData::Float(Arc::new(vec![1.0; ne])),
        });
        let s = subdiv_settings(&SubsurfSettings::default(), EvalMode::Viewport);
        let out = subdivide(&m, &s, &CatmullClark).unwrap().unwrap();
        let AttrData::Int(mi) = &out.attributes().get("material_index").unwrap().data else {
            panic!()
        };
        assert_eq!(&mi[0..4], &[0, 0, 0, 0]);
        assert_eq!(mi[4], 1);
        let AttrData::Float(c) = &out.attributes().get("crease_edge").unwrap().data else {
            panic!()
        };
        // 24 edges along coarse edges keep the crease, 24 inner edges get 0.
        assert_eq!(c.iter().filter(|&&x| x == 1.0).count(), 24);
        assert_eq!(c.iter().filter(|&&x| x == 0.0).count(), 24);
        // A fully creased cube stays a cube: every vertex on its surface.
        for p in out.positions() {
            assert!(p.iter().any(|c| (c.abs() - 1.0).abs() < 1e-5), "{p:?}");
        }
    }

    #[test]
    fn level_zero_and_estimate() {
        let m = cube();
        assert_eq!(estimate_corners(&m, 1), 6 * 4 * 4);
        assert_eq!(estimate_corners(&m, 2), 6 * 16 * 4);
    }
}
