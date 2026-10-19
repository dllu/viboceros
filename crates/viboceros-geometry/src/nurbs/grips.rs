//! Control edits keep knots, weights and repeated seam controls intact.
use super::*;
use std::collections::BTreeSet;

impl NurbsCurve {
    pub fn with_transformed_grips(
        &self,
        indices: &BTreeSet<usize>,
        transform: AffineTransform3,
    ) -> Result<Self, GeometryError> {
        self.with_mapped_grips(indices, |point| transform.transform_point(point))
    }

    /// Map selected Euclidean controls, retaining weights, knots and seam aliases.
    pub fn with_mapped_grips(
        &self,
        indices: &BTreeSet<usize>,
        map: impl Fn(Point3) -> Result<Point3, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let count = self.grip_count()?;
        if let Some(&index) = indices.last().filter(|&&index| index >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        let mapped = self.control_points[..count]
            .iter()
            .enumerate()
            .map(|(i, c)| {
                if indices.contains(&i) {
                    map(c.point())
                } else {
                    Ok(c.point())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        if mapped
            .iter()
            .zip(&self.control_points)
            .all(|(p, c)| *p == c.point())
        {
            return Ok(self.clone());
        }
        // A grip edit rebuilds all aliases from the shared displayed grips.
        // This closes a numerically coincident seam exactly, including when
        // a different grip moves. A no-op above leaves the source untouched.
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(i, c)| WeightedPoint3::try_new(mapped[i % count], c.weight()))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(self.degree, controls, self.knots.clone())
            .map(|curve| curve.preserving_representation(self))
    }
}
