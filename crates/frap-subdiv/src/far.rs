// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSubdiv 3.7 `Far`, to be ported for bit-exact face-varying evaluation (deviations D-121,
//! D-124; ARCHITECTURE §22.10 WP 8.11): feature-adaptive refinement with
//! `considerFVarChannels`, `StencilTableFactory` for `INTERPOLATE_FACE_VARYING` with
//! intermediate levels, `AppendLocalPointStencilTableFaceVarying`, the face-varying patch
//! tables and `EvaluatePatchBasis<float>`, in `f32` with the FMA contractions of Blender's
//! clang build (`f32::mul_add` where clang fuses), so the subdivided UVs of shared corners
//! differ by the same ulps as Blender's and the glTF exporter splits the same vertices
//! (Suzanne + SUBSURF 1: 2893 glTF vertices).
//!
//! M8 W0 stub (decision log M8W0-8): WP 8.11 owns `crates/frap-subdiv/**` (reassigned) and
//! adds `far/**` below this file.
