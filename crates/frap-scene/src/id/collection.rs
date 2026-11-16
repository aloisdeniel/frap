// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Collection` (§5.6). Also the type of `Scene.collection`, the embedded master
//! collection ("Scene Collection", `is_embedded_data == True`, verified).

use frap_rna::*;
use smol_str::SmolStr;

/// A collection (a `bpy.data.collections` ID, or a scene's embedded master collection).
#[derive(Clone, Debug)]
pub struct Collection {
    /// ID header (`is_embedded_data` for a scene's master collection).
    pub header: IdHeader,
    /// `collection.objects` (link order).
    pub objects: Vec<IdRef<ObjectId>>,
    /// `collection.children`
    pub children: Vec<IdRef<CollectionId>>,
    /// `collection.hide_select`
    pub hide_select: bool,
    /// `collection.hide_viewport`
    pub hide_viewport: bool,
    /// `collection.hide_render`
    pub hide_render: bool,
    /// `collection.color_tag` ("NONE", "COLOR_01" … "COLOR_08")
    pub color_tag: SmolStr,
}

impl Collection {
    /// New, empty collection (`bpy.data.collections.new(name)`).
    pub fn new(name: impl Into<SmolStr>) -> Collection {
        Collection::with_header(IdHeader::new(name))
    }
    /// A scene's embedded master collection ("Scene Collection").
    pub fn master() -> Collection {
        Collection::with_header(IdHeader::embedded("Scene Collection"))
    }
    fn with_header(header: IdHeader) -> Collection {
        Collection {
            header,
            objects: Vec::new(),
            children: Vec::new(),
            hide_select: false,
            hide_viewport: false,
            hide_render: false,
            color_tag: "NONE".into(),
        }
    }
}

crate::id::name_only_rna!(Collection, frap_spec_rna::COLLECTION, CollectionId);

impl IdType for Collection {
    type Kind = CollectionId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        self.objects.iter().for_each(|o| f(o.any()));
        self.children.iter().for_each(|c| f(c.any()));
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        let _ = (from, to);
        todo!("WP 1.10: unlink / relink objects and children")
    }
}
