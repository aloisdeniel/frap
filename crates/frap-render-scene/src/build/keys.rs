// SPDX-License-Identifier: GPL-3.0-or-later
//! Cache keys of the render scene: a streaming 64-bit hash (MurmurHash64A mixing, fixed seed,
//! little-endian words) so keys are identical on every OS and every run.
//!
//! - [`MeshKey`]: a 64-bit content hash of the extracted [`RMesh`] (an evaluated fingerprint:
//!   equal data always gives equal keys; different data gives different keys except for a
//!   hash collision, which is improbable (about 2^-64 per pair) but not impossible, so a
//!   `GpuCache` hit is "very likely the same mesh", not a proof of it).
//! - [`MatKey`] / [`ImageKey`]: `(IdKey, generation)` of the datablock (every mutation, including
//!   one of an embedded node tree, bumps the owner's generation, §5.3).

use frap_rna::IdHeader;

use crate::types::{ImageKey, MatKey, MeshKey, RMesh};

const M: u64 = 0xc6a4_a793_5bd1_e995;
const R: u32 = 47;

/// Streaming MurmurHash64A-style hasher over `u64` words.
#[derive(Clone, Debug)]
pub struct KeyHasher {
    h: u64,
    words: u64,
}

impl KeyHasher {
    /// A hasher for one key domain (`tag` separates mesh, material and image keys).
    pub fn new(tag: &[u8; 2]) -> KeyHasher {
        let mut h = KeyHasher {
            h: 0x9e37_79b9_7f4a_7c15,
            words: 0,
        };
        h.u64(u64::from(u16::from_le_bytes(*tag)));
        h
    }
    /// Mixes one word.
    pub fn u64(&mut self, k: u64) {
        let mut k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);
        self.h ^= k;
        self.h = self.h.wrapping_mul(M);
        self.words += 1;
    }
    /// Mixes 32-bit words, two per `u64`, preceded by their count.
    pub fn u32s(&mut self, v: impl ExactSizeIterator<Item = u32>) {
        self.u64(v.len() as u64);
        let mut pending: Option<u32> = None;
        for w in v {
            match pending.take() {
                None => pending = Some(w),
                Some(lo) => self.u64(u64::from(lo) | (u64::from(w) << 32)),
            }
        }
        if let Some(lo) = pending {
            self.u64(u64::from(lo));
        }
    }
    /// Mixes floats by their bit patterns.
    pub fn f32s(&mut self, v: &[f32]) {
        self.u32s(v.iter().map(|x| x.to_bits()));
    }
    /// Final avalanche.
    pub fn finish(&self) -> u64 {
        let mut h = self.h ^ self.words.wrapping_mul(M);
        h ^= h >> R;
        h = h.wrapping_mul(M);
        h ^= h >> R;
        h
    }
}

/// Content key of an extracted mesh.
pub fn mesh_key(m: &RMesh) -> MeshKey {
    let mut h = KeyHasher::new(b"ME");
    h.f32s(m.positions.as_flattened());
    h.f32s(m.normals.as_flattened());
    h.u64(m.uvs.len() as u64);
    for uv in &m.uvs {
        h.f32s(uv.as_flattened());
    }
    h.u64(m.colors.len() as u64);
    for c in &m.colors {
        h.f32s(c.as_flattened());
    }
    match &m.tangents {
        None => h.u64(0),
        Some(t) => {
            h.u64(1);
            h.f32s(t.as_flattened());
        }
    }
    h.u32s(m.indices.iter().copied());
    h.u64(m.submeshes.len() as u64);
    for (slot, r) in &m.submeshes {
        h.u64(u64::from(*slot));
        h.u64(u64::from(r.start) | (u64::from(r.end) << 32));
    }
    h.u32s(m.edges.as_flattened().iter().copied());
    MeshKey(h.finish())
}

fn id_key(tag: &[u8; 2], key: frap_rna::IdKey, header: &IdHeader) -> u64 {
    let mut h = KeyHasher::new(tag);
    h.u64(key.get());
    h.u64(header.generation);
    h.finish()
}

/// Key of a material datablock.
pub fn mat_key(key: frap_rna::IdKey, header: &IdHeader) -> MatKey {
    MatKey(id_key(b"MA", key, header))
}

/// Key of an image datablock.
pub fn image_key(key: frap_rna::IdKey, header: &IdHeader) -> ImageKey {
    ImageKey(id_key(b"IM", key, header))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_element_changes_the_key() {
        let mut m = RMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
            submeshes: vec![(0, 0..3)],
            ..Default::default()
        };
        let k = mesh_key(&m);
        assert_eq!(k, mesh_key(&m.clone()));
        m.positions[2][1] = 1.0 + f32::EPSILON;
        assert_ne!(k, mesh_key(&m));
        m.positions[2][1] = 1.0;
        assert_eq!(k, mesh_key(&m));
        m.submeshes = vec![(1, 0..3)];
        assert_ne!(k, mesh_key(&m));
        m.submeshes = vec![(0, 0..3)];
        // Moving a value between layers changes the key (lengths are hashed).
        m.uvs = vec![vec![], vec![[0.5, 0.5]]];
        let a = mesh_key(&m);
        m.uvs = vec![vec![[0.5, 0.5]], vec![]];
        assert_ne!(a, mesh_key(&m));
    }

    #[test]
    fn equal_input_equal_key_and_domains_differ() {
        let mut h = KeyHasher::new(b"ME");
        h.f32s(&[1.0, 2.0, 3.0]);
        let a = h.finish();
        let mut h2 = KeyHasher::new(b"ME");
        h2.f32s(&[1.0, 2.0, 3.0]);
        assert_eq!(a, h2.finish());
        let mut h3 = KeyHasher::new(b"MA");
        h3.f32s(&[1.0, 2.0, 3.0]);
        assert_ne!(a, h3.finish());
    }
}
