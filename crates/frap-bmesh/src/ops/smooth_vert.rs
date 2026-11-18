// SPDX-License-Identifier: GPL-3.0-or-later
//! `bmesh.ops.smooth_vert` (Blender 5.2.4 `bmo_smooth_vert_exec`, `operators/bmo_utils.cc`),
//! implemented by WP 2.10.
//!
//! Python signature (docs/research/blender-ops-mesh-uv.md, Appendix C):
//! `bmesh.ops.smooth_vert(bmesh, verts=[], factor=0.0, mirror_clip_x=False, mirror_clip_y=False, mirror_clip_z=False, clip_dist=0.0, use_axis_x=False, use_axis_y=False, use_axis_z=False)`.
//!
//! **Create/kill order**: nothing is created or killed. Every input vertex moves towards the
//! mean of its edge neighbours (`v' = v·(1−f) + mean·f`, computed for all vertices before any
//! moves); a vertex without edges keeps its position; with `mirror_clip_*` a coordinate
//! within `clip_dist` of the mirror plane is set to zero; only the `use_axis_*` coordinates
//! are written. Default cube, all vertices, factor 0.5: every coordinate becomes ±2/3
//! ([MESH §4.14]).
//!
//! Operator type flags: `NORMALS_CALC`.

use super::triangulate::math;
use crate::types::OpTypeFlag;
use crate::{BMElem, BMesh, BmError, VertId};

/// Input slots of `smooth_vert` (Blender's slot defaults in `Default`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SmoothVertIn {
    /// `verts`.
    pub verts: Vec<VertId>,
    /// `factor`.
    pub factor: f32,
    /// `mirror_clip_x`.
    pub mirror_clip_x: bool,
    /// `mirror_clip_y`.
    pub mirror_clip_y: bool,
    /// `mirror_clip_z`.
    pub mirror_clip_z: bool,
    /// `clip_dist`.
    pub clip_dist: f32,
    /// `use_axis_x`.
    pub use_axis_x: bool,
    /// `use_axis_y`.
    pub use_axis_y: bool,
    /// `use_axis_z`.
    pub use_axis_z: bool,
}

/// Output slots of `smooth_vert` (Blender's operator has none).
#[derive(Clone, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct SmoothVertOut {
    /// Always empty (`bmesh.ops.smooth_vert` has no output slots).
    pub geom: Vec<BMElem>,
}

/// `BMO_OPTYPE_FLAG_*` of `bmo_smooth_vert_def`.
pub const TYPE_FLAG: OpTypeFlag = OpTypeFlag::NORMALS_CALC;

/// Runs `smooth_vert` on `bm`.
pub fn smooth_vert(bm: &mut BMesh, i: &SmoothVertIn) -> Result<SmoothVertOut, BmError> {
    if let Some(dead) = i.verts.iter().find(|v| !bm.verts.contains(v.0)) {
        return Err(BmError::Invalid(format!(
            "smooth_vert: dead vertex {dead:?} in verts"
        )));
    }
    bm.bmo_exec(TYPE_FLAG, |bm| {
        let mut cos: Vec<[f32; 3]> = Vec::with_capacity(i.verts.len());
        for &v in &i.verts {
            frap_math::budget::check();
            let vco = bm.vert(v).co;
            let mut co = [0.0f32; 3];
            let edges = bm.vert_edges(v);
            for &e in &edges {
                co = math::add(co, bm.vert(bm.edge_other_vert(e, v)).co);
            }
            if edges.is_empty() {
                cos.push(vco);
                continue;
            }
            co = math::scale(co, 1.0 / edges.len() as f32);
            co = math::interp(vco, co, i.factor);
            if i.mirror_clip_x && vco[0].abs() <= i.clip_dist {
                co[0] = 0.0;
            }
            if i.mirror_clip_y && vco[1].abs() <= i.clip_dist {
                co[1] = 0.0;
            }
            if i.mirror_clip_z && vco[2].abs() <= i.clip_dist {
                co[2] = 0.0;
            }
            cos.push(co);
        }
        for (&v, co) in i.verts.iter().zip(cos) {
            let c = &mut bm.vert_mut(v).co;
            if i.use_axis_x {
                c[0] = co[0];
            }
            if i.use_axis_y {
                c[1] = co[1];
            }
            if i.use_axis_z {
                c[2] = co[2];
            }
        }
    });
    Ok(SmoothVertOut::default())
}
