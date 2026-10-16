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
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(index, control)| {
                let point = if indices.contains(&(index % count)) {
                    map(control.point())?
                } else {
                    control.point()
                };
                WeightedPoint3::try_new(point, control.weight())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(self.degree, controls, self.knots.clone())
    }
}
