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

crate::id::id_rna! { Library as frap_spec_rna::LIBRARY, "LIBRARY" {
    filepath => filepath,
    parent => id(parent),
    is_archive => readonly(|l| l.is_archive),
}}
impl frap_rna::IdType for Library {
    type Kind = frap_rna::LibraryId;
    fn header(&self) -> &frap_rna::IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut frap_rna::IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(frap_rna::IdAny)) {
        if let Some(p) = self.parent {
            f(p.any());
        }
    }
    fn remap_ref(&mut self, from: frap_rna::IdAny, to: Option<frap_rna::IdAny>) {
        if self.parent.map(|p| p.any()) == Some(from) {
            self.parent = to.and_then(frap_rna::IdRef::from_any);
        }
    }
}

impl Library {
    /// A library (`bpy.data.libraries` entry) with the given file path.
    pub fn new(name: &str, filepath: &str) -> Library {
        Library {
            header: frap_rna::IdHeader::new(name),
            filepath: filepath.into(),
            parent: None,
            is_archive: false,
        }
    }
}
