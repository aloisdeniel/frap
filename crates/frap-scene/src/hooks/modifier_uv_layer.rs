// SPDX-License-Identifier: GPL-3.0-or-later
//! `uv_layer` of the UV_PROJECT, UV_WARP, WARP and WAVE modifiers (WP 8.3.12): the name is
//! kept only when the object's mesh has a UV map of that name, else it becomes `''`.
//!
//! Ported from blender/source/blender/makesrna/intern/rna_modifier.cc
//! (`RNA_MOD_UVLAYER_NAME_SET`) and blender/source/blender/makesrna/intern/rna_object.cc
//! (`rna_object_uvlayer_name_set`), Blender 5.2.4, GPL-2.0-or-later.

use frap_rna::{MeshId, PropDef, RnaError, RnaPtr, Value, rna_hook};

use crate::Host;
use crate::id::paths::{internal, object_of};
use crate::id::{Modifier, ModifierSettings};

/// Declares a `=> hook` binding (the registry test pairs every `RNA_HOOKS` entry with one).
macro_rules! hook_decl {
    ($name:ident, $srna:literal, $prop:literal, $coll:literal) => {
        #[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
        #[frap_rna::linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
        #[linkme(crate = frap_rna::linkme)]
        static $name: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
            srna: $srna,
            prop: $prop,
            coll: $coll,
        };
    };
}

hook_decl!(
    UV_PROJECT_UV_LAYER_DECL,
    "UVProjectModifier",
    "uv_layer",
    false
);
hook_decl!(UV_WARP_UV_LAYER_DECL, "UVWarpModifier", "uv_layer", false);
hook_decl!(WARP_UV_LAYER_DECL, "WarpModifier", "uv_layer", false);
hook_decl!(WAVE_UV_LAYER_DECL, "WaveModifier", "uv_layer", false);

/// Stores `v` (or `''`) in the modifier's `uv_layer`.
fn set_uv_layer(h: &mut dyn Host, this: &RnaPtr, v: Value) -> Result<(), RnaError> {
    let name = match v {
        Value::Str(s) => s,
        _ => return Err(internal("uv_layer: not a string")),
    };
    let ob = object_of(this)?;
    let (main, log) = h.main_mut();
    let o = main
        .objects
        .get(ob)
        .ok_or_else(|| internal("uv_layer: the object was removed"))?;
    let i = Modifier::index_in(&o.modifiers, &this.path)
        .ok_or_else(|| internal("uv_layer: not a modifier pointer"))?;
    let exists = o.object_type() == "MESH"
        && o.data
            .and_then(|d| d.typed::<MeshId>())
            .and_then(|m| main.meshes.get(m))
            .is_some_and(|m| m.uv_maps().iter().any(|(n, _)| *n == name.as_str()));
    let value = if exists {
        name.to_string()
    } else {
        String::new()
    };
    let Some(o) = main.objects.get_mut(ob, log) else {
        return Ok(());
    };
    match &mut o.modifiers[i].settings {
        ModifierSettings::UvProject(s) => s.uv_layer = value,
        ModifierSettings::UvWarp(s) => s.uv_layer = value,
        ModifierSettings::Warp(s) => s.uv_layer = value,
        ModifierSettings::Wave(s) => s.uv_layer = value,
        _ => {
            return Err(internal(
                "uv_layer: not a UV_PROJECT/UV_WARP/WARP/WAVE modifier",
            ));
        }
    }
    Ok(())
}

#[rna_hook(srna = "UVProjectModifier", prop = "uv_layer", krate = crate)]
fn set_uv_project(
    h: &mut dyn Host,
    this: &RnaPtr,
    _prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_uv_layer(h, this, v)
}

#[rna_hook(srna = "UVWarpModifier", prop = "uv_layer", krate = crate)]
fn set_uv_warp(
    h: &mut dyn Host,
    this: &RnaPtr,
    _prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_uv_layer(h, this, v)
}

#[rna_hook(srna = "WarpModifier", prop = "uv_layer", krate = crate)]
fn set_warp(
    h: &mut dyn Host,
    this: &RnaPtr,
    _prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_uv_layer(h, this, v)
}

#[rna_hook(srna = "WaveModifier", prop = "uv_layer", krate = crate)]
fn set_wave(
    h: &mut dyn Host,
    this: &RnaPtr,
    _prop: &'static PropDef,
    v: Value,
) -> Result<(), RnaError> {
    set_uv_layer(h, this, v)
}
