use std::ops::RangeInclusive;

use crate::integration::integrate_adaptive;
use crate::nurbs::{
    clamped_uniform_knots, curve_points_coincident, de_boor, knot_vector_is_periodic,
    project_homogeneous, stable_divided_difference, validate_direction,
};
use crate::{
    AffineTransform3, BoundingBox3, Frame3, GeometryError, MAX_CURVE_DIVISION_POINTS, Point3, Real,
    Tolerance, TriangleMesh, UnitVector3, Vector3, WeightedPoint3, require_finite,
};

/// A finite tensor-product non-uniform rational B-spline surface.
///
/// Control points use row-major `(u, v)` order: `u` varies fastest and the
/// point at `(u, v)` is stored at `v * control_point_count_u + u`.
#[derive(Clone, Debug, PartialEq)]
pub struct NurbsSurface {
    degree_u: usize,
    degree_v: usize,
    control_point_count_u: usize,
    control_point_count_v: usize,
    control_points: Vec<WeightedPoint3>,
    knots_u: Vec<Real>,
    knots_v: Vec<Real>,
    rational: bool,
}

impl NurbsSurface {
    /// Constructs a non-rational surface whose control weights are all one.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<Point3>,
        knots_u: Vec<Real>,
        knots_v: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        let control_points = control_points
            .into_iter()
            .map(|point| WeightedPoint3::try_new(point, 1.0))
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
        )
    }

    /// Constructs a rational surface after validating both knot directions
    /// and every control point in the rectangular net.
    #[allow(clippy::too_many_arguments)]
    pub fn try_new_rational(
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<WeightedPoint3>,
        knots_u: Vec<Real>,
        knots_v: Vec<Real>,
    ) -> Result<Self, GeometryError> {
        validate_direction(degree_u, control_point_count_u, &knots_u)?;
        validate_direction(degree_v, control_point_count_v, &knots_v)?;
        let expected = control_point_count_u
            .checked_mul(control_point_count_v)
            .ok_or(GeometryError::InvalidControlNet {
                context: "control-point count overflowed usize",
            })?;
        if control_points.len() != expected {
            return Err(GeometryError::InvalidControlNetSize {
                expected,
                actual: control_points.len(),
            });
        }
        for (index, control_point) in control_points.iter().enumerate() {
            if !control_point.weight().is_finite() || control_point.weight() <= 0.0 {
                return Err(GeometryError::InvalidWeight { index });
            }
        }
        let first_weight = control_points[0].weight();
        let rational = control_points
            .iter()
            .any(|control_point| control_point.weight() != first_weight);
        Ok(Self {
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
            rational,
        })
    }

    /// Constructs a non-rational surface with open, clamped, uniformly spaced
    /// knots in both parametric directions.
    pub fn try_clamped_uniform(
        degree_u: usize,
        degree_v: usize,
        control_point_count_u: usize,
        control_point_count_v: usize,
        control_points: Vec<Point3>,
    ) -> Result<Self, GeometryError> {
        let knots_u = clamped_uniform_knots(degree_u, control_point_count_u)?;
        let knots_v = clamped_uniform_knots(degree_v, control_point_count_v)?;
        Self::try_new(
            degree_u,
            degree_v,
            control_point_count_u,
            control_point_count_v,
            control_points,
            knots_u,
            knots_v,
        )
    }

    /// Constructs the exact ruled surface swept by a NURBS curve between two
    /// translation offsets.
    ///
    /// The curve is the U direction. V is degree one and parameterized by the
    /// physical distance between the offsets, matching Rhino straight
    /// extrusions. Rational weights and the complete U knot vector are
    /// preserved exactly.
    pub fn try_extruded_curve(
        curve: &crate::NurbsCurve,
        start_offset: Vector3,
        end_offset: Vector3,
    ) -> Result<Self, GeometryError> {
        let path = Vector3::try_new(
            end_offset.x() - start_offset.x(),
            end_offset.y() - start_offset.y(),
            end_offset.z() - start_offset.z(),
        )?;
        let path_length = path.length()?;
        if path_length == 0.0 {
            return Err(GeometryError::Degenerate {
                context: "curve extrusion path",
            });
        }

        let control_count_u = curve.control_points().len();
        let control_count =
            control_count_u
                .checked_mul(2)
                .ok_or(GeometryError::InvalidControlNet {
                    context: "extruded control-point count overflowed usize",
                })?;
        let mut controls = Vec::new();
        controls.try_reserve_exact(control_count).map_err(|_| {
            GeometryError::InvalidControlNet {
                context: "extruded control net exceeds addressable memory",
            }
        })?;
        for offset in [start_offset, end_offset] {
            for control in curve.control_points() {
                controls.push(WeightedPoint3::try_new(
                    control.point().translated(offset)?,
                    control.weight(),
                )?);
            }
        }
        Self::try_new_rational(
            curve.degree(),
            1,
            control_count_u,
            2,
            controls,
            curve.knots().to_vec(),
            vec![0.0, 0.0, path_length, path_length],
        )
    }

    /// Constructs a bilinear surface from four perimeter-ordered corners.
    /// The order is first, adjacent second, opposite third, adjacent fourth.
    pub fn try_bilinear(corners: [Point3; 4]) -> Result<Self, GeometryError> {
        Self::try_new(
            1,
            1,
            2,
            2,
            vec![corners[0], corners[1], corners[3], corners[2]],
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
    }

    #[inline]
    pub const fn degree_u(&self) -> usize {
        self.degree_u
    }

    #[inline]
    pub const fn degree_v(&self) -> usize {
        self.degree_v
    }

    #[inline]
    pub const fn control_point_count_u(&self) -> usize {
        self.control_point_count_u
    }

    #[inline]
    pub const fn control_point_count_v(&self) -> usize {
        self.control_point_count_v
    }

    #[inline]
    pub fn control_points(&self) -> &[WeightedPoint3] {
        &self.control_points
    }

    pub fn control_point(&self, u: usize, v: usize) -> Option<WeightedPoint3> {
        (u < self.control_point_count_u && v < self.control_point_count_v)
            .then(|| self.control_points[self.control_index(u, v)])
    }

    #[inline]
    pub fn knots_u(&self) -> &[Real] {
        &self.knots_u
    }

    #[inline]
    pub fn knots_v(&self) -> &[Real] {
        &self.knots_v
    }

    #[inline]
    pub const fn is_rational(&self) -> bool {
        self.rational
    }

    /// Returns whether the U knot vector and repeated end controls form an
    /// OpenNURBS-style periodic surface direction.
    pub fn is_periodic_u(&self) -> bool {
        if !knot_vector_is_periodic(
            self.degree_u + 1,
            self.control_point_count_u,
            &self.knots_u[1..self.knots_u.len() - 1],
        ) {
            return false;
        }
        (0..self.control_point_count_v).all(|v| {
            (0..self.degree_u).all(|u| {
                let repeated = self.control_point_count_u - self.degree_u + u;
                curve_points_coincident(
                    self.control_points[self.control_index(u, v)].point(),
                    self.control_points[self.control_index(repeated, v)].point(),
                )
            })
        })
    }

    /// Returns whether the V knot vector and repeated end controls form an
    /// OpenNURBS-style periodic surface direction.
    pub fn is_periodic_v(&self) -> bool {
        if !knot_vector_is_periodic(
            self.degree_v + 1,
            self.control_point_count_v,
            &self.knots_v[1..self.knots_v.len() - 1],
        ) {
            return false;
        }
        (0..self.control_point_count_u).all(|u| {
            (0..self.degree_v).all(|v| {
                let repeated = self.control_point_count_v - self.degree_v + v;
                curve_points_coincident(
                    self.control_points[self.control_index(u, v)].point(),
                    self.control_points[self.control_index(u, repeated)].point(),
                )
            })
        })
    }

    /// Returns control-net locations in Rhino `ExtractPt` grip order. Repeated
    /// periodic controls and exact clamped closing seams are represented by a
    /// single grip in each direction.
    pub fn extract_point_locations(&self) -> Vec<Point3> {
        let periodic_u = self.is_periodic_u();
        let periodic_v = self.is_periodic_v();
        let repeated_u_seam = !periodic_u
            && knots_are_clamped(self.degree_u, &self.knots_u)
            && (0..self.control_point_count_v).all(|v| {
                self.control_points[self.control_index(0, v)].point()
                    == self.control_points[self.control_index(self.control_point_count_u - 1, v)]
                        .point()
            });
        let repeated_v_seam = !periodic_v
            && knots_are_clamped(self.degree_v, &self.knots_v)
            && (0..self.control_point_count_u).all(|u| {
                self.control_points[self.control_index(u, 0)].point()
                    == self.control_points[self.control_index(u, self.control_point_count_v - 1)]
                        .point()
            });
        let retained_u = self.control_point_count_u
            - if periodic_u {
                self.degree_u
            } else {
                usize::from(repeated_u_seam)
            };
        let retained_v = self.control_point_count_v
            - if periodic_v {
                self.degree_v
            } else {
                usize::from(repeated_v_seam)
            };
        let mut points = Vec::with_capacity(retained_u * retained_v);
        for u in 0..retained_u {
            for v in 0..retained_v {
                points.push(self.control_points[self.control_index(u, v)].point());
            }
        }
        points
    }

    pub fn domain_u(&self) -> RangeInclusive<Real> {
        self.knots_u[self.degree_u]..=self.knots_u[self.control_point_count_u]
    }

    pub fn domain_v(&self) -> RangeInclusive<Real> {
        self.knots_v[self.degree_v]..=self.knots_v[self.control_point_count_v]
    }

    pub fn spans_u(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        nonempty_spans(&self.knots_u, self.degree_u, self.control_point_count_u)
    }

    pub fn spans_v(&self) -> impl Iterator<Item = (Real, Real)> + '_ {
        nonempty_spans(&self.knots_v, self.degree_v, self.control_point_count_v)
    }

    pub fn control_point_bounds(&self) -> BoundingBox3 {
        BoundingBox3::from_points(
            self.control_points
                .iter()
                .map(|control_point| control_point.point()),
        )
        .expect("a valid NURBS surface has control points")
    }

    pub fn parameter_at_u(&self, normalized: Real) -> Result<Real, GeometryError> {
        normalized_parameter(normalized, self.domain_u())
    }

    pub fn parameter_at_v(&self, normalized: Real) -> Result<Real, GeometryError> {
        normalized_parameter(normalized, self.domain_v())
    }

    /// Evaluates a surface point with the tensor-product homogeneous de Boor
    /// algorithm.
    pub fn evaluate(&self, u: Real, v: Real) -> Result<Point3, GeometryError> {
        self.evaluate_homogeneous(u, v)
            .and_then(project_homogeneous)
    }

    /// Evaluates the polynomial/rational continuation of the first or last
    /// knot span when either parameter lies outside the natural domain.
    /// Surface space morphs use this continuation for source geometry that
    /// crosses a target surface edge, matching Rhino's splop behavior.
    pub fn evaluate_extended(&self, u: Real, v: Real) -> Result<Point3, GeometryError> {
        let span_u = extended_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?;
        let span_v = extended_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?;
        self.evaluate_homogeneous_at_spans(u, v, span_u, span_v)
            .and_then(project_homogeneous)
    }

    /// Evaluates a point and its exact first partial derivatives.
    pub fn evaluate_with_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let span_u = checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?;
        let span_v = checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?;
        self.evaluate_with_derivatives_at_spans(u, v, span_u, span_v)
    }

    /// Evaluates a surface continuation and its exact first partial
    /// derivatives outside the natural parameter domain.
    pub fn evaluate_extended_with_derivatives(
        &self,
        u: Real,
        v: Real,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let span_u = extended_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?;
        let span_v = extended_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?;
        self.evaluate_with_derivatives_at_spans(u, v, span_u, span_v)
    }

    fn evaluate_with_derivatives_at_spans(
        &self,
        u: Real,
        v: Real,
        span_u: usize,
        span_v: usize,
    ) -> Result<(Point3, Vector3, Vector3), GeometryError> {
        let active = self.active_homogeneous_control_net(span_u, span_v)?;
        let homogeneous = evaluate_tensor_product(
            &active,
            self.degree_u + 1,
            &self.knots_u,
            self.degree_u,
            span_u,
            u,
            &self.knots_v,
            self.degree_v,
            span_v,
            v,
        )?;
        let point = project_homogeneous(homogeneous)?;

        let derivative_u_controls =
            derivative_controls_u(&active, self.degree_u, self.degree_v, span_u, &self.knots_u)?;
        let homogeneous_u = evaluate_tensor_product(
            &derivative_u_controls,
            self.degree_u,
            &self.knots_u[1..self.knots_u.len() - 1],
            self.degree_u - 1,
            span_u - 1,
            u,
            &self.knots_v,
            self.degree_v,
            span_v,
            v,
        )?;

        let derivative_v_controls =
            derivative_controls_v(&active, self.degree_u, self.degree_v, span_v, &self.knots_v)?;
        let homogeneous_v = evaluate_tensor_product(
            &derivative_v_controls,
            self.degree_u + 1,
            &self.knots_u,
            self.degree_u,
            span_u,
            u,
            &self.knots_v[1..self.knots_v.len() - 1],
            self.degree_v - 1,
            span_v - 1,
            v,
        )?;

        let derivative_u = project_derivative(point, homogeneous, homogeneous_u)?;
        let derivative_v = project_derivative(point, homogeneous, homogeneous_v)?;
        Ok((point, derivative_u, derivative_v))
    }

    pub fn normal_at(
        &self,
        u: Real,
        v: Real,
        tolerance: Tolerance,
    ) -> Result<UnitVector3, GeometryError> {
        let (_, derivative_u, derivative_v) = self.evaluate_with_derivatives(u, v)?;
        derivative_u.cross(derivative_v)?.normalized(tolerance)
    }

    /// Evaluates the right-handed surface frame used by Rhino: x follows the
    /// positive U derivative, y is the component of the positive V derivative
    /// perpendicular to x, and z is the surface normal.
    pub fn frame_at(
        &self,
        u: Real,
        v: Real,
        tolerance: Tolerance,
    ) -> Result<Frame3, GeometryError> {
        let (point, derivative_u, derivative_v) = self.evaluate_with_derivatives(u, v)?;
        Frame3::try_from_directions(point, derivative_u, derivative_v, tolerance)
    }

    /// Finds natural surface parameters nearest to a finite model-space
    /// point. A bounded multi-start search followed by tangent-plane Newton
    /// refinement handles rational and non-uniform surfaces without assuming
    /// normalized parameter domains.
    pub fn closest_parameters(
        &self,
        target: Point3,
        tolerance: Tolerance,
    ) -> Result<(Real, Real), GeometryError> {
        let u_domain = self.domain_u();
        let v_domain = self.domain_v();
        let u_start = *u_domain.start();
        let u_end = *u_domain.end();
        let v_start = *v_domain.start();
        let v_end = *v_domain.end();
        let u_seeds = closest_parameter_seeds(self.spans_u(), u_start, u_end);
        let v_seeds = closest_parameter_seeds(self.spans_v(), v_start, v_end);
        let mut seeds = Vec::with_capacity(u_seeds.len() * v_seeds.len());
        for &v in &v_seeds {
            for &u in &u_seeds {
                if let Ok(point) = self.evaluate(u, v)
                    && let Ok(distance) = point.distance_to(target)
                {
                    seeds.push((distance, u, v));
                }
            }
        }
        seeds.sort_by(|left, right| left.0.total_cmp(&right.0));
        seeds.truncate(16);
        let mut best = seeds.first().copied().ok_or(GeometryError::Degenerate {
            context: "NURBS surface closest-point search",
        })?;
        for (_, seed_u, seed_v) in seeds {
            if let Ok((u, v, distance)) = self.refine_closest_parameters(
                target,
                seed_u,
                seed_v,
                [u_start, u_end],
                [v_start, v_end],
                tolerance,
            ) && distance < best.0
            {
                best = (distance, u, v);
            }
        }
        Ok((best.1, best.2))
    }

    fn refine_closest_parameters(
        &self,
        target: Point3,
        mut u: Real,
        mut v: Real,
        u_domain: [Real; 2],
        v_domain: [Real; 2],
        tolerance: Tolerance,
    ) -> Result<(Real, Real, Real), GeometryError> {
        let mut distance = self.evaluate(u, v)?.distance_to(target)?;
        for _ in 0..64 {
            let (point, derivative_u, derivative_v) = self.evaluate_with_derivatives(u, v)?;
            let residual = point.vector_to(target)?;
            let x_axis = derivative_u.normalized(tolerance)?;
            let u_speed = derivative_u.length()?;
            let v_along_x = derivative_v.dot(x_axis.as_vector())?;
            let derivative_v_values = derivative_v.to_array();
            let x_values = x_axis.as_vector().to_array();
            let v_perpendicular = Vector3::try_new(
                (-v_along_x).mul_add(x_values[0], derivative_v_values[0]),
                (-v_along_x).mul_add(x_values[1], derivative_v_values[1]),
                (-v_along_x).mul_add(x_values[2], derivative_v_values[2]),
            )?;
            let y_axis = v_perpendicular.normalized(tolerance)?;
            let v_speed = v_perpendicular.length()?;
            let tangent_x = residual.dot(x_axis.as_vector())?;
            let tangent_y = residual.dot(y_axis.as_vector())?;
            if tangent_x.hypot(tangent_y) <= tolerance.absolute() {
                break;
            }
            let delta_v = tangent_y / v_speed;
            let delta_u = tangent_x / u_speed - v_along_x * delta_v / u_speed;
            require_finite([delta_u, delta_v], "surface closest-point step")?;
            let mut step = 1.0;
            let mut accepted = None;
            for _ in 0..24 {
                let candidate_u = (u + step * delta_u).clamp(u_domain[0], u_domain[1]);
                let candidate_v = (v + step * delta_v).clamp(v_domain[0], v_domain[1]);
                if candidate_u == u && candidate_v == v {
                    break;
                }
                let candidate_distance = self
                    .evaluate(candidate_u, candidate_v)?
                    .distance_to(target)?;
                if candidate_distance <= distance {
                    accepted = Some((candidate_u, candidate_v, candidate_distance));
                    break;
                }
                step *= 0.5;
            }
            let Some((next_u, next_v, next_distance)) = accepted else {
                break;
            };
            u = next_u;
            v = next_v;
            distance = next_distance;
        }
        Ok((u, v, distance))
    }

    /// Divides the U-varying isocurve at `v` into equal arc-length segments
    /// and returns natural U parameters.
    pub fn divide_u_isocurve_by_count(
        &self,
        v: Real,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Real>, GeometryError> {
        self.divide_isocurve_by_count(
            SurfaceIsoDirection::U,
            v,
            segment_count,
            include_start,
            tolerance,
        )
    }

    /// Divides the V-varying isocurve at `u` into equal arc-length segments
    /// and returns natural V parameters.
    pub fn divide_v_isocurve_by_count(
        &self,
        u: Real,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Real>, GeometryError> {
        self.divide_isocurve_by_count(
            SurfaceIsoDirection::V,
            u,
            segment_count,
            include_start,
            tolerance,
        )
    }

    pub fn transformed(&self, transform: AffineTransform3) -> Result<Self, GeometryError> {
        let control_points = self
            .control_points
            .iter()
            .map(|control_point| {
                WeightedPoint3::try_new(
                    transform.transform_point(control_point.point())?,
                    control_point.weight(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::try_new_rational(
            self.degree_u,
            self.degree_v,
            self.control_point_count_u,
            self.control_point_count_v,
            control_points,
            self.knots_u.clone(),
            self.knots_v.clone(),
        )
    }

    /// Produces a regular display mesh inside every nonempty knot-span pair.
    /// Span boundaries are duplicated so a fully multiple knot cannot create
    /// triangles that bridge a discontinuity. Singular triangles, such as the
    /// collapsed row at a sphere pole, are omitted.
    pub fn tessellate(
        &self,
        samples_per_span: usize,
        tolerance: Tolerance,
    ) -> Result<TriangleMesh, GeometryError> {
        if samples_per_span == 0 {
            return Err(GeometryError::InvalidTessellationResolution);
        }
        let spans_u = self.spans_u().collect::<Vec<_>>();
        let spans_v = self.spans_v().collect::<Vec<_>>();
        let vertices_per_patch = samples_per_span
            .checked_add(1)
            .and_then(|side| side.checked_mul(side))
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let patch_count = spans_u
            .len()
            .checked_mul(spans_v.len())
            .ok_or(GeometryError::TooManyMeshVertices)?;
        let capacity = vertices_per_patch
            .checked_mul(patch_count)
            .ok_or(GeometryError::TooManyMeshVertices)?;
        if capacity > u32::MAX as usize {
            return Err(GeometryError::TooManyMeshVertices);
        }

        let mut vertices = Vec::with_capacity(capacity);
        let mut triangles = Vec::new();
        let domain_u_end = *self.domain_u().end();
        let domain_v_end = *self.domain_v().end();
        let side = samples_per_span + 1;
        for &(v_start, v_end) in &spans_v {
            for &(u_start, u_end) in &spans_u {
                let offset = u32::try_from(vertices.len())
                    .map_err(|_| GeometryError::TooManyMeshVertices)?;
                for v_sample in 0..=samples_per_span {
                    let v =
                        span_parameter(v_start, v_end, v_sample, samples_per_span, domain_v_end);
                    for u_sample in 0..=samples_per_span {
                        let u = span_parameter(
                            u_start,
                            u_end,
                            u_sample,
                            samples_per_span,
                            domain_u_end,
                        );
                        vertices.push(self.evaluate(u, v)?);
                    }
                }
                for row in 0..samples_per_span {
                    for column in 0..samples_per_span {
                        let local_lower_left = row
                            .checked_mul(side)
                            .and_then(|index| index.checked_add(column))
                            .and_then(|index| u32::try_from(index).ok())
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let lower_left = offset
                            .checked_add(local_lower_left)
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let lower_right = lower_left + 1;
                        let row_stride =
                            u32::try_from(side).map_err(|_| GeometryError::TooManyMeshVertices)?;
                        let upper_left = lower_left
                            .checked_add(row_stride)
                            .ok_or(GeometryError::TooManyMeshVertices)?;
                        let upper_right = upper_left + 1;
                        push_if_nondegenerate(
                            &vertices,
                            &mut triangles,
                            [lower_left, lower_right, upper_right],
                            tolerance,
                        )?;
                        push_if_nondegenerate(
                            &vertices,
                            &mut triangles,
                            [lower_left, upper_right, upper_left],
                            tolerance,
                        )?;
                    }
                }
            }
        }
        TriangleMesh::try_new(vertices, triangles, tolerance)
    }

    fn evaluate_homogeneous(&self, u: Real, v: Real) -> Result<[Real; 4], GeometryError> {
        let span_u = checked_span(self.degree_u, self.control_point_count_u, &self.knots_u, u)?;
        let span_v = checked_span(self.degree_v, self.control_point_count_v, &self.knots_v, v)?;
        self.evaluate_homogeneous_at_spans(u, v, span_u, span_v)
    }

    fn evaluate_homogeneous_at_spans(
        &self,
        u: Real,
        v: Real,
        span_u: usize,
        span_v: usize,
    ) -> Result<[Real; 4], GeometryError> {
        let active = self.active_homogeneous_control_net(span_u, span_v)?;
        evaluate_tensor_product(
            &active,
            self.degree_u + 1,
            &self.knots_u,
            self.degree_u,
            span_u,
            u,
            &self.knots_v,
            self.degree_v,
            span_v,
            v,
        )
    }

    fn active_homogeneous_control_net(
        &self,
        span_u: usize,
        span_v: usize,
    ) -> Result<Vec<[Real; 4]>, GeometryError> {
        let first_u = span_u - self.degree_u;
        let first_v = span_v - self.degree_v;
        let mut weight_scale: Real = 0.0;
        for local_v in 0..=self.degree_v {
            for local_u in 0..=self.degree_u {
                weight_scale = weight_scale.max(
                    self.control_points[self.control_index(first_u + local_u, first_v + local_v)]
                        .weight(),
                );
            }
        }
        let mut active = Vec::with_capacity((self.degree_u + 1) * (self.degree_v + 1));
        for local_v in 0..=self.degree_v {
            for local_u in 0..=self.degree_u {
                let control =
                    self.control_points[self.control_index(first_u + local_u, first_v + local_v)];
                let weight = control.weight() / weight_scale;
                let point = control.point();
                let homogeneous = [
                    point.x() * weight,
                    point.y() * weight,
                    point.z() * weight,
                    weight,
                ];
                require_finite(homogeneous, "homogeneous NURBS surface control point")?;
                active.push(homogeneous);
            }
        }
        Ok(active)
    }

    #[inline]
    fn control_index(&self, u: usize, v: usize) -> usize {
        v * self.control_point_count_u + u
    }

    fn divide_isocurve_by_count(
        &self,
        direction: SurfaceIsoDirection,
        constant_parameter: Real,
        segment_count: usize,
        include_start: bool,
        tolerance: Tolerance,
    ) -> Result<Vec<Real>, GeometryError> {
        if segment_count == 0 {
            return Err(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let point_count = segment_count
            .checked_add(usize::from(include_start))
            .ok_or(GeometryError::InvalidCurveDivisionCount {
                actual: segment_count,
                maximum: MAX_CURVE_DIVISION_POINTS,
            })?;
        if point_count > MAX_CURVE_DIVISION_POINTS {
            return Err(GeometryError::TooManyCurveDivisionPoints {
                maximum: MAX_CURVE_DIVISION_POINTS,
            });
        }
        let sampler =
            SurfaceIsoArcLengthSampler::try_new(self, direction, constant_parameter, tolerance)?;
        let first_index = usize::from(!include_start);
        let mut parameters = Vec::with_capacity(point_count);
        for index in first_index..=segment_count {
            let distance = if index == segment_count {
                sampler.total_length
            } else {
                sampler.total_length * (index as Real / segment_count as Real)
            };
            parameters.push(sampler.parameter_at_distance(distance)?);
        }
        Ok(parameters)
    }
}

#[derive(Clone, Copy)]
enum SurfaceIsoDirection {
    U,
    V,
}

#[derive(Clone, Copy)]
struct SurfaceIsoSpan {
    start: Real,
    end: Real,
    length: Real,
    cumulative_start: Real,
    cumulative_end: Real,
}

struct SurfaceIsoArcLengthSampler<'a> {
    surface: &'a NurbsSurface,
    direction: SurfaceIsoDirection,
    constant_parameter: Real,
    spans: Vec<SurfaceIsoSpan>,
    total_length: Real,
    tolerance: Tolerance,
}

impl<'a> SurfaceIsoArcLengthSampler<'a> {
    fn try_new(
        surface: &'a NurbsSurface,
        direction: SurfaceIsoDirection,
        constant_parameter: Real,
        tolerance: Tolerance,
    ) -> Result<Self, GeometryError> {
        require_finite([constant_parameter], "surface isocurve parameter")?;
        let raw_spans = match direction {
            SurfaceIsoDirection::U => {
                checked_span(
                    surface.degree_v,
                    surface.control_point_count_v,
                    &surface.knots_v,
                    constant_parameter,
                )?;
                surface.spans_u().collect::<Vec<_>>()
            }
            SurfaceIsoDirection::V => {
                checked_span(
                    surface.degree_u,
                    surface.control_point_count_u,
                    &surface.knots_u,
                    constant_parameter,
                )?;
                surface.spans_v().collect::<Vec<_>>()
            }
        };
        let mut spans = Vec::with_capacity(raw_spans.len());
        let mut sum = 0.0;
        let mut correction = 0.0;
        for (start, end) in raw_spans {
            let length = integrate_surface_speed(start, end, tolerance, |parameter| {
                let (_, derivative_u, derivative_v) = match direction {
                    SurfaceIsoDirection::U => {
                        surface.evaluate_with_derivatives(parameter, constant_parameter)?
                    }
                    SurfaceIsoDirection::V => {
                        surface.evaluate_with_derivatives(constant_parameter, parameter)?
                    }
                };
                match direction {
                    SurfaceIsoDirection::U => derivative_u.length(),
                    SurfaceIsoDirection::V => derivative_v.length(),
                }
            })?;
            if length == 0.0 {
                continue;
            }
            let cumulative_start = sum + correction;
            compensated_add(&mut sum, &mut correction, length);
            let cumulative_end = sum + correction;
            spans.push(SurfaceIsoSpan {
                start,
                end,
                length,
                cumulative_start,
                cumulative_end,
            });
        }
        let total_length = sum + correction;
        require_finite([total_length], "surface isocurve length")?;
        if spans.is_empty() || total_length <= 0.0 {
            return Err(GeometryError::Degenerate {
                context: "surface isocurve",
            });
        }
        Ok(Self {
            surface,
            direction,
            constant_parameter,
            spans,
            total_length,
            tolerance,
        })
    }

    fn parameter_at_distance(&self, distance: Real) -> Result<Real, GeometryError> {
        require_finite([distance], "surface isocurve arc-length distance")?;
        if distance < 0.0 || distance > self.total_length {
            return Err(GeometryError::ArcLengthOutOfDomain {
                distance,
                length: self.total_length,
            });
        }
        if distance == 0.0 {
            return Ok(self.spans[0].start);
        }
        if distance == self.total_length {
            return Ok(self.spans.last().expect("an isocurve has spans").end);
        }
        let span = self.spans[self
            .spans
            .partition_point(|span| span.cumulative_end < distance)
            .min(self.spans.len() - 1)];
        let target = (distance - span.cumulative_start).clamp(0.0, span.length);
        if target == 0.0 {
            return Ok(span.start);
        }
        if target == span.length {
            return Ok(span.end);
        }
        let distance_tolerance = surface_distance_tolerance(span.length, self.tolerance);
        let mut lower = span.start;
        let mut upper = span.end;
        let mut parameter = stable_surface_lerp(span.start, span.end, target / span.length);
        for _ in 0..80 {
            let length = integrate_surface_speed(
                span.start,
                parameter,
                Tolerance::try_new(
                    distance_tolerance,
                    self.tolerance.relative(),
                    self.tolerance.angular(),
                )?,
                |value| self.speed(value),
            )?;
            let residual = length - target;
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
            parameter = (speed > 0.0)
                .then(|| parameter - residual / speed)
                .filter(|candidate| {
                    candidate.is_finite() && *candidate > lower && *candidate < upper
                })
                .unwrap_or(midpoint);
        }
        Err(GeometryError::NumericalIntegrationDidNotConverge)
    }

    fn speed(&self, parameter: Real) -> Result<Real, GeometryError> {
        let (_, derivative_u, derivative_v) = match self.direction {
            SurfaceIsoDirection::U => self
                .surface
                .evaluate_with_derivatives(parameter, self.constant_parameter)?,
            SurfaceIsoDirection::V => self
                .surface
                .evaluate_with_derivatives(self.constant_parameter, parameter)?,
        };
        match self.direction {
            SurfaceIsoDirection::U => derivative_u.length(),
            SurfaceIsoDirection::V => derivative_v.length(),
        }
    }
}

fn integrate_surface_speed(
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
    let tighter = surface_distance_tolerance(coarse, tolerance);
    if tighter < tolerance.absolute() {
        integrate_adaptive(start, end, tighter, tolerance.relative(), speed)
    } else {
        Ok(coarse)
    }
}

fn surface_distance_tolerance(length: Real, tolerance: Tolerance) -> Real {
    let relative = tolerance.relative() * length.abs();
    let roundoff = 64.0 * Real::EPSILON * length.abs();
    tolerance
        .absolute()
        .min(relative)
        .max(roundoff)
        .max(Real::MIN_POSITIVE)
}

fn stable_surface_lerp(start: Real, end: Real, fraction: Real) -> Real {
    start.mul_add(1.0 - fraction, end * fraction)
}

fn compensated_add(sum: &mut Real, correction: &mut Real, value: Real) {
    let next = *sum + value;
    if sum.abs() >= value.abs() {
        *correction += (*sum - next) + value;
    } else {
        *correction += (value - next) + *sum;
    }
    *sum = next;
}

fn knots_are_clamped(degree: usize, knots: &[Real]) -> bool {
    knots[..=degree].iter().all(|knot| *knot == knots[0])
        && knots[knots.len() - degree - 1..]
            .iter()
            .all(|knot| *knot == knots[knots.len() - 1])
}

#[allow(clippy::too_many_arguments)]
fn evaluate_tensor_product(
    controls: &[[Real; 4]],
    row_width: usize,
    knots_u: &[Real],
    degree_u: usize,
    span_u: usize,
    u: Real,
    knots_v: &[Real],
    degree_v: usize,
    span_v: usize,
    v: Real,
) -> Result<[Real; 4], GeometryError> {
    debug_assert_eq!(controls.len(), row_width * (degree_v + 1));
    let mut evaluated_u = Vec::with_capacity(degree_v + 1);
    for row in controls.chunks_exact(row_width) {
        evaluated_u.push(de_boor(knots_u, degree_u, span_u, u, row.to_vec())?);
    }
    de_boor(knots_v, degree_v, span_v, v, evaluated_u)
}

fn derivative_controls_u(
    controls: &[[Real; 4]],
    degree_u: usize,
    degree_v: usize,
    span_u: usize,
    knots_u: &[Real],
) -> Result<Vec<[Real; 4]>, GeometryError> {
    let first_u = span_u - degree_u;
    let source_width = degree_u + 1;
    let mut result = Vec::with_capacity(degree_u * (degree_v + 1));
    for row in controls.chunks_exact(source_width) {
        for local_u in 0..degree_u {
            let index = first_u + local_u;
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    row[local_u + 1][coordinate],
                    row[local_u][coordinate],
                    degree_u,
                    knots_u[index + 1],
                    knots_u[index + degree_u + 1],
                )?;
            }
            result.push(derivative);
        }
    }
    Ok(result)
}

fn derivative_controls_v(
    controls: &[[Real; 4]],
    degree_u: usize,
    degree_v: usize,
    span_v: usize,
    knots_v: &[Real],
) -> Result<Vec<[Real; 4]>, GeometryError> {
    let first_v = span_v - degree_v;
    let row_width = degree_u + 1;
    let mut result = Vec::with_capacity(row_width * degree_v);
    for local_v in 0..degree_v {
        let index = first_v + local_v;
        for local_u in 0..row_width {
            let lower = controls[local_v * row_width + local_u];
            let upper = controls[(local_v + 1) * row_width + local_u];
            let mut derivative = [0.0; 4];
            for coordinate in 0..4 {
                derivative[coordinate] = stable_divided_difference(
                    upper[coordinate],
                    lower[coordinate],
                    degree_v,
                    knots_v[index + 1],
                    knots_v[index + degree_v + 1],
                )?;
            }
            result.push(derivative);
        }
    }
    Ok(result)
}

fn project_derivative(
    point: Point3,
    homogeneous: [Real; 4],
    derivative: [Real; 4],
) -> Result<Vector3, GeometryError> {
    let weight = homogeneous[3];
    let weight_derivative = derivative[3];
    let point = point.to_array();
    let projected = std::array::from_fn(|coordinate| {
        (-point[coordinate]).mul_add(weight_derivative, derivative[coordinate]) / weight
    });
    Vector3::try_from(projected)
}

fn checked_span(
    degree: usize,
    control_point_count: usize,
    knots: &[Real],
    parameter: Real,
) -> Result<usize, GeometryError> {
    require_finite([parameter], "NURBS surface parameter")?;
    let start = knots[degree];
    let end = knots[control_point_count];
    if parameter < start || parameter > end {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter,
            domain_start: start,
            domain_end: end,
        });
    }
    extended_span(degree, control_point_count, knots, parameter)
}

fn extended_span(
    degree: usize,
    control_point_count: usize,
    knots: &[Real],
    parameter: Real,
) -> Result<usize, GeometryError> {
    require_finite([parameter], "NURBS surface parameter")?;
    let last_control = control_point_count - 1;
    if parameter >= knots[last_control + 1] {
        return Ok(last_control);
    }
    if parameter <= knots[degree] {
        return Ok(degree);
    }
    let mut low = degree;
    let mut high = last_control + 1;
    let mut middle = (low + high) / 2;
    while parameter < knots[middle] || parameter >= knots[middle + 1] {
        if parameter < knots[middle] {
            high = middle;
        } else {
            low = middle;
        }
        middle = (low + high) / 2;
    }
    Ok(middle)
}

fn nonempty_spans(
    knots: &[Real],
    degree: usize,
    control_point_count: usize,
) -> impl Iterator<Item = (Real, Real)> + '_ {
    knots
        .windows(2)
        .skip(degree)
        .take(control_point_count - degree)
        .filter_map(|pair| (pair[0] < pair[1]).then_some((pair[0], pair[1])))
}

fn closest_parameter_seeds(
    spans: impl Iterator<Item = (Real, Real)>,
    domain_start: Real,
    domain_end: Real,
) -> Vec<Real> {
    const MAX_SEEDS: usize = 33;
    let spans = spans.collect::<Vec<_>>();
    let mut seeds = Vec::new();
    if spans.len() <= 10 {
        for (start, end) in spans {
            seeds.extend([start, start * 0.5 + end * 0.5, end]);
        }
    }
    let remaining = MAX_SEEDS.saturating_sub(seeds.len()).max(2);
    for index in 0..remaining {
        let fraction = index as Real / (remaining - 1) as Real;
        seeds.push(domain_start.mul_add(1.0 - fraction, domain_end * fraction));
    }
    seeds.sort_by(Real::total_cmp);
    seeds.dedup();
    seeds
}

fn normalized_parameter(
    normalized: Real,
    domain: RangeInclusive<Real>,
) -> Result<Real, GeometryError> {
    if !normalized.is_finite() {
        return Err(GeometryError::NonFinite {
            context: "normalized NURBS surface parameter",
        });
    }
    if !(0.0..=1.0).contains(&normalized) {
        return Err(GeometryError::ParameterOutOfDomain {
            parameter: normalized,
            domain_start: 0.0,
            domain_end: 1.0,
        });
    }
    let parameter = domain
        .start()
        .mul_add(1.0 - normalized, domain.end() * normalized);
    require_finite([parameter], "NURBS surface parameter")?;
    Ok(parameter)
}

fn span_parameter(
    start: Real,
    end: Real,
    sample: usize,
    sample_count: usize,
    domain_end: Real,
) -> Real {
    let fraction = sample as Real / sample_count as Real;
    let parameter = start.mul_add(1.0 - fraction, end * fraction);
    if sample == sample_count && end < domain_end {
        parameter.next_down().max(start)
    } else {
        parameter
    }
}

fn push_if_nondegenerate(
    vertices: &[Point3],
    triangles: &mut Vec<[u32; 3]>,
    triangle: [u32; 3],
    tolerance: Tolerance,
) -> Result<(), GeometryError> {
    let points = triangle.map(|index| vertices[index as usize]);
    let Ok(first) = points[0].vector_to(points[1])?.normalized(tolerance) else {
        return Ok(());
    };
    let Ok(second) = points[0].vector_to(points[2])?.normalized(tolerance) else {
        return Ok(());
    };
    if first.as_vector().cross(second.as_vector())?.length()? > tolerance.angular() {
        triangles.push(triangle);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(x: Real, y: Real, z: Real) -> Point3 {
        Point3::try_new(x, y, z).unwrap()
    }

    fn assert_point_near(actual: Point3, expected: Point3) {
        assert!(actual.is_near(
            expected,
            Tolerance::try_new(1.0e-11, 1.0e-12, 1.0e-12).unwrap()
        ));
    }

    #[test]
    fn bilinear_surface_interpolates_corners_and_has_exact_partials() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(4.0, 0.0, 0.0),
            point(4.0, 2.0, 2.0),
            point(0.0, 2.0, 2.0),
        ])
        .unwrap();
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), point(0.0, 0.0, 0.0));
        assert_eq!(surface.evaluate(1.0, 1.0).unwrap(), point(4.0, 2.0, 2.0));
        let (center, derivative_u, derivative_v) =
            surface.evaluate_with_derivatives(0.5, 0.5).unwrap();
        assert_eq!(center, point(2.0, 1.0, 1.0));
        assert_eq!(derivative_u, Vector3::try_new(4.0, 0.0, 0.0).unwrap());
        assert_eq!(derivative_v, Vector3::try_new(0.0, 2.0, 2.0).unwrap());
        let normal = surface.normal_at(0.5, 0.5, Tolerance::DEFAULT).unwrap();
        assert!(normal.y() < 0.0 && normal.z() > 0.0);
    }

    #[test]
    fn rational_surface_represents_an_exact_quarter_cylinder() {
        let middle_weight = 0.5_f64.sqrt();
        let mut controls = Vec::new();
        for z in [0.0, 3.0] {
            controls.extend([
                WeightedPoint3::try_new(point(1.0, 0.0, z), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0, z), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0, z), 1.0).unwrap(),
            ]);
        }
        let surface = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let coordinate = 0.5_f64.sqrt();
        let (midpoint, tangent, vertical) = surface.evaluate_with_derivatives(0.5, 0.25).unwrap();
        assert_point_near(midpoint, point(coordinate, coordinate, 0.75));
        assert!(Tolerance::DEFAULT.approx_eq(midpoint.x().hypot(midpoint.y()), 1.0));
        assert!(
            Tolerance::DEFAULT.approx_eq(
                Vector3::try_new(midpoint.x(), midpoint.y(), 0.0)
                    .unwrap()
                    .dot(tangent)
                    .unwrap(),
                0.0
            )
        );
        assert!(Tolerance::DEFAULT.approx_eq(vertical.x(), 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(vertical.y(), 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(vertical.z(), 3.0));
    }

    #[test]
    fn exact_curve_extrusion_preserves_u_data_and_uses_path_length_for_v() {
        let middle_weight = 0.5_f64.sqrt();
        let curve = crate::NurbsCurve::try_new_rational(
            2,
            vec![
                WeightedPoint3::try_new(point(1.0, 0.0, 0.0), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0, 0.0), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0, 0.0), 1.0).unwrap(),
            ],
            vec![2.0, 2.0, 2.0, 7.0, 7.0, 7.0],
        )
        .unwrap();
        let surface = NurbsSurface::try_extruded_curve(
            &curve,
            Vector3::try_new(0.0, 0.0, -2.0).unwrap(),
            Vector3::try_new(0.0, 0.0, 3.0).unwrap(),
        )
        .unwrap();
        assert_eq!(surface.degree_u(), 2);
        assert_eq!(surface.degree_v(), 1);
        assert_eq!(surface.control_point_count_u(), 3);
        assert_eq!(surface.control_point_count_v(), 2);
        assert_eq!(surface.knots_u(), curve.knots());
        assert_eq!(surface.knots_v(), &[0.0, 0.0, 5.0, 5.0]);
        assert!(surface.is_rational());
        for u in [2.0, 3.25, 7.0] {
            let base = curve.evaluate(u).unwrap();
            assert_point_near(
                surface.evaluate(u, 0.0).unwrap(),
                base.translated(Vector3::try_new(0.0, 0.0, -2.0).unwrap())
                    .unwrap(),
            );
            assert_point_near(
                surface.evaluate(u, 5.0).unwrap(),
                base.translated(Vector3::try_new(0.0, 0.0, 3.0).unwrap())
                    .unwrap(),
            );
        }
        let (_, _, derivative_v) = surface.evaluate_with_derivatives(4.0, 2.5).unwrap();
        assert_eq!(derivative_v, Vector3::try_new(0.0, 0.0, 1.0).unwrap());

        let zero = Vector3::try_new(0.0, 0.0, 0.0).unwrap();
        assert!(NurbsSurface::try_extruded_curve(&curve, zero, zero).is_err());
    }

    #[test]
    fn surface_frames_and_isocurve_division_match_a_quarter_cylinder() {
        let middle_weight = 0.5_f64.sqrt();
        let mut controls = Vec::new();
        for z in [0.0, 3.0] {
            controls.extend([
                WeightedPoint3::try_new(point(1.0, 0.0, z), 1.0).unwrap(),
                WeightedPoint3::try_new(point(1.0, 1.0, z), middle_weight).unwrap(),
                WeightedPoint3::try_new(point(0.0, 1.0, z), 1.0).unwrap(),
            ]);
        }
        let surface = NurbsSurface::try_new_rational(
            2,
            1,
            3,
            2,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();

        let parameters = surface
            .divide_u_isocurve_by_count(0.25, 3, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(parameters.len(), 4);
        for (parameter, angle) in parameters.into_iter().zip([
            0.0,
            std::f64::consts::FRAC_PI_6,
            std::f64::consts::FRAC_PI_3,
            std::f64::consts::FRAC_PI_2,
        ]) {
            let actual = surface.evaluate(parameter, 0.25).unwrap();
            assert_point_near(actual, point(angle.cos(), angle.sin(), 0.75));
        }
        let v_parameters = surface
            .divide_v_isocurve_by_count(0.37, 2, true, Tolerance::DEFAULT)
            .unwrap();
        assert_eq!(v_parameters, vec![0.0, 0.5, 1.0]);

        let frame = surface.frame_at(0.5, 0.25, Tolerance::DEFAULT).unwrap();
        assert_point_near(frame.origin(), surface.evaluate(0.5, 0.25).unwrap());
        assert!(frame.x_axis().x() < 0.0 && frame.x_axis().y() > 0.0);
        assert!(frame.y_axis().z() > 0.0);
        assert!(frame.z_axis().x() > 0.0 && frame.z_axis().y() > 0.0);

        let target = surface
            .evaluate(0.37, 0.62)
            .unwrap()
            .translated(
                surface
                    .normal_at(0.37, 0.62, Tolerance::DEFAULT)
                    .unwrap()
                    .as_vector()
                    .scaled(2.0)
                    .unwrap(),
            )
            .unwrap();
        let (closest_u, closest_v) = surface
            .closest_parameters(target, Tolerance::DEFAULT)
            .unwrap();
        assert!((closest_u - 0.37).abs() <= 1.0e-8, "closest_u={closest_u}");
        assert!((closest_v - 0.62).abs() <= 1.0e-8, "closest_v={closest_v}");
    }

    #[test]
    fn isocurve_division_uses_the_requested_surface_edge() {
        let surface = NurbsSurface::try_new(
            2,
            1,
            3,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(5.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(0.0, 10.0, 10.0),
                point(0.0, 20.0, 10.0),
                point(10.0, 10.0, 10.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let u = surface
            .divide_u_isocurve_by_count(0.0, 3, true, Tolerance::DEFAULT)
            .unwrap();
        let v = surface
            .divide_v_isocurve_by_count(0.0, 2, true, Tolerance::DEFAULT)
            .unwrap();
        for (actual, expected) in u.into_iter().zip([0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0]) {
            assert!(Tolerance::DEFAULT.approx_eq(actual, expected));
        }
        assert_eq!(v, vec![0.0, 0.5, 1.0]);
    }

    #[test]
    fn tensor_evaluation_is_symmetric_when_the_quadratic_direction_is_v() {
        let middle_weight = 0.5_f64.sqrt();
        let controls = vec![
            WeightedPoint3::try_new(point(1.0, 0.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 0.0, 3.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0, 0.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(1.0, 1.0, 3.0), middle_weight).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0, 0.0), 1.0).unwrap(),
            WeightedPoint3::try_new(point(0.0, 1.0, 3.0), 1.0).unwrap(),
        ];
        let surface = NurbsSurface::try_new_rational(
            1,
            2,
            2,
            3,
            controls,
            vec![0.0, 0.0, 1.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let coordinate = 0.5_f64.sqrt();
        let (midpoint, axial, tangent) = surface.evaluate_with_derivatives(0.25, 0.5).unwrap();
        assert_point_near(midpoint, point(coordinate, coordinate, 0.75));
        assert!(Tolerance::DEFAULT.approx_eq(axial.x(), 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(axial.y(), 0.0));
        assert!(Tolerance::DEFAULT.approx_eq(axial.z(), 3.0));
        assert!(
            Tolerance::DEFAULT.approx_eq(
                Vector3::try_new(midpoint.x(), midpoint.y(), 0.0)
                    .unwrap()
                    .dot(tangent)
                    .unwrap(),
                0.0
            )
        );
    }

    #[test]
    fn uniformly_scaling_surface_weights_does_not_change_evaluation() {
        let points = [
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(0.0, 3.0, 1.0),
            point(2.0, 3.0, 1.0),
        ];
        let make_surface = |scale: Real| {
            NurbsSurface::try_new_rational(
                1,
                1,
                2,
                2,
                points
                    .into_iter()
                    .zip([1.0, 0.25, 2.0, 0.5])
                    .map(|(point, weight)| WeightedPoint3::try_new(point, weight * scale).unwrap())
                    .collect(),
                vec![0.0, 0.0, 1.0, 1.0],
                vec![0.0, 0.0, 1.0, 1.0],
            )
            .unwrap()
        };
        assert_point_near(
            make_surface(1.0).evaluate(0.37, 0.64).unwrap(),
            make_surface(1.0e200).evaluate(0.37, 0.64).unwrap(),
        );
    }

    #[test]
    fn analytic_partials_match_centered_differences_on_a_rational_patch() {
        let controls = [
            (point(0.0, 0.0, 0.0), 1.0),
            (point(1.0, 0.0, 1.0), 0.7),
            (point(2.0, 0.0, 0.5), 1.2),
            (point(0.0, 1.0, 0.5), 0.8),
            (point(1.0, 1.0, 2.0), 1.5),
            (point(2.0, 1.0, 1.0), 0.9),
            (point(0.0, 2.0, 0.0), 1.1),
            (point(1.0, 2.0, 0.75), 0.6),
            (point(2.0, 2.0, 0.0), 1.0),
        ]
        .into_iter()
        .map(|(point, weight)| WeightedPoint3::try_new(point, weight).unwrap())
        .collect();
        let surface = NurbsSurface::try_new_rational(
            2,
            2,
            3,
            3,
            controls,
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        )
        .unwrap();
        let u = 0.37;
        let v = 0.46;
        let step = 1.0e-6;
        let (_, analytic_u, analytic_v) = surface.evaluate_with_derivatives(u, v).unwrap();
        let difference = |negative: Point3, positive: Point3| {
            Vector3::try_new(
                (positive.x() - negative.x()) / (2.0 * step),
                (positive.y() - negative.y()) / (2.0 * step),
                (positive.z() - negative.z()) / (2.0 * step),
            )
            .unwrap()
        };
        let numeric_u = difference(
            surface.evaluate(u - step, v).unwrap(),
            surface.evaluate(u + step, v).unwrap(),
        );
        let numeric_v = difference(
            surface.evaluate(u, v - step).unwrap(),
            surface.evaluate(u, v + step).unwrap(),
        );
        let tolerance = Tolerance::try_new(1.0e-7, 1.0e-7, 1.0e-9).unwrap();
        for (analytic, numeric) in [(analytic_u, numeric_u), (analytic_v, numeric_v)] {
            assert!(tolerance.approx_eq(analytic.x(), numeric.x()));
            assert!(tolerance.approx_eq(analytic.y(), numeric.y()));
            assert!(tolerance.approx_eq(analytic.z(), numeric.z()));
        }
    }

    #[test]
    fn validates_control_net_and_direction_structure() {
        let corners = vec![point(0.0, 0.0, 0.0); 3];
        assert!(matches!(
            NurbsSurface::try_new(
                1,
                1,
                2,
                2,
                corners,
                vec![0.0, 0.0, 1.0, 1.0],
                vec![0.0, 0.0, 1.0, 1.0],
            ),
            Err(GeometryError::InvalidControlNetSize {
                expected: 4,
                actual: 3
            })
        ));
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(1.0, 0.0, 0.0),
            point(1.0, 1.0, 0.0),
            point(0.0, 1.0, 0.0),
        ])
        .unwrap();
        assert!(matches!(
            surface.evaluate(-0.1, 0.5),
            Err(GeometryError::ParameterOutOfDomain { .. })
        ));
        assert!(surface.evaluate(0.5, Real::NAN).is_err());
    }

    #[test]
    fn clamped_uniform_surface_has_expected_knots_and_corners() {
        let controls = (0..3)
            .flat_map(|v| (0..4).map(move |u| point(u as Real, v as Real, (u * v) as Real)))
            .collect::<Vec<_>>();
        let surface = NurbsSurface::try_clamped_uniform(2, 2, 4, 3, controls.clone()).unwrap();
        assert_eq!(surface.knots_u(), &[0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]);
        assert_eq!(surface.knots_v(), &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
        assert_eq!(surface.evaluate(0.0, 0.0).unwrap(), controls[0]);
        assert_eq!(surface.evaluate(1.0, 1.0).unwrap(), controls[11]);
    }

    #[test]
    fn affine_transform_preserves_surface_evaluation() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(2.0, 0.0, 0.0),
            point(2.0, 3.0, 1.0),
            point(0.0, 3.0, 1.0),
        ])
        .unwrap();
        let transform = AffineTransform3::try_new(
            [[2.0, -1.0, 0.0], [0.5, 3.0, 0.0], [0.0, 0.0, 4.0]],
            Vector3::try_new(4.0, -2.0, 7.0).unwrap(),
        )
        .unwrap();
        let transformed = surface.transformed(transform).unwrap();
        assert_point_near(
            transformed.evaluate(0.37, 0.64).unwrap(),
            transform
                .transform_point(surface.evaluate(0.37, 0.64).unwrap())
                .unwrap(),
        );
        assert_eq!(transformed.knots_u(), surface.knots_u());
        assert_eq!(transformed.knots_v(), surface.knots_v());
    }

    #[test]
    fn extracts_closed_and_periodic_surface_grips_without_repeated_seams() {
        let periodic_u = NurbsSurface::try_new(
            2,
            1,
            5,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(2.0, 0.0, 0.0),
                point(0.0, 0.0, 3.0),
                point(2.0, 0.0, 3.0),
                point(1.0, 2.0, 3.0),
                point(0.0, 0.0, 3.0),
                point(2.0, 0.0, 3.0),
            ],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(periodic_u.is_periodic_u());
        assert!(!periodic_u.is_periodic_v());
        assert_eq!(
            periodic_u.extract_point_locations(),
            vec![
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 3.0),
                point(2.0, 0.0, 0.0),
                point(2.0, 0.0, 3.0),
                point(1.0, 2.0, 0.0),
                point(1.0, 2.0, 3.0),
            ]
        );

        let closed_u = NurbsSurface::try_new(
            2,
            1,
            4,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 2.0, 0.0),
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 4.0),
                point(3.0, 0.0, 4.0),
                point(3.0, 2.0, 4.0),
                point(0.0, 0.0, 4.0),
            ],
            vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        assert!(!closed_u.is_periodic_u());
        assert_eq!(
            closed_u.extract_point_locations(),
            vec![
                point(0.0, 0.0, 0.0),
                point(0.0, 0.0, 4.0),
                point(3.0, 0.0, 0.0),
                point(3.0, 0.0, 4.0),
                point(3.0, 2.0, 0.0),
                point(3.0, 2.0, 4.0),
            ]
        );

        let periodic_v = NurbsSurface::try_new(
            1,
            2,
            2,
            5,
            vec![
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(3.0, 2.0, 0.0),
                point(0.0, 1.0, 2.0),
                point(3.0, 1.0, 2.0),
                point(0.0, 0.0, 0.0),
                point(3.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(3.0, 2.0, 0.0),
            ],
            vec![0.0, 0.0, 1.0, 1.0],
            vec![-2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
        )
        .unwrap();
        assert!(!periodic_v.is_periodic_u());
        assert!(periodic_v.is_periodic_v());
        assert_eq!(periodic_v.extract_point_locations().len(), 6);
    }

    #[test]
    fn tessellation_is_oriented_and_does_not_bridge_full_knot_breaks() {
        let surface = NurbsSurface::try_new(
            1,
            1,
            4,
            2,
            vec![
                point(0.0, 0.0, 0.0),
                point(1.0, 0.0, 0.0),
                point(10.0, 0.0, 0.0),
                point(11.0, 0.0, 0.0),
                point(0.0, 2.0, 0.0),
                point(1.0, 2.0, 0.0),
                point(10.0, 2.0, 0.0),
                point(11.0, 2.0, 0.0),
            ],
            vec![0.0, 0.0, 0.5, 0.5, 1.0, 1.0],
            vec![0.0, 0.0, 1.0, 1.0],
        )
        .unwrap();
        let mesh = surface.tessellate(1, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.triangles().len(), 4);
        for triangle_index in 0..mesh.triangles().len() {
            let points = mesh.triangle_points(triangle_index).unwrap();
            let minimum = points
                .iter()
                .map(|point| point.x())
                .fold(Real::INFINITY, Real::min);
            let maximum = points
                .iter()
                .map(|point| point.x())
                .fold(Real::NEG_INFINITY, Real::max);
            assert!(maximum - minimum <= 1.0 + Tolerance::DEFAULT.absolute());
            assert_eq!(mesh.face_normal(triangle_index).unwrap().z(), 1.0);
        }
    }

    #[test]
    fn rejects_zero_tessellation_resolution_and_degenerate_surface() {
        let surface = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
        ])
        .unwrap();
        assert_eq!(
            surface.tessellate(0, Tolerance::DEFAULT),
            Err(GeometryError::InvalidTessellationResolution)
        );
        assert_eq!(
            surface.tessellate(1, Tolerance::DEFAULT),
            Err(GeometryError::EmptyMesh)
        );

        let singular_boundary = NurbsSurface::try_bilinear([
            point(0.0, 0.0, 0.0),
            point(0.0, 0.0, 0.0),
            point(2.0, 2.0, 0.0),
            point(0.0, 2.0, 0.0),
        ])
        .unwrap();
        let mesh = singular_boundary.tessellate(1, Tolerance::DEFAULT).unwrap();
        assert_eq!(mesh.triangles().len(), 1);
        assert_eq!(mesh.face_normal(0).unwrap().z(), 1.0);
    }
}
