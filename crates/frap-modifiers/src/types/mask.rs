// SPDX-License-Identifier: GPL-3.0-or-later
//! MASK (`MaskModifier`): keeps the vertices whose weight in `vertex_group` is above
//! `threshold` (mode VERTEX_GROUP), or that a selected bone of `armature` influences above it
//! (mode ARMATURE), inverted by `invert_vertex_group`, with the edges and faces they span.
//! `use_smooth` (VERTEX_GROUP only) cuts the boundary faces at the threshold instead, adding
//! interpolated vertices and closing edges. A mesh without vertex-group weights becomes empty
//! (unchanged when inverted).
//!
//! ARMATURE mode reads the pose channels' selection (`PoseBone.select`) of the armature: a
//! vertex group counts when a selected channel with a bone has its name. An armature object
//! always has a pose when it is evaluated (the depsgraph's `BKE_pose_rebuild`), so one whose
//! pose was never built selects nothing.
//!
//! Ported from blender/source/blender/modifiers/intern/MOD_mask.cc (Blender 5.2.4).

use frap_mesh::DeformVert;
use frap_rna::{ArmatureId, IdAny, IdRef, ObjectId};
use frap_scene::Main;
use frap_scene::id::Object;
use frap_scene::generated::modifier_settings::MaskSettings;
use smallvec::{SmallVec, smallvec};

use super::smooth::common::{Sources, Src, Topology, defgroup_index, find_weight, rebuild};
use crate::prelude::*;

fn deps(s: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    let mut v = SmallVec::new();
    if let ModifierSettings::Mask(m) = s
        && let Some(o) = m.armature
    {
        v.push(o);
    }
    v
}

#[modifier(
    MASK,
    kind = Nonconstructive,
    flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE,
    depends_on = deps,
    krate = crate
)]
fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> {
    let ModifierSettings::Mask(s) = &md.settings else {
        return Ok(mesh);
    };
    // The armature object: `is_disabled` when it is not an armature; `None` when unset or
    // without a pose.
    let armature = match s
        .armature
        .and_then(IdRef::<ObjectId>::from_any)
        .and_then(|r| cx.main().objects.get(r))
    {
        Some(ob) if ob.object_type() != "ARMATURE" => return Ok(mesh),
        Some(ob) => Some(ArmatureInfo::of(cx.main(), ob)),
        None => None,
    };
    Ok(mask(s, armature.as_ref(), mesh))
}

/// What ARMATURE mode reads of the armature object.
#[derive(Clone, Debug, Default)]
pub(crate) struct ArmatureInfo {
    /// `armature_ob->pose != nullptr`.
    has_pose: bool,
    /// The names of the selected pose channels that have a bone.
    selected: Vec<smol_str::SmolStr>,
}

impl ArmatureInfo {
    /// The evaluated armature object `ob`: its pose is rebuilt from the armature's bones when
    /// missing (`BKE_pose_rebuild` in the depsgraph's `build_rig`), so it always has one, with
    /// new channels unselected.
    fn of(main: &Main, ob: &Object) -> ArmatureInfo {
        let arm = ob
            .data
            .and_then(IdRef::<ArmatureId>::from_any)
            .and_then(|r| main.armatures.get(r));
        let selected = ob.pose.as_ref().map_or_else(Vec::new, |pose| {
            pose.bones
                .iter()
                .filter(|pb| pb.select && arm.is_some_and(|a| a.bone(&pb.name).is_some()))
                .map(|pb| pb.name.clone())
                .collect()
        });
        ArmatureInfo {
            has_pose: arm.is_some() || ob.pose.is_some(),
            selected,
        }
    }

    /// `pchan && pchan->bone_get(ob) && (pchan->flag & POSE_SELECTED)` for the channel named
    /// like the group (`BKE_pose_channel_find_name`).
    fn selected_bone_uses_group(&self, group: &str) -> bool {
        self.selected.iter().any(|n| n == group)
    }
}

// Ported from blender/source/blender/modifiers/intern/MOD_mask.cc (modify_mesh and its helpers).
/// The MASK result of `mesh`.
pub(crate) fn mask(s: &MaskSettings, armature: Option<&ArmatureInfo>, mesh: Mesh) -> Mesh {
    let invert_mask = s.invert_vertex_group;
    let use_interpolation = s.mode == "VERTEX_GROUP" && s.use_smooth;
    let dverts: &[DeformVert] = mesh.deform_verts().unwrap_or(&[]);
    if dverts.is_empty() {
        return if invert_mask {
            mesh
        } else {
            rebuild(&mesh, Topology::default(), &Sources::default())
        };
    }
    if !matches!(s.mode.as_str(), "ARMATURE" | "VERTEX_GROUP")
        || mesh.verts_num() == 0
        || mesh.vertex_group_names.is_empty()
    {
        return mesh;
    }
    let mut defgrp_index = 0u32;
    let mut vertex_mask: Vec<bool>;
    if s.mode == "ARMATURE" {
        let Some(arm) = armature.filter(|a| a.has_pose) else {
            return mesh;
        };
        let uses: Vec<bool> = mesh
            .vertex_group_names
            .iter()
            .map(|g| arm.selected_bone_uses_group(g))
            .collect();
        vertex_mask = dverts
            .iter()
            .map(|dv| {
                dv.weights.iter().any(|&(g, w)| {
                    uses.get(g as usize).copied().unwrap_or(false) && w > s.threshold
                })
            })
            .collect();
    } else {
        let Some(index) = defgroup_index(&mesh, &s.vertex_group) else {
            return mesh;
        };
        defgrp_index = index;
        vertex_mask = dverts
            .iter()
            .map(|dv| find_weight(dv, index) > s.threshold)
            .collect();
    }
    if invert_mask {
        for m in &mut vertex_mask {
            *m = !*m;
        }
    }
    let weight = |v: usize| find_weight(&dverts[v], defgrp_index);
    let interp_fac = |v1: usize, v2: usize| {
        let (w1, w2) = (weight(v1), weight(v2));
        (s.threshold - w1) / (w2 - w1)
    };

    // compute_masked_verts.
    let mut vertex_map = vec![-1i32; mesh.verts_num()];
    let mut verts_masked_num = 0usize;
    for (i, m) in vertex_mask.iter().enumerate() {
        if *m {
            vertex_map[i] = verts_masked_num as i32;
            verts_masked_num += 1;
        }
    }
    // computed_masked_edges(_smooth).
    let edges_src = mesh.edges();
    let mut edge_map = vec![-1i32; edges_src.len()];
    let mut edges_masked_num = 0usize;
    let mut verts_add_num = 0usize;
    for (i, e) in edges_src.iter().enumerate() {
        let (v1, v2) = (vertex_mask[e[0] as usize], vertex_mask[e[1] as usize]);
        if v1 && v2 {
            edge_map[i] = edges_masked_num as i32;
            edges_masked_num += 1;
        } else if use_interpolation && v1 != v2 {
            edge_map[i] = -2;
            verts_add_num += 1;
        }
    }
    edges_masked_num += verts_add_num;
    // computed_masked_faces.
    let cv = mesh.corner_verts();
    let ce = mesh.corner_edges();
    let mut masked_face_indices: Vec<usize> = Vec::new();
    let mut loop_starts: Vec<usize> = Vec::new();
    let mut loops_masked_num = 0usize;
    for f in 0..mesh.faces_num() {
        let r = mesh.face(f);
        if cv[r.clone()].iter().all(|&v| vertex_mask[v as usize]) {
            masked_face_indices.push(f);
            loop_starts.push(loops_masked_num);
            loops_masked_num += r.len();
        }
    }
    let faces_masked_num = masked_face_indices.len();
    // compute_interpolated_faces.
    let (mut edges_add_num, mut faces_add_num, mut loops_add_num) = (0usize, 0usize, 0usize);
    if use_interpolation {
        for f in 0..mesh.faces_num() {
            let r = mesh.face(f);
            let fv = &cv[r.clone()];
            let n = fv.len();
            let mut in_count = 0usize;
            let mut start: Option<usize> = None;
            for (j, &v) in fv.iter().enumerate() {
                if vertex_mask[v as usize] {
                    in_count += 1;
                } else if start.is_none() {
                    start = Some(j);
                }
            }
            if in_count > 0 && in_count < n {
                let start = start.expect("a vertex outside the mask");
                let mut last_in = vertex_mask[fv[start] as usize];
                let mut dst_totloop = 0usize;
                for j in 0..n {
                    let v_in = vertex_mask[fv[(start + 1 + j) % n] as usize];
                    if v_in && !last_in {
                        dst_totloop = 3;
                    } else if !v_in && last_in {
                        masked_face_indices.push(f);
                        loop_starts.push(loops_masked_num + loops_add_num);
                        loops_add_num += dst_totloop;
                        faces_add_num += 1;
                        edges_add_num += 1;
                    } else if v_in && last_in {
                        dst_totloop += 1;
                    }
                    last_in = v_in;
                }
            }
        }
    }

    let verts_total = verts_masked_num + verts_add_num;
    let edges_total = edges_masked_num + edges_add_num;
    let faces_total = faces_masked_num + faces_add_num;
    let loops_total = loops_masked_num + loops_add_num;
    let mut src = Sources {
        vert: vec![Src::Default; verts_total],
        edge: vec![Src::Default; edges_total],
        face: vec![Src::Default; faces_total],
        corner: vec![Src::Default; loops_total],
    };
    let mut dst_edges = vec![[0i32; 2]; edges_total];
    // copy_masked_verts_to_new_mesh.
    for (i, &d) in vertex_map.iter().enumerate() {
        if d != -1 {
            src.vert[d as usize] = Src::Copy(i);
        }
    }
    if use_interpolation {
        // add_interp_verts_copy_edges_to_new_mesh.
        let mut vert_index = verts_total - verts_add_num;
        let mut edge_index = edges_masked_num - verts_add_num;
        for (i, e) in edges_src.iter().enumerate() {
            if edge_map[i] != -1 {
                let i_dst = if edge_map[i] == -2 {
                    edge_index
                } else {
                    edge_map[i] as usize
                };
                src.edge[i_dst] = Src::Copy(i);
                dst_edges[i_dst] = [vertex_map[e[0] as usize], vertex_map[e[1] as usize]];
            }
            if edge_map[i] == -2 {
                let i_dst = edge_index;
                edge_index += 1;
                edge_map[i] = i_dst as i32;
                let e_dst = &mut dst_edges[i_dst];
                if !vertex_mask[e[0] as usize] {
                    e_dst[0] = vert_index as i32;
                } else {
                    e_dst[1] = e_dst[0];
                    e_dst[0] = vert_index as i32;
                }
                let fac = interp_fac(e[0] as usize, e[1] as usize);
                src.vert[vert_index] =
                    Src::Mix(smallvec![(e[0] as usize, 1.0 - fac), (e[1] as usize, fac)]);
                vert_index += 1;
            }
        }
    } else {
        // copy_masked_edges_to_new_mesh.
        for (i, e) in edges_src.iter().enumerate() {
            let d = edge_map[i];
            if d < 0 {
                continue;
            }
            src.edge[d as usize] = Src::Copy(i);
            dst_edges[d as usize] = [vertex_map[e[0] as usize], vertex_map[e[1] as usize]];
        }
    }
    // copy_masked_faces_to_new_mesh.
    let mut face_offsets = vec![0i32; faces_total + 1];
    let mut dst_cv = vec![0i32; loops_total];
    let mut dst_ce = vec![0i32; loops_total];
    for i_dst in 0..faces_masked_num {
        let f = masked_face_indices[i_dst];
        let r = mesh.face(f);
        let start = loop_starts[i_dst];
        face_offsets[i_dst] = start as i32;
        src.face[i_dst] = Src::Copy(f);
        for (k, c) in r.enumerate() {
            src.corner[start + k] = Src::Copy(c);
            dst_cv[start + k] = vertex_map[cv[c] as usize];
            dst_ce[start + k] = edge_map[ce[c] as usize];
        }
    }
    if use_interpolation {
        // add_interpolated_faces_to_new_mesh.
        let mut edge_index = edges_total - edges_add_num;
        let mut sub_face_index = 0i32;
        let mut last_i_src: Option<usize> = None;
        for i_dst in faces_masked_num..masked_face_indices.len() {
            let i_src = masked_face_indices[i_dst];
            if last_i_src == Some(i_src) {
                sub_face_index += 1;
            } else {
                sub_face_index = 0;
                last_i_src = Some(i_src);
            }
            let r = mesh.face(i_src);
            let i_ml_src = r.start;
            let n = r.len();
            let mut i_ml_dst = loop_starts[i_dst];
            src.face[i_dst] = Src::Copy(i_src);
            face_offsets[i_dst] = i_ml_dst as i32;
            let fv = &cv[r.clone()];
            let fe = &ce[r];
            // Ring search starting at a vertex which is not included in the mask.
            let mut start: i32 = -sub_face_index - 1;
            let mut skip = false;
            for (j, &v) in fv.iter().enumerate() {
                if !vertex_mask[v as usize] {
                    if start == -1 {
                        start = j as i32;
                        break;
                    }
                    if !skip {
                        skip = true;
                    }
                } else if skip {
                    skip = false;
                    start += 1;
                }
            }
            let start = start.max(0) as usize;
            let mut last_index = start;
            let mut last_in = vertex_mask[fv[last_index] as usize];
            for j in 0..n {
                let index = (start + 1 + j) % n;
                let v_in = vertex_mask[fv[index] as usize];
                if v_in && !last_in {
                    // Start a new cut.
                    let fac = interp_fac(fv[last_index] as usize, fv[index] as usize);
                    src.corner[i_ml_dst] = Src::Mix(smallvec![
                        (i_ml_src + last_index, 1.0 - fac),
                        (i_ml_src + index, fac)
                    ]);
                    dst_ce[i_ml_dst] = edge_map[fe[last_index] as usize];
                    dst_cv[i_ml_dst] = dst_edges[dst_ce[i_ml_dst] as usize][0];
                    i_ml_dst += 1;
                    src.corner[i_ml_dst] = Src::Copy(i_ml_src + index);
                    dst_cv[i_ml_dst] = vertex_map[fv[index] as usize];
                    dst_ce[i_ml_dst] = edge_map[fe[index] as usize];
                    i_ml_dst += 1;
                } else if !v_in && last_in {
                    // End the active cut.
                    let fac = interp_fac(fv[last_index] as usize, fv[index] as usize);
                    src.corner[i_ml_dst] = Src::Mix(smallvec![
                        (i_ml_src + last_index, 1.0 - fac),
                        (i_ml_src + index, fac)
                    ]);
                    dst_ce[i_ml_dst] = edge_index as i32;
                    dst_cv[i_ml_dst] = dst_edges[edge_map[fe[last_index] as usize] as usize][0];
                    // The closing edge.
                    dst_edges[edge_index] =
                        [dst_cv[face_offsets[i_dst] as usize], dst_cv[i_ml_dst]];
                    edge_index += 1;
                    // Only one cut per iteration.
                    break;
                } else if v_in && last_in {
                    // Extend the active face.
                    src.corner[i_ml_dst] = Src::Copy(i_ml_src + index);
                    dst_cv[i_ml_dst] = vertex_map[fv[index] as usize];
                    dst_ce[i_ml_dst] = edge_map[fe[index] as usize];
                    i_ml_dst += 1;
                }
                last_index = index;
                last_in = v_in;
            }
        }
    }
    face_offsets[faces_total] = loops_total as i32;
    let topo = Topology {
        edges: dst_edges,
        face_offsets,
        corner_verts: dst_cv,
        corner_edges: dst_ce,
    };
    rebuild(&mesh, topo, &src)
}

#[cfg(test)]
mod tests {
    use super::super::smooth::common::testing::*;
    use super::*;

    fn settings(p: &serde_json::Value) -> MaskSettings {
        MaskSettings {
            mode: p["mode"].as_str().unwrap().into(),
            armature: None,
            vertex_group: p["vertex_group"].as_str().unwrap().into(),
            invert_vertex_group: p["invert_vertex_group"].as_bool().unwrap(),
            use_smooth: p["use_smooth"].as_bool().unwrap(),
            threshold: p["threshold"].as_f64().unwrap() as f32,
        }
    }

    /// Blender 5.2.4 dumps (`tests/golden/fixtures/modifiers/b/vgroups/mask.json`).
    #[test]
    fn matches_blender() {
        let d = load("vgroups/mask.json");
        for case in d["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let mesh = mesh_from_dump(&case["input"]);
            let s = settings(&case["props"]);
            let out = mask(&s, None, mesh);
            assert_topology(&out, &case["output"]);
            assert_positions(&out, &case["output"], 1e-5);
            // Interpolated weights.
            let want = case["output"]["weights"].as_array().unwrap();
            let none = vec![DeformVert::default(); out.verts_num()];
            let got = out.deform_verts().unwrap_or(&none);
            assert_eq!(got.len(), want.len(), "{name}: weights");
            for (g, w) in got.iter().zip(want) {
                let w: Vec<(u32, f32)> = w
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| (p[0].as_u64().unwrap() as u32, p[1].as_f64().unwrap() as f32))
                    .collect();
                assert_eq!(g.weights.len(), w.len(), "{name}");
                for (a, b) in g.weights.iter().zip(&w) {
                    assert_eq!(a.0, b.0, "{name}");
                    assert!((a.1 - b.1).abs() < 1e-5, "{name}: {a:?} vs {b:?}");
                }
            }
        }
    }

    #[test]
    fn armature_without_bones_selects_nothing() {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        m.vertex_group_names = vec!["Bone".into()];
        *m.deform_verts_mut() = vec![
            DeformVert {
                weights: smallvec![(0, 1.0)]
            };
            8
        ];
        let s = MaskSettings {
            mode: "ARMATURE".into(),
            ..MaskSettings::default()
        };
        let arm = ArmatureInfo {
            has_pose: true,
            selected: Vec::new(),
        };
        assert_eq!(mask(&s, Some(&arm), m.clone()).verts_num(), 0);
        let inv = MaskSettings {
            invert_vertex_group: true,
            ..s.clone()
        };
        assert_eq!(mask(&inv, Some(&arm), m.clone()).verts_num(), 8);
        // No armature: unchanged.
        assert_eq!(mask(&s, None, m).verts_num(), 8);
    }

    /// Two groups named like bones, the cube's top in "Top" (0.8) and its +X half in "Side"
    /// (0.2 or 0.6), and one group without a bone.
    fn cube_with_bone_groups() -> Mesh {
        let mut m = frap_mesh::primitives::cube(2.0, false, [1.0; 3]);
        m.vertex_group_names = vec!["Top".into(), "Side".into(), "Loose".into()];
        let pos = m.positions().to_vec();
        *m.deform_verts_mut() = pos
            .iter()
            .map(|p| {
                let mut w = SmallVec::new();
                if p[2] > 0.0 {
                    w.push((0, 0.8));
                }
                if p[0] > 0.0 {
                    w.push((1, if p[1] > 0.0 { 0.6 } else { 0.2 }));
                }
                w.push((2, 1.0));
                DeformVert { weights: w }
            })
            .collect();
        m
    }

    #[test]
    fn armature_mode_keeps_vertices_of_selected_bones_above_threshold() {
        let m = cube_with_bone_groups();
        let s = MaskSettings {
            mode: "ARMATURE".into(),
            threshold: 0.5,
            ..MaskSettings::default()
        };
        let arm = |names: &[&str]| ArmatureInfo {
            has_pose: true,
            selected: names.iter().map(|n| (*n).into()).collect(),
        };
        // "Top": the 4 top vertices.
        assert_eq!(mask(&s, Some(&arm(&["Top"])), m.clone()).verts_num(), 4);
        // "Side": only the 2 vertices weighing 0.6 (0.2 is below the threshold).
        assert_eq!(mask(&s, Some(&arm(&["Side"])), m.clone()).verts_num(), 2);
        // Both: the union (top 4 plus the bottom (+X, +Y) vertex).
        assert_eq!(mask(&s, Some(&arm(&["Top", "Side"])), m.clone()).verts_num(), 5);
        // Inverted: the complement.
        let inv = MaskSettings {
            invert_vertex_group: true,
            ..s.clone()
        };
        assert_eq!(mask(&inv, Some(&arm(&["Top", "Side"])), m.clone()).verts_num(), 3);
        // `use_smooth` is VERTEX_GROUP only: no interpolated vertices.
        let smooth = MaskSettings {
            use_smooth: true,
            ..s.clone()
        };
        assert_eq!(mask(&smooth, Some(&arm(&["Top"])), m).verts_num(), 4);
    }

    #[test]
    fn armature_info_reads_selected_channels_with_bones() {
        use frap_scene::id::{Armature, Bone, Pose};
        let mut main = Main::default();
        let mut log = frap_scene::ChangeLog::default();
        let mut a = Armature::new("Arm");
        a.bones = vec![Bone::new("Top"), Bone::new("Side")];
        let ar = main.armatures.insert("Arm", a.clone(), &mut log);
        let mut ob = Object::new("Arm", Some(ar.any()));
        // Never built: the evaluated object has a pose, with nothing selected.
        let info = ArmatureInfo::of(&main, &ob);
        assert!(info.has_pose);
        assert!(info.selected.is_empty());
        Pose::rebuild(&mut ob.pose, &a);
        let pose = ob.pose.as_mut().unwrap();
        pose.bones[0].select = true;
        // A selected channel without a bone does not count.
        pose.ensure("Ghost").select = true;
        let info = ArmatureInfo::of(&main, &ob);
        assert_eq!(info.selected, vec![smol_str::SmolStr::from("Top")]);
        assert!(info.selected_bone_uses_group("Top"));
        assert!(!info.selected_bone_uses_group("Side"));
        assert!(!info.selected_bone_uses_group("Ghost"));
    }
}
