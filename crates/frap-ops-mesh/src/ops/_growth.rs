// SPDX-License-Identifier: GPL-3.0-or-later
//! (Not an operator: the leading `_` keeps it out of the registry test.) `--max-elements`
//! pre-estimates of the multiplicative edit-mode operators (ARCHITECTURE §12,
//! [`frap_math::budget::reserve_corners`]): `mesh.subdivide`, `mesh.subdivide_edgering`,
//! `mesh.loopcut`, `mesh.spin`, `mesh.screw` and `mesh.bevel` estimate the face corners every
//! mesh in Edit Mode will hold from their counts (cuts, steps, segments) and the selection, so
//! an absurd request (`subdivide(number_cuts=100)` twice) fails in microseconds with
//! `LIMIT_EXCEEDED` instead of allocating gigabytes. The estimates are close upper bounds for
//! the common cases (exact for fully selected quads and triangles under `subdivide`); growth
//! they miss is still stopped by the BMesh allocation backstop
//! ([`frap_math::budget::check_corners`]).

use frap_bmesh::{BMesh, ElemFlag};
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId};

/// Selected and visible (`BM_ELEM_SELECT` without `BM_ELEM_HIDDEN`).
fn usable(flag: ElemFlag) -> bool {
    flag.contains(ElemFlag::SELECT) && !flag.contains(ElemFlag::HIDDEN)
}

/// Current face corners of `bm` (`bm->totloop`).
pub(crate) fn corners(bm: &BMesh) -> u64 {
    bm.loops_num() as u64
}

/// Visible faces with their corner count and how many of their edges are selected.
fn faces_by_selected_edges(bm: &BMesh) -> impl Iterator<Item = (u64, u64)> + '_ {
    bm.faces
        .iter()
        .filter(|(_, f)| !f.flag.contains(ElemFlag::HIDDEN))
        .map(|(_, f)| {
            let mut l = f.l_first;
            let mut k = 0u64;
            for _ in 0..f.len {
                let lp = bm.lp(l);
                if usable(bm.edge(lp.e).flag) {
                    k += 1;
                }
                l = lp.next;
            }
            (u64::from(f.len), k)
        })
}

/// Face corners after `mesh.subdivide` with `cuts`: a face whose edges are all cut becomes a
/// grid of `(cuts + 1)²` faces (exact for quads and triangles), a partially cut face at most
/// `cuts + 1` faces of its grown outline (`len + k · cuts` corners for `k` cut edges).
pub(crate) fn subdivide_corners(bm: &BMesh, cuts: u64) -> Option<u64> {
    let c1 = cuts.checked_add(1)?;
    let mut total = corners(bm);
    for (len, k) in faces_by_selected_edges(bm) {
        let after = if k == 0 {
            continue;
        } else if k == len {
            len.checked_mul(c1.checked_mul(c1)?)?
        } else {
            len.checked_add(k.checked_mul(cuts)?)?.checked_mul(c1)?
        };
        total = total.checked_add(after)?.checked_sub(len)?;
    }
    Some(total)
}

/// Face corners after an edge-ring cut with `cuts` (`mesh.loopcut`,
/// `mesh.subdivide_edgering`): a face crossed by the ring (two selected edges) becomes
/// `cuts + 1` faces; a face with one selected edge gains `cuts` corners; a face where rings
/// cross is grid-filled (`(cuts + 1)²` faces).
pub(crate) fn ring_cut_corners(bm: &BMesh, cuts: u64) -> Option<u64> {
    let c1 = cuts.checked_add(1)?;
    let mut total = corners(bm);
    for (len, k) in faces_by_selected_edges(bm) {
        let added = match k {
            0 => continue,
            1 => cuts,
            2 => len.checked_mul(cuts)?,
            // Intersecting rings: grid fill (#31939).
            _ => len.checked_mul(c1.checked_mul(c1)?.checked_sub(1)?)?,
        };
        total = total.checked_add(added)?;
    }
    Some(total)
}

/// Face corners after `steps` extrusions of the selection (`spin`, `screw`): each step
/// duplicates the selected vertices, joins each to its copy, adds one side quad per selected
/// edge and moves (or, with `dupli`, copies) the selected faces.
pub(crate) fn extrude_steps_corners(bm: &BMesh, steps: u64, dupli: bool) -> Option<u64> {
    let se = bm.edges.iter().filter(|(_, e)| usable(e.flag)).count() as u64;
    let sc: u64 = bm
        .faces
        .iter()
        .filter(|(_, f)| usable(f.flag))
        .map(|(_, f)| u64::from(f.len))
        .sum();
    let per_step = (4 * se).checked_add(if dupli { sc } else { 0 })?;
    corners(bm)
        .checked_add(per_step.checked_mul(steps)?)?
        .checked_add(sc)
}

/// Face corners after `mesh.bevel` with `segments`: a strip of `segments` quads along every
/// selected edge (edge bevel), a vertex patch of about `valence · (segments / 2 + 1)²` quads at
/// every beveled vertex, and `segments + 1` more corners on each face corner at a beveled
/// vertex.
pub(crate) fn bevel_corners(bm: &BMesh, segments: u64, affect_vertices: bool) -> Option<u64> {
    let mut total = corners(bm);
    let seg = segments.max(1);
    let half = seg / 2 + 1;
    let patch = half.checked_mul(half)?.checked_mul(4)?;
    for (s, v) in bm.verts.iter() {
        if v.flag.contains(ElemFlag::HIDDEN) {
            continue;
        }
        let edges = bm.vert_edges(frap_bmesh::VertId(s));
        let beveled = if affect_vertices {
            usable(v.flag)
        } else {
            edges.iter().any(|&e| usable(bm.edge(e).flag))
        };
        if !beveled {
            continue;
        }
        let valence = edges.len() as u64;
        let per_vert = patch.checked_add(seg.checked_add(1)?.checked_mul(2)?)?;
        total = total.checked_add(valence.checked_mul(per_vert)?)?;
    }
    if !affect_vertices {
        let se = bm.edges.iter().filter(|(_, e)| usable(e.flag)).count() as u64;
        total = total.checked_add(se.checked_mul(seg)?.checked_mul(4)?)?;
    }
    Some(total)
}

/// The pre-check over every mesh in Edit Mode (one statement, one budget): `after(mesh, bm)`
/// estimates the face corners `mesh` holds once the operator ran (`None`: the estimate
/// overflowed).
pub(crate) fn reserve(
    cx: &mut OpCtx,
    mut after: impl FnMut(IdRef<MeshId>, &BMesh) -> Option<u64>,
) -> Result<(), OpError> {
    let meshes = helpers::iter_edit_meshes(cx);
    let (_, edit) = cx.split_edit();
    let mut seen: Vec<IdRef<MeshId>> = Vec::new();
    let mut total = Some(0u64);
    for (_, me) in meshes {
        if seen.contains(&me) {
            continue;
        }
        seen.push(me);
        let Some(em) = edit.meshes.get(&me.key()) else {
            continue;
        };
        total = total
            .zip(after(me, &em.bm))
            .and_then(|(t, n)| t.checked_add(n));
    }
    frap_math::budget::reserve_corners(total, total).map_err(|e| OpError::Limit(e.to_string()))
}
