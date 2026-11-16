// SPDX-License-Identifier: GPL-3.0-or-later
//! `frap-rna` (layer L1): identity, RNA metadata, values and coercion, property paths,
//! data-access traits, operator spec types, errors and reports (ARCHITECTURE §5.1, §9, §10.1,
//! §13.4). CONTRACT frozen at M1 W0 after RFC 0001; contents owned by WP 1.2.
#![deny(unsafe_code)]

pub mod def;
pub mod error;
pub mod id;
pub mod id_kinds;
pub mod naming;
pub mod op;
pub mod ptr;
pub mod serde_buf;
pub mod serde_id;
pub mod testing;
pub mod traits;
pub mod value;

pub use def::{
    ArrayShape, Defaults, Deprecation, EnumDefault, EnumItem, FloatStorage, FuncDef, FuncFlags,
    PropDef, PropFlags, PropKind, StructDef, Subtype, Unit,
};
pub use error::{
    ERROR_CODES, ErrorCode, ErrorCodeDoc, FrapError, PyExc, Report, ReportType, RnaError,
    RnaErrorKind, Span, codes,
};
pub use id::{EmbeddedSlot, IdAny, IdCode, IdHeader, IdKey, IdKind, IdProp, IdRef, IdType};
pub use id_kinds::{
    ActionId, ArmatureId, CameraId, CollectionId, ImageId, KeyId, LibraryId, LightId, MaterialId,
    MeshId, NodeTreeId, ObjectId, PointCloudId, SceneId, TextureId, WorldId,
};
pub use op::{
    BitSet, FromValue, OpArgs, OpOptions, OpProps, OperatorSpec, PollCell, PollMatrix, Prop,
};
pub use ptr::{IdNames, Resolved, RnaPtr, Root, Seg};
pub use traits::{
    Bulk, CollRef, Field, Item, Rna, RnaColl, RnaCollMut, RnaCollMutGuard, RnaMut, RnaMutGuard,
    SetOutcome, StructRef,
};
pub use value::{
    CoerceCx, Coerced, EulerOrder, Input, MathTag, MathValue, Value, clamp_report, coerce,
    describe_input, py_float_repr,
};

/// RNA-tier proc-macros (WP 1.2).
pub use frap_macros::{rna_func, rna_hook, rna_struct};
/// Re-export for registration sites (`#[linkme::distributed_slice(...)]`).
pub use linkme;
