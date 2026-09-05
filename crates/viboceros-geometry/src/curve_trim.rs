//! Native-domain trimming, cyclic edits, and closest-point dispatch.

use crate::parameter::{check_interval, map_parameter, shifted_parameter, wrapped_parameter};
use crate::{
    Curve3, CurveRef, CurveSegment3, GeometryError, Point3, PolyCurve3, Polyline3, Real, Tolerance,
};
use std::ops::RangeInclusive;

#[cfg(test)]
mod tests;

impl CurveRef<'_> {
    /// Finds a closest location in the source's native interval. Circular
    /// curves use analytic projection, without changing to rational parameters.
    pub fn closest_parameter(
        self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<Real, GeometryError> {
        match self {
            Self::Line(c) => self.parameter_at(c.closest_parameter(target, validation())?),
            Self::Circle(c) => {
                let delta = c.center().vector_to(target)?;
                let angle = delta
                    .dot(c.y_axis().as_vector())?
                    .atan2(delta.dot(c.x_axis().as_vector())?)
                    .rem_euclid(std::f64::consts::TAU);
                map_parameter(angle, 0.0..=std::f64::consts::TAU, c.domain())
            }
            Self::Arc(c) => {
                let delta = c.center().vector_to(target)?;
                let angle = delta
                    .dot(c.y_axis().as_vector())?
                    .atan2(delta.dot(c.x_axis().as_vector())?)
                    .rem_euclid(std::f64::consts::TAU);
                if angle <= c.sweep_radians() {
                    map_parameter(angle, 0.0..=c.sweep_radians(), c.domain())
                } else if c.start()?.distance_to(target)? <= c.end()?.distance_to(target)? {
                    Ok(*c.domain().start())
                } else {
                    Ok(*c.domain().end())
                }
            }
            Self::Ellipse(c) => c.to_nurbs()?.closest_parameter(target, tolerance),
            Self::NurbsCurve(c) => c.closest_parameter(target, tolerance),
            Self::Polyline(c) => {
                let mut best = (Real::INFINITY, *c.domain().start());
                for segment in c.segments() {
                    let t = CurveRef::Line(&segment).closest_parameter(target, tolerance)?;
                    let distance = segment.evaluate(t)?.distance_to(target)?;
                    if distance < best.0 {
                        best = (distance, t);
                    }
                }
                Ok(best.1)
            }
            Self::PolyCurve(c) => {
                let mut best = (Real::INFINITY, *c.domain().start());
                for (index, segment) in c.segments().iter().enumerate() {
                    let t = segment.as_ref().closest_parameter(target, tolerance)?;
                    let distance = segment.evaluate(t)?.distance_to(target)?;
                    if distance < best.0 {
                        best = (distance, c.polycurve_parameter(index, t)?);
                    }
                }
                Ok(best.1)
            }
        }
    }
}

impl Curve3 {
    /// Extracts an increasing native interval without rationalizing analytic
    /// leaves. Ellipses use their parameter-equivalent rational representation.
    pub fn try_trimmed(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        if let Self::NurbsCurve(c) = self {
            return Ok(Self::NurbsCurve(c.try_trimmed(domain)?));
        }
        validate_trim(&domain, self.as_ref().domain())?;
        if domain == self.as_ref().domain() {
            return Ok(self.clone());
        }
        Ok(match self {
            Self::Circle(c) => Self::Arc(
                crate::CircularArc3::try_from_circle_sweep(*c, std::f64::consts::TAU)?
                    .try_reparameterized(c.domain())?
                    .try_trimmed(domain)?,
            ),
            Self::Ellipse(c) => Self::NurbsCurve(c.to_nurbs()?.try_trimmed(domain)?),
            Self::PolyCurve(c) => Self::PolyCurve(c.try_trimmed(domain)?),
            _ => CurveSegment3::try_from_curve(self)?
                .try_trimmed(domain)?
                .into_curve(),
        })
    }

    /// Directed native subcurve: reverse decreasing open intervals, and wrap
    /// decreasing closed intervals forward across the existing seam.
    pub fn try_subcurve(&self, start: Real, end: Real) -> Result<Self, GeometryError> {
        let domain = self.as_ref().domain();
        if !start.is_finite()
            || !end.is_finite()
            || start == end
            || !domain.contains(&start)
            || !domain.contains(&end)
        {
            return Err(GeometryError::InvalidCurveTrimInterval);
        }
        if start < end {
            return self.try_trimmed(start..=end);
        }
        if !self.as_ref().is_closed()? {
            return self.try_trimmed(end..=start)?.reversed(validation());
        }
        if start == *domain.end() {
            return self.try_trimmed(*domain.start()..=end);
        }
        if end == *domain.start() {
            return self.try_trimmed(start..=*domain.end());
        }
        self.closed_subcurve_across_seam(start, end)
    }

    /// Splits at distinct interior native parameters. Closed results traverse
    /// the sorted stations cyclically; one station relocates the closed seam.
    pub fn try_split_at_parameters(&self, parameters: &[Real]) -> Result<Vec<Self>, GeometryError> {
        let domain = self.as_ref().domain();
        let mut cuts = parameters.to_vec();
        cuts.sort_by(Real::total_cmp);
        if cuts.is_empty()
            || cuts
                .iter()
                .any(|t| !t.is_finite() || *t <= *domain.start() || *t >= *domain.end())
            || cuts.windows(2).any(|p| p[0] == p[1])
        {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }
        if self.as_ref().is_closed()? {
            if cuts.len() == 1 {
                return Ok(vec![self.closed_subcurve_across_seam(cuts[0], cuts[0])?]);
            }
            let mut pieces = cuts
                .windows(2)
                .map(|p| self.try_subcurve(p[0], p[1]))
                .collect::<Result<Vec<_>, _>>()?;
            pieces.push(self.try_subcurve(*cuts.last().unwrap(), cuts[0])?);
            Ok(pieces)
        } else {
            cuts.insert(0, *domain.start());
            cuts.push(*domain.end());
            cuts.windows(2)
                .map(|p| self.try_trimmed(p[0]..=p[1]))
                .collect()
        }
    }

    fn closed_subcurve_across_seam(&self, start: Real, end: Real) -> Result<Self, GeometryError> {
        let domain = self.as_ref().domain();
        let left = self.try_trimmed(start..=*domain.end())?.to_polycurve()?;
        let right = self.try_trimmed(*domain.start()..=end)?.to_polycurve()?;
        Ok(Self::PolyCurve(PolyCurve3::concatenate(&[left, right])?))
    }

    /// Rotates a closed curve's seam while retaining native leaf classes and
    /// parameter speed. The returned interval starts at the requested parameter.
    pub fn try_change_closed_seam(&self, parameter: Real) -> Result<Self, GeometryError> {
        crate::require_finite([parameter], "curve seam parameter")?;
        if !self.as_ref().is_closed()? {
            return Err(GeometryError::CurveSeamMustBeClosed);
        }
        if let Self::NurbsCurve(c) = self {
            return Ok(Self::NurbsCurve(c.try_change_closed_seam(parameter)?));
        }
        let domain = self.as_ref().domain();
        if !domain.contains(&parameter) {
            let wrapped = wrapped_parameter(parameter, &domain)?;
            return self
                .try_change_closed_seam(wrapped)?
                .try_reparameterized(parameter..=shifted_parameter(parameter, &domain)?);
        }
        Ok(match self {
            Self::Circle(c) => Self::Circle(c.try_change_closed_seam(parameter)?),
            Self::Arc(c) => Self::Arc(c.try_change_closed_seam(parameter)?),
            Self::Ellipse(c) => Self::NurbsCurve(c.to_nurbs()?.try_change_closed_seam(parameter)?),
            Self::NurbsCurve(c) => Self::NurbsCurve(c.try_change_closed_seam(parameter)?),
            Self::Polyline(c) => Self::Polyline(polyline_seam(c, parameter)?),
            Self::PolyCurve(c) => {
                let domain = c.domain();
                let end = shifted_parameter(parameter, &domain)?;
                if parameter == *domain.start() || parameter == *domain.end() {
                    Self::PolyCurve(c.try_reparameterized(parameter..=end)?)
                } else if c.segments().len() == 1 {
                    let local = c.segment_parameter(0, parameter)?;
                    let segment = c.segments()[0]
                        .as_ref()
                        .to_owned()
                        .try_change_closed_seam(local)?;
                    Self::PolyCurve(PolyCurve3::try_with_segment_domains(
                        vec![CurveSegment3::try_from_curve(&segment)?],
                        vec![parameter, end],
                    )?)
                } else {
                    Self::PolyCurve(PolyCurve3::concatenate(&[
                        c.try_trimmed(parameter..=*domain.end())?,
                        c.try_trimmed(*domain.start()..=parameter)?,
                    ])?)
                }
            }
            Self::Line(_) => unreachable!("a line cannot be closed"),
        })
    }
}

fn validate_trim(
    domain: &RangeInclusive<Real>,
    source: RangeInclusive<Real>,
) -> Result<(), GeometryError> {
    if check_interval(domain).is_err()
        || !source.contains(domain.start())
        || !source.contains(domain.end())
    {
        return Err(GeometryError::InvalidCurveTrimInterval);
    }
    Ok(())
}

fn polyline_seam(curve: &Polyline3, parameter: Real) -> Result<Polyline3, GeometryError> {
    let domain = curve.domain();
    let end = shifted_parameter(parameter, &domain)?;
    if parameter == *domain.start() || parameter == *domain.end() {
        let CurveSegment3::Polyline(result) =
            CurveSegment3::Polyline(curve.clone()).try_reparameterized(parameter..=end)?
        else {
            unreachable!()
        };
        return Ok(result);
    }
    let (i, fraction) = curve.parameter_location(parameter)?;
    let seam = if fraction <= 8.0 * Real::EPSILON {
        curve.parameters()[i]
    } else if fraction >= 1.0 - 8.0 * Real::EPSILON {
        curve.parameters()[i + 1]
    } else {
        parameter
    };
    if seam == *domain.start() || seam == *domain.end() {
        let CurveSegment3::Polyline(result) =
            CurveSegment3::Polyline(curve.clone()).try_reparameterized(parameter..=end)?
        else {
            unreachable!()
        };
        return Ok(result);
    }
    let point = curve.evaluate(seam)?;
    let mut vertices = vec![point];
    let mut parameters = vec![seam];
    for (&p, &t) in curve.vertices().iter().zip(curve.parameters()) {
        if t > seam {
            vertices.push(p);
            parameters.push(t);
        }
    }
    for (&p, &t) in curve.vertices().iter().zip(curve.parameters()).skip(1) {
        if t < seam {
            vertices.push(p);
            parameters.push(shifted_parameter(t, &domain)?);
        }
    }
    vertices.push(point);
    parameters.push(shifted_parameter(seam, &domain)?);
    let result = CurveSegment3::Polyline(Polyline3::try_with_parameters(
        vertices,
        parameters,
        validation(),
    )?)
    .try_reparameterized(parameter..=end)?;
    let CurveSegment3::Polyline(result) = result else {
        unreachable!()
    };
    Ok(result)
}

fn validation() -> Tolerance {
    Tolerance::try_new(
        Real::MIN_POSITIVE,
        Tolerance::DEFAULT.relative(),
        Tolerance::DEFAULT.angular(),
    )
    .expect("positive internal tolerance")
}
