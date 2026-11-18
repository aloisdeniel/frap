// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared operator helpers (ARCHITECTURE §10.3): placement of new objects, adding objects with
//! Blender's naming and selection rules, the scene frame range fallback, the multi-object edit
//! loop and mode-handler lookup. Ports of `ED_object_add_generic_get_opts`,
//! `ED_object_rotation_from_view` and `ED_object_add_type` (`editors/object/object_add.cc`) and
//! of `BKE_view_layer_array_from_objects_in_mode_params` (`blenkernel/intern/layer.cc`),
//! Blender 5.2.4, GPL-2.0-or-later.

use frap_math::EulerOrder;
use frap_math::euler::{eul_o_to_mat3, mat3_normalized_to_eul_o, quat_to_eul_o};
use frap_math::glam::Mat3;
use frap_math::matrix::RotationMode;
use frap_math::quat::{axis_angle_to_mat3, mat3_normalized_to_quat, normalize_qt, quat_to_mat3};
use frap_mesh::Mesh;
use frap_rna::{IdAny, IdKey, IdRef, MeshId, ObjectId, Prop};
use frap_scene::generated::plain::View3DCursor;
use frap_scene::{ObjectMode, WindowState};

use crate::ctx::OpCtx;
pub use crate::ctx::Placement;

pub mod edit_merge;
pub mod edit_mode;
use crate::def::{OpError, OpStatus};
use crate::modes::{MODE_HANDLERS, ModeHandler};
use crate::poll;

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

impl Align {
    /// From Blender's identifier (`'WORLD'`, `'VIEW'`, `'CURSOR'`).
    pub fn from_identifier(s: &str) -> Option<Align> {
        match s {
            "WORLD" => Some(Align::World),
            "VIEW" => Some(Align::View),
            "CURSOR" => Some(Align::Cursor),
            _ => None,
        }
    }
}

/// `ED_object_rotation_from_view(C, rot, 'Z')` with frap's virtual 3D View (§8, D-10): the
/// view quaternion (`viewquat`, here recovered from the view matrix), `w` negated (the inverse
/// rotation), as XYZ Euler.
///
/// The view matrix must be the one `viewquat` produces (`view3d_viewmatrix_set`): Blender reads
/// `viewquat`, and RNA's `region_3d.view_rotation` is its inverse. With the factory Layout
/// viewport the result is `(1.1088, 0, 1.1549)` (verified). The `view_matrix` stored in the
/// factory file is stale in `-b` (never redrawn) and would give `(1.1087, 0.0133, 1.1483)`.
pub fn view_rotation(win: &WindowState) -> [f32; 3] {
    let m = Mat3::from_mat4(win.view3d.view_matrix);
    let mut q = mat3_normalized_to_quat(&m);
    q[0] = -q[0];
    quat_to_eul_o(q, EulerOrder::XYZ)
}

/// `BKE_scene_cursor_rot_to_mat3` then `mat3_normalized_to_eul`: the cursor rotation in any
/// `rotation_mode`, as XYZ Euler (`align='CURSOR'`).
pub fn cursor_rotation(cursor: &View3DCursor) -> [f32; 3] {
    let m = match RotationMode::from_blender(&cursor.rotation_mode) {
        Some(RotationMode::Euler(order)) => eul_o_to_mat3(cursor.rotation_euler, order),
        Some(RotationMode::AxisAngle) => {
            let a = cursor.rotation_axis_angle;
            axis_angle_to_mat3([a[1], a[2], a[3]], a[0])
        }
        Some(RotationMode::Quaternion) => quat_to_mat3(normalize_qt(cursor.rotation_quaternion).0),
        None => eul_o_to_mat3(cursor.rotation_euler, EulerOrder::XYZ),
    };
    mat3_normalized_to_eul_o(&m, EulerOrder::XYZ)
}

/// Is-set semantics of the add operators (`ED_object_add_generic_get_opts`, [OBJ §1.3]):
/// unset `location` → 3D cursor; set `rotation` always wins, otherwise from `align` (`WORLD`
/// the prop's default, `VIEW` the virtual view rotation, `CURSOR` the cursor rotation); unset
/// `scale` → `(1, 1, 1)`, a set one is used as is (verified: `scale=(2, 0, 1)` flattens Y and
/// an explicit `(0, 0, 0)` collapses the mesh). `align` is taken by value (decision log W0-25).
pub fn placement(
    cx: &OpCtx,
    location: &Prop<[f32; 3]>,
    rotation: &Prop<[f32; 3]>,
    align: Align,
    scale: &Prop<[f32; 3]>,
) -> Result<Placement, OpError> {
    let c = cx.ctx();
    let location = if location.is_set {
        location.value
    } else {
        c.cursor().location
    };
    let rotation = if rotation.is_set {
        rotation.value
    } else {
        match align {
            Align::World => rotation.value,
            Align::View => view_rotation(c.win),
            Align::Cursor => cursor_rotation(c.cursor()),
        }
    };
    let scale = if scale.is_set { scale.value } else { [1.0; 3] };
    Ok(Placement {
        location,
        rotation,
        scale,
    })
}

/// The handler for `mode` on objects of `object_type` (an exact type match first, then a
/// handler for any type), or `OP_NOT_IMPLEMENTED` naming the mode (§8).
pub fn mode_handler(mode: ObjectMode, object_type: &str) -> Result<&'static ModeHandler, OpError> {
    MODE_HANDLERS
        .iter()
        .find(|h| h.mode == mode && h.object_type == object_type)
        .or_else(|| {
            MODE_HANDLERS
                .iter()
                .find(|h| h.mode == mode && h.object_type.is_empty())
        })
        .ok_or_else(|| {
            OpError::NotImplemented(format!(
                "frap: mode '{}' is not implemented for {object_type} objects yet (no mode handler)",
                mode.as_str()
            ))
        })
}

fn handler_status(r: Result<OpStatus, OpError>, what: &str) -> Result<(), OpError> {
    match r? {
        OpStatus::Finished => Ok(()),
        OpStatus::Cancelled => Err(OpError::Runtime(format!("could not {what}"))),
    }
}

/// `ED_object_add_type` leaves Edit Mode before adding (`ED_object_editmode_exit_ex`).
fn exit_edit_mode(cx: &mut OpCtx) -> Result<(), OpError> {
    let (objects, ty) = {
        let c = cx.ctx();
        let Some((_, ob)) = poll::edit_object(&c) else {
            return Ok(());
        };
        let ty = ob.object_type();
        (
            iter_mode_objects(cx, ObjectMode::Edit)
                .into_iter()
                .map(|(o, _)| o)
                .collect::<Vec<_>>(),
            ty,
        )
    };
    let h = mode_handler(ObjectMode::Edit, ty)?;
    handler_status((h.exit)(cx, &objects), "leave Edit Mode")
}

/// `bpy.ops.object.mode_set(mode='OBJECT')` as an operator body calls it (the glTF exporter's
/// `if bpy.context.active_object.mode != 'OBJECT'`): when the active object is in another mode,
/// that mode's handler leaves it for the active object and every other object of its type in
/// the same mode (base order), which writes edit data back, then the Object Mode handler enters.
/// Returns whether the mode changed. Without an active object, or already in Object Mode,
/// nothing happens.
pub fn mode_set_object(cx: &mut OpCtx) -> Result<bool, OpError> {
    let (objects, ty, current) = {
        let c = cx.ctx();
        let Some((active, ob)) = poll::active(&c) else {
            return Ok(false);
        };
        let (ty, current) = (ob.object_type(), ob.mode);
        if current == ObjectMode::Object {
            return Ok(false);
        }
        let mut objects = vec![active];
        objects.extend(c.view_layer().bases.iter().map(|b| b.object).filter(|&r| {
            r != active
                && c.main
                    .objects
                    .get(r)
                    .is_some_and(|o| o.mode == current && o.object_type() == ty)
        }));
        (objects, ty, current)
    };
    let leave = mode_handler(current, ty)?;
    let enter = mode_handler(ObjectMode::Object, ty)?;
    handler_status((leave.exit)(cx, &objects), "leave the current mode")?;
    handler_status((enter.enter)(cx, &objects), "enter Object Mode")?;
    Ok(true)
}

fn enter_edit_mode(cx: &mut OpCtx, ob: IdRef<ObjectId>) -> Result<(), OpError> {
    let ty = cx
        .main()
        .objects
        .get(ob)
        .map(|o| o.object_type())
        .unwrap_or("");
    let h = mode_handler(ObjectMode::Edit, ty)?;
    handler_status((h.enter)(cx, &[ob]), "enter Edit Mode")
}

/// Moves the object's entry before its data's in the change log, as Blender's report lists
/// them (object first).
fn order_log(cx: &mut OpCtx, ob: IdAny, data: IdAny) {
    let (_, log) = cx.main_mut();
    if let (Some(a), Some(b)) = (
        log.added.iter().position(|x| *x == data),
        log.added.iter().position(|x| *x == ob),
    ) && a < b
    {
        log.added.swap(a, b);
    }
}

/// `make_prim_init` + `make_prim_finish`. In Object Mode creates the mesh and the object, both
/// requested as `name` and **uniquified independently** in `meshes` and `objects` (after
/// deleting the factory Cube: object `Cube`, mesh `Cube.001`, verified), links the object to
/// `context.collection`, deselects the others, selects and activates it, then enters Edit Mode
/// if `enter_editmode` (through the `EDIT` mode handler). With a mesh in Edit Mode Blender
/// merges the new geometry into it instead ([`edit_merge::merge_into_edit_mesh`]). Another
/// object type in Edit Mode leaves it first. The caller bakes `scale` and the
/// creation matrix into `mesh` (`frap_mesh::primitives::creation_matrix`); object scale stays 1.
pub fn add_primitive_or_merge(
    cx: &mut OpCtx,
    name: &str,
    mut mesh: Mesh,
    at: Placement,
    enter_editmode: bool,
) -> Result<IdRef<ObjectId>, OpError> {
    if let Some((edit_ob, ob)) = poll::edit_object(&cx.ctx())
        && ob.object_type() == "MESH"
    {
        // Blender merges the new geometry into the edit mesh (WP 2.2, `edit_merge.rs`).
        return edit_merge::merge_into_edit_mesh(cx, edit_ob, mesh, at);
    }
    exit_edit_mode(cx)?;
    let (main, log) = cx.main_mut();
    mesh.header.name = name.into();
    let me = main.meshes.insert(name, mesh, log);
    let ob = cx.add_object(name, Some(me.any()), at);
    order_log(cx, ob.any(), me.any());
    if enter_editmode {
        enter_edit_mode(cx, ob)?;
    }
    Ok(ob)
}

/// `add_primitive_or_merge`'s rule for non-mesh data (cameras, lights, empties, armatures):
/// the object is requested as `name` and uniquified in `objects`; `data` was inserted by the
/// caller under its own requested name, uniquified independently (camera object `Camera.001`
/// may use data `Camera`). Leaves Edit Mode first, links to `context.collection`, deselects
/// others, selects and activates it (context view layer). `data: None` adds an Empty.
/// `enter_editmode` enters Edit Mode through the mode handler of the object's type
/// (`OP_NOT_IMPLEMENTED` for armatures until M6).
pub fn add_object_with_data(
    cx: &mut OpCtx,
    name: &str,
    data: Option<IdAny>,
    at: Placement,
    enter_editmode: bool,
) -> Result<IdRef<ObjectId>, OpError> {
    exit_edit_mode(cx)?;
    let ob = cx.add_object(name, data, at);
    if let Some(d) = data {
        order_log(cx, ob.any(), d);
    }
    if enter_editmode {
        enter_edit_mode(cx, ob)?;
    }
    Ok(ob)
}

/// Frame range of animation operators: an unset `frame_start`/`frame_end` falls back to the
/// context scene's `frame_start`/`frame_end` (is-set semantics, §10.3). Returns `(start, end)`.
pub fn frame_range_or_scene(
    cx: &OpCtx,
    frame_start: &Prop<i32>,
    frame_end: &Prop<i32>,
) -> (i32, i32) {
    let scene = cx.ctx().scene();
    (
        if frame_start.is_set {
            frame_start.value
        } else {
            scene.frame_start
        },
        if frame_end.is_set {
            frame_end.value
        } else {
            scene.frame_end
        },
    )
}

/// Objects in `mode` with the active object's type, active first, then the other bases of the
/// context view layer in order, visible only, each object data once
/// (`BKE_view_layer_array_from_objects_in_mode_params` with `no_dup_data`). Empty without an
/// active object; an active object not in `mode` is skipped but still sets the type.
fn iter_mode_objects(cx: &OpCtx, mode: ObjectMode) -> Vec<(IdRef<ObjectId>, Option<IdAny>)> {
    let c = cx.ctx();
    let Some((active, act)) = poll::active(&c) else {
        return Vec::new();
    };
    let ty = act.object_type();
    let vl = c.view_layer();
    let in_mode = |r: IdRef<ObjectId>| -> Option<Option<IdAny>> {
        let base = vl.bases.iter().find(|b| b.object == r)?;
        let ob = c.main.objects.get(r)?;
        let visible = !base.flag.contains(frap_scene::BaseFlag::HIDDEN) && !ob.hide_viewport;
        (ob.mode == mode && ob.object_type() == ty && visible).then_some(ob.data)
    };
    let order =
        std::iter::once(active).chain(vl.bases.iter().map(|b| b.object).filter(|r| *r != active));
    let mut seen: Vec<IdKey> = Vec::new();
    let mut out = Vec::new();
    for r in order {
        let Some(data) = in_mode(r) else { continue };
        if let Some(d) = data {
            if seen.contains(&d.key) {
                continue;
            }
            seen.push(d.key);
        }
        out.push((r, data));
    }
    out
}

/// Multi-object edit loop: every mesh object in Edit Mode with unique mesh data
/// (`context.objects_in_mode_unique_data`), active first, then base order. Callers take
/// `OpCtx::bmesh_mut(mesh)` per entry.
pub fn iter_edit_meshes(cx: &OpCtx) -> Vec<(IdRef<ObjectId>, IdRef<MeshId>)> {
    iter_mode_objects(cx, ObjectMode::Edit)
        .into_iter()
        .filter_map(|(ob, data)| Some((ob, data?.typed::<MeshId>()?)))
        .collect()
}
