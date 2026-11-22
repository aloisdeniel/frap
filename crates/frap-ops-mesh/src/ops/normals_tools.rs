// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.ops.mesh.normals_tools(mode='COPY', absolute=False)`, tier T1
//! (`tests/golden/cases/edit/normals_m8/normals_tools*.py`). Port of `edbm_normals_tools_exec`
//! (`editors/mesh/editmesh_tools.cc`), Blender 5.2.4, GPL-2.0-or-later.
//!
//! The copy buffer is the scene's `tool_settings.normal_vector` (an RNA property: readable and
//! writable). On every mesh in Edit Mode (unique data) with corners, the custom normals of the
//! selection (by select mode and selection history, whole smooth fans) are edited; the
//! `custom_normal` layer is created when missing, whatever the mode:
//!
//! - `COPY` (meshes without a selected vertex or face are skipped): copies one normal into the
//!   buffer, in this priority: the only selected custom normal, the normal of the only selected
//!   face, the custom normals of the only selected vertex when they are all equal (within 1e-4;
//!   otherwise nothing is copied). Anything else (and any mesh after the first that copied)
//!   reports the error `Can only copy one custom normal, vertex normal or face normal`.
//! - `PASTE`: sets the normals to the buffer. The buffer is normalised in place first; when it
//!   is shorter than 1e-4 nothing is pasted. With `absolute`, each normal points from its vertex
//!   to the buffer taken as a position (`normalize(buffer - vertex)`, the original normal when
//!   that is shorter than 1e-4), and the buffer is not normalised.
//! - `ADD` / `MULTIPLY`: adds the buffer to / multiplies it component-wise with each normal and
//!   normalises (the original normal when the result is shorter than 1e-4).
//! - `RESET`: zeroes the buffer and resets the normals to the automatic ones.
//!
//! The operator always finishes (an error report raises `RuntimeError` in `bpy.ops`).

use frap_bmesh::{BMesh, ElemFlag};
use frap_geom::normals::{V3, normalize_v3};
use frap_op::operator;
use frap_op::prelude::*;
use frap_spec_ops_mesh::mesh::{self, normals_tools::Mode};

use super::flip_normals::{clnor, edit_meshes_where, sel};

/// The `COPY` error report.
const COPY_ERROR: &str = "Can only copy one custom normal, vertex normal or face normal";

/// `compare_v3v3(a, b, limit)`.
fn compare_v3v3(a: V3, b: V3, limit: f32) -> bool {
    (a[0] - b[0]).abs() <= limit && (a[1] - b[1]).abs() <= limit && (a[2] - b[2]).abs() <= limit
}

/// The outcome of `COPY` on one mesh.
enum Copy {
    /// Skipped (no selected vertex or face).
    Skip,
    /// The error report.
    Error,
    /// Copied this normal (or `None`: the vertex's normals differ, nothing copied).
    Done(Option<V3>),
}

fn copy(bm: &BMesh, arr: &clnor::EditArray, done_copy: bool) -> Copy {
    let (totvertsel, _, totfacesel) = sel(bm);
    if totfacesel == 0 && totvertsel == 0 {
        return Copy::Skip;
    }
    let totloop = arr.data.len();
    if done_copy || (totfacesel != 1 && totloop != 1 && totvertsel != 1) {
        return Copy::Error;
    }
    if totloop == 1 {
        return Copy::Done(Some(arr.data[0].nloc));
    }
    if totfacesel == 1 {
        // The last selected face (`BM_ITER_MESH` keeps copying).
        let no = bm
            .face_ids()
            .into_iter()
            .rev()
            .find(|&f| bm.face(f).flag.contains(ElemFlag::SELECT))
            .map(|f| bm.face(f).no);
        return Copy::Done(no);
    }
    if totloop != 0 {
        let first = arr.data[0].nloc;
        let same = arr.data.iter().all(|d| compare_v3v3(first, d.nloc, 1e-4));
        return Copy::Done(same.then_some(first));
    }
    Copy::Done(None)
}

/// Encodes `n` as the custom normal of `d`'s loop.
fn write(bm: &mut BMesh, sp: &clnor::Spaces, d: &clnor::EditData, n: V3) {
    let v = sp.encode(d.loop_, n);
    clnor::set(bm, d.loop_, v);
}

/// `PASTE`, `ADD`, `MULTIPLY` and `RESET` on one mesh; `buffer` is the tool settings' vector,
/// updated in place as Blender does.
fn edit(
    bm: &mut BMesh,
    sp: &clnor::Spaces,
    arr: &mut clnor::EditArray,
    mode: Mode,
    absolute: bool,
    buffer: &mut V3,
) {
    match mode {
        Mode::Copy => {}
        Mode::Paste => {
            if !absolute {
                let (n, len) = normalize_v3(*buffer);
                *buffer = n;
                if len < clnor::CLNORS_VALID_VEC_LEN {
                    return;
                }
            }
            for d in &arr.data {
                if absolute {
                    let abs = [
                        -d.loc[0] + buffer[0],
                        -d.loc[1] + buffer[1],
                        -d.loc[2] + buffer[2],
                    ];
                    let (mut n, len) = normalize_v3(abs);
                    if len < clnor::CLNORS_VALID_VEC_LEN {
                        n = d.niloc;
                    }
                    write(bm, sp, d, n);
                } else {
                    write(bm, sp, d, *buffer);
                }
            }
        }
        Mode::Multiply | Mode::Add => {
            for d in &mut arr.data {
                let b = *buffer;
                let m = if mode == Mode::Multiply {
                    [d.nloc[0] * b[0], d.nloc[1] * b[1], d.nloc[2] * b[2]]
                } else {
                    [d.nloc[0] + b[0], d.nloc[1] + b[1], d.nloc[2] + b[2]]
                };
                let (mut n, len) = normalize_v3(m);
                if len < clnor::CLNORS_VALID_VEC_LEN {
                    n = d.niloc;
                }
                d.nloc = n;
                let d = d.clone();
                write(bm, sp, &d, n);
            }
        }
        Mode::Reset => {
            *buffer = [0.0; 3];
            for d in &arr.data {
                write(bm, sp, d, [0.0; 3]);
            }
        }
    }
}

/// `scene.tool_settings.normal_vector`.
fn set_buffer(cx: &mut OpCtx, v: V3) {
    let scene = cx.ctx().win.scene;
    let (main, log) = cx.main_mut();
    if let Some(s) = main.scenes.get_mut(scene, log)
        && s.tool_settings.normal_vector != v
    {
        s.tool_settings.normal_vector = v;
    }
}

#[operator(poll = poll::EDITMESH, tier = T1)]
fn exec(cx: &mut OpCtx, p: mesh::NormalsTools) -> OpResult {
    let mode = p.mode.value;
    let absolute = p.absolute.value;
    let mut buffer = cx.ctx().tool_settings().normal_vector;
    let start = buffer;
    let mut done_copy = false;
    let mut errors = 0usize;
    for me in edit_meshes_where(cx, |bm| bm.loops_num() > 0) {
        let bm = cx.bmesh_mut(me)?;
        let sp = clnor::update(bm);
        let mut arr = clnor::editdata_array(bm, &sp, false, clnor::Mark::Indiv);
        if mode == Mode::Copy {
            match copy(bm, &arr, done_copy) {
                Copy::Skip => {}
                Copy::Error => errors += 1,
                Copy::Done(n) => {
                    if let Some(n) = n {
                        buffer = n;
                    }
                    done_copy = true;
                }
            }
            continue;
        }
        edit(bm, &sp, &mut arr, mode, absolute, &mut buffer);
    }
    if buffer != start {
        set_buffer(cx, buffer);
    }
    if errors > 0 {
        // Blender reports once per mesh and keeps what every mesh did (the `custom_normal`
        // layer it added, even on a single failing mesh); the error reports
        // make `bpy.ops` raise, and frap's statement is rolled back (deviation D-150).
        let msg = vec![COPY_ERROR; errors].join("\nError: ");
        return Err(OpError::Runtime(msg));
    }
    Ok(OpStatus::Finished)
}
