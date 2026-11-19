// SPDX-License-Identifier: GPL-3.0-or-later
//! `NODES` (Geometry Nodes modifier). frap has no geometry-nodes evaluator: the modifier
//! dispatches on its node group's **(library file name, group name)** and evaluates the
//! groups it knows natively. Only the Essentials "Smooth by Angle" group (linked by
//! `object.shade_auto_smooth`, `frap_nodes::builtin_groups::smooth_by_angle`) is supported; a
//! local group with the same name is not it, and any other group is the modifier error
//! `Unsupported node group "X" (frap supports: Smooth by Angle)` (the mesh passes through).
//!
//! Without a node group the modifier is disabled (`is_disabled` of `MOD_nodes.cc`).
//!
//! "Smooth by Angle" is a port of what its 14 nodes compute, from
//! blender/source/blender/nodes/geometry/nodes/node_geo_set_shade_smooth.cc,
//! node_geo_input_mesh_edge_angle.cc (`create_edge_map`, `AngleFieldInput`),
//! blender/source/blender/blenkernel/intern/mesh_attributes.cc
//! (`adapt_mesh_domain_face_to_edge_impl<bool>`: `Is Shade Smooth` is `!sharp_face` read on the
//! edges, where a face's `true` reaches every edge of the face), node_geo_input_face_smooth.cc,
//! blender/source/blender/blenkernel/intern/
//! geometry_fields.cc (`try_capture_fields_on_geometry`) and blenlib's
//! `angle_normalized_v3v3`, Blender 5.2.4, GPL-2.0-or-later:
//! 1. on the edges (when there are any), selection = `Is Edge Smooth` (`!sharp_edge`) or
//!    `Ignore Sharpness`; on the selected edges `sharp_edge = !(edge angle <= Angle and
//!    (no adjacent face sharp or Ignore Sharpness))`, the other edges keep their value (new
//!    attributes start false and are appended; an existing BOOLEAN EDGE `sharp_edge` keeps its
//!    position; one of another type or domain is read converted (nonzero is sharp, verified on
//!    5.2.4 with an INT EDGE `sharp_edge`) and replaced by a BOOLEAN EDGE one at the end). The edge angle is the angle between the `face_normal_calc` normals of the two
//!    faces of a manifold edge, 0 for any other edge;
//! 2. on the faces (when there are any), every face smooth: `sharp_face` is removed.

use frap_geom::normals::{dot, face_normal_calc, len_v3v3};
use frap_mesh::{AttrData, Attribute, Domain, Mesh, SHARP_EDGE, SHARP_FACE};
use frap_nodes::builtin_groups::smooth_by_angle as sba;

/// The built-in group this evaluator recognises (identifiers, library names, its builder).
pub use frap_nodes::builtin_groups::smooth_by_angle;
use frap_rna::{IdAny, NodeTreeId};
use frap_scene::id::{GnValue, ModifierSettings};
use smallvec::SmallVec;

use crate::prelude::*;

/// `is_smooth_by_angle_modifier` (`editors/object/object_edit.cc`): a NODES modifier whose
/// group is the Essentials "Smooth by Angle" linked from `…/geometry_nodes_essentials.blend`
/// (what `object.shade_auto_smooth` reuses and `shade_flat`/`shade_smooth` remove).
pub fn is_smooth_by_angle_modifier(main: &frap_scene::Main, m: &Modifier) -> bool {
    let ModifierSettings::Nodes(s) = &m.settings else {
        return false;
    };
    let Some(tree) = s
        .node_group
        .and_then(|g| g.typed::<NodeTreeId>())
        .and_then(|r| main.node_groups.get(r))
    else {
        return false;
    };
    let library = tree
        .header
        .library
        .and_then(|l| main.libraries.get(l))
        .map(|l| l.filepath.as_str());
    sba::is_smooth_by_angle(&tree.header.name, library)
}

/// The node group (part of the fingerprint).
fn depends_on(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    match s {
        ModifierSettings::Nodes(n) => n.node_group.into_iter().collect(),
        _ => SmallVec::new(),
    }
}

/// `is_disabled`: no node group.
fn is_disabled(s: &ModifierSettings) -> Option<&'static str> {
    match s {
        ModifierSettings::Nodes(n) if n.node_group.is_none() => Some("no node group"),
        _ => None,
    }
}

#[modifier(
    NODES,
    kind = Constructive,
    flags = ACCEPTS_MESH | ACCEPTS_CVS | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | SUPPORTS_MAPPING | ACCEPTS_GREASE_PENCIL,
    depends_on = depends_on,
    is_disabled = is_disabled,
    krate = crate
)]
fn eval(md: &Modifier, mut mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Nodes(s) = &md.settings else {
        return Ok(mesh);
    };
    let Some(group) = s.node_group else {
        return Ok(mesh);
    };
    let main = cx.main();
    let tree = group
        .typed::<NodeTreeId>()
        .and_then(|r| main.node_groups.get(r))
        .ok_or_else(|| ModError::Error("frap internal: the node group no longer exists".into()))?;
    let library = tree
        .header
        .library
        .and_then(|l| main.libraries.get(l))
        .map(|l| l.filepath.as_str());
    if !sba::is_smooth_by_angle(&tree.header.name, library) {
        return Err(ModError::Error(format!(
            "Unsupported node group \"{}\" (frap supports: Smooth by Angle)",
            tree.header.name
        )));
    }
    let input = |id: &str| s.inputs.0.get(id);
    for id in [sba::INPUT_ANGLE, sba::INPUT_IGNORE_SHARPNESS] {
        if input(id).is_some_and(|i| i.r#type == "ATTRIBUTE") {
            return Err(ModError::Error(format!(
                "frap: input {id} driven by an attribute is not supported (Smooth by Angle)"
            )));
        }
    }
    let angle = match input(sba::INPUT_ANGLE).map(|i| &i.value) {
        Some(GnValue::Float(f)) => *f,
        _ => sba::DEFAULT_ANGLE,
    };
    let ignore_sharpness = match input(sba::INPUT_IGNORE_SHARPNESS).map(|i| &i.value) {
        Some(GnValue::Bool(b)) => *b,
        _ => false,
    };
    smooth_by_angle(&mut mesh, angle, ignore_sharpness);
    Ok(mesh)
}

/// `safe_asinf`.
fn safe_asinf(a: f32) -> f32 {
    if a <= -1.0 {
        -std::f32::consts::FRAC_PI_2
    } else if a >= 1.0 {
        std::f32::consts::FRAC_PI_2
    } else {
        libm::asinf(a)
    }
}

/// `angle_normalized_v3v3`.
fn angle_normalized_v3v3(v1: [f32; 3], v2: [f32; 3]) -> f32 {
    if dot(v1, v2) >= 0.0 {
        return 2.0 * safe_asinf(len_v3v3(v1, v2) / 2.0);
    }
    let v2_n = [-v2[0], -v2[1], -v2[2]];
    std::f32::consts::PI - 2.0 * safe_asinf(len_v3v3(v1, v2_n) / 2.0)
}

/// `Edge Angle` → `Unsigned Angle` per edge (`create_edge_map` + `AngleFieldInput`).
fn unsigned_edge_angles(mesh: &Mesh) -> Vec<f32> {
    let offsets = mesh.face_offsets();
    let corner_verts = mesh.corner_verts();
    let corner_edges = mesh.corner_edges();
    let positions = mesh.positions();
    let mut edge_map = vec![[-1i32; 2]; mesh.edges_num()];
    for f in 0..mesh.faces_num() {
        for &e in &corner_edges[offsets[f] as usize..offsets[f + 1] as usize] {
            let Some(entry) = edge_map.get_mut(e as usize) else {
                continue;
            };
            if entry[0] == -1 {
                entry[0] = f as i32;
            } else if entry[1] == -1 {
                entry[1] = f as i32;
            } else {
                *entry = [-2, -2];
            }
        }
    }
    let normal = |f: usize| {
        face_normal_calc(
            positions,
            &corner_verts[offsets[f] as usize..offsets[f + 1] as usize],
        )
    };
    edge_map
        .iter()
        .map(|m| {
            if m[0] < 0 || m[1] < 0 {
                return 0.0;
            }
            angle_normalized_v3v3(normal(m[0] as usize), normal(m[1] as usize))
        })
        .collect()
}

/// What the "Smooth by Angle" group does to `mesh` (module docs).
pub fn smooth_by_angle(mesh: &mut Mesh, angle: f32, ignore_sharpness: bool) {
    let edges_num = mesh.edges_num();
    if edges_num > 0 {
        // `sharp_edge` read as a boolean on the edges (`lookup<bool>`: any type converted,
        // nonzero is true, and any domain adapted), the "Is Edge Smooth" input and, for an
        // attribute stored with another type or domain, the initial value of the edges that
        // are not selected (`initialize_new_data`).
        let sharp_before: Option<Vec<bool>> = mesh.sharp_edge_flags().map(|c| c.into_owned());
        let edge_smooth: Vec<bool> = match &sharp_before {
            Some(sharp) => sharp.iter().map(|s| !s).collect(),
            None => vec![true; edges_num],
        };
        // `Is Shade Smooth` is `!sharp_face` with `sharp_face` read on the edge domain (a
        // boolean face value reaches an edge when any adjacent face has it): an edge is smooth
        // when none of its faces is sharp (a loose edge, or no `sharp_face`, is smooth).
        let mut edge_face_smooth = vec![true; edges_num];
        if let Some(sharp) = mesh.sharp_face_flags() {
            let offsets = mesh.face_offsets();
            let corner_edges = mesh.corner_edges();
            for (f, s) in sharp.iter().enumerate() {
                if *s {
                    for &e in &corner_edges[offsets[f] as usize..offsets[f + 1] as usize] {
                        if let Some(x) = edge_face_smooth.get_mut(e as usize) {
                            *x = false;
                        }
                    }
                }
            }
        }
        let angles = unsigned_edge_angles(mesh);
        let existing = mesh.sharp_edges().map(<[bool]>::to_vec);
        let in_place = existing.is_some();
        let mut sharp = existing
            .or(sharp_before)
            .unwrap_or_else(|| vec![false; edges_num]);
        for e in 0..edges_num {
            if edge_smooth[e] || ignore_sharpness {
                let smooth = angles[e] <= angle && (edge_face_smooth[e] || ignore_sharpness);
                sharp[e] = !smooth;
            }
        }
        let attrs = mesh.attributes_mut();
        if !in_place {
            // Absent, or stored with another domain or type: re-added at the end.
            attrs.remove(SHARP_EDGE);
        }
        attrs.insert(Attribute {
            name: SHARP_EDGE.into(),
            domain: Domain::Edge,
            data: AttrData::Bool(sharp.into()),
        });
    }
    if mesh.faces_num() > 0 {
        mesh.attributes_mut().remove(SHARP_FACE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> Mesh {
        let mut m = frap_mesh::primitives::cube::cube(2.0, false, [1.0; 3]);
        m.shade_flat();
        m
    }

    #[test]
    fn cube_edges_are_all_sharp_at_30_degrees() {
        let mut m = cube();
        smooth_by_angle(&mut m, sba::DEFAULT_ANGLE, false);
        assert!(m.attributes().get(SHARP_FACE).is_none());
        assert_eq!(m.sharp_edges().unwrap(), vec![true; 12].as_slice());
    }

    #[test]
    fn flat_faces_keep_their_edges_sharp_unless_ignored() {
        // 100° > 90°: smooth by angle, but every face is flat, so no edge is smooth.
        let mut m = cube();
        smooth_by_angle(&mut m, 100f32.to_radians(), false);
        assert_eq!(m.sharp_edges().unwrap(), vec![true; 12].as_slice());
        let mut m = cube();
        smooth_by_angle(&mut m, 100f32.to_radians(), true);
        assert_eq!(m.sharp_edges().unwrap(), vec![false; 12].as_slice());
        // Already sharp edges stay sharp (not selected) unless sharpness is ignored.
        let mut m = cube();
        m.shade_smooth();
        let mut flags = vec![false; 12];
        flags[3] = true;
        m.set_bool_attribute(SHARP_EDGE, Domain::Edge, flags);
        smooth_by_angle(&mut m, 100f32.to_radians(), false);
        let sharp = m.sharp_edges().unwrap();
        assert!(sharp[3]);
        assert_eq!(sharp.iter().filter(|s| **s).count(), 1);
    }

    #[test]
    fn a_sharp_edge_of_another_type_is_read_converted() {
        // Verified on 5.2.4: an INT EDGE `sharp_edge` with edge 2 = 1, Smooth by Angle at
        // 1.6 rad on the smooth cube: edge 2 stays sharp, the attribute becomes BOOLEAN EDGE
        // and moves to the end.
        let mut m = cube();
        m.shade_smooth();
        let mut v = vec![0i32; 12];
        v[2] = 1;
        m.attributes_mut().insert(Attribute {
            name: SHARP_EDGE.into(),
            domain: Domain::Edge,
            data: AttrData::Int(std::sync::Arc::new(v)),
        });
        smooth_by_angle(&mut m, 1.6, false);
        let mut expected = vec![false; 12];
        expected[2] = true;
        assert_eq!(m.sharp_edges().unwrap(), expected.as_slice());
        assert_eq!(
            m.attributes().iter().last().map(|a| a.name.as_str()),
            Some(SHARP_EDGE)
        );
    }

    #[test]
    fn angles_of_non_manifold_edges_are_zero() {
        let mut m = Mesh::new("Tri");
        m.from_pydata(
            &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            &[],
            &[vec![0, 1, 2]],
            false,
        );
        assert_eq!(unsigned_edge_angles(&m), vec![0.0; 3]);
        assert!(
            (angle_normalized_v3v3([0.0, 0.0, 1.0], [1.0, 0.0, 0.0]) - 1.570_796_4).abs() < 1e-6
        );
        assert!(
            (angle_normalized_v3v3([0.0, 0.0, 1.0], [0.0, 0.0, -1.0]) - std::f32::consts::PI).abs()
                < 1e-6
        );
    }
}
