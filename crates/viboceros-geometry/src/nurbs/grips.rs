//! Control edits keep knots, weights and repeated seam controls intact.
use super::*;
use std::collections::BTreeSet;

impl NurbsCurve {
    /// Full-multiplicity knots with a jump in the first parameter derivative.
    /// Uses the relative component test in OpenNURBS GetNextDiscontinuity.
    pub fn full_multiplicity_c1_break_parameters(&self) -> Result<Vec<Real>, GeometryError> {
        let mut result = Vec::new();
        for (k, multiplicity) in self.interior_knot_groups() {
            if multiplicity != self.degree {
                continue;
            }
            let a = self
                .evaluate_with_derivative_on_side(k, crate::ParameterSide::Left)?
                .1
                .to_array();
            let b = self
                .evaluate_with_derivative_on_side(k, crate::ParameterSide::Right)?
                .1
                .to_array();
            let left = a.into_iter().map(Real::abs).fold(0., Real::max);
            let scale = b.into_iter().map(Real::abs).fold(left, Real::max);
            if scale > 0.
                && a.into_iter().zip(b).any(|(a, b)| {
                    (a / scale - b / scale).abs() > (left / scale) * Real::EPSILON.sqrt()
                })
            {
                result.push(k);
            }
        }
        Ok(result)
    }

    /// Replace selected logical control weights while retaining their Euclidean
    /// locations, degree and knots. Closing and periodic aliases share a weight.
    pub fn with_grip_weight(
        &self,
        indices: &BTreeSet<usize>,
        weight: Real,
    ) -> Result<Self, GeometryError> {
        WeightedPoint3::try_new(self.control_points[0].point(), weight)?;
        let count = self.grip_count()?;
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices
            .iter()
            .all(|&i| self.control_points[i].weight() == weight)
        {
            return Ok(self.clone());
        }
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let source = self.control_points[i % count];
                WeightedPoint3::try_new(
                    source.point(),
                    if indices.contains(&(i % count)) {
                        weight
                    } else {
                        source.weight()
                    },
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut controls = controls;
        if controls.iter().all(|c| c.weight() == controls[0].weight()) {
            for c in &mut controls {
                *c = WeightedPoint3::try_new(c.point(), 1.)?;
            }
        }
        if controls == self.control_points {
            return Ok(self.clone());
        }
        Self::try_new_rational(self.degree, controls, self.knots.clone())
    }

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
