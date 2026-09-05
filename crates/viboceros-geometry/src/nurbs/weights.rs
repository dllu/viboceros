//! Homogeneous rescaling and projective endpoint-weight normalization.

mod end_weights;
pub(super) use end_weights::change_bezier_end_weights;

use super::{GeometryError, NurbsCurve, Point3, Real, WeightedPoint3, finite_midpoint};

#[cfg(test)]
mod tests;

pub(crate) fn rescale_controls(
    controls: &[WeightedPoint3],
    from: Real,
    to: Real,
) -> Result<Vec<WeightedPoint3>, GeometryError> {
    controls
        .iter()
        .map(|control| {
            let weight = if control.weight == from {
                to
            } else {
                crate::parameter::scaled_ratio(control.weight, to, from)?
            };
            WeightedPoint3::try_new(control.point, weight)
        })
        .collect()
}

impl NurbsCurve {
    pub(super) fn try_append_clamped(&self, next: &Self) -> Result<Self, GeometryError> {
        let a = self.control_points[self.control_points.len() - 1]
            .point
            .to_array();
        let b = next.control_points[0].point.to_array();
        let join = Point3::try_from(std::array::from_fn(|i| finite_midpoint(a[i], b[i])))?;
        self.try_append_clamped_at_join(next, join)
    }

    pub(super) fn try_append_clamped_at_join(
        &self,
        next: &Self,
        join: Point3,
    ) -> Result<Self, GeometryError> {
        if self.degree != next.degree {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let degree = self.degree;
        let left_end = *self.domain().end();
        let right_start = *next.domain().start();
        if left_end != right_start
            || !self.knots[self.knots.len() - degree - 1..]
                .iter()
                .all(|knot| *knot == left_end)
            || !next.knots[..=degree]
                .iter()
                .all(|knot| *knot == right_start)
        {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        let left = self.control_points[self.control_points.len() - 1];
        let right = next.control_points[0];
        let scaled = rescale_controls(&next.control_points, right.weight, left.weight).ok();
        let shared = scaled.is_some();
        let next_controls = scaled.as_deref().unwrap_or(&next.control_points);
        let output_count = self
            .control_points
            .len()
            .checked_add(next_controls.len())
            .and_then(|n| n.checked_sub(usize::from(shared)))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "appended control count overflowed usize",
            })?;
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(output_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "appended control count exceeds addressable memory",
            })?;
        controls.extend_from_slice(&self.control_points);
        let knot_count = output_count
            .checked_add(degree)
            .and_then(|n| n.checked_add(1))
            .ok_or(GeometryError::InvalidKnotVector {
                context: "appended knot count overflowed usize",
            })?;
        let mut knots = Vec::new();
        knots
            .try_reserve_exact(knot_count)
            .map_err(|_| GeometryError::InvalidKnotVector {
                context: "appended knot count exceeds addressable memory",
            })?;
        knots.extend_from_slice(&self.knots);
        if shared {
            *controls.last_mut().expect("NURBS controls exist") =
                WeightedPoint3::try_new(join, left.weight)?;
            knots.pop();
        }
        // If a common scale would overflow or erase any weight, retain both
        // seam controls and a full-order knot. Each side keeps its exact locus.
        controls.extend_from_slice(&next_controls[usize::from(shared)..]);
        knots.extend_from_slice(&next.knots[degree + 1..]);
        debug_assert_eq!(knots.len(), knot_count);
        Self::try_new_rational(degree, controls, knots)
    }
}
