//! Boundary-target curve extension with shared nearest-hit and topology policy.
use super::*;
use crate::intersection::curve_surface_intersections;

type ParameterEdit<'a, E> =
    dyn FnMut(&NurbsCurve, RangeInclusive<Real>) -> Result<NurbsCurve, E> + 'a;
type EndpointClamp<'a, E> = dyn FnMut(&NurbsCurve, bool) -> Result<NurbsCurve, E> + 'a;

/// Exact producers used by boundary extension; intersection, source classification,
/// nearest-hit ordering and joining policy remain in the geometry core.
pub struct CurveExtensionOperations<'a, E> {
    pub extend: &'a mut ParameterEdit<'a, E>,
    pub trim: &'a mut ParameterEdit<'a, E>,
    /// `true` clamps only the active start, `false` only the active end.
    pub clamp_endpoint: &'a mut EndpointClamp<'a, E>,
}

impl<E: From<GeometryError>> CurveExtensionOperations<'_, E> {
    fn segment(
        &mut self,
        source: &NurbsCurve,
        at_start: bool,
        parameter: Real,
    ) -> Result<NurbsCurve, E> {
        source.extension_segment_with_operations(at_start, parameter, self.extend, self.trim)
    }
}

fn with_portable_extension_operations<T>(
    run: impl FnOnce(&mut CurveExtensionOperations<'_, GeometryError>) -> Result<T, GeometryError>,
) -> Result<T, GeometryError> {
    let mut extend = NurbsCurve::try_extended_to;
    let mut trim = NurbsCurve::try_trimmed;
    let mut clamp_endpoint = |source: &NurbsCurve, at_start| {
        if at_start {
            source.clamped_at_start(*source.domain().start())
        } else {
            source.clamped_at_end(*source.domain().end())
        }
    };
    run(&mut CurveExtensionOperations {
        extend: &mut extend,
        trim: &mut trim,
        clamp_endpoint: &mut clamp_endpoint,
    })
}

#[derive(Clone, Copy, Debug)]
struct CurveBoundaryExtensionTarget {
    parameter: Real,
    length: Real,
}

#[derive(Clone, Copy, Debug)]
struct CurveBoundaryExtensionTargets {
    style: CurveExtensionStyle,
    start: Option<CurveBoundaryExtensionTarget>,
    end: Option<CurveBoundaryExtensionTarget>,
}

impl NurbsCurve {
    /// Extends the requested end or ends to the nearest intersections with
    /// any supplied boundary curve and applies Rhino's `Join=Merge` cleanup.
    pub fn try_merged_to_curve_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[Self],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let boundaries = boundaries
            .iter()
            .cloned()
            .map(CurveExtensionBoundary::Curve)
            .collect::<Vec<_>>();
        self.try_merged_to_boundaries(side, style, &boundaries, tolerance)
    }

    /// Extends to the nearest curve, surface, or trimmed B-rep intersection
    /// and applies Rhino's `Join=Merge` cleanup.
    pub fn try_merged_to_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        with_portable_extension_operations(|operations| {
            self.try_merged_to_boundaries_with_operations(
                side, style, boundaries, tolerance, operations,
            )
        })
    }

    /// Shared boundary policy with delegated extrapolation, extraction and endpoint clamping.
    pub fn try_merged_to_boundaries_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<Self, E> {
        let targets =
            self.curve_boundary_extension_targets(side, style, boundaries, tolerance, operations)?;
        match targets.style {
            CurveExtensionStyle::Smooth => {
                self.smooth_boundary_extension(&targets, false, operations)
            }
            CurveExtensionStyle::Line => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_merged_linearly_by_length(side, length, tolerance)
                })
                .map_err(E::from),
            CurveExtensionStyle::Arc => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_merged_circularly_by_length(side, length, tolerance)
                })
                .map_err(E::from),
            CurveExtensionStyle::Natural => {
                unreachable!("boundary extension resolves Natural before constructing targets")
            }
        }
    }

    /// Extends the requested end or ends to boundary curves while retaining
    /// an explicit segment boundary, matching Rhino's `Join=Yes` topology.
    pub fn try_joined_to_curve_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[Self],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        let boundaries = boundaries
            .iter()
            .cloned()
            .map(CurveExtensionBoundary::Curve)
            .collect::<Vec<_>>();
        self.try_joined_to_boundaries(side, style, &boundaries, tolerance)
    }

    /// Extends to curve, surface, or trimmed B-rep boundaries while retaining
    /// an explicit segment seam, matching Rhino's `Join=Yes` topology.
    pub fn try_joined_to_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        with_portable_extension_operations(|operations| {
            self.try_joined_to_boundaries_with_operations(
                side, style, boundaries, tolerance, operations,
            )
        })
    }

    /// Shared boundary policy with delegated extrapolation, extraction and endpoint clamping.
    pub fn try_joined_to_boundaries_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<Self, E> {
        let targets =
            self.curve_boundary_extension_targets(side, style, boundaries, tolerance, operations)?;
        match targets.style {
            CurveExtensionStyle::Smooth => {
                self.smooth_boundary_extension(&targets, true, operations)
            }
            CurveExtensionStyle::Line => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_joined_linearly_by_length(side, length, tolerance)
                })
                .map_err(E::from),
            CurveExtensionStyle::Arc => self
                .apply_boundary_extension_lengths(&targets, |curve, side, length| {
                    curve.try_joined_circularly_by_length(side, length, tolerance)
                })
                .map_err(E::from),
            CurveExtensionStyle::Natural => {
                unreachable!("boundary extension resolves Natural before constructing targets")
            }
        }
    }

    /// Creates independent extension curves from the selected ends to the
    /// nearest boundary hits without modifying this source curve.
    pub fn try_separate_extensions_to_curve_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[Self],
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        let boundaries = boundaries
            .iter()
            .cloned()
            .map(CurveExtensionBoundary::Curve)
            .collect::<Vec<_>>();
        self.try_separate_extensions_to_boundaries(side, style, &boundaries, tolerance)
    }

    /// Creates independent extension curves ending at the nearest curve,
    /// surface, or trimmed B-rep boundary hits without modifying this source.
    pub fn try_separate_extensions_to_boundaries(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
    ) -> Result<Vec<Self>, GeometryError> {
        with_portable_extension_operations(|operations| {
            self.try_separate_extensions_to_boundaries_with_operations(
                side, style, boundaries, tolerance, operations,
            )
        })
    }

    /// Shared boundary policy with delegated extrapolation, extraction and endpoint clamping.
    pub fn try_separate_extensions_to_boundaries_with_operations<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<Vec<Self>, E> {
        let targets =
            self.curve_boundary_extension_targets(side, style, boundaries, tolerance, operations)?;
        let mut extensions = Vec::with_capacity(
            usize::from(targets.start.is_some()) + usize::from(targets.end.is_some()),
        );
        for (target, extension_side) in [
            (targets.start, CurveExtensionSide::Start),
            (targets.end, CurveExtensionSide::End),
        ] {
            let Some(target) = target else {
                continue;
            };
            let mut pieces = match targets.style {
                CurveExtensionStyle::Smooth => {
                    vec![operations.segment(
                        self,
                        extension_side == CurveExtensionSide::Start,
                        target.parameter,
                    )?]
                }
                CurveExtensionStyle::Line => self
                    .try_separate_linear_extensions_by_length(
                        extension_side,
                        target.length,
                        tolerance,
                    )
                    .map_err(E::from)?,
                CurveExtensionStyle::Arc => self
                    .try_separate_circular_extensions_by_length(
                        extension_side,
                        target.length,
                        tolerance,
                    )
                    .map_err(E::from)?,
                CurveExtensionStyle::Natural => {
                    unreachable!("boundary extension resolves Natural before constructing targets")
                }
            };
            debug_assert_eq!(pieces.len(), 1);
            extensions.append(&mut pieces);
        }
        Ok(extensions)
    }

    fn curve_boundary_extension_targets<E: From<GeometryError>>(
        &self,
        side: CurveExtensionSide,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<CurveBoundaryExtensionTargets, E> {
        if boundaries.is_empty() {
            return Err(GeometryError::EmptyCurveExtensionBoundaries.into());
        }
        if self.is_closed()? {
            return Err(GeometryError::CurveExtensionMustBeOpen.into());
        }
        let style = match style {
            CurveExtensionStyle::Natural
                if self.degree == 1 || self.is_linear_at_zero_tolerance()? =>
            {
                CurveExtensionStyle::Line
            }
            CurveExtensionStyle::Natural
                if self.try_canonical_circular_arc(tolerance)?.is_some() =>
            {
                CurveExtensionStyle::Arc
            }
            CurveExtensionStyle::Natural => CurveExtensionStyle::Smooth,
            style => style,
        };
        let start = matches!(side, CurveExtensionSide::Start | CurveExtensionSide::Both)
            .then(|| {
                self.curve_boundary_extension_target(true, style, boundaries, tolerance, operations)
            })
            .transpose()?
            .flatten();
        let end = matches!(side, CurveExtensionSide::End | CurveExtensionSide::Both)
            .then(|| {
                self.curve_boundary_extension_target(
                    false, style, boundaries, tolerance, operations,
                )
            })
            .transpose()?
            .flatten();
        if (matches!(side, CurveExtensionSide::Start) && start.is_none())
            || (matches!(side, CurveExtensionSide::End) && end.is_none())
            || (matches!(side, CurveExtensionSide::Both) && start.is_none() && end.is_none())
        {
            return Err(GeometryError::CurveExtensionBoundaryNotFound.into());
        }
        Ok(CurveBoundaryExtensionTargets { style, start, end })
    }

    fn curve_boundary_extension_target<E: From<GeometryError>>(
        &self,
        at_start: bool,
        style: CurveExtensionStyle,
        boundaries: &[CurveExtensionBoundary],
        tolerance: Tolerance,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<Option<CurveBoundaryExtensionTarget>, E> {
        let domain = self.domain();
        let endpoint_parameter = if at_start {
            *domain.start()
        } else {
            *domain.end()
        };
        let endpoint = self.evaluate(endpoint_parameter)?;

        if style == CurveExtensionStyle::Smooth {
            let adjacent_span = if at_start {
                self.spans()
                    .next()
                    .expect("a validated NURBS curve has an active span")
                    .1
                    - endpoint_parameter
            } else {
                endpoint_parameter
                    - self
                        .spans()
                        .last()
                        .expect("a validated NURBS curve has an active span")
                        .0
            };
            let mut step = adjacent_span;
            for _ in 0..40 {
                let outer_parameter = if at_start {
                    endpoint_parameter - step
                } else {
                    endpoint_parameter + step
                };
                if !outer_parameter.is_finite() || outer_parameter == endpoint_parameter {
                    break;
                }
                let extension = operations.segment(self, at_start, outer_parameter)?;
                if let Some(hit) = extension
                    .nearest_boundary_intersection(boundaries, at_start, endpoint, tolerance)?
                {
                    let interval = if at_start {
                        hit.first_parameter..=endpoint_parameter
                    } else {
                        endpoint_parameter..=hit.first_parameter
                    };
                    let length = (operations.trim)(&extension, interval)?.length(tolerance)?;
                    if length > tolerance.absolute() {
                        return Ok(Some(CurveBoundaryExtensionTarget {
                            parameter: hit.first_parameter,
                            length,
                        }));
                    }
                }
                step *= 2.0;
            }
            return Ok(None);
        }

        let maximum_boundary_distance = boundaries.iter().try_fold(
            0.0_f64,
            |maximum, boundary| -> Result<Real, GeometryError> {
                let bounds = boundary.control_bounds();
                let farthest = Point3::try_new(
                    farthest_coordinate(endpoint.x(), bounds.min().x(), bounds.max().x()),
                    farthest_coordinate(endpoint.y(), bounds.min().y(), bounds.max().y()),
                    farthest_coordinate(endpoint.z(), bounds.min().z(), bounds.max().z()),
                )?;
                Ok(maximum.max(endpoint.distance_to(farthest)?))
            },
        )?;
        if !maximum_boundary_distance.is_finite() || maximum_boundary_distance == 0.0 {
            return Ok(None);
        }
        let probe_length = if style == CurveExtensionStyle::Arc {
            let curvature = CurveRef::NurbsCurve(self).curvature_vector(endpoint_parameter)?;
            let magnitude = curvature.length()?;
            if self.degree == 1 || magnitude == 0.0 {
                maximum_boundary_distance * 2.0
            } else {
                std::f64::consts::TAU / magnitude
            }
        } else {
            maximum_boundary_distance * 2.0
        };
        if !probe_length.is_finite() || probe_length <= tolerance.absolute() {
            return Ok(None);
        }
        let extension = match style {
            CurveExtensionStyle::Arc => {
                self.circular_extension_piece(at_start, probe_length, tolerance, false)?
            }
            CurveExtensionStyle::Line => self
                .try_separate_linear_extensions_by_length(
                    if at_start {
                        CurveExtensionSide::Start
                    } else {
                        CurveExtensionSide::End
                    },
                    probe_length,
                    tolerance,
                )?
                .pop()
                .expect("a one-sided linear extension creates one curve"),
            CurveExtensionStyle::Natural | CurveExtensionStyle::Smooth => {
                unreachable!("natural and smooth boundary targets are handled separately")
            }
        };
        let Some(hit) =
            extension.nearest_boundary_intersection(boundaries, at_start, endpoint, tolerance)?
        else {
            return Ok(None);
        };
        let interval = if at_start {
            hit.first_parameter..=*extension.domain().end()
        } else {
            *extension.domain().start()..=hit.first_parameter
        };
        let length = (operations.trim)(&extension, interval)?.length(tolerance)?;
        if length <= tolerance.absolute() {
            return Ok(None);
        }
        Ok(Some(CurveBoundaryExtensionTarget {
            parameter: hit.first_parameter,
            length,
        }))
    }

    fn nearest_boundary_intersection(
        &self,
        boundaries: &[CurveExtensionBoundary],
        at_start: bool,
        source_endpoint: Point3,
        tolerance: Tolerance,
    ) -> Result<Option<CurveCurveIntersection>, GeometryError> {
        let mut nearest: Option<CurveCurveIntersection> = None;
        let domain = self.domain();
        let source_parameter = if at_start {
            *domain.end()
        } else {
            *domain.start()
        };
        for boundary in boundaries {
            let intersections = match boundary {
                CurveExtensionBoundary::Curve(curve) => {
                    self.verified_curve_boundary_hits(curve, tolerance)?
                }
                CurveExtensionBoundary::Surface(surface) => {
                    self.verified_surface_boundary_hits(surface, tolerance)?
                }
                CurveExtensionBoundary::Brep(brep) => {
                    let mut intersections = Vec::new();
                    for edge in brep.edges() {
                        intersections
                            .extend(self.verified_curve_boundary_hits(edge.curve(), tolerance)?);
                    }
                    for face in brep.faces() {
                        for intersection in
                            curve_surface_intersections(self, face.surface(), tolerance)?
                        {
                            if self.domain().contains(&intersection.curve_parameter)
                                && face.surface().domain_u().contains(&intersection.u)
                                && face.surface().domain_v().contains(&intersection.v)
                                && self.evaluate(intersection.curve_parameter)?.is_near(
                                    face.surface().evaluate(intersection.u, intersection.v)?,
                                    tolerance,
                                )
                                && face.contains_parameters(
                                    intersection.u,
                                    intersection.v,
                                    tolerance,
                                )?
                            {
                                intersections.push(CurveCurveIntersection {
                                    first_parameter: intersection.curve_parameter,
                                    second_parameter: intersection.u,
                                    point: intersection.point,
                                });
                            }
                        }
                    }
                    intersections
                }
            };
            for intersection in intersections {
                // The unchanged endpoint cannot be a positive-length hit.
                // Its evaluated point can drift on a very distant probe, so
                // enforce direction in the parameter chart as well.
                if (at_start && intersection.first_parameter >= source_parameter)
                    || (!at_start && intersection.first_parameter <= source_parameter)
                {
                    continue;
                }
                if intersection.point.distance_to(source_endpoint)? <= tolerance.absolute() {
                    continue;
                }
                let is_nearer = nearest.is_none_or(|current| {
                    if at_start {
                        intersection.first_parameter > current.first_parameter
                    } else {
                        intersection.first_parameter < current.first_parameter
                    }
                });
                if is_nearer {
                    nearest = Some(intersection);
                }
            }
        }
        Ok(nearest)
    }

    // An enlarged probe's bounding-box scale must not authorize a spurious
    // model-space hit. Check both original loci at the reported parameters
    // using the document's coordinatewise tolerance, before selecting a hit.
    fn verified_curve_boundary_hits(
        &self,
        boundary: &Self,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveCurveIntersection>, GeometryError> {
        let mut result = Vec::new();
        for hit in self.intersections_with_curve(boundary, tolerance)? {
            if self.domain().contains(&hit.first_parameter)
                && boundary.domain().contains(&hit.second_parameter)
                && self
                    .evaluate(hit.first_parameter)?
                    .is_near(boundary.evaluate(hit.second_parameter)?, tolerance)
            {
                result.push(hit);
            }
        }
        Ok(result)
    }

    fn verified_surface_boundary_hits(
        &self,
        boundary: &NurbsSurface,
        tolerance: Tolerance,
    ) -> Result<Vec<CurveCurveIntersection>, GeometryError> {
        let mut result = Vec::new();
        for hit in curve_surface_intersections(self, boundary, tolerance)? {
            if self.domain().contains(&hit.curve_parameter)
                && boundary.domain_u().contains(&hit.u)
                && boundary.domain_v().contains(&hit.v)
                && self
                    .evaluate(hit.curve_parameter)?
                    .is_near(boundary.evaluate(hit.u, hit.v)?, tolerance)
            {
                result.push(CurveCurveIntersection {
                    first_parameter: hit.curve_parameter,
                    second_parameter: hit.u,
                    point: hit.point,
                });
            }
        }
        Ok(result)
    }

    fn smooth_boundary_extension<E: From<GeometryError>>(
        &self,
        targets: &CurveBoundaryExtensionTargets,
        retain_seams: bool,
        operations: &mut CurveExtensionOperations<'_, E>,
    ) -> Result<Self, E> {
        let domain = self.domain();
        if !retain_seams {
            return (operations.extend)(
                self,
                targets
                    .start
                    .map_or(*domain.start(), |target| target.parameter)
                    ..=targets.end.map_or(*domain.end(), |target| target.parameter),
            );
        }
        let start_extension = targets
            .start
            .map(|target| operations.segment(self, true, target.parameter))
            .transpose()?;
        let end_extension = targets
            .end
            .map(|target| operations.segment(self, false, target.parameter))
            .transpose()?;
        let mut source = self.clone();
        if start_extension.is_some() {
            source = (operations.clamp_endpoint)(&source, true)?;
        }
        if end_extension.is_some() {
            source = (operations.clamp_endpoint)(&source, false)?;
        }
        let mut result = if let Some(extension) = start_extension {
            extension.try_append_clamped(&source)?
        } else {
            source
        };
        if let Some(extension) = end_extension {
            result = result.try_append_clamped(&extension)?;
        }
        Ok(result)
    }

    fn apply_boundary_extension_lengths(
        &self,
        targets: &CurveBoundaryExtensionTargets,
        mut extend: impl FnMut(&Self, CurveExtensionSide, Real) -> Result<Self, GeometryError>,
    ) -> Result<Self, GeometryError> {
        let mut result = self.clone();
        if let Some(target) = targets.start {
            result = extend(&result, CurveExtensionSide::Start, target.length)?;
        }
        if let Some(target) = targets.end {
            result = extend(&result, CurveExtensionSide::End, target.length)?;
        }
        Ok(result)
    }
}

fn farthest_coordinate(origin: Real, minimum: Real, maximum: Real) -> Real {
    if (origin - minimum).abs() >= (maximum - origin).abs() {
        minimum
    } else {
        maximum
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[derive(Debug, PartialEq)]
    enum Failure {
        Geometry(GeometryError),
        Producer,
    }
    impl From<GeometryError> for Failure {
        fn from(error: GeometryError) -> Self {
            Self::Geometry(error)
        }
    }

    #[test]
    fn boundary_policy_propagates_each_producer_error_before_returning_geometry() {
        let point = |x, y| Point3::try_new(x, y, 0.).unwrap();
        let source = NurbsCurve::try_new(
            2,
            vec![point(0., 0.), point(2., 3.), point(4., 0.)],
            vec![0., 0., 0., 1., 1., 1.],
        )
        .unwrap();
        let before = source.clone();
        let boundary = CurveExtensionBoundary::Curve(
            NurbsCurve::try_new(
                1,
                vec![point(7., -20.), point(7., 20.)],
                vec![0., 0., 40., 40.],
            )
            .unwrap(),
        );
        for failed_stage in 0..3 {
            let calls = [Cell::new(0), Cell::new(0), Cell::new(0)];
            let mut extend = |curve: &NurbsCurve, interval| {
                calls[0].set(calls[0].get() + 1);
                if failed_stage == 0 {
                    Err(Failure::Producer)
                } else {
                    Ok(curve.try_extended_to(interval)?)
                }
            };
            let mut trim = |curve: &NurbsCurve, interval| {
                calls[1].set(calls[1].get() + 1);
                if failed_stage == 1 {
                    Err(Failure::Producer)
                } else {
                    Ok(curve.try_trimmed(interval)?)
                }
            };
            let mut clamp_endpoint = |_: &NurbsCurve, _: bool| {
                calls[2].set(calls[2].get() + 1);
                Err(Failure::Producer)
            };
            let mut operations = CurveExtensionOperations {
                extend: &mut extend,
                trim: &mut trim,
                clamp_endpoint: &mut clamp_endpoint,
            };
            assert_eq!(
                source.try_joined_to_boundaries_with_operations(
                    CurveExtensionSide::End,
                    CurveExtensionStyle::Smooth,
                    std::slice::from_ref(&boundary),
                    Tolerance::DEFAULT,
                    &mut operations,
                ),
                Err(Failure::Producer)
            );
            assert!(calls[failed_stage].get() > 0);
            assert_eq!(source, before);
            assert!(matches!(
                source.try_joined_to_boundaries_with_operations(
                    CurveExtensionSide::End,
                    CurveExtensionStyle::Smooth,
                    &[],
                    Tolerance::DEFAULT,
                    &mut operations,
                ),
                Err(Failure::Geometry(
                    GeometryError::EmptyCurveExtensionBoundaries
                ))
            ));
        }
    }
}
