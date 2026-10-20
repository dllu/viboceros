use crate::{
    Frame3, GeometryError, MAX_CURVE_DIVISION_POINTS, NurbsCurve, NurbsCurve2,
    NurbsCurveSamplingSpan, NurbsSurface, Point2, Point3, Real, Tolerance, Vector3, WeightedPoint2,
    remap_scalar, require_finite,
};
mod certificate;
pub use certificate::PreparedSurfaceCurveBounds;
pub(crate) use certificate::edit_point_bounds;
#[cfg(test)]
pub(crate) mod certified_tests;
mod interpolation;
mod linear;
mod periodic;

const PULLBACK_DEGREE: usize = 3;
const PULLBACK_SAMPLES_PER_SPAN: usize = 16;
const MAX_PULLBACK_SUBDIVISION_DEPTH: usize = 20;
const MAX_PULLBACK_SEGMENTS: usize = (MAX_CURVE_DIVISION_POINTS - 1) / PULLBACK_DEGREE;

#[derive(Clone, Copy, Debug)]
struct PullbackNode {
    parameter: Real,
    point: Point2,
    // Model-space derivative with respect to this original span's fraction.
    derivative: Vector3,
    constrained: bool,
}

#[derive(Clone, Copy, Debug)]
struct PullbackSegment {
    start: Real,
    end: Real,
    controls: [Point2; 4],
    disjoint: bool,
}

struct PullbackFitter<'a> {
    surface: &'a NurbsSurface,
    span: NurbsCurveSamplingSpan<'a>,
    native_domain: [Real; 2],
    tolerance: Tolerance,
    numerical_tolerance: Tolerance,
    segments: Vec<PullbackSegment>,
    certificate: Option<certificate::PullbackCertificate>,
    closed: [bool; 2],
    seam_split: bool,
}

fn model_points_near(
    first: Point3,
    second: Point3,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    let scale = first
        .to_array()
        .into_iter()
        .chain(second.to_array())
        .map(Real::abs)
        .fold(1.0_f64, Real::max);
    let allowed = tolerance.absolute().max(tolerance.relative() * scale);
    Ok(first.distance_to(second)? <= allowed)
}

fn snap_domain_roundoff(parameter: Real, domain: [Real; 2]) -> Real {
    // Roundoff is relative to chart width, not model units or the chart's
    // absolute origin. A large origin must not swallow valid interior picks.
    let Ok(fraction) = remap_scalar(parameter, domain, [0., 1.]) else {
        return parameter;
    };
    let epsilon = 4096. * Real::EPSILON;
    if fraction.abs() <= epsilon {
        domain[0]
    } else if (fraction - 1.).abs() <= epsilon {
        domain[1]
    } else {
        parameter
    }
}

fn numerical_pullback_tolerance(tolerance: Tolerance) -> Result<Tolerance, GeometryError> {
    // Keep closest-point iteration noise well below the model-space budget
    // used to accept a fitted span. Angular degeneracy remains the caller's
    // policy because tightening it could invert a nearly singular surface.
    let tighten = |value: Real| {
        let tightened = value * 1.0e-4;
        if tightened > 0.0 { tightened } else { value }
    };
    Tolerance::try_new(
        tighten(tolerance.absolute()),
        tighten(tolerance.relative()),
        tolerance.angular(),
    )
}

fn pullback_step(
    surface: &NurbsSurface,
    point: Point2,
    model_step: Vector3,
    tolerance: Tolerance,
) -> Result<[Real; 2], GeometryError> {
    let (origin, derivative_u, derivative_v) =
        surface.evaluate_with_derivatives(point.x(), point.y())?;
    let u = derivative_u.normalized_nonzero()?;
    let v = derivative_v.normalized_nonzero()?;
    let frame = Frame3::try_from_unit_directions(origin, u, v, tolerance)?;
    let cosine = u.as_vector().dot(v.as_vector())?;
    let sine = u.as_vector().cross(v.as_vector())?.length()?;
    let v_motion = model_step.dot(frame.y_axis().as_vector())? / sine;
    let u_motion = (-cosine).mul_add(v_motion, model_step.dot(u.as_vector())?);
    let step = [
        derivative_u.parameter_step(u_motion),
        derivative_v.parameter_step(v_motion),
    ];
    require_finite(step, "surface pullback parameter step")?;
    Ok(step)
}

fn pullback_node(
    surface: &NurbsSurface,
    span: NurbsCurveSamplingSpan<'_>,
    fraction: Real,
    tolerance: Tolerance,
    numerical_tolerance: Tolerance,
) -> Result<PullbackNode, GeometryError> {
    let (model_point, derivative) = span.evaluate_with_derivative(fraction)?;
    let point = pullback_parameters(surface, model_point, numerical_tolerance)?;
    if !model_points_near(
        surface.evaluate(point.x(), point.y())?,
        model_point,
        tolerance,
    )? {
        return Err(GeometryError::InvalidControlNet {
            context: "curve must lie on the surface for parameter-space pullback",
        });
    }
    Ok(PullbackNode {
        parameter: fraction,
        point,
        derivative,
        constrained: false,
    })
}

fn pullback_parameters(
    surface: &NurbsSurface,
    model_point: Point3,
    numerical_tolerance: Tolerance,
) -> Result<Point2, GeometryError> {
    let (u, v) = surface.closest_parameters(model_point, numerical_tolerance)?;
    let domain_u = [*surface.domain_u().start(), *surface.domain_u().end()];
    let domain_v = [*surface.domain_v().start(), *surface.domain_v().end()];
    Point2::try_new(
        snap_domain_roundoff(u, domain_u),
        snap_domain_roundoff(v, domain_v),
    )
}

fn hermite_segment(
    surface: &NurbsSurface,
    start: PullbackNode,
    end: PullbackNode,
    native_domain: [Real; 2],
    tolerance: Tolerance,
) -> Result<PullbackSegment, GeometryError> {
    let extent = end.parameter - start.parameter;
    if extent <= 0. {
        return Err(GeometryError::InvalidKnotVector {
            context: "surface pullback span must advance",
        });
    }
    // Scale the fractional model derivative before solving for UV motion.
    // Neither a native dUV/dt nor a native model-space speed need fit in f64.
    let first = pullback_step(
        surface,
        start.point,
        start.derivative.scaled(extent / 3.)?,
        tolerance,
    )?;
    let last = pullback_step(
        surface,
        end.point,
        end.derivative.scaled(-extent / 3.)?,
        tolerance,
    )?;
    Ok(PullbackSegment {
        start: remap_scalar(start.parameter, [0., 1.], native_domain)?,
        end: remap_scalar(end.parameter, [0., 1.], native_domain)?,
        controls: [
            start.point,
            Point2::try_new(start.point.x() + first[0], start.point.y() + first[1])?,
            Point2::try_new(end.point.x() + last[0], end.point.y() + last[1])?,
            end.point,
        ],
        disjoint: false,
    })
}

fn fixed_pullback_node(
    surface: &NurbsSurface,
    span: NurbsCurveSamplingSpan<'_>,
    fraction: Real,
    point: Point2,
    tolerance: Tolerance,
) -> Result<PullbackNode, GeometryError> {
    let (model_point, derivative) = span.evaluate_with_derivative(fraction)?;
    if surface
        .evaluate(point.x(), point.y())?
        .distance_to(model_point)?
        > tolerance.absolute()
    {
        return Err(GeometryError::InvalidControlNet {
            context: "fixed pullback endpoint exceeds the model-space tolerance",
        });
    }
    Ok(PullbackNode {
        parameter: fraction,
        point,
        derivative,
        constrained: true,
    })
}

pub(crate) fn constrain_curve_endpoints(
    curve: NurbsCurve2,
    endpoints: [Point2; 2],
) -> Result<Option<NurbsCurve2>, GeometryError> {
    if curve.start_point()? == endpoints[0] && curve.end_point()? == endpoints[1] {
        return Ok(Some(curve));
    }
    let domain = curve.domain();
    let order = curve.degree() + 1;
    let knots = curve.knots();
    if !knots[..order].iter().all(|k| *k == *domain.start())
        || !knots[knots.len() - order..]
            .iter()
            .all(|k| *k == *domain.end())
    {
        return Ok(None);
    }
    let mut controls = curve.control_points().to_vec();
    let last = controls.len() - 1;
    controls[0] = WeightedPoint2::try_new(endpoints[0], controls[0].weight())?;
    controls[last] = WeightedPoint2::try_new(endpoints[1], controls[last].weight())?;
    Ok(Some(NurbsCurve2::try_new_rational(
        curve.degree(),
        controls,
        knots.to_vec(),
    )?))
}

fn evaluate_cubic(controls: [Point2; 4], parameter: Real) -> Result<Point2, GeometryError> {
    let complement = 1.0 - parameter;
    let start_weight = complement * complement * complement;
    let first_weight = 3.0 * parameter * complement * complement;
    let second_weight = 3.0 * parameter * parameter * complement;
    let end_weight = parameter * parameter * parameter;
    let coordinate = |value: fn(Point2) -> Real| {
        value(controls[0]).mul_add(
            start_weight,
            value(controls[1]).mul_add(
                first_weight,
                value(controls[2]).mul_add(second_weight, value(controls[3]) * end_weight),
            ),
        )
    };
    Point2::try_new(coordinate(Point2::x), coordinate(Point2::y))
}

fn segment_matches_curve(
    surface: &NurbsSurface,
    span: NurbsCurveSamplingSpan<'_>,
    interval: [Real; 2],
    segment: PullbackSegment,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for sample in 1..PULLBACK_SAMPLES_PER_SPAN {
        let normalized = sample as Real / PULLBACK_SAMPLES_PER_SPAN as Real;
        let fraction = interval[0].mul_add(1. - normalized, interval[1] * normalized);
        let uv = evaluate_cubic(segment.controls, normalized)?;
        let Ok(surface_point) = surface.evaluate(uv.x(), uv.y()) else {
            return Ok(false);
        };
        if !model_points_near(surface_point, span.evaluate(fraction)?, tolerance)? {
            return Ok(false);
        }
    }
    Ok(true)
}

impl PullbackFitter<'_> {
    fn append_span(
        &mut self,
        mut start: PullbackNode,
        mut end: PullbackNode,
        depth: usize,
    ) -> Result<(), GeometryError> {
        if self.closed.into_iter().any(|v| v) {
            let interval = [start.parameter, end.parameter];
            let periodic = periodic::Context::new(
                self.surface,
                self.closed,
                self.numerical_tolerance,
                self.tolerance,
            );
            for (node, fraction) in [(&mut start, 0.125), (&mut end, 0.875)] {
                if !node.constrained {
                    let model = self.span.evaluate(node.parameter)?;
                    if !periodic.near_seam(node.point, model)? {
                        continue;
                    }
                    let t = interval[0].mul_add(1. - fraction, interval[1] * fraction);
                    let interior = pullback_node(
                        self.surface,
                        self.span,
                        t,
                        self.tolerance,
                        self.numerical_tolerance,
                    )?;
                    let point = periodic.endpoint_branch(node.point, interior.point, model)?;
                    if point != node.point {
                        *node = fixed_pullback_node(
                            self.surface,
                            self.span,
                            node.parameter,
                            point,
                            self.tolerance,
                        )?;
                    }
                }
            }
            if depth < MAX_PULLBACK_SUBDIVISION_DEPTH
                && let Some(cut) = periodic.cut(interval, [start.point, end.point], |t| {
                    self.span.evaluate(t)
                })?
            {
                let left = fixed_pullback_node(
                    self.surface,
                    self.span,
                    cut.parameter,
                    cut.points[0],
                    self.tolerance,
                )?;
                let right = fixed_pullback_node(
                    self.surface,
                    self.span,
                    cut.parameter,
                    cut.points[1],
                    self.tolerance,
                )?;
                self.seam_split = true;
                self.append_span(start, left, depth + 1)?;
                return self.append_span(right, end, depth + 1);
            }
        }
        let mut candidate = hermite_segment(
            self.surface,
            start,
            end,
            self.native_domain,
            self.numerical_tolerance,
        )?;
        if candidate.end <= candidate.start {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let matches = segment_matches_curve(
            self.surface,
            self.span,
            [start.parameter, end.parameter],
            candidate,
            self.tolerance,
        )?;
        let certified = if matches {
            match &mut self.certificate {
                Some(certificate) => certificate.segment(candidate, self.tolerance.absolute())?,
                None => true,
            }
        } else {
            false
        };
        if certified {
            if self.segments.len() == MAX_PULLBACK_SEGMENTS {
                return Err(GeometryError::TooManySurfacePullbackControlPoints {
                    maximum: MAX_CURVE_DIVISION_POINTS,
                });
            }
            if let Some(previous) = self.segments.last() {
                candidate.disjoint = periodic::changes_branch(
                    self.surface,
                    previous.controls[3],
                    candidate.controls[0],
                    self.closed,
                )?;
                self.seam_split |= candidate.disjoint;
            }
            self.segments.push(candidate);
            return Ok(());
        }
        if depth == MAX_PULLBACK_SUBDIVISION_DEPTH {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let middle_native = candidate.start.mul_add(0.5, candidate.end * 0.5);
        let middle_parameter = remap_scalar(middle_native, self.native_domain, [0., 1.])?;
        if middle_native == candidate.start
            || middle_native == candidate.end
            || middle_parameter <= start.parameter
            || middle_parameter >= end.parameter
        {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: self.tolerance.absolute(),
            });
        }
        let middle = pullback_node(
            self.surface,
            self.span,
            middle_parameter,
            self.tolerance,
            self.numerical_tolerance,
        )?;
        self.append_span(start, middle, depth + 1)?;
        self.append_span(middle, end, depth + 1)
    }
}

fn piecewise_cubic(segments: &[PullbackSegment]) -> Result<NurbsCurve2, GeometryError> {
    let first = segments.first().ok_or(GeometryError::InvalidControlNet {
        context: "surface pullback requires at least one nonempty span",
    })?;
    let last = segments
        .last()
        .expect("a first segment implies a last segment");
    let control_count = segments
        .iter()
        .skip(1)
        .try_fold(PULLBACK_DEGREE + 1, |n, s| {
            n.checked_add(PULLBACK_DEGREE + usize::from(s.disjoint))
        })
        .filter(|count| *count <= MAX_CURVE_DIVISION_POINTS)
        .ok_or(GeometryError::TooManySurfacePullbackControlPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        })?;
    let knot_count = control_count.checked_add(PULLBACK_DEGREE + 1).ok_or(
        GeometryError::TooManySurfacePullbackControlPoints {
            maximum: MAX_CURVE_DIVISION_POINTS,
        },
    )?;
    let mut controls = Vec::with_capacity(control_count);
    let mut knots = Vec::with_capacity(knot_count);
    knots.extend(std::iter::repeat_n(first.start, PULLBACK_DEGREE + 1));
    controls.extend(first.controls);
    for segment in &segments[1..] {
        knots.extend(std::iter::repeat_n(
            segment.start,
            PULLBACK_DEGREE + usize::from(segment.disjoint),
        ));
        controls.extend_from_slice(&segment.controls[usize::from(!segment.disjoint)..]);
    }
    knots.extend(std::iter::repeat_n(last.end, PULLBACK_DEGREE + 1));
    NurbsCurve2::try_new(PULLBACK_DEGREE, controls, knots)
}

fn parameter_curve_matches(
    surface: &NurbsSurface,
    curve: &NurbsCurve,
    parameter_curve: &NurbsCurve2,
    tolerance: Tolerance,
) -> Result<bool, GeometryError> {
    for (span_start, span_end) in parameter_curve.spans() {
        for sample in 0..=PULLBACK_SAMPLES_PER_SPAN {
            let normalized = sample as Real / PULLBACK_SAMPLES_PER_SPAN as Real;
            let parameter = span_start.mul_add(1.0 - normalized, span_end * normalized);
            let uv = parameter_curve.evaluate(parameter)?;
            if !model_points_near(
                surface.evaluate(uv.x(), uv.y())?,
                curve.evaluate(parameter)?,
                tolerance,
            )? {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

impl NurbsSurface {
    /// Pulls a model-space NURBS curve into this surface's parameter space.
    ///
    /// Exact affine, projective, and eligible bilinear inverses retain the
    /// source NURBS structure. Automatically discovered straight UV paths,
    /// including eligible seam and singular-endpoint branches, require a
    /// continuous absolute-error certificate. Other regular parameterizations use adaptive
    /// piecewise-cubic Hermite pullback. Every fitted span is verified in
    /// model space at the supplied tolerance, and the source parameter domain
    /// is retained.
    pub fn try_pullback_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
    ) -> Result<NurbsCurve2, GeometryError> {
        self.pullback_curve(curve, tolerance, false, None)
            .map(|(curve, _)| curve)
    }

    fn pullback_curve(
        &self,
        curve: &NurbsCurve,
        tolerance: Tolerance,
        certified: bool,
        endpoints: Option<[Point2; 2]>,
    ) -> Result<(NurbsCurve2, Option<Real>), GeometryError> {
        if let Some(points) = endpoints {
            for (point, parameter) in points
                .into_iter()
                .zip([*curve.domain().start(), *curve.domain().end()])
            {
                if !self.domain_u().contains(&point.x()) || !self.domain_v().contains(&point.y()) {
                    return Err(GeometryError::InvalidControlNet {
                        context: "fixed pullback endpoints must lie in the natural surface domain",
                    });
                }
                if self
                    .evaluate(point.x(), point.y())?
                    .distance_to(curve.evaluate(parameter)?)?
                    > tolerance.absolute()
                {
                    return Err(GeometryError::InvalidControlNet {
                        context: "fixed pullback endpoint exceeds the model-space tolerance",
                    });
                }
            }
        }
        let proposal = match self.try_pullback_exact_curve(curve, tolerance) {
            Ok(exact) => match endpoints {
                Some(points) => constrain_curve_endpoints(exact, points)?,
                None => Some(exact),
            },
            Err(_) => None,
        };
        if let Some(exact) = proposal {
            let bound = if certified {
                self.parameter_curve_deviation_bound(&exact, curve, tolerance.absolute())?
            } else {
                None
            };
            if !certified || bound.is_some() {
                return Ok((exact, bound));
            }
        }

        let mut fitting_endpoints = endpoints;
        if let Some(points) = endpoints {
            // A full closed isocurve has distinct UV seam endpoints even
            // though its spatial endpoints coincide. This generic proposal
            // needs neither primitive recognition nor closest-point guesses.
            let line = linear::parameter_line(curve, points)?;
            if let Some(bound) =
                self.parameter_curve_deviation_bound(&line, curve, tolerance.absolute())?
            {
                return Ok((line, Some(bound)));
            }
        } else {
            let discovery = linear::discover(self, curve, tolerance)?;
            if let Some((line, bound)) = discovery.curve {
                return Ok((line, Some(bound)));
            }
            fitting_endpoints = discovery.endpoints;
        }

        if certified
            && let Some(points) = fitting_endpoints
            && let Ok(Some(result)) = interpolation::fit_on_isocurve(self, curve, points, tolerance)
        {
            return Ok((result.0, Some(result.1)));
        }
        match self.hermite_pullback(
            curve,
            fitting_endpoints,
            tolerance,
            certified,
            endpoints.is_some(),
        ) {
            Ok(result) => Ok(result),
            Err(
                GeometryError::Degenerate { .. }
                | GeometryError::NonFinite { .. }
                | GeometryError::SurfacePullbackDidNotConverge { .. }
                | GeometryError::SurfaceCurveCertificateWorkLimit,
            ) => interpolation::fit(self, curve, endpoints, tolerance)
                .map(|(uv, bound)| (uv, Some(bound))),
            Err(error) => Err(error),
        }
    }

    fn hermite_pullback(
        &self,
        curve: &NurbsCurve,
        fitting_endpoints: Option<[Point2; 2]>,
        tolerance: Tolerance,
        certified: bool,
        explicit_endpoints: bool,
    ) -> Result<(NurbsCurve2, Option<Real>), GeometryError> {
        let certificate = if certified {
            Some(certificate::PullbackCertificate::new(self, curve)?.ok_or(
                GeometryError::SurfacePullbackDidNotConverge {
                    tolerance: tolerance.absolute(),
                },
            )?)
        } else {
            None
        };
        let sampler = curve.parameter_sampler()?;
        let mut spans = curve.spans().zip(sampler.spans()).peekable();
        let Some(&((first_start, first_end), first_span)) = spans.peek() else {
            return Err(GeometryError::InvalidControlNet {
                context: "surface pullback requires a nonempty source span",
            });
        };
        let domain = curve.domain();
        let mut fitter = PullbackFitter {
            surface: self,
            span: first_span,
            native_domain: [first_start, first_end],
            tolerance,
            numerical_tolerance: numerical_pullback_tolerance(tolerance)?,
            segments: Vec::new(),
            certificate,
            closed: [
                crate::surface_geodesic::chart_closes(self, 0)?,
                crate::surface_geodesic::chart_closes(self, 1)?,
            ],
            seam_split: false,
        };
        for ((span_start, span_end), span) in spans {
            fitter.span = span;
            fitter.native_domain = [span_start, span_end];
            let mut start =
                if let Some(points) = fitting_endpoints.filter(|_| span_start == *domain.start()) {
                    fixed_pullback_node(self, span, 0., points[0], tolerance)?
                } else {
                    pullback_node(self, span, 0., tolerance, fitter.numerical_tolerance)?
                };
            let mut end =
                if let Some(points) = fitting_endpoints.filter(|_| span_end == *domain.end()) {
                    fixed_pullback_node(self, span, 1., points[1], tolerance)?
                } else {
                    pullback_node(self, span, 1., tolerance, fitter.numerical_tolerance)?
                };
            start.constrained &= explicit_endpoints;
            end.constrained &= explicit_endpoints;
            fitter.append_span(start, end, 0)?;
        }
        let parameter_curve = piecewise_cubic(&fitter.segments)?;
        if !parameter_curve_matches(self, curve, &parameter_curve, tolerance)? {
            return Err(GeometryError::SurfacePullbackDidNotConverge {
                tolerance: tolerance.absolute(),
            });
        }
        let bound = if fitter.seam_split {
            Some(
                self.parameter_curve_deviation_bound(
                    &parameter_curve,
                    curve,
                    tolerance.absolute(),
                )?
                .ok_or(GeometryError::SurfacePullbackDidNotConverge {
                    tolerance: tolerance.absolute(),
                })?,
            )
        } else {
            None
        };
        Ok((parameter_curve, bound))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AffineTransform3, WeightedPoint3};

    fn point(x: Real, y: Real) -> Point3 {
        Point3::try_new(x, y, 0.0).unwrap()
    }

    fn trapezoid_surface() -> NurbsSurface {
        NurbsSurface::try_bilinear([
            point(0.0, 0.0),
            point(10.0, 0.0),
            point(8.0, 10.0),
            point(0.0, 10.0),
        ])
        .unwrap()
    }

    #[test]
    fn fitted_pullback_recovers_curved_parameters_on_a_nonaffine_plane() {
        let surface = trapezoid_surface();
        let curve = NurbsCurve::try_new(
            3,
            vec![
                point(0.0, 2.0),
                point(3.2, 20.0 / 3.0),
                point(82.0 / 15.0, 8.0),
                point(8.8, 6.0),
            ],
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            surface
                .try_pullback_exact_curve(&curve, Tolerance::DEFAULT)
                .is_err()
        );

        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.degree(), 3);
        assert_eq!(parameter_curve.control_points().len(), 4);
        assert_eq!(parameter_curve.domain(), curve.domain());
        for (actual, expected) in parameter_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), expected[0]));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), expected[1]));
            assert_eq!(actual.weight(), 1.0);
        }
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }

        let reparameterized = surface
            .try_reparameterized(-2.0..=4.0, 10.0..=20.0)
            .unwrap();
        let reparameterized_curve = reparameterized
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(reparameterized_curve.control_points().len(), 4);
        for (actual, expected) in reparameterized_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), 6.0 * expected[0] - 2.0));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), 10.0 * expected[1] + 10.0));
        }

        let transform = AffineTransform3::try_new(
            [[2.0, -1.0, 0.5], [0.25, 3.0, -0.5], [0.5, 0.75, 4.0]],
            Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
        )
        .unwrap();
        let transformed_surface = reparameterized.transformed(transform).unwrap();
        let transformed_curve = curve.transformed(transform).unwrap();
        let transformed_pullback = transformed_surface
            .try_pullback_curve(&transformed_curve, Tolerance::DEFAULT)
            .unwrap();
        for (actual, expected) in transformed_pullback
            .control_points()
            .iter()
            .zip(reparameterized_curve.control_points())
        {
            assert!(actual.point().is_near(expected.point(), Tolerance::DEFAULT));
        }
    }

    #[test]
    fn fitted_pullback_subdivides_and_rejects_off_surface_curves() {
        let surface = trapezoid_surface();
        let curve = NurbsCurve::try_new(
            2,
            vec![point(0.0, 2.0), point(5.0, 9.0), point(8.8, 6.0)],
            vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.degree(), 3);
        assert!(parameter_curve.control_points().len() > 4);
        assert_eq!(parameter_curve.domain(), curve.domain());
        for sample in 0..=128 {
            let parameter = 2.0 + 3.0 * sample as Real / 128.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }

        let off_surface = NurbsCurve::try_new(
            2,
            vec![
                point(0.0, 2.0),
                Point3::try_new(5.0, 9.0, 0.01).unwrap(),
                point(8.8, 6.0),
            ],
            vec![2.0, 2.0, 2.0, 5.0, 5.0, 5.0],
        )
        .unwrap();
        assert!(matches!(
            surface.try_pullback_curve(&off_surface, Tolerance::DEFAULT),
            Err(GeometryError::InvalidControlNet { .. })
        ));
    }

    #[test]
    fn fitted_pullback_handles_a_rational_nonprojective_surface() {
        let surface_controls = [
            WeightedPoint3::try_new(point(0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(10.0, 0.0), 2.0).unwrap(),
            WeightedPoint3::try_new(point(0.0, 10.0), 1.5).unwrap(),
            WeightedPoint3::try_new(point(8.0, 10.0), 3.0).unwrap(),
        ];
        let surface = NurbsSurface::try_new_rational(
            1,
            1,
            2,
            2,
            surface_controls.to_vec(),
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();

        // Compose the homogeneous bilinear surface with
        // u=s, v=0.2+1.4s-s^2, then convert the resulting cubic power basis
        // to Bezier controls. This gives an exact rational spatial cutter
        // whose known UV preimage is polynomial even though the surface does
        // not have a projective inverse.
        let homogeneous = surface_controls.map(|control| {
            let point = control.point();
            let weight = control.weight();
            [point.x() * weight, point.y() * weight, 0.0, weight]
        });
        let origin = homogeneous[0];
        let axis_u: [Real; 4] =
            std::array::from_fn(|coordinate| homogeneous[1][coordinate] - origin[coordinate]);
        let axis_v: [Real; 4] =
            std::array::from_fn(|coordinate| homogeneous[2][coordinate] - origin[coordinate]);
        let twist: [Real; 4] = std::array::from_fn(|coordinate| {
            homogeneous[3][coordinate] - homogeneous[1][coordinate] - homogeneous[2][coordinate]
                + origin[coordinate]
        });
        let power = [
            std::array::from_fn(|coordinate| axis_v[coordinate].mul_add(0.2, origin[coordinate])),
            std::array::from_fn(|coordinate| {
                twist[coordinate].mul_add(0.2, axis_v[coordinate].mul_add(1.4, axis_u[coordinate]))
            }),
            std::array::from_fn(|coordinate| twist[coordinate].mul_add(1.4, -axis_v[coordinate])),
            twist.map(|coordinate| -coordinate),
        ];
        let curve_homogeneous = [
            power[0],
            std::array::from_fn(|coordinate| {
                power[1][coordinate].mul_add(1.0 / 3.0, power[0][coordinate])
            }),
            std::array::from_fn(|coordinate| {
                power[2][coordinate].mul_add(
                    1.0 / 3.0,
                    power[1][coordinate].mul_add(2.0 / 3.0, power[0][coordinate]),
                )
            }),
            std::array::from_fn(|coordinate| {
                power[3][coordinate]
                    + power[2][coordinate]
                    + power[1][coordinate]
                    + power[0][coordinate]
            }),
        ];
        let curve = NurbsCurve::try_new_rational(
            3,
            curve_homogeneous
                .map(|control| {
                    WeightedPoint3::try_new(
                        Point3::try_new(
                            control[0] / control[3],
                            control[1] / control[3],
                            control[2] / control[3],
                        )
                        .unwrap(),
                        control[3],
                    )
                    .unwrap()
                })
                .to_vec(),
            vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(
            surface
                .try_pullback_exact_curve(&curve, Tolerance::DEFAULT)
                .is_err()
        );

        let parameter_curve = surface
            .try_pullback_curve(&curve, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameter_curve.control_points().len(), 4);
        for (actual, expected) in parameter_curve.control_points().iter().zip([
            [0.0, 0.2],
            [1.0 / 3.0, 2.0 / 3.0],
            [2.0 / 3.0, 0.8],
            [1.0, 0.6],
        ]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().x(), expected[0]));
            assert!(Tolerance::DEFAULT.approx_eq(actual.point().y(), expected[1]));
        }
        for sample in 0..=64 {
            let parameter = sample as Real / 64.0;
            let uv = parameter_curve.evaluate(parameter).unwrap();
            assert!(
                surface
                    .evaluate(uv.x(), uv.y())
                    .unwrap()
                    .is_near(curve.evaluate(parameter).unwrap(), Tolerance::DEFAULT)
            );
        }
    }
}
