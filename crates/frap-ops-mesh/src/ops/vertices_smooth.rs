// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.vertices_smooth(factor=0.0, repeat=1, xaxis=True, yaxis=True, zaxis=True)`,
//! tier T1 (`tests/golden/cases/edit/misc/vertices_smooth.py`). Port of
//! `edbm_do_smooth_vertex_exec` (`editors/mesh/editmesh_tools.cc`, Blender 5.2.4,
//! GPL-2.0-or-later).
//!
//! For every mesh in Edit Mode with a selected vertex (multi-object edit), `repeat` times
//! (0 counts as 1): `bmesh.ops.smooth_vert` on the selected vertices (each moves towards the
//! mean of its edge neighbours by `factor`, only along the enabled axes). The default
//! `factor=0` is a no-op; default cube, factor 0.5 → every coordinate ±2/3 ([MESH §4.14]).
//! A visible MIRROR modifier with `use_clip` keeps vertices within its `merge_threshold` of
//! each enabled mirror plane on the plane. Nothing selected anywhere: WARNING `No selected
//! vertex` and `CANCELLED`. frap meshes have no X-mirror editing (`use_mirror_x`), so the
//! mirror cache step of Blender never runs.

use frap_bmesh::ops::smooth_vert::{SmoothVertIn, smooth_vert};
use frap_op::operator;
use frap_op::prelude::*;
use frap_scene::id::ModifierSettings;
use frap_spec_ops_mesh::mesh;

use super::split::edbm;

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::VerticesSmooth) -> OpResult {
    let fac = p.factor.value;
    let repeat = if p.repeat.value == 0 { 1 } else { p.repeat.value };
    let mut tot_selected = 0;
    for (ob, me) in helpers::iter_edit_meshes(cx) {
        let (v, _, _) = edbm::read(cx, me, edbm::sel_counts).unwrap_or_default();
        if v == 0 {
            continue;
        }
        tot_selected += 1;
        // A MIRROR modifier with clipping flags vertices within its tolerance of the planes.
        let (mut mirr, mut clip_dist) = ([false; 3], 0.0f32);
        if let Some(o) = cx.main().objects.get(ob) {
            for md in &o.modifiers {
                if let ModifierSettings::Mirror(m) = &md.settings
                    && md.show_viewport
                    && m.use_clip
                {
                    for (k, on) in m.use_axis.iter().enumerate() {
                        if *on {
                            mirr[k] = true;
                        }
                    }
                    clip_dist = m.merge_threshold;
                }
            }
        }
        for _ in 0..repeat {
            let bm = cx.bmesh_mut(me)?;
            let verts = edbm::selected_verts(bm);
            let r = smooth_vert(
                bm,
                &SmoothVertIn {
                    verts,
                    factor: fac,
                    mirror_clip_x: mirr[0],
                    mirror_clip_y: mirr[1],
                    mirror_clip_z: mirr[2],
                    clip_dist,
                    use_axis_x: p.xaxis.value,
                    use_axis_y: p.yaxis.value,
                    use_axis_z: p.zaxis.value,
                },
            );
            edbm::finish(cx, r)?;
        }
    }
    if tot_selected == 0 {
        cx.report(ReportType::Warning, "No selected vertex");
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
