// SPDX-License-Identifier: GPL-3.0-or-later
//! Custom split normals of an edit BMesh: moved to `frap_bmesh::clnor` (M5+M6 W0) so the bevel's
//! `harden_normals` shares it; re-exported here for the custom-normal operators.

pub(crate) use frap_bmesh::clnor::*;
