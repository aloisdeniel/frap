// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.dissolve_faces` (Blender 5.2.4 `bmo_dissolve_faces_exec`, `operators/
//! bmo_dissolve.cc`), implemented by WP 2.7 (Delete, dissolve, merge).
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.dissolve_faces(bmesh, faces=[], use_verts=False)`.
//!
//! **Create/kill order** (ported from blender/source/blender/bmesh/operators/bmo_dissolve.cc):
//! the input faces are grouped, in input order, into manifold regions (`BMW_ISLAND_MANIFOLD`,
//! breadth-first); a region of one face is left alone. Each region of two or more faces is
//! joined with `BM_faces_join(do_del = true)`: the new n-gon is **created** first (taking the
//! next free face slot), then the region's interior edges (and with them its old faces) and
//! interior vertices are killed. When the result duplicates an existing face, the new face is
//! killed and the existing one kept. Leftover region faces are deleted (`context = 'FACES'`).
//! With `use_verts`, vertices that end up between exactly two edges (and did not start that
//! way) are dissolved in vertex order (`BM_vert_collapse_edge`).
//!
//! Operator type flags: `NORMALS_CALC | SELECT_FLUSH | SELECT_VALIDATE`.

use crate::ops::delete::{DeleteIn, delete};
use crate::ops::dissolve_verts::{bm_vert_collapse_edge_and_merge, mods};
use crate::types::OpTypeFlag;
use crate::walkers::WalkOpts;
use crate::{BMElem, BMesh, BmError, FaceId};

/// Input slots of `dissolve_faces` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DissolveFacesIn {
    /// `faces`.
    pub faces: Vec<FaceId>,
    /// `use_verts`.
    pub use_verts: bool,
}

/// Output slots of `dissolve_faces`.
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct DissolveFacesOut {
    /// `region.out` as elements.
    pub geom: Vec<BMElem>,
    /// `region.out`: the joined faces, in face order.
    pub region: Vec<FaceId>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_dissolve_faces_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC
    .union(OpTypeFlag::SELECT_FLUSH)
    .union(OpTypeFlag::SELECT_VALIDATE);

const FACE_MARK: u16 = 1;
const FACE_ORIG: u16 = 2;
const FACE_NEW: u16 = 4;
const FACE_TAG: u16 = 8;
const VERT_MARK: u16 = 1;

/// Runs `dissolve_faces` on `bm`.
pub fn dissolve_faces(bm: &mut BMesh, i: &DissolveFacesIn) -> Result<DissolveFacesOut, BmError> {
    if let Some(f) = i.faces.iter().find(|f| !bm.faces.contains(f.0)) {
        return Err(BmError::Invalid(format!("dissolve_faces: dead face {f:?}")));
    }
    let region = bm.bmo_exec(TYPE_FLAG, |bm| {
        dissolve_faces_exec(bm, &i.faces, i.use_verts)
    });
    Ok(DissolveFacesOut {
        geom: region.iter().map(|&f| BMElem::Face(f)).collect(),
        region,
    })
}

fn dissolve_faces_exec(bm: &mut BMesh, faces: &[FaceId], use_verts: bool) -> Vec<FaceId> {
    if use_verts {
        // Tag verts that start out with only 2 edges, don't remove these later.
        for v in bm.vert_ids() {
            let mark = !mods::vert_is_edge_pair(bm, v);
            bm.oflag_set(v, VERT_MARK, mark);
        }
    }
    for &f in faces {
        bm.oflag_enable(f, FACE_MARK | FACE_TAG);
    }

    // Collect regions.
    let mut regions: Vec<Vec<FaceId>> = Vec::new();
    for &f in faces {
        if !bm.faces.contains(f.0) || !bm.oflag_test(f, FACE_TAG) {
            continue;
        }
        let mask = |bm: &BMesh, x: FaceId| bm.oflag_test(x, FACE_MARK);
        let o = WalkOpts {
            test_hidden: false,
            mask_face: Some(&mask),
            ..WalkOpts::default()
        };
        let region = bm.walk_face_island_ex(f, &o, true);
        if region.len() >= 2 {
            for &x in &region {
                bm.oflag_disable(x, FACE_TAG);
                bm.oflag_enable(x, FACE_ORIG);
            }
            regions.push(region);
        }
    }

    // Track how many faces we should end up with.
    let mut totface_target = bm.faces_num() as i64;
    for region in &regions {
        frap_math::budget::check();
        let (f_new, f_double) = bm.faces_join(region, true, true);
        if let Some(mut f_new) = f_new {
            // All the joined faces are gone and the fresh `f_new` is their union.
            totface_target -= region.len() as i64 - 1;
            if let Some(fd) = f_double {
                // Keep the pre-existing face and its custom data.
                bm.face_kill(f_new);
                totface_target -= 1;
                f_new = fd;
            }
            bm.oflag_disable(f_new, FACE_ORIG);
            bm.oflag_enable(f_new, FACE_NEW);
        } else {
            // Prevent these faces from being removed.
            for &x in region {
                if bm.faces.contains(x.0) {
                    bm.oflag_disable(x, FACE_ORIG);
                }
            }
        }
    }

    // Typically no faces need to be deleted.
    if totface_target != bm.faces_num() as i64 {
        let geom: Vec<BMElem> = bm
            .face_ids()
            .into_iter()
            .filter(|&f| bm.oflag_test(f, FACE_ORIG))
            .map(BMElem::Face)
            .collect();
        let _ = delete(
            bm,
            &DeleteIn {
                geom,
                context: "FACES",
            },
        );
    }

    if use_verts {
        for v in bm.vert_ids() {
            if !bm.verts.contains(v.0) || !bm.oflag_test(v, VERT_MARK) {
                continue;
            }
            if mods::vert_is_edge_pair(bm, v) {
                bm_vert_collapse_edge_and_merge(bm, v, true);
            }
        }
    }

    bm.face_ids()
        .into_iter()
        .filter(|&f| bm.oflag_test(f, FACE_NEW))
        .collect()
}
