//! Grip order is U-major; storage order is V-major. Seams alias unique grips.
use super::*;
use std::collections::BTreeSet;

impl NurbsSurface {
    /// Replace weights in the displayed U-major grip chart, updating every
    /// physical seam alias while keeping the complete tensor basis unchanged.
    pub fn with_grip_weight(
        &self,
        indices: &BTreeSet<usize>,
        weight: Real,
    ) -> Result<Self, GeometryError> {
        WeightedPoint3::try_new(self.control_points[0].point(), weight)?;
        let [u_count, v_count] = self.grip_dimensions();
        let count = u_count * v_count;
        if let Some(&index) = indices.last().filter(|&&i| i >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        if indices.iter().all(|&i| {
            self.control_points[self.control_index(i / v_count, i % v_count)].weight() == weight
        }) {
            return Ok(self.clone());
        }
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let u = i % self.control_point_count_u;
                let v = i / self.control_point_count_u;
                let grip = (u % u_count) * v_count + v % v_count;
                let source = self.control_points[self.control_index(u % u_count, v % v_count)];
                WeightedPoint3::try_new(
                    source.point(),
                    if indices.contains(&grip) {
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
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            controls,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )
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
        let [u_count, v_count] = self.grip_dimensions();
        let count = u_count * v_count;
        if let Some(&index) = indices.last().filter(|&&index| index >= count) {
            return Err(GeometryError::InvalidControlPointIndex { index, count });
        }
        let controls = self
            .control_points
            .iter()
            .enumerate()
            .map(|(index, control)| {
                let u = index % self.control_point_count_u;
                let v = index / self.control_point_count_u;
                let grip = (u % u_count) * v_count + v % v_count;
                let point = if indices.contains(&grip) {
                    map(control.point())?
                } else {
                    control.point()
                };
                WeightedPoint3::try_new(point, control.weight())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            controls,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )
    }
}
