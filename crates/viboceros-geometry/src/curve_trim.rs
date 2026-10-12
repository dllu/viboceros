//! Native-domain trimming, cyclic edits, and closest-point dispatch.

use crate::parameter::{check_interval, map_parameter, shifted_parameter, wrapped_parameter};
use crate::point::PointDistance;
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
                } else if !target.compare_distances(c.start()?, c.end()?).is_gt() {
                    Ok(*c.domain().start())
                } else {
                    Ok(*c.domain().end())
                }
            }
            Self::Ellipse(c) => c.closest_parameter(target),
            Self::NurbsCurve(c) => c.closest_parameter(target, tolerance),
            Self::Polyline(c) => {
                let mut best: Option<(PointDistance, Real)> = None;
                for segment in c.segments() {
                    let t = CurveRef::Line(&segment).closest_parameter(target, tolerance)?;
                    let distance = PointDistance::new(target, segment.evaluate(t)?);
                    if best.is_none_or(|(previous, _)| distance.compare(&previous, target).is_lt())
                    {
                        best = Some((distance, t));
                    }
                }
                Ok(best.expect("a polyline contains at least one segment").1)
            }
            Self::PolyCurve(c) => {
                let mut best: Option<(PointDistance, Real)> = None;
                for (index, segment) in c.segments().iter().enumerate() {
                    let t = segment.as_ref().closest_parameter(target, tolerance)?;
                    let distance = PointDistance::new(target, segment.evaluate(t)?);
                    if best.is_none_or(|(previous, _)| distance.compare(&previous, target).is_lt())
                    {
                        best = Some((distance, c.polycurve_parameter(index, t)?));
                    }
                }
                Ok(best.expect("a polycurve contains at least one segment").1)
            }
        }
    }
}

impl Curve3 {
    /// Extracts an increasing native interval without rationalizing analytic
    /// leaves. Ellipses use their parameter-equivalent rational representation.
    pub fn try_trimmed(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        self.try_trimmed_with_nurbs(domain, &mut |curve, interval| curve.try_trimmed(interval))
    }

    /// Retain native leaf types and composite domains while delegating NURBS
    /// control extraction to a caller-supplied geometry backend.
    pub fn try_trimmed_with_nurbs<E: From<GeometryError>>(
        &self,
        domain: RangeInclusive<Real>,
        trim: &mut impl FnMut(&crate::NurbsCurve, RangeInclusive<Real>) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Self, E> {
        if let Self::NurbsCurve(c) = self {
            crate::parameter::check_trim_interval(&domain, c.domain())?;
            if domain == c.domain() {
                return Ok(self.clone());
            }
            return Ok(Self::NurbsCurve(trim(c, domain)?));
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
            Self::Ellipse(c) => Self::NurbsCurve(trim(&c.to_nurbs()?, domain)?),
            Self::PolyCurve(c) => Self::PolyCurve(c.try_trimmed_with_nurbs(domain, trim)?),
            _ => CurveSegment3::try_from_curve(self)?
                .try_trimmed(domain)?
                .into_curve(),
        })
    }

    /// Directed native subcurve: reverse decreasing open intervals, and wrap
    /// decreasing closed intervals forward across the existing seam.
    pub fn try_subcurve(&self, start: Real, end: Real) -> Result<Self, GeometryError> {
        self.try_subcurve_with_nurbs(start, end, &mut |curve, interval| {
            curve.try_trimmed(interval)
        })
    }

    /// Apply the same directed interval and closed-seam policy with an external
    /// NURBS segment extractor. Analytic leaves retain their native classes.
    pub fn try_subcurve_with_nurbs<E: From<GeometryError>>(
        &self,
        start: Real,
        end: Real,
        trim: &mut impl FnMut(&crate::NurbsCurve, RangeInclusive<Real>) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Self, E> {
        let domain = self.as_ref().domain();
        if !start.is_finite()
            || !end.is_finite()
            || start == end
            || !domain.contains(&start)
            || !domain.contains(&end)
        {
            return Err(GeometryError::InvalidCurveTrimInterval.into());
        }
        if start < end {
            return self.try_trimmed_with_nurbs(start..=end, trim);
        }
        if !self.as_ref().is_closed()? {
            return Ok(self
                .try_trimmed_with_nurbs(end..=start, trim)?
                .reversed(validation())?);
        }
        if start == *domain.end() {
            return self.try_trimmed_with_nurbs(*domain.start()..=end, trim);
        }
        if end == *domain.start() {
            return self.try_trimmed_with_nurbs(start..=*domain.end(), trim);
        }
        self.closed_subcurve_across_seam_with_nurbs(start, end, trim)
    }

    /// Splits at distinct interior native parameters. Closed results traverse
    /// the sorted stations cyclically; one station relocates the closed seam.
    pub fn try_split_at_parameters(&self, parameters: &[Real]) -> Result<Vec<Self>, GeometryError> {
        self.try_split_at_parameters_with_nurbs(parameters, &mut |curve, interval| {
            curve.try_trimmed(interval)
        })
    }

    /// Split with the existing ordering/seam rules and an external NURBS
    /// extractor, including NURBS leaves in flat polycurves.
    pub fn try_split_at_parameters_with_nurbs<E: From<GeometryError>>(
        &self,
        parameters: &[Real],
        trim: &mut impl FnMut(&crate::NurbsCurve, RangeInclusive<Real>) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Vec<Self>, E> {
        let domain = self.as_ref().domain();
        let mut cuts = parameters.to_vec();
        cuts.sort_by(Real::total_cmp);
        if cuts.is_empty()
            || cuts
                .iter()
                .any(|t| !t.is_finite() || *t <= *domain.start() || *t >= *domain.end())
            || cuts.windows(2).any(|p| p[0] == p[1])
        {
            return Err(GeometryError::InvalidCurveSplitParameter.into());
        }
        if self.as_ref().is_closed()? {
            if cuts.len() == 1 {
                return Ok(vec![
                    self.closed_subcurve_across_seam_with_nurbs(cuts[0], cuts[0], trim)?,
                ]);
            }
            let mut pieces = cuts
                .windows(2)
                .map(|p| self.try_subcurve_with_nurbs(p[0], p[1], trim))
                .collect::<Result<Vec<_>, _>>()?;
            pieces.push(self.try_subcurve_with_nurbs(*cuts.last().unwrap(), cuts[0], trim)?);
            Ok(pieces)
        } else {
            cuts.insert(0, *domain.start());
            cuts.push(*domain.end());
            cuts.windows(2)
                .map(|p| self.try_trimmed_with_nurbs(p[0]..=p[1], trim))
                .collect()
        }
    }

    fn closed_subcurve_across_seam_with_nurbs<E: From<GeometryError>>(
        &self,
        start: Real,
        end: Real,
        trim: &mut impl FnMut(&crate::NurbsCurve, RangeInclusive<Real>) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Self, E> {
        let domain = self.as_ref().domain();
        let left = self
            .try_trimmed_with_nurbs(start..=*domain.end(), trim)?
            .to_polycurve()?;
        let right = self
            .try_trimmed_with_nurbs(*domain.start()..=end, trim)?
            .to_polycurve()?;
        Ok(Self::PolyCurve(PolyCurve3::concatenate(&[left, right])?))
    }

    /// Rotates a closed curve's seam while retaining native leaf classes and
    /// parameter speed. The returned interval starts at the requested parameter.
    pub fn try_change_closed_seam(&self, parameter: Real) -> Result<Self, GeometryError> {
        self.try_change_closed_seam_with_nurbs(
            parameter,
            &mut |curve, parameter| curve.try_change_closed_seam(parameter),
            &mut |curve, interval| curve.try_trimmed(interval),
        )
    }

    /// Preserve analytic/composite seam policy while delegating NURBS seam
    /// relocation and partial NURBS leaves to an external geometry backend.
    pub fn try_change_closed_seam_with_nurbs<E: From<GeometryError>>(
        &self,
        parameter: Real,
        seam: &mut impl FnMut(&crate::NurbsCurve, Real) -> Result<crate::NurbsCurve, E>,
        trim: &mut impl FnMut(&crate::NurbsCurve, RangeInclusive<Real>) -> Result<crate::NurbsCurve, E>,
    ) -> Result<Self, E> {
        crate::require_finite([parameter], "curve seam parameter")?;
        if !self.as_ref().is_closed()? {
            return Err(GeometryError::CurveSeamMustBeClosed.into());
        }
        if let Self::NurbsCurve(c) = self {
            return Ok(Self::NurbsCurve(seam(c, parameter)?));
        }
        let domain = self.as_ref().domain();
        if !domain.contains(&parameter) {
            let wrapped = wrapped_parameter(parameter, &domain)?;
            return Ok(self
                .try_change_closed_seam_with_nurbs(wrapped, seam, trim)?
                .try_reparameterized(parameter..=shifted_parameter(parameter, &domain)?)?);
        }
        Ok(match self {
            Self::Circle(c) => Self::Circle(c.try_change_closed_seam(parameter)?),
            Self::Arc(c) => Self::Arc(c.try_change_closed_seam(parameter)?),
            Self::Ellipse(c) => Self::NurbsCurve(seam(&c.to_nurbs()?, parameter)?),
            Self::NurbsCurve(c) => Self::NurbsCurve(seam(c, parameter)?),
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
                        .try_change_closed_seam_with_nurbs(local, seam, trim)?;
                    Self::PolyCurve(PolyCurve3::try_with_segment_domains(
                        vec![CurveSegment3::try_from_curve(&segment)?],
                        vec![parameter, end],
                    )?)
                } else {
                    Self::PolyCurve(PolyCurve3::concatenate(&[
                        c.try_trimmed_with_nurbs(parameter..=*domain.end(), trim)?,
                        c.try_trimmed_with_nurbs(*domain.start()..=parameter, trim)?,
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
