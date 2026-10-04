//! Grip order is U-major; storage order is V-major. Seams alias unique grips.
use super::*;
use std::collections::BTreeSet;

impl NurbsSurface {
    pub fn with_transformed_grips(
        &self,
        indices: &BTreeSet<usize>,
        transform: AffineTransform3,
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
                    transform.transform_point(control.point())?
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
