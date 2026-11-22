// SPDX-License-Identifier: GPL-3.0-or-later
//! Smart UV Project (`smart_project_exec` of `uvedit_unwrap_ops.cc`): groups faces by their
//! projection normals (`smart_uv_project_calculate_project_normals`, `angle_limit`,
//! `area_weight`), projects each group onto its normal's plane ([`project_face`]), then the
//! caller packs the islands ([`crate::pack`]) with `rotate_method`, `margin_method` and
//! `island_margin`. Layout equal to Blender's headless output [MESH App. B.5].
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_unwrap_ops.cc (Blender 5.2.4).
//! The face sort is macOS's C library `qsort` ([`crate::pack::qsort_libc`], what the reference
//! build calls), because faces of equal area are common and their order decides the groups.

use core::cmp::Ordering;

use crate::pack::qsort_libc;
use crate::param::bli::{self, V3};

/// One projection group: the normal and the faces projected along it (face indices in the
/// caller's order).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SmartGroup {
    /// Projection normal.
    pub normal: [f32; 3],
    /// Faces of the group.
    pub faces: Vec<u32>,
}

/// `smart_uv_project_area_ignore`: faces at or below this area get zero UVs.
pub const AREA_IGNORE: f32 = 1e-12;

/// `smart_uv_project_calculate_project_normals`.
fn calculate_project_normals(
    thick: &[usize],
    normals: &[[f32; 3]],
    areas: &[f32],
    limit_half_cos: f32,
    limit_cos: f32,
    area_weight: f32,
) -> Vec<V3> {
    if thick.is_empty() {
        return Vec::new();
    }
    let mut tag = vec![false; thick.len()];
    let mut project_normal = normals[thick[0]];
    let mut project_thick: Vec<usize> = Vec::new();
    let mut out: Vec<V3> = Vec::new();
    loop {
        frap_math::budget::check();
        for fi in (0..thick.len()).rev() {
            if tag[fi] {
                continue;
            }
            if bli::dot3(normals[thick[fi]], project_normal) > limit_half_cos {
                project_thick.push(fi);
                tag[fi] = true;
            }
        }
        let mut average = [0.0f32; 3];
        for &fi in &project_thick {
            let no = normals[thick[fi]];
            let w = if area_weight <= 0.0 {
                1.0
            } else if area_weight >= 1.0 {
                areas[thick[fi]]
            } else {
                areas[thick[fi]] * area_weight + (1.0 - area_weight)
            };
            if area_weight <= 0.0 {
                average = [average[0] + no[0], average[1] + no[1], average[2] + no[2]];
            } else {
                average = [
                    average[0] + no[0] * w,
                    average[1] + no[1] * w,
                    average[2] + no[2] * w,
                ];
            }
        }
        let (n, len) = bli::normalize3(average);
        if len != 0.0 {
            out.push(n);
        }

        // Find the most unique angle that points away from the other normals.
        let mut angle_best = 1.0f32;
        let mut angle_best_index = 0usize;
        for fi in (0..thick.len()).rev() {
            if tag[fi] {
                continue;
            }
            let mut angle_test = -1.0f32;
            for p in &out {
                angle_test = angle_test.max(bli::dot3(*p, normals[thick[fi]]));
            }
            if angle_test < angle_best {
                angle_best = angle_test;
                angle_best_index = fi;
            }
        }
        if angle_best < limit_cos {
            project_normal = normals[thick[angle_best_index]];
            project_thick.clear();
            project_thick.push(angle_best_index);
            tag[angle_best_index] = true;
        } else if !out.is_empty() {
            break;
        }
    }
    out
}

/// Groups the faces (`face_normals`, weighted by `face_areas`, both in the caller's face
/// order, typically the selected faces) into projection groups (`angle_limit` in radians).
/// Faces whose area is at most [`AREA_IGNORE`] belong to no group: Blender zeroes their UVs.
pub fn project_groups(
    face_normals: &[[f32; 3]],
    face_areas: &[f32],
    angle_limit: f32,
    area_weight: f32,
) -> Vec<SmartGroup> {
    let limit_cos = libm::cosf(angle_limit);
    let limit_half_cos = libm::cosf(angle_limit / 2.0);

    // `qsort(thick_faces, smart_uv_project_thickface_area_cmp_fn)`: largest area first.
    let mut thick: Vec<usize> = (0..face_normals.len()).collect();
    qsort_libc(&mut thick, &mut |&a: &usize, &b: &usize| {
        let (aa, ab) = (face_areas[a], face_areas[b]);
        if !(aa > AREA_IGNORE || ab > AREA_IGNORE) {
            return Ordering::Equal;
        }
        if aa < ab {
            Ordering::Greater
        } else if aa > ab {
            Ordering::Less
        } else {
            Ordering::Equal
        }
    });
    // Remove all zero area faces.
    while let Some(&last) = thick.last() {
        if face_areas[last] > AREA_IGNORE {
            break;
        }
        thick.pop();
    }

    let normals = calculate_project_normals(
        &thick,
        face_normals,
        face_areas,
        limit_half_cos,
        limit_cos,
        area_weight,
    );
    if normals.is_empty() {
        return Vec::new();
    }
    let mut groups: Vec<Vec<u32>> = vec![Vec::new(); normals.len()];
    for fi in (0..thick.len()).rev() {
        let no = face_normals[thick[fi]];
        let mut angle_best = bli::dot3(no, normals[0]);
        let mut best = 0usize;
        for (p, pn) in normals.iter().enumerate().skip(1) {
            let t = bli::dot3(no, *pn);
            if t > angle_best {
                angle_best = t;
                best = p;
            }
        }
        // `BLI_linklist_prepend`: the lists end up in ascending sorted order.
        groups[best].push(thick[fi] as u32);
    }
    normals
        .into_iter()
        .zip(groups)
        .filter(|(_, faces)| !faces.is_empty())
        .map(|(normal, mut faces)| {
            faces.reverse();
            SmartGroup { normal, faces }
        })
        .collect()
}

/// Projects one face of a group: `mul_v2_m3v3(uv, axis_dominant_v3_to_m3(normal), co)`.
pub fn project_face(normal: [f32; 3], cos: &[[f32; 3]], out: &mut [[f32; 2]]) {
    let m = bli::axis_dominant_v3_to_m3(normal);
    for (co, uv) in cos.iter().zip(out.iter_mut()) {
        *uv = bli::mul_v2_m3v3(&m, *co);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_normals_make_six_groups() {
        let normals = [
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
        ];
        let groups = project_groups(&normals, &[4.0; 6], 66f32.to_radians(), 0.0);
        assert_eq!(groups.len(), 6);
        let mut all: Vec<u32> = groups.iter().flat_map(|g| g.faces.clone()).collect();
        all.sort();
        assert_eq!(all, [0, 1, 2, 3, 4, 5]);
        for g in &groups {
            assert_eq!(g.normal, normals[g.faces[0] as usize]);
        }
    }

    #[test]
    fn zero_area_faces_are_dropped() {
        let normals = [[0.0, 0.0, 1.0], [0.0, 0.0, 1.0]];
        let groups = project_groups(&normals, &[1.0, 0.0], 1.0, 0.0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].faces, [0]);
    }

    #[test]
    fn projection_is_in_plane() {
        let mut uv = [[0.0; 2]; 2];
        project_face(
            [0.0, 0.0, 1.0],
            &[[1.0, 2.0, 3.0], [1.0, 2.0, -3.0]],
            &mut uv,
        );
        assert_eq!(uv[0], uv[1]);
    }
}
