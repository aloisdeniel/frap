// SPDX-License-Identifier: GPL-3.0-or-later
//! `bpy.types.Key` (`Mesh.shape_keys`) and `ShapeKey` key blocks. Positions are absolute per
//! key block, as Blender stores them; evaluation mixes them relatively ([`Key::evaluate`],
//! called by `frap-eval`'s `key_eval` before the modifier stack), and in Edit Mode each block
//! is a BMesh shape layer (`frap-bmesh/src/shape.rs`).
//!
//! WP 6.3 (M6). Ported from blender/source/blender/blenkernel/intern/key.cc
//! (`BKE_keyblock_add`, `BKE_keyblock_add_ctime`, `BKE_key_sort`, `BKE_keyblock_move`,
//! `BKE_keyblock_rename`, `BKE_object_shapekey_remove`'s block part,
//! `BKE_key_evaluate_object_ex`, `key_evaluate_relative_float3`, `key_evaluate_absolute`,
//! `get_keys_for_absolute_eval`, `key_curve_position_weights`) and
//! blender/source/blender/makesrna/intern/rna_key.cc (the RNA setters), Blender 5.2.4,
//! GPL-2.0-or-later.
//!
//! Verified on 5.2.4: `ShapeKey.value` is clamped to `slider_min..slider_max`, `slider_min`
//! to `-10..slider_max - 0.001` and `slider_max` to `slider_min + 0.001..10`; `frame` is
//! `pos × 100` and read-only; `relative_key` takes a block of the same key (another struct is
//! ignored); names are unique per key (`BLI_uniquename`, default `Key`, 63 bytes).

use frap_anim::AnimData;
use frap_rna::{
    Bulk, CollRef, Field, IdAny, IdHeader, IdRef, Item, MathTag, MeshId, PropDef, Rna, RnaColl,
    RnaCollMut, RnaCollMutGuard, RnaError, RnaMut, RnaMutGuard, SetOutcome, StructDef, StructRef,
    Value,
};
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
    /// `Key.uidgen`: the next [`ShapeKey::uid`] (`BKE_key_add` starts at 1).
    pub uidgen: i32,
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
            uidgen: 1,
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
    /// `select` (`KEYBLOCK_SEL`)
    pub select: bool,
    /// `KeyBlock.pos`: the position of an absolute key (`frame / 100`).
    pub pos: f32,
    /// `KeyBlock.uid`: unique in the key (from `Key.uidgen`), matches the block to its Edit
    /// Mode shape layer. 0 until the block is added to a key ([`Key::add_block`],
    /// [`Key::ensure_uids`]).
    pub uid: i32,
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
            select: false,
            pos: 0.0,
            uid: 0,
        }
    }

    /// `rna_ShapeKey_value_set`: clamped to the slider range.
    pub fn set_value(&mut self, v: f32) {
        self.value = v.max(self.slider_min).min(self.slider_max);
    }

    /// `rna_ShapeKey_slider_min_set`.
    pub fn set_slider_min(&mut self, v: f32) {
        self.slider_min = v.max(-10.0).min(self.slider_max - SLIDER_TOL);
    }

    /// `rna_ShapeKey_slider_max_set`.
    pub fn set_slider_max(&mut self, v: f32) {
        self.slider_max = v.max(self.slider_min + SLIDER_TOL).min(10.0);
    }

    /// `BKE_keyblock_copy_settings`: everything but the name, data and flags.
    pub fn copy_settings_from(&mut self, src: &ShapeKey) {
        self.pos = src.pos;
        self.value = src.value;
        self.interpolation = src.interpolation.clone();
        self.relative_key = src.relative_key;
        self.vertex_group = src.vertex_group.clone();
        self.slider_min = src.slider_min;
        self.slider_max = src.slider_max;
    }
}

/// `SHAPEKEY_SLIDER_TOL`.
const SLIDER_TOL: f32 = 0.001;
/// `sizeof(KeyBlock.name) - 1`.
pub const KEY_BLOCK_NAME_MAX: usize = 63;

/// How [`Key::evaluate`] reads the data of the object being evaluated.
pub struct EvalInput<'a> {
    /// Vertex count of the object's mesh.
    pub verts: usize,
    /// `BKE_keyblock_from_object`: the active block (`active_shape_key_index`).
    pub active: Option<usize>,
    /// `OB_SHAPE_LOCK` (`show_only_shape_key`).
    pub shape_lock: bool,
    /// The coordinates to use for the active block instead of its data (Edit Mode's
    /// `key_block_get_data` hack: the edit BMesh positions).
    pub active_override: Option<&'a [[f32; 3]]>,
    /// The weights of a vertex group by name (`None`: no such group or no weights).
    pub weights: &'a dyn Fn(&str) -> Option<Vec<f32>>,
}

impl Key {
    /// `BKE_keyblock_add`: appends a block named `name` (`None`: `Basis` for the first,
    /// `Key N` after), unique in the key, with `pos` one step after the last block's. Returns
    /// its index. The data is empty (callers fill it, `BKE_keyblock_convert_from_mesh`).
    pub fn add_block(&mut self, name: Option<&str>) -> usize {
        let curpos = self.key_blocks.last().map_or(-0.1, |k| k.pos);
        let tot = self.key_blocks.len() + 1;
        let requested: String = match name {
            Some(n) => n.to_owned(),
            None if tot == 1 => "Basis".to_owned(),
            None => format!("Key {}", tot - 1),
        };
        let name = self.unique_block_name(None, &requested);
        let mut kb = ShapeKey::new(name, Vec::new());
        kb.pos = curpos + 0.1;
        kb.uid = self.next_uid();
        self.key_blocks.push(kb);
        self.key_blocks.len() - 1
    }

    /// `key->uidgen++`.
    fn next_uid(&mut self) -> i32 {
        let uid = self.uidgen.max(1);
        self.uidgen = uid + 1;
        uid
    }

    /// Whether every block has a uid of its own (blocks built outside [`Key::add_block`], by
    /// `object.join` or a file reader, start at 0).
    pub fn uids_valid(&self) -> bool {
        self.key_blocks.iter().enumerate().all(|(i, k)| {
            k.uid > 0 && k.uid < self.uidgen && !self.key_blocks[..i].iter().any(|o| o.uid == k.uid)
        })
    }

    /// Gives every block without a uid of its own (0 or a duplicate) a new one (the uid check
    /// of `BM_mesh_bm_from_me` and file reading).
    pub fn ensure_uids(&mut self) {
        if self.uids_valid() {
            return;
        }
        let max = self.key_blocks.iter().map(|k| k.uid).max().unwrap_or(0);
        self.uidgen = self.uidgen.max(max + 1).max(1);
        for i in 0..self.key_blocks.len() {
            let uid = self.key_blocks[i].uid;
            if uid <= 0 || self.key_blocks[..i].iter().any(|o| o.uid == uid) {
                self.key_blocks[i].uid = self.next_uid();
            }
        }
    }

    /// `BKE_keyblock_add_ctime(key, name, do_force = false)`: [`Key::add_block`], then for an
    /// absolute key the block moves to the current evaluation time unless a block already
    /// sits there, and the blocks are re-sorted (`BKE_key_sort`). Returns the block's index.
    pub fn add_block_ctime(&mut self, name: Option<&str>) -> usize {
        let i = self.add_block(name);
        let cpos = self.eval_time / 100.0;
        if self.use_relative {
            return i;
        }
        if self
            .key_blocks
            .iter()
            .any(|k| compare_ff(k.pos, cpos, 1e-3))
        {
            return i;
        }
        self.key_blocks[i].pos = cpos;
        self.sort(i)
    }

    /// `BKE_key_sort` on an absolute key (a relative key is left alone, as `object.join`
    /// does).
    pub fn sort_absolute(&mut self) {
        if !self.use_relative {
            self.sort(0);
        }
    }

    /// `BKE_key_sort`: moves the first out-of-order block before the first block with a
    /// larger `pos`. Returns where block `tracked` ended up.
    fn sort(&mut self, tracked: usize) -> usize {
        let n = self.key_blocks.len();
        let Some(k) = (0..n.saturating_sub(1))
            .find(|&k| self.key_blocks[k].pos > self.key_blocks[k + 1].pos)
            .map(|k| k + 1)
        else {
            return tracked;
        };
        let kb = self.key_blocks.remove(k);
        let at = self
            .key_blocks
            .iter()
            .position(|b| b.pos > kb.pos)
            .unwrap_or(self.key_blocks.len());
        self.key_blocks.insert(at, kb);
        let mut order: Vec<usize> = (0..n).collect();
        let moved = order.remove(k);
        order.insert(at, moved);
        order.iter().position(|&o| o == tracked).unwrap_or(tracked)
    }

    /// `BLI_uniquename(&key->block, kb, "Key", '.', …)`: `requested` made unique among the
    /// blocks other than `skip`.
    pub fn unique_block_name(&self, skip: Option<usize>, requested: &str) -> SmolStr {
        frap_rna::naming::unique_subname(
            &|n| {
                self.key_blocks
                    .iter()
                    .enumerate()
                    .any(|(i, k)| Some(i) != skip && k.name == n)
            },
            requested,
            KEY_BLOCK_NAME_MAX,
            "Key",
        )
    }

    /// `BKE_keyblock_rename` (the block part; animation paths are fixed by the caller).
    pub fn rename_block(&mut self, i: usize, requested: &str) -> SmolStr {
        let name = self.unique_block_name(Some(i), requested);
        if let Some(k) = self.key_blocks.get_mut(i) {
            k.name = name.clone();
        }
        name
    }

    /// The index of the block named `name` (`BKE_keyblock_find_name`).
    pub fn find_block(&self, name: &str) -> Option<usize> {
        self.key_blocks.iter().position(|k| k.name == name)
    }

    /// The block part of `BKE_object_shapekey_remove`: removes block `i` and remaps the
    /// `relative_key` of the others (a block relative to it falls back to the basis, higher
    /// indices shift down). Returns the removed block; when it was the reference key the
    /// caller writes the new reference key's data into the mesh.
    pub fn remove_block(&mut self, i: usize) -> Option<ShapeKey> {
        if i >= self.key_blocks.len() {
            return None;
        }
        for kb in &mut self.key_blocks {
            if kb.relative_key == i {
                kb.relative_key = 0;
            } else if kb.relative_key >= i {
                kb.relative_key = kb.relative_key.saturating_sub(1);
            }
        }
        Some(self.key_blocks.remove(i))
    }

    /// `BKE_keyblock_move`: moves block `org` (`None`: the active one) to `new` (both
    /// clamped), swapping absolute positions on the way and remapping relative keys and the
    /// active index (`active`, 0-based). Returns whether anything moved.
    pub fn move_block(&mut self, org: Option<usize>, new: i64, active: &mut i64) -> bool {
        let totkey = self.key_blocks.len() as i64;
        if totkey == 0 {
            return false;
        }
        let act_index = *active;
        let org_index = org.map_or(act_index, |o| o as i64).clamp(0, totkey - 1);
        let new_index = new.clamp(0, totkey - 1);
        if new_index == org_index {
            return false;
        }
        let (o, n) = (org_index as usize, new_index as usize);
        // Swapping with the neighbours one by one also swaps their `pos`.
        if n > o {
            for k in o..n {
                self.key_blocks.swap(k, k + 1);
                let (a, b) = (self.key_blocks[k].pos, self.key_blocks[k + 1].pos);
                self.key_blocks[k].pos = b;
                self.key_blocks[k + 1].pos = a;
            }
        } else {
            for k in (n..o).rev() {
                self.key_blocks.swap(k, k + 1);
                let (a, b) = (self.key_blocks[k].pos, self.key_blocks[k + 1].pos);
                self.key_blocks[k].pos = b;
                self.key_blocks[k + 1].pos = a;
            }
        }
        for kb in &mut self.key_blocks {
            let r = kb.relative_key as i64;
            if r == org_index {
                kb.relative_key = n;
            } else if r < org_index && r >= new_index {
                kb.relative_key += 1;
            } else if r > org_index && r <= new_index {
                kb.relative_key -= 1;
            }
        }
        if org_index == act_index {
            *active = new_index;
        } else if act_index < org_index && act_index >= new_index {
            *active += 1;
        } else if act_index > org_index && act_index <= new_index {
            *active -= 1;
        }
        true
    }

    /// `BKE_keyblock_is_basis`: another block of a relative key is relative to block `i`.
    pub fn is_basis(&self, i: usize) -> bool {
        self.use_relative
            && self
                .key_blocks
                .iter()
                .enumerate()
                .any(|(k, kb)| k != i && kb.relative_key == i)
    }

    fn block_data<'a>(&'a self, i: usize, input: &EvalInput<'a>) -> &'a [[f32; 3]] {
        if Some(i) == input.active
            && let Some(o) = input.active_override
            && o.len() == self.key_blocks[i].data.len()
        {
            return o;
        }
        &self.key_blocks[i].data
    }

    /// `copy_key_float3`: block `i` into `out` (stepping through blocks of another length).
    fn copy_block(&self, i: usize, input: &EvalInput, out: &mut [[f32; 3]]) {
        let n = out.len();
        let src = self.block_data(i, input);
        if n == 0 || src.is_empty() {
            return;
        }
        if src.len() == n {
            out.copy_from_slice(src);
        } else {
            let step = src.len() as f32 / n as f32;
            for (k, o) in out.iter_mut().enumerate() {
                *o = src[((step * k as f32) as usize).min(src.len() - 1)];
            }
        }
    }

    /// `BKE_key_evaluate_object_ex` for a mesh object: the mixed positions, or `None` without
    /// blocks or vertices.
    ///
    /// Relative keys: the reference key's data, plus for every other unmuted block with a
    /// non-zero value and the mesh's vertex count `value × weight × (block − its relative
    /// key)` (`add_weighted_vector`; the relative key's own data, never the edited one).
    /// Absolute keys interpolate the blocks around `eval_time / 100`. `show_only_shape_key`
    /// copies the active block (the reference key when it is muted) blended with its vertex
    /// group.
    pub fn evaluate(&self, input: &EvalInput) -> Option<Vec<[f32; 3]>> {
        self.evaluate_ex(input)
    }

    /// [`Key::evaluate`] for a mesh object in Object Mode: `verts` vertices, `active` the
    /// object's active block (`active_shape_key_index`), `shape_lock` its
    /// `show_only_shape_key`, `weights` the weights of a vertex group by name.
    pub fn evaluate_mesh(
        &self,
        verts: usize,
        active: Option<usize>,
        shape_lock: bool,
        weights: &dyn Fn(&str) -> Option<Vec<f32>>,
    ) -> Option<Vec<[f32; 3]>> {
        self.evaluate_ex(&EvalInput {
            verts,
            active,
            shape_lock,
            active_override: None,
            weights,
        })
    }

    fn evaluate_ex(&self, input: &EvalInput) -> Option<Vec<[f32; 3]>> {
        let tot = input.verts;
        if self.key_blocks.is_empty() || tot == 0 {
            return None;
        }
        let mut out = vec![[0.0f32; 3]; tot];
        if input.shape_lock {
            let mut kb = input
                .active
                .filter(|&a| a < self.key_blocks.len())
                .unwrap_or(0);
            if self.key_blocks[kb].mute {
                kb = 0;
            }
            let weights = (input.weights)(&self.key_blocks[kb].vertex_group)
                .filter(|_| !self.key_blocks[kb].vertex_group.is_empty());
            match weights {
                None => self.copy_block(kb, input, &mut out),
                Some(w) => {
                    if self.key_blocks[kb].data.len() != tot {
                        return Some(out);
                    }
                    let src = self.block_data(kb, input);
                    let refd = self.block_data(0, input);
                    for (i, o) in out.iter_mut().enumerate() {
                        *o = refd[i];
                        let wi = w.get(i).copied().unwrap_or(0.0);
                        if wi != 0.0 {
                            add_weighted(o, wi, refd[i], src[i]);
                        }
                    }
                }
            }
            return Some(out);
        }
        if self.use_relative {
            self.copy_block(0, input, &mut out);
            for (k, kb) in self.key_blocks.iter().enumerate().skip(1) {
                if kb.mute || kb.data.len() != tot {
                    continue;
                }
                let influence = kb.value;
                if influence == 0.0 {
                    continue;
                }
                let Some(reference) = self.key_blocks.get(kb.relative_key) else {
                    continue;
                };
                let weights = if kb.vertex_group.is_empty() {
                    None
                } else {
                    (input.weights)(&kb.vertex_group)
                };
                let from = self.block_data(k, input);
                let reffrom = &reference.data;
                if reffrom.len() < tot {
                    continue;
                }
                for (i, o) in out.iter_mut().enumerate() {
                    let w = match &weights {
                        Some(w) => w.get(i).copied().unwrap_or(0.0) * influence,
                        None => influence,
                    };
                    add_weighted(o, w, reffrom[i], from[i]);
                }
            }
        } else {
            let (keys, weights, simple) = self.absolute_keys(self.eval_time / 100.0);
            if simple {
                self.copy_block(keys[2], input, &mut out);
            } else {
                let data: Vec<&[[f32; 3]]> =
                    keys.iter().map(|&k| self.block_data(k, input)).collect();
                let steps: Vec<f32> = data.iter().map(|d| d.len() as f32 / tot as f32).collect();
                for (i, o) in out.iter_mut().enumerate() {
                    let at = |k: usize| {
                        let d = data[k];
                        d[((i as f32 * steps[k]) as usize).min(d.len().saturating_sub(1))]
                    };
                    let (k1, k2, k3, k4) = (at(0), at(1), at(2), at(3));
                    for c in 0..3 {
                        o[c] = weights[0] * k1[c]
                            + weights[1] * k2[c]
                            + weights[2] * k3[c]
                            + weights[3] * k4[c];
                    }
                }
            }
        }
        Some(out)
    }

    /// `get_keys_for_absolute_eval`: the four blocks around `eval_time` and their weights;
    /// `true` when block `[2]` is to be copied as is.
    fn absolute_keys(&self, eval_time: f32) -> ([usize; 4], [f32; 4], bool) {
        let kbs = &self.key_blocks;
        let last = kbs.len() - 1;
        let pos = |i: usize| kbs[i].pos;
        let eval_time = eval_time.max(pos(0)).min(pos(last));
        let mut t = [0usize; 4];
        let mut w = [pos(0); 4];
        if last == 0 {
            return (t, w, true);
        }
        t[2] = 1;
        w[2] = pos(1);
        t[3] = if last >= 2 { 2 } else { 1 };
        w[3] = pos(t[3]);
        let mut iter = t[3];
        while w[2] < eval_time {
            if iter == last {
                if w[2] == w[3] {
                    break;
                }
            } else {
                iter += 1;
            }
            w[0] = w[1];
            t[0] = t[1];
            w[1] = w[2];
            t[1] = t[2];
            w[2] = w[3];
            t[2] = t[3];
            w[3] = pos(iter);
            t[3] = iter;
        }
        let ty = |i: usize| KeyInterp::from_identifier(&kbs[i].interpolation);
        let bspline = ty(t[1]) == KeyInterp::BSpline || ty(t[2]) == KeyInterp::BSpline;
        let mut eval_time = eval_time;
        if !bspline {
            if eval_time <= w[1] {
                w[2] = w[1];
                t[2] = t[1];
                return (t, w, true);
            }
            if eval_time >= w[2] {
                return (t, w, true);
            }
        } else if eval_time > w[2] {
            eval_time = w[2];
            t[3] = t[2];
            w[3] = w[2];
        }
        let mut delta = w[2] - w[1];
        if delta == 0.0 {
            if !bspline {
                return (t, w, true);
            }
        } else {
            delta = (eval_time - w[1]) / delta;
        }
        let mut weights = key_curve_position_weights(delta, ty(t[1]));
        if ty(t[1]) != ty(t[2]) {
            let other = key_curve_position_weights(delta, ty(t[2]));
            // `interp_v4_v4v4`.
            let s = 1.0 - delta;
            for (w, o) in weights.iter_mut().zip(other) {
                *w = s * *w + delta * o;
            }
        }
        (t, weights, false)
    }
}

impl Key {
    fn block_refs(&self) -> Vec<frap_bmesh::shape::BlockRef<'_>> {
        self.key_blocks
            .iter()
            .map(|k| frap_bmesh::shape::BlockRef {
                name: &k.name,
                data: &k.data,
                relative: k.relative_key,
                uid: k.uid,
            })
            .collect()
    }

    /// Entering Edit Mode (`BM_mesh_bm_from_me` with `use_shapekey`): one shape layer per
    /// block, the vertex positions from the active block (`active`, 0-based), which becomes
    /// `bm->shapenr`. The blocks need their uids ([`Key::ensure_uids`]).
    pub fn load_into_bmesh(&self, bm: &mut frap_bmesh::BMesh, active: Option<usize>) {
        frap_bmesh::shape::load_layers(bm, &self.block_refs(), active, true);
    }

    /// `BM_mesh_bm_from_me` without `use_shapekey` (Object Mode BMesh tools): one shape layer
    /// per block, the vertex positions left alone, no active block.
    pub fn load_layers_into_bmesh(&self, bm: &mut frap_bmesh::BMesh) {
        frap_bmesh::shape::load_layers(bm, &self.block_refs(), None, false);
    }

    /// `bm_to_mesh_shape` of a BMesh without an active block (an Object Mode conversion, or a
    /// BMesh made by a tool): every block takes its layer.
    pub fn write_layers_from_bmesh(&mut self, bm: &mut frap_bmesh::BMesh) {
        bm.shapenr = 0;
        self.write_back_from_bmesh(bm);
    }

    /// Leaving Edit Mode (`bm_to_mesh_shape`): the edited block is `bm->shapenr` (the block
    /// the BMesh was made with, by position in the current block list); a layer whose block
    /// was removed while editing is re-added as a new block (`BKE_keyblock_add` with the
    /// layer's name and uid); every block takes its layer, matched by uid (the active one the
    /// edited coordinates, the blocks depending on an edited basis its offset when no vertex
    /// was created while editing); a block without a layer is rebuilt from its data through
    /// the original vertex indices. Returns the positions the mesh must take instead of the
    /// edited ones (the reference key's, when another block was edited).
    pub fn write_back_from_bmesh(&mut self, bm: &mut frap_bmesh::BMesh) -> Option<Vec<[f32; 3]>> {
        let active = usize::try_from(bm.shapenr - 1)
            .ok()
            .filter(|&a| a < self.key_blocks.len());
        let uids: Vec<i32> = self.key_blocks.iter().map(|k| k.uid).collect();
        for (name, uid) in frap_bmesh::shape::orphan_layers(bm, &uids) {
            let i = self.add_block(Some(&name));
            self.key_blocks[i].uid = uid;
            self.uidgen = self.uidgen.max(uid + 1);
        }
        let wb = frap_bmesh::shape::write_back(bm, &self.block_refs(), active, self.use_relative);
        for (kb, data) in self.key_blocks.iter_mut().zip(wb.blocks) {
            kb.data = data;
        }
        wb.positions
    }
}

impl Key {
    /// The canonical pointer `bpy.data.shape_keys['Key'].key_blocks["Name"]` of a block.
    pub fn block_ptr(key: IdRef<frap_rna::KeyId>, name: &str) -> frap_rna::RnaPtr {
        frap_rna::RnaPtr::id(key.any(), &frap_spec_rna::KEY)
            .child(frap_rna::Seg::Prop("key_blocks"), &frap_spec_rna::SHAPE_KEY)
            .child(frap_rna::Seg::Key(name.into()), &frap_spec_rna::SHAPE_KEY)
    }

    /// The block a `key_blocks[<key or index>]…` path names.
    pub fn block_index_in(&self, path: &[frap_rna::Seg]) -> Option<usize> {
        use frap_rna::Seg;
        match path {
            [Seg::Prop("key_blocks"), Seg::Key(k), ..] => self.find_block(k),
            [Seg::Prop("key_blocks"), Seg::Index(i), ..] => {
                let n = self.key_blocks.len() as i64;
                let i = if *i < 0 { i + n } else { *i };
                (0..n).contains(&i).then_some(i as usize)
            }
            _ => None,
        }
    }

    /// The key and block a `ShapeKey` pointer names (pointers to blocks are rooted at their
    /// key, as Blender's `RNA_pointer_create_discrete(key, RNA_ShapeKey, kb)` are).
    pub fn block_of(
        main: &crate::data::Main,
        this: &frap_rna::RnaPtr,
    ) -> Option<(IdRef<frap_rna::KeyId>, usize)> {
        let frap_rna::Root::Id(a) = this.root else {
            return None;
        };
        let k: IdRef<frap_rna::KeyId> = a.typed()?;
        let i = main.shape_keys.get(k)?.block_index_in(&this.path)?;
        Some((k, i))
    }
}

/// The weights of vertex group `name` of `mesh` (`get_weights_array`): `None` when the mesh
/// has no such group or no weights (the block then has full influence).
pub fn mesh_group_weights(mesh: &frap_mesh::Mesh, name: &str) -> Option<Vec<f32>> {
    let g = mesh.vertex_group_names.iter().position(|n| n == name)? as u32;
    let dv = mesh.deform_verts()?;
    Some(
        dv.iter()
            .map(|d| {
                d.weights
                    .iter()
                    .find(|(x, _)| *x == g)
                    .map_or(0.0, |(_, w)| *w)
            })
            .collect(),
    )
}

impl Key {
    /// The mesh and key of a mesh object.
    fn object_mesh(
        main: &crate::data::Main,
        ob: IdRef<frap_rna::ObjectId>,
    ) -> Option<IdRef<MeshId>> {
        let d = main.objects.get(ob)?.data?;
        let me = d.typed::<MeshId>()?;
        main.meshes.get(me).map(|_| me)
    }

    /// `BKE_key_evaluate_object` for a mesh object in Object Mode (the mix `from_mix` and
    /// `apply_mix` read): `None` without a key, blocks or vertices.
    pub fn evaluate_object(
        main: &crate::data::Main,
        ob: IdRef<frap_rna::ObjectId>,
    ) -> Option<Vec<[f32; 3]>> {
        let me = Self::object_mesh(main, ob)?;
        let mesh = main.meshes.get(me)?;
        let key = main.shape_keys.get(mesh.shape_keys?)?;
        let o = main.objects.get(ob)?;
        let active = usize::try_from(o.active_shape_key_index)
            .ok()
            .filter(|&a| a < key.key_blocks.len());
        key.evaluate_mesh(mesh.verts_num(), active, o.show_only_shape_key, &|n| {
            mesh_group_weights(mesh, n)
        })
    }

    /// `BKE_key_add(bmain, mesh)` with `KEY_RELATIVE` when the mesh has no key: a key named
    /// `Key` (unique) owned by `me`. Returns the mesh's key.
    pub fn ensure_for_mesh(
        main: &mut crate::data::Main,
        log: &mut crate::data::ChangeLog,
        me: IdRef<MeshId>,
    ) -> Option<IdRef<frap_rna::KeyId>> {
        let mesh = main.meshes.get(me)?;
        if let Some(k) = mesh.shape_keys
            && main.shape_keys.get(k).is_some()
        {
            return Some(k);
        }
        let mut key = Key::new("Key");
        key.user = Some(me);
        let k = main.shape_keys.insert("Key", key, log);
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.shape_keys = Some(k);
        }
        Some(k)
    }

    /// `BKE_object_shapekey_insert` for a mesh object (`insert_meshkey`): a new block named
    /// `name` (`None`: `Basis` / `Key N`) holding the mesh positions, or with `from_mix` on an
    /// existing key the current mix (`BKE_key_evaluate_object`). The first block of a new key
    /// is always the mesh. Returns the key and the block's index; `None` for objects that are
    /// not meshes.
    pub fn object_insert(
        main: &mut crate::data::Main,
        log: &mut crate::data::ChangeLog,
        ob: IdRef<frap_rna::ObjectId>,
        name: Option<&str>,
        from_mix: bool,
    ) -> Option<(IdRef<frap_rna::KeyId>, usize)> {
        let me = Self::object_mesh(main, ob)?;
        let had_key = main
            .meshes
            .get(me)
            .and_then(|m| m.shape_keys)
            .is_some_and(|k| main.shape_keys.get(k).is_some());
        let mix = (had_key && from_mix).then(|| Self::evaluate_object(main, ob));
        let positions = main.meshes.get(me)?.positions().to_vec();
        let k = Self::ensure_for_mesh(main, log, me)?;
        let key = main.shape_keys.get_mut(k, log)?;
        let i = key.add_block_ctime(name);
        key.key_blocks[i].data = match mix {
            Some(m) => m.unwrap_or_default(),
            None => positions,
        };
        // "Set the first active when none is set when called from RNA" (`shapenr <= 0`).
        if main
            .objects
            .get(ob)
            .is_some_and(|o| o.active_shape_key_index < 0)
            && let Some(o) = main.objects.get_mut(ob, log)
        {
            o.active_shape_key_index = 0;
        }
        Some((k, i))
    }

    /// `BKE_object_shapekey_remove`: removes block `i` of the object's key. When it was the
    /// reference key the mesh takes the new reference key's coordinates; the active index
    /// moves down (`shapenr--` above the first block); the key is freed with its last block.
    pub fn object_remove(
        main: &mut crate::data::Main,
        log: &mut crate::data::ChangeLog,
        ob: IdRef<frap_rna::ObjectId>,
        i: usize,
    ) -> bool {
        let Some(me) = Self::object_mesh(main, ob) else {
            return false;
        };
        let Some(k) = main.meshes.get(me).and_then(|m| m.shape_keys) else {
            return false;
        };
        let Some(key) = main.shape_keys.get_mut(k, log) else {
            return false;
        };
        if key.remove_block(i).is_none() {
            return false;
        }
        let new_ref = (i == 0)
            .then(|| key.key_blocks.first().map(|kb| kb.data.clone()))
            .flatten();
        let empty = key.key_blocks.is_empty();
        if let Some(data) = new_ref
            && let Some(m) = main.meshes.get_mut(me, log)
        {
            let pos = m.positions_mut();
            let n = data.len().min(pos.len());
            pos[..n].copy_from_slice(&data[..n]);
        }
        if let Some(o) = main.objects.get_mut(ob, log) {
            if empty {
                // `shapenr = 0`: unset.
                o.active_shape_key_index = -1;
            } else if o.active_shape_key_index > 0 {
                o.active_shape_key_index -= 1;
            }
        }
        if empty {
            Self::object_free(main, log, ob);
        }
        true
    }

    /// `BKE_object_shapekey_free`: removes the object's key (the mesh loses it). False when
    /// there was none.
    pub fn object_free(
        main: &mut crate::data::Main,
        log: &mut crate::data::ChangeLog,
        ob: IdRef<frap_rna::ObjectId>,
    ) -> bool {
        let Some(me) = Self::object_mesh(main, ob) else {
            return false;
        };
        let Some(k) = main.meshes.get(me).and_then(|m| m.shape_keys) else {
            return false;
        };
        if let Some(m) = main.meshes.get_mut(me, log) {
            m.shape_keys = None;
        }
        main.remove(k.any(), log);
        true
    }
}

/// `add_weighted_vector`.
fn add_weighted(o: &mut [f32; 3], w: f32, a: [f32; 3], b: [f32; 3]) {
    o[0] += w * (b[0] - a[0]);
    o[1] += w * (b[1] - a[1]);
    o[2] += w * (b[2] - a[2]);
}

/// `compare_ff`.
fn compare_ff(a: f32, b: f32, max_diff: f32) -> bool {
    (a - b).abs() <= max_diff
}

/// `KeyInterpolationType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum KeyInterp {
    /// `KEY_LINEAR`
    Linear,
    /// `KEY_CARDINAL`
    Cardinal,
    /// `KEY_BSPLINE`
    BSpline,
    /// `KEY_CATMULL_ROM`
    CatmullRom,
}

impl KeyInterp {
    /// From the RNA identifier (`KEY_LINEAR` for anything else).
    pub fn from_identifier(s: &str) -> KeyInterp {
        match s {
            "KEY_CARDINAL" => KeyInterp::Cardinal,
            "KEY_BSPLINE" => KeyInterp::BSpline,
            "KEY_CATMULL_ROM" => KeyInterp::CatmullRom,
            _ => KeyInterp::Linear,
        }
    }
}

/// `key_curve_position_weights`.
pub fn key_curve_position_weights(t: f32, ty: KeyInterp) -> [f32; 4] {
    let t2 = t * t;
    let t3 = t2 * t;
    match ty {
        KeyInterp::Linear => [0.0, -t + 1.0, t, 0.0],
        KeyInterp::Cardinal | KeyInterp::CatmullRom => {
            let fc = if ty == KeyInterp::Cardinal { 0.71 } else { 0.5 };
            [
                -fc * t3 + 2.0 * fc * t2 - fc * t,
                (2.0 - fc) * t3 + (fc - 3.0) * t2 + 1.0,
                (fc - 2.0) * t3 + (3.0 - 2.0 * fc) * t2 + fc * t,
                fc * t3 - fc * t2,
            ]
        }
        KeyInterp::BSpline => [
            -0.16666666 * t3 + 0.5 * t2 - 0.5 * t + 0.16666666,
            0.5 * t3 - t2 + 0.666_666_7, // `0.66666666f`
            -0.5 * t3 + 0.5 * t2 + 0.5 * t + 0.16666666,
            0.16666666 * t3,
        ],
    }
}

// ---------------------------------------------------------------------------------- RNA

crate::id::id_rna! { Key as frap_spec_rna::KEY, "KEY" {
    reference_key => hook,
    key_blocks => coll(key_blocks, item = frap_spec_rna::SHAPE_KEY),
    user => hook,
    use_relative => use_relative,
    eval_time => eval_time,
}}

fn vec3(v: [f32; 3]) -> Value {
    Value::FloatArray {
        data: v.iter().copied().collect(),
        shape: frap_rna::ArrayShape::Fixed(3),
        tag: MathTag::Vector,
    }
}

/// `ShapeKey` bindings (hand-written: `data`/`points` are proxies over `data`; `name` and
/// `relative_key` need the key, `hooks/shape_key_{name,relative_key}.rs`).
impl Rna for ShapeKey {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::SHAPE_KEY
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        let f = |v: f32| Ok(Field::Value(Value::Float(v)));
        let b = |v: bool| Ok(Field::Value(Value::Bool(v)));
        match prop.identifier {
            "name" => Ok(Field::Value(Value::Str(self.name.clone()))),
            "select" => b(self.select),
            "frame" => f(self.pos * 100.0),
            "value" => f(self.value),
            "interpolation" => Ok(Field::Value(Value::Enum(self.interpolation.clone()))),
            "vertex_group" => Ok(Field::Value(Value::Str(self.vertex_group.clone()))),
            "relative_key" => Ok(Field::NeedsHook),
            "mute" => b(self.mute),
            "lock_shape" => b(self.lock_shape),
            "slider_min" => f(self.slider_min),
            "slider_max" => f(self.slider_max),
            "data" | "points" => Ok(Field::Coll(CollRef::Owned(Box::new(ShapeKeyPoints {
                data: &self.data,
            })))),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

impl RnaMut for ShapeKey {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        let srna = self.srna();
        let fl = v.as_f32();
        let bo = v.as_bool();
        match prop.identifier {
            "name" | "relative_key" => return Ok(SetOutcome::NeedsHook),
            "select" => self.select = bo.unwrap_or(self.select),
            "value" => {
                if let Some(x) = fl {
                    self.set_value(x);
                }
            }
            "interpolation" => {
                if let Some(s) = v.as_str() {
                    self.interpolation = s.into();
                }
            }
            "vertex_group" => {
                if let Some(s) = v.as_str() {
                    self.vertex_group =
                        frap_rna::naming::truncate_utf8(s, KEY_BLOCK_NAME_MAX).into();
                }
            }
            "mute" => self.mute = bo.unwrap_or(self.mute),
            "lock_shape" => self.lock_shape = bo.unwrap_or(self.lock_shape),
            "slider_min" => {
                if let Some(x) = fl {
                    self.set_slider_min(x);
                }
            }
            "slider_max" => {
                if let Some(x) = fl {
                    self.set_slider_max(x);
                }
            }
            "frame" | "data" | "points" => {
                return Err(frap_rna::traits::read_only(srna, prop.identifier));
            }
            _ => return Err(frap_rna::traits::no_attribute(srna, prop.identifier)),
        }
        Ok(SetOutcome::Done)
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a struct",
        ))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        match prop.identifier {
            "data" | "points" => Ok(Box::new(ShapeKeyPointsMut {
                data: &mut self.data,
            })),
            _ => Err(frap_rna::traits::wrong_access(
                self.srna(),
                prop.identifier,
                "a collection",
            )),
        }
    }
}

/// `ShapeKey.data` / `ShapeKey.points`: one `ShapeKeyPoint` per vertex.
pub struct ShapeKeyPoints<'a> {
    /// The block's coordinates.
    pub data: &'a [[f32; 3]],
}

fn points_foreach_get(
    data: &[[f32; 3]],
    prop: &'static PropDef,
    range: std::ops::Range<usize>,
) -> Result<Bulk, RnaError> {
    if prop.identifier != "co" {
        return Err(frap_rna::traits::no_attribute(
            &frap_spec_rna::SHAPE_KEY_POINT,
            prop.identifier,
        ));
    }
    let end = range.end.min(data.len());
    let start = range.start.min(end);
    Ok(Bulk::F32(
        data[start..end]
            .iter()
            .flat_map(|c| c.iter().copied())
            .collect(),
    ))
}

impl RnaColl for ShapeKeyPoints<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::SHAPE_KEY_POINT
    }
    fn len(&self) -> usize {
        self.data.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match self.data.get(i) {
            Some(co) => Ok(Item::Struct(StructRef::Owned(Box::new(ShapeKeyPointRef {
                co: *co,
            })))),
            None => Err(frap_rna::traits::index_out_of_range(i, self.data.len())),
        }
    }
    fn foreach_get(
        &self,
        prop: &'static PropDef,
        range: std::ops::Range<usize>,
    ) -> Result<Bulk, RnaError> {
        points_foreach_get(self.data, prop, range)
    }
}

/// Mutable `ShapeKey.data` / `points`.
pub struct ShapeKeyPointsMut<'a> {
    /// The block's coordinates.
    pub data: &'a mut Vec<[f32; 3]>,
}

impl RnaColl for ShapeKeyPointsMut<'_> {
    fn item_type(&self) -> &'static StructDef {
        &frap_spec_rna::SHAPE_KEY_POINT
    }
    fn len(&self) -> usize {
        self.data.len()
    }
    fn key_of(&self, _i: usize) -> Option<SmolStr> {
        None
    }
    fn find(&self, _key: &str) -> Option<usize> {
        None
    }
    fn item(&self, i: usize) -> Result<Item<'_>, RnaError> {
        match self.data.get(i) {
            Some(co) => Ok(Item::Struct(StructRef::Owned(Box::new(ShapeKeyPointRef {
                co: *co,
            })))),
            None => Err(frap_rna::traits::index_out_of_range(i, self.data.len())),
        }
    }
    fn foreach_get(
        &self,
        prop: &'static PropDef,
        range: std::ops::Range<usize>,
    ) -> Result<Bulk, RnaError> {
        points_foreach_get(self.data, prop, range)
    }
}

impl RnaCollMut for ShapeKeyPointsMut<'_> {
    fn item_mut(&mut self, i: usize) -> Result<RnaMutGuard<'_>, RnaError> {
        let len = self.data.len();
        match self.data.get_mut(i) {
            Some(co) => Ok(Box::new(ShapeKeyPointMut { co })),
            None => Err(frap_rna::traits::index_out_of_range(i, len)),
        }
    }
    fn foreach_set(&mut self, prop: &'static PropDef, data: &Bulk) -> Result<(), RnaError> {
        let n = self.data.len();
        match (prop.identifier, data) {
            ("co", Bulk::F32(v)) if v.len() == n * 3 => {
                for (c, x) in self.data.iter_mut().zip(v.chunks_exact(3)) {
                    *c = [x[0], x[1], x[2]];
                }
                Ok(())
            }
            ("co", _) => Err(RnaError::new(
                frap_rna::RnaErrorKind::Other,
                frap_rna::PyExc::RuntimeError,
                "internal error setting the array",
            )),
            _ => Err(frap_rna::traits::no_attribute(
                &frap_spec_rna::SHAPE_KEY_POINT,
                prop.identifier,
            )),
        }
    }
}

/// One `ShapeKeyPoint` (read).
pub struct ShapeKeyPointRef {
    /// `co`
    pub co: [f32; 3],
}

impl Rna for ShapeKeyPointRef {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::SHAPE_KEY_POINT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "co" => Ok(Field::Value(vec3(self.co))),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

/// One `ShapeKeyPoint` (write).
pub struct ShapeKeyPointMut<'a> {
    /// `co`
    pub co: &'a mut [f32; 3],
}

impl Rna for ShapeKeyPointMut<'_> {
    fn srna(&self) -> &'static StructDef {
        &frap_spec_rna::SHAPE_KEY_POINT
    }
    fn get(&self, prop: &'static PropDef) -> Result<Field<'_>, RnaError> {
        match prop.identifier {
            "co" => Ok(Field::Value(vec3(*self.co))),
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
}

impl RnaMut for ShapeKeyPointMut<'_> {
    fn set(&mut self, prop: &'static PropDef, v: Value) -> Result<SetOutcome, RnaError> {
        match (prop.identifier, v.as_vec3()) {
            ("co", Some(c)) => {
                *self.co = c;
                Ok(SetOutcome::Done)
            }
            _ => Err(frap_rna::traits::no_attribute(self.srna(), prop.identifier)),
        }
    }
    fn struct_mut(&mut self, prop: &'static PropDef) -> Result<RnaMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a struct",
        ))
    }
    fn coll_mut(&mut self, prop: &'static PropDef) -> Result<RnaCollMutGuard<'_>, RnaError> {
        Err(frap_rna::traits::wrong_access(
            self.srna(),
            prop.identifier,
            "a collection",
        ))
    }
}

/// `ShapeKey.name` / `relative_key` are hooks: their binding is hand-written above.
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL_SHAPE_KEY_NAME: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ShapeKey",
    prop: "name",
    coll: false,
};

/// See [`DECL_SHAPE_KEY_NAME`].
#[allow(unsafe_code)] // linkme registration (RFC 0001 F-LINK-1)
#[linkme::distributed_slice(frap_rna::testing::HOOK_DECLS)]
static DECL_SHAPE_KEY_RELATIVE_KEY: frap_rna::testing::HookDecl = frap_rna::testing::HookDecl {
    srna: "ShapeKey",
    prop: "relative_key",
    coll: false,
};

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

#[cfg(test)]
mod tests {
    use super::*;

    fn none(_: &str) -> Option<Vec<f32>> {
        None
    }

    fn key3() -> Key {
        let mut k = Key::new("Key");
        for (n, z) in [("Basis", 0.0), ("A", 1.0), ("B", 2.0)] {
            let i = k.add_block(Some(n));
            k.key_blocks[i].data = vec![[0.0, 0.0, z], [1.0, 0.0, z]];
        }
        k
    }

    #[test]
    fn add_block_names_follow_blender() {
        let mut k = Key::new("Key");
        let mut add = |n: Option<&str>| {
            let i = k.add_block(n);
            k.key_blocks[i].name.clone()
        };
        assert_eq!(add(None), "Basis");
        assert_eq!(add(None), "Key 1");
        assert_eq!(add(Some("Key 1")), "Key 1.001");
        assert_eq!(add(Some("")), "Key");
        assert!((k.key_blocks[1].pos - 0.1).abs() < 1e-6);
    }

    #[test]
    fn relative_mix_adds_weighted_offsets() {
        let mut k = key3();
        k.key_blocks[1].value = 0.5;
        k.key_blocks[2].value = 0.25;
        k.key_blocks[2].relative_key = 1;
        let w = |n: &str| (n == "G").then(|| vec![1.0, 0.0]);
        let input = EvalInput {
            verts: 2,
            active: Some(0),
            shape_lock: false,
            active_override: None,
            weights: &none,
        };
        let out = k.evaluate(&input).unwrap();
        assert!((out[0][2] - 0.75).abs() < 1e-6);
        k.key_blocks[1].vertex_group = "G".into();
        let input = EvalInput {
            weights: &w,
            ..input
        };
        let out = k.evaluate(&input).unwrap();
        assert!((out[0][2] - 0.75).abs() < 1e-6);
        assert!((out[1][2] - 0.25).abs() < 1e-6);
        k.key_blocks[2].mute = true;
        let out = k.evaluate(&input).unwrap();
        assert!((out[1][2]).abs() < 1e-6);
    }

    #[test]
    fn shape_lock_copies_the_active_block() {
        let k = key3();
        let input = EvalInput {
            verts: 2,
            active: Some(2),
            shape_lock: true,
            active_override: None,
            weights: &none,
        };
        assert_eq!(k.evaluate(&input).unwrap()[0][2], 2.0);
    }

    #[test]
    fn move_block_remaps_relative_and_active() {
        let mut k = key3();
        k.key_blocks[2].relative_key = 1;
        let mut active = 2;
        assert!(k.move_block(None, 1, &mut active));
        assert_eq!(active, 1);
        assert_eq!(k.key_blocks[1].name, "B");
        assert_eq!(k.key_blocks[1].relative_key, 2);
        assert!(!k.move_block(Some(1), 1, &mut active));
    }

    #[test]
    fn remove_block_remaps_relative_keys() {
        let mut k = key3();
        k.key_blocks[2].relative_key = 1;
        k.remove_block(1);
        assert_eq!(k.key_blocks[1].relative_key, 0);
        let mut k = key3();
        k.key_blocks[1].relative_key = 2;
        k.remove_block(0);
        assert_eq!(k.key_blocks[0].relative_key, 1);
    }

    #[test]
    fn slider_setters_clamp() {
        let mut s = ShapeKey::new("A", vec![]);
        s.set_slider_max(0.0);
        assert!((s.slider_max - 0.001).abs() < 1e-6);
        s.set_value(5.0);
        assert!((s.value - 0.001).abs() < 1e-6);
        s.set_slider_min(-20.0);
        assert_eq!(s.slider_min, -10.0);
    }

    #[test]
    fn absolute_keys_interpolate_linearly() {
        let mut k = key3();
        k.use_relative = false;
        k.eval_time = 15.0;
        let input = EvalInput {
            verts: 2,
            active: None,
            shape_lock: false,
            active_override: None,
            weights: &none,
        };
        // pos 0, 0.1, 0.2; t = 0.15 is halfway between A and B.
        let out = k.evaluate(&input).unwrap();
        assert!((out[0][2] - 1.5).abs() < 1e-5, "{out:?}");
    }
}
