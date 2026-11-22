// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.point_normals(mode='COORDINATES', invert=False, align=False,
//! target_location=(0.0, 0.0, 0.0), spherize=False, spherize_strength=0.1)`, tier T1
//! (`tests/golden/cases/edit/normals_m8/point_normals*.py`). Port of `edbm_point_normals_exec`,
//! `point_normals_init`, `point_normals_apply` and `bmesh_selected_verts_center_calc`
//! (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! Headless-extended: Blender's interactive invoke is a modal tool following the mouse, the 3D
//! cursor or a picked element; frap (like Blender's `exec`) always points the normals at
//! `target_location` and ignores `mode` (`MOUSE` behaves as `COORDINATES`).
//!
//! Only the **edit object** (the active object) is changed. Its custom normals to edit are those
//! of the selection (`BM_loop_normal_editdata_array_init`: by select mode and selection history,
//! whole smooth fans); without any the operator is `CANCELLED`. The target is moved into local
//! space by subtracting the object's `location` only (rotation and scale are ignored, as in
//! Blender). Each normal becomes `target - vertex` (with `align`: `target - centre of the
//! selected vertices`, the same for all); `spherize` instead mixes the original normal with the
//! normalised direction (`(1 - spherize_strength) * old + spherize_strength * dir`); `invert`
//! negates the result. Normals shorter than 1e-4 after that are left unchanged. The
//! `custom_normal` layer is created when missing.

use frap_bmesh::{BMesh, ElemFlag};
use frap_geom::normals::{V3, normalize_v3, scale, sub};
use frap_op::operator;
use frap_op::prelude::*;
use frap_rna::MeshId;
use frap_spec_ops_mesh::mesh;

use super::flip_normals::clnor;

/// The settings of one `point_normals_apply`.
#[derive(Copy, Clone, Debug)]
pub(crate) struct PointTo {
    /// The target in the object's local space (`target - obedit->loc`).
    pub target: V3,
    /// `invert`.
    pub invert: bool,
    /// `align`.
    pub align: bool,
    /// `spherize`.
    pub spherize: bool,
    /// `spherize_strength`.
    pub strength: f32,
}

// Ported from blender/source/blender/editors/mesh/editmesh_tools.cc
// (bmesh_selected_verts_center_calc).
/// The mean of the selected vertices (`mul_v3_fl(center, 1.0f / i)`).
fn selected_verts_center(bm: &BMesh) -> V3 {
    let mut c = [0.0f32; 3];
    let mut i = 0usize;
    for v in bm.vert_ids() {
        let vv = bm.vert(v);
        if vv.flag.contains(ElemFlag::SELECT) {
            c[0] += vv.co[0];
            c[1] += vv.co[1];
            c[2] += vv.co[2];
            i += 1;
        }
    }
    scale(c, 1.0 / i as f32)
}

/// `point_normals_init` + `point_normals_apply`: `false` (CANCELLED) when no custom normal is
/// selected.
pub(crate) fn point_normals(bm: &mut BMesh, p: PointTo) -> bool {
    let sp = clnor::update(bm);
    let mut arr = clnor::editdata_array(bm, &sp, false, clnor::Mark::Indiv);
    if arr.data.is_empty() {
        return false;
    }
    let center = if p.align {
        selected_verts_center(bm)
    } else {
        [0.0; 3]
    };
    let target = p.target;
    for d in &mut arr.data {
        if p.spherize {
            let (dir, _) = normalize_v3(sub(target, d.loc));
            let dir = scale(dir, p.strength);
            let old = scale(d.niloc, 1.0 - p.strength);
            d.nloc = [old[0] + dir[0], old[1] + dir[1], old[2] + dir[2]];
        } else if p.align {
            d.nloc = sub(target, center);
        } else {
            d.nloc = sub(target, d.loc);
        }
        if p.invert {
            d.nloc = [-d.nloc[0], -d.nloc[1], -d.nloc[2]];
        }
        let (n, len) = normalize_v3(d.nloc);
        d.nloc = n;
        if len >= clnor::CLNORS_VALID_VEC_LEN {
            let v = sp.encode(d.loop_, n);
            clnor::set(bm, d.loop_, v);
        }
    }
    true
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::PointNormals) -> OpResult {
    let edit = {
        let ctx = cx.ctx();
        poll::edit_object(&ctx).and_then(|(_, ob)| {
            let me = ob.data.and_then(|d| d.typed::<MeshId>())?;
            Some((me, ob.location))
        })
    };
    let Some((me, loc)) = edit else {
        return Ok(OpStatus::Cancelled);
    };
    let t = p.target_location.value;
    let settings = PointTo {
        target: sub(t, loc),
        invert: p.invert.value,
        align: p.align.value,
        spherize: p.spherize.value,
        strength: p.spherize_strength.value,
    };
    if !point_normals(cx.bmesh_mut(me)?, settings) {
        // Blender keeps the `custom_normal` layer its init added; frap rolls the cancelled
        // statement back (deviation D-151).
        return Ok(OpStatus::Cancelled);
    }
    Ok(OpStatus::Finished)
}
