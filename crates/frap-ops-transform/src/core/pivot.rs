// SPDX-License-Identifier: GPL-3.0-or-later
//! Pivot points (`tool_settings.transform_pivot_point`, `center_override`). Ports of
//! `calculateCenter`, `calculateCenterMedian`, `calculateCenterBound` and
//! `calculateCenterCursor` (`editors/transform/transform_generics.cc`), Blender 5.2.4,
//! GPL-2.0-or-later. The active element's center is mode-specific: the handler passes it.

use glam::Vec3;

/// `transform_pivot_point` (`V3D_AROUND_*`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PivotPoint {
    /// `'BOUNDING_BOX_CENTER'`
    BoundingBoxCenter,
    /// `'CURSOR'`
    Cursor,
    /// `'INDIVIDUAL_ORIGINS'`
    IndividualOrigins,
    /// `'MEDIAN_POINT'` (factory default)
    MedianPoint,
    /// `'ACTIVE_ELEMENT'`
    ActiveElement,
}

impl PivotPoint {
    /// From a `transform_pivot_point` identifier (unknown → MEDIAN_POINT).
    pub fn from_identifier(s: &str) -> PivotPoint {
        match s {
            "BOUNDING_BOX_CENTER" => PivotPoint::BoundingBoxCenter,
            "CURSOR" => PivotPoint::Cursor,
            "INDIVIDUAL_ORIGINS" => PivotPoint::IndividualOrigins,
            "ACTIVE_ELEMENT" => PivotPoint::ActiveElement,
            _ => PivotPoint::MedianPoint,
        }
    }
    /// `transdata_check_local_center`: each element transforms about its own center.
    pub fn is_local(self) -> bool {
        self == PivotPoint::IndividualOrigins
    }
}

/// `calculateCenterMedian`: the mean of the centers (`None` for no center).
pub fn median(centers: &[Vec3]) -> Option<Vec3> {
    if centers.is_empty() {
        return None;
    }
    let mut partial = Vec3::ZERO;
    for c in centers {
        partial += *c;
    }
    Some(partial * (1.0 / centers.len() as f32))
}

/// `calculateCenterBound`: the middle of the bounding box of the centers.
pub fn bounds_center(centers: &[Vec3]) -> Option<Vec3> {
    let first = *centers.first()?;
    let (mut min, mut max) = (first, first);
    for c in &centers[1..] {
        min = min.min(*c);
        max = max.max(*c);
    }
    Some((min + max) * 0.5)
}

/// `calculateCenter`: the transform center (`t->center_global`). `centers` are the centers of
/// the selected elements that count (`TD_SELECTED` without `TD_NOCENTER`); `active` is the
/// active element's center, if any (`calculateCenterActive`, falls back to the median).
/// `center_override` wins when set.
pub fn center(
    pivot: PivotPoint,
    centers: &[Vec3],
    cursor: Vec3,
    active: Option<Vec3>,
    center_override: Option<Vec3>,
) -> Vec3 {
    if let Some(c) = center_override {
        return c;
    }
    let med = || median(centers).unwrap_or(Vec3::ZERO);
    match pivot {
        PivotPoint::BoundingBoxCenter => bounds_center(centers).unwrap_or(Vec3::ZERO),
        PivotPoint::Cursor => cursor,
        PivotPoint::ActiveElement => active.unwrap_or_else(med),
        PivotPoint::MedianPoint | PivotPoint::IndividualOrigins => med(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centers() {
        let c = [
            Vec3::ZERO,
            Vec3::new(4.0, 2.0, 0.0),
            Vec3::new(1.0, 0.0, 6.0),
        ];
        assert!(
            median(&c)
                .unwrap()
                .abs_diff_eq(Vec3::new(5.0 / 3.0, 2.0 / 3.0, 2.0), 1e-6)
        );
        assert_eq!(bounds_center(&c), Some(Vec3::new(2.0, 1.0, 3.0)));
        assert_eq!(median(&[]), None);
        let cur = Vec3::new(1.0, 1.0, 1.0);
        assert_eq!(center(PivotPoint::Cursor, &c, cur, None, None), cur);
        assert_eq!(
            center(PivotPoint::ActiveElement, &c, cur, None, None),
            median(&c).unwrap()
        );
        assert_eq!(
            center(PivotPoint::ActiveElement, &c, cur, Some(Vec3::X), None),
            Vec3::X
        );
        assert_eq!(
            center(PivotPoint::MedianPoint, &c, cur, None, Some(Vec3::Y)),
            Vec3::Y
        );
        assert_eq!(
            PivotPoint::from_identifier("BOGUS"),
            PivotPoint::MedianPoint
        );
    }
}
