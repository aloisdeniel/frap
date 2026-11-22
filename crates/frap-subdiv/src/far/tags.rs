// SPDX-License-Identifier: GPL-3.0-or-later
//! The bit-field tags of `Vtr::internal::Level` and `FVarLevel` (`VTag`, `ETag`, `ValueTag`,
//! `FVarLevel::ETag`) and `Sdc::Crease::Rule`, with OpenSubdiv's bit layout so composite tags
//! (bitwise OR over a face's corners) and feature masks combine exactly as in C++.
//!
//! Ported from OpenSubdiv 3.7.0 `opensubdiv/vtr/level.h`, `opensubdiv/vtr/fvarLevel.h` and
//! `opensubdiv/sdc/crease.h` (Pixar, TOST / Apache-2.0 modified, GPL-compatible).

/// `Sdc::Crease::RULE_UNKNOWN`
pub(crate) const RULE_UNKNOWN: u8 = 0;
/// `Sdc::Crease::RULE_SMOOTH`
pub(crate) const RULE_SMOOTH: u8 = 1;
/// `Sdc::Crease::RULE_DART`
pub(crate) const RULE_DART: u8 = 2;
/// `Sdc::Crease::RULE_CREASE`
pub(crate) const RULE_CREASE: u8 = 4;
/// `Sdc::Crease::RULE_CORNER`
pub(crate) const RULE_CORNER: u8 = 8;

macro_rules! bit {
    ($get:ident, $set:ident, $bit:expr) => {
        #[inline]
        #[allow(dead_code)]
        pub(crate) fn $get(self) -> bool {
            (self.0 >> $bit) & 1 != 0
        }
        #[inline]
        #[allow(dead_code)]
        pub(crate) fn $set(&mut self, on: bool) {
            if on {
                self.0 |= 1 << $bit;
            } else {
                self.0 &= !(1 << $bit);
            }
        }
    };
}

/// `Level::VTag` (16 bits, C++ declaration order from the least significant bit).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct VTag(pub(crate) u16);

impl VTag {
    bit!(non_manifold, set_non_manifold, 0);
    bit!(xordinary, set_xordinary, 1);
    bit!(boundary, set_boundary, 2);
    bit!(corner, set_corner, 3);
    bit!(inf_sharp, set_inf_sharp, 4);
    bit!(semi_sharp, set_semi_sharp, 5);
    bit!(semi_sharp_edges, set_semi_sharp_edges, 6);
    bit!(incomplete, set_incomplete, 11);
    bit!(incid_irreg_face, set_incid_irreg_face, 12);
    bit!(inf_sharp_edges, set_inf_sharp_edges, 13);
    bit!(inf_sharp_crease, set_inf_sharp_crease, 14);
    bit!(inf_irregular, set_inf_irregular, 15);

    /// `_rule` (4 bits at bit 7): one `RULE_*` value, or a union of them in a composite tag.
    #[inline]
    pub(crate) fn rule(self) -> u8 {
        ((self.0 >> 7) & 0xf) as u8
    }

    #[inline]
    pub(crate) fn set_rule(&mut self, rule: u8) {
        self.0 = (self.0 & !(0xf << 7)) | ((rule as u16 & 0xf) << 7);
    }

    /// `VTag::BitwiseOr`
    pub(crate) fn bitwise_or(tags: &[VTag]) -> VTag {
        VTag(tags.iter().fold(0, |a, t| a | t.0))
    }
}

/// `Level::ETag`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ETag(pub(crate) u8);

impl ETag {
    bit!(non_manifold, set_non_manifold, 0);
    bit!(boundary, set_boundary, 1);
    bit!(inf_sharp, set_inf_sharp, 2);
    bit!(semi_sharp, set_semi_sharp, 3);
}

/// `FVarLevel::ETag`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FvETag(pub(crate) u8);

impl FvETag {
    bit!(mismatch, set_mismatch, 0);
    bit!(discts_v0, set_discts_v0, 1);
    bit!(discts_v1, set_discts_v1, 2);
    bit!(linear, set_linear, 3);

    /// `FVarLevel::ETag::combineWithLevelETag`
    pub(crate) fn combine_with_level_etag(self, mut level: ETag) -> ETag {
        if self.mismatch() {
            level.set_boundary(true);
            level.set_inf_sharp(true);
        }
        level
    }
}

/// `FVarLevel::ValueTag`
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ValueTag(pub(crate) u8);

impl ValueTag {
    bit!(mismatch, set_mismatch, 0);
    bit!(xordinary, set_xordinary, 1);
    bit!(non_manifold, set_non_manifold, 2);
    bit!(crease, set_crease, 3);
    bit!(semi_sharp, set_semi_sharp, 4);
    bit!(dep_sharp, set_dep_sharp, 5);
    bit!(inf_sharp_edges, set_inf_sharp_edges, 6);
    bit!(inf_irregular, set_inf_irregular, 7);

    /// `isCorner`
    pub(crate) fn is_corner(self) -> bool {
        !self.crease()
    }

    /// `isInfSharp`
    pub(crate) fn is_inf_sharp(self) -> bool {
        !self.semi_sharp() && !self.crease()
    }

    /// `ValueTag::combineWithLevelVTag`
    pub(crate) fn combine_with_level_vtag(self, mut level: VTag) -> VTag {
        if self.mismatch() {
            if self.is_corner() {
                level.set_rule(RULE_CORNER);
            } else {
                level.set_rule(RULE_CREASE);
            }
            if self.crease() || self.semi_sharp() {
                level.set_inf_sharp(false);
                level.set_inf_sharp_crease(true);
                level.set_corner(false);
            } else {
                level.set_inf_sharp(true);
                level.set_inf_sharp_crease(false);
                level.set_corner(!self.inf_irregular() && !self.inf_sharp_edges());
            }
            level.set_inf_sharp_edges(true);
            level.set_inf_irregular(self.inf_irregular());
            level.set_boundary(true);
            level.set_xordinary(self.xordinary());
            let nm = level.non_manifold() | self.non_manifold();
            level.set_non_manifold(nm);
        }
        level
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_bits_round_trip_and_combine() {
        let mut t = VTag::default();
        t.set_rule(RULE_CREASE);
        t.set_boundary(true);
        assert_eq!(t.rule(), RULE_CREASE);
        assert!(t.boundary());
        let mut u = VTag::default();
        u.set_rule(RULE_SMOOTH);
        let c = VTag::bitwise_or(&[t, u]);
        assert_eq!(c.rule(), RULE_CREASE | RULE_SMOOTH);
        let mut v = ValueTag::default();
        v.set_mismatch(true);
        let w = v.combine_with_level_vtag(u);
        assert_eq!(w.rule(), RULE_CORNER);
        assert!(w.inf_sharp() && w.corner() && w.boundary());
    }
}
