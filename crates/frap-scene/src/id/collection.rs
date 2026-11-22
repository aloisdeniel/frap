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
    /// `collection.instance_offset` (subtracted before the instancer's matrix; WP 8.3.4).
    pub instance_offset: [f32; 3],
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
            instance_offset: [0.0; 3],
        }
    }
}

crate::id::id_rna! { Collection as frap_spec_rna::COLLECTION, "COLLECTION" {
    objects => id_coll(objects, item = frap_spec_rna::OBJECT, api = "CollectionObjects"),
    children => id_coll(children, item = frap_spec_rna::COLLECTION, api = "CollectionChildren"),
    all_objects => hook_coll,
    hide_select => hide_select (set = hook),
    hide_viewport => hide_viewport (set = hook),
    hide_render => hide_render,
    color_tag => color_tag,
    instance_offset => instance_offset,
}}

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
    /// Unlink (`to = None`) or relink: an object or child listed twice after a relink keeps
    /// its first place (Blender's `collection_object_remap` removes duplicates).
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        fn remap<K: IdKind>(v: &mut Vec<IdRef<K>>, from: IdAny, to: Option<IdAny>) {
            let to = to.and_then(IdRef::<K>::from_any);
            let mut out: Vec<IdRef<K>> = Vec::with_capacity(v.len());
            for r in v.drain(..) {
                let r = if r.any() == from {
                    match to {
                        Some(t) => t,
                        None => continue,
                    }
                } else {
                    r
                };
                if !out.contains(&r) {
                    out.push(r);
                }
            }
            *v = out;
        }
        remap(&mut self.objects, from, to);
        remap(&mut self.children, from, to);
    }
}
