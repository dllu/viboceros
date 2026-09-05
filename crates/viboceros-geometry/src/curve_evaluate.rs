//! Shared native curve domains and allocation-free analytic evaluation.

use crate::parameter::{map_parameter, scaled_ratio};
use crate::{CurveEvaluationSide, CurveRef, GeometryError, Point3, Real, Vector3};
use std::f64::consts::{FRAC_1_SQRT_2, TAU};
use std::ops::RangeInclusive;

#[cfg(test)]
mod tests;

impl CurveRef<'_> {
    /// The native parameter interval, independent of the current curve length.
    pub fn domain(self) -> RangeInclusive<Real> {
        match self {
            Self::Line(c) => c.domain(),
            Self::Circle(c) => c.domain(),
            Self::Arc(c) => c.domain(),
            Self::Ellipse(c) => c.domain(),
            Self::Polyline(c) => c.domain(),
            Self::NurbsCurve(c) => c.domain(),
            Self::PolyCurve(c) => c.domain(),
        }
    }

    /// Maps a checked normalized coordinate in `[0,1]` to the native interval.
    pub fn parameter_at(self, normalized: Real) -> Result<Real, GeometryError> {
        if let Self::NurbsCurve(curve) = self {
            return curve.parameter_at(normalized);
        }
        map_parameter(normalized, 0.0..=1.0, self.domain())
    }

    /// Evaluates a checked native parameter. This does not extrapolate.
    pub fn evaluate(self, parameter: Real) -> Result<Point3, GeometryError> {
        match self {
            Self::Line(c) => c.evaluate(parameter),
            Self::Circle(c) => c.evaluate(parameter),
            Self::Arc(c) => c.evaluate(parameter),
            Self::Ellipse(c) => c.evaluate(parameter),
            Self::Polyline(c) => c.evaluate(parameter),
            Self::NurbsCurve(c) => c.evaluate(parameter),
            Self::PolyCurve(c) => c.evaluate(parameter),
        }
    }

    /// Point and first derivative with respect to the native parameter.
    /// A second derivative need not be representable for this call to succeed.
    pub fn evaluate_with_derivative(
        self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        match self {
            Self::NurbsCurve(c) => return c.evaluate_with_derivative(parameter),
            Self::PolyCurve(c) => return c.evaluate_with_derivative(parameter),
            _ => {}
        }
        Ok((
            self.evaluate(parameter)?,
            self.analytic_derivative(parameter, 1)?,
        ))
    }

    /// Point and first/second native derivatives. At an interior junction the
    /// right-hand segment is active; the final endpoint uses the last segment.
    pub fn evaluate_with_second_derivative(
        self,
        parameter: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        match self {
            Self::NurbsCurve(c) => return c.evaluate_with_second_derivative(parameter),
            Self::PolyCurve(c) => {
                return c.evaluate_with_second_derivative(parameter, CurveEvaluationSide::Right);
            }
            _ => {}
        }
        Ok((
            self.evaluate(parameter)?,
            self.analytic_derivative(parameter, 1)?,
            self.analytic_derivative(parameter, 2)?,
        ))
    }
    fn analytic_derivative(self, parameter: Real, order: usize) -> Result<Vector3, GeometryError> {
        let domain = self.domain();
        let width = domain.end() - domain.start();
        let zero = Vector3::try_new(0.0, 0.0, 0.0)?;
        match self {
            Self::Line(c) => {
                if order == 1 {
                    scale(c.start().vector_to(c.end())?, 1.0, width)
                } else {
                    Ok(zero)
                }
            }
            Self::Polyline(c) => {
                let (i, _) = c.parameter_location(parameter)?;
                if order == 1 {
                    scale(
                        c.vertices()[i].vector_to(c.vertices()[i + 1])?,
                        1.0,
                        c.parameters()[i + 1] - c.parameters()[i],
                    )
                } else {
                    Ok(zero)
                }
            }
            Self::Ellipse(c) => {
                let jet = ellipse_unit_jet(parameter, domain)?;
                let mut vector = combine(
                    c.x_axis().as_vector(),
                    c.y_axis().as_vector(),
                    c.radius_x() * jet[order][0],
                    c.radius_y() * jet[order][1],
                )?;
                for _ in 0..order {
                    vector = scale(vector, 4.0, width)?;
                }
                Ok(vector)
            }
            Self::Circle(c) => angular_derivative(
                c.x_axis().as_vector(),
                c.y_axis().as_vector(),
                c.radius(),
                TAU,
                domain,
                parameter,
                order,
            ),
            Self::Arc(c) => angular_derivative(
                c.x_axis().as_vector(),
                c.y_axis().as_vector(),
                c.radius(),
                c.sweep_radians(),
                domain,
                parameter,
                order,
            ),
            _ => unreachable!("rational/composite derivatives dispatch before analytic evaluation"),
        }
    }
}

fn angular_derivative(
    x: Vector3,
    y: Vector3,
    radius: Real,
    sweep: Real,
    domain: RangeInclusive<Real>,
    parameter: Real,
    order: usize,
) -> Result<Vector3, GeometryError> {
    let angle = map_parameter(parameter, domain.clone(), 0.0..=sweep)?;
    let (sine, cosine) = angle.sin_cos();
    let mut vector = if order == 1 {
        combine(x, y, -sine, cosine)?
    } else {
        combine(x, y, -cosine, -sine)?
    }
    .scaled(radius)?;
    for _ in 0..order {
        vector = scale(vector, sweep, domain.end() - domain.start())?;
    }
    Ok(vector)
}

/// Unit-circle rational quadratic values and derivatives with respect to the
/// local quarter parameter. Exact knots choose the next quarter, except the
/// final domain endpoint, which retains the last quarter's one-sided jet.
pub(crate) fn ellipse_unit_jet(
    parameter: Real,
    domain: RangeInclusive<Real>,
) -> Result<[[Real; 2]; 3], GeometryError> {
    let t = map_parameter(parameter, domain, 0.0..=4.0)?;
    let quadrant = (t.floor() as usize).min(3);
    let u = t - quadrant as Real;
    let v = 1.0 - u;
    let w = FRAC_1_SQRT_2;
    let cross = 2.0 * w * u * v;
    let d = v * v + cross + u * u;
    let d1 = 2.0 * (1.0 - w) * (2.0 * u - 1.0);
    let d2 = 4.0 * (1.0 - w);
    let mut jet = [[0.0; 2]; 3];
    for (i, (n, n1)) in [
        (v * v + cross, -2.0 * v + 2.0 * w * (1.0 - 2.0 * u)),
        (u * u + cross, 2.0 * u + 2.0 * w * (1.0 - 2.0 * u)),
    ]
    .into_iter()
    .enumerate()
    {
        jet[0][i] = n / d;
        jet[1][i] = (n1 - jet[0][i] * d1) / d;
        jet[2][i] = (2.0 - 4.0 * w - 2.0 * jet[1][i] * d1 - jet[0][i] * d2) / d;
    }
    Ok(jet.map(|[x, y]| match quadrant {
        0 => [x, y],
        1 => [-y, x],
        2 => [-x, -y],
        _ => [y, -x],
    }))
}

pub(crate) fn scale(
    vector: Vector3,
    numerator: Real,
    denominator: Real,
) -> Result<Vector3, GeometryError> {
    let c = vector.to_array();
    Vector3::try_new(
        scaled_ratio(c[0], numerator, denominator)?,
        scaled_ratio(c[1], numerator, denominator)?,
        scaled_ratio(c[2], numerator, denominator)?,
    )
}
fn combine(x: Vector3, y: Vector3, a: Real, b: Real) -> Result<Vector3, GeometryError> {
    Vector3::try_new(
        x.x().mul_add(a, y.x() * b),
        x.y().mul_add(a, y.y() * b),
        x.z().mul_add(a, y.z() * b),
    )
}
