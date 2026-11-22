// SPDX-License-Identifier: GPL-3.0-or-later
//! `Far::StencilTableReal<float>` for one face-varying channel: `StencilBuilder`'s weight
//! table (factorised, compacted stencils whose entries keep first-seen order),
//! `StencilTableFactory::Create` with `INTERPOLATE_FACE_VARYING`,
//! `AppendLocalPointStencilTableFaceVarying`, and `Osd::CpuEvalStencils` (the slow path for
//! two-component data, with clang's fused multiply-adds).
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/far/{stencilBuilder,stencilTable,
//! stencilTableFactory}.{h,cpp}` and `opensubdiv/osd/cpuKernel.cpp` (Pixar, TOST / Apache-2.0
//! modified).

use super::primvar::{self, FvarSink};
use super::refiner::TopologyRefiner;

/// A table of stencils over `num_control` control values.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StencilTable {
    pub num_control: i32,
    pub sizes: Vec<i32>,
    pub offsets: Vec<i32>,
    pub indices: Vec<i32>,
    pub weights: Vec<f32>,
}

impl StencilTable {
    pub(crate) fn num_stencils(&self) -> usize {
        self.sizes.len()
    }

    /// `GetStencil(i)`: `(indices, weights)`.
    pub(crate) fn stencil(&self, i: usize) -> (&[i32], &[f32]) {
        let o = self.offsets[i] as usize;
        let n = self.sizes[i] as usize;
        (&self.indices[o..o + n], &self.weights[o..o + n])
    }

    /// `generateOffsets`
    pub(crate) fn generate_offsets(&mut self) {
        self.offsets.clear();
        let mut o = 0;
        for &s in &self.sizes {
            self.offsets.push(o);
            o += s;
        }
    }

    /// `Osd::CpuEvalStencils` on a two-component buffer: `data[num_control + i]` gets stencil
    /// `i` applied to `data[..]` (`dst[k] += src[k] * weight`, fused).
    pub(crate) fn eval_into(&self, data: &mut [[f32; 2]]) {
        let base = self.num_control as usize;
        for i in 0..self.num_stencils() {
            let (idx, w) = self.stencil(i);
            let mut r = [0.0f32; 2];
            for j in 0..idx.len() {
                let s = data[idx[j] as usize];
                r[0] = s[0].mul_add(w[j], r[0]);
                r[1] = s[1].mul_add(w[j], r[1]);
            }
            data[base + i] = r;
        }
    }
}

/// `internal::WeightTable<float>` (scalar weights).
struct WeightTable {
    dests: Vec<i32>,
    sources: Vec<i32>,
    weights: Vec<f32>,
    indices: Vec<i32>,
    sizes: Vec<i32>,
    size: i32,
    last_offset: i32,
    coarse_vert_count: i32,
    compact: bool,
}

impl WeightTable {
    fn new(coarse: i32, gen_ctrl_vert_stencils: bool, compact: bool) -> WeightTable {
        let mut t = WeightTable {
            dests: Vec::new(),
            sources: Vec::new(),
            weights: Vec::new(),
            indices: Vec::new(),
            sizes: Vec::new(),
            size: 0,
            last_offset: 0,
            coarse_vert_count: coarse,
            compact,
        };
        if gen_ctrl_vert_stencils {
            for i in 0..coarse {
                t.indices.push(i);
                t.sizes.push(1);
                t.dests.push(i);
                t.sources.push(i);
                t.weights.push(1.0);
            }
            t.size = t.sources.len() as i32;
            t.last_offset = t.size - 1;
        }
        t
    }

    /// `WeightTable::AddWithWeight` (scalar accumulator).
    fn add_with_weight(&mut self, src: i32, dest: i32, weight: f32) {
        if src < self.coarse_vert_count {
            self.merge(src, dest, weight, 1.0, self.last_offset, self.size);
            return;
        }
        let len = self.sizes[src as usize];
        let start = self.indices[src as usize];
        for i in start..start + len {
            let s = self.sources[i as usize];
            let w = self.weights[i as usize];
            self.merge(s, dest, w, weight, self.last_offset, self.size);
        }
    }

    fn merge(&mut self, src: i32, dst: i32, weight: f32, factor: f32, last: i32, table_size: i32) {
        if self.compact && !self.dests.is_empty() && self.dests[last as usize] == dst {
            for i in last..table_size {
                if self.sources[i as usize] == src {
                    self.weights[i as usize] += weight * factor;
                    return;
                }
            }
        }
        self.add(src, dst, weight * factor);
    }

    fn add(&mut self, src: i32, dst: i32, weight: f32) {
        if self.dests.last() != Some(&dst) {
            if dst as usize + 1 > self.indices.len() {
                self.indices.resize(dst as usize + 1, 0);
                self.sizes.resize(dst as usize + 1, 0);
            }
            self.indices[dst as usize] = self.sources.len() as i32;
            self.sizes[dst as usize] = 0;
            self.last_offset = self.sources.len() as i32;
        }
        self.size += 1;
        self.sizes[dst as usize] += 1;
        self.dests.push(dst);
        self.sources.push(src);
        self.weights.push(weight);
    }

    fn num_verts_in_stencil(&self, i: usize) -> i32 {
        if self.sizes.is_empty() || i > self.sizes.len() - 1 {
            0
        } else {
            self.sizes[i]
        }
    }
}

impl FvarSink for WeightTable {
    fn add_with_weight(&mut self, dst: i32, src: i32, weight: f32) {
        WeightTable::add_with_weight(self, src, dst, weight);
    }
}

/// `StencilTableFactory::Create` with `INTERPOLATE_FACE_VARYING` for `channel`
/// (`generateOffsets`, `factorizeIntermediateLevels`, no control-vertex stencils).
pub(crate) fn create_fvar_table(
    refiner: &TopologyRefiner,
    channel: usize,
    intermediate_levels: bool,
) -> StencilTable {
    let num_control = refiner.levels[0].num_fvar_values(channel);
    let max_level = refiner.max_level();
    if max_level == 0 {
        return StencilTable {
            num_control,
            ..StencilTable::default()
        };
    }
    let mut builder = WeightTable::new(num_control, true, true);
    let mut src = 0i32;
    let mut dst = num_control;
    for level in 1..=max_level {
        frap_math::budget::check();
        primvar::interpolate_face_varying(refiner, level, src, dst, channel, &mut builder);
        src = dst;
        dst += refiner.levels[level].num_fvar_values(channel);
    }
    let first = if intermediate_levels {
        num_control as usize
    } else {
        src as usize
    };
    // StencilTableReal(numControlVerts, offsets, sizes, sources, weights, false, firstOffset)
    let mut t = StencilTable {
        num_control,
        ..StencilTable::default()
    };
    for i in first..builder.indices.len() {
        let sz = builder.sizes[i] as usize;
        let off = builder.indices[i] as usize;
        t.offsets.push(t.indices.len() as i32);
        t.sizes.push(sz as i32);
        t.indices.extend_from_slice(&builder.sources[off..off + sz]);
        t.weights.extend_from_slice(&builder.weights[off..off + sz]);
    }
    t
}

/// `StencilTableFactory::AppendLocalPointStencilTableFaceVarying` (factorised): `None` when
/// there are no local points.
pub(crate) fn append_local_point_table(
    refiner: &TopologyRefiner,
    base: &StencilTable,
    local: &StencilTable,
    channel: usize,
) -> Result<Option<StencilTable>, String> {
    if local.num_stencils() == 0 {
        return Ok(None);
    }
    let n_control = refiner.levels[0].num_fvar_values(channel);
    if base.num_stencils() == 0 {
        let mut t = local.clone();
        t.num_control = n_control;
        return Ok(Some(t));
    }
    let total: i32 = refiner
        .levels
        .iter()
        .map(|l| l.num_fvar_values(channel))
        .sum();
    let n_base = base.num_stencils() as i32;
    let offset = if n_base == total {
        0
    } else if n_base == total - n_control {
        n_control
    } else {
        return Err("local point stencils do not match the refined values".into());
    };
    let mut builder = WeightTable::new(n_control, false, true);
    for i in 0..local.num_stencils() {
        let (idx, w) = local.stencil(i);
        for j in 0..idx.len() {
            let weight = w[j];
            if weight == 0.0 {
                continue;
            }
            let k = idx[j] - offset;
            if k < 0 {
                return Err("local point stencil references a control value".into());
            }
            let (bidx, bw) = base.stencil(k as usize);
            for m in 0..bidx.len() {
                let bwm = bw[m];
                if bwm == 0.0 {
                    continue;
                }
                builder.add_with_weight(bidx[m], i as i32, weight * bwm);
            }
        }
    }
    let mut t = base.clone();
    for i in 0..local.num_stencils() {
        let size = builder.num_verts_in_stencil(i) as usize;
        let o = if size > 0 {
            builder.indices[i] as usize
        } else {
            0
        };
        t.indices.extend_from_slice(&builder.sources[o..o + size]);
        t.weights.extend_from_slice(&builder.weights[o..o + size]);
        t.sizes.push(size as i32);
    }
    t.generate_offsets();
    Ok(Some(t))
}
