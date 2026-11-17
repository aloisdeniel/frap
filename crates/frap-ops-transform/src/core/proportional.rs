// SPDX-License-Identifier: GPL-3.0-or-later
//! Proportional editing falloff (`proportional_edit_falloff`, `proportional_size`). Port of
//! `calculatePropRatio` (`editors/transform/transform_generics.cc`, Blender 5.2.4,
//! GPL-2.0-or-later). Distances are computed by the mode handler (objects: distance to the
//! nearest selected center, each center mapped by its `td->mtx` as `set_prop_dist` does; edit
//! mode adds connectivity in M2).
//!
//! `RANDOM` multiplies by a random number: Blender seeds it from the clock, frap from a fixed
//! seed (deterministic output, rule §21.4).

use frap_math::BliRng;

/// `proportional_edit_falloff` (`PROP_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Falloff {
    /// `'SMOOTH'` (default)
    Smooth,
    /// `'SPHERE'`
    Sphere,
    /// `'ROOT'`
    Root,
    /// `'INVERSE_SQUARE'`
    InverseSquare,
    /// `'SHARP'`
    Sharp,
    /// `'LINEAR'`
    Linear,
    /// `'CONSTANT'`
    Constant,
    /// `'RANDOM'`
    Random,
}

impl Falloff {
    /// From a `proportional_edit_falloff` identifier (unknown → SMOOTH).
    pub fn from_identifier(s: &str) -> Falloff {
        match s {
            "SPHERE" => Falloff::Sphere,
            "ROOT" => Falloff::Root,
            "INVERSE_SQUARE" => Falloff::InverseSquare,
            "SHARP" => Falloff::Sharp,
            "LINEAR" => Falloff::Linear,
            "CONSTANT" => Falloff::Constant,
            "RANDOM" => Falloff::Random,
            _ => Falloff::Smooth,
        }
    }
}

/// Proportional-editing settings of one transform (`T_PROP_EDIT`).
#[derive(Debug)]
pub struct Proportional {
    /// Falloff curve.
    pub falloff: Falloff,
    /// `proportional_size` (`t->prop_size`).
    pub size: f32,
    rng: BliRng,
}

/// Fixed seed of the `RANDOM` falloff (Blender seeds from the time of day).
pub const RANDOM_SEED: u32 = 0;

impl Proportional {
    /// New settings.
    pub fn new(falloff: Falloff, size: f32) -> Proportional {
        Proportional {
            falloff,
            size,
            rng: BliRng::new(RANDOM_SEED),
        }
    }

    /// `calculatePropRatio` for one unselected element at distance `rdist` from the selection
    /// (selected elements have factor 1): 0 beyond `proportional_size`, else the falloff of
    /// `(size - rdist) / size`.
    pub fn factor(&mut self, rdist: f32) -> f32 {
        if rdist > self.size {
            return 0.0;
        }
        let dist = ((self.size - rdist) / self.size).max(0.0);
        match self.falloff {
            Falloff::Sharp => dist * dist,
            Falloff::Smooth => 3.0 * dist * dist - 2.0 * dist * dist * dist,
            Falloff::Root => dist.sqrt(),
            Falloff::Linear => dist,
            Falloff::Constant => 1.0,
            Falloff::Sphere => (2.0 * dist - dist * dist).sqrt(),
            Falloff::Random => self.rng.get_float() * dist,
            Falloff::InverseSquare => dist * (2.0 - dist),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falloffs() {
        // Linear, size 8: an object 7.2445 away moves by 0.0944 of the selection (verified with
        // the factory Light on 5.2.4).
        let mut p = Proportional::new(Falloff::Linear, 8.0);
        assert!((p.factor(7.2445) - 0.0944).abs() < 1e-4);
        assert_eq!(p.factor(8.5), 0.0);
        let mut p = Proportional::new(Falloff::Smooth, 1.0);
        assert_eq!(p.factor(0.0), 1.0);
        assert!((p.factor(0.5) - 0.5).abs() < 1e-6);
        let mut c = Proportional::new(Falloff::Constant, 2.0);
        assert_eq!(c.factor(1.9), 1.0);
        let mut s = Proportional::new(Falloff::Sphere, 1.0);
        assert!((s.factor(0.5) - 0.75f32.sqrt()).abs() < 1e-6);
        let mut i = Proportional::new(Falloff::InverseSquare, 1.0);
        assert!((i.factor(0.5) - 0.75).abs() < 1e-6);
        let mut r = Proportional::new(Falloff::Random, 1.0);
        let (a, b) = (
            r.factor(0.5),
            Proportional::new(Falloff::Random, 1.0).factor(0.5),
        );
        assert_eq!(a, b, "deterministic");
        assert!((0.0..=0.5).contains(&a));
        assert_eq!(Falloff::from_identifier("ROOT"), Falloff::Root);
        assert_eq!(Falloff::from_identifier("SHARP"), Falloff::Sharp);
    }
}
