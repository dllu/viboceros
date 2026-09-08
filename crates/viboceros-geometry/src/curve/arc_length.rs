//! Parameter-bearing arc-length sampling, inversion, and repeated-query tables.

use super::{CurveRef, CurveSample};
use crate::{
    Curve3, GeometryError, ParameterSide, Point3, Real, Tolerance, UnitVector3,
    integration::integrate_adaptive, parameter::scaled_ratio, require_finite,
};
use std::f64::consts::FRAC_PI_2;

// Bounds the optional cache to 16 MiB of parameter/length pairs. Counts
// include both endpoints of every variable-speed span, not just one span.
const MAX_LOOKUP_NODES: usize = 1_048_576;

fn affordable_lookup_subdivisions(
    variable_spans: usize,
    preferred: usize,
) -> Result<Option<usize>, GeometryError> {
    if preferred == 0 {
        return Err(GeometryError::InvalidArcLengthLookupBudget {
            maximum: MAX_LOOKUP_NODES,
        });
    }
    if variable_spans == 0 {
        return Ok(None);
    }
    let affordable = (MAX_LOOKUP_NODES / variable_spans).saturating_sub(1);
    Ok((affordable > 0).then_some(preferred.min(affordable)))
}

fn checked_lookup_nodes_per_span(
    variable_spans: usize,
    subdivisions: usize,
) -> Result<usize, GeometryError> {
    let invalid = || GeometryError::InvalidArcLengthLookupBudget {
        maximum: MAX_LOOKUP_NODES,
    };
    let nodes = subdivisions.checked_add(1).ok_or_else(invalid)?;
    let total = variable_spans.checked_mul(nodes).ok_or_else(invalid)?;
    if subdivisions == 0 || nodes > MAX_LOOKUP_NODES || total > MAX_LOOKUP_NODES {
        return Err(invalid());
    }
    Ok(nodes)
}

#[derive(Clone, Copy, Debug)]
enum LinearSpan {
    Line,
    Polyline(usize),
    CompositeLine(usize),
    CompositePolyline(usize, usize),
}

type RawSpan = (Real, Real, Real, bool, Option<LinearSpan>);

#[derive(Clone, Copy, Debug)]
struct ParameterSpan {
    start: Real,
    end: Real,
    length: Real,
    cumulative_start: Real,
    cumulative_end: Real,
    variable_speed: bool,
    linear: Option<LinearSpan>,
}

pub(crate) struct ArcLengthSampler<'a> {
    source: CurveRef<'a>,
    // All internal spans and lookup nodes belong to this frame when present.
    // Only public parameter inputs/outputs are mapped to/from the source.
    normalized: Option<Curve3>,
    spans: Vec<ParameterSpan>,
    lookup_tables: Vec<Vec<ArcLengthLookupNode>>,
    total_length: Real,
    tolerance: Tolerance,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ArcLengthLookupNode {
    parameter: Real,
    length: Real,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ArcLengthKink {
    pub(crate) distance: Real,
    pub(crate) incoming_tangent: UnitVector3,
    pub(crate) outgoing_tangent: UnitVector3,
}

impl<'a> ArcLengthSampler<'a> {
    pub(crate) fn try_new(
        curve: CurveRef<'a>,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let normalized = match curve {
            CurveRef::NurbsCurve(c) if c.domain() != (0.0..=1.0) => {
                Some(Curve3::NurbsCurve(c.for_integration()?.into_owned()))
            }
            CurveRef::PolyCurve(c) => match c.for_integration()? {
                std::borrow::Cow::Borrowed(_) => None,
                std::borrow::Cow::Owned(c) => Some(Curve3::PolyCurve(c)),
            },
            CurveRef::Polyline(c) => match c.for_integration()? {
                std::borrow::Cow::Borrowed(_) => None,
                std::borrow::Cow::Owned(c) => Some(Curve3::Polyline(c)),
            },
            _ => None,
        };
        let integration_curve = normalized.as_ref().map(Curve3::as_ref).unwrap_or(curve);
        let raw_spans = raw_spans(integration_curve, tolerance)?;
        let mut spans = Vec::with_capacity(raw_spans.len());
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end, length, variable_speed, linear) in raw_spans {
            require_finite([start, end, length], "curve arc-length span")?;
            if start >= end || length < 0.0 {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            if length == 0.0 {
                continue;
            }
            let cumulative_start = sum + correction;
            neumaier_add(&mut sum, &mut correction, length);
            let cumulative_end = sum + correction;
            spans.push(ParameterSpan {
                start,
                end,
                length,
                cumulative_start,
                cumulative_end,
                variable_speed,
                linear,
            });
        }
        let total_length = sum + correction;
        require_finite([total_length], "curve arc length")?;
        if spans.is_empty() || total_length <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "arc-length curve",
            });
        }
        Ok(Self {
            source: curve,
            normalized,
            lookup_tables: vec![Vec::new(); spans.len()],
            spans,
            total_length,
            tolerance,
        })
    }

    pub(crate) fn total_length(&self) -> Real {
        self.total_length
    }

    fn curve(&self) -> CurveRef<'_> {
        self.normalized
            .as_ref()
            .map(Curve3::as_ref)
            .unwrap_or(self.source)
    }

    fn source_parameter(&self, parameter: Real) -> Result<Real, GeometryError> {
        if self.normalized.is_some() {
            self.source.parameter_at(parameter)
        } else {
            Ok(parameter)
        }
    }

    // The caller has already checked membership in the source domain.
    fn integration_parameter(&self, parameter: Real) -> Real {
        if self.normalized.is_none() {
            return parameter;
        }
        let domain = self.source.domain();
        let (start, end) = (*domain.start(), *domain.end());
        if parameter == start {
            return 0.0;
        }
        if parameter == end {
            return 1.0;
        }
        let width = end - start;
        if width.is_finite() {
            (parameter - start) / width
        } else {
            let scale = start.abs().max(end.abs());
            (parameter / scale - start / scale) / (end / scale - start / scale)
        }
    }

    pub(crate) fn distance_at_parameter(&self, parameter: Real) -> Result<Real, GeometryError> {
        let domain = self.source.domain();
        if !parameter.is_finite() || !domain.contains(&parameter) {
            return Err(GeometryError::ParameterOutOfDomain {
                parameter,
                domain_start: *domain.start(),
                domain_end: *domain.end(),
            });
        }
        let parameter = self.integration_parameter(parameter);
        let index = self
            .spans
            .partition_point(|s| s.end < parameter)
            .min(self.spans.len() - 1);
        let span = self.spans[index];
        if parameter <= span.start {
            return Ok(span.cumulative_start);
        }
        if parameter >= span.end {
            return Ok(span.cumulative_end);
        }
        let partial = if span.variable_speed {
            let table = &self.lookup_tables[index];
            let table_length = table.last().map_or(span.length, |node| node.length);
            let integration_tolerance = scaled_ratio(
                numerical_distance_tolerance(span.length, self.tolerance),
                table_length,
                span.length,
            )?;
            let node = table
                .get(
                    table
                        .partition_point(|n| n.parameter <= parameter)
                        .saturating_sub(1),
                )
                .copied()
                .unwrap_or(ArcLengthLookupNode {
                    parameter: span.start,
                    length: 0.0,
                });
            let partial = node.length
                + if node.parameter == parameter {
                    0.0
                } else {
                    self.partial_parameter_length(node.parameter, parameter, integration_tolerance)?
                };
            // Prefix integration and full-span integration need not round to
            // the same total. Use the inverse query's canonical span scale.
            scaled_ratio(partial, span.length, table_length)?.clamp(0.0, span.length)
        } else {
            span.length * ((parameter - span.start) / (span.end - span.start))
        };
        Ok(span.cumulative_start + partial)
    }

    pub(crate) fn natural_break_distances(&self) -> impl Iterator<Item = Real> + '_ {
        self.spans
            .iter()
            .take(self.spans.len().saturating_sub(1))
            .map(|span| span.cumulative_end)
    }

    /// Precomputes adaptive-integration prefix brackets that make repeated
    /// arc-length inversions substantially cheaper. Ordinary one-shot curve
    /// queries avoid this setup cost; adaptive algorithms opt in explicitly.
    pub(crate) fn prepare_repeated_sampling(
        &mut self,
        subdivisions_per_span: usize,
    ) -> Result<(), GeometryError> {
        let nodes_per_span = checked_lookup_nodes_per_span(
            self.spans.iter().filter(|span| span.variable_speed).count(),
            subdivisions_per_span,
        )?;
        let mut tables = Vec::with_capacity(self.spans.len());
        for span in &self.spans {
            if !span.variable_speed {
                tables.push(Vec::new());
                continue;
            }
            let mut nodes = Vec::with_capacity(nodes_per_span);
            nodes.push(ArcLengthLookupNode {
                parameter: span.start,
                length: 0.0,
            });
            let mut sum = 0.0;
            let mut correction = 0.0;
            let absolute_tolerance = numerical_distance_tolerance(span.length, self.tolerance)
                / subdivisions_per_span as Real;
            let mut previous = span.start;
            for division in 1..=subdivisions_per_span {
                let parameter = if division == subdivisions_per_span {
                    span.end
                } else {
                    stable_lerp(
                        span.start,
                        span.end,
                        division as Real / subdivisions_per_span as Real,
                    )
                };
                let length = integrate_adaptive(
                    previous,
                    parameter,
                    absolute_tolerance.max(Real::MIN_POSITIVE),
                    self.tolerance.relative(),
                    |value| self.speed(value),
                )?;
                neumaier_add(&mut sum, &mut correction, length);
                nodes.push(ArcLengthLookupNode {
                    parameter,
                    length: sum + correction,
                });
                previous = parameter;
            }
            let table_length = sum + correction;
            if !table_length.is_finite() || table_length <= 0.0 {
                return Err(GeometryError::NumericalIntegrationDidNotConverge);
            }
            tables.push(nodes);
        }
        self.lookup_tables = tables;
        Ok(())
    }

    /// Best-effort cache sizing for algorithms whose correctness does not
    /// depend on caching. Reduce density to fit the aggregate node budget;
    /// retain uncached integration if even two nodes per span cannot fit.
    /// Numerical errors still propagate and existing tables remain intact.
    pub(crate) fn prepare_budgeted_repeated_sampling(
        &mut self,
        preferred_subdivisions: usize,
    ) -> Result<(), GeometryError> {
        let count = self.spans.iter().filter(|span| span.variable_speed).count();
        if let Some(subdivisions) = affordable_lookup_subdivisions(count, preferred_subdivisions)? {
            self.prepare_repeated_sampling(subdivisions)?;
        }
        Ok(())
    }

    /// Returns arc-length locations and one-sided tangents where adjacent
    /// natural spans meet at an angle larger than `angle_tolerance_radians`.
    pub(crate) fn kinks(
        &self,
        angle_tolerance_radians: Real,
    ) -> Result<Vec<ArcLengthKink>, GeometryError> {
        require_finite([angle_tolerance_radians], "curve kink angle tolerance")?;
        if !(0.0..=std::f64::consts::PI).contains(&angle_tolerance_radians) {
            return Err(GeometryError::InvalidCurveFitAngleTolerance);
        }

        let mut distances = Vec::new();
        for spans in self.spans.windows(2) {
            let left = spans[0];
            let right = spans[1];
            let left_tangent = self
                .curve()
                .evaluate_with_tangent_on_side(left.end, ParameterSide::Left)?
                .tangent();
            let right_tangent = self
                .curve()
                .evaluate_with_tangent_on_side(right.start, ParameterSide::Right)?
                .tangent();
            let dot = left_tangent
                .as_vector()
                .dot(right_tangent.as_vector())?
                .clamp(-1.0, 1.0);
            let sine = left_tangent
                .as_vector()
                .cross(right_tangent.as_vector())?
                .length()?;
            if sine.atan2(dot) > angle_tolerance_radians {
                distances.push(ArcLengthKink {
                    distance: left.cumulative_end,
                    incoming_tangent: left_tangent,
                    outgoing_tangent: right_tangent,
                });
            }
        }
        Ok(distances)
    }

    fn parameter_start(&self) -> Real {
        self.spans[0].start
    }

    pub(crate) fn point_at_distance(&self, distance: Real) -> Result<Point3, GeometryError> {
        self.point_at_distance_impl(distance, None)
    }

    pub(super) fn point_at_distance_with_fractional_tolerance(
        &self,
        distance: Real,
        fractional_tolerance: Real,
    ) -> Result<Point3, GeometryError> {
        self.point_at_distance_impl(distance, Some(fractional_tolerance))
    }

    fn point_at_distance_impl(
        &self,
        distance: Real,
        fractional_tolerance: Option<Real>,
    ) -> Result<Point3, GeometryError> {
        let parameter = self.parameter_at_distance_impl(distance, fractional_tolerance)?;
        if let Some((line, fraction)) = self.linear_distance_location(distance) {
            return line.point_at(fraction);
        }
        if distance == self.total_length {
            self.source.end_point()
        } else {
            self.evaluate(parameter)
        }
    }

    pub(crate) fn parameter_at_distance(&self, distance: Real) -> Result<Real, GeometryError> {
        self.source_parameter(self.parameter_at_distance_impl(distance, None)?)
    }

    fn parameter_at_distance_impl(
        &self,
        distance: Real,
        fractional_tolerance: Option<Real>,
    ) -> Result<Real, GeometryError> {
        require_finite([distance], "curve arc-length distance")?;
        if distance < 0.0 || distance > self.total_length {
            return Err(GeometryError::ArcLengthOutOfDomain {
                distance,
                length: self.total_length,
            });
        }
        if distance == 0.0 {
            return Ok(self.parameter_start());
        }
        if distance == self.total_length {
            return Ok(self.spans.last().expect("a sampler has spans").end);
        }

        let span_index = self
            .spans
            .partition_point(|span| span.cumulative_end < distance)
            .min(self.spans.len() - 1);
        let span = self.spans[span_index];
        let local_distance = (distance - span.cumulative_start).clamp(0.0, span.length);
        if local_distance == 0.0 {
            return Ok(span.start);
        }
        if local_distance == span.length {
            return Ok(span.end);
        }
        if !span.variable_speed {
            let fraction = local_distance / span.length;
            return Ok(stable_lerp(span.start, span.end, fraction));
        }

        let distance_tolerance = fractional_tolerance
            .map(|fractional| {
                (fractional * self.total_length.abs())
                    .max(64.0 * Real::EPSILON * self.total_length.abs())
                    .max(Real::MIN_POSITIVE)
            })
            .unwrap_or_else(|| numerical_distance_tolerance(span.length, self.tolerance));
        self.parameter_at_span_distance(span_index, span, local_distance, distance_tolerance)
    }

    pub(crate) fn sample_at_distance(&self, distance: Real) -> Result<CurveSample, GeometryError> {
        let parameter = self.parameter_at_distance_impl(distance, None)?;
        let mut sample = if let Some((line, fraction)) = self.linear_distance_location(distance) {
            CurveRef::Line(&line).evaluate_with_tangent(fraction)?
        } else {
            self.curve().evaluate_with_tangent(parameter)?
        };
        sample.parameter = self.source_parameter(parameter)?;
        if distance == self.total_length {
            sample.point = self.source.end_point()?;
        }
        Ok(sample)
    }

    // Distance has already been validated. Sample linear geometry from its
    // local distance fraction, never from a rounded native parameter. At an
    // exact junction use the outgoing segment, matching right-sided tangents.
    fn linear_distance_location(&self, distance: Real) -> Option<(crate::LineSegment, Real)> {
        let index = self
            .spans
            .partition_point(|span| span.cumulative_end <= distance)
            .min(self.spans.len() - 1);
        let span = self.spans[index];
        let (curve, edge) = match span.linear? {
            LinearSpan::Line => (self.curve(), None),
            LinearSpan::Polyline(edge) => (self.curve(), Some(edge)),
            LinearSpan::CompositeLine(segment) => {
                let CurveRef::PolyCurve(curve) = self.curve() else {
                    unreachable!()
                };
                (curve.segments()[segment].as_ref(), None)
            }
            LinearSpan::CompositePolyline(segment, edge) => {
                let CurveRef::PolyCurve(curve) = self.curve() else {
                    unreachable!()
                };
                (curve.segments()[segment].as_ref(), Some(edge))
            }
        };
        let (start, end) = match (curve, edge) {
            (CurveRef::Line(line), None) => (line.start(), line.end()),
            (CurveRef::Polyline(curve), Some(edge)) => {
                (curve.vertices()[edge], curve.vertices()[edge + 1])
            }
            _ => unreachable!("linear span metadata matches its source geometry"),
        };
        let fraction = ((distance - span.cumulative_start) / span.length).clamp(0.0, 1.0);
        Some((
            crate::LineSegment::from_validated(start, end, [0.0, 1.0]),
            fraction,
        ))
    }

    fn parameter_at_span_distance(
        &self,
        span_index: usize,
        span: ParameterSpan,
        target: Real,
        distance_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        let table = &self.lookup_tables[span_index];
        let (inversion_target, distance_tolerance) = if table.is_empty() {
            (target, distance_tolerance)
        } else {
            let table_length = table.last().expect("a lookup table has an end").length;
            (
                scaled_ratio(target, table_length, span.length)?,
                scaled_ratio(distance_tolerance, table_length, span.length)?,
            )
        };
        let (prefix_parameter, prefix_length, mut lower, mut upper, mut parameter) =
            if table.is_empty() {
                (
                    span.start,
                    0.0,
                    span.start,
                    span.end,
                    stable_lerp(span.start, span.end, target / span.length),
                )
            } else {
                let upper_index = table
                    .partition_point(|node| node.length < inversion_target)
                    .clamp(1, table.len() - 1);
                let lower_node = table[upper_index - 1];
                let upper_node = table[upper_index];
                if inversion_target == lower_node.length {
                    return Ok(lower_node.parameter);
                }
                if inversion_target == upper_node.length {
                    return Ok(upper_node.parameter);
                }
                let fraction = (inversion_target - lower_node.length)
                    / (upper_node.length - lower_node.length);
                (
                    lower_node.parameter,
                    lower_node.length,
                    lower_node.parameter,
                    upper_node.parameter,
                    stable_lerp(lower_node.parameter, upper_node.parameter, fraction),
                )
            };

        for _ in 0..80 {
            let length = prefix_length
                + self.partial_parameter_length(prefix_parameter, parameter, distance_tolerance)?;
            let residual = length - inversion_target;
            if residual.abs() <= distance_tolerance {
                return Ok(parameter);
            }
            if residual < 0.0 {
                lower = parameter;
            } else {
                upper = parameter;
            }

            let midpoint = lower * 0.5 + upper * 0.5;
            if midpoint <= lower || midpoint >= upper {
                return Ok(midpoint.clamp(span.start, span.end));
            }
            let speed = self.speed(parameter)?;
            let newton = (speed > 0.0)
                .then(|| parameter - residual / speed)
                .filter(|candidate| {
                    candidate.is_finite() && *candidate > lower && *candidate < upper
                });
            parameter = newton.unwrap_or(midpoint);
        }
        Err(GeometryError::NumericalIntegrationDidNotConverge)
    }

    fn partial_parameter_length(
        &self,
        start: Real,
        parameter: Real,
        absolute_tolerance: Real,
    ) -> Result<Real, GeometryError> {
        if parameter <= start {
            return Ok(0.0);
        }
        integrate_adaptive(
            start,
            parameter,
            absolute_tolerance,
            self.tolerance.relative(),
            |value| self.speed(value),
        )
    }

    fn speed(&self, parameter: Real) -> Result<Real, GeometryError> {
        self.curve().evaluate_with_derivative(parameter)?.1.length()
    }

    fn evaluate(&self, parameter: Real) -> Result<Point3, GeometryError> {
        self.curve().evaluate(parameter)
    }
}

fn raw_spans(curve: CurveRef<'_>, tolerance: Tolerance) -> Result<Vec<RawSpan>, GeometryError> {
    Ok(match curve {
        CurveRef::Line(line) => vec![(
            *line.domain().start(),
            *line.domain().end(),
            line.length()?,
            false,
            Some(LinearSpan::Line),
        )],
        CurveRef::Circle(circle) => {
            let quadrant_length = circle.length()? * 0.25;
            (0..4)
                .map(|quadrant| {
                    Ok((
                        curve.parameter_at(quadrant as Real * 0.25)?,
                        curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        quadrant_length,
                        false,
                        None,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Arc(arc) => vec![(
            *arc.domain().start(),
            *arc.domain().end(),
            arc.length()?,
            false,
            None,
        )],
        CurveRef::Ellipse(ellipse) => {
            let quadrant_length = integrate_speed(0.0, FRAC_PI_2, tolerance, |angle| {
                let (sine, cosine) = angle.sin_cos();
                let speed = (ellipse.radius_x() * sine).hypot(ellipse.radius_y() * cosine);
                require_finite([speed], "ellipse speed")?;
                Ok(speed)
            })?;
            (0..4)
                .map(|quadrant| {
                    Ok((
                        curve.parameter_at(quadrant as Real * 0.25)?,
                        curve.parameter_at((quadrant + 1) as Real * 0.25)?,
                        quadrant_length,
                        true,
                        None,
                    ))
                })
                .collect::<Result<Vec<_>, GeometryError>>()?
        }
        CurveRef::Polyline(polyline) => polyline
            .segments()
            .enumerate()
            .map(|(index, segment)| {
                Ok((
                    polyline.parameters()[index],
                    polyline.parameters()[index + 1],
                    segment.length()?,
                    false,
                    Some(LinearSpan::Polyline(index)),
                ))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::NurbsCurve(curve) => curve
            .spans()
            .map(|(start, end)| {
                let length = integrate_speed(start, end, tolerance, |parameter| {
                    curve.derivative_at(parameter)?.length()
                })?;
                Ok((start, end, length, true, None))
            })
            .collect::<Result<Vec<_>, GeometryError>>()?,
        CurveRef::PolyCurve(curve) => {
            let mut spans = Vec::new();
            for (index, segment) in curve.segments().iter().enumerate() {
                for (start, end, length, variable_speed, linear) in
                    raw_spans(segment.as_ref(), tolerance)?
                {
                    spans.push((
                        curve.polycurve_parameter(index, start)?,
                        curve.polycurve_parameter(index, end)?,
                        length,
                        variable_speed,
                        linear.map(|linear| match linear {
                            LinearSpan::Line => LinearSpan::CompositeLine(index),
                            LinearSpan::Polyline(edge) => {
                                LinearSpan::CompositePolyline(index, edge)
                            }
                            _ => unreachable!("polycurve leaves are not nested composites"),
                        }),
                    ));
                }
            }
            spans
        }
    })
}

fn integrate_speed(
    start: Real,
    end: Real,
    tolerance: Tolerance,
    mut speed: impl FnMut(Real) -> Result<Real, GeometryError>,
) -> Result<Real, GeometryError> {
    let coarse = integrate_adaptive(
        start,
        end,
        tolerance.absolute(),
        tolerance.relative(),
        &mut speed,
    )?;
    let tighter = numerical_distance_tolerance(coarse, tolerance);
    if tighter < tolerance.absolute() {
        integrate_adaptive(start, end, tighter, tolerance.relative(), speed)
    } else {
        Ok(coarse)
    }
}

fn numerical_distance_tolerance(length: Real, tolerance: Tolerance) -> Real {
    let relative = tolerance.relative() * length.abs();
    let roundoff = 64.0 * Real::EPSILON * length.abs();
    tolerance
        .absolute()
        .min(relative)
        .max(roundoff)
        .max(Real::MIN_POSITIVE)
}

fn stable_lerp(start: Real, end: Real, fraction: Real) -> Real {
    start.mul_add(1.0 - fraction, end * fraction)
}

fn neumaier_add(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NurbsCurve, Vector3};

    #[test]
    fn optional_cache_density_adapts_without_exceeding_aggregate_budget() {
        for (spans, preferred, expected) in [
            (0, 32, None),
            (1, 32, Some(32)),
            (MAX_LOOKUP_NODES / 32, 32, Some(31)),
            (MAX_LOOKUP_NODES / 2, 32, Some(1)),
            (MAX_LOOKUP_NODES / 2 + 1, 32, None),
            (usize::MAX, 32, None),
            (1, usize::MAX, Some(MAX_LOOKUP_NODES - 1)),
        ] {
            let actual = affordable_lookup_subdivisions(spans, preferred).unwrap();
            assert_eq!(actual, expected);
            if let Some(subdivisions) = actual {
                checked_lookup_nodes_per_span(spans, subdivisions).unwrap();
            }
        }
        assert!(affordable_lookup_subdivisions(0, 0).is_err());
        assert!(affordable_lookup_subdivisions(1, 0).is_err());
    }

    #[test]
    fn invalid_lookup_request_preserves_existing_tables() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let curve = curve.try_insert_knot(0.5, 1).unwrap();
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        sampler.prepare_repeated_sampling(16).unwrap();
        let original = sampler.lookup_tables.clone();
        let sample = sampler
            .sample_at_distance(sampler.total_length() * 0.37)
            .unwrap();
        for invalid in [0, usize::MAX, MAX_LOOKUP_NODES, MAX_LOOKUP_NODES / 2] {
            assert!(sampler.prepare_repeated_sampling(invalid).is_err());
            assert_eq!(sampler.lookup_tables, original);
            assert_eq!(
                sampler
                    .sample_at_distance(sampler.total_length() * 0.37)
                    .unwrap(),
                sample
            );
        }
    }

    #[test]
    fn lookup_budget_checks_aggregate_counts_and_overflow_without_allocating() {
        assert_eq!(checked_lookup_nodes_per_span(0, 32).unwrap(), 33);
        assert_eq!(
            checked_lookup_nodes_per_span(1, MAX_LOOKUP_NODES - 1).unwrap(),
            MAX_LOOKUP_NODES
        );
        assert_eq!(
            checked_lookup_nodes_per_span(2, MAX_LOOKUP_NODES / 2 - 1).unwrap(),
            MAX_LOOKUP_NODES / 2
        );
        for (spans, subdivisions) in [
            (1, 0),
            (0, usize::MAX),
            (1, MAX_LOOKUP_NODES),
            (2, MAX_LOOKUP_NODES / 2),
            (usize::MAX, 1),
        ] {
            assert!(matches!(
                checked_lookup_nodes_per_span(spans, subdivisions),
                Err(GeometryError::InvalidArcLengthLookupBudget {
                    maximum: MAX_LOOKUP_NODES
                })
            ));
        }
    }

    #[test]
    fn lookup_distance_and_inverse_share_the_same_length_scale() {
        for scale in [1e-150, 1., 1e150] {
            let curve = NurbsCurve::try_new(
                2,
                vec![
                    Point3::try_new(0., 0., 0.).unwrap(),
                    Point3::try_new(0.5 * scale, scale, 0.).unwrap(),
                    Point3::try_new(scale, 0., 0.).unwrap(),
                ],
                vec![0., 0., 0., 1., 1., 1.],
            )
            .unwrap();
            let tolerance = Tolerance::try_new(0.01 * scale, 0.01, 1e-10).unwrap();
            let mut sampler =
                ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), tolerance).unwrap();
            sampler.prepare_repeated_sampling(16).unwrap();
            let half_length = sampler.total_length() * 0.5;
            // Symmetry gives an exact station even with loose length tolerance.
            assert!((sampler.parameter_at_distance(half_length).unwrap() - 0.5).abs() < 1e-12);
            assert!(
                (sampler.distance_at_parameter(0.5).unwrap() / scale - half_length / scale).abs()
                    < 1e-12
            );
            // Every exact prefix node must use the same public distance unit in
            // both directions, not only the symmetric midpoint.
            let table = &sampler.lookup_tables[0];
            let table_length = table.last().unwrap().length;
            for node in table {
                let expected = (node.length / table_length) * sampler.total_length();
                let actual = sampler.distance_at_parameter(node.parameter).unwrap();
                assert!((actual / scale - expected / scale).abs() < 1e-12);
                assert!(
                    (sampler.parameter_at_distance(actual).unwrap() - node.parameter).abs() < 1e-12
                );
            }
            for parameter in [0.013, 0.137, 0.371, 0.499] {
                let left = sampler.distance_at_parameter(parameter).unwrap();
                let right = sampler.distance_at_parameter(1. - parameter).unwrap();
                assert!(
                    (left / scale + right / scale - sampler.total_length() / scale).abs() < 1e-12
                );
            }
        }
    }

    #[test]
    fn normalized_sampler_preserves_multispan_kinks_and_native_parameters() {
        let curve = NurbsCurve::try_new(
            1,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
                Point3::try_new(1., 1., 0.).unwrap(),
            ],
            vec![0., 0., 0.5, 1., 1.],
        )
        .unwrap()
        .try_reparameterized(-f64::MAX..=f64::MAX)
        .unwrap();
        let mut sampler =
            ArcLengthSampler::try_new(CurveRef::NurbsCurve(&curve), Tolerance::DEFAULT).unwrap();
        sampler.prepare_repeated_sampling(8).unwrap();
        assert!((sampler.total_length() - 2.).abs() < 1e-12);
        let kinks = sampler.kinks(0.1).unwrap();
        assert_eq!(kinks.len(), 1);
        assert!((kinks[0].distance - 1.).abs() < 1e-12);
        assert_eq!(
            kinks[0].incoming_tangent.as_vector().to_array(),
            [1., 0., 0.]
        );
        assert_eq!(
            kinks[0].outgoing_tangent.as_vector().to_array(),
            [0., 1., 0.]
        );
        let sample = sampler.sample_at_distance(1.).unwrap();
        assert_eq!(sample.parameter(), 0.);
        assert_eq!(sample.point(), Point3::try_new(1., 0., 0.).unwrap());
        for distance in [0., 0.5, 1., 1.5, 2.] {
            let parameter = sampler.parameter_at_distance(distance).unwrap();
            assert!((sampler.distance_at_parameter(parameter).unwrap() - distance).abs() < 1e-12);
        }
        for invalid in [f64::NAN, f64::INFINITY] {
            assert!(sampler.distance_at_parameter(invalid).is_err());
            assert!(sampler.sample_at_distance(invalid).is_err());
        }
    }

    #[test]
    fn composite_linear_samples_preserve_tiny_leaf_intervals() {
        use crate::{CurveSegment3, PolyCurve3, Polyline3};
        let leaf = Polyline3::try_with_parameters(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            vec![0., f64::from_bits(1), 1.],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let lines = PolyCurve3::try_with_segment_domains(
            leaf.segments().map(CurveSegment3::Line).collect::<Vec<_>>(),
            vec![0., f64::from_bits(1), 1.],
        )
        .unwrap();
        let composite =
            PolyCurve3::try_with_segment_domains(vec![CurveSegment3::Polyline(leaf)], vec![0., 1.])
                .unwrap();
        for curve in [lines, composite] {
            let sampler =
                ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), Tolerance::DEFAULT).unwrap();
            for distance in [0.25, 0.5, 0.75, 1.25, 1.5, 1.75] {
                let expected = if distance < 1. {
                    Point3::try_new(distance, 0., 0.).unwrap()
                } else {
                    Point3::try_new(1., distance - 1., 0.).unwrap()
                };
                assert_eq!(sampler.point_at_distance(distance).unwrap(), expected);
                assert_eq!(
                    sampler.sample_at_distance(distance).unwrap().point(),
                    expected
                );
            }
            assert_eq!(
                sampler
                    .sample_at_distance(1.)
                    .unwrap()
                    .tangent()
                    .as_vector()
                    .to_array(),
                [0., 1., 0.]
            );
        }
    }

    #[test]
    fn polyline_distance_samples_do_not_round_through_tiny_native_spans() {
        let curve = crate::Polyline3::try_with_parameters(
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
                .map(|p| Point3::try_from(p).unwrap())
                .to_vec(),
            vec![0., f64::from_bits(1), 1.],
            Tolerance::DEFAULT,
        )
        .unwrap();
        let sampler =
            ArcLengthSampler::try_new(CurveRef::Polyline(&curve), Tolerance::DEFAULT).unwrap();
        for distance in [0.25, 0.5, 0.75] {
            let expected = Point3::try_new(distance, 0., 0.).unwrap();
            assert_eq!(sampler.point_at_distance(distance).unwrap(), expected);
            let sample = sampler.sample_at_distance(distance).unwrap();
            assert_eq!(sample.point(), expected);
            assert_eq!(sample.tangent().as_vector().to_array(), [1., 0., 0.]);
            assert_eq!(
                sample.parameter(),
                sampler.parameter_at_distance(distance).unwrap()
            );
        }
        assert_eq!(
            sampler
                .sample_at_distance(1.)
                .unwrap()
                .tangent()
                .as_vector()
                .to_array(),
            [0., 1., 0.]
        );
    }

    #[test]
    fn polyline_division_preserves_points_on_narrow_parameter_domains() {
        use crate::{CurveSegment3, PolyCurve3, Polyline3};
        let vertices = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]]
            .map(|p| Point3::try_from(p).unwrap())
            .to_vec();
        for parameters in [
            vec![0., f64::from_bits(1), f64::from_bits(2)],
            vec![
                1.,
                f64::from_bits(1_f64.to_bits() + 1),
                f64::from_bits(1_f64.to_bits() + 2),
            ],
            vec![-1e200, 0., 1e200],
        ] {
            let curve = Polyline3::try_with_parameters(
                vertices.clone(),
                parameters.clone(),
                Tolerance::DEFAULT,
            )
            .unwrap();
            let actual = CurveRef::Polyline(&curve)
                .sample_equal_length_points(4, true, Tolerance::DEFAULT)
                .unwrap();
            let expected = [
                [0., 0., 0.],
                [0.5, 0., 0.],
                [1., 0., 0.],
                [1., 0.5, 0.],
                [1., 1., 0.],
            ]
            .map(|p| Point3::try_from(p).unwrap());
            assert_eq!(actual, expected, "{parameters:?}");
            let composite = PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::Polyline(curve.clone())],
                vec![0., 1.],
            )
            .unwrap();
            assert_eq!(
                CurveRef::PolyCurve(&composite)
                    .sample_equal_length_points(4, true, Tolerance::DEFAULT)
                    .unwrap(),
                expected
            );
            let sampler =
                ArcLengthSampler::try_new(CurveRef::Polyline(&curve), Tolerance::DEFAULT).unwrap();
            assert_eq!(sampler.parameter_at_distance(1.).unwrap(), parameters[1]);
            assert_eq!(sampler.distance_at_parameter(parameters[1]).unwrap(), 1.);
            assert_eq!(sampler.kinks(0.1).unwrap().len(), 1);
        }
    }

    #[test]
    fn normalized_polycurve_sampling_retains_junction_tangents() {
        use crate::{CurveSegment3, LineSegment, PolyCurve3};
        let points =
            [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.]].map(|p| Point3::try_from(p).unwrap());
        let segments = points
            .windows(2)
            .map(|p| {
                CurveSegment3::Line(LineSegment::try_new(p[0], p[1], Tolerance::DEFAULT).unwrap())
            })
            .collect::<Vec<_>>();
        for parameters in [
            vec![0., f64::from_bits(1), f64::from_bits(2)],
            vec![-1e200, 0., 1e200],
        ] {
            let curve =
                PolyCurve3::try_with_segment_domains(segments.clone(), parameters.clone()).unwrap();
            let sampler =
                ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), Tolerance::DEFAULT).unwrap();
            let kinks = sampler.kinks(0.1).unwrap();
            assert_eq!(kinks.len(), 1);
            assert_eq!(kinks[0].distance, 1.);
            assert_eq!(
                kinks[0].incoming_tangent.as_vector().to_array(),
                [1., 0., 0.]
            );
            assert_eq!(
                kinks[0].outgoing_tangent.as_vector().to_array(),
                [0., 1., 0.]
            );
            let corner = sampler.sample_at_distance(1.).unwrap();
            assert_eq!(corner.point(), points[1]);
            assert_eq!(corner.parameter(), parameters[1]);
            assert_eq!(sampler.distance_at_parameter(parameters[1]).unwrap(), 1.);
            assert_eq!(
                sampler.point_at_distance(0.5).unwrap(),
                Point3::try_new(0.5, 0., 0.).unwrap()
            );
            assert_eq!(
                sampler.point_at_distance(1.5).unwrap(),
                Point3::try_new(1., 0.5, 0.).unwrap()
            );
        }
    }

    #[test]
    fn polycurve_sampling_preserves_leaf_spans_on_tiny_outer_domains() {
        use crate::{Circle3, CurveSegment3, PolyCurve3};
        let tolerance = Tolerance::DEFAULT;
        let circle = Circle3::try_new(
            Point3::try_new(0., 0., 0.).unwrap(),
            2.,
            UnitVector3::try_new(0., 0., 1., tolerance).unwrap(),
            tolerance,
        )
        .unwrap();
        let leaf = circle.to_nurbs().unwrap();
        for domain in [
            [0., 1.],
            [1., f64::from_bits(1_f64.to_bits() + 1)],
            [0., f64::from_bits(1)],
            [-f64::MAX / 2., f64::MAX / 2.],
        ] {
            let curve = PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::NurbsCurve(leaf.clone())],
                domain.to_vec(),
            )
            .unwrap();
            let original = curve.clone();
            let mut sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), tolerance)
                .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
            assert!((sampler.total_length() - 4. * std::f64::consts::PI).abs() < 1e-10);
            for cached in [false, true] {
                if cached {
                    sampler.prepare_repeated_sampling(16).unwrap();
                }
                for i in 0..=8 {
                    let fraction = i as f64 / 8.;
                    let distance = sampler.total_length() * fraction;
                    let sample = sampler.sample_at_distance(distance).unwrap();
                    let expected = circle
                        .point_at_angle(fraction * std::f64::consts::TAU)
                        .unwrap();
                    assert!(
                        sample.point().distance_to(expected).unwrap() < 1e-9,
                        "{domain:?}, cached={cached}, i={i}: {:?} != {expected:?}",
                        sample.point()
                    );
                    assert!(curve.domain().contains(&sample.parameter()));
                    assert_eq!(
                        sample.parameter(),
                        sampler.parameter_at_distance(distance).unwrap()
                    );
                    if domain[0] == 0. && domain[1] == 1. || domain[1] > 1e100 {
                        assert!(
                            (sampler.distance_at_parameter(sample.parameter()).unwrap() - distance)
                                .abs()
                                < 1e-9
                        );
                    }
                }
                assert_eq!(sampler.distance_at_parameter(domain[0]).unwrap(), 0.);
                assert_eq!(
                    sampler.distance_at_parameter(domain[1]).unwrap(),
                    sampler.total_length()
                );
            }
            assert_eq!(curve, original);
        }
    }

    #[test]
    fn polycurve_sampling_conditions_independent_nurbs_leaf_domains() {
        use crate::{CurveSegment3, PolyCurve3};
        let arch = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let tolerance = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        let reference = CurveRef::NurbsCurve(&arch)
            .sample_equal_length_points(8, true, tolerance)
            .unwrap();
        for domain in [
            1.0..=f64::from_bits(1_f64.to_bits() + 1),
            0.0..=f64::from_bits(1),
            0.0..=1e-200,
            0.0..=1e200,
        ] {
            let curve = PolyCurve3::try_with_segment_domains(
                vec![CurveSegment3::NurbsCurve(
                    arch.try_reparameterized(domain.clone()).unwrap(),
                )],
                vec![0., 1.],
            )
            .unwrap();
            let mut sampler = ArcLengthSampler::try_new(CurveRef::PolyCurve(&curve), tolerance)
                .unwrap_or_else(|error| panic!("{domain:?}: {error}"));
            sampler.prepare_repeated_sampling(16).unwrap();
            for (i, expected) in reference.iter().enumerate() {
                let distance = sampler.total_length() * (i as f64 / 8.);
                let sample = sampler.sample_at_distance(distance).unwrap();
                assert!(sample.point().distance_to(*expected).unwrap() < 1e-10);
                assert!(
                    (sampler.distance_at_parameter(sample.parameter()).unwrap() - distance).abs()
                        < 1e-10
                );
            }
        }
    }

    #[test]
    fn nurbs_division_points_survive_extreme_parameter_domains() {
        let curve = NurbsCurve::try_new(
            2,
            vec![
                Point3::try_new(0., 0., 0.).unwrap(),
                Point3::try_new(0.5, 1., 0.).unwrap(),
                Point3::try_new(1., 0., 0.).unwrap(),
            ],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let t = Tolerance::try_new(1e-12, 1e-12, 1e-10).unwrap();
        let reference = CurveRef::NurbsCurve(&curve)
            .sample_equal_length_points(8, true, t)
            .unwrap();
        assert!(
            reference[4]
                .distance_to(Point3::try_new(0.5, 0.5, 0.).unwrap())
                .unwrap()
                < 1e-12
        );
        for domain in [
            1.0..=f64::from_bits(1.0_f64.to_bits() + 1),
            0.0..=f64::from_bits(1),
            -f64::MAX..=f64::MAX,
            0.0..=1e-200,
            0.0..=1e200,
        ] {
            let mapped = curve.try_reparameterized(domain.clone()).unwrap();
            let actual = CurveRef::NurbsCurve(&mapped)
                .sample_equal_length_points(8, true, t)
                .unwrap();
            for (actual, expected) in actual.iter().zip(&reference) {
                assert!(actual.distance_to(*expected).unwrap() < 1e-11, "{domain:?}");
            }
            let mut sampler = ArcLengthSampler::try_new(CurveRef::NurbsCurve(&mapped), t).unwrap();
            sampler.prepare_repeated_sampling(16).unwrap();
            let exact_length = 0.5 * 5_f64.sqrt() + 0.25 * 2_f64.asinh();
            assert!((sampler.total_length() - exact_length).abs() < 1e-11);
            for (i, expected) in reference.iter().enumerate() {
                let distance = sampler.total_length() * (i as Real / 8.0);
                let sample = sampler.sample_at_distance(distance).unwrap();
                assert!(sample.point().distance_to(*expected).unwrap() < 1e-11);
                assert!(domain.contains(&sample.parameter()));
                assert_eq!(
                    sample.parameter(),
                    sampler.parameter_at_distance(distance).unwrap()
                );
                let tangent = Vector3::try_new(1., 2. - 4. * expected.x(), 0.)
                    .unwrap()
                    .normalized_nonzero()
                    .unwrap();
                assert!(
                    sample
                        .tangent()
                        .as_vector()
                        .dot(tangent.as_vector())
                        .unwrap()
                        > 1. - 1e-12
                );
                // One-ulp domains cannot encode interior stations. On
                // well-resolved domains, public parameters must roundtrip.
                if *domain.end() > 1e100 || *domain.end() == 1e-200 {
                    let recovered = sampler.distance_at_parameter(sample.parameter()).unwrap();
                    assert!((recovered - distance).abs() < 1e-10, "{domain:?}: {i}");
                }
            }
            assert_eq!(sampler.distance_at_parameter(*domain.start()).unwrap(), 0.);
            assert_eq!(
                sampler.distance_at_parameter(*domain.end()).unwrap(),
                sampler.total_length()
            );
        }
    }
}
