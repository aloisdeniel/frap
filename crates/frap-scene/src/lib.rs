// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-scene` (layer L6): `Main` (= `bpy.data`), `IdMap<T>`, every datablock type except
//! Mesh/NodeTree/Action, `Scene`/`ViewLayer`/`Base`/`Collection`, `WindowState`, `EditState`,
//! `Context`, the `Host` and `Evaluator` traits, the RNA hook and function registries, factory
//! startup, generated plain structs and the scene RNA bindings (ARCHITECTURE §5, §8, §9.5).
//!
//! Ownership (M1): WP 1.10 owns everything except `src/generated/**` (WP 1.1) and this file.
//! CONTRACT files (frozen at M1 W0): `data.rs`, `context.rs`, `host.rs`, `eval.rs`.
#![deny(unsafe_code)]

mod context;
mod data;
pub mod edit_armature;
mod eval;
mod host;
pub mod id;
/// Multiple scenes (M3, WP 3.10).
pub mod scenes;
pub mod startup;
/// The window's scene (M3, WP 3.10).
pub mod window;

/// Generated plain structs (`cargo xtask codegen`, WP 1.1).
#[rustfmt::skip]
#[allow(missing_docs, clippy::all)]
pub mod generated;

/// RNA functions that only touch scene data (`select_set`, `hide_set`, …), one file each
/// (`funcs/<struct>_<func>.rs`), registered in `RNA_FUNCS`.
pub mod funcs {
    automod::dir!("src/funcs");
}
/// Setters/getters with side effects (`rotation_mode`, `parent`, `name`, `matrix_world`, …),
/// one file each, registered in `RNA_HOOKS`.
pub mod hooks {
    automod::dir!("src/hooks");
}

pub use context::{
    Base, BaseFlag, BoneRef, Context, ContextMode, EditArmature, EditMesh, EditState,
    LayerCollection, LayerCollectionPath, ObjectMode, Preferences, ViewCache, ViewLayer,
    VirtualView3D, WindowState,
};
pub use data::{ChangeLog, IdMap, Main, RenameMode, RenameResult};
pub use eval::{EvaluatedObject, EvaluatedPose, ModifierStatus};
pub use host::{
    EvalError, EvalMode, Evaluator, FuncArgs, Host, HostRead, RNA_FUNCS, RNA_HOOKS, RnaFuncImpl,
    RnaFuncReg, RnaGetHook, RnaHookReg, RnaSetHook, Snapshot, find_func, find_hook,
};
