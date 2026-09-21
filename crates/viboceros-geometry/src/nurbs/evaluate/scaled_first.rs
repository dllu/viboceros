//! First derivatives in a fractional interval, before native speed can overflow.
use super::*;

impl NurbsCurve {
    pub(in crate::nurbs) fn scaled_first_jet(
        &self,
        span: usize,
        parameter: Real,
        width: Real,
        exact: impl Fn() -> Result<(Point3, Vector3), GeometryError>,
    ) -> Result<(Point3, Vector3), GeometryError> {
        self.with_evaluation_controls(
            span,
            |origin, controls| {
                let h = de_boor(&self.knots, self.degree, span, parameter, controls.clone())?;
                let local = project_homogeneous(h)?;
                let mut first = Vec::with_capacity(self.degree);
                let mut first_bounds = [0_f64; 4];
                for i in 0..self.degree {
                    let index = span - self.degree + i;
                    let denominator = self.knots[index + self.degree + 1] - self.knots[index + 1];
                    if !denominator.is_normal() || denominator <= 0. {
                        return Err(range_loss());
                    }
                    let mut control = [0.; 4];
                    for axis in 0..4 {
                        let delta = controls[i + 1][axis] - controls[i][axis];
                        if delta == 0. {
                            continue;
                        }
                        if !delta.is_normal() {
                            return Err(range_loss());
                        }
                        let ratio = crate::parameter::scaled_ratio(delta, width, denominator)?;
                        let value = ratio * self.degree as Real;
                        // Do not magnify an underflowed intermediate later.
                        if !ratio.is_normal() || !value.is_normal() {
                            return Err(range_loss());
                        }
                        control[axis] = value;
                        first_bounds[axis] = first_bounds[axis].max(value.abs());
                    }
                    first.push(control);
                }
                let h1 = de_boor(
                    &self.knots[1..self.knots.len() - 1],
                    self.degree - 1,
                    span - 1,
                    parameter,
                    first,
                )?;
                let mut derivative = [0.; 3];
                for (axis, coordinate) in local.to_array().into_iter().enumerate() {
                    let numerator = (-coordinate).mul_add(h1[3], h1[axis]);
                    let envelope = first_bounds[axis] + coordinate.abs() * first_bounds[3];
                    let cancellation_guard =
                        64. * Real::EPSILON * (self.degree as Real).powi(2) * envelope;
                    // Homogeneous derivative blends may already have lost a
                    // small residual before the quotient rule sees it. Include
                    // their control envelope, not only the rounded h1 terms.
                    if envelope > 0. && numerator.abs() <= cancellation_guard {
                        return Err(range_loss());
                    }
                    // An apparently stationary coordinate after cancellation
                    // needs exact recovery, as does any later range loss.
                    if !numerator.is_normal()
                        && (h1[axis] != 0. || (coordinate != 0. && h1[3] != 0.))
                    {
                        return Err(range_loss());
                    }
                    let value = numerator / h[3];
                    if numerator != 0. && !value.is_normal() {
                        return Err(range_loss());
                    }
                    derivative[axis] = value;
                }
                Ok((
                    self.restore_evaluated_point(span, parameter, local, origin)?,
                    Vector3::try_from(derivative)?,
                ))
            },
            exact,
        )
    }
}

fn range_loss() -> GeometryError {
    GeometryError::NonFinite {
        context: "fractional NURBS derivative intermediate",
    }
}
