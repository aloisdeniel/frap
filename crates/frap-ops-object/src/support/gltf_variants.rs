// SPDX-License-Identifier: GPL-3.0-or-later
//! Shared parts of the glTF add-on's variant operators (`ops/scene__gltf2_*.rs`, M8 W0,
//! decision log M8W0-6): the registration gate (the add-on registers them in
//! `variant_register` while `KHR_materials_variants_ui` is on, so otherwise they are unknown,
//! as in Blender 5.2.4), the add-on's polls (`poll()` answers `False` with Blender's default
//! message) and the error mapping of `frap_scene::gltf_variants`.

use frap_op::{OpError, OpGate, PollFail};
use frap_rna::{IdRef, ObjectId};
use frap_scene::Context;
use frap_scene::gltf_variants::{VariantOpError, first_scene_variants, material_slot_count, ui_on};

/// How to register the operators (`error.hint` while they are unknown).
pub const WHEN: &str = "the glTF add-on registers its variant operators while \
     bpy.context.preferences.addons['io_scene_gltf2'].preferences.KHR_materials_variants_ui = True";

fn registered(c: &Context) -> bool {
    ui_on(c.prefs)
}

/// The [`OpGate`] of one variant operator.
pub const fn gate(idname: &'static str) -> OpGate {
    OpGate {
        idname,
        registered,
        when: WHEN,
    }
}

/// A failed add-on poll: Blender's default message (the add-on's polls return a bare bool).
fn fail(hint: &str) -> PollFail {
    PollFail {
        message: None,
        unmet: Default::default(),
        hint: Some(hint.to_owned()),
    }
}

/// `len(bpy.data.scenes[0].gltf2_KHR_materials_variants_variants) > 0`
pub fn has_variants(c: &Context) -> Result<(), PollFail> {
    if first_scene_variants(c.main).is_empty() {
        Err(fail(
            "add a variant first: bpy.ops.scene.gltf2_variant_add() (the add-on reads \
             bpy.data.scenes[0])",
        ))
    } else {
        Ok(())
    }
}

/// `bpy.context.object` (the active object, hidden or not).
pub fn object(c: &Context) -> Result<IdRef<ObjectId>, PollFail> {
    c.object().ok_or_else(|| {
        fail("make an object active: bpy.context.view_layer.objects.active = bpy.data.objects['Cube']")
    })
}

/// `bpy.context.object and bpy.context.object.type == "MESH"`
pub fn mesh_object(c: &Context) -> Result<IdRef<ObjectId>, PollFail> {
    let ob = object(c)?;
    let is_mesh = c
        .main
        .objects
        .get(ob)
        .and_then(|o| o.data)
        .and_then(|d| d.typed::<frap_rna::MeshId>())
        .is_some();
    if is_mesh {
        Ok(ob)
    } else {
        Err(fail("make a mesh object active"))
    }
}

/// `bpy.context.object and len(bpy.context.object.material_slots) > 0`
pub fn object_with_slots(c: &Context) -> Result<IdRef<ObjectId>, PollFail> {
    let ob = object(c)?;
    if material_slot_count(c.main, ob) > 0 {
        Ok(ob)
    } else {
        Err(fail(
            "the active object needs a material slot: bpy.ops.object.material_slot_add()",
        ))
    }
}

/// An error the add-on's `execute` raises (Blender reports the Python exception as the
/// operator's `RuntimeError`).
pub fn exec_error(e: VariantOpError) -> OpError {
    OpError::Runtime(format!("Python: the glTF add-on's operator raised {e}"))
}
