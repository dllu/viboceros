//! Representation-aware curve ownership and endpoint editing.

#[cfg(test)]
mod tests;

use crate::{
    Circle3, CircularArc3, CurveRef, Ellipse3, GeometryError, LineSegment, NurbsCurve, Point3,
    PolyCurve3, Polyline3, Real, Tolerance, WeightedPoint3,
};

/// Owned counterpart of [`CurveRef`], used by operations that can change a
/// curve's representation (for example adding a line to an open NURBS curve).
#[derive(Clone, Debug, PartialEq)]
pub enum Curve3 {
    Line(LineSegment),
    Circle(Circle3),
    Arc(CircularArc3),
    Ellipse(Ellipse3),
    Polyline(Polyline3),
    NurbsCurve(NurbsCurve),
    PolyCurve(PolyCurve3),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveClosure {
    AlreadyClosed,
    EndpointMoved,
    SegmentAdded,
    GapTooWide,
    NotClosable,
}

impl CurveRef<'_> {
    pub fn to_owned(self) -> Curve3 {
        match self {
            Self::Line(curve) => Curve3::Line(*curve),
            Self::Circle(curve) => Curve3::Circle(*curve),
            Self::Arc(curve) => Curve3::Arc(*curve),
            Self::Ellipse(curve) => Curve3::Ellipse(*curve),
            Self::Polyline(curve) => Curve3::Polyline(curve.clone()),
            Self::NurbsCurve(curve) => Curve3::NurbsCurve(curve.clone()),
            Self::PolyCurve(curve) => Curve3::PolyCurve(curve.clone()),
        }
    }

    /// Exact rational locus in the native curve domain. Analytic angular
    /// parameterization is converted to the corresponding rational one.
    pub fn to_nurbs(self) -> Result<NurbsCurve, GeometryError> {
        match self {
            Self::Line(curve) => curve.to_nurbs(),
            Self::Circle(curve) => curve.to_nurbs()?.try_reparameterized(0.0..=curve.length()?),
            Self::Arc(curve) => curve.to_nurbs()?.try_reparameterized(curve.domain()),
            Self::Ellipse(curve) => curve
                .to_nurbs()?
                .try_reparameterized(0.0..=std::f64::consts::TAU),
            Self::Polyline(curve) => curve.to_native_nurbs(),
            Self::NurbsCurve(curve) => Ok(curve.clone()),
            Self::PolyCurve(curve) => curve.to_nurbs(),
        }
    }
}

impl Curve3 {
    pub fn as_ref(&self) -> CurveRef<'_> {
        match self {
            Self::Line(curve) => CurveRef::Line(curve),
            Self::Circle(curve) => CurveRef::Circle(curve),
            Self::Arc(curve) => CurveRef::Arc(curve),
            Self::Ellipse(curve) => CurveRef::Ellipse(curve),
            Self::Polyline(curve) => CurveRef::Polyline(curve),
            Self::NurbsCurve(curve) => CurveRef::NurbsCurve(curve),
            Self::PolyCurve(curve) => CurveRef::PolyCurve(curve),
        }
    }

    pub fn reversed(&self, tolerance: Tolerance) -> Result<Self, GeometryError> {
        Ok(match self {
            Self::Line(curve) => Self::Line(curve.reversed()),
            Self::Circle(curve) => Self::Circle(curve.reversed()),
            Self::Arc(curve) => Self::Arc(curve.reversed(tolerance)?),
            Self::Ellipse(curve) => Self::Ellipse(curve.reversed()),
            Self::Polyline(curve) => Self::Polyline(curve.reversed()),
            Self::NurbsCurve(curve) => Self::NurbsCurve(curve.reversed()?),
            Self::PolyCurve(curve) => Self::PolyCurve(curve.reversed()?),
        })
    }

    pub fn to_polycurve(&self) -> Result<PolyCurve3, GeometryError> {
        match self {
            Self::PolyCurve(curve) => Ok(curve.clone()),
            _ => PolyCurve3::try_new(vec![self.as_ref().to_nurbs()?]),
        }
    }

    /// Closes a curve without fitting. Already closed geometry is unchanged;
    /// eligible flexible endpoints can move, otherwise a straight segment is
    /// appended when allowed. A zero closure tolerance completes an analytic
    /// arc's supporting circle, without changing its native interval.
    pub fn close(
        &self,
        closure_tolerance: Real,
        close_wide_gaps_with_line: bool,
        tolerance: Tolerance,
    ) -> Result<(Self, CurveClosure), GeometryError> {
        if !closure_tolerance.is_finite() || closure_tolerance < 0.0 {
            return Err(GeometryError::InvalidCurveClosureTolerance);
        }
        if self.as_ref().is_closed()? {
            return Ok((self.clone(), CurveClosure::AlreadyClosed));
        }
        if let Self::Line(_) = self {
            return Ok((self.clone(), CurveClosure::NotClosable));
        }
        if let Self::NurbsCurve(curve) = self
            && curve.is_linear_at_zero_tolerance()?
        {
            return Ok((self.clone(), CurveClosure::NotClosable));
        }
        if let Self::Polyline(curve) = self
            && curve.segment_count() < 2
        {
            return Ok((self.clone(), CurveClosure::NotClosable));
        }
        if let Self::Arc(curve) = self
            && closure_tolerance == 0.0
        {
            return Ok((Self::Arc(curve.closed()), CurveClosure::EndpointMoved));
        }
        let start = self.as_ref().start_point()?;
        let end = self.as_ref().end_point()?;
        let gap = start.distance_to(end)?;
        if closure_tolerance == 0.0 || gap <= closure_tolerance {
            let moved = match self {
                Self::Polyline(curve) if curve.segment_count() >= 3 => {
                    let mut vertices = curve.vertices().to_vec();
                    *vertices.last_mut().expect("a polyline has vertices") = start;
                    match Polyline3::try_with_parameters(
                        vertices,
                        curve.parameters().to_vec(),
                        tolerance,
                    ) {
                        Ok(curve) => Some(Self::Polyline(curve)),
                        Err(GeometryError::DegeneratePolylineSegment { .. }) => None,
                        Err(error) => return Err(error),
                    }
                }
                Self::NurbsCurve(curve) => Some(Self::NurbsCurve(
                    curve.try_with_endpoints(None, Some(start))?,
                )),
                Self::PolyCurve(curve) => Some(Self::PolyCurve(
                    curve.try_with_endpoints(None, Some(start))?,
                )),
                _ => None,
            };
            if let Some(moved) = moved
                && moved.as_ref().is_closed()?
            {
                return Ok((moved, CurveClosure::EndpointMoved));
            }
        }
        if !close_wide_gaps_with_line {
            return Ok((self.clone(), CurveClosure::GapTooWide));
        }
        let segment = LineSegment::try_new(
            end,
            start,
            Tolerance::try_new(f64::MIN_POSITIVE, tolerance.relative(), tolerance.angular())?,
        )?;
        let appended = PolyCurve3::concatenate(&[
            self.to_polycurve()?,
            PolyCurve3::try_new(vec![segment.to_nurbs()?])?,
        ])?;
        Ok((Self::PolyCurve(appended), CurveClosure::SegmentAdded))
    }
}

impl NurbsCurve {
    /// Replaces natural endpoints while keeping their rational weights. A
    /// nontrivial edit clamps the active domain first; knot refinement preserves
    /// the locus before the endpoint control is moved. Exact no-ops do not clamp.
    pub fn try_with_endpoints(
        &self,
        start: Option<Point3>,
        end: Option<Point3>,
    ) -> Result<Self, GeometryError> {
        let start = match start {
            Some(point) => (self.evaluate(*self.domain().start())? != point).then_some(point),
            None => None,
        };
        let end = match end {
            Some(point) => (self.evaluate(*self.domain().end())? != point).then_some(point),
            None => None,
        };
        if start.is_none() && end.is_none() {
            return Ok(self.clone());
        }
        let curve = self.clamped_to_active_domain()?;
        let mut controls = curve.control_points().to_vec();
        if let Some(point) = start {
            controls[0] = WeightedPoint3::try_new(point, controls[0].weight())?;
        }
        if let Some(point) = end {
            let last = controls.len() - 1;
            controls[last] = WeightedPoint3::try_new(point, controls[last].weight())?;
        }
        Self::try_new_rational(curve.degree(), controls, curve.knots().to_vec())
    }
}

impl PolyCurve3 {
    /// Edits only the exterior segment endpoints, retaining all outer intervals.
    pub fn try_with_endpoints(
        &self,
        start: Option<Point3>,
        end: Option<Point3>,
    ) -> Result<Self, GeometryError> {
        let mut segments = self.segments().to_vec();
        let last = segments.len() - 1;
        if last == 0 {
            segments[0] = segments[0].try_with_endpoints(start, end)?;
        } else {
            segments[0] = segments[0].try_with_endpoints(start, None)?;
            segments[last] = segments[last].try_with_endpoints(None, end)?;
        }
        Self::try_with_segment_domains(segments, self.parameters().to_vec())
    }
}
