// SPDX-License-Identifier: GPL-3.0-or-later
//! `BliRng`: port of Blender's `BLI_rng` (drand48-style LCG) for `select_random`,
//! `vertex_random`, `randomize_transform` and subdivide fractal (§21.4). WP 1.2 makes it
//! reproduce a Blender-dumped sequence (`tools/blender/dump_rng.py`).

/// Blender's `RNG` (48-bit LCG).
#[derive(Clone, Debug)]
pub struct BliRng {
    /// 48-bit state.
    pub x: u64,
}

impl BliRng {
    /// `BLI_rng_new(seed)`.
    pub fn new(seed: u32) -> BliRng {
        let _ = seed;
        todo!("WP 1.2: port BLI_rng_seed")
    }
}
