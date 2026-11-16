// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared operator helpers (ARCHITECTURE §10.3). M0: placement and add-primitive in object mode.
//! M1 W0 froze the signatures of the remaining §10.3 helpers; WP 1.12 owns their bodies.

use frap_mesh::Mesh;
use frap_rna::{IdAny, IdRef, MeshId, ObjectId, Prop};

use crate::ctx::OpCtx;
pub use crate::ctx::Placement;
use crate::def::OpError;

/// `align` values of the add operators.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Align {
    /// WORLD
    World,
    /// VIEW
    View,
    /// CURSOR
    Cursor,
}

/// Is-set semantics of the add operators [OBJ §1.3]: unset `location` → 3D cursor; unset
/// `rotation` → from `align`; `scale` (0,0,0) → 1.
pub fn placement(
    cx: &OpCtx,
    location: &Prop<[f32; 3]>,
    rotation: &Prop<[f32; 3]>,
    align: Align,
    scale: &Prop<[f32; 3]>,
) -> Result<Placement, OpError> {
    let location = if location.is_set {
        location.value
    } else {
        cx.ctx().cursor().location
    };
    let rotation = if rotation.is_set {
        rotation.value
    } else {
        match align {
            Align::World => [0.0; 3],
            Align::View | Align::Cursor => {
                return Err(OpError::NotImplemented(
                    "frap M0: align='VIEW' and align='CURSOR' arrive in M1 (virtual 3D view, cursor rotation)".into(),
                ));
            }
        }
    };
    let scale = if scale.value == [0.0; 3] {
        [1.0; 3]
    } else {
        scale.value
    };
    Ok(Placement {
        location,
        rotation,
        scale,
    })
}

/// Object mode: creates the mesh and the object (both requested as `name`, uniquified
/// independently), links the object to `context.collection`, selects only it and makes it
/// active. Edit mode (merge into the edit mesh) arrives in M2.
pub fn add_primitive_or_merge(
    cx: &mut OpCtx,
    name: &str,
    mut mesh: Mesh,
    at: Placement,
    enter_editmode: bool,
) -> Result<IdRef<ObjectId>, OpError> {
    if enter_editmode {
        return Err(OpError::NotImplemented(
            "frap M0: enter_editmode=True needs Edit Mode (M2)".into(),
        ));
    }
    let (main, log) = cx.main_mut();
    mesh.header.name = name.into();
    let me = main.meshes.insert(name, mesh, log);
    let ob = cx.add_object(name, Some(me.any()), at);
    // Order the log as Blender's report would list it: object before its data.
    let (_, log) = cx.main_mut();
    if let (Some(a), Some(b)) = (
        log.added.iter().position(|x| *x == me.any()),
        log.added.iter().position(|x| *x == ob.any()),
    ) {
        log.added.swap(a, b);
    }
    Ok(ob)
}

/// `add_primitive_or_merge`'s rule for non-mesh data (cameras, lights, empties, armatures):
/// the object is requested as `name` and uniquified in `objects`; `data` was inserted by the
/// caller under its own requested name, uniquified independently (camera object `Camera.001`
/// may use data `Camera`). Links to `context.collection`, deselects others, selects and
/// activates it (context view layer). `data: None` adds an Empty. `enter_editmode` (armatures)
/// arrives with the M6 edit-armature mode handler.
pub fn add_object_with_data(
    cx: &mut OpCtx,
    name: &str,
    data: Option<IdAny>,
    at: Placement,
    enter_editmode: bool,
) -> Result<IdRef<ObjectId>, OpError> {
    if enter_editmode {
        return Err(OpError::NotImplemented(
            "frap M1: enter_editmode=True needs an Edit Mode handler (M2 meshes, M6 armatures)"
                .into(),
        ));
    }
    let ob = cx.add_object(name, data, at);
    // Order the log as Blender's report would list it: object before its data.
    if let Some(d) = data {
        let (_, log) = cx.main_mut();
        if let (Some(a), Some(b)) = (
            log.added.iter().position(|x| *x == d),
            log.added.iter().position(|x| *x == ob.any()),
        ) {
            log.added.swap(a, b);
        }
    }
    Ok(ob)
}

/// Frame range of animation operators: an unset `frame_start`/`frame_end` falls back to the
/// scene's `frame_start`/`frame_end` (is-set semantics, §10.3). Returns `(start, end)`.
pub fn frame_range_or_scene(
    cx: &OpCtx,
    frame_start: &Prop<i32>,
    frame_end: &Prop<i32>,
) -> (i32, i32) {
    let _ = (cx, frame_start, frame_end);
    todo!("WP 1.12 (scene frame range from WP 1.10's Scene bindings; used from M6)")
}

/// Multi-object edit loop: every object in Edit Mode with unique mesh data
/// (`context.objects_in_mode_unique_data`), with its mesh, in Blender's order. Callers take
/// `OpCtx::bmesh_mut(mesh)` per entry.
pub fn iter_edit_meshes(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    let _ = cx;
    todo!("WP 1.12 (M2: needs Edit Mode)")
}
