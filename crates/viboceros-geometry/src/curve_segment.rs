//! Non-composite native curve segments and their parameterized operations.

use crate::parameter::{check_interval, checked_parameter, map_parameter};
use crate::{
    AffineTransform3, BoundingBox3, CircularArc3, Curve3, CurveRef, GeometryError, LineSegment,
    NurbsCurve, Point3, Polyline3, Real, Tolerance, Vector3,
};
use std::ops::RangeInclusive;

#[cfg(test)]
mod tests;

/// A native leaf in a flat polycurve. Analytic arcs retain angular speed;
/// converting them to rational NURBS preserves the locus but not that speed.
#[derive(Clone, Debug, PartialEq)]
pub enum CurveSegment3 {
    Line(LineSegment),
    Arc(CircularArc3),
    Polyline(Polyline3),
    NurbsCurve(NurbsCurve),
}

impl From<NurbsCurve> for CurveSegment3 {
    fn from(curve: NurbsCurve) -> Self {
        Self::NurbsCurve(curve)
    }
}
impl From<LineSegment> for CurveSegment3 {
    fn from(curve: LineSegment) -> Self {
        Self::Line(curve)
    }
}
impl From<CircularArc3> for CurveSegment3 {
    fn from(curve: CircularArc3) -> Self {
        Self::Arc(curve)
    }
}
impl From<Polyline3> for CurveSegment3 {
    fn from(curve: Polyline3) -> Self {
        Self::Polyline(curve)
    }
}

impl CurveSegment3 {
    pub fn try_from_curve(curve: &Curve3) -> Result<Self, GeometryError> {
        Ok(match curve {
            Curve3::Line(curve) => Self::Line(*curve),
            Curve3::Arc(curve) => Self::Arc(*curve),
            Curve3::Circle(curve) => Self::Arc(
                CircularArc3::try_from_circle_sweep(*curve, std::f64::consts::TAU)?
                    .try_reparameterized(curve.domain())?,
            ),
            Curve3::Polyline(curve) => Self::Polyline(curve.clone()),
            Curve3::NurbsCurve(curve) => Self::NurbsCurve(curve.clone()),
            Curve3::Ellipse(_) => Self::NurbsCurve(curve.as_ref().to_nurbs()?),
            Curve3::PolyCurve(_) => {
                return Err(GeometryError::InvalidPolyCurve {
                    context: "nested composites must be flattened",
                });
            }
        })
    }

    pub fn as_ref(&self) -> CurveRef<'_> {
        match self {
            Self::Line(c) => CurveRef::Line(c),
            Self::Arc(c) => CurveRef::Arc(c),
            Self::Polyline(c) => CurveRef::Polyline(c),
            Self::NurbsCurve(c) => CurveRef::NurbsCurve(c),
        }
    }

    pub fn into_curve(self) -> Curve3 {
        match self {
            Self::Line(c) => Curve3::Line(c),
            Self::Arc(c) => Curve3::Arc(c),
            Self::Polyline(c) => Curve3::Polyline(c),
            Self::NurbsCurve(c) => Curve3::NurbsCurve(c),
        }
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        match self {
            Self::Line(c) => c.domain(),
            Self::Arc(c) => c.domain(),
            Self::Polyline(c) => c.domain(),
            Self::NurbsCurve(c) => c.domain(),
        }
    }

    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        self.as_ref().parameter_at(normalized)
    }

    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        match self {
            Self::Line(c) => c.evaluate(parameter),
            Self::Arc(c) => c.evaluate(parameter),
            Self::Polyline(c) => c.evaluate(parameter),
            Self::NurbsCurve(c) => c.evaluate(parameter),
        }
    }

    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        self.as_ref().evaluate_with_derivative(parameter)
    }

    pub fn derivative_at(&self, parameter: Real) -> Result<Vector3, GeometryError> {
        Ok(self.evaluate_with_derivative(parameter)?.1)
    }

    pub fn evaluate_with_second_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        self.as_ref().evaluate_with_second_derivative(parameter)
    }

    pub fn spans(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        let mut index = match self {
            Self::NurbsCurve(curve) => curve.degree(),
            _ => 0,
        };
        std::iter::from_fn(move || match self {
            Self::Line(_) | Self::Arc(_) => {
                if index > 0 {
                    None
                } else {
                    index += 1;
                    Some((*self.domain().start(), *self.domain().end()))
                }
            }
            Self::Polyline(curve) => {
                let pair = curve.parameters().get(index..index + 2)?;
                index += 1;
                Some((pair[0], pair[1]))
            }
            Self::NurbsCurve(curve) => {
                while index < curve.control_points().len() {
                    let a = curve.knots()[index];
                    let b = curve.knots()[index + 1];
                    index += 1;
                    if a < b {
                        return Some((a, b));
                    }
                }
                None
            }
        })
    }

    pub fn degree(&self) -> usize {
        match self {
            Self::Line(_) | Self::Polyline(_) => 1,
            Self::Arc(_) => 2,
            Self::NurbsCurve(c) => c.degree(),
        }
    }
    pub fn is_closed(&self) -> Result<bool, GeometryError> {
        self.as_ref().is_closed()
    }
    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        self.as_ref().length(tolerance)
    }
    pub fn to_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        self.as_ref().to_nurbs()
    }

    pub fn control_point_bounds(&self) -> BoundingBox3 {
        match self {
            Self::Line(c) => {
                BoundingBox3::from_points([c.start(), c.end()]).expect("validated line")
            }
            Self::Arc(c) => c.bounds(),
            Self::Polyline(c) => c.bounds(),
            Self::NurbsCurve(c) => c.control_point_bounds(),
        }
    }

    pub fn extract_point_locations(&self) -> Result<Vec<Point3>, GeometryError> {
        match self {
            Self::Line(curve) => Ok(vec![curve.start(), curve.end()]),
            Self::Polyline(curve) => {
                let mut points = curve.vertices().to_vec();
                if curve.is_closed() {
                    points.pop();
                }
                Ok(points)
            }
            _ => self.to_nurbs()?.extract_point_locations(),
        }
    }

    pub fn control_polygon(&self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        match self {
            Self::Polyline(curve) => Ok(curve.clone()),
            _ => self.to_nurbs()?.control_polygon(tolerance),
        }
    }

    pub fn reversed(&self) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Line(c) => Self::Line(c.reversed()),
            Self::Arc(c) => Self::Arc(c.reversed(validation())?),
            Self::Polyline(c) => Self::Polyline(c.reversed()),
            Self::NurbsCurve(c) => Self::NurbsCurve(c.reversed()?),
        })
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Line(c) => Self::Line(c.transformed(transform, validation())?),
            Self::Arc(c) => match c.transformed_similarity(transform, validation())? {
                Some(c) => Self::Arc(c),
                None => Self::NurbsCurve(self.to_nurbs()?.transformed(transform)?),
            },
            Self::Polyline(c) => Self::Polyline(c.transformed(transform, validation())?),
            Self::NurbsCurve(c) => Self::NurbsCurve(c.transformed(transform)?),
        })
    }

    pub fn try_with_endpoints(
        &self,
        start: Option<Point3>,
        end: Option<Point3>,
    ) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Line(c) => Self::Line(c.try_with_endpoints(start, end, validation())?),
            Self::Arc(c) => Self::Arc(c.try_with_endpoints(start, end, validation())?),
            Self::Polyline(c) => {
                let mut points = c.vertices().to_vec();
                if let Some(p) = start {
                    points[0] = p;
                }
                if let Some(p) = end {
                    *points.last_mut().unwrap() = p;
                }
                Self::Polyline(Polyline3::try_with_parameters(
                    points,
                    c.parameters().to_vec(),
                    validation(),
                )?)
            }
            Self::NurbsCurve(c) => Self::NurbsCurve(c.try_with_endpoints(start, end)?),
        })
    }

    pub fn try_reparameterized(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        check_interval(&domain)?;
        Ok(match self {
            Self::Line(c) => Self::Line(c.try_reparameterized(domain)?),
            Self::Arc(c) => Self::Arc(c.try_reparameterized(domain)?),
            Self::NurbsCurve(c) => Self::NurbsCurve(c.try_reparameterized(domain)?),
            Self::Polyline(c) => Self::Polyline(Polyline3::try_with_parameters(
                c.vertices().to_vec(),
                c.parameters()
                    .iter()
                    .map(|&t| map_parameter(t, c.domain(), domain.clone()))
                    .collect::<Result<_, _>>()?,
                validation(),
            )?),
        })
    }

    pub fn try_trimmed(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        check_interval(&domain)?;
        checked_parameter(*domain.start(), self.domain())?;
        checked_parameter(*domain.end(), self.domain())?;
        if domain == self.domain() {
            return Ok(self.clone());
        }
        Ok(match self {
            Self::Line(c) => Self::Line(
                LineSegment::try_new(
                    c.evaluate(*domain.start())?,
                    c.evaluate(*domain.end())?,
                    validation(),
                )?
                .try_reparameterized(domain)?,
            ),
            Self::Arc(c) => Self::Arc(c.try_trimmed(domain)?),
            Self::NurbsCurve(c) => Self::NurbsCurve(c.try_trimmed(domain)?),
            Self::Polyline(c) => {
                let mut points = vec![c.evaluate(*domain.start())?];
                let mut parameters = vec![*domain.start()];
                for (&p, &t) in c.vertices().iter().zip(c.parameters()) {
                    if t > *domain.start() && t < *domain.end() {
                        points.push(p);
                        parameters.push(t);
                    }
                }
                points.push(c.evaluate(*domain.end())?);
                parameters.push(*domain.end());
                Self::Polyline(Polyline3::try_with_parameters(
                    points,
                    parameters,
                    validation(),
                )?)
            }
        })
    }
}

fn validation() -> Tolerance {
    Tolerance::try_new(
        Real::MIN_POSITIVE,
        Tolerance::DEFAULT.relative(),
        Tolerance::DEFAULT.angular(),
    )
    .expect("positive internal tolerance")
}
