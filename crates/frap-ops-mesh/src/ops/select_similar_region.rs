// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.select_similar_region()`, tier T1 (`tests/golden/cases/edit/select_m8/
//! similar_region.py`). Port of `edbm_select_similar_region_exec`
//! (`editors/mesh/editmesh_select.cc`, Blender 5.2.4, GPL-2.0-or-later); the topology match is
//! `frap_bmesh::ops::similar::region_match` (`BM_mesh_region_match`).
//!
//! On the context edit mesh: each group of selected faces connected through edges is a source
//! region; every other face region with the same topology (found from the region's innermost
//! edge) gets selected. Fewer than two selected faces is Blender's error `No face regions
//! selected` (`CANCELLED`); when nothing matched the WARNING `No matching face regions found`
//! is reported (still `FINISHED`).

use frap_bmesh::ops::similar::{region_match, selected_face_groups};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh;

use super::select_more::common;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, _p: mesh::SelectSimilarRegion) -> OpResult {
    let me = {
        let c = cx.ctx();
        poll::edit_object(&c).and_then(|(_, o)| o.data?.typed::<MeshId>())
    };
    let Some(me) = me else {
        return Ok(OpStatus::Cancelled);
    };
    let Some(regions) = common::read(cx, me, |bm| {
        if bm.selected_counts().2 < 2 {
            return None;
        }
        let mut found = Vec::new();
        for group in selected_face_groups(bm) {
            found.extend(region_match(bm, &group));
        }
        Some(found)
    })
    .flatten() else {
        return Err(OpError::Runtime("No face regions selected".into()));
    };
    if regions.is_empty() {
        cx.report(ReportType::Warning, "No matching face regions found");
        return Ok(OpStatus::Finished);
    }
    let bm = cx.bmesh_mut(me)?;
    for region in regions {
        for f in region {
            bm.face_select_set(f, true);
        }
    }
    Ok(OpStatus::Finished)
}
