// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Library` (§5.1): only the libraries frap's built-in linked groups need.

use smol_str::SmolStr;

/// A library (`LI`). Linked IDs point at it through `IdHeader::library`.
#[derive(Clone, Debug)]
pub struct Library {
    /// ID header.
    pub header: frap_rna::IdHeader,
    /// `Library.filepath` (stored as given).
    pub filepath: SmolStr,
    /// `Library.parent`
    pub parent: Option<frap_rna::IdRef<frap_rna::LibraryId>>,
    /// `Library.is_archive`
    pub is_archive: bool,
}

crate::id::name_only_rna!(Library, frap_spec_rna::LIBRARY, LibraryId);
crate::id::id_type!(Library, LibraryId);
