// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-modifiers` (layer L7): the modifier plugin contract (ARCHITECTURE §7.2, CONTRACT frozen
//! at M4 W0): [`ModifierTypeInfo`] entries registered into [`MODIFIER_TYPES`] by
//! `#[modifier(...)]`, one evaluator per file in `types/<type>.rs`, the [`ModEvalCx`] they
//! evaluate in (implemented by `frap-eval`), the type registry ([`registry`]) and the stack
//! walk ([`stack`]).
//!
//! ```ignore
//! // crates/frap-modifiers/src/types/mirror.rs — the whole unit of work
//! use frap_modifiers::prelude::*;
//!
//! #[modifier(MIRROR, kind = Constructive, flags = ACCEPTS_MESH | SUPPORTS_MAPPING | SUPPORTS_EDITMODE | ENABLE_IN_EDITMODE | ACCEPTS_CVS)]
//! fn eval(md: &Modifier, mesh: Mesh, cx: &mut dyn ModEvalCx) -> Result<Mesh, ModError> { … }
//! ```
//!
//! A type without an evaluator passes the mesh through with a WARNING
//! (`Modifier 'Remesh' (REMESH) is not evaluated by frap; passed through`).
//!
//! Ownership (M4): WP 4.1 owns `src/{lib,stack,registry}.rs` and `src/types/nodes.rs` after W0;
//! WPs 4.2–4.6 (and 6.5 in M6) add `src/types/<type>.rs` files (automod, no `mod` edit).
#![deny(unsafe_code)]

pub mod registry;
pub mod stack;

/// One evaluator per modifier type (`types/<type>.rs`), each registering one
/// [`ModifierTypeInfo`] with `#[modifier(...)]`.
pub mod types {
    automod::dir!(pub "src/types");
}

use std::sync::Arc;

use frap_boolean::BooleanSolver;
use frap_imbuf::DecodedImage;
use frap_rna::{IdAny, IdRef, ImageId, ObjectId, Report};
use frap_subdiv::Subdivider;
use glam::Mat4;
use smallvec::SmallVec;

pub use frap_macros::modifier;
pub use frap_mesh::Mesh;
pub use frap_scene::generated::modifier_settings::ModifierType;
pub use frap_scene::id::{Modifier, ModifierSettings, Object};
pub use frap_scene::{EvalMode, EvaluatedPose, Main};
#[doc(hidden)]
pub use linkme;

/// Blender's `ModifierTypeType` (without `None`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ModKind {
    /// Only moves vertices (`deform_verts`).
    OnlyDeform,
    /// Adds geometry.
    Constructive,
    /// Can add and remove geometry.
    Nonconstructive,
    /// `DeformOrConstruct` (particle-style types).
    DeformOrConstruct,
    /// Changes custom-data layers only (`NonGeometrical`: UV Project, vertex weight types).
    NonGeometrical,
}

bitflags::bitflags! {
    /// Blender's `ModifierTypeFlag` (`eModifierTypeFlag_*`), same bit values.
    #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
    pub struct ModTypeFlags: u32 {
        /// `AcceptsMesh`
        const ACCEPTS_MESH = 1 << 0;
        /// `AcceptsCVs`
        const ACCEPTS_CVS = 1 << 1;
        /// `SupportsMapping` (`show_on_cage` is meaningful).
        const SUPPORTS_MAPPING = 1 << 2;
        /// `SupportsEditmode`
        const SUPPORTS_EDITMODE = 1 << 3;
        /// `EnableInEditmode` (a new modifier has `show_in_editmode` on).
        const ENABLE_IN_EDITMODE = 1 << 4;
        /// `RequiresOriginalData` (cannot follow a non-deforming modifier).
        const REQUIRES_ORIGINAL_DATA = 1 << 5;
        /// `UsesPointCache`
        const USES_POINT_CACHE = 1 << 6;
        /// `Single` (at most one per object).
        const SINGLE = 1 << 7;
        /// `NoUserAdd`
        const NO_USER_ADD = 1 << 8;
        /// `AcceptsVertexCosOnly`
        const ACCEPTS_VERTEX_COS_ONLY = 1 << 10;
        /// `AcceptsBMesh`
        const ACCEPTS_BMESH = 1 << 11;
        /// `AcceptsGreasePencil`
        const ACCEPTS_GREASE_PENCIL = 1 << 12;
    }
}

/// Why a modifier did not produce a mesh. The stack records it as
/// [`frap_scene::ModifierStatus::Error`] and continues with the input mesh, as Blender does
/// with a modifier error (`BKE_modifier_set_error`).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ModError {
    /// Blender's modifier error text (`"Cannot execute boolean operation"`, …).
    #[error("{0}")]
    Error(String),
    /// A dependency could not be evaluated (operand object, armature, …).
    #[error("{0}")]
    Dependency(String),
}

/// One modifier type (ARCHITECTURE §7.2, CONTRACT). Registered by `#[modifier(...)]`.
#[derive(Debug)]
pub struct ModifierTypeInfo {
    /// The type (generated enum).
    pub ty: ModifierType,
    /// `ModifierTypeType`.
    pub kind: ModKind,
    /// `ModifierTypeFlag`.
    pub flags: ModTypeFlags,
    /// IDs the result depends on (operand objects, textures, node groups): part of the
    /// evaluation fingerprint (§7.3).
    pub depends_on: fn(&ModifierSettings) -> SmallVec<[IdAny; 2]>,
    /// `is_disabled`: the reason a modifier is skipped (`"Boolean: no operand"`), or `None`.
    pub is_disabled: fn(&ModifierSettings) -> Option<&'static str>,
    /// Evaluates the modifier on `mesh`.
    pub eval: fn(&Modifier, Mesh, &mut dyn ModEvalCx) -> Result<Mesh, ModError>,
}

/// Every modifier evaluator.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice]
pub static MODIFIER_TYPES: [ModifierTypeInfo];

/// Keeps every `types/<type>.rs` registration in the final binary (§10.5): `frap-ops-all`
/// references it next to the ops crates' `link()`, so the linker never drops a type file.
pub fn link() {}

/// `depends_on` of a type without dependencies.
pub fn no_deps(_: &ModifierSettings) -> SmallVec<[IdAny; 2]> {
    SmallVec::new()
}

/// `is_disabled` of a type that is never disabled.
pub fn never_disabled(_: &ModifierSettings) -> Option<&'static str> {
    None
}

/// What an evaluator may read and ask for (ARCHITECTURE §7.2, CONTRACT; implemented by
/// `frap-eval`). `main`, `object_ref` and `frame` were added at M4 W0 (decision log M34W0):
/// DISPLACE reads its `Texture`, BOOLEAN a collection's objects, NODES the node group's
/// library, BUILD the scene frame.
pub trait ModEvalCx {
    /// `bpy.data` as evaluated.
    fn main(&self) -> &Main;
    /// The object whose stack is evaluated.
    fn object(&self) -> &Object;
    /// Its reference.
    fn object_ref(&self) -> IdRef<ObjectId>;
    /// The evaluated mesh of another object (operands, mirror objects), in this mode.
    fn evaluated_mesh(&mut self, ob: IdRef<ObjectId>) -> Result<Arc<Mesh>, ModError>;
    /// The evaluated world matrix of an object.
    fn object_matrix_world(&mut self, ob: IdRef<ObjectId>) -> Mat4;
    /// The evaluated pose of an armature object (M6).
    fn pose(&mut self, arm: IdRef<ObjectId>) -> Result<Arc<EvaluatedPose>, ModError>;
    /// Decoded pixels of an image (DISPLACE textures); `None` when it cannot be loaded.
    fn image(&mut self, im: IdRef<ImageId>) -> Option<Arc<DecodedImage>>;
    /// Viewport or render evaluation (levels, `show_render`).
    fn mode(&self) -> EvalMode;
    /// The scene time (`DEG_get_ctime`: the frame, BUILD).
    fn frame(&self) -> f32;
    /// Adds a report (WARNING/ERROR) to the evaluated object's warnings.
    fn report(&mut self, r: Report);
    /// The subdivision backend (SUBSURF).
    fn subdivider(&self) -> &dyn Subdivider;
    /// The boolean backend (BOOLEAN).
    fn boolean_solver(&self) -> &dyn BooleanSolver;
    /// Whether the armature data of `arm` is in Edit Mode (`arm->edbo`: the ARMATURE deform
    /// is skipped, WP 6.5). Default: never.
    fn armature_in_edit_mode(&self, _arm: IdRef<ObjectId>) -> bool {
        false
    }
    /// `ArmatureModifierData::vert_coords_prev` of this stack evaluation (WP 6.5): the
    /// coordinates an ARMATURE modifier hands to the `use_multi_modifier` ones after it
    /// (`MOD_previous_vcos_store`). Lives only for one evaluation of one object's stack, never
    /// on mesh data. Default: no slot (nothing is stored).
    fn prev_coords_slot(&mut self) -> Option<&mut Option<Vec<[f32; 3]>>> {
        None
    }
}

/// What a `types/<type>.rs` file needs.
pub mod prelude {
    pub use crate::{
        ModError, ModEvalCx, ModKind, ModTypeFlags, Modifier, ModifierSettings, ModifierType,
        ModifierTypeInfo, modifier, never_disabled, no_deps,
    };
    pub use frap_mesh::Mesh;
    /// The return type of `depends_on`.
    pub use smallvec::SmallVec;
}
