// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Key` (`Mesh.shape_keys`) and `ShapeKey` key blocks. Positions are absolute per
//! key block, as Blender stores them; evaluation mixes them relatively (`frap-eval`'s
//! `key_eval`, WP 6.3), and in Edit Mode each block is a BMesh shape layer
//! (`frap-bmesh/src/shape.rs`).
//!
//! M5+M6 W0 contract; WP 6.3 owns the file, including the RNA bindings (only `ID.name` is
//! bound until it lands).

use frap_anim::AnimData;
use frap_rna::{IdAny, IdHeader, IdRef, MeshId};
use smol_str::SmolStr;

/// `bpy.types.Key`.
#[derive(Clone, Debug)]
pub struct Key {
    /// ID header.
    pub header: IdHeader,
    /// `Key.user` (the mesh owning the key).
    pub user: Option<IdRef<MeshId>>,
    /// `Key.key_blocks`; index 0 is the basis (`reference_key`).
    pub key_blocks: Vec<ShapeKey>,
    /// `Key.use_relative`
    pub use_relative: bool,
    /// `Key.eval_time` (absolute keys)
    pub eval_time: f32,
    /// `Key.animation_data` (shape-key values are keyed into the object's action under a `KE`
    /// slot; this is the key's own AnimData, ARCHITECTURE §18)
    pub animation_data: Option<AnimData>,
}

impl Key {
    /// A key without blocks for `user` (`Object.shape_key_add` creates it as "Key").
    pub fn new(name: impl Into<SmolStr>) -> Key {
        Key {
            header: IdHeader::new(name),
            user: None,
            key_blocks: Vec::new(),
            use_relative: true,
            eval_time: 0.0,
            animation_data: None,
        }
    }
}

/// `bpy.types.ShapeKey` (a `KeyBlock`).
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeKey {
    /// `name`
    pub name: SmolStr,
    /// Absolute positions, one per mesh vertex (`ShapeKey.data[i].co`).
    pub data: Vec<[f32; 3]>,
    /// `value`
    pub value: f32,
    /// `slider_min`
    pub slider_min: f32,
    /// `slider_max`
    pub slider_max: f32,
    /// `mute`
    pub mute: bool,
    /// `relative_key` (index into `Key::key_blocks`)
    pub relative_key: usize,
    /// `vertex_group` (name; empty = none)
    pub vertex_group: SmolStr,
    /// `interpolation` (`KEY_LINEAR`, `KEY_CARDINAL`, `KEY_CATMULL_ROM`, `KEY_BSPLINE`)
    pub interpolation: SmolStr,
    /// `lock_shape`
    pub lock_shape: bool,
}

impl ShapeKey {
    /// A block named `name` holding `data`, relative to the basis.
    pub fn new(name: impl Into<SmolStr>, data: Vec<[f32; 3]>) -> ShapeKey {
        ShapeKey {
            name: name.into(),
            data,
            value: 0.0,
            slider_min: 0.0,
            slider_max: 1.0,
            mute: false,
            relative_key: 0,
            vertex_group: SmolStr::default(),
            interpolation: "KEY_LINEAR".into(),
            lock_shape: false,
        }
    }
}

crate::id::name_only_rna!(Key, frap_spec_rna::KEY, KeyId);

impl frap_rna::IdType for Key {
    type Kind = frap_rna::KeyId;
    fn header(&self) -> &IdHeader {
        &self.header
    }
    fn header_mut(&mut self) -> &mut IdHeader {
        &mut self.header
    }
    // `Key.user` is a back pointer (`IDWALK_CB_LOOPBACK`): not a user, not remapped as one.
    fn for_each_ref(&self, f: &mut dyn FnMut(IdAny)) {
        if let Some(a) = self.animation_data.as_ref().and_then(|a| a.action) {
            f(a.into());
        }
    }
    fn remap_ref(&mut self, from: IdAny, to: Option<IdAny>) {
        crate::id::remap_action(&mut self.animation_data, from, to);
        if let Some(u) = self.user
            && IdAny::from(u) == from
        {
            self.user = to.and_then(|t| t.typed());
        }
    }
}
