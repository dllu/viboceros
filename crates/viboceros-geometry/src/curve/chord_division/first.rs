//! One endpoint sphere crossing, without computing subsequent chord stations.
use super::*;
#[cfg(test)]
mod tests;

impl CurveRef<'_> {
    /// Find the first point at a straight-line distance from the selected
    /// natural endpoint, traversing inward. Returns its original native chart;
    /// `None` means the sphere never meets the bounded curve. Denominator poles
    /// are rejected. This does not integrate arc length or traverse later chords.
    pub fn point_at_chord_distance(
        self,
        distance: Real,
        from_end: bool,
        tolerance: Tolerance,
    ) -> Result<Option<CurveDivisionPoint>, GeometryError> {
        if !distance.is_finite() || distance <= 0. {
            return Err(GeometryError::InvalidCurveDivisionLength);
        }
        let fraction = if let CurveRef::Line(line) = self {
            let length = line.length()?;
            if distance > length {
                return Ok(None);
            }
            Some(distance / length)
        } else if let Some((radius, sweep)) = match self {
            CurveRef::Circle(c) => Some((c.radius(), std::f64::consts::TAU)),
            CurveRef::Arc(c) => Some((c.radius(), c.sweep_radians())),
            _ => None,
        } {
            let ratio = (distance / radius) * 0.5;
            if ratio > 1. {
                return Ok(None);
            }
            let angle = 2. * ratio.asin();
            if angle > sweep {
                return Ok(None);
            }
            Some(angle / sweep)
        } else {
            None
        };
        if let Some(f) = fraction {
            let parameter = self.parameter_at(if from_end { 1. - f } else { f })?;
            if parameter
                == if from_end {
                    *self.domain().end()
                } else {
                    *self.domain().start()
                }
            {
                return Err(GeometryError::BezierDecompositionLimit);
            }
            return Ok(Some(CurveDivisionPoint {
                parameter,
                point: self.evaluate(parameter)?,
            }));
        }
        let source = self.to_nurbs()?;
        let search = if from_end {
            source.reversed()?
        } else {
            source.clone()
        };
        let center = if from_end {
            self.end_point()?
        } else {
            self.start_point()?
        };
        for span in validated_spans(&search)? {
            let coefficients = sphere_coefficients(&span, center, distance)?;
            let domain = span.domain();
            if let Some(parameter) = first_root(
                &coefficients,
                [*domain.start(), *domain.end()],
                Some(*search.domain().start()),
            )? {
                let point = search.evaluate(parameter)?;
                let native = native_parameter(
                    self,
                    &source,
                    if from_end { -parameter } else { parameter },
                    point,
                    tolerance,
                )?;
                return Ok(Some(CurveDivisionPoint {
                    parameter: native,
                    point,
                }));
            }
        }
        Ok(None)
    }
}
