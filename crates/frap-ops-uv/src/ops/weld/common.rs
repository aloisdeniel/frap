// SPDX-License-Identifier: GPL-3.0-or-later
//! What WP 8.3.9's UV editing operators share: the "Geometric Utilities" of `uvedit_ops.cc`
//! (`ED_uvedit_foreach_uv`, `ED_uvedit_minmax_multi`, `uvedit_median_multi`,
//! `ED_uvedit_center_multi`), the edit loop over the meshes in Edit Mode with a UV map, and
//! frap's virtual Image Editor (headless-extended: Blender's `SpaceImage`, which `-b` lacks).
//!
//! Ported from blender/source/blender/editors/uvedit/uvedit_ops.cc, Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! - **Selection.** Every test goes through the UV selection API of WP 8.3.8
//!   ([`crate::select::UvSel`]): `uvedit_face_visible_test`, `uvedit_uv_select_test`, with
//!   `uv_select_sync_valid` false at the start of each operator (deviation D-111).
//! - **Virtual Image Editor.** frap behaves as if an Image Editor in UV mode without an image
//!   existed (as WP 8.3.8's Image Editor operators do): `ED_space_image_get_size` is
//!   `IMG_SIZE_FALLBACK` (256 × 256), the pivot (`sima->around`) is the factory
//!   `BOUNDING_BOX_CENTER`, `custom_grid_subdivisions` is (10, 10) and the 2D cursor
//!   (`sima->cursor`) is the scene's `ToolSettings::uv_cursor`, which only `uv.snap_cursor`
//!   writes (`uv.cursor_set` is OUT).

use frap_bmesh::LoopId;
use frap_op::OpCtx;
use frap_op::prelude::*;
use frap_rna::{IdRef, MeshId, ObjectId};
use smol_str::SmolStr;

use crate::select::{Ts, Uv, UvSel, active_uv, op};

/// `ED_space_image_get_size` of the virtual Image Editor (no image: `IMG_SIZE_FALLBACK`).
pub(crate) const IMAGE_SIZE: [i32; 2] = [256, 256];
/// `SpaceUVEditor.custom_grid_subdivisions` of a new Image Editor (`sima->custom_grid_subdiv`).
pub(crate) const CUSTOM_GRID_SUBDIV: [i32; 2] = [10, 10];
/// `SpaceUVEditor.tile_grid_shape` of a new Image Editor.
pub(crate) const TILE_GRID_SHAPE: [i32; 2] = [1, 1];

/// The `--describe` note of the operators that need an Image Editor in Blender.
pub(crate) const IMAGE_EDITOR_NOTE: &str = "headless-extended: runs as in an Image Editor in UV \
     mode without an image (Blender's poll needs one; blender -b has none): image size 256 x 256, \
     pivot BOUNDING_BOX_CENTER, the 2D cursor kept per scene and moved only by uv.snap_cursor \
     (starts at the origin)";

/// `sima->around`: `V3D_AROUND_CENTER_BOUNDS` or the median (`ED_uvedit_center_multi`'s
/// `mode`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum Around {
    /// `V3D_AROUND_CENTER_BOUNDS`
    Bounds,
    /// Any other pivot: the median of the selected UVs.
    Median,
}

/// The virtual Image Editor's pivot (`BOUNDING_BOX_CENTER`, the factory `SpaceImage` value).
pub(crate) const SIMA_AROUND: Around = Around::Bounds;

/// The virtual Image Editor's 2D cursor (`sima->cursor`).
pub(crate) fn cursor(cx: &OpCtx) -> Uv {
    cx.ctx().tool_settings().uv_cursor
}

/// Moves the virtual Image Editor's 2D cursor.
pub(crate) fn set_cursor(cx: &mut OpCtx, co: Uv) {
    op::set_tool_settings(cx, |ts| ts.uv_cursor = co);
}

/// `BKE_view_layer_array_from_objects_in_edit_mode_unique_data_with_uvs`.
pub(crate) fn objects(cx: &mut OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    op::objects(cx)
}

/// `bm->totvertsel == 0`.
pub(crate) fn no_vert_selected(cx: &mut OpCtx, me: IdRef<MeshId>) -> bool {
    op::edit_bm(cx, me).is_none_or(|bm| bm.selected_counts().0 == 0)
}

/// Runs `f` on the UV selection of `me`'s edit BMesh and writes the UV column back when `f`
/// reports a change (`None` without a UV map).
pub(crate) fn edit<R>(
    cx: &mut OpCtx,
    me: IdRef<MeshId>,
    ts: Ts,
    f: impl FnOnce(&mut UvSel) -> (R, bool),
) -> Result<Option<R>, OpError> {
    let Some(name) = uv_map_name(cx, me) else {
        return Ok(None);
    };
    let bm = cx.bmesh_mut(me)?;
    let Some(mut s) = UvSel::new(ts, bm) else {
        return Ok(None);
    };
    let (r, changed) = f(&mut s);
    if changed {
        write_back(&mut s, &name);
    }
    Ok(Some(r))
}

/// The name of `me`'s active UV map.
pub(crate) fn uv_map_name(cx: &mut OpCtx, me: IdRef<MeshId>) -> Option<SmolStr> {
    let bm = op::edit_bm(cx, me)?;
    let i = active_uv(&bm)?;
    Some(bm.ldata.layers[i].name.clone())
}

/// Writes the UV column of `s` to the layer named `name`.
pub(crate) fn write_back(s: &mut UvSel, name: &str) {
    let uv = std::mem::take(&mut s.uv);
    super::super::unwrap::uvedit::write_uvs(s.bm, name, uv.clone());
    s.uv = uv;
}

/// The pin flags of the active UV map by loop slot (`BM_ELEM_CD_GET_BOOL(l, offsets.pin)`;
/// all false without the `.pn.<uv map>` layer).
pub(crate) fn pin_flags(s: &UvSel) -> Vec<bool> {
    match active_uv(s.bm) {
        Some(i) => super::super::unwrap::uvedit::read_pins(s.bm, i),
        None => vec![false; s.bm.loops.capacity()],
    }
}

/// The loops `ED_uvedit_foreach_uv` visits, in its order: the corners of the (visible) faces
/// whose `uvedit_uv_select_test` is `selected`.
pub(crate) fn foreach_loops(s: &UvSel, skip_invisible: bool, selected: bool) -> Vec<LoopId> {
    let mut out = Vec::new();
    // Check selection for quick return.
    if s.ts.sync {
        let totvertsel = s.bm.selected_counts().0;
        let target = if selected { 0 } else { s.bm.verts_num() };
        if totvertsel == target {
            return out;
        }
    }
    for f in s.faces() {
        if skip_invisible && !s.face_visible(f) {
            continue;
        }
        for l in s.face_loops(f) {
            if s.uv_select_test(l) == selected {
                out.push(l);
            }
        }
    }
    out
}

/// `ED_uvedit_foreach_uv`: calls `f` on every visited UV.
pub(crate) fn foreach_uv(
    s: &mut UvSel,
    skip_invisible: bool,
    selected: bool,
    mut f: impl FnMut(&mut Uv),
) {
    for l in foreach_loops(s, skip_invisible, selected) {
        f(&mut s.uv[l.slot()]);
    }
}

/// `INIT_MINMAX2`.
pub(crate) fn init_minmax() -> (Uv, Uv) {
    ([f32::MAX, f32::MAX], [-f32::MAX, -f32::MAX])
}

/// `minmax_v2v2_v2`.
#[inline]
pub(crate) fn minmax(min: &mut Uv, max: &mut Uv, v: Uv) {
    if min[0] > v[0] {
        min[0] = v[0];
    }
    if min[1] > v[1] {
        min[1] = v[1];
    }
    if max[0] < v[0] {
        max[0] = v[0];
    }
    if max[1] < v[1] {
        max[1] = v[1];
    }
}

/// Calls `f` on the selected visible UVs of every object (`ED_uvedit_foreach_uv_multi` with
/// `skip_invisible` and selected UVs), read-only.
pub(crate) fn foreach_uv_multi(
    cx: &mut OpCtx,
    objects: &[(IdRef<ObjectId>, IdRef<MeshId>)],
    ts: Ts,
    selected: bool,
    mut f: impl FnMut(Uv),
) {
    for &(_, me) in objects {
        op::read(cx, me, ts, |s| {
            for l in foreach_loops(s, true, selected) {
                f(s.uv(l));
            }
        });
    }
}

/// `ED_uvedit_minmax_multi`: the bounds of the selected visible UVs (`None` without any).
pub(crate) fn minmax_multi(
    cx: &mut OpCtx,
    objects: &[(IdRef<ObjectId>, IdRef<MeshId>)],
    ts: Ts,
) -> (Uv, Uv, bool) {
    let (mut min, mut max) = init_minmax();
    let mut changed = false;
    foreach_uv_multi(cx, objects, ts, true, |uv| {
        minmax(&mut min, &mut max, uv);
        changed = true;
    });
    (min, max, changed)
}

/// `uvedit_median_multi`.
fn median_multi(
    cx: &mut OpCtx,
    objects: &[(IdRef<ObjectId>, IdRef<MeshId>)],
    ts: Ts,
) -> (Uv, bool) {
    let mut sel = 0u32;
    let mut co = [0.0f32; 2];
    foreach_uv_multi(cx, objects, ts, true, |uv| {
        co[0] += uv[0];
        co[1] += uv[1];
        sel += 1;
    });
    let fac = 1.0 / sel as f32;
    co[0] *= fac;
    co[1] *= fac;
    (co, sel != 0)
}

/// `ED_uvedit_center_multi`: the bounds centre or the median of the selected visible UVs
/// (`None` without any).
pub(crate) fn center_multi(
    cx: &mut OpCtx,
    objects: &[(IdRef<ObjectId>, IdRef<MeshId>)],
    ts: Ts,
    mode: Around,
) -> Option<Uv> {
    match mode {
        Around::Bounds => {
            let (min, max, changed) = minmax_multi(cx, objects, ts);
            changed.then(|| mid(min, max))
        }
        Around::Median => {
            let (co, changed) = median_multi(cx, objects, ts);
            changed.then_some(co)
        }
    }
}

/// `mid_v2_v2v2`.
#[inline]
pub(crate) fn mid(a: Uv, b: Uv) -> Uv {
    [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])]
}

/// `normalize_v2` (in place), returning the length.
#[inline]
pub(crate) fn normalize(v: &mut Uv) -> f32 {
    let (n, d) = frap_uv::param::bli::normalize2(*v);
    *v = n;
    d
}

/// `dot_v2v2`.
#[inline]
pub(crate) fn dot(a: Uv, b: Uv) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// `cross_v2v2`.
#[inline]
pub(crate) fn cross(a: Uv, b: Uv) -> f32 {
    a[0] * b[1] - a[1] * b[0]
}

/// `sub_v2_v2v2`.
#[inline]
pub(crate) fn sub(a: Uv, b: Uv) -> Uv {
    [a[0] - b[0], a[1] - b[1]]
}

/// `len_squared_v2v2`.
#[inline]
pub(crate) fn len_squared(a: Uv, b: Uv) -> f32 {
    let d = sub(a, b);
    d[0] * d[0] + d[1] * d[1]
}

/// `angle_v2v2`.
pub(crate) fn angle(a: Uv, b: Uv) -> f32 {
    let (v1, _) = frap_uv::param::bli::normalize2(a);
    let (v2, _) = frap_uv::param::bli::normalize2(b);
    frap_uv::param::bli::angle_normalized_v2v2(v1, v2)
}

/// `BM_mesh_elem_index_ensure(bm, BM_VERT)` as a table by vertex slot.
pub(crate) fn vert_index(s: &UvSel) -> Vec<u32> {
    let mut idx = vec![u32::MAX; s.bm.verts.capacity()];
    for (i, v) in s.bm.vert_ids().into_iter().enumerate() {
        idx[v.slot()] = i as u32;
    }
    idx
}

/// `BM_mesh_elem_index_ensure(bm, BM_FACE)` as a table by face slot.
pub(crate) fn face_index(s: &UvSel) -> Vec<u32> {
    let mut idx = vec![u32::MAX; s.bm.faces.capacity()];
    for (i, f) in s.bm.face_ids().into_iter().enumerate() {
        idx[f.slot()] = i as u32;
    }
    idx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minmax_and_mid_follow_blender() {
        let (mut min, mut max) = init_minmax();
        minmax(&mut min, &mut max, [0.25, 0.5]);
        minmax(&mut min, &mut max, [0.75, -0.5]);
        assert_eq!((min, max), ([0.25, -0.5], [0.75, 0.5]));
        assert_eq!(mid(min, max), [0.5, 0.0]);
        // Without any UV the bounds centre is the midpoint of the INIT_MINMAX2 sentinels.
        let (min, max) = init_minmax();
        assert_eq!(mid(min, max), [0.0, 0.0]);
    }

    #[test]
    fn angle_and_normalize() {
        let mut v = [3.0, 4.0];
        assert_eq!(normalize(&mut v), 5.0);
        assert!((v[0] - 0.6).abs() < 1e-7 && (v[1] - 0.8).abs() < 1e-7);
        let mut z = [0.0, 0.0];
        assert_eq!(normalize(&mut z), 0.0);
        assert!((angle([1.0, 0.0], [0.0, 2.0]) - core::f32::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((angle([1.0, 0.0], [-1.0, 0.0]) - core::f32::consts::PI).abs() < 1e-6);
        assert_eq!(cross([1.0, 0.0], [0.0, 1.0]), 1.0);
    }
}
