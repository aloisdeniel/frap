// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.context` members (ARCHITECTURE §8, research §1.4). `Context` is computed, so its members
//! are answered here from `frap_scene::Context` and re-rooted at their canonical pointers
//! (F-TR-2: `bpy.context.view_layer` is `bpy.data.scenes['Scene'].view_layers["ViewLayer"]`).

use frap_rna::{EmbeddedSlot, FrapError, IdAny, PropDef, PyExc, RnaPtr, Root, Seg, Value};
use frap_scene::{ContextMode, HostRead};
use smol_str::SmolStr;

use super::Target;
use super::access::id_srna;
use crate::errors::*;

pub(crate) mod ui;

fn id_target(host: &dyn HostRead, id: Option<IdAny>) -> Result<Target, FrapError> {
    match id {
        Some(id) => Ok(Target::Struct(RnaPtr::id(id, id_srna(host, id)?))),
        None => Ok(Target::None),
    }
}

fn id_list(host: &dyn HostRead, ids: Vec<IdAny>) -> Result<Target, FrapError> {
    Ok(Target::List {
        what: "list",
        items: ids
            .into_iter()
            .map(|id| id_target(host, Some(id)))
            .collect::<Result<_, _>>()?,
    })
}

fn not_yet(member: &str) -> FrapError {
    FrapError::new(
        PyExc::NotImplementedError,
        NOT_IMPLEMENTED,
        format!("frap: bpy.context.{member} is not implemented yet"),
    )
}

/// Reads `bpy.context.<prop>`: a dereferenced target (IDs, structs, lists) or a plain value.
pub fn member(host: &dyn HostRead, prop: &'static PropDef) -> Result<Member, FrapError> {
    let ctx = host.ctx();
    let main = host.main();
    let scene = ctx.win.scene.any();
    let scene_ptr =
        || -> Result<RnaPtr, FrapError> { Ok(RnaPtr::id(scene, id_srna(host, scene)?)) };
    // `edit_object` is set in the edit modes only (not in Pose Mode).
    let in_edit = matches!(
        ctx.mode(),
        ContextMode::EditMesh | ContextMode::EditArmature
    );
    let active = ctx.active_object();
    let t = match prop.identifier {
        "scene" => id_target(host, Some(scene))?,
        "blend_data" => Target::Data,
        "view_layer" => Target::Struct(
            scene_ptr()?
                .child(Seg::Prop("view_layers"), &frap_spec_rna::VIEW_LAYER)
                .child(
                    Seg::Key(ctx.view_layer().name.clone()),
                    &frap_spec_rna::VIEW_LAYER,
                ),
        ),
        "tool_settings" => Target::Struct(
            scene_ptr()?.child(Seg::Prop("tool_settings"), &frap_spec_rna::TOOL_SETTINGS),
        ),
        "collection" => match ctx.collection() {
            Some(c) => id_target(host, Some(c.any()))?,
            None => Target::Struct(RnaPtr::new(
                Root::Embedded {
                    owner: scene,
                    slot: EmbeddedSlot::MasterCollection,
                },
                &frap_spec_rna::COLLECTION,
            )),
        },
        "active_object" | "object" => id_target(host, active.map(|o| o.any()))?,
        "edit_object" => id_target(host, active.filter(|_| in_edit).map(|o| o.any()))?,
        "selected_objects" => id_list(
            host,
            ctx.selected_objects().iter().map(|o| o.any()).collect(),
        )?,
        "selected_editable_objects" => id_list(
            host,
            ctx.selected_objects()
                .iter()
                .filter(|o| {
                    main.objects
                        .get(**o)
                        .is_some_and(|ob| ob.header.library.is_none())
                })
                .map(|o| o.any())
                .collect(),
        )?,
        "visible_objects" => id_list(
            host,
            ctx.visible_objects().iter().map(|o| o.any()).collect(),
        )?,
        "selectable_objects" => id_list(
            host,
            ctx.selectable_objects().iter().map(|o| o.any()).collect(),
        )?,
        "editable_objects" => id_list(
            host,
            ctx.editable_objects().iter().map(|o| o.any()).collect(),
        )?,
        // Multi-object modes (§6.3): the active object first, then every visible object of
        // its type in the same mode (`Context::objects_in_mode*`).
        "objects_in_mode" => id_list(
            host,
            ctx.objects_in_mode().iter().map(|o| o.any()).collect(),
        )?,
        "objects_in_mode_unique_data" => id_list(
            host,
            ctx.objects_in_mode_unique_data()
                .iter()
                .map(|o| o.any())
                .collect(),
        )?,
        "mode" => {
            return Ok(Member::Value(Value::Enum(SmolStr::new(
                ctx.mode().as_str(),
            ))));
        }
        "engine" => {
            return Ok(Member::Value(Value::Str(ctx.scene().render.engine.clone())));
        }
        // UI members of the virtual 3D View (§8: `window` stub, `space_data` stub with
        // `shading` and `region_3d`, read-only `preferences`), WP 2.3 (`context/ui.rs`).
        "window" | "space_data" | "preferences" | "area" | "region" | "region_data" | "screen" => {
            return ui::member(host, prop).map_err(|e| {
                if e.code == NOT_IMPLEMENTED {
                    not_yet(prop.identifier)
                } else {
                    e
                }
            });
        }
        // The animation members (WP 6.4): Blender answers them from an animation editor
        // (Dope Sheet, Graph Editor, NLA); in the 3D Viewport every frap statement runs in
        // they are `None` (verified on 5.2.4; `frap_scene::Context` documents the same).
        "active_action" | "active_editable_fcurve" | "active_nla_track" | "active_nla_strip" => {
            Target::None
        }
        // The bone members (WP 6.2): edit bones need the edit state, so frap-scene answers
        // them from the host.
        other => match frap_scene::edit_armature::ctx::bone_member(host, other) {
            Some(Value::Ptr(p)) => Target::Struct(*p),
            Some(Value::Id(id)) => id_target(host, id)?,
            Some(Value::Tuple(items)) => Target::List {
                what: "list",
                items: items
                    .into_iter()
                    .map(|v| match v {
                        Value::Ptr(p) => Target::Struct(*p),
                        _ => Target::None,
                    })
                    .collect(),
            },
            Some(_) => Target::None,
            None => return Err(not_yet(other)),
        },
    };
    Ok(Member::Target(t))
}

/// Members of the spec's `Context` that only exist in the Properties editor's buttons
/// context: inside the VIEW_3D override every statement runs in (D-10), Blender 5.2.4 raises
/// `'Context' object has no attribute 'mesh'` for each of them (verified, also in plain `-b`).
pub const ABSENT: &[&str] = &[
    "camera",
    "light",
    "material",
    "material_slot",
    "mesh",
    "world",
    "depsgraph",
];

/// A context member.
pub enum Member {
    /// A pointer, struct or list (re-rooted).
    Target(Target),
    /// A plain value (`mode`, `engine`).
    Value(Value),
}
