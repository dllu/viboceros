//! Exact piecewise rational curves with independent segment parameterizations.

use std::ops::RangeInclusive;

use crate::{
    AffineTransform3, BoundingBox3, GeometryError, NurbsCurve, Point3, Polyline3, Real, Tolerance,
    Vector3, nurbs::curve_points_coincident, require_finite,
};

#[cfg(test)]
mod tests;

pub const MAX_POLYCURVE_SEGMENTS: usize = 65_536;
const MAX_CONVERSION_CONTROLS: usize = 1_000_000;

/// Which segment supplies a derivative at a polycurve junction.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CurveEvaluationSide {
    Left,
    #[default]
    Right,
}

/// A flat sequence of exact NURBS segments, without fitting or endpoint edits.
///
/// The outer parameter intervals need not equal the segments' natural domains.
/// Both are retained explicitly, so a reparameterization never changes segment
/// controls, weights, or knots. Analytic curves can be added in their exact
/// rational forms. Nested composites can be flattened with [`Self::concatenate`].
/// Junctions must be coincident under the kernel's fixed curve-coincidence
/// predicate; closing model-tolerance gaps is a separate editing operation.
#[derive(Clone, Debug, PartialEq)]
pub struct PolyCurve3 {
    segments: Vec<NurbsCurve>,
    parameters: Vec<Real>,
    bounds: BoundingBox3,
}

impl PolyCurve3 {
    /// One connected control polygon through the original segment controls,
    /// removing duplicated junction controls without elevating degrees.
    pub fn control_polygon(&self, tolerance: Tolerance) -> Result<Polyline3, GeometryError> {
        let mut points = Vec::new();
        for segment in &self.segments {
            let polygon = segment.control_polygon(tolerance)?;
            let vertices = polygon.vertices();
            let skip = usize::from(points.last() == vertices.first());
            points.extend_from_slice(&vertices[skip..]);
        }
        Polyline3::try_new(points, tolerance)
    }

    /// Appends segments in the supplied order. The first natural domain starts
    /// the composite, and each later segment contributes its natural span.
    pub fn try_new(segments: Vec<NurbsCurve>) -> Result<Self, GeometryError> {
        check_count(segments.len())?;
        let mut parameters = Vec::with_capacity(segments.len() + 1);
        parameters.push(*segments[0].domain().start());
        for segment in &segments {
            let start = *parameters.last().expect("the initial parameter exists");
            parameters.push(append_end(start, segment.domain())?);
        }
        Self::try_with_segment_domains(segments, parameters)
    }

    /// Constructs a composite with one strictly increasing outer break per
    /// segment endpoint. Local segment domains are kept unchanged.
    pub fn try_with_segment_domains(
        segments: Vec<NurbsCurve>,
        parameters: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        check_count(segments.len())?;
        if parameters.len() != segments.len() + 1 {
            return Err(invalid(
                "one more outer parameter than segments is required",
            ));
        }
        check_interval(parameters[0], parameters[segments.len()])?;
        for (index, segment) in segments.iter().enumerate() {
            check_interval(parameters[index], parameters[index + 1])?;
            check_interval(*segment.domain().start(), *segment.domain().end())?;
            if segments.len() > 1 && segment.is_closed()? {
                return Err(invalid(
                    "closed segments are only valid in a single-segment polycurve",
                ));
            }
            if index > 0 {
                let previous = &segments[index - 1];
                if !curve_points_coincident(
                    previous.evaluate(*previous.domain().end())?,
                    segment.evaluate(*segment.domain().start())?,
                ) {
                    return Err(invalid("adjacent segment endpoints are not coincident"));
                }
            }
        }
        let bounds = segments
            .iter()
            .skip(1)
            .try_fold(segments[0].control_point_bounds(), |bounds, segment| {
                bounds.union(segment.control_point_bounds())
            })?;
        Ok(Self {
            segments,
            parameters,
            bounds,
        })
    }

    pub fn segments(&self) -> &[NurbsCurve] {
        &self.segments
    }

    pub fn parameters(&self) -> &[Real] {
        &self.parameters
    }

    pub fn domain(&self) -> RangeInclusive<Real> {
        self.parameters[0]..=self.parameters[self.segments.len()]
    }

    /// Control-hull bounds, like [`NurbsCurve::control_point_bounds`].
    pub fn control_point_bounds(&self) -> BoundingBox3 {
        self.bounds
    }

    pub fn segment_domain(&self, index: usize) -> Result<RangeInclusive<Real>, GeometryError> {
        self.segment(index)?;
        Ok(self.parameters[index]..=self.parameters[index + 1])
    }

    /// Exact junctions select the following segment by default. At the two
    /// natural endpoints both side choices use the only available segment.
    pub fn segment_index(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<usize, GeometryError> {
        checked_parameter(parameter, self.domain())?;
        let after = match side {
            CurveEvaluationSide::Left => self.parameters.partition_point(|t| *t < parameter),
            CurveEvaluationSide::Right => self.parameters.partition_point(|t| *t <= parameter),
        };
        Ok(after.saturating_sub(1).min(self.segments.len() - 1))
    }

    pub fn segment_parameter(&self, index: usize, parameter: Real) -> Result<Real, GeometryError> {
        let segment = self.segment(index)?;
        map_parameter(parameter, self.segment_domain(index)?, segment.domain())
    }

    pub fn polycurve_parameter(
        &self,
        index: usize,
        parameter: Real,
    ) -> Result<Real, GeometryError> {
        let segment = self.segment(index)?;
        map_parameter(parameter, segment.domain(), self.segment_domain(index)?)
    }

    pub fn parameter_at(&self, normalized: Real) -> Result<Real, GeometryError> {
        map_parameter(normalized, 0.0..=1.0, self.domain())
    }

    pub fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        let index = self.segment_index(parameter, CurveEvaluationSide::Right)?;
        self.segments[index].evaluate(self.segment_parameter(index, parameter)?)
    }

    pub fn evaluate_with_derivative(
        &self,
        parameter: Real,
    ) -> Result<(Point3, Vector3), GeometryError> {
        let index = self.segment_index(parameter, CurveEvaluationSide::Right)?;
        let (point, derivative) = self.segments[index]
            .evaluate_with_derivative(self.segment_parameter(index, parameter)?)?;
        Ok((point, self.scale_derivative(index, derivative)?))
    }

    pub fn evaluate_with_second_derivative(
        &self,
        parameter: Real,
        side: CurveEvaluationSide,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let index = self.segment_index(parameter, side)?;
        let (point, first, second) = self.segments[index]
            .evaluate_with_second_derivative(self.segment_parameter(index, parameter)?)?;
        Ok((
            point,
            self.scale_derivative(index, first)?,
            self.scale_derivative(index, self.scale_derivative(index, second)?)?,
        ))
    }

    fn scale_derivative(
        &self,
        index: usize,
        derivative: Vector3,
    ) -> Result<Vector3, GeometryError> {
        let local = self.segments[index].domain();
        let numerator = *local.end() - *local.start();
        let denominator = self.parameters[index + 1] - self.parameters[index];
        // Multiplying the ratio first can overflow although each final
        // derivative coordinate is representable. Choose a safe multiplication
        // and division order for each coordinate instead.
        let coordinates = derivative.to_array();
        let mut scaled = [0.0; 3];
        for (result, value) in scaled.iter_mut().zip(coordinates) {
            *result = scaled_ratio(value, numerator, denominator)?;
        }
        Vector3::try_from(scaled)
    }

    pub fn length(&self, tolerance: Tolerance) -> Result<Real, GeometryError> {
        let tolerance = Tolerance::try_new(
            (tolerance.absolute() / self.segments.len() as Real).max(Real::MIN_POSITIVE),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let mut sum = 0.0;
        let mut correction = 0.0;
        for segment in &self.segments {
            let value = segment.length(tolerance)?;
            let next = sum + value;
            correction += if sum.abs() >= value.abs() {
                (sum - next) + value
            } else {
                (value - next) + sum
            };
            sum = next;
        }
        let result = sum + correction;
        require_finite([result], "polycurve length")?;
        Ok(result)
    }

    pub fn is_closed(&self) -> Result<bool, GeometryError> {
        let start = self.evaluate(*self.domain().start())?;
        let end = self.evaluate(*self.domain().end())?;
        if !curve_points_coincident(start, end) {
            return Ok(false);
        }
        // Endpoint coincidence alone would incorrectly close a collapsed curve.
        let first = self.evaluate(self.parameter_at(1.0 / 3.0)?)?;
        let second = self.evaluate(self.parameter_at(2.0 / 3.0)?)?;
        Ok(!curve_points_coincident(start, first)
            && !curve_points_coincident(start, second)
            && !curve_points_coincident(end, first)
            && !curve_points_coincident(end, second))
    }

    pub fn reversed(&self) -> Result<Self, GeometryError> {
        Self::try_with_segment_domains(
            self.segments
                .iter()
                .rev()
                .map(NurbsCurve::reversed)
                .collect::<Result<_, _>>()?,
            self.parameters.iter().rev().map(|t| -*t).collect(),
        )
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        Self::try_with_segment_domains(
            self.segments
                .iter()
                .map(|s| s.transformed(transform))
                .collect::<Result<_, _>>()?,
            self.parameters.clone(),
        )
    }

    /// Affinely rescales existing intervals without changing relative segment
    /// speeds. RhinoCommon's Domain setter instead redistributes by length.
    pub fn try_reparameterized(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        check_interval(*domain.start(), *domain.end())?;
        Self::try_with_segment_domains(
            self.segments.clone(),
            self.parameters
                .iter()
                .map(|t| map_parameter(*t, self.domain(), domain.clone()))
                .collect::<Result<_, _>>()?,
        )
    }

    /// Assigns outer intervals proportional to segment arc lengths, as Rhino 8
    /// does when setting a polycurve's Domain. Each segment keeps its original
    /// internal parameterization; this is not arc-length inversion.
    pub fn try_reparameterized_by_length(
        &self,
        domain: RangeInclusive<Real>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        check_interval(*domain.start(), *domain.end())?;
        let tolerance = Tolerance::try_new(
            (tolerance.absolute() / self.segments.len() as Real).max(Real::MIN_POSITIVE),
            tolerance.relative(),
            tolerance.angular(),
        )?;
        let mut cumulative = vec![0.0];
        let mut sum: Real = 0.0;
        let mut correction = 0.0;
        for segment in &self.segments {
            let length = segment.length(tolerance)?;
            if length <= 0.0 {
                return Err(invalid(
                    "cannot assign a length-based domain to a zero-length segment",
                ));
            }
            let next = sum + length;
            correction += if sum.abs() >= length.abs() {
                (sum - next) + length
            } else {
                (length - next) + sum
            };
            sum = next;
            cumulative.push(sum + correction);
        }
        let total = sum + correction;
        check_interval(0.0, total)?;
        let parameters = cumulative
            .into_iter()
            .map(|t| map_parameter(t, 0.0..=total, domain.clone()))
            .collect::<Result<_, _>>()?;
        Self::try_with_segment_domains(self.segments.clone(), parameters)
    }

    /// Trims only the first and last retained segments, leaving complete
    /// interior segments untouched and retaining the outer parameter domain.
    pub fn try_trimmed(&self, domain: RangeInclusive<Real>) -> Result<Self, GeometryError> {
        check_interval(*domain.start(), *domain.end())?;
        checked_parameter(*domain.start(), self.domain())?;
        checked_parameter(*domain.end(), self.domain())?;
        let first = self.segment_index(*domain.start(), CurveEvaluationSide::Right)?;
        let last = self.segment_index(*domain.end(), CurveEvaluationSide::Left)?;
        let mut segments = Vec::with_capacity(last - first + 1);
        let mut parameters = vec![*domain.start()];
        for index in first..=last {
            let start = self.parameters[index].max(*domain.start());
            let end = self.parameters[index + 1].min(*domain.end());
            let local =
                self.segment_parameter(index, start)?..=self.segment_parameter(index, end)?;
            let source = &self.segments[index];
            segments.push(if local == source.domain() {
                source.clone()
            } else {
                source.try_trimmed(local)?
            });
            parameters.push(end);
        }
        Self::try_with_segment_domains(segments, parameters)
    }

    pub fn try_split(&self, parameter: Real) -> Result<(Self, Self), GeometryError> {
        let domain = self.domain();
        if !parameter.is_finite() || parameter <= *domain.start() || parameter >= *domain.end() {
            return Err(GeometryError::InvalidCurveSplitParameter);
        }
        Ok((
            self.try_trimmed(*domain.start()..=parameter)?,
            self.try_trimmed(parameter..=*domain.end())?,
        ))
    }

    /// Flattens composites without changing their segment geometry or local
    /// domains. Later composites are shifted to follow the previous endpoint.
    pub fn concatenate(curves: &[Self]) -> Result<Self, GeometryError> {
        let count = curves.iter().try_fold(0_usize, |sum, curve| {
            sum.checked_add(curve.segments.len())
                .ok_or(invalid("too many segments"))
        })?;
        check_count(count)?;
        let mut segments = Vec::with_capacity(count);
        let mut parameters = vec![*curves[0].domain().start()];
        for curve in curves {
            let offset = *parameters.last().expect("the initial parameter exists");
            let end = append_end(offset, curve.domain())?;
            for (index, segment) in curve.segments.iter().enumerate() {
                segments.push(segment.clone());
                parameters.push(map_parameter(
                    curve.parameters[index + 1],
                    curve.domain(),
                    offset..=end,
                )?);
            }
        }
        Self::try_with_segment_domains(segments, parameters)
    }

    /// Converts to one exact piecewise NURBS curve. Full-order junction knots
    /// keep each segment's homogeneous scale independent: no averaging of
    /// endpoints, ratio of unrelated weights, or geometric fitting is needed.
    /// This preserves parameterization but need not produce Rhino's minimal
    /// control-point/knot representation.
    pub fn to_nurbs(&self) -> Result<NurbsCurve, GeometryError> {
        let degree = self
            .segments
            .iter()
            .map(NurbsCurve::degree)
            .max()
            .expect("segments exist");
        let mut controls = Vec::new();
        let mut knots = Vec::new();
        for (index, source) in self.segments.iter().enumerate() {
            let source = source.clamped_to_active_domain()?;
            let delta = degree - source.degree();
            let extra = source
                .spans()
                .count()
                .checked_mul(delta)
                .ok_or(invalid("NURBS conversion is too large"))?;
            let count = controls
                .len()
                .checked_add(source.control_points().len())
                .and_then(|n| n.checked_add(extra))
                .ok_or(invalid("NURBS conversion is too large"))?;
            if count > MAX_CONVERSION_CONTROLS {
                return Err(invalid("NURBS conversion is too large"));
            }
            let segment = source
                .try_change_degree(degree, false)?
                .try_reparameterized(self.segment_domain(index)?)?;
            controls.extend_from_slice(segment.control_points());
            if index == 0 {
                knots.extend_from_slice(segment.knots());
            } else {
                knots.extend_from_slice(&segment.knots()[degree + 1..]);
            }
        }
        NurbsCurve::try_new_rational(degree, controls, knots)
    }

    fn segment(&self, index: usize) -> Result<&NurbsCurve, GeometryError> {
        self.segments
            .get(index)
            .ok_or(invalid("segment index is out of range"))
    }
}

fn invalid(context: &'static str) -> GeometryError {
    GeometryError::InvalidPolyCurve { context }
}

fn check_count(count: usize) -> Result<(), GeometryError> {
    if count == 0 || count > MAX_POLYCURVE_SEGMENTS {
        Err(invalid("segment count is outside the supported range"))
    } else {
        Ok(())
    }
}

fn check_interval(start: Real, end: Real) -> Result<(), GeometryError> {
    if !start.is_finite() || !end.is_finite() || end <= start || !(end - start).is_finite() {
        Err(invalid(
            "parameter intervals must have positive finite length",
        ))
    } else {
        Ok(())
    }
}

fn append_end(start: Real, domain: RangeInclusive<Real>) -> Result<Real, GeometryError> {
    check_interval(*domain.start(), *domain.end())?;
    let span = *domain.end() - *domain.start();
    let end = if start == *domain.start() {
        *domain.end()
    } else {
        start + span
    };
    check_interval(start, end)?;
    // Addition rounds at the destination offset, not at the small source
    // span. Requiring relative accuracy in the latter rejects valid short
    // closing segments. Collapsed/overflowed intervals are rejected above.
    if ((end - start) - span).abs() > Real::EPSILON.sqrt() * span {
        return Err(invalid(
            "appended parameter span loses significant precision at this offset",
        ));
    }
    Ok(end)
}

fn checked_parameter(parameter: Real, domain: RangeInclusive<Real>) -> Result<(), GeometryError> {
    require_finite([parameter], "polycurve parameter")?;
    if domain.contains(&parameter) {
        Ok(())
    } else {
        Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start: *domain.start(),
            domain_end: *domain.end(),
        })
    }
}

fn map_parameter(
    value: Real,
    source: RangeInclusive<Real>,
    target: RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    checked_parameter(value, source.clone())?;
    if value == *source.start() {
        return Ok(*target.start());
    }
    if value == *source.end() {
        return Ok(*target.end());
    }
    let from_start = value - *source.start();
    let from_end = *source.end() - value;
    let numerator = *target.end() - *target.start();
    let denominator = *source.end() - *source.start();
    let result = if from_start <= from_end {
        *target.start() + scaled_ratio(from_start, numerator, denominator)?
    } else {
        *target.end() - scaled_ratio(from_end, numerator, denominator)?
    };
    require_finite([result], "polycurve parameter mapping")?;
    Ok(result.clamp(*target.start(), *target.end()))
}

fn scaled_ratio(value: Real, numerator: Real, denominator: Real) -> Result<Real, GeometryError> {
    if value == 0.0 {
        return Ok(0.0);
    }
    let ratio = numerator / denominator;
    let product = value * numerator;
    let quotient = value / denominator;
    let orders = [
        (ratio, value * ratio),
        (product, product / denominator),
        (quotient, quotient * numerator),
    ];
    // Prefer a normal intermediate so its subnormal rounding is not magnified
    // later. Try every ordering before rejecting a representable final value.
    let result = orders
        .iter()
        .find(|(intermediate, result)| {
            intermediate.is_normal() && result.is_finite() && *result != 0.0
        })
        .or_else(|| orders.iter().find(|(_, result)| result.is_finite()))
        .map(|(_, result)| *result)
        .ok_or(GeometryError::NonFinite {
            context: "polycurve derivative",
        })?;
    require_finite([result], "polycurve derivative")?;
    Ok(result)
}
